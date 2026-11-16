use super::*;
use crate::{audio, ffi::SampleFormat, pitch::detect};
use std::f64::consts::TAU;

const RATE: i32 = 48_000;
const MONO: AudioLayout = AudioLayout {
    rate: RATE,
    channels: 1,
    format: SampleFormat::Float,
};

fn noise(seed: &mut u32) -> f64 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    f64::from(*seed) / f64::from(u32::MAX) * 2.0 - 1.0
}

#[test]
fn detects_guitar_plucks_at_multiple_rates_and_offsets() {
    let mut worst: f64 = 0.;
    for rate in [8000, 16000, 23999, 44100, 48000, 96000, 384000] {
        for notes in TUNINGS {
            for midi in notes {
                let expected = frequency(midi, 432);
                for offset in [-45., -2., 0., 2., 41.] {
                    let actual = expected * (offset / 1200_f64).exp2();
                    let mut seed = 42;
                    let samples: Vec<_> = (0..rate * 64 / 1000)
                        .map(|i| {
                            let t = f64::from(i) / f64::from(rate);
                            let phase = TAU * actual * t;
                            ((-5. * t).exp()
                                * (0.13 * phase.sin()
                                    + 0.70 * (2.0004 * phase + 0.4).sin()
                                    + 0.08 * (3.001 * phase).sin()
                                    + 0.06 * (4.003 * phase).sin())
                                + 0.009 * noise(&mut seed)) as f32
                        })
                        .collect();
                    let error = (cents(detect(&samples, rate).hz, expected) - offset).abs();
                    assert!(
                        error < 2.,
                        "{rate} Hz, MIDI {midi}, {offset}¢: {error}¢ error"
                    );
                    worst = worst.max(error);
                }
            }
        }
    }
    println!("1260 plucked-string fixtures, worst pitch error {worst:.4}¢");
}

#[test]
fn handles_frequency_limits_and_rejects_silence_dc_noise_and_short_frames() {
    for hz in [56., 73.416, 440., 880., 1174.66] {
        let samples: Vec<_> = (0..3072)
            .map(|i| (0.11 * (TAU * f64::from(i) * hz / f64::from(RATE)).sin() + 0.03) as f32)
            .collect();
        assert!(cents(detect(&samples, RATE as u32).hz, hz).abs() < 2.);
    }
    assert_eq!(detect(&[0.; 3072], RATE as u32).hz, 0.);
    assert_eq!(detect(&[0.03; 3072], RATE as u32).hz, 0.);
    assert_eq!(detect(&[0.03; 2000], RATE as u32).hz, 0.);
    assert_eq!(detect(&[0.03; 3072], 1).hz, 0.);
    let mut seed = 42;
    let samples: Vec<_> = (0..3072)
        .map(|_| (0.15 * noise(&mut seed)) as f32)
        .collect();
    assert_eq!(detect(&samples, RATE as u32).hz, 0.);
}

#[test]
fn keeps_tuning_presets_reference_and_capture_ranges_distinct() {
    let mut engine = new_engine(-12, 990);
    assert_eq!(engine.view().preset, 0);
    assert_eq!(engine.view().reference, 466);
    engine.set_reference(432);
    engine.set_reference(500);
    assert_eq!(engine.view().reference, 432);
    engine.set_preset(1);
    engine.set_preset(12);
    assert_eq!(engine.string_midi(0), 38);
    assert_eq!(engine.string_hz(0), frequency(38, 432));
    engine.set_target(-2);
    assert_eq!(engine.view().target, -2);
    assert_eq!(nearest_note(432., 432), 69);

    let standard = TUNINGS[0].map(|note| frequency(note, 440));
    let drop_d = TUNINGS[1].map(|note| frequency(note, 440));
    assert_eq!(nearest_string(frequency(38, 440), &standard, 120.), None);
    assert_eq!(nearest_string(frequency(38, 440), &drop_d, 120.), Some(0));
    assert_eq!(nearest_string(frequency(62, 440), &standard, 400.), Some(5));
    assert_eq!(nearest_string(frequency(69, 440), &standard, 400.), None);
}

fn float_samples(hz: f64, frames: usize) -> Vec<u8> {
    (0..frames)
        .flat_map(|i| {
            let phase = TAU * hz * i as f64 / f64::from(RATE);
            let value =
                (0.17 * phase.sin() + 0.20 * (2. * phase).sin() + 0.05 * (3. * phase).sin()) as f32;
            value.to_ne_bytes()
        })
        .collect()
}

