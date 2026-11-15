//! Random number generation for dice.
//!
//! Game-affecting rolls are made **only** on the authoritative host using [`GameRng`]
//! (ChaCha20 seeded from the operating system). Tests use fixed seeds or [`ScriptedRng`].

use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

/// Source of uniformly distributed die results.
pub trait DiceRng {
    /// Returns a uniformly distributed value in `1..=sides`. `sides` must be >= 1.
    fn roll_die(&mut self, sides: u32) -> u32;
}

/// Cryptographically strong, unbiased dice RNG used by the authoritative host.
pub struct GameRng(ChaCha20Rng);

impl GameRng {
    /// Seeds from the OS entropy source. Falls back to a time-based seed only if the OS source fails.
    pub fn from_entropy() -> Self {
        let seed = getrandom::u64().unwrap_or_else(|_| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x5eed)
        });
        Self(ChaCha20Rng::seed_from_u64(seed))
    }

    /// Deterministic generator, for tests and replays.
    pub fn seeded(seed: u64) -> Self {
        Self(ChaCha20Rng::seed_from_u64(seed))
    }
}

impl DiceRng for GameRng {
    fn roll_die(&mut self, sides: u32) -> u32 {
        assert!(sides >= 1, "a die needs at least one side");
        // Rejection sampling: avoids the modulo bias of `next_u32() % sides`.
        let zone = u32::MAX - (u32::MAX % sides);
        loop {
            let v = self.0.next_u32();
            if v < zone {
                return v % sides + 1;
            }
        }
    }
}

/// Returns pre-scripted results in order. Used by tests to force exact rolls
/// (e.g. a natural 20). Panics if it runs out of values, which makes test intent explicit.
#[derive(Debug, Clone, Default)]
pub struct ScriptedRng {
    values: std::collections::VecDeque<u32>,
}

impl ScriptedRng {
    pub fn new(values: impl IntoIterator<Item = u32>) -> Self {
        Self {
            values: values.into_iter().collect(),
        }
    }
    pub fn remaining(&self) -> usize {
        self.values.len()
    }
}

impl DiceRng for ScriptedRng {
    fn roll_die(&mut self, sides: u32) -> u32 {
        let v = self.values.pop_front().expect("ScriptedRng exhausted");
        assert!(
            v >= 1 && v <= sides,
            "scripted value {v} impossible on a d{sides}"
        );
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_rng_stays_in_range_and_covers_all_faces() {
        let mut rng = GameRng::seeded(42);
        for sides in [4u32, 6, 8, 10, 12, 20, 100] {
            let mut seen = vec![false; sides as usize];
            for _ in 0..(sides * 200) {
                let v = rng.roll_die(sides);
                assert!((1..=sides).contains(&v));
                seen[(v - 1) as usize] = true;
            }
            assert!(seen.iter().all(|s| *s), "d{sides} never produced some face");
        }
    }

    #[test]
    fn d20_distribution_is_roughly_uniform() {
        let mut rng = GameRng::seeded(7);
        let mut counts = [0u32; 20];
        let n = 200_000;
        for _ in 0..n {
            counts[(rng.roll_die(20) - 1) as usize] += 1;
        }
        let expected = n as f64 / 20.0;
        for c in counts {
            assert!(((c as f64) - expected).abs() / expected < 0.05);
        }
    }
}
