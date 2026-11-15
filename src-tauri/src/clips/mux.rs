//! A clip's compressed samples into an MP4, as they are (the sink writer's passthrough: each
//! stream's input type is its encoder's output type, nothing is encoded again).

use std::path::Path;
use std::sync::Arc;

use windows::Win32::Media::MediaFoundation::{
    IMFMediaType, MF_TRANSCODE_CONTAINERTYPE, MFCreateAttributes, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample, MFCreateSinkWriterFromURL,
    MFGetAttributesAsBlob, MFGetAttributesAsBlobSize, MFInitAttributesFromBlob, MFSampleExtension_CleanPoint, MFTranscodeContainerType_MPEG4,
};
#[cfg(test)]
use windows::Win32::Media::MediaFoundation::{MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_SOURCE_READERF_ENDOFSTREAM, MFCreateSourceReaderFromURL};
use windows::core::{Error, HSTRING, Result};

/// One compressed sample (100 ns units).
#[derive(Clone)]
pub struct Sample {
    pub time: i64,
    pub duration: i64,
    pub key: bool,
    pub data: Arc<[u8]>,
}

/// One stream: its media type (a blob, `type_blob`), its samples in time order, and for H.264
/// the SPS/PPS seen earlier in the stream, for a clip whose own samples carry none.
pub struct Track {
    pub kind: Vec<u8>,
    pub samples: Vec<Sample>,
    pub header: Option<Vec<u8>>,
}

/// A media type as bytes, to carry it to another thread and back (`MFInitAttributesFromBlob`).
pub fn type_blob(t: &IMFMediaType) -> Result<Vec<u8>> {
    // SAFETY: sizes, then fills, a buffer of that size.
    unsafe {
        let size = MFGetAttributesAsBlobSize(t)?;
        let mut blob = vec![0u8; size as usize];
        MFGetAttributesAsBlob(t, &mut blob)?;
        Ok(blob)
    }
}

/// The SPS and PPS (Annex B, with start codes) of the first sample that carries them in-band
/// (pure). The MP4's codec header must be these: the encoder's output type can hold another SPS
/// than the one it encodes with, and decoders then misread every slice.
pub fn sequence_header(samples: &[Sample]) -> Option<Vec<u8>> {
    samples.iter().take(64).find_map(|s| {
        let nals = nal_units(&s.data);
        let sps = nals.iter().find(|n| n.first().is_some_and(|b| b & 0x1f == 7))?;
        let pps = nals.iter().find(|n| n.first().is_some_and(|b| b & 0x1f == 8))?;
        Some([&[0, 0, 0, 1][..], sps, &[0, 0, 0, 1], pps].concat())
    })
}

/// The NAL units of an Annex B buffer, without their start codes (pure).
fn nal_units(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    starts
        .iter()
        .enumerate()
        .map(|(k, &s)| {
            let mut end = starts.get(k + 1).map_or(data.len(), |&n| n - 3);
            // A 4-byte start code's leading zero belongs to the next one.
            while end > s && data[end - 1] == 0 {
                end -= 1;
            }
            &data[s..end]
        })
        .collect()
}

fn media_type(blob: &[u8]) -> Result<IMFMediaType> {
    // SAFETY: a new type filled from our own blob.
    unsafe {
        let t = MFCreateMediaType()?;
        MFInitAttributesFromBlob(&t, blob)?;
        Ok(t)
    }
}