#[test]
fn handles_pcm_formats_fragmented_frames_and_stereo_channel_switch() {
    for format in [
        SampleFormat::Float,
        SampleFormat::Int32,
        SampleFormat::Int16,
        SampleFormat::UInt8,
    ] {
        let layout = AudioLayout {
            rate: RATE,
            channels: 1,
            format,
        };
        let encoded = audio::render(
            AudioLayout {
                rate: RATE,
                channels: 1,
                format,
            },
            Cue::Reference,
            196.,
            350,
        );
        let mut input = audio::Input::new(layout).unwrap();
        for bytes in encoded[..encoded.len() / 2].chunks(121) {
            input.push(bytes, None);
        }
        assert!(cents(input.detect().hz, 196.).abs() < 2., "{format:?}");
    }

    let mut input = audio::Input::new(AudioLayout {
        rate: RATE,
        channels: 2,
        format: SampleFormat::Float,
    })
    .unwrap();
    let signal = float_samples(196., 6000);
    let pcm = signal
        .chunks_exact(4)
        .flat_map(|sample| [0, 0, 0, 0, sample[0], sample[1], sample[2], sample[3]])
        .collect::<Vec<_>>();
    for bytes in pcm.chunks(311) {
        input.push(bytes, None);
    }
    assert!(cents(input.detect().hz, 196.).abs() < 2.);
    assert!(
        audio::Input::new(AudioLayout {
            rate: 0,
            channels: 2,
            format: SampleFormat::Float
        })
        .is_none()
    );
    assert!(
        audio::Input::new(AudioLayout {
            rate: RATE,
            channels: 100,
            format: SampleFormat::Float
        })
        .is_none()
    );
    let invalid_sample = f32::NAN.to_ne_bytes().repeat(3072);
    let mut input = audio::Input::new(MONO).unwrap();
    input.push(&invalid_sample, None);
    assert_eq!(input.detect().hz, 0.);
}

fn feed(engine: &mut Engine, now: &mut u64, hz: f64, frames: usize) {
    for bytes in float_samples(hz, frames).chunks_exact(1200 * size_of::<f32>()) {
        *now += 25;
        engine.tick(*now, bytes, OutputState::Active);
    }
}

fn finish_output(engine: &mut Engine, now: &mut u64) {
    let cue = engine.playback();
    engine.output_started(*now);
    let emitted = engine.render_audio(MONO);
    for bytes in emitted.chunks(1200 * size_of::<f32>()) {
        *now += 25;
        engine.tick(*now, bytes, OutputState::Active);
    }
    *now += 200;
    engine.tick(*now, &[], OutputState::Drained);
    for bytes in emitted.chunks(1200 * size_of::<f32>()).take(8) {
        *now += 25;
        engine.tick(*now, bytes, OutputState::Active);
    }
    *now += GUARD_MS;
    engine.tick(*now, &[], OutputState::Active);
    assert_eq!(engine.playback().cue, Cue::Off);
    assert_ne!(cue.cue, Cue::Off);
}

