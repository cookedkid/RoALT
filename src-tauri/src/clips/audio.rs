//! One process's own sound (Windows process loopback): what the client plays, nothing else on
//! the PC. 48 kHz, 16-bit stereo PCM; Windows converts to it (`AUTOCONVERTPCM`). Documented for
//! build 20348+, and works from 19041 (as OBS uses it); `start` fails where it doesn't.

use std::sync::mpsc;
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
    AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
    AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS, ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
    IActivateAudioInterfaceCompletionHandler_Impl, IAudioCaptureClient, IAudioClient, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
    VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVE_FORMAT_PCM, WAVEFORMATEX,
};
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0};
use windows::Win32::System::Com::{BLOB, IAgileObject};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::Win32::System::Variant::VT_BLOB;
use windows::core::{HRESULT, IUnknown, Interface, implement};

pub const RATE: u32 = 48_000;
pub const CHANNELS: u16 = 2;
/// Bytes per sample frame (16-bit stereo).
pub const FRAME: usize = 4;

/// One packet: interleaved PCM, and when its first frame played (QPC, 100 ns); silence is zeros.
pub struct Packet {
    pub pcm: Vec<u8>,
    pub time: i64,
}

#[implement(IActivateAudioInterfaceCompletionHandler, IAgileObject)]
struct Done(mpsc::Sender<()>);

// Agile: Windows may call it on any thread.
impl windows::Win32::System::Com::IAgileObject_Impl for Done_Impl {}

impl IActivateAudioInterfaceCompletionHandler_Impl for Done_Impl {
    fn ActivateCompleted(&self, _op: windows_core::Ref<'_, IActivateAudioInterfaceAsyncOperation>) -> windows_core::Result<()> {
        let _ = self.0.send(());
        Ok(())
    }
}

/// A running capture of `pid`'s sound (and its child processes'). Call on an MTA thread.
pub struct Capture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    event: HANDLE,
}

// SAFETY: the audio client from process-loopback activation is free-threaded (agile); the event
// handle is a plain kernel handle.
unsafe impl Send for Capture {}

impl Capture {
    pub fn start(pid: u32) -> windows_core::Result<Self> {
        let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
            ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
            Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS { TargetProcessId: pid, ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE },
            },
        };
        // Never dropped: dropping a PROPVARIANT clears it, which would hand our stack to CoTaskMemFree.
        let pv = std::mem::ManuallyDrop::new(PROPVARIANT {
            Anonymous: PROPVARIANT_0 {
                Anonymous: std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                    vt: VT_BLOB,
                    Anonymous: PROPVARIANT_0_0_0 {
                        blob: BLOB { cbSize: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32, pBlobData: (&raw mut params).cast() },
                    },
                    ..Default::default()
                }),
            },
        });
        let (tx, rx) = mpsc::channel();
        let handler: IActivateAudioInterfaceCompletionHandler = Done(tx).into();
        // SAFETY: `pv` and `params` outlive the call and the wait below; the PROPVARIANT is never
        // cleared (it points at our stack, not at memory COM should free).
        let op = unsafe { ActivateAudioInterfaceAsync(VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, &IAudioClient::IID, Some(&*pv), &handler)? };
        rx.recv_timeout(Duration::from_secs(5)).map_err(|_| windows_core::Error::from_hresult(HRESULT(0x8000_000A_u32 as i32)))?;
        let mut hr = HRESULT(0);
        let mut unk: Option<IUnknown> = None;
        // SAFETY: the operation completed (the handler ran).
        unsafe { op.GetActivateResult(&mut hr, &mut unk)? };
        hr.ok()?;
        let client: IAudioClient = unk.ok_or_else(|| windows_core::Error::from_hresult(HRESULT(0x8000_4005_u32 as i32)))?.cast()?;
        let format = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: CHANNELS,
            nSamplesPerSec: RATE,
            nAvgBytesPerSec: RATE * FRAME as u32,
            nBlockAlign: FRAME as u16,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        let flags = AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
        // SAFETY: plain COM calls on the activated client; `format` lives across Initialize.
        unsafe {
            client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 200_000, 0, &format, None)?;
            let event = CreateEventW(None, false, false, None)?;
            client.SetEventHandle(event)?;
            let capture: IAudioCaptureClient = client.GetService()?;
            client.Start()?;
            Ok(Capture { client, capture, event })
        }
    }

    /// The packets ready now, after waiting up to `wait` for one.
    pub fn read(&self, wait: Duration) -> windows_core::Result<Vec<Packet>> {
        let mut out = Vec::new();
        // SAFETY: the buffer from GetBuffer is read before ReleaseBuffer, within its length.
        unsafe {
            if WaitForSingleObject(self.event, wait.as_millis() as u32) != WAIT_OBJECT_0 {
                return Ok(out);
            }
            while self.capture.GetNextPacketSize()? > 0 {
                let (mut data, mut frames, mut flags, mut qpc) = (std::ptr::null_mut(), 0u32, 0u32, 0u64);
                self.capture.GetBuffer(&mut data, &mut frames, &mut flags, None, Some(&mut qpc))?;
                let len = frames as usize * FRAME;
                // AUDCLNT_BUFFERFLAGS_SILENT: the bytes may be anything; it is silence.
                let pcm = if flags & 0x2 != 0 || data.is_null() { vec![0; len] } else { std::slice::from_raw_parts(data, len).to_vec() };
                self.capture.ReleaseBuffer(frames)?;
                out.push(Packet { pcm, time: qpc as i64 });
            }
        }
        Ok(out)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // SAFETY: stop and close what `start` made.
        unsafe {
            let _ = self.client.Stop();
            let _ = CloseHandle(self.event);
        }
    }
}

