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
    Spell,
    Trap,
    Chest,
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
        Sfx::Spell => samples(0.9, |t, n| {
            // Rising shimmer.
            let f = 400.0 + 900.0 * t;
            ((TAU * f * t).sin() * 0.25 + (TAU * f * 1.5 * t).sin() * 0.12 + n.next() * 0.05)
                * env(t, 0.08, 0.35)
        }),
        Sfx::Trap => samples(0.5, |t, n| {
            // Metallic snap.
            ((TAU * 1800.0 * t).sin() * 0.3 + n.next() * 0.6) * env(t, 0.001, 0.05)
                + (TAU * 120.0 * t).sin() * env(t, 0.0, 0.12) * 0.4
        }),
        Sfx::Chest => samples(0.5, |t, n| {
            ((TAU * (140.0 + 60.0 * t) * t).sin() * 0.4 + n.next() * 0.1) * env(t, 0.02, 0.15)
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
        Sfx::Spell,
        Sfx::Trap,
        Sfx::Chest,
    ] {
        let bytes: Arc<[u8]> = wav(&synth(s)).into();
        bank.sounds.insert(s, sources.add(AudioSource { bytes }));
    }
}

/// Sound ids a scripted event or the DM can request (`Event::Sound`).
pub fn sfx_by_id(id: &str) -> Option<Sfx> {
    Some(match id {
        "dice" => Sfx::Dice,
        "hit" => Sfx::Hit,
        "door" => Sfx::Door,
        "horn" => Sfx::Horn,
        "heal" => Sfx::Heal,
        "spell" => Sfx::Spell,
        "trap" => Sfx::Trap,
        "chest" => Sfx::Chest,
        "click" => Sfx::Click,
        _ => return None,
    })
}

/// Original, procedurally composed music loops (no samples, no third-party material).
pub const MUSIC_TRACKS: &[&str] = &["exploration", "tavern", "mystery", "battle"];
/// Procedural ambience loops.
pub const AMBIENCE_TRACKS: &[&str] = &["cave", "wind", "fire", "forest"];

fn note(semitones_from_a3: i32) -> f32 {
    220.0 * 2f32.powf(semitones_from_a3 as f32 / 12.0)
}

/// Builds a looping track. Each track is a short composition: a drone, a plucked
/// pattern and (for battle) percussion. Loops are seamless because every voice
/// repeats with a period that divides the loop length.
pub fn synth_loop(id: &str) -> Option<Vec<f32>> {
    use std::f32::consts::TAU;
    let pluck = |t: f32, f: f32| {
        // Karplus-ish: decaying harmonics.
        ((TAU * f * t).sin() + 0.4 * (TAU * 2.0 * f * t).sin() + 0.15 * (TAU * 3.0 * f * t).sin())
            * (-t * 3.5).exp()
    };
    let melodic = |len: f32,
                   bpm: f32,
                   pattern: &[i32],
                   drone: &[i32],
                   pluck_vol: f32,
                   drone_vol: f32,
                   drums: bool| {
        let beat = 60.0 / bpm;
        let pat = pattern.to_vec();
        let drone = drone.to_vec();
        let mut lp = 0.0f32;
        samples(len, move |t, n| {
            let step = (t / beat) as usize;
            let local = t - step as f32 * beat;
            let mut v = 0.0;
            let p = pat[step % pat.len()];
            if p > -100 {
                v += pluck(local, note(p)) * pluck_vol;
            }
            // Slow-swelling drone (period = loop length so it is seamless).
            let swell = 0.75 + 0.25 * (TAU * t / len).sin();
            for d in &drone {
                v += (TAU * note(*d) * t).sin() * drone_vol * swell;
            }
            if drums {
                let b = step % 4;
                if b == 0 || b == 2 {
                    v += (TAU * 55.0 * local).sin() * (-local * 18.0).exp() * 0.6;
                }
                if b == 1 || b == 3 {
                    lp += (n.next() - lp) * 0.5;
                    v += lp * (-local * 30.0).exp() * 0.35;
                }
            }
            v * 0.8
        })
    };
    const R: i32 = -1000; // rest
    Some(match id {
        // D dorian, unhurried.
        "exploration" => melodic(
            16.0,
            90.0,
            &[
                5, R, 12, 8, 7, R, 3, 5, 0, R, 3, 5, 7, R, 10, 8, 5, R, 12, 15, 14, R, 12, 10,
            ],
            &[-7, 0],
            0.22,
            0.05,
            false,
        ),
        // Major, lively.
        "tavern" => melodic(
            12.0,
            140.0,
            &[
                3, 7, 10, 7, 15, 10, 7, 10, 5, 8, 12, 8, 17, 12, 8, 12, 3, 7, 10, 15, 14, 10, 7, 3,
                1, 5, 8, 5, 10, 8, 5, 1,
            ],
            &[-9],
            0.2,
            0.04,
            false,
        ),
        // Sparse, low, unresolved.
        "mystery" => melodic(
            16.0,
            60.0,
            &[0, R, R, 1, R, R, -2, R, R, R, 6, R, R, 5, R, R],
            &[-12, -6],
            0.25,
            0.04,
            false,
        ),
        // Minor, driving, with drums. 128 bpm × 32 steps = 15 s.
        "battle" => melodic(
            15.0,
            128.0,
            &[
                0, 0, 3, 0, 7, 0, 6, 3, 0, 0, 3, 0, 10, 8, 7, 3, -2, -2, 1, -2, 5, -2, 3, 1, -4,
                -4, 0, 3, 7, 3, 2, -1,
            ],
            &[-12],
            0.2,
            0.04,
            true,
        ),
        "cave" => {
            let mut lp = 0.0f32;
            samples(12.0, move |t, n| {
                lp += (n.next() - lp) * 0.01;
                let rumble = lp * 0.6;
                // Water drips at fixed (looping) times.
                let drips = [0.7, 3.1, 5.4, 7.9, 10.2];
                let d: f32 = drips
                    .iter()
                    .map(|h| {
                        if t >= *h {
                            (TAU * (900.0 + 500.0 * (t - h)) * t).sin()
                                * (-(t - h) * 40.0).exp()
                                * 0.25
                        } else {
                            0.0
                        }
                    })
                    .sum();
                rumble + d
            })
        }
        "wind" => {
            let mut lp = 0.0f32;
            samples(12.0, move |t, n| {
                let cutoff = 0.02 + 0.03 * (0.5 + 0.5 * (TAU * t / 12.0).sin());
                lp += (n.next() - lp) * cutoff;
                lp * 2.2
            })
        }
        "fire" => samples(10.0, |t, n| {
            let base = n.next() * 0.06;
            // Crackles: sparse impulses from the noise source.
            let c = n.next();
            let crackle = if c > 0.995 { n.next() * 0.8 } else { 0.0 };
            base * (0.7 + 0.3 * (TAU * t * 0.3).sin()) + crackle
        }),
        "forest" => {
            let mut lp = 0.0f32;
            samples(12.0, move |t, n| {
                lp += (n.next() - lp) * 0.03;
                let chirps = [1.0, 1.15, 4.6, 8.2, 8.35];
                let c: f32 = chirps
                    .iter()
                    .map(|h| {
                        if t >= *h && t < h + 0.12 {
                            (TAU * (3200.0 + 2000.0 * (t - h) / 0.12) * t).sin()
                                * 0.12
                                * (1.0 - (t - h) / 0.12)
                        } else {
                            0.0
                        }
                    })
                    .sum();
                lp * 0.8 + c
            })
        }
        _ => return None,
    })
}

