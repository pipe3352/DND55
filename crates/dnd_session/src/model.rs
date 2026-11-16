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
pub const STATE_FORMAT_VERSION: u32 = 2;

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
    /// Expended class/spell resources.
    #[serde(default)]
    pub resources: Resources,
    /// Spell (id) this creature is concentrating on.
    #[serde(default)]
    pub concentration: Option<String>,
    /// Creature type ("Humanoid" for player characters).
    #[serde(default = "humanoid")]
    pub creature_type: String,
    /// Saving throw bonuses by ability (index = Ability::index()).
    #[serde(default)]
    pub saves: [i32; 6],
    /// Ability modifiers.
    #[serde(default)]
    pub mods: [i32; 6],
    #[serde(default = "one_u8")]
    pub attacks_per_action: u8,
    /// Monster save-based actions (breath weapons...).
    #[serde(default)]
    pub save_actions: Vec<dnd_rules::monster::SaveAction>,
    #[serde(default)]
    pub status: CombatStatus,
}

fn humanoid() -> String {
    "Humanoid".into()
}
fn one_u8() -> u8 {
    1
}

/// Expended uses of limited features. Reset by rests (see `Session::rest`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Resources {
    pub slots_used: [u8; 9],
    pub pact_used: u8,
    pub rage_used: u8,
    pub second_wind_used: u8,
    pub action_surge_used: u8,
    pub focus_used: u8,
    pub lay_on_hands_used: i32,
    pub bardic_used: u8,
    pub channel_divinity_used: u8,
    pub innate_sorcery_used: u8,
    pub free_smite_used: bool,
    pub arcane_recovery_used: bool,
    pub hit_dice_used: u8,
    /// Monster save actions (by index) waiting to recharge.
    pub recharging: Vec<usize>,
}