#[test]
fn hands_free_tracks_any_string_chimes_once_and_rechecks() {
    let mut engine = new_engine(0, 440);
    assert!(engine.configure_input(MONO));
    assert_eq!(engine.view().target, -1);
    assert_eq!(engine.playback().cue, Cue::Off);
    let mut now = 0;
    feed(
        &mut engine,
        &mut now,
        frequency(64, 440) * (-180. / 1200_f64).exp2(),
        19200,
    );
    assert_eq!(engine.view().active_string, 5);
    assert!((engine.view().cents + 180.).abs() < 2.);
    assert_eq!(engine.view().tuned_mask, 0);

    let mut mask = 0;
    for index in [4, 0, 3, 5, 1, 2] {
        let hz = engine.string_hz(index);
        feed(&mut engine, &mut now, hz, 19200);
        mask |= 1 << index;
        assert_eq!(engine.view().active_string, index as i32);
        assert_eq!(engine.view().tuned_mask, mask);
        assert!(engine.view().in_tune);
        let expected = if mask == 63 {
            Cue::Complete
        } else {
            Cue::Chime
        };
        assert_eq!(engine.playback().cue, expected);
        finish_output(&mut engine, &mut now);
        assert_eq!(
            engine.view().tuned_mask,
            mask,
            "speaker cue must not mark another string"
        );
        assert_eq!(engine.view().active_string, index as i32);
    }
    let hz = engine.string_hz(4);
    feed(&mut engine, &mut now, hz, 14400);
    assert_eq!(engine.playback().cue, Cue::Off);
    feed(&mut engine, &mut now, hz * 1.04, 14400);
    assert_eq!(engine.view().tuned_mask, 63 ^ (1 << 4));
    assert_eq!(engine.playback().cue, Cue::Lower);
    engine.play_reference();
    assert_eq!(engine.playback().hz, hz);
    finish_output(&mut engine, &mut now);
    assert_eq!(
        engine.view().tuned_mask,
        63 ^ (1 << 4),
        "reference and its tail must not count"
    );
    feed(&mut engine, &mut now, hz, 19200);
    assert_eq!(engine.view().tuned_mask, 63);
    assert_eq!(engine.playback().cue, Cue::Complete);
    engine.set_target(4);
    assert_eq!(engine.view().target, 4);
    assert_eq!(engine.playback().cue, Cue::Off);
    engine.clear_progress();
    now += GUARD_MS;
    feed(&mut engine, &mut now, hz, 19200);
    assert!(engine.view().in_tune);
    assert_eq!(
        engine.playback().cue,
        Cue::Off,
        "manual selection is silent"
    );

    engine.set_target(-1);
    engine.drop_input(true);
    assert!(engine.configure_input(MONO));
    feed(&mut engine, &mut now, frequency(64, 440), 19200);
    assert_eq!(engine.view().active_string, 5);
    assert_eq!(
        engine.playback().cue,
        Cue::Chime,
        "Auto resumes after reconnecting"
    );
}

#[test]
fn does_not_confirm_discontinuous_pitch_or_hold_a_stale_reading() {
    let mut engine = new_engine(0, 440);
    for i in 1..20 {
        engine.now_ms = i * 25;
        engine.update_reading(Reading {
            hz: if i % 2 == 0 { 110. } else { 0. },
            rms: 0.3,
        });
    }
    assert_eq!(engine.view().tuned_mask, 0);
    assert_eq!(engine.view().hz, 0.);

    engine.set_target(1);
    engine.configure_input(MONO);
    let mut now = 500;
    feed(&mut engine, &mut now, 110., 14400);
    assert!(engine.view().in_tune);
    engine.tick(now + 200, &[], OutputState::Active);
    assert_eq!(engine.view().hz, 0.);
    assert!(!engine.view().in_tune);
}

fn exercise_check(engine: &mut Engine, now: &mut u64, missing: Option<usize>, offset: f64) {
    engine.check_audio();
    for index in 0..3 {
        assert_eq!(engine.view().check_index, index as i32);
        let tone = if missing == Some(index) {
            0.
        } else {
            engine.playback().hz * (offset / 1200.).exp2()
        };
        engine.output_started(*now);
        feed(engine, now, tone, 52800);
        *now += 200;
        engine.tick(*now, &[], OutputState::Drained);
    }
}

#[test]
fn speaker_check_distinguishes_pass_missing_mismatch_and_transport_failure() {
    let mut engine = new_engine(0, 440);
    engine.check_audio();
    assert_eq!(engine.view().notice, Notice::CheckNoInput);
    assert_eq!(engine.view().check_serial, 1);
    engine.configure_input(MONO);
    let mut now = 0;
    exercise_check(&mut engine, &mut now, None, 1.);
    assert_eq!(engine.view().notice, Notice::CheckPassed);
    assert!(engine.view().max_error < 2.);
    exercise_check(&mut engine, &mut now, Some(1), 0.);
    assert_eq!(engine.view().notice, Notice::CheckMissing);
    assert_eq!(engine.view().missing_mask, 1 << 1);
    exercise_check(&mut engine, &mut now, None, 12.);
    assert_eq!(engine.view().notice, Notice::CheckPitchMismatch);
    assert!((engine.view().max_error - 12.).abs() < 1.);
    engine.check_audio();
    engine.output_started(now);
    let serial = engine.view().check_serial;
    engine.tick(now + 4000, &[], OutputState::Active);
    assert_eq!(engine.view().notice, Notice::OutputTimeout);
    assert_eq!(engine.view().check_serial, serial + 1);
    assert_eq!(engine.playback().cue, Cue::Off);
    engine.check_audio();
    engine.drop_input(true);
    assert_eq!(engine.view().notice, Notice::CheckInputLost);
    assert_eq!(engine.view().check_index, -1);
}