/// Writes `video` (and `audio`) to an MP4 at `path`, samples interleaved by time. Call with MF
/// started on this thread.
pub fn write(path: &Path, video: &Track, audio: Option<&Track>) -> Result<()> {
    // SAFETY: the sink writer's documented sequence; buffers are locked only to copy into them.
    unsafe {
        let mut attrs = None;
        MFCreateAttributes(&mut attrs, 1)?;
        let attrs = attrs.ok_or_else(Error::empty)?;
        // The container by attribute, not by the file's name (`.part.mp4` while it is written).
        attrs.SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4)?;
        let writer = MFCreateSinkWriterFromURL(&HSTRING::from(path.as_os_str()), None, &attrs)?;
        let mut tracks = vec![(video, 0u32)];
        tracks.extend(audio.filter(|a| !a.samples.is_empty()).map(|a| (a, 0u32)));
        for (n, (track, index)) in tracks.iter_mut().enumerate() {
            let t = media_type(&track.kind)?;
            if n == 0
                && let Some(header) = sequence_header(&track.samples).or_else(|| track.header.clone())
            {
                t.SetBlob(&windows::Win32::Media::MediaFoundation::MF_MT_MPEG_SEQUENCE_HEADER, &header)?;
            }
            *index = writer.AddStream(&t)?;
            writer.SetInputMediaType(*index, &t, None)?;
        }
        writer.BeginWriting()?;
        let mut next = vec![0usize; tracks.len()];
        loop {
            // The earliest sample left of any track.
            let pick = (0..tracks.len()).filter(|&i| next[i] < tracks[i].0.samples.len()).min_by_key(|&i| tracks[i].0.samples[next[i]].time);
            let Some(i) = pick else { break };
            let s = &tracks[i].0.samples[next[i]];
            next[i] += 1;
            let buffer = MFCreateMemoryBuffer(s.data.len() as u32)?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None)?;
            std::ptr::copy_nonoverlapping(s.data.as_ptr(), data, s.data.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(s.data.len() as u32)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(s.time)?;
            sample.SetSampleDuration(s.duration)?;
            if s.key {
                sample.SetUINT32(&MFSampleExtension_CleanPoint, 1)?;
            }
            writer.WriteSample(tracks[i].1, &sample)?;
        }
        writer.Finalize()
    }
}

