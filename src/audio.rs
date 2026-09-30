//! Sound effects, synthesised at startup so the game ships no audio files.
//! Samples are generated here, wrapped in a WAV header, and handed to Bevy
//! as ordinary `AudioSource` assets.

use std::f32::consts::TAU;

use bevy::audio::Volume;
use bevy::prelude::*;
use fastrand::Rng;

use crate::practice::{FeedbackKind, KeyFeedback, LessonCompleted, TypingSystems};
use crate::save::SaveData;

pub const SAMPLE_RATE: u32 = 44_100;

pub struct SfxPlugin;

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_sounds)
            .add_systems(Update, play_sounds.after(TypingSystems));
    }
}

#[derive(Resource)]
struct Sfx {
    key: Handle<AudioSource>,
    miss: Handle<AudioSource>,
    backspace: Handle<AudioSource>,
    lesson: Handle<AudioSource>,
    unlock: Handle<AudioSource>,
}

fn load_sounds(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut add = |samples: Vec<f32>| {
        sources.add(AudioSource {
            bytes: wav_bytes(&samples).into(),
        })
    };
    commands.insert_resource(Sfx {
        key: add(key_click()),
        miss: add(error_buzz()),
        backspace: add(backspace_tick()),
        lesson: add(lesson_chime()),
        unlock: add(unlock_fanfare()),
    });
}

fn play_sounds(
    mut commands: Commands,
    sfx: Res<Sfx>,
    save: Res<SaveData>,
    mut feedback: MessageReader<KeyFeedback>,
    mut completed: MessageReader<LessonCompleted>,
    mut rng: Local<Rng>,
) {
    if !save.sound_on {
        feedback.clear();
        completed.clear();
        return;
    }
    for event in feedback.read() {
        let (sound, volume, speed) = match event.kind {
            // A little pitch variation keeps fast typing from sounding robotic.
            FeedbackKind::Hit => (&sfx.key, 0.5, 0.95 + rng.f32() * 0.1),
            FeedbackKind::Miss | FeedbackKind::Blocked => (&sfx.miss, 0.45, 1.0),
            FeedbackKind::Backspace => (&sfx.backspace, 0.4, 1.0),
        };
        play(&mut commands, sound, volume, speed);
    }
    for event in completed.read() {
        let sound = if event.summary.unlocked.is_some() {
            &sfx.unlock
        } else {
            &sfx.lesson
        };
        play(&mut commands, sound, 0.6, 1.0);
    }
}

fn play(commands: &mut Commands, sound: &Handle<AudioSource>, volume: f32, speed: f32) {
    commands.spawn((
        AudioPlayer::new(sound.clone()),
        PlaybackSettings {
            volume: Volume::Linear(volume),
            speed,
            ..PlaybackSettings::DESPAWN
        },
    ));
}

/// A soft mechanical key: a low thock, a burst of noise and a faint tick.
pub fn key_click() -> Vec<f32> {
    let mut noise = Rng::with_seed(1);
    let samples = render(0.06, |t| {
        let thock = (TAU * 160.0 * t).sin() * (-t * 70.0).exp();
        let click = (noise.f32() * 2.0 - 1.0) * (-t * 450.0).exp();
        let tick = (TAU * 2600.0 * t).sin() * (-t * 350.0).exp();
        0.6 * thock + 0.3 * click + 0.15 * tick
    });
    normalize(samples, 0.8)
}

/// A low, slightly detuned buzz for a wrong key.
pub fn error_buzz() -> Vec<f32> {
    let samples = render(0.16, |t| {
        let envelope = (t / 0.005).min(1.0) * (-t * 18.0).exp();
        let tone = (TAU * 110.0 * t).sin() + (TAU * 116.5 * t).sin();
        (tone * 2.0).tanh() * envelope
    });
    normalize(samples, 0.8)
}

/// A short, quiet tick for Backspace.
pub fn backspace_tick() -> Vec<f32> {
    let samples = render(0.03, |t| (TAU * 900.0 * t).sin() * (-t * 160.0).exp());
    normalize(samples, 0.6)
}

/// Two rising notes when a lesson ends.
pub fn lesson_chime() -> Vec<f32> {
    let samples = render(0.6, |t| note(t, 0.0, 659.25) + note(t, 0.12, 880.0));
    normalize(samples, 0.8)
}

/// A rising arpeggio when a new key unlocks.
pub fn unlock_fanfare() -> Vec<f32> {
    let notes = [(0.0, 523.25), (0.1, 659.25), (0.2, 783.99), (0.3, 1046.5)];
    let samples = render(1.0, |t| {
        notes
            .iter()
            .map(|&(start, freq)| note(t, start, freq))
            .sum()
    });
    normalize(samples, 0.8)
}

/// A bell-like note that starts `start` seconds in.
fn note(t: f32, start: f32, freq: f32) -> f32 {
    let t = t - start;
    if t < 0.0 {
        return 0.0;
    }
    let envelope = (t / 0.004).min(1.0) * (-t * 6.0).exp();
    envelope * ((TAU * freq * t).sin() + 0.3 * (TAU * 2.0 * freq * t).sin())
}

/// Calls `f` with the time of each sample in `seconds` of audio.
fn render(seconds: f32, f: impl FnMut(f32) -> f32) -> Vec<f32> {
    let count = (seconds * SAMPLE_RATE as f32) as usize;
    (0..count)
        .map(|i| i as f32 / SAMPLE_RATE as f32)
        .map(f)
        .collect()
}

/// Scales the samples so the loudest one reaches `peak`.
fn normalize(mut samples: Vec<f32>, peak: f32) -> Vec<f32> {
    let loudest = samples.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
    if loudest > 0.0 {
        for sample in &mut samples {
            *sample *= peak / loudest;
        }
    }
    samples
}

/// Encodes mono samples in [-1, 1] as a 16-bit PCM WAV file.
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes()); // format chunk size
    bytes.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1_u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // bytes per second
    bytes.extend_from_slice(&2_u16.to_le_bytes()); // bytes per sample
    bytes.extend_from_slice(&16_u16.to_le_bytes()); // bits per sample
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for &sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16_at(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    }

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn wav_header_describes_the_samples() {
        let bytes = wav_bytes(&[0.0, 0.5, -0.5]);
        assert_eq!(bytes.len(), 44 + 6);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(u32_at(&bytes, 4), 36 + 6);
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(u16_at(&bytes, 20), 1);
        assert_eq!(u16_at(&bytes, 22), 1);
        assert_eq!(u32_at(&bytes, 24), SAMPLE_RATE);
        assert_eq!(u16_at(&bytes, 34), 16);
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(u32_at(&bytes, 40), 6);
        assert_eq!(u16_at(&bytes, 46) as i16, i16::MAX / 2);
    }

    #[test]
    fn sounds_are_audible_and_in_range() {
        let sounds = [
            ("key", key_click()),
            ("miss", error_buzz()),
            ("backspace", backspace_tick()),
            ("lesson", lesson_chime()),
            ("unlock", unlock_fanfare()),
        ];
        for (name, samples) in sounds {
            assert!(!samples.is_empty(), "{name} is empty");
            let peak = samples.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
            assert!(peak > 0.1 && peak <= 1.0, "{name} peaks at {peak}");
            assert!(samples.iter().all(|s| s.is_finite()), "{name} has NaN");
        }
    }
}
