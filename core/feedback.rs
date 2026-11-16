use crate::{ffi::Cue, pitch::TOLERANCE};

const DIRECTION_INTERVAL_MS: u64 = 850;
const MIN_DIRECTION_CHANGE: f64 = 8.0;
pub(crate) const MAX_DIRECTION_CENTS: f64 = 160.0;

#[derive(Default)]
pub(crate) struct DirectionFeedback {
    candidate: Option<(usize, Cue)>,
    frames: u8,
    last: Option<(usize, Cue, f64)>,
    quiet_frames: u8,
    not_before: u64,
    previous_rms: f64,
}

pub(crate) fn is_feedback(cue: Cue) -> bool {
    matches!(cue, Cue::Raise | Cue::Lower | Cue::Chime | Cue::Complete)
}

impl DirectionFeedback {
    pub fn interrupt(&mut self) {
        self.candidate = None;
        self.frames = 0;
        self.quiet_frames = 0;
    }

    pub fn unpitched(&mut self) {
        self.candidate = None;
        self.frames = 0;
        self.quiet_frames = self.quiet_frames.saturating_add(1);
        if self.quiet_frames >= 7 {
            self.last = None;
        }
    }

    pub fn confirmed(&mut self, now: u64) {
        self.not_before = now + DIRECTION_INTERVAL_MS;
        self.interrupt();
    }

    pub fn observe_attack(&mut self, rms: f64) {
        // A fresh pluck can rise out of a still-ringing note.
        if self.previous_rms > 0. && rms > self.previous_rms * 1.7 + 0.003 {
            self.last = None;
        }
        self.previous_rms = rms;
    }

    pub fn direction(&mut self, now: u64, index: usize, cents: f64, measured: f64) -> Option<Cue> {
        self.quiet_frames = 0;
        if !(TOLERANCE..=MAX_DIRECTION_CENTS).contains(&cents.abs())
            || !(TOLERANCE..=MAX_DIRECTION_CENTS).contains(&measured.abs())
            || cents.abs() <= TOLERANCE
            || measured.abs() <= TOLERANCE
            || cents.is_sign_negative() != measured.is_sign_negative()
        {
            self.interrupt();
            return None;
        }
        let cue = if cents < 0. { Cue::Raise } else { Cue::Lower };
        if self.candidate != Some((index, cue)) {
            self.candidate = Some((index, cue));
            self.frames = 0;
        }
        self.frames = self.frames.saturating_add(1);
        if self.frames < 4 || now < self.not_before {
            return None;
        }
        if self
            .last
            .is_some_and(|(previous_index, previous_cue, previous_cents)| {
                previous_index == index
                    && previous_cue == cue
                    && (previous_cents - cents).abs() < MIN_DIRECTION_CHANGE
            })
        {
            return None;
        }
        self.last = Some((index, cue, cents));
        self.confirmed(now);
        Some(cue)
    }
}
