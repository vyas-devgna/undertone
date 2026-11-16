# Engineering notes

## Tracking pitch

Undertone analyzes a 64 ms sliding window about every 25 ms. A rolling buffer decimates new input once and reuses it across overlapping windows. The YIN detector looks for the first deep dip in the cumulative mean-normalized difference, then checks for a better match at twice that period when the second harmonic is strong. It interpolates the raw difference curve for a fractional period. [YIN](https://doi.org/10.1121/1.1458024)

Targets use twelve-tone equal temperament. For MIDI note `n`, the target is `A4 × 2^((n − 69)/12)`. The meter reports `1200 × log2(measured / target)` cents. Consecutive readings are required before marking a string tuned. [ISO 16](https://www.iso.org/standard/3601.html)

## Listening and playback

Direction cues wait for stable pitch, use a cooldown, and rearm after a fresh pluck or a meaningful change. Capture frames during output and its short decay are excluded from tuning. Midpoint pitches can be ambiguous with one microphone and a badly detuned string. Auto follows the nearest preset target for display and withholds directional cues beyond its narrower confidence range.

The three-tone speaker check measures its received signal at known frequencies. It checks agreement across the local audio path. Measuring absolute clock accuracy requires an independent reference. In automatic operation, microphone capture resumes when a usable input becomes available.

## Verification

- The Rust suite covers 1,260 harmonic-rich plucks across seven sample rates. The worst error on that synthetic set was 0.52 cents.
- Twelve pitch-shifted excerpts from recorded acoustic open strings produced the expected tighten or loosen cue. [Recording dataset](https://huggingface.co/datasets/collegefishiesd/guitar-fretboard-notes)
- The desktop build was exercised through PipeWire for arbitrary string order, all four audible signals, source removal, and output feedback suppression. The physical speaker and microphone loopback also passed.
- On the development laptop, the Rust detector processed a 48 kHz window in a median 21.36 µs. The previous detector took 109.43 µs on the same 569 windows. Both accepted the same 362 windows and differed by at most 0.000085 cents. These measurements cover detector computation and were taken over five alternating runs on one pinned CPU.