/// AAC frames are this many sample frames (21.3 ms at 48 kHz).
const AAC_FRAME: usize = 1024;
/// One AAC frame's length, 100 ns units.
pub const AAC_DURATION: i64 = AAC_FRAME as i64 * 10_000_000 / RATE as i64;
/// A lull this long without packets is silence the client didn't send (it sends none when quiet).
const LULL: i64 = 500_000;
/// A packet this much earlier than where the sound is has its overlap dropped (20 ms).
const OVERLAP: i64 = 200_000;
/// A gap this long is not filled with silence: the sound starts again where it is (5 s).
const RESYNC: i64 = 50_000_000;

/// Windows' AAC encoder (software, sync): 48 kHz 16-bit stereo PCM in, 160 kbps AAC out.
pub struct Aac {
    mft: windows::Win32::Media::MediaFoundation::IMFTransform,
    out_size: u32,
    /// Its output type (a blob, with the AudioSpecificConfig the MP4 needs).
    pub kind: Vec<u8>,
}

impl Aac {
    pub fn new() -> windows_core::Result<Self> {
        use windows::Win32::Media::MediaFoundation::*;
        let input = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Audio, guidSubtype: MFAudioFormat_PCM };
        let output = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Audio, guidSubtype: MFAudioFormat_AAC };
        // SAFETY: MF calls with local arguments; the activate array is freed after use. The
        // AAC encoder takes its input type before its output type.
        unsafe {
            let (mut list, mut count) = (std::ptr::null_mut::<Option<IMFActivate>>(), 0u32);
            MFTEnumEx(MFT_CATEGORY_AUDIO_ENCODER, MFT_ENUM_FLAG_SYNCMFT | MFT_ENUM_FLAG_SORTANDFILTER, Some(&input), Some(&output), &mut list, &mut count)?;
            let found = if list.is_null() { Vec::new() } else { std::slice::from_raw_parts_mut(list, count as usize).iter_mut().map(Option::take).collect() };
            if !list.is_null() {
                windows::Win32::System::Com::CoTaskMemFree(Some(list.cast()));
            }
            let mft: IMFTransform = found.into_iter().flatten().next().ok_or_else(windows_core::Error::empty)?.ActivateObject()?;
            let pcm = MFCreateMediaType()?;
            pcm.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            pcm.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
            pcm.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            pcm.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, RATE)?;
            pcm.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, CHANNELS as u32)?;
            pcm.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, FRAME as u32)?;
            pcm.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, RATE * FRAME as u32)?;
            mft.SetInputType(0, &pcm, 0)?;
            let aac = MFCreateMediaType()?;
            aac.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            aac.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
            aac.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
            aac.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, RATE)?;
            aac.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, CHANNELS as u32)?;
            aac.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 20_000)?;
            aac.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)?;
            mft.SetOutputType(0, &aac, 0)?;
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            let out_size = mft.GetOutputStreamInfo(0)?.cbSize.max(8192);
            let kind = super::mux::type_blob(&mft.GetOutputCurrentType(0)?)?;
            Ok(Aac { mft, out_size, kind })
        }
    }

    /// Encodes one AAC frame of PCM shown at `time`; what comes out goes to `out` as (time, bytes).
    pub fn encode(&mut self, pcm: &[u8], time: i64, out: &mut Vec<(i64, Vec<u8>)>) -> windows_core::Result<()> {
        use windows::Win32::Media::MediaFoundation::*;
        // SAFETY: copies into a buffer of its own length; output samples are ours, read within
        // their length and dropped once copied.
        unsafe {
            let buffer = MFCreateMemoryBuffer(pcm.len() as u32)?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None)?;
            std::ptr::copy_nonoverlapping(pcm.as_ptr(), data, pcm.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(pcm.len() as u32)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(time)?;
            // Never zero: the encoder divides by it.
            sample.SetSampleDuration(((pcm.len() / FRAME) as i64 * 10_000_000 / RATE as i64).max(1))?;
            self.mft.ProcessInput(0, &sample, 0)?;
            loop {
                let buffer = MFCreateMemoryBuffer(self.out_size)?;
                let sample = MFCreateSample()?;
                sample.AddBuffer(&buffer)?;
                let mut buffers = [MFT_OUTPUT_DATA_BUFFER { dwStreamID: 0, pSample: std::mem::ManuallyDrop::new(Some(sample)), dwStatus: 0, pEvents: std::mem::ManuallyDrop::new(None) }];
                let mut status = 0;
                let done = self.mft.ProcessOutput(0, &mut buffers, &mut status);
                let [mut b] = buffers;
                let (sample, events) = (std::mem::ManuallyDrop::take(&mut b.pSample), std::mem::ManuallyDrop::take(&mut b.pEvents));
                drop(events);
                match done {
                    Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(()),
                    Err(e) => return Err(e),
                    Ok(()) => {}
                }
                let Some(sample) = sample else { return Ok(()) };
                let t = sample.GetSampleTime().unwrap_or(time);
                let b = sample.ConvertToContiguousBuffer()?;
                let (mut p, mut len) = (std::ptr::null_mut(), 0u32);
                b.Lock(&mut p, None, Some(&mut len))?;
                let bytes = std::slice::from_raw_parts(p, len as usize).to_vec();
                b.Unlock()?;
                if !bytes.is_empty() {
                    out.push((t, bytes));
                }
            }
        }
    }
}