#[test]
fn speaker_check_can_find_a_low_amplitude_tone_in_noise() {
    let mut seed = 42;
    let signal: Vec<_> = (0..6000)
        .map(|i| {
            let t = f64::from(i) / 12000.;
            (0.005 * (TAU * 440.62 * t).sin()
                + 0.08 * (TAU * 32. * t).sin()
                + 0.0375 * noise(&mut seed)) as f32
        })
        .collect();
    let hz = pitch::reference_tone(&signal, 12000., 440.).unwrap();
    assert!(cents(hz, 440.62).abs() < 1.25, "measured {hz:.4} Hz");
    assert!(pitch::reference_tone(&[0.; 6000], 12000., 440.).is_none());
}

#[test]
fn directional_audio_rearms_for_new_plucks_but_not_sustained_notes() {
    let mut engine = new_engine(0, 440);
    assert!(engine.configure_input(MONO));
    let mut now = 0;
    let target = engine.string_hz(3);
    let flat = target * (-24. / 1200_f64).exp2();
    let sharp = target * (19. / 1200_f64).exp2();
    feed(&mut engine, &mut now, flat, 19200);
    assert_eq!(engine.playback().cue, Cue::Raise);
    finish_output(&mut engine, &mut now);
    feed(&mut engine, &mut now, flat, 96000);
    assert_eq!(
        engine.playback().cue,
        Cue::Off,
        "a sustained note must stay quiet"
    );
    feed(&mut engine, &mut now, 0., 16800);
    feed(&mut engine, &mut now, flat, 19200);
    assert_eq!(
        engine.playback().cue,
        Cue::Raise,
        "a fresh pluck can prompt again"
    );
    finish_output(&mut engine, &mut now);
    feed(&mut engine, &mut now, sharp, 19200);
    assert_eq!(engine.playback().cue, Cue::Lower);
    engine.set_feedback_enabled(false);
    assert_eq!(engine.playback().cue, Cue::Off);
    now += GUARD_MS;
    feed(&mut engine, &mut now, target, 19200);
    assert!(engine.view().in_tune);
    assert_eq!(
        engine.playback().cue,
        Cue::Off,
        "silent mode still tracks progress"
    );
    engine.play_reference();
    assert_eq!(
        engine.playback().cue,
        Cue::Reference,
        "explicit playback stays available"
    );
}

#[test]
fn cues_encode_opposite_directions_and_do_not_feedback_on_output_failure() {
    let tone = |cue, beat: usize, hz: f64| {
        let pcm = audio::render(MONO, cue, 0., 145);
        let phase = TAU * hz / f64::from(RATE);
        let (x, y) = pcm
            .chunks_exact(4)
            .skip(beat * 3120 + 480)
            .take(1230)
            .enumerate()
            .fold((0., 0.), |(x, y), (i, sample)| {
                let value = f64::from(f32::from_ne_bytes(sample.try_into().unwrap()));
                let (sine, cosine) = (phase * i as f64).sin_cos();
                (x + value * cosine, y + value * sine)
            });
        x.hypot(y)
    };
    for (cue, first, second) in [(Cue::Raise, 1318.51, 1760.), (Cue::Lower, 1760., 1318.51)] {
        assert!(tone(cue, 0, first) > tone(cue, 0, second) * 6.);
        assert!(tone(cue, 1, second) > tone(cue, 1, first) * 6.);
    }

    let mut engine = new_engine(0, 440);
    engine.configure_input(MONO);
    let mut now = 0;
    feed(&mut engine, &mut now, 107., 19200);
    assert_eq!(engine.playback().cue, Cue::Raise);
    engine.output_failed(Notice::OutputMissing);
    now += GUARD_MS;
    feed(&mut engine, &mut now, 114., 19200);
    assert!(engine.view().cents > 0.);
    assert_eq!(engine.playback().cue, Cue::Off);
}

#[test]
fn a_fresh_attack_can_rearm_feedback_while_a_note_is_still_ringing() {
    let mut direction = DirectionFeedback::default();
    let mut cue = None;
    for i in 1..=12 {
        direction.observe_attack(0.025);
        cue = direction.direction(i * 25, 3, -15., -15.).or(cue);
    }
    assert_eq!(cue, Some(Cue::Raise));
    for i in 40..=48 {
        direction.observe_attack(0.020);
        assert_eq!(direction.direction(i * 25, 3, -15., -15.), None);
    }
    direction.observe_attack(0.080);
    assert_eq!(direction.direction(1250, 3, -15., -15.), Some(Cue::Raise));
}
