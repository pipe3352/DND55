//! Events describe *validated* outcomes. Clients animate them (movement, floating text, VFX) but
//! never decide outcomes from them.

use crate::model::Audience;
use dnd_world::map::Cell;
use dnd_world::{EntityId, RoomId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FloatKind {
    Damage,
    CriticalDamage,
    Healing,
    Miss,
    TempHp,
    Condition,
    Resisted,
    Immune,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vfx {
    Slash,
    Pierce,
    Bludgeon,
    Arrow,
    Fire,
    Cold,
    Lightning,
    Poison,
    Acid,
    Radiant,
    Necrotic,
    Force,
    Thunder,
    Psychic,
    Healing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Log {
        text: String,
    },
    DiceRolled {
        actor: String,
        label: String,
        detail: String,
        total: i32,
    },
    Moved {
        entity: EntityId,
        path: Vec<Cell>,
    },
    AttackVisual {
        attacker: EntityId,
        target: EntityId,
        vfx: Vfx,
        ranged: bool,
        hit: bool,
    },
    Floating {
        entity: EntityId,
        text: String,
        kind: FloatKind,
    },
    RoomChanged {
        room: RoomId,
        name: String,
    },
    CombatStarted,
    TurnStarted {
        entity: EntityId,
        round: u32,
    },
    CombatEnded,
    ReactionPrompt {
        reactor: EntityId,
        mover: EntityId,
    },
    Chat {
        from: String,
        text: String,
    },
    Notice {
        text: String,
    },
}

impl Event {
    /// The creature an event is about; used to avoid leaking hidden creatures to players.
    pub fn subject(&self) -> Option<EntityId> {
        match self {
            Event::Moved { entity, .. }
            | Event::Floating { entity, .. }
            | Event::TurnStarted { entity, .. } => Some(*entity),
            Event::AttackVisual { target, .. } => Some(*target),
            Event::ReactionPrompt { reactor, .. } => Some(*reactor),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Routed {
    pub audience: Audience,
    pub event: Event,
}
