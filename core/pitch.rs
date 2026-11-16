use std::f64::consts::TAU;

pub(crate) const WINDOW_MS: usize = 64;
pub(crate) const TOLERANCE: f64 = 2.0;
pub const TUNINGS: [[i32; 6]; 6] = [
    [40, 45, 50, 55, 59, 64], // Standard
    [38, 45, 50, 55, 59, 64], // Drop D
    [38, 45, 50, 55, 57, 62], // DADGAD
    [38, 45, 50, 54, 57, 62], // Open D
    [38, 43, 50, 55, 59, 62], // Open G
    [39, 44, 49, 54, 58, 63], // E-flat standard
];

#[derive(Debug, Default)]
pub struct Reading {
    pub hz: f64,
    pub rms: f64,
}

pub fn frequency(midi: i32, reference: i32) -> f64 {
    f64::from(reference) * (f64::from(midi - 69) / 12.0).exp2()
}

pub fn cents(hz: f64, target: f64) -> f64 {
    1200.0 * (hz / target).log2()
}

pub(crate) fn nearest_note(hz: f64, reference: i32) -> i32 {
    (69.0 + cents(hz, f64::from(reference)) / 100.0).round() as i32
}

pub(crate) fn nearest_string(hz: f64, targets: &[f64; 6], limit: f64) -> Option<usize> {
    let mut best = None;
    let mut distance = limit;
    for (index, &target) in targets.iter().enumerate() {
        let error = cents(hz, target).abs();
        if error < distance {
            best = Some(index);
            distance = error;
        }
    }
    best
}

/// Detect one pitch from a mono PCM window at its original sample rate.
pub fn detect(samples: &[f32], sample_rate: u32) -> Reading {
    let rate = sample_rate as usize;
    if !(8_000..=384_000).contains(&rate) {
        return Reading::default();
    }
    let frame_count = rate * WINDOW_MS / 1000;
    let Some(samples) = samples
        .get(samples.len().saturating_sub(frame_count)..)
        .filter(|tail| tail.len() == frame_count)
    else {
        return Reading::default();
    };
    let stride = (rate / 12_000).max(1);
    let mut downsampled = [0.0; 1536];
    for (out, input) in downsampled.iter_mut().zip(samples.chunks_exact(stride)) {
        *out = input.iter().sum::<f32>() / stride as f32;
    }
    detect_window(
        &downsampled[..frame_count / stride],
        rate as f64 / stride as f64,
    )
}

#[inline]
fn squared_distance(first: &[f32], second: &[f32]) -> f32 {
    // Independent sums let LLVM use SIMD without reordering the float additions.
    let (mut a, mut b, mut c, mut d) = (0., 0., 0., 0.);
    let mut pairs = first.chunks_exact(4).zip(second.chunks_exact(4));
    for (x, y) in pairs.by_ref() {
        let (p, q, r, s) = (x[0] - y[0], x[1] - y[1], x[2] - y[2], x[3] - y[3]);
        a += p * p;
        b += q * q;
        c += r * r;
        d += s * s;
    }
    let remaining = first.len() % 4;
    let tail = (0..remaining)
        .map(|i| {
            let index = first.len() - remaining + i;
            (first[index] - second[index]).powi(2)
        })
        .sum::<f32>();
    (a + b) + (c + d) + tail
}

