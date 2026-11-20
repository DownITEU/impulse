//! The cues' own tones, built in memory: struck notes with a soft click and an exponential decay.
//! Modelled on a hardware switch's two sounds: a pair of notes a fourth apart, the off sound a fourth below the on.

use std::f32::consts::TAU;

use super::Cue;

const SAMPLE_RATE: u32 = 44_100;
/// Quiet by default: a confirmation must not startle over a game's sound.
const PEAK: f32 = 0.35;
/// A struck note's rise, and the fade that makes the end click-free.
const ATTACK_MS: f32 = 6.;
const RELEASE_MS: f32 = 25.;
/// The second harmonic of each note, -23 dB: what gives a sine its bell-like body.
const OVERTONE: f32 = 0.07;
/// The lower note of a pair, -4 dB.
const UNDER_LEVEL: f32 = 0.63;
/// A fourth down.
const FOURTH: f32 = 0.75;
/// The click is high-passed noise that dies within a few milliseconds.
const CLICK_LEVEL: f32 = 0.1;
const CLICK_MS: f32 = 14.;

struct Voice {
    /// The higher note, in hertz.
    hertz: f32,
    /// A second note, as a ratio of `hertz`.
    under: Option<f32>,
    /// Time constant of the exponential decay.
    decay_ms: f32,
    length_ms: u32,
    /// Loudness against the loudest cue.
    gain: f32,
}

impl Cue {
    fn voice(self) -> Voice {
        let toggle = |hertz| Voice { hertz, under: Some(FOURTH), decay_ms: 55., length_ms: 150, gain: 1. };
        match self {
            Cue::ToggleOn => toggle(880.),
            Cue::ToggleOff => toggle(660.),
            Cue::Slider => Voice { hertz: 1175., under: None, decay_ms: 14., length_ms: 50, gain: 0.6 },
            Cue::Number => Voice { hertz: 784., under: None, decay_ms: 20., length_ms: 60, gain: 0.7 },
            Cue::Button => Voice { hertz: 1047., under: Some(FOURTH), decay_ms: 30., length_ms: 90, gain: 0.9 },
            Cue::Selection => Voice { hertz: 740., under: None, decay_ms: 38., length_ms: 100, gain: 0.8 },
        }
    }
}

/// A complete mono 16-bit WAV file of the cue.
pub(super) fn wav(cue: Cue) -> Vec<u8> {
    let samples = render(&cue.voice(), cue as u32 + 1);
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes()); // bytes per frame
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

/// The notes over their envelope plus the click, scaled so the loudest sample is `PEAK` times the voice's gain.
fn render(voice: &Voice, seed: u32) -> Vec<i16> {
    let rate = SAMPLE_RATE as f32;
    let count = (SAMPLE_RATE * voice.length_ms / 1000) as usize;
    let (attack, release, click_rise) = (rate * ATTACK_MS / 1000., rate * RELEASE_MS / 1000., rate * 0.0005);
    let notes = [(voice.hertz, 1.), (voice.hertz * voice.under.unwrap_or(0.), if voice.under.is_some() { UNDER_LEVEL } else { 0. })];
    let (mut noise, mut last) = (Noise(seed), 0.);
    let raw: Vec<f32> = (0..count)
        .map(|i| {
            let (at, seconds) = (i as f32, i as f32 / rate);
            let wave: f32 = notes
                .iter()
                .map(|&(hertz, level)| level * ((TAU * hertz * seconds).sin() + OVERTONE * (TAU * 2. * hertz * seconds).sin()))
                .sum();
            let envelope = (at / attack).min(1.) * (-seconds * 1000. / voice.decay_ms).exp() * ((count as f32 - at) / release).min(1.);
            // The difference of two noise samples leaves the highs.
            let hiss = noise.next();
            let click = (hiss - last) * CLICK_LEVEL * (-seconds * 1000. / CLICK_MS).exp() * (at / click_rise).min(1.);
            last = hiss;
            wave * envelope + click
        })
        .collect();
    let loudest = raw.iter().fold(f32::EPSILON, |most, sample| most.max(sample.abs()));
    let scale = PEAK * voice.gain * f32::from(i16::MAX) / loudest;
    raw.into_iter().map(|sample| (sample * scale) as i16).collect()
}

/// White noise in -1..1 from a fixed seed, so a cue sounds the same on every start.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2. - 1.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<Vec<u8>> {
        Cue::ALL.iter().map(|&cue| wav(cue)).collect()
    }

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    fn u16_at(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
    }

    fn samples(bytes: &[u8]) -> Vec<i16> {
        bytes[44..].as_chunks::<2>().0.iter().map(|&pair| i16::from_le_bytes(pair)).collect()
    }

    #[test]
    fn the_header_describes_mono_16_bit_pcm_at_44100_hz() {
        for bytes in all() {
            assert_eq!(&bytes[0..4], b"RIFF");
            assert_eq!(&bytes[8..12], b"WAVE");
            assert_eq!(&bytes[12..16], b"fmt ");
            assert_eq!(&bytes[36..40], b"data");
            assert_eq!(u32_at(&bytes, 16), 16);
            assert_eq!(u16_at(&bytes, 20), 1);
            assert_eq!(u16_at(&bytes, 22), 1);
            assert_eq!(u32_at(&bytes, 24), 44_100);
            assert_eq!(u32_at(&bytes, 28), 44_100 * 2);
            assert_eq!(u16_at(&bytes, 32), 2);
            assert_eq!(u16_at(&bytes, 34), 16);
        }
    }

    #[test]
    fn the_lengths_in_the_header_match_the_bytes() {
        for bytes in all() {
            assert_eq!(u32_at(&bytes, 40) as usize, bytes.len() - 44);
            assert_eq!(u32_at(&bytes, 4) as usize, bytes.len() - 8);
        }
    }

    #[test]
    fn a_toggle_lasts_150_ms_and_a_tick_is_shorter() {
        let millis = |cue| samples(&wav(cue)).len() * 1000 / 44_100;
        assert_eq!((millis(Cue::ToggleOn), millis(Cue::ToggleOff)), (150, 150));
        assert!(Cue::ALL.iter().all(|&cue| (50..=150).contains(&millis(cue))));
    }

    #[test]
    fn every_cue_builds_sound() {
        for bytes in all() {
            assert!(samples(&bytes).iter().any(|&sample| sample != 0));
        }
    }

    #[test]
    fn the_six_cues_differ_from_each_other() {
        let all = all();
        for (i, first) in all.iter().enumerate() {
            for second in &all[i + 1..] {
                assert_ne!(first, second);
            }
        }
    }

    #[test]
    fn no_cue_is_louder_than_its_gain_of_0_35_of_full_scale() {
        for &cue in &Cue::ALL {
            let limit = (0.35 * cue.voice().gain * f32::from(i16::MAX)) as i32 + 1;
            assert!(samples(&wav(cue)).iter().all(|&sample| i32::from(sample).abs() <= limit));
        }
    }

    #[test]
    fn a_cue_starts_and_ends_at_silence() {
        for bytes in all() {
            let samples = samples(&bytes);
            assert_eq!(samples[0], 0);
            assert!(i32::from(*samples.last().unwrap()).abs() < 200);
        }
    }

    #[test]
    fn off_is_on_a_fourth_lower() {
        let (on, off) = (Cue::ToggleOn.voice(), Cue::ToggleOff.voice());
        assert_eq!(on.hertz * FOURTH, off.hertz);
    }

    #[test]
    fn a_cue_sounds_the_same_every_time() {
        assert_eq!(wav(Cue::ToggleOn), wav(Cue::ToggleOn));
    }
}
