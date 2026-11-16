mod audio;
mod feedback;
pub mod pitch;

use feedback::{DirectionFeedback, MAX_DIRECTION_CENTS, is_feedback};
use ffi::{AudioLayout, Cue, Notice, OutputState, Playback, View};
use pitch::{Reading, TOLERANCE, TUNINGS, cents, frequency, nearest_note, nearest_string};

#[cxx::bridge(namespace = "undertone")]
pub mod ffi {
    #[derive(Debug)]
    enum SampleFormat {
        Invalid,
        Float,
        Int32,
        Int16,
        UInt8,
    }

    struct AudioLayout {
        rate: i32,
        channels: i32,
        format: SampleFormat,
    }

    #[derive(Debug)]
    enum Cue {
        Off,
        Chime,
        Raise,
        Lower,
        Complete,
        Reference,
        Check,
    }

    #[derive(Debug)]
    enum OutputState {
        Active,
        Drained,
        Failed,
    }

    #[derive(Debug)]
    enum Notice {
        Clear,
        CheckNoInput,
        CheckInputLost,
        OutputMissing,
        OutputUnsupported,
        OutputFailed,
        OutputTimeout,
        CheckPassed,
        CheckPitchMismatch,
        CheckMissing,
    }

    #[derive(Clone, Copy, Debug)]
    struct Playback {
        serial: u64,
        cue: Cue,
        hz: f64,
        duration_ms: u32,
    }

    #[derive(Clone, Copy, Debug)]
    struct View {
        preset: i32,
        reference: i32,
        target: i32,
        hz: f64,
        cents: f64,
        level: f64,
        note: i32,
        active_string: i32,
        tuned_mask: i32,
        in_tune: bool,
        off_target: bool,
        check_index: i32,
        notice: Notice,
        check_serial: u64,
        max_error: f64,
        missing_mask: u8,
    }

    extern "Rust" {
        type Engine;
        fn new_engine(preset: i32, reference: i32) -> Box<Engine>;
        fn view(self: &Engine) -> View;
        fn playback(self: &Engine) -> Playback;
        fn string_midi(self: &Engine, index: usize) -> i32;
        fn string_hz(self: &Engine, index: usize) -> f64;
        fn check_midi(index: i32) -> i32;
        fn configure_input(self: &mut Engine, layout: AudioLayout) -> bool;
        fn drop_input(self: &mut Engine, lost: bool);
        fn set_preset(self: &mut Engine, preset: i32);
        fn set_reference(self: &mut Engine, reference: i32);
        fn set_target(self: &mut Engine, target: i32);
        fn set_feedback_enabled(self: &mut Engine, enabled: bool);
        fn clear_progress(self: &mut Engine);
        fn play_reference(self: &mut Engine);
        fn check_audio(self: &mut Engine);
        fn stop_sound(self: &mut Engine);
        fn render_audio(self: &Engine, layout: AudioLayout) -> Vec<u8>;
        fn output_started(self: &mut Engine, now_ms: u64);
        fn output_failed(self: &mut Engine, notice: Notice);
        fn tick(self: &mut Engine, now_ms: u64, bytes: &[u8], output: OutputState);
    }
}

const CHECK_NOTES: [i32; 3] = [69, 76, 81];
const GUARD_MS: u64 = 225;
const FEEDBACK_GUARD_MS: u64 = 125;

pub struct Engine {
    view: View,
    targets: [f64; 6],
    playback: Playback,
    input: Option<audio::Input>,
    now_ms: u64,
    last_signal: u64,
    last_input: u64,
    started_at: Option<u64>,
    quiet_until: u64,
    candidate_hz: f64,
    candidate_frames: u8,
    tune_frames: u8,
    detuned_frames: u8,
    feedback_enabled: bool,
    feedback_retry_at: u64,
    direction: DirectionFeedback,
    check_samples: Vec<f32>,
    check_errors: [Option<f64>; 3],
}