/// Ongoing per-creature combat states set by features and spells.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CombatStatus {
    /// Barbarian Rage is active; `rage_extended` = extended during the current turn.
    pub raging: bool,
    pub rage_extended: bool,
    pub rage_rounds: u32,
    /// Reckless Attack until the start of this creature's next turn.
    pub reckless: bool,
    /// Next attack roll against this creature has Advantage (Guiding Bolt, Stunning Strike save).
    pub exposed: Option<(EntityId, u32)>,
    /// Can't take Reactions (Shocking Grasp) until the start of its next turn.
    pub no_reactions: bool,
    /// Can't regain HP until the end of this caster's next turn (Chill Touch).
    pub no_healing_from: Option<(EntityId, u32)>,
    /// Speed reduction until the start of the source's next turn (Ray of Frost, Slow mastery).
    pub speed_penalty: Option<(EntityId, i32)>,
    /// Bardic Inspiration die held (sides) and whether to add it to the next D20 Test.
    pub inspiration: Option<u32>,
    pub inspiration_armed: bool,
    /// Stunning Strike prepared for the next hit this turn.
    pub stunning_strike_ready: bool,
    /// Milestone levels granted by the DM (allow level up without XP).
    pub milestone_levels: u8,
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
    pub fn sheet_mut(&mut self) -> Option<&mut CharacterSheet> {
        match &mut self.kind {
            CreatureKind::Pc { sheet } => Some(sheet),
            CreatureKind::Monster { .. } => None,
        }
    }
    pub fn effective_speed(&self) -> i32 {
        let penalty = self.status.speed_penalty.map(|(_, p)| p).unwrap_or(0);
        self.conditions
            .effective_speed((self.speed_ft - penalty).max(0))
    }
    pub fn save_bonus(&self, a: dnd_rules::Ability) -> i32 {
        self.saves[a.index()]
    }
    pub fn level(&self) -> u8 {
        self.sheet().map(|s| s.level).unwrap_or(0)
    }
    pub fn is_class(&self, c: dnd_rules::content::ClassId) -> bool {
        self.sheet().is_some_and(|s| s.class == c)
    }
    pub fn has_feature(&self, name: &str) -> bool {
        self.sheet().is_some_and(|s| s.has_class_feature(name))
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
            armor_class: 0,
            speed_ft: 0,
            senses: Senses::default(),
            defenses: Defenses::default(),
            attacks: vec![],
            initiative_bonus: 0,
            dex_score: 10,
            hidden: false,
            portrait: None,
            effects: ActiveEffects::default(),
            carried_light: None,
            resources: Resources::default(),
            concentration: None,
            creature_type: humanoid(),
            saves: [0; 6],
            mods: [0; 6],
            attacks_per_action: 1,
            save_actions: vec![],
            status: CombatStatus::default(),
            kind: CreatureKind::Pc {
                sheet: Box::new(sheet),
            },
        };
        c.refresh_from_sheet();
        c
    }

    /// Recomputes derived statistics after the sheet changed (level up, equipment, ASI).
    pub fn refresh_from_sheet(&mut self) {
        let Some(sheet) = self.sheet().cloned() else {
            return;
        };
        let scores = sheet.scores();
        self.name = sheet.name.trim().to_string();
        let new_max = sheet.max_hp();
        if new_max != self.hp.max {
            let delta = new_max - self.hp.max;
            self.hp.max = new_max;
            if !self.hp.dead {
                self.hp.current = (self.hp.current + delta.max(0)).clamp(0, new_max);
            }
        }
        self.armor_class = sheet.armor_class();
        self.speed_ft = sheet.speed_ft();
        self.senses = Senses {
            darkvision_ft: sheet.darkvision_ft(),
            ..Default::default()
        };
        self.attacks = sheet.attacks();
        self.initiative_bonus = sheet.initiative_bonus();
        self.dex_score = scores.get(dnd_rules::Ability::Dexterity);
        self.attacks_per_action = sheet.attacks_per_action();
        for a in dnd_rules::Ability::ALL {
            self.saves[a.index()] = sheet.save_bonus(a);
            self.mods[a.index()] = sheet.modifier(a);
        }
        self.defenses = Defenses::default();
        if sheet.species == dnd_rules::content::SpeciesId::Dwarf {
            // Dwarven Resilience: Resistance to Poison damage.
            self.defenses
                .resistances
                .insert(dnd_rules::DamageType::Poison);
        }
    }

    pub fn from_template(
        id: EntityId,
        t: &MonsterTemplate,
        name: String,
        room: RoomId,
        pos: Cell,
    ) -> Self {
        let mut mods = [0; 6];
        for a in dnd_rules::Ability::ALL {
            mods[a.index()] = t.scores.modifier(a);
        }
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
                darkvision_ft: t.darkvision_ft.max(t.truesight_ft),
                blindsight_ft: t.blindsight_ft,
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
            resources: Resources::default(),
            concentration: None,
            creature_type: t.creature_type.clone(),
            saves: t.saves,
            mods,
            attacks_per_action: t.attacks_per_action.max(1),
            save_actions: t.save_actions.clone(),
            status: CombatStatus::default(),
        }
    }
}

/// Purely visual atmosphere chosen by the DM per room. It never changes visibility or rules
/// (use the room's ambient light and heavily-obscured areas for that).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum WeatherKind {
    #[default]
    None,
    Rain,
    Snow,
    Fog,
    Leaves,
    Dust,
    Embers,
    Fireflies,
    Magic,
    Ash,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    pub kind: WeatherKind,
    /// 0..1
    pub intensity: f32,
}

