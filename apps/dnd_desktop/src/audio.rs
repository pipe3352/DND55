//! Sound effects synthesized at startup (original and license-free): dice, weapons, impacts,
//! healing, doors. Imported music/ambience packs are planned (see docs/ROADMAP.md).

use crate::state::AudioSettings;
use bevy::audio::{AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sfx {
    Dice,
    Swing,
    Bow,
    Hit,
    Crit,
    Miss,
    Heal,
    Door,
    Horn,
    Turn,
    Step,
    Click,
}

/// Whether an audio device was found at startup (the game runs silently otherwise).
#[derive(Resource, Clone, Copy)]
pub struct AudioStatus {
    pub available: bool,
}

#[derive(Resource, Default)]
pub struct SfxBank {
    sounds: HashMap<Sfx, Handle<AudioSource>>,
}

impl SfxBank {
    pub fn play(&self, commands: &mut Commands, s: Sfx, settings: &AudioSettings) {
        let vol = settings.master * settings.effects * if s == Sfx::Step { 0.35 } else { 1.0 };
        if vol <= 0.001 {
            return;
        }
        if let Some(h) = self.sounds.get(&s) {
            commands.spawn((
                AudioPlayer::new(h.clone()),
                PlaybackSettings {
                    volume: Volume::Linear(vol),
                    ..PlaybackSettings::DESPAWN
                },
            ));
        }
    }
}

const RATE: u32 = 22_050;

/// 16-bit mono PCM WAV.
pub fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&RATE.to_le_bytes());
    v.extend_from_slice(&(RATE * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    v
}

struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn samples(secs: f32, mut f: impl FnMut(f32, &mut Noise) -> f32) -> Vec<f32> {
    let mut n = Noise(0x9e37_79b9);
    (0..(secs * RATE as f32) as usize)
        .map(|i| f(i as f32 / RATE as f32, &mut n))
        .collect()
}

fn env(t: f32, attack: f32, decay: f32) -> f32 {
    if t < attack {
        t / attack
    } else {
        (-(t - attack) / decay).exp()
    }
}

pub fn synth(s: Sfx) -> Vec<f32> {
    use std::f32::consts::TAU;
    match s {
        Sfx::Dice => samples(0.55, |t, n| {
            // A few wooden clicks with decreasing spacing.
            let hits = [0.0, 0.11, 0.2, 0.27, 0.32, 0.36];
            hits.iter()
                .map(|h| {
                    if t >= *h {
                        n.next() * env(t - h, 0.001, 0.012) * 0.7
                    } else {
                        0.0
                    }
                })
                .sum::<f32>()
        }),
        Sfx::Swing => {
            let mut lp = 0.0f32;
            samples(0.28, |t, n| {
                let cutoff = 0.05 + 0.5 * (t / 0.28);
                lp += (n.next() - lp) * cutoff;
                lp * (t * std::f32::consts::PI / 0.28).sin() * 1.4
            })
        }
        Sfx::Bow => samples(0.3, |t, n| {
            (TAU * 180.0 * t * (1.0 - t)).sin() * env(t, 0.002, 0.05) * 0.7
                + n.next() * env(t, 0.0, 0.03) * 0.2
        }),
        Sfx::Hit => samples(0.3, |t, n| {
            ((TAU * 70.0 * t).sin() * 0.8 + n.next() * 0.4) * env(t, 0.002, 0.07)
        }),
        Sfx::Crit => samples(0.6, |t, n| {
            ((TAU * 60.0 * t).sin() * 0.8 + n.next() * 0.4) * env(t, 0.002, 0.1)
                + (TAU * 880.0 * t).sin() * env(t, 0.01, 0.2) * 0.3
        }),
        Sfx::Miss => {
            let mut lp = 0.0f32;
            samples(0.22, |t, n| {
                lp += (n.next() - lp) * 0.08;
                lp * (t * std::f32::consts::PI / 0.22).sin() * 1.2
            })
        }
        Sfx::Heal => samples(0.7, |t, _| {
            [523.0, 659.0, 784.0]
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let st = i as f32 * 0.1;
                    if t >= st {
                        (TAU * f * t).sin() * env(t - st, 0.01, 0.25) * 0.25
                    } else {
                        0.0
                    }
                })
                .sum()
        }),
        Sfx::Door => samples(0.6, |t, n| {
            ((TAU * (90.0 + 40.0 * (t * 9.0).sin()) * t).sin() * 0.5 + n.next() * 0.15)
                * env(t, 0.05, 0.25)
        }),
        Sfx::Horn => samples(1.0, |t, _| {
            let f = 220.0;
            ((TAU * f * t).sin()
                + 0.5 * (TAU * 2.0 * f * t).sin()
                + 0.25 * (TAU * 3.0 * f * t).sin())
                * env(t, 0.08, 0.5)
                * 0.35
        }),
        Sfx::Turn => samples(0.35, |t, _| {
            ((TAU * 660.0 * t).sin() * 0.3 + (TAU * 990.0 * t).sin() * 0.15) * env(t, 0.005, 0.1)
        }),
        Sfx::Step => samples(0.08, |t, n| n.next() * env(t, 0.001, 0.015) * 0.5),
        Sfx::Click => samples(0.05, |t, _| {
            (TAU * 1200.0 * t).sin() * env(t, 0.001, 0.01) * 0.4
        }),
    }
}

pub fn build_bank(mut bank: ResMut<SfxBank>, sources: Option<ResMut<Assets<AudioSource>>>) {
    let Some(mut sources) = sources else { return }; // audio disabled
    for s in [
        Sfx::Dice,
        Sfx::Swing,
        Sfx::Bow,
        Sfx::Hit,
        Sfx::Crit,
        Sfx::Miss,
        Sfx::Heal,
        Sfx::Door,
        Sfx::Horn,
        Sfx::Turn,
        Sfx::Step,
        Sfx::Click,
    ] {
        let bytes: Arc<[u8]> = wav(&synth(s)).into();
        bank.sounds.insert(s, sources.add(AudioSource { bytes }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid() {
        let w = wav(&synth(Sfx::Dice));
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        let data_len = u32::from_le_bytes(w[40..44].try_into().unwrap()) as usize;
        assert_eq!(w.len(), 44 + data_len);
    }
}