fn new_engine(preset: i32, reference: i32) -> Box<Engine> {
    let preset = preset.clamp(0, TUNINGS.len() as i32 - 1);
    let reference = reference.clamp(415, 466);
    Box::new(Engine {
        view: View {
            preset,
            reference,
            target: -1,
            hz: 0.,
            cents: 0.,
            level: 0.,
            note: -1,
            active_string: -1,
            tuned_mask: 0,
            in_tune: false,
            off_target: false,
            check_index: -1,
            notice: Notice::Clear,
            check_serial: 0,
            max_error: 0.,
            missing_mask: 0,
        },
        targets: TUNINGS[preset as usize].map(|midi| frequency(midi, reference)),
        playback: Playback {
            serial: 0,
            cue: Cue::Off,
            hz: 0.,
            duration_ms: 0,
        },
        input: None,
        now_ms: 0,
        last_signal: 0,
        last_input: 0,
        started_at: None,
        quiet_until: 0,
        candidate_hz: 0.,
        candidate_frames: 0,
        tune_frames: 0,
        detuned_frames: 0,
        feedback_enabled: true,
        feedback_retry_at: 0,
        direction: DirectionFeedback::default(),
        check_samples: Vec::new(),
        check_errors: [None; 3],
    })
}

fn check_midi(index: i32) -> i32 {
    CHECK_NOTES.get(index as usize).copied().unwrap_or(69)
}

impl Engine {
    fn view(&self) -> View {
        self.view
    }
    fn playback(&self) -> Playback {
        self.playback
    }

    fn string_midi(&self, index: usize) -> i32 {
        TUNINGS[self.view.preset as usize][index]
    }

    fn string_hz(&self, index: usize) -> f64 {
        self.targets[index]
    }

    fn configure_input(&mut self, layout: AudioLayout) -> bool {
        self.input = audio::Input::new(layout);
        self.input.is_some()
    }

    fn drop_input(&mut self, lost: bool) {
        let was_checking = self.view.check_index >= 0;
        self.input = None;
        self.direction = DirectionFeedback::default();
        self.view.active_string = -1;
        self.stop_sound();
        if lost && was_checking {
            self.view.notice = Notice::CheckInputLost;
        }
    }

    fn set_preset(&mut self, preset: i32) {
        if !(0..TUNINGS.len() as i32).contains(&preset) || self.view.preset == preset {
            return;
        }
        self.view.preset = preset;
        self.view.target = -1;
        self.refresh_targets();
    }

    fn set_reference(&mut self, reference: i32) {
        if !(415..=466).contains(&reference) || self.view.reference == reference {
            return;
        }
        self.view.reference = reference;
        self.refresh_targets();
    }

    fn refresh_targets(&mut self) {
        self.targets =
            TUNINGS[self.view.preset as usize].map(|midi| frequency(midi, self.view.reference));
        self.view.active_string = -1;
        self.stop_sound();
        self.clear_progress();
    }

    fn set_target(&mut self, target: i32) {
        if !(-2..6).contains(&target) || self.view.target == target {
            return;
        }
        self.view.target = target;
        self.direction = DirectionFeedback::default();
        self.view.active_string = -1;
        self.stop_sound();
    }

    fn set_feedback_enabled(&mut self, enabled: bool) {
        self.feedback_enabled = enabled;
        if !enabled && is_feedback(self.playback.cue) {
            self.stop_sound();
        }
        self.direction = DirectionFeedback::default();
    }

    fn clear_progress(&mut self) {
        self.view.tuned_mask = 0;
        self.direction = DirectionFeedback::default();
        self.clear_reading();
    }

    fn clear_reading(&mut self) {
        if let Some(input) = &mut self.input {
            input.clear();
        }
        self.direction.interrupt();
        self.clear_signal();
        self.view.level = 0.;
    }

    fn clear_signal(&mut self) {
        self.view.note = -1;
        if self.view.target != -1 {
            self.view.active_string = -1;
        }
        self.view.hz = 0.;
        self.view.cents = 0.;
        self.view.in_tune = false;
        self.view.off_target = false;
        self.candidate_frames = 0;
        self.candidate_hz = 0.;
        self.tune_frames = 0;
        self.detuned_frames = 0;
    }

    fn play_reference(&mut self) {
        if matches!(self.playback.cue, Cue::Reference | Cue::Check) {
            self.stop_sound();
            return;
        }
        let hz = match self.view.target {
            -2 => frequency(
                if self.view.note >= 0 {
                    self.view.note
                } else {
                    69
                },
                self.view.reference,
            ),
            -1 => self.targets[self.view.active_string.max(0) as usize],
            target => self.targets[target as usize],
        };
        self.view.notice = Notice::Clear;
        self.request_output(Cue::Reference, hz, 1250);
    }