/// Now on the capture clock (QPC, 100 ns).
pub fn now() -> i64 {
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    let (mut count, mut freq) = (0i64, 0i64);
    // SAFETY: two reads into locals.
    unsafe {
        let _ = QueryPerformanceCounter(&mut count);
        let _ = QueryPerformanceFrequency(&mut freq);
    }
    (count as i128 * 10_000_000 / (freq.max(1) as i128)) as i64
}

/// PCM waiting for a whole AAC frame, from `time` on; silence is added where the client sent
/// nothing, so the sound stays on the picture's clock.
#[derive(Default)]
struct Pending {
    pcm: Vec<u8>,
    time: Option<i64>,
}

impl Pending {
    fn end(&self) -> Option<i64> {
        self.time.map(|t| t + (self.pcm.len() / FRAME) as i64 * 10_000_000 / RATE as i64)
    }

    fn silence_until(&mut self, until: i64) {
        let Some(end) = self.end() else {
            self.time = Some(until);
            return;
        };
        if until - end > RESYNC {
            // Far behind (the PC slept, the thread stalled): start again from `until` rather than
            // filling minutes of zeros.
            self.pcm.clear();
            self.time = Some(until);
        } else if until > end {
            let frames = ((until - end) * RATE as i64 / 10_000_000) as usize;
            self.pcm.resize(self.pcm.len() + frames * FRAME, 0);
        }
    }

    fn add(&mut self, mut p: Packet) {
        if self.time.is_none() {
            self.time = Some(p.time);
        }
        // A gap in what the client sent: silence in between.
        self.silence_until(p.time);
        // Sound from before where we are (the silence clock ran ahead of a late packet): its
        // overlap is dropped, so the sound never drifts later than the picture.
        if let Some(end) = self.end()
            && end - p.time > OVERLAP
        {
            let frames = ((end - p.time) * RATE as i64 / 10_000_000) as usize;
            p.pcm.drain(..(frames * FRAME).min(p.pcm.len()));
        }
        self.pcm.extend_from_slice(&p.pcm);
    }

    /// Whole AAC frames, each with its time.
    fn frames(&mut self) -> Vec<(i64, Vec<u8>)> {
        let mut out = Vec::new();
        let bytes = AAC_FRAME * FRAME;
        while self.pcm.len() >= bytes {
            let Some(t) = self.time else { break };
            out.push((t, self.pcm.drain(..bytes).collect()));
            self.time = Some(t + AAC_DURATION);
        }
        out
    }
}

