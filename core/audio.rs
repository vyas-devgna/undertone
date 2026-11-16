use std::{collections::VecDeque, f64::consts::TAU};

use crate::{
    ffi::{AudioLayout, Cue, SampleFormat},
    pitch::{self, Reading, WINDOW_MS},
};

const MAX_CHANNELS: usize = 32;

fn bytes_per_sample(format: SampleFormat) -> Option<usize> {
    match format {
        SampleFormat::Float | SampleFormat::Int32 => Some(4),
        SampleFormat::Int16 => Some(2),
        SampleFormat::UInt8 => Some(1),
        _ => None,
    }
}

fn sample(format: SampleFormat, bytes: &[u8]) -> f32 {
    let value = match format {
        SampleFormat::Float => f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        SampleFormat::Int32 => {
            i32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32 / 2147483648.0
        }
        SampleFormat::Int16 => f32::from(i16::from_ne_bytes([bytes[0], bytes[1]])) / 32768.0,
        SampleFormat::UInt8 => (f32::from(bytes[0]) - 128.0) / 128.0,
        _ => 0.,
    };
    if value.is_finite() {
        value.clamp(-1.0, 1.0)
    } else {
        0.
    }
}

fn layout_size(layout: &AudioLayout) -> Option<(usize, usize, usize)> {
    let rate = layout.rate as usize;
    let channels = layout.channels as usize;
    if !(8_000..=384_000).contains(&rate) || !(1..=MAX_CHANNELS).contains(&channels) {
        return None;
    }
    Some((rate, channels, bytes_per_sample(layout.format)?))
}

pub(crate) struct Input {
    format: SampleFormat,
    sample_bytes: usize,
    channels: usize,
    frame_bytes: usize,
    partial: Vec<u8>,
    channel: usize,
    stride: usize,
    pub rate: f64,
    window_size: usize,
    window: VecDeque<f32>,
    cutoff: f32,
    high_pass: f32,
    previous: Option<f32>,
    sum: f32,
    count: usize,
}

impl Input {
    pub fn new(layout: AudioLayout) -> Option<Self> {
        let (rate, channels, sample_bytes) = layout_size(&layout)?;
        let stride = (rate / 12_000).max(1);
        let window_size = rate * WINDOW_MS / 1000 / stride;
        let frame_bytes = channels * sample_bytes;
        Some(Self {
            format: layout.format,
            sample_bytes,
            channels,
            frame_bytes,
            partial: Vec::with_capacity(frame_bytes),
            channel: 0,
            stride,
            rate: rate as f64 / stride as f64,
            window_size,
            window: VecDeque::with_capacity(window_size),
            cutoff: (-TAU * 52.0 / rate as f64).exp() as f32,
            high_pass: 0.,
            previous: None,
            sum: 0.,
            count: 0,
        })
    }

    pub fn clear(&mut self) {
        self.partial.clear();
        self.window.clear();
        self.high_pass = 0.;
        self.previous = None;
        self.sum = 0.;
        self.count = 0;
    }

    pub fn push(&mut self, mut bytes: &[u8], mut capture: Option<&mut Vec<f32>>) -> bool {
        let mut advanced = false;
        if !self.partial.is_empty() {
            let count = (self.frame_bytes - self.partial.len()).min(bytes.len());
            self.partial.extend_from_slice(&bytes[..count]);
            bytes = &bytes[count..];
            if self.partial.len() < self.frame_bytes {
                return false;
            }
            let value = sample(
                self.format,
                &self.partial[self.channel * self.sample_bytes..],
            );
            self.partial.clear();
            advanced |= self.push_sample(value, capture.as_deref_mut());
        }

        let frames = bytes.chunks_exact(self.frame_bytes);
        if self.channels > 1 && frames.len() >= 32 {
            let mut energy = [0_f64; MAX_CHANNELS];
            for frame in frames.clone() {
                for (channel, channel_energy) in
                    frame.chunks_exact(self.sample_bytes).zip(&mut energy)
                {
                    *channel_energy += f64::from(sample(self.format, channel)).powi(2);
                }
            }
            let mut strongest = self.channel;
            for index in 0..self.channels {
                if energy[index] > energy[strongest] {
                    strongest = index;
                }
            }
            // Keep phase continuity when two channels carry equally strong signals.
            if energy[strongest] > energy[self.channel] * 1.6 {
                self.channel = strongest;
                self.clear();
            }
        }
        self.partial.extend_from_slice(frames.remainder());
        let offset = self.channel * self.sample_bytes;
        for frame in frames {
            let value = sample(self.format, &frame[offset..]);
            advanced |= self.push_sample(value, capture.as_deref_mut());
        }
        advanced
    }