    fn check_audio(&mut self) {
        if self.view.check_index >= 0 {
            self.stop_sound();
            return;
        }
        self.stop_sound();
        if self.input.is_none() {
            self.view.notice = Notice::CheckNoInput;
            self.view.check_serial += 1;
            return;
        }
        self.check_errors = [None; 3];
        self.view.check_index = 0;
        self.start_check_tone();
    }

    fn start_check_tone(&mut self) {
        self.check_samples.clear();
        self.request_output(
            Cue::Check,
            frequency(check_midi(self.view.check_index), self.view.reference),
            1100,
        );
    }

    fn request_output(&mut self, cue: Cue, hz: f64, duration_ms: u32) {
        self.playback = Playback {
            serial: self.playback.serial + 1,
            cue,
            hz,
            duration_ms,
        };
        self.started_at = None;
        if !is_feedback(cue) {
            self.clear_reading();
        }
    }

    fn stop_sound(&mut self) {
        self.view.notice = Notice::Clear;
        if self.view.check_index >= 0 {
            self.view.check_index = -1;
            self.view.check_serial += 1;
        }
        if self.playback.cue != Cue::Off {
            let guard = if is_feedback(self.playback.cue) {
                FEEDBACK_GUARD_MS
            } else {
                GUARD_MS
            };
            self.request_output(Cue::Off, 0., 0);
            self.quiet_until = self.now_ms + guard;
        } else {
            self.clear_reading();
        }
        self.check_samples.clear();
    }

    fn render_audio(&self, layout: AudioLayout) -> Vec<u8> {
        audio::render(
            layout,
            self.playback.cue,
            self.playback.hz,
            self.playback.duration_ms,
        )
    }

    fn output_started(&mut self, now_ms: u64) {
        if self.playback.cue != Cue::Off {
            self.started_at = Some(now_ms.max(self.now_ms));
        }
    }

    fn output_failed(&mut self, notice: Notice) {
        if is_feedback(self.playback.cue) {
            self.feedback_retry_at = self.now_ms + 5_000;
        }
        self.stop_sound();
        self.view.notice = notice;
    }

    fn tick(&mut self, now_ms: u64, bytes: &[u8], output: OutputState) {
        self.now_ms = now_ms.max(self.now_ms);
        if self.playback.cue != Cue::Off {
            self.tick_output(bytes, output);
            return;
        }
        if self.now_ms < self.quiet_until {
            return;
        }
        if let Some(input) = &mut self.input {
            if input.push(bytes, None) {
                self.last_input = self.now_ms;
                let reading = input.detect();
                self.update_reading(reading);
            } else if self.now_ms.saturating_sub(self.last_input) >= 175
                && (self.view.hz > 0. || self.view.level > 0. || self.candidate_frames > 0)
            {
                self.clear_reading();
            }
        }
    }

    fn tick_output(&mut self, bytes: &[u8], output: OutputState) {
        let Some(start) = self.started_at else { return };
        let elapsed = self.now_ms.saturating_sub(start);
        let duration = u64::from(self.playback.duration_ms);
        if output == OutputState::Failed {
            self.output_failed(Notice::OutputFailed);
            return;
        }
        if self.playback.cue == Cue::Check && (345..945).contains(&elapsed) {
            if let Some(input) = &mut self.input {
                input.push(bytes, Some(&mut self.check_samples));
            }
        }
        let drain_margin = if is_feedback(self.playback.cue) {
            0
        } else {
            180
        };
        if elapsed >= duration + drain_margin && output == OutputState::Drained {
            if self.view.check_index >= 0 {
                self.finish_check_tone();
            } else {
                self.stop_sound();
            }
        } else if elapsed > duration + 2500 {
            self.output_failed(Notice::OutputTimeout);
        }
    }

    fn finish_check_tone(&mut self) {
        let expected = self.playback.hz;
        let measured = self
            .input
            .as_ref()
            .and_then(|input| pitch::reference_tone(&self.check_samples, input.rate, expected));
        self.check_errors[self.view.check_index as usize] = measured.map(|hz| cents(hz, expected));
        self.view.check_index += 1;
        if (self.view.check_index as usize) < CHECK_NOTES.len() {
            self.start_check_tone();
            return;
        }
        let mut missing = 0;
        let mut max_error: f64 = 0.;
        for (index, error) in self.check_errors.iter().enumerate() {
            if let Some(error) = error {
                max_error = max_error.max(error.abs());
            } else {
                missing |= 1 << index;
            }
        }
        self.stop_sound();
        self.view.max_error = max_error;
        self.view.missing_mask = missing;
        self.view.notice = if missing != 0 {
            Notice::CheckMissing
        } else if max_error <= 4. {
            Notice::CheckPassed
        } else {
            Notice::CheckPitchMismatch
        };
    }