/// Reads an MP4's video back, decoded (NV12, the decoder's own output): how many frames, the
/// last one's time, and the brightness (Y, 16 is black) at the centre of the frame nearest `at`
/// (a check for tests).
#[cfg(test)]
pub fn read_back(path: &Path, at: i64) -> Result<(usize, i64, u8)> {
    use windows::Win32::Media::MediaFoundation::{MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MFMediaType_Video, MFVideoFormat_NV12};
    // SAFETY: a source reader on our own file, read to its end; each buffer is read within its length.
    unsafe {
        let reader = MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), None)?;
        let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        let nv12 = MFCreateMediaType()?;
        nv12.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        nv12.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        reader.SetCurrentMediaType(stream, None, &nv12)?;
        let size = reader.GetCurrentMediaType(stream)?.GetUINT64(&MF_MT_FRAME_SIZE)?;
        let (w, h) = ((size >> 32) as usize, (size & 0xffff_ffff) as usize);
        let (mut frames, mut last, mut luma) = (0, 0, 0u8);
        loop {
            let (mut flags, mut time, mut sample) = (0u32, 0i64, None);
            reader.ReadSample(stream, 0, None, Some(&mut flags), Some(&mut time), Some(&mut sample))?;
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                return Ok((frames, last, luma));
            }
            let Some(sample) = sample else { continue };
            frames += 1;
            last = time;
            if (time - at).abs() < 200_000 {
                let buffer = sample.ConvertToContiguousBuffer()?;
                let (mut data, mut len) = (std::ptr::null_mut(), 0u32);
                buffer.Lock(&mut data, None, Some(&mut len))?;
                // Rows may be padded: the stride is what the Y plane's length says.
                let stride = (len as usize * 2 / 3) / h.max(1);
                let i = (h / 2) * stride + w / 2;
                if i < len as usize {
                    luma = *data.add(i);
                }
                buffer.Unlock()?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(data: &[u8]) -> Sample {
        Sample { time: 0, duration: 0, key: true, data: data.into() }
    }

    /// The MP4's codec header comes from the stream, not the encoder's type: AMD's encoder kept
    /// another SPS in its type, and every decoder misread the slices.
    #[test]
    fn codec_header_from_the_stream() {
        let first = [0, 0, 0, 1, 0x09, 0x10, 0, 0, 0, 1, 0x67, 0x4d, 0x04, 0x2a, 0, 0, 1, 0x68, 0xee, 0x3c, 0x80, 0, 0, 0, 1, 0x65, 0x88, 0, 0, 1, 0x0c, 0xff];
        assert_eq!(nal_units(&first), [&[0x09, 0x10][..], &[0x67, 0x4d, 0x04, 0x2a], &[0x68, 0xee, 0x3c, 0x80], &[0x65, 0x88], &[0x0c, 0xff]]);
        assert_eq!(
            sequence_header(&[sample(&[0, 0, 1, 0x41, 1]), sample(&first)]),
            Some(vec![0, 0, 0, 1, 0x67, 0x4d, 0x04, 0x2a, 0, 0, 0, 1, 0x68, 0xee, 0x3c, 0x80])
        );
        assert_eq!(sequence_header(&[sample(&[0, 0, 1, 0x41, 1])]), None);
    }

    /// Control for the passthrough check: an MP4 Windows encodes itself (sink writer, NV12 in,
    /// H.264 out) must decode with `read_back`. `cargo test clips::mux -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn reference_file_decodes() {
        use windows::Win32::Media::MediaFoundation::*;
        // SAFETY: COM, MF and the sink writer's documented sequence on this test thread.
        unsafe {
            windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED).ok().ok();
            MFStartup(MF_VERSION, 0).expect("MFStartup");
            let (w, h) = (1920u32, 1080u32);
            let path = std::env::temp_dir().join(format!("roalt-clip-ref-{}.mp4", uuid::Uuid::new_v4()));
            let writer = MFCreateSinkWriterFromURL(&HSTRING::from(path.as_os_str()), None, None).expect("writer");
            let out = MFCreateMediaType().unwrap();
            out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).unwrap();
            out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264).unwrap();
            out.SetUINT32(&MF_MT_AVG_BITRATE, 2_000_000).unwrap();
            out.SetUINT64(&MF_MT_FRAME_SIZE, ((w as u64) << 32) | h as u64).unwrap();
            out.SetUINT64(&MF_MT_FRAME_RATE, (30u64 << 32) | 1).unwrap();
            out.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).unwrap();
            let index = writer.AddStream(&out).unwrap();
            let input = MFCreateMediaType().unwrap();
            input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).unwrap();
            input.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12).unwrap();
            input.SetUINT64(&MF_MT_FRAME_SIZE, ((w as u64) << 32) | h as u64).unwrap();
            input.SetUINT64(&MF_MT_FRAME_RATE, (30u64 << 32) | 1).unwrap();
            input.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).unwrap();
            writer.SetInputMediaType(index, &input, None).unwrap();
            writer.BeginWriting().unwrap();
            let size = (w * h * 3 / 2) as usize;
            for i in 0..30i64 {
                let buffer = MFCreateMemoryBuffer(size as u32).unwrap();
                let mut data = std::ptr::null_mut();
                buffer.Lock(&mut data, None, None).unwrap();
                std::ptr::write_bytes(data, 120, (w * h) as usize);
                std::ptr::write_bytes(data.add((w * h) as usize), 128, size - (w * h) as usize);
                buffer.Unlock().unwrap();
                buffer.SetCurrentLength(size as u32).unwrap();
                let s = MFCreateSample().unwrap();
                s.AddBuffer(&buffer).unwrap();
                s.SetSampleTime(i * 333_333).unwrap();
                s.SetSampleDuration(333_333).unwrap();
                writer.WriteSample(index, &s).unwrap();
            }
            writer.Finalize().unwrap();
            let got = read_back(&path, 500_000);
            println!("reference {}: {got:?}", path.display());
        }
    }
}