    fn push_sample(&mut self, value: f32, capture: Option<&mut Vec<f32>>) -> bool {
        let previous = self.previous.replace(value).unwrap_or(value);
        self.high_pass = self.cutoff * (self.high_pass + value - previous);
        self.sum += self.high_pass;
        self.count += 1;
        if self.count != self.stride {
            return false;
        }
        let downsampled = self.sum / self.stride as f32;
        self.count = 0;
        self.sum = 0.;
        if let Some(data) = capture {
            // Capture at most one second at the decimated rate.
            if data.len() < self.rate as usize {
                data.push(downsampled);
            }
        } else {
            if self.window.len() == self.window_size {
                self.window.pop_front();
            }
            self.window.push_back(downsampled);
        }
        true
    }

    pub fn detect(&mut self) -> Reading {
        if self.window.len() < self.window_size {
            return Reading::default();
        }
        pitch::detect_window(self.window.make_contiguous(), self.rate)
    }
}

pub(crate) fn render(layout: AudioLayout, cue: Cue, hz: f64, duration_ms: u32) -> Vec<u8> {
    let Some((rate, channels, width)) = layout_size(&layout) else {
        return Vec::new();
    };
    let frames = rate * duration_ms as usize / 1000;
    let mut pcm = Vec::with_capacity(frames * width * channels);
    let phase_step = TAU * hz / rate as f64;
    for i in 0..frames {
        let time = i as f64 / rate as f64;
        let value = match cue {
            Cue::Raise => cue_notes(time, &[1318.51, 1760.0], 0.065),
            Cue::Lower => cue_notes(time, &[1760.0, 1318.51], 0.065),
            Cue::Complete => cue_notes(time, &[1318.51, 1661.22, 1975.53], 0.087),
            Cue::Chime => {
                let envelope = (time / 0.012).min(1.0) * (1.0 - time / 0.20).max(0.).powi(2);
                0.25 * envelope * ((TAU * 880.0 * time).sin() + 0.44 * (TAU * 1318.51 * time).sin())
            }
            _ => {
                let phase = phase_step * i as f64;
                let envelope = (time / 0.035)
                    .min((f64::from(duration_ms) / 1000.0 - time) / 0.085)
                    .clamp(0., 1.);
                let gain = if cue == Cue::Check { 0.60 } else { 0.28 };
                gain * envelope * (phase.sin() + 0.22 * (2.0 * phase).sin())
            }
        };
        let encoded = match layout.format {
            SampleFormat::Float => (value as f32).to_ne_bytes(),
            SampleFormat::Int32 => ((value * 2147483647.0) as i32).to_ne_bytes(),
            SampleFormat::Int16 => {
                let [a, b] = ((value * 32767.0) as i16).to_ne_bytes();
                [a, b, 0, 0]
            }
            _ => [(128.0 + value * 127.0) as u8, 0, 0, 0],
        };
        for _ in 0..channels {
            pcm.extend_from_slice(&encoded[..width]);
        }
    }
    pcm
}

fn cue_notes(time: f64, notes: &[f64], beat: f64) -> f64 {
    let index = (time / beat) as usize;
    let Some(&hz) = notes.get(index) else {
        return 0.;
    };
    let local = time - index as f64 * beat;
    let last = index + 1 == notes.len();
    let end = if last { beat } else { beat * 0.84 };
    let envelope = (local / 0.007).min((end - local) / 0.021).clamp(0., 1.);
    (if index == 0 { 0.19 } else { 0.16 }) * envelope * (TAU * hz * local).sin()
}