    fn update_reading(&mut self, Reading { hz, rms }: Reading) {
        let level = ((20. * rms.max(0.00001).log10() + 62.) / 62.).clamp(0., 1.);
        self.view.level = (level * 8. - 0.6).ceil().max(0.) / 8.;
        if hz <= 0. {
            self.direction.unpitched();
            self.candidate_frames = 0;
            self.tune_frames = 0;
            self.detuned_frames = 0;
            if self.now_ms.saturating_sub(self.last_signal) >= 175 {
                self.clear_signal();
            }
            return;
        }
        if self.candidate_hz <= 0. || cents(hz, self.candidate_hz).abs() > 55. {
            self.direction.interrupt();
            self.candidate_frames = 0;
            self.tune_frames = 0;
            self.detuned_frames = 0;
        }
        self.candidate_hz = hz;
        if self.candidate_frames < 2 {
            self.candidate_frames += 1;
            // A stream of unstable candidates must not leave the last note on the meter.
            if self.now_ms.saturating_sub(self.last_signal) >= 175 {
                self.view.hz = 0.;
                self.view.in_tune = false;
                self.view.off_target = false;
            }
            return;
        }
        self.last_signal = self.now_ms;

        let matched = match self.view.target {
            -2 => None,
            -1 => nearest_string(hz, &self.targets, 400.),
            target => {
                let index = target as usize;
                (cents(hz, self.targets[index]).abs() <= 500.).then_some(index)
            }
        };
        let chromatic = self.view.target == -2;
        let target_note = matched.map(|index| self.string_midi(index));
        let detected = nearest_note(hz, self.view.reference);
        let note = target_note.unwrap_or(detected);
        let off_target = target_note.is_none() && !chromatic;
        if self.view.hz > 0. && note == self.view.note && cents(hz, self.view.hz).abs() < 40. {
            self.view.hz += 0.4 * (hz - self.view.hz);
        } else {
            self.view.hz = hz;
            self.tune_frames = 0;
            self.detuned_frames = 0;
        }

        self.view.note = note;
        self.view.off_target = off_target;
        self.view.active_string = matched.map_or(-1, |index| index as i32);
        let expected = matched.map_or_else(
            || frequency(note, self.view.reference),
            |index| self.targets[index],
        );
        self.view.cents = if off_target {
            0.
        } else {
            cents(self.view.hz, expected)
        };
        let measured = cents(hz, expected);
        if !off_target && self.view.cents.abs() <= TOLERANCE && measured.abs() <= TOLERANCE {
            self.tune_frames = (self.tune_frames + 1).min(6);
        } else {
            self.tune_frames = 0;
        }
        self.view.in_tune = self.tune_frames >= 6;
        let Some(index) = matched else {
            self.detuned_frames = 0;
            self.direction.interrupt();
            return;
        };
        let feedback = self.view.target == -1
            && self.feedback_enabled
            && self.now_ms >= self.feedback_retry_at;
        let bit = 1 << index;
        if self.view.in_tune {
            let first = self.view.tuned_mask & bit == 0;
            self.view.tuned_mask |= bit;
            if feedback && first {
                self.direction.confirmed(self.now_ms);
                if self.view.tuned_mask == (1 << self.targets.len()) - 1 {
                    self.request_output(Cue::Complete, 0., 300);
                } else {
                    self.request_output(Cue::Chime, 0., 215);
                }
            }
        }
        let detuned = self.view.cents.abs() > 5.
            && measured.abs() > 5.
            && (self.view.target != -1 || measured.abs() <= MAX_DIRECTION_CENTS);
        self.detuned_frames = if detuned {
            (self.detuned_frames + 1).min(4)
        } else {
            0
        };
        if self.detuned_frames >= 4 {
            self.view.tuned_mask &= !bit;
        }
        if !feedback || self.playback.cue != Cue::Off {
            return;
        }
        self.direction.observe_attack(rms);
        if let Some(cue) = self
            .direction
            .direction(self.now_ms, index, self.view.cents, measured)
        {
            self.request_output(cue, 0., 145);
        }
    }
}

#[cfg(test)]
mod tests;