pub(crate) fn detect_window(samples: &[f32], rate: f64) -> Reading {
    let (sum, energy) = samples.iter().fold((0., 0.), |(sum, energy), &x| {
        let x = f64::from(x);
        (sum + x, energy + x * x)
    });
    let count = samples.len() as f64;
    let rms = (energy / count - (sum / count).powi(2)).max(0.).sqrt();
    let mut reading = Reading { hz: 0., rms };
    if rms < 0.0012 {
        return reading;
    }

    let min_lag = ((rate / 1250.) as usize).max(2);
    let max_lag = (rate / 54.) as usize + 1;
    let comparisons = samples.len() - max_lag;
    let first = &samples[..comparisons];
    let mut raw = [0_f32; 448];
    let mut normalized = [1_f32; 448];
    let mut running = 0_f32;
    let mut candidate = None;
    let mut search_end = max_lag;

    // Find YIN's first deep trough, then compute only far enough to check
    // whether a strong second harmonic masked a better full-period match.
    for lag in 1..=max_lag {
        let difference = squared_distance(first, &samples[lag..lag + comparisons]);
        raw[lag] = difference;
        running += difference;
        if running > 0. {
            normalized[lag] = difference * lag as f32 / running;
        }
        let previous = lag - 1;
        if candidate.is_none()
            && previous >= min_lag
            && normalized[previous] < 0.20
            && normalized[previous] <= normalized[previous - 1]
            && normalized[previous] <= normalized[lag]
        {
            candidate = Some(previous);
            search_end = (2 * previous + 3).min(max_lag);
        }
        if lag >= search_end {
            break;
        }
    }

    if let Some(lag) = candidate {
        let mut period = lag;
        for doubled in 2 * lag - 2..=(2 * lag + 2).min(max_lag - 1) {
            if normalized[lag] - normalized[doubled] > 0.025
                && normalized[doubled] < 0.6 * normalized[period]
                && normalized[doubled] <= normalized[doubled - 1]
                && normalized[doubled] <= normalized[doubled + 1]
            {
                period = doubled;
            }
        }
        let [left, center, right] = [raw[period - 1], raw[period], raw[period + 1]].map(f64::from);
        let curvature = left + right - 2.0 * center;
        let fraction = if curvature > 0. {
            (0.5 * (left - right) / curvature).clamp(-0.75, 0.75)
        } else {
            0.
        };
        reading.hz = rate / (period as f64 + fraction);
    }
    reading
}

pub(crate) fn reference_tone(samples: &[f32], effective_rate: f64, expected: f64) -> Option<f64> {
    if (samples.len() as f64) < effective_rate * 0.30 {
        return None;
    }
    let count = samples.len() as f64;
    let window: Vec<_> = samples
        .iter()
        .enumerate()
        .map(|(i, &sample)| {
            f64::from(sample) * (0.5 - 0.5 * (TAU * i as f64 / (count - 1.0)).cos())
        })
        .collect();
    let amplitude = |hz: f64| {
        let (sine, cosine) = (TAU * hz / effective_rate).sin_cos();
        let (mut real, mut imaginary, mut x, mut y) = (0., 0., 1., 0.);
        for &sample in &window {
            real += sample * x;
            imaginary += sample * y;
            (x, y) = (cosine * x - sine * y, sine * x + cosine * y);
        }
        4.0 * real.hypot(imaginary) / count
    };

    let step = effective_rate / count;
    let range = (expected * 0.035 / step).ceil() as i32;
    let spectrum: Vec<_> = (-range - 1..=range + 1)
        .map(|bin| amplitude(expected + f64::from(bin) * step))
        .collect();
    let (index, &center) = spectrum
        .iter()
        .enumerate()
        .skip(1)
        .take(spectrum.len() - 2)
        .max_by(|(_, a), (_, b)| a.total_cmp(b))?;
    let (left, right) = (spectrum[index - 1], spectrum[index + 1]);
    if center < 0.00045 || center < left || center < right {
        return None;
    }
    let curvature = left + right - 2.0 * center;
    let fraction = if curvature < 0. {
        0.5 * (left - right) / curvature
    } else {
        0.
    };
    let found = expected + (index as f64 - f64::from(range + 1) + fraction) * step;
    let noise = [-8., -5., 5., 8.]
        .iter()
        .map(|bin| amplitude(found + bin * step))
        .sum::<f64>()
        / 4.0;
    (center > 4.0 * noise).then_some(found)
}
