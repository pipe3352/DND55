//! Ongoing effects created by spells and features: lights, buffs, conditions with repeated saves.
//! Durations are counted in rounds and tick at the start of the creator's turn during combat;
//! outside combat they last until dismissed, concentration ends, or a Long Rest.

use dnd_rules::Ability;
use dnd_rules::conditions::Condition;
use dnd_world::map::Cell;
use dnd_world::{EntityId, RoomId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightAnchor {
    Creature(EntityId),
    Cell(RoomId, Cell),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectKind {
    Light { anchor: LightAnchor, bright_ft: u32, dim_ft: u32 },
    /// Bless: +1d4 to attack rolls and saving throws.
    Bless { targets: Vec<EntityId> },
    /// Mage Armor: base AC 13 + Dex while unarmored.
    MageArmor { target: EntityId },
    /// Shield: +5 AC until the start of the caster's next turn.
    Shield { target: EntityId },
    /// Sorcerer Innate Sorcery: +1 spell save DC, Advantage on Sorcerer spell attacks.
    InnateSorcery { target: EntityId },
    /// A condition imposed by a spell or feature; with `repeat_save` the target repeats the save at the
    /// end of each of its turns.
    Condition { target: EntityId, condition: Condition, save: Option<(Ability, i32)>, repeat_save: bool, ends_at_source_turn: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effect {
    pub id: u64,
    pub source: EntityId,
    /// Spell id or feature name.
    pub origin: String,
    pub kind: EffectKind,
    /// Rounds left (ticks at the start of the source's turn). `None` = until dismissed.
    pub rounds_left: Option<u32>,
    pub concentration: bool,
}

impl Effect {
    pub fn targets(&self) -> Vec<EntityId> {
        match &self.kind {
            EffectKind::Light { anchor: LightAnchor::Creature(e), .. } => vec![*e],
            EffectKind::Light { .. } => vec![],
            EffectKind::Bless { targets } => targets.clone(),
            EffectKind::MageArmor { target } | EffectKind::Shield { target } | EffectKind::InnateSorcery { target } => vec![*target],
            EffectKind::Condition { target, .. } => vec![*target],
        }
    }

    pub fn label(&self) -> String {
        let name = dnd_rules::spells::spell(&self.origin).map(|s| s.name.clone()).unwrap_or_else(|| self.origin.clone());
        match self.rounds_left {
            Some(r) if r < u32::MAX / 2 => format!("{name} ({r} rounds)"),
            _ => name,
        }
    }
}
