//! The last seconds of one stream's compressed samples, kept from a keyframe on (pure).

use std::collections::VecDeque;

use super::mux::{Sample, Track};

/// One stream's samples in time order (100 ns units), its media type (a blob), and the newest
/// SPS/PPS seen in it (some encoders send them only with the first keyframe, and a clip may start
/// at a later one).
#[derive(Default)]
pub struct Ring {
    samples: VecDeque<Sample>,
    pub kind: Vec<u8>,
    pub header: Option<Vec<u8>>,
}

impl Ring {
    /// Adds a sample and drops what `keep` no longer needs: everything before the newest
    /// keyframe at or before `newest − keep` (so a cut of `keep` always starts on a keyframe).
    pub fn push(&mut self, s: Sample, keep: i64) {
        let cutoff = s.time - keep;
        self.samples.push_back(s);
        while let Some(k) = self.samples.iter().skip(1).position(|s| s.key).map(|p| p + 1) {
            if self.samples[k].time > cutoff {
                break;
            }
            self.samples.drain(..k);
        }
        // An encoder that sends no later keyframe would keep everything, for hours. A minute (or
        // `keep`, if longer) past the window the oldest go anyway: far more than any keyframe
        // interval, also at a few frames a second. A ring left without a keyframe saves nothing.
        const MINUTE: i64 = 60 * 10_000_000;
        while self.samples.front().is_some_and(|f| cutoff - f.time > keep.max(MINUTE)) {
            self.samples.pop_front();
        }
    }

    pub fn has_key(&self) -> bool {
        self.samples.iter().any(|s| s.key)
    }
}

/// The clip of the last `keep` before `now`: video from the newest keyframe at or before
/// `now − keep` (else the first one there is), audio over the same span, all rebased so the
/// clip starts at 0. It ends at the last frame there is: a ring that stopped (a minimized client,
/// a crashed one) is cut against its own end, not the clock. After a gap (frames stopped, then
/// came back with a forced keyframe) it starts at that keyframe, never at a frozen picture from
/// before the gap. None: no keyframe yet.
pub fn cut(video: &Ring, audio: &Ring, keep: i64, now: i64, frame: i64, audio_frame: i64) -> Option<(Track, Track)> {
    let samples = &video.samples;
    let now = now.min(samples.back()?.time + frame);
    let cutoff = now - keep;
    let first = samples
        .iter()
        .enumerate()
        .rposition(|(i, s)| s.key && (s.time <= cutoff || i > 0 && samples[i - 1].time < cutoff))
        .or_else(|| samples.iter().position(|s| s.key))?;
    let start = samples[first].time;
    let picked: Vec<&Sample> = samples.iter().skip(first).collect();
    let video_out = picked
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let time = if i == 0 { 0 } else { s.time - start };
            let next = picked.get(i + 1).map_or(time + frame, |n| n.time - start);
            Sample { time, duration: (next - time).max(1), ..(*s).clone() }
        })
        .collect();
    let audio_out = audio
        .samples
        .iter()
        .filter(|s| s.time >= start && s.time < now)
        .map(|s| Sample { time: s.time - start, duration: audio_frame, ..s.clone() })
        .collect();
    Some((
        Track { kind: video.kind.clone(), samples: video_out, header: video.header.clone() },
        Track { kind: audio.kind.clone(), samples: audio_out, header: None },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: i64 = 10_000_000;

    fn sample(time: i64, key: bool) -> Sample {
        Sample { time, duration: 0, key, data: vec![key as u8].into() }
    }

    /// Video at 1 frame per 0.5 s, a keyframe every 2 s, for `secs`.
    fn video(secs: i64, keep: i64) -> Ring {
        let mut r = Ring::default();
        for i in 0..secs * 2 {
            r.push(sample(i * S / 2, i % 4 == 0), keep);
        }
        r
    }

    #[test]
    fn trims_to_one_keyframe_before_the_window() {
        let r = video(20, 5 * S);
        // Newest at 19.5 s: keeps from the keyframe at or before 14.5 s, which is 14 s.
        assert_eq!(r.samples.front().map(|s| (s.time, s.key)), Some((14 * S, true)));
        assert!(r.has_key());
    }

    #[test]
    fn cut_starts_at_zero_on_a_keyframe() {
        let v = video(20, 5 * S);
        let mut a = Ring::default();
        for i in 0..20 * 10 {
            a.push(sample(i * S / 10, true), 5 * S);
        }
        let (vt, at) = cut(&v, &a, 5 * S, 20 * S, S / 2, S / 10).expect("a clip");
        assert_eq!((vt.samples[0].time, vt.samples[0].key), (0, true));
        assert!(vt.samples.windows(2).all(|w| w[0].time + w[0].duration == w[1].time), "durations meet");
        let span = vt.samples.last().map(|s| s.time).unwrap_or(0);
        assert!((5 * S..=7 * S).contains(&span), "about the asked length, from a keyframe: {span}");
        assert!(at.samples.first().is_some_and(|s| s.time >= 0) && at.samples.iter().all(|s| s.time >= 0), "no audio before the video");
    }

    #[test]
    fn a_long_gap_never_makes_a_long_clip() {
        // A keyframe, then 10 minutes of nothing (minimized), then 2 s of frames.
        let mut v = Ring::default();
        v.push(sample(0, true), 30 * S);
        for i in 0..4 {
            v.push(sample(600 * S + i * S / 2, i == 0), 30 * S);
        }
        let (vt, _) = cut(&v, &Ring::default(), 30 * S, 602 * S, S / 2, S / 10).expect("a clip");
        let span = vt.samples.last().map(|s| s.time + s.duration).unwrap_or(0);
        assert_eq!(span, 2 * S, "the frames after the gap, from their keyframe: not 10 minutes, no frozen picture");
    }

    #[test]
    fn a_ring_that_stopped_is_cut_against_its_own_end() {
        // A crashed or minimized client: 20 s of video, then nothing for 5 minutes; sound went on.
        let v = video(20, 30 * S);
        let mut a = Ring::default();
        for i in 0..320 * 10 {
            a.push(sample(i * S / 10, true), 30 * S);
        }
        let (vt, at) = cut(&v, &a, 5 * S, 320 * S, S / 2, S / 10).expect("a clip");
        let span = vt.samples.last().map(|s| s.time + s.duration).unwrap_or(0);
        assert!((5 * S..=7 * S).contains(&span), "the last seconds there were: {span}");
        assert!(at.samples.iter().all(|s| s.time < span), "no sound after the picture ends");
    }

    #[test]
    fn stops_growing_without_keyframes() {
        let mut v = Ring::default();
        for i in 0..130 {
            v.push(sample(i * S / 2, i == 0), 5 * S);
        }
        assert!(v.has_key(), "a keyframe a minute old is still a slow encoder's");
        for i in 130..6000 {
            v.push(sample(i * S / 2, false), 5 * S);
        }
        assert!(v.samples.len() <= 131, "{} samples kept", v.samples.len());
        assert!(!v.has_key());
    }

    #[test]
    fn nothing_to_cut_without_a_keyframe() {
        let mut v = Ring::default();
        v.push(sample(0, false), 30 * S);
        assert!(cut(&v, &Ring::default(), 30 * S, S, S / 60, S / 50).is_none());
        assert!(cut(&Ring::default(), &Ring::default(), 30 * S, S, S / 60, S / 50).is_none());
    }
}