#[derive(Default)]
struct LoopSlot {
    id: Option<String>,
    entity: Option<Entity>,
}

/// Plays the room's music and ambience (battle music during combat in your room).
#[derive(Resource, Default)]
pub struct Soundscape {
    cache: HashMap<String, Handle<AudioSource>>,
    music: LoopSlot,
    ambience: LoopSlot,
}

pub fn update_soundscape(
    mut commands: Commands,
    mut scape: ResMut<Soundscape>,
    sources: Option<ResMut<Assets<AudioSource>>>,
    gv: Res<crate::state::GameView>,
    settings: Res<AudioSettings>,
    mut sinks: Query<&mut AudioSink>,
) {
    let Some(mut sources) = sources else { return };
    let (want_music, want_amb) = match gv
        .view
        .as_ref()
        .and_then(|v| v.room.as_ref().map(|r| (v, r)))
    {
        Some((v, r)) => {
            let fighting = v.combat.is_some() && r.is_party_room;
            (
                if fighting {
                    Some("battle".to_string())
                } else {
                    r.music.clone()
                },
                r.ambience.clone(),
            )
        }
        None => (None, None),
    };
    let scape = &mut *scape;
    for (slot, want, vol) in [
        (
            &mut scape.music,
            want_music,
            settings.master * settings.music * 0.6,
        ),
        (
            &mut scape.ambience,
            want_amb,
            settings.master * settings.ambience,
        ),
    ] {
        if slot.id != want {
            if let Some(e) = slot.entity.take() {
                commands.entity(e).despawn();
            }
            slot.id = want.clone();
            if let Some(id) = want {
                let handle = match scape.cache.get(&id) {
                    Some(h) => Some(h.clone()),
                    None => synth_loop(&id).map(|s| {
                        let bytes: Arc<[u8]> = wav(&s).into();
                        let h = sources.add(AudioSource { bytes });
                        scape.cache.insert(id.clone(), h.clone());
                        h
                    }),
                };
                if let Some(h) = handle {
                    slot.entity = Some(
                        commands
                            .spawn((
                                AudioPlayer::new(h),
                                PlaybackSettings {
                                    volume: Volume::Linear(vol),
                                    ..PlaybackSettings::LOOP
                                },
                            ))
                            .id(),
                    );
                }
            }
        } else if let Some(mut sink) = slot.entity.and_then(|e| sinks.get_mut(e).ok()) {
            use bevy::audio::AudioSinkPlayback;
            if (sink.volume().to_linear() - vol).abs() > 0.01 {
                sink.set_volume(Volume::Linear(vol));
            }
        }
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

    #[test]
    fn every_listed_track_synthesizes() {
        for id in MUSIC_TRACKS.iter().chain(AMBIENCE_TRACKS) {
            let s = synth_loop(id).unwrap_or_else(|| panic!("{id}"));
            assert!(s.len() > 22_050 * 5, "{id} too short");
            assert!(s.iter().all(|v| v.is_finite()), "{id} has NaN");
        }
        assert!(synth_loop("nope").is_none());
    }
}