/// Captures `pid`'s sound and hands AAC frames to `put` until `stop`. Call on its own thread.
pub fn record(pid: u32, stop: &std::sync::atomic::AtomicBool, kind: &mut dyn FnMut(Vec<u8>), put: &mut dyn FnMut(i64, Vec<u8>)) -> windows_core::Result<()> {
    let capture = Capture::start(pid)?;
    let mut aac = Aac::new()?;
    kind(aac.kind.clone());
    let mut pending = Pending::default();
    let mut out = Vec::new();
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        let packets = capture.read(Duration::from_millis(10))?;
        if packets.is_empty() {
            // Quiet: Windows sends nothing, so the clock fills it, a little behind now.
            pending.silence_until(now() - LULL);
        }
        packets.into_iter().for_each(|p| pending.add(p));
        for (t, pcm) in pending.frames() {
            aac.encode(&pcm, t, &mut out)?;
        }
        for (t, bytes) in out.drain(..) {
            put(t, bytes);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_keeps_the_clock() {
        let mut p = Pending::default();
        p.add(Packet { pcm: vec![1; 480 * FRAME], time: 0 });
        // 100 ms with nothing, then more sound: silence fills the gap.
        p.add(Packet { pcm: vec![2; 480 * FRAME], time: 1_100_000 });
        let frames = p.frames();
        assert!(frames.windows(2).all(|w| w[1].0 - w[0].0 == AAC_DURATION), "frames are back to back");
        let all: Vec<u8> = frames.iter().flat_map(|f| f.1.clone()).collect();
        assert_eq!(all[..480 * FRAME], vec![1; 480 * FRAME][..]);
        // 10 ms of sound, then 100 ms of silence (4800 frames); 5 AAC frames are whole so far.
        assert_eq!(all.len(), 5 * AAC_FRAME * FRAME);
        assert!(all[480 * FRAME..].iter().all(|&b| b == 0), "the gap is silence");
        // A quiet client: the clock alone moves it on (640 frames left, plus 1 s).
        let before = p.end().unwrap_or(0);
        p.silence_until(before + 10_000_000);
        assert_eq!(p.frames().len(), (640 + 48_000) / AAC_FRAME);
    }

    #[test]
    fn late_packets_and_long_stalls_never_drift() {
        let mut p = Pending::default();
        p.add(Packet { pcm: vec![1; 4800 * FRAME], time: 0 });
        // The silence clock ran 50 ms ahead; then a packet stamped 100 ms arrives: 50 ms of it overlaps.
        p.silence_until(1_500_000);
        p.add(Packet { pcm: vec![2; 4800 * FRAME], time: 1_000_000 });
        assert_eq!(p.end(), Some(2_000_000), "the overlap was dropped: the sound ends where its packet does");
        // A 10-minute stall: no 10 minutes of zeros, the sound starts again there.
        p.silence_until(6_000_000_000);
        assert_eq!((p.time, p.pcm.len()), (Some(6_000_000_000), 0));
    }

    /// Loudest sample in 16-bit PCM.
    fn peak(pcm: &[u8]) -> i32 {
        pcm.as_chunks::<2>().0.iter().map(|b| (i16::from_le_bytes(*b) as i32).abs()).max().unwrap_or(0)
    }

    /// Step 1 (a): a child process plays a sound; only its sound is captured, also when muted.
    /// `cargo test clips::audio -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn captures_one_process() {
        // SAFETY: COM for this test thread.
        unsafe { windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED).ok().ok() };
        for mute in [false, true] {
            let script = "$p = New-Object Media.SoundPlayer 'C:\\Windows\\Media\\Alarm01.wav'; $p.PlayLooping(); Start-Sleep -Seconds 12";
            let mut child = std::process::Command::new("powershell").args(["-NoProfile", "-Command", script]).spawn().expect("powershell");
            if mute {
                // Waits until its audio session exists, as a launch's mute does.
                assert!(crate::instances::audio::set_mute(child.id(), true), "muted");
            } else {
                std::thread::sleep(Duration::from_secs(2));
            }
            let cap = Capture::start(child.id()).expect("process loopback activates on this Windows");
            let (mut bytes, mut loudest) = (0usize, 0);
            let end = std::time::Instant::now() + Duration::from_secs(3);
            while std::time::Instant::now() < end {
                for p in cap.read(Duration::from_millis(50)).expect("read") {
                    bytes += p.pcm.len();
                    loudest = loudest.max(peak(&p.pcm));
                }
            }
            let _ = child.kill();
            let _ = child.wait();
            println!("muted: {mute}, captured {:.1} s of audio, peak {loudest}", bytes as f64 / (RATE as f64 * FRAME as f64));
            if !mute {
                assert!(loudest > 1000, "the child's sound was captured");
            }
        }
    }
}