impl Default for Weather {
    fn default() -> Self {
        Weather {
            kind: WeatherKind::None,
            intensity: 0.5,
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
    #[serde(default)]
    pub ambience: Option<String>,
    /// Cosmetic weather/atmosphere particles for this room (no rules effect).
    #[serde(default)]
    pub weather: Weather,
    /// DM (and permitted player) drawings on the map.
    #[serde(default)]
    pub drawings: Vec<Drawing>,
    /// Container contents by object id (hidden from players until opened).
    #[serde(default)]
    pub containers: BTreeMap<dnd_world::ObjectId, Vec<dnd_rules::items::ItemStack>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawVisibility {
    All,
    DmOnly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DrawShape {
    /// Freehand stroke through world-space points (in cells, fractional).
    Stroke(Vec<(f32, f32)>),
    Line((f32, f32), (f32, f32)),
    Arrow((f32, f32), (f32, f32)),
    Circle((f32, f32), f32),
    Text((f32, f32), String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Drawing {
    pub id: u64,
    pub shape: DrawShape,
    pub color: [u8; 4],
    pub width: f32,
    pub visibility: DrawVisibility,
    /// Temporary drawings are cleared together with "Clear temporary".
    pub temporary: bool,
    pub author: String,
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
            ambience: None,
            weather: Weather::default(),
            drawings: vec![],
            containers: BTreeMap::new(),
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
    /// Remaining attacks of the current Attack action (Extra Attack, Multiattack).
    #[serde(default)]
    pub attacks_left: u8,
    /// Extra bonus-action Unarmed Strikes available (Martial Arts, Flurry of Blows).
    #[serde(default)]
    pub bonus_attacks: u8,
    /// Action Surge gave an extra action that can't be the Magic action.
    #[serde(default)]
    pub extra_actions: u8,
    /// "On a turn, you can expend only one spell slot to cast a spell."
    #[serde(default)]
    pub spell_slot_used: bool,
    #[serde(default)]
    pub sneak_attack_used: bool,
    #[serde(default)]
    pub frenzy_used: bool,
    #[serde(default)]
    pub moved: bool,
    #[serde(default)]
    pub steady_aim: bool,
    #[serde(default)]
    pub surge_used_this_turn: bool,
    /// Last creature hit with a melee weapon/Unarmed Strike this turn and whether it was a crit (Divine Smite).
    #[serde(default)]
    pub last_melee_hit: Option<(EntityId, bool)>,
}

impl TurnState {
    pub fn fresh(entity: EntityId, movement_left_ft: i32) -> Self {
        TurnState {
            entity,
            movement_left_ft,
            action_used: false,
            bonus_action_used: false,
            disengaged: false,
            attacks_left: 0,
            bonus_attacks: 0,
            extra_actions: 0,
            spell_slot_used: false,
            sneak_attack_used: false,
            frenzy_used: false,
            moved: false,
            steady_aim: false,
            surge_used_this_turn: false,
            last_melee_hit: None,
        }
    }
}

/// An attack that hit and is waiting for the target's reaction (Shield, Uncanny Dodge, Deflect Attacks).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingHit {
    pub attacker: EntityId,
    pub target: EntityId,
    pub result: dnd_rules::attack::AttackResult,
    pub ranged: bool,
    pub melee_weapon_hit: bool,
    /// Options the target may choose.
    pub options: Vec<HitReaction>,
    pub vfx: crate::event::Vfx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitReaction {
    Shield,
    UncannyDodge,
    DeflectAttacks,
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
    #[serde(default)]
    pub pending_hit: Option<PendingHit>,
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
    /// Players may add (temporary, visible-to-all) drawings.
    #[serde(default)]
    pub players_can_draw: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            diagonal_rule: DiagonalRule::Uniform,
            shared_party_vision: true,
            exploration_movement: true,
            players_can_draw: false,
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
    /// Active spell and feature effects.
    #[serde(default)]
    pub effects: Vec<crate::effects::Effect>,
    /// DM-authored scripted events.
    #[serde(default)]
    pub scripts: Vec<crate::scripts::ScriptedEvent>,
    #[serde(default)]
    pub pending_dialogue: Option<crate::scripts::PendingDialogue>,
}

pub const LOG_CAP: usize = 1000;
