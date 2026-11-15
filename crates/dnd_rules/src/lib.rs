//! # dnd_rules — D&D 2024 rules engine (SRD 5.2.1)
//!
//! Pure, deterministic rules logic with **no dependency on rendering, networking or IO**.
//! Everything here can be unit tested without launching the game.
//!
//! Every rule implemented in this crate cites the SRD 5.2.1 section it comes from.
//! Rules that are not yet implemented are tracked in `docs/RULES_COVERAGE.md`.
//!
//! This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1")
//! by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1
//! is licensed under the Creative Commons Attribution 4.0 International License,
//! available at https://creativecommons.org/licenses/by/4.0/legalcode.

pub mod ability;
pub mod attack;
pub mod character;
pub mod conditions;
pub mod content;
pub mod d20;
pub mod damage;
pub mod dice;
pub mod equipment;
pub mod hp;
pub mod monster;
pub mod rng;

pub use ability::{Ability, AbilityScores, Skill, ability_modifier, proficiency_bonus};
pub use d20::{D20Roll, RollMode};
pub use damage::{DamageType, Defenses};
pub use dice::{DiceExpr, DiceRoll};
pub use rng::{DiceRng, GameRng};

/// The rules edition this engine implements. Shown in UI and stored in saves so that
/// content authored for another ruleset is never silently mixed in.
pub const RULESET_ID: &str = "srd-5.2.1";

/// CC-BY-4.0 attribution statement required by SRD 5.2.1 (copied verbatim from its Legal Information page).
pub const SRD_ATTRIBUTION: &str = "This work includes material from the System Reference Document 5.2.1 (\u{201c}SRD 5.2.1\u{201d}) by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.";
