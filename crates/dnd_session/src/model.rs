//! Persistent game state. Everything in [`GameState`] is saved; transient UI state never lives here.

use dnd_rules::attack::AttackProfile;
use dnd_rules::character::CharacterSheet;
use dnd_rules::conditions::ConditionSet;
use dnd_rules::content::Size;
use dnd_rules::d20::D20Roll;
use dnd_rules::damage::Defenses;
use dnd_rules::hp::{HitPoints, ZeroHpRule};
use dnd_rules::monster::MonsterTemplate;
use dnd_world::map::{Cell, GridMap};
use dnd_world::path::DiagonalRule;
use dnd_world::vision::{PointLight, Senses};
use dnd_world::{EntityId, RoomId, WorldGraph};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Bumped whenever the serialized layout changes; `dnd_persist` migrates older saves.
pub const STATE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlayerId(pub u32);

/// Who is issuing a command. The DM is a distinct principal with authority over everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Principal {
    Dm,
    Player(PlayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Controller {
    Dm,
    Player(PlayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Faction {
    Party,
    Hostile,
    Neutral,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureKind {
    Pc { sheet: Box<CharacterSheet> },
    Monster { template_id: String },
}

/// Short-lived rule effects tracked between turns.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveEffects {
    /// Sap: Disadvantage on this creature's next attack roll before the start of the source's next turn.
    pub sapped_by: Option<EntityId>,
    /// Vex: Advantage on the next attack roll against the target before the end of this creature's next turn.
    /// Value = number of this creature's turn ends remaining.
    pub vex: Vec<(EntityId, u8)>,
    /// Dodge: until the start of this creature's next turn.
    pub dodging: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub id: EntityId,
    pub name: String,
    pub room: RoomId,
    pub pos: Cell,
    pub kind: CreatureKind,
    pub controller: Controller,
    pub faction: Faction,
    pub size: Size,
    pub hp: HitPoints,
    pub conditions: ConditionSet,
    pub armor_class: i32,
    pub speed_ft: i32,
    pub senses: Senses,
    pub defenses: Defenses,
    pub attacks: Vec<AttackProfile>,
    pub initiative_bonus: i32,
    pub dex_score: u8,
    /// Hidden creatures are never sent to players.
    pub hidden: bool,
    /// SHA-256 hex of the portrait PNG in the asset store.
    pub portrait: Option<String>,
    pub effects: ActiveEffects,
    /// Light carried by the creature (torch, Light cantrip on its gear...).
    pub carried_light: Option<(u32, u32)>,
}

impl Creature {
    pub fn is_alive(&self) -> bool {
        !self.hp.dead
    }
    pub fn sheet(&self) -> Option<&CharacterSheet> {
        match &self.kind {
            CreatureKind::Pc { sheet } => Some(sheet),
            CreatureKind::Monster { .. } => None,
        }
    }
    pub fn effective_speed(&self) -> i32 {
        self.conditions.effective_speed(self.speed_ft)
    }
    /// Longest melee reach among this creature's attacks (5 ft minimum: everyone can Unarmed Strike).
    pub fn melee_reach_ft(&self) -> u32 {
        self.attacks
            .iter()
            .filter(|a| a.kind == dnd_rules::equipment::WeaponKind::Melee)
            .map(|a| a.reach_ft)
            .max()
            .unwrap_or(5)
            .max(5)
    }

    pub fn from_sheet(
        id: EntityId,
        sheet: CharacterSheet,
        room: RoomId,
        pos: Cell,
        player: PlayerId,
    ) -> Self {
        let scores = sheet.scores();
        let mut c = Creature {
            id,
            name: sheet.name.trim().to_string(),
            room,
            pos,
            controller: Controller::Player(player),
            faction: Faction::Party,
            size: sheet.size,
            hp: HitPoints::new(sheet.max_hp(), ZeroHpRule::Character),
            conditions: ConditionSet::default(),
            armor_class: sheet.armor_class(),
            speed_ft: sheet.speed_ft(),
            senses: Senses {
                darkvision_ft: sheet.darkvision_ft(),
                ..Default::default()
            },
            defenses: Defenses::default(),
            attacks: sheet.attacks(),
            initiative_bonus: sheet.initiative_bonus(),
            dex_score: scores.get(dnd_rules::Ability::Dexterity),
            hidden: false,
            portrait: None,
            effects: ActiveEffects::default(),
            carried_light: None,
            kind: CreatureKind::Pc {
                sheet: Box::new(sheet),
            },
        };
        if c.sheet()
            .is_some_and(|s| s.species == dnd_rules::content::SpeciesId::Dwarf)
        {
            // Dwarven Resilience: Resistance to Poison damage.
            c.defenses.resistances.insert(dnd_rules::DamageType::Poison);
        }
        c
    }

    pub fn from_template(
        id: EntityId,
        t: &MonsterTemplate,
        name: String,
        room: RoomId,
        pos: Cell,
    ) -> Self {
        Creature {
            id,
            name,
            room,
            pos,
            kind: CreatureKind::Monster {
                template_id: t.id.clone(),
            },
            controller: Controller::Dm,
            faction: Faction::Hostile,
            size: t.size,
            hp: HitPoints::new(t.hit_points, ZeroHpRule::MonsterDies),
            conditions: ConditionSet::default(),
            armor_class: t.armor_class,
            speed_ft: t.speed_ft,
            senses: Senses {
                darkvision_ft: t.darkvision_ft,
                ..Default::default()
            },
            defenses: t.defenses.clone(),
            attacks: t.attacks.clone(),
            initiative_bonus: t.initiative_bonus,
            dex_score: t.scores.get(dnd_rules::Ability::Dexterity),
            hidden: false,
            portrait: None,
            effects: ActiveEffects::default(),
            carried_light: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomState {
    pub map: GridMap,
    /// Incremented on every map edit; lets the server resend the map only when it changed.
    pub map_rev: u64,
    /// Default arrival cell.
    pub entry: Cell,
    pub visited: bool,
    /// Party-shared exploration (fog of war "explored" layer).
    pub explored: Vec<bool>,
    /// Spell or DM-placed lights not tied to an object.
    pub extra_lights: Vec<PointLight>,
    pub music: Option<String>,
}

impl RoomState {
    pub fn new(map: GridMap) -> Self {
        let n = (map.width * map.height) as usize;
        RoomState {
            entry: (1, 1),
            map,
            map_rev: 1,
            visited: false,
            explored: vec![false; n],
            extra_lights: vec![],
            music: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: PlayerId,
    pub name: String,
    pub character: Option<EntityId>,
    pub ready: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitiativeEntry {
    pub entity: EntityId,
    pub roll: D20Roll,
    pub total: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnState {
    pub entity: EntityId,
    pub movement_left_ft: i32,
    pub action_used: bool,
    pub bonus_action_used: bool,
    pub disengaged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingReaction {
    pub reactor: EntityId,
    pub mover: EntityId,
    /// Cells the mover still intends to enter after the reaction resolves.
    pub remaining_path: Vec<(Cell, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Combat {
    pub room: RoomId,
    pub round: u32,
    pub order: Vec<InitiativeEntry>,
    pub index: usize,
    pub turn: TurnState,
    pub reactions_used: BTreeSet<EntityId>,
    pub pending: Option<PendingReaction>,
    pub paused: bool,
}

impl Combat {
    pub fn active(&self) -> EntityId {
        self.turn.entity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Audience {
    All,
    DmOnly,
    /// The DM and one player (private/whispered rolls).
    DmAnd(PlayerId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    pub seq: u64,
    pub audience: Audience,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub diagonal_rule: DiagonalRule,
    /// Players see what any party member sees (and share exploration).
    pub shared_party_vision: bool,
    /// Out of combat, players may move freely.
    pub exploration_movement: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            diagonal_rule: DiagonalRule::Uniform,
            shared_party_vision: true,
            exploration_movement: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    pub format_version: u32,
    pub ruleset: String,
    pub campaign_name: String,
    pub settings: Settings,
    pub graph: WorldGraph,
    pub rooms: BTreeMap<RoomId, RoomState>,
    pub party_room: RoomId,
    pub creatures: BTreeMap<EntityId, Creature>,
    pub players: BTreeMap<PlayerId, PlayerInfo>,
    pub combat: Option<Combat>,
    pub custom_monsters: Vec<MonsterTemplate>,
    pub log: Vec<LogEntry>,
    pub session_started: bool,
    pub dm_view_room: RoomId,
    pub next_entity: u64,
    pub next_player: u32,
    pub next_log: u64,
}

pub const LOG_CAP: usize = 1000;
