//! The authoritative session: validates commands from untrusted principals and applies the rules.
//!
//! Invariant: a command either succeeds completely or returns an error **without mutating state**.
//! Handlers therefore validate everything first and mutate last.

use crate::assets::AssetStore;
use crate::command::{Command, DmCommand};
use crate::effects::EffectKind;
use crate::event::{Event, FloatKind, Routed, Vfx};
use crate::model::*;
use dnd_rules::attack::{AttackModifiers, DamagePart, resolve_attack};
use dnd_rules::character::ScoreMethod;
use dnd_rules::conditions::{
    AttackSituation, Condition, attack_roll_sources, hit_becomes_critical,
};
use dnd_rules::d20::{D20Roll, ModeSources, RollMode};
use dnd_rules::damage::DamageType;
use dnd_rules::dice::DiceExpr;
use dnd_rules::equipment::WeaponKind;
use dnd_rules::hp::{DeathSaveResult, LifeState};
use dnd_rules::monster::{MonsterTemplate, srd_bestiary};
use dnd_rules::rng::{DiceRng, GameRng};
use dnd_world::catalog::Catalog;
use dnd_world::map::{AmbientLight, Cell, GridMap, MapObject};
use dnd_world::path::{MoveContext, find_path, grid_distance_ft};
use dnd_world::vision::{
    LightLevel, PointLight, Seen, collect_lights, light_map, line_of_sight, perceive,
};
use dnd_world::{EntityId, ObjectId, RoomId};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    #[error("not authorized")]
    NotAuthorized,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("rules: {0}")]
    Rules(String),
}

type Res<T> = Result<T, CommandError>;

fn invalid<T>(s: impl Into<String>) -> Res<T> {
    Err(CommandError::Invalid(s.into()))
}
fn rules<T>(s: impl Into<String>) -> Res<T> {
    Err(CommandError::Rules(s.into()))
}

type RoomSnapshot = (GridMap, Vec<PointLight>);
const UNDO_DEPTH: usize = 64;

pub struct Session {
    pub state: GameState,
    pub assets: AssetStore,
    pub catalog: Catalog,
    pub(crate) rng: Box<dyn DiceRng + Send>,
    undo: HashMap<RoomId, (Vec<RoomSnapshot>, Vec<RoomSnapshot>)>,
    pub(crate) out: Vec<Routed>,
}

impl Session {
    /// A new campaign with one blank room.
    pub fn new_campaign(name: &str) -> Self {
        let mut graph = dnd_world::WorldGraph::default();
        let first = graph.add_room("Entrance");
        let mut rooms = BTreeMap::new();
        rooms.insert(
            first,
            RoomState::new(GridMap::new(20, 15, AmbientLight::Bright).expect("valid size")),
        );
        let state = GameState {
            format_version: STATE_FORMAT_VERSION,
            ruleset: dnd_rules::RULESET_ID.into(),
            campaign_name: name.into(),
            settings: Settings::default(),
            graph,
            rooms,
            party_room: first,
            creatures: BTreeMap::new(),
            players: BTreeMap::new(),
            combat: None,
            custom_monsters: vec![],
            log: vec![],
            session_started: false,
            dm_view_room: first,
            next_entity: 1,
            next_player: 1,
            next_log: 1,
            effects: vec![],
            scripts: vec![],
            pending_dialogue: None,
        };
        Self::from_state(
            state,
            AssetStore::default(),
            Box::new(GameRng::from_entropy()),
        )
    }

    pub fn from_state(state: GameState, assets: AssetStore, rng: Box<dyn DiceRng + Send>) -> Self {
        Session {
            state,
            assets,
            catalog: Catalog::builtin(),
            rng,
            undo: HashMap::new(),
            out: vec![],
        }
    }

    /// Replaces the RNG (tests use scripted dice).
    pub fn set_rng(&mut self, rng: Box<dyn DiceRng + Send>) {
        self.rng = rng;
    }

    pub fn bestiary(&self) -> Vec<MonsterTemplate> {
        let mut b = srd_bestiary();
        b.extend(self.state.custom_monsters.iter().cloned());
        b
    }

    // ------------------------------------------------------------------ players

    pub fn add_player(&mut self, name: &str) -> PlayerId {
        let id = PlayerId(self.state.next_player);
        self.state.next_player += 1;
        let name: String = name.trim().chars().take(32).collect();
        self.state.players.insert(
            id,
            PlayerInfo {
                id,
                name: name.clone(),
                character: None,
                ready: false,
                connected: true,
            },
        );
        self.log(Audience::All, format!("{name} joined the session."));
        id
    }

    pub fn set_connected(&mut self, p: PlayerId, connected: bool) {
        if let Some(info) = self.state.players.get_mut(&p) {
            info.connected = connected;
            let name = info.name.clone();
            self.log(
                Audience::All,
                format!(
                    "{name} {}.",
                    if connected {
                        "reconnected"
                    } else {
                        "disconnected"
                    }
                ),
            );
        }
    }

    pub fn find_player_by_name(&self, name: &str) -> Option<PlayerId> {
        self.state
            .players
            .values()
            .find(|p| p.name.eq_ignore_ascii_case(name.trim()))
            .map(|p| p.id)
    }

    // ------------------------------------------------------------------ helpers

    pub(crate) fn log(&mut self, audience: Audience, text: impl Into<String>) {
        let text = text.into();
        let seq = self.state.next_log;
        self.state.next_log += 1;
        self.state.log.push(LogEntry {
            seq,
            audience,
            text: text.clone(),
        });
        if self.state.log.len() > LOG_CAP {
            let excess = self.state.log.len() - LOG_CAP;
            self.state.log.drain(0..excess);
        }
        self.out.push(Routed {
            audience,
            event: Event::Log { text },
        });
    }

    pub(crate) fn emit(&mut self, audience: Audience, event: Event) {
        self.out.push(Routed { audience, event });
    }

    pub(crate) fn next_id(&mut self) -> u64 {
        let id = self.state.next_entity;
        self.state.next_entity += 1;
        id
    }

    pub fn creature(&self, id: EntityId) -> Res<&Creature> {
        self.state
            .creatures
            .get(&id)
            .ok_or_else(|| CommandError::NotFound(id.to_string()))
    }
    pub(crate) fn creature_mut(&mut self, id: EntityId) -> &mut Creature {
        self.state
            .creatures
            .get_mut(&id)
            .expect("validated before mutation")
    }
    pub(crate) fn room(&self, id: RoomId) -> Res<&RoomState> {
        self.state
            .rooms
            .get(&id)
            .ok_or_else(|| CommandError::NotFound(id.to_string()))
    }

    pub fn controls(who: Principal, c: &Creature) -> bool {
        match who {
            Principal::Dm => true,
            Principal::Player(p) => c.controller == Controller::Player(p),
        }
    }

    pub(crate) fn require_dm(who: Principal) -> Res<()> {
        if who == Principal::Dm {
            Ok(())
        } else {
            Err(CommandError::NotAuthorized)
        }
    }

    pub(crate) fn display_name(&self, who: Principal) -> String {
        match who {
            Principal::Dm => "DM".into(),
            Principal::Player(p) => self
                .state
                .players
                .get(&p)
                .map(|i| i.name.clone())
                .unwrap_or_else(|| "?".into()),
        }
    }

    pub fn are_hostile(a: &Creature, b: &Creature) -> bool {
        matches!(
            (a.faction, b.faction),
            (Faction::Party, Faction::Hostile) | (Faction::Hostile, Faction::Party)
        )
    }

    /// Light level of every cell of a room, including lights carried by creatures in it.
    /// Every light in a room besides light-emitting map objects: DM-placed lights, carried light
    /// sources and spell lights (Light, Daylight...) anchored to creatures or cells.
    pub fn dynamic_lights(&self, room: RoomId) -> Vec<PointLight> {
        let Ok(r) = self.room(room) else {
            return vec![];
        };
        let mut extra = r.extra_lights.clone();
        extra.extend(
            self.state
                .creatures
                .values()
                .filter(|c| c.room == room && c.is_alive())
                .filter_map(|c| {
                    c.carried_light.map(|(b, d)| PointLight {
                        cell: c.pos,
                        bright_ft: b,
                        dim_ft: d,
                    })
                }),
        );
        for e in &self.state.effects {
            if let EffectKind::Light {
                anchor,
                bright_ft,
                dim_ft,
            } = &e.kind
            {
                let cell = match anchor {
                    crate::effects::LightAnchor::Creature(id) => self
                        .state
                        .creatures
                        .get(id)
                        .filter(|c| c.room == room)
                        .map(|c| c.pos),
                    crate::effects::LightAnchor::Cell(r, c) => (*r == room).then_some(*c),
                };
                if let Some(cell) = cell {
                    extra.push(PointLight {
                        cell,
                        bright_ft: *bright_ft,
                        dim_ft: *dim_ft,
                    });
                }
            }
        }
        extra
    }

    pub fn light_levels(&self, room: RoomId) -> Vec<LightLevel> {
        let Ok(r) = self.room(room) else {
            return vec![];
        };
        light_map(&r.map, &collect_lights(&r.map, &self.dynamic_lights(room)))
    }

    /// Gameplay visibility: can `viewer` see `target`? Uses light, walls, Darkvision, Blindsight, Blinded and Invisible.
    pub fn can_see(&self, viewer: &Creature, target: &Creature, lights: &[LightLevel]) -> bool {
        if viewer.room != target.room {
            return false;
        }
        let Ok(r) = self.room(viewer.room) else {
            return false;
        };
        let mut senses = viewer.senses;
        senses.blinded = viewer.conditions.has(Condition::Blinded);
        if target.conditions.has(Condition::Invisible) {
            // Only Blindsight (or similar special senses) perceives an Invisible creature.
            let d = grid_distance_ft(viewer.pos, target.pos) as u32;
            return senses.blindsight_ft > 0
                && d <= senses.blindsight_ft
                && line_of_sight(&r.map, viewer.pos, target.pos);
        }
        !matches!(
            perceive(&r.map, lights, viewer.pos, senses, target.pos),
            Seen::NotVisible
        )
    }

    pub(crate) fn combat_in(&self, room: RoomId) -> Option<&Combat> {
        self.state.combat.as_ref().filter(|c| c.room == room)
    }

    // ------------------------------------------------------------------ apply

    /// Validates and applies a command. Returns the events to route to clients.
    pub fn apply(&mut self, who: Principal, cmd: Command) -> Res<Vec<Routed>> {
        self.out.clear();
        if let Principal::Player(p) = who
            && !self.state.players.contains_key(&p)
        {
            return Err(CommandError::NotAuthorized);
        }
        let edited = cmd.edited_room();
        let snapshot = edited.and_then(|r| {
            self.state
                .rooms
                .get(&r)
                .map(|rs| (rs.map.clone(), rs.extra_lights.clone()))
        });
        let result = self.dispatch(who, cmd);
        match result {
            Ok(()) => {
                if let (Some(room), Some(snap)) = (edited, snapshot) {
                    let (undo, redo) = self.undo.entry(room).or_default();
                    undo.push(snap);
                    if undo.len() > UNDO_DEPTH {
                        undo.remove(0);
                    }
                    redo.clear();
                    if let Some(rs) = self.state.rooms.get_mut(&room) {
                        rs.map_rev += 1;
                    }
                }
                self.update_fog();
                Ok(std::mem::take(&mut self.out))
            }
            Err(e) => {
                self.out.clear();
                Err(e)
            }
        }
    }

    pub(crate) fn dispatch(&mut self, who: Principal, cmd: Command) -> Res<()> {
        match cmd {
            Command::Chat { text } => {
                let text: String = text.trim().chars().take(500).collect();
                if text.is_empty() {
                    return invalid("empty message");
                }
                let from = self.display_name(who);
                self.emit(Audience::All, Event::Chat { from, text });
                Ok(())
            }
            Command::RollDice {
                expr,
                mode,
                private,
                label,
            } => self.roll_dice(who, &expr, mode, private, label),
            Command::SubmitCharacter {
                sheet,
                portrait_png,
            } => self.submit_character(who, *sheet, portrait_png),
            Command::SetReady(r) => {
                let Principal::Player(p) = who else {
                    return invalid("only players ready up");
                };
                self.state.players.get_mut(&p).expect("checked").ready = r;
                Ok(())
            }
            Command::Move { entity, to } => self.cmd_move(who, entity, to),
            Command::Attack {
                attacker,
                target,
                attack,
            } => self.cmd_attack(who, attacker, target, attack),
            Command::Dash { entity } => self.cmd_simple_action(who, entity, "Dash"),
            Command::Disengage { entity } => self.cmd_simple_action(who, entity, "Disengage"),
            Command::Dodge { entity } => self.cmd_simple_action(who, entity, "Dodge"),
            Command::EndTurn => self.cmd_end_turn(who),
            Command::ReactionDecision { accept, attack } => self.cmd_reaction(who, accept, attack),
            Command::Interact { entity, object } => self.cmd_interact(who, entity, object),
            Command::CastSpell {
                caster,
                spell,
                slot,
                targets,
                point,
            } => self.cmd_cast_spell(who, caster, &spell, slot, targets, point),
            Command::UseFeature {
                entity,
                feature,
                target,
                amount,
            } => self.cmd_use_feature(who, entity, feature, target, amount),
            Command::HitReaction { choice } => self.cmd_hit_reaction(who, choice),
            Command::EndConcentration { entity } => {
                let c = self.creature(entity)?;
                if !Self::controls(who, c) {
                    return Err(CommandError::NotAuthorized);
                }
                if c.concentration.is_none() {
                    return rules("not concentrating");
                }
                self.end_concentration(entity, "ended by choice");
                Ok(())
            }
            Command::PrepareSpells {
                entity,
                cantrips,
                prepared,
            } => self.cmd_prepare_spells(who, entity, cantrips, prepared),
            Command::LevelUp { entity, choices } => self.cmd_level_up(who, entity, *choices),
            Command::SpendHitDie { entity } => self.cmd_spend_hit_die(who, entity),
            Command::UseItem {
                entity,
                item,
                target,
            } => self.cmd_use_item(who, entity, &item, target),
            Command::EquipItem {
                entity,
                item,
                equip,
            } => self.cmd_equip(who, entity, &item, equip),
            Command::GiveItem {
                from,
                to,
                item,
                quantity,
            } => self.cmd_give(who, from, to, &item, quantity),
            Command::TakeFromContainer {
                entity,
                object,
                item,
                quantity,
            } => self.cmd_take_from_container(who, entity, object, &item, quantity),
            Command::MonsterAction {
                monster,
                action,
                targets,
                point,
            } => self.cmd_monster_action(who, monster, action, targets, point),
            Command::DialogueChoice { choice } => {
                if self.state.pending_dialogue.is_none() {
                    return invalid("no dialogue is waiting");
                }
                self.cmd_dialogue_choice(choice)
            }
            Command::AddDrawing { room, drawing } => self.cmd_add_drawing(who, room, *drawing),
            Command::Dm(d) => {
                Self::require_dm(who)?;
                self.dm(d)
            }
        }
    }

    // ------------------------------------------------------------------ dice

    pub(crate) fn roll_dice(
        &mut self,
        who: Principal,
        expr: &str,
        mode: RollMode,
        private: bool,
        label: String,
    ) -> Res<()> {
        let parsed = DiceExpr::parse(expr).map_err(|e| CommandError::Invalid(e.to_string()))?;
        let label: String = label.chars().take(60).collect();
        let actor = self.display_name(who);
        let audience = match (private, who) {
            (false, _) => Audience::All,
            (true, Principal::Dm) => Audience::DmOnly,
            (true, Principal::Player(p)) => Audience::DmAnd(p),
        };
        let (detail, total) = if mode != RollMode::Normal {
            // Advantage/Disadvantage only applies to d20 tests: require exactly one d20.
            let is_d20 = parsed.terms.len() == 1
                && parsed.terms[0].count == 1
                && parsed.terms[0].sides == 20
                && !parsed.terms[0].negative;
            if !is_d20 {
                return invalid("Advantage/Disadvantage requires a single d20 (e.g. 1d20+5)");
            }
            let r = D20Roll::roll(self.rng.as_mut(), mode, parsed.modifier, 0);
            (r.breakdown(), r.total)
        } else {
            let r = parsed.roll(self.rng.as_mut(), false);
            (r.breakdown(), r.total)
        };
        self.log(
            audience,
            format!(
                "{actor} rolls {}{}: {detail}",
                if label.is_empty() { "" } else { "for " },
                label
            ),
        );
        self.emit(
            audience,
            Event::DiceRolled {
                actor,
                label,
                detail,
                total,
            },
        );
        Ok(())
    }

    // ------------------------------------------------------------------ characters

    pub(crate) fn submit_character(
        &mut self,
        who: Principal,
        sheet: dnd_rules::character::CharacterSheet,
        png: Option<Vec<u8>>,
    ) -> Res<()> {
        let Principal::Player(p) = who else {
            return invalid("the DM creates NPCs with SpawnMonster");
        };
        let errors: Vec<String> = sheet
            .validate()
            .into_iter()
            .filter(|i| i.severity == dnd_rules::character::Severity::Error)
            .map(|i| i.detail)
            .collect();
        if !errors.is_empty() {
            return invalid(errors.join("; "));
        }
        // Rolled scores must be rolled by the host (pending feature); manual scores need a DM override.
        // Until then, players can't claim them — otherwise a client could submit any "rolled" 18s.
        if matches!(
            sheet.score_method,
            ScoreMethod::Rolled | ScoreMethod::ManualDmApproved
        ) {
            return rules(
                "rolled or manual ability scores need host-side rolling / DM approval (not yet available); use the Standard Array or Point Buy",
            );
        }
        let existing = self.state.players[&p].character;
        if existing.is_some() && self.state.combat.is_some() {
            return rules("can't change your character during combat");
        }
        let portrait = match png {
            Some(bytes) => Some(
                self.assets
                    .add_portrait(&bytes)
                    .map_err(|e| CommandError::Invalid(e.to_string()))?,
            ),
            None => None,
        };
        let room = self.state.party_room;
        let name = sheet.name.trim().to_string();
        match existing.filter(|id| self.state.creatures.contains_key(id)) {
            Some(id) => {
                let old = self.state.creatures[&id].clone();
                let mut c = Creature::from_sheet(id, sheet, old.room, old.pos, p);
                c.portrait = portrait.or(old.portrait);
                self.state.creatures.insert(id, c);
                self.log(
                    Audience::All,
                    format!("{} updated their character {name}.", self.display_name(who)),
                );
            }
            None => {
                let entry = self.room(room)?.entry;
                let pos = self.free_cell_near(room, entry, None).ok_or_else(|| {
                    CommandError::Rules("no free space at the entry point".into())
                })?;
                let id = EntityId(self.next_id());
                let mut c = Creature::from_sheet(id, sheet, room, pos, p);
                c.portrait = portrait;
                self.state.creatures.insert(id, c);
                self.state.players.get_mut(&p).expect("checked").character = Some(id);
                self.log(Audience::All, format!("{name} enters the adventure."));
            }
        }
        Ok(())
    }

    /// Nearest cell (by ring) to `near` that is walkable and unoccupied.
    pub(crate) fn free_cell_near(
        &self,
        room: RoomId,
        near: Cell,
        ignore: Option<EntityId>,
    ) -> Option<Cell> {
        let r = self.state.rooms.get(&room)?;
        let occupied = |c: Cell| {
            self.state
                .creatures
                .values()
                .any(|cr| cr.room == room && cr.is_alive() && cr.pos == c && Some(cr.id) != ignore)
        };
        for radius in 0..(r.map.width.max(r.map.height)) {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    let c = (near.0 + dx, near.1 + dy);
                    if r.map.in_bounds(c) && !r.map.flags(c).blocks_movement && !occupied(c) {
                        return Some(c);
                    }
                }
            }
        }
        None
    }

    // ------------------------------------------------------------------ movement

    pub(crate) fn cmd_move(&mut self, who: Principal, entity: EntityId, to: Cell) -> Res<()> {
        let c = self.creature(entity)?;
        if !Self::controls(who, c) {
            return Err(CommandError::NotAuthorized);
        }
        if !c.is_alive() {
            return rules("dead creatures can't move");
        }
        if c.effective_speed() == 0 {
            return rules(format!("{} has Speed 0", c.name));
        }
        if who != Principal::Dm && c.room != self.state.party_room {
            return Err(CommandError::NotAuthorized);
        }
        let room = c.room;
        let budget = match self.combat_in(room) {
            Some(cb) => {
                if cb.paused {
                    return rules("combat is paused");
                }
                if cb.pending.is_some() {
                    return rules("waiting for a reaction decision");
                }
                if cb.active() != entity {
                    return rules("it isn't this creature's turn");
                }
                cb.turn.movement_left_ft
            }
            None => {
                if who != Principal::Dm && !self.state.settings.exploration_movement {
                    return rules("the DM has paused exploration movement");
                }
                i32::MAX
            }
        };
        let ctx = Occupancy::new(self, entity);
        let r = self.room(room)?;
        let path = find_path(
            &r.map,
            &ctx,
            c.pos,
            to,
            self.state.settings.diagonal_rule,
            budget,
        )
        .ok_or_else(|| CommandError::Rules("no legal path within remaining movement".into()))?;
        let steps: Vec<(Cell, i32)> = path.steps.iter().map(|s| (s.cell, s.cost_ft)).collect();
        self.walk(entity, steps);
        Ok(())
    }

    /// Moves step by step, stopping when an Opportunity Attack is possible (the attack happens right
    /// before the mover leaves the reactor's reach). Assumes the path was validated.
    pub(crate) fn walk(&mut self, entity: EntityId, steps: Vec<(Cell, i32)>) {
        let mut walked = vec![self.state.creatures[&entity].pos];
        let room = self.state.creatures[&entity].room;
        let in_combat = self.combat_in(room).is_some();
        let lights = if in_combat {
            self.light_levels(room)
        } else {
            vec![]
        };
        for (i, (cell, cost)) in steps.iter().enumerate() {
            if in_combat {
                let from = self.state.creatures[&entity].pos;
                if let Some(reactor) = self.opportunity_reactor(entity, from, *cell, &lights) {
                    let remaining = steps[i..].to_vec();
                    let cb = self.state.combat.as_mut().expect("in combat");
                    cb.pending = Some(PendingReaction {
                        reactor,
                        mover: entity,
                        remaining_path: remaining,
                    });
                    if walked.len() > 1 {
                        self.emit(
                            Audience::All,
                            Event::Moved {
                                entity,
                                path: walked.clone(),
                            },
                        );
                    }
                    let (rn, mn) = (
                        self.state.creatures[&reactor].name.clone(),
                        self.state.creatures[&entity].name.clone(),
                    );
                    self.log(
                        Audience::All,
                        format!("{mn} is about to leave {rn}'s reach — Opportunity Attack?"),
                    );
                    self.emit(
                        Audience::All,
                        Event::ReactionPrompt {
                            reactor,
                            mover: entity,
                        },
                    );
                    return;
                }
                let cb = self.state.combat.as_mut().expect("in combat");
                cb.turn.movement_left_ft -= cost;
            }
            self.creature_mut(entity).pos = *cell;
            walked.push(*cell);
            if in_combat && let Some(cb) = self.state.combat.as_mut() {
                cb.turn.moved = true;
            }
            // Scripted area triggers fire for player characters stepping into the area.
            if self.state.creatures[&entity].faction == Faction::Party
                && !self.state.scripts.is_empty()
            {
                let room = self.state.creatures[&entity].room;
                self.fire_scripts_for(
                    crate::scripts::Trigger::EnterArea {
                        room,
                        a: *cell,
                        b: *cell,
                    },
                    Some(entity),
                );
                if !self
                    .state
                    .creatures
                    .get(&entity)
                    .is_some_and(|c| c.is_alive() && c.hp.current > 0)
                {
                    break;
                }
            }
        }
        if walked.len() > 1 {
            self.emit(
                Audience::All,
                Event::Moved {
                    entity,
                    path: walked,
                },
            );
        }
    }

    pub(crate) fn opportunity_reactor(
        &self,
        mover: EntityId,
        from: Cell,
        to: Cell,
        lights: &[LightLevel],
    ) -> Option<EntityId> {
        let m = &self.state.creatures[&mover];
        let cb = self.state.combat.as_ref()?;
        if cb.turn.entity == mover && cb.turn.disengaged {
            return None;
        }
        self.state
            .creatures
            .values()
            .filter(|h| h.room == m.room && h.id != mover && h.is_alive() && h.hp.current > 0)
            .filter(|h| Self::are_hostile(h, m))
            .filter(|h| h.conditions.can_act() && !cb.reactions_used.contains(&h.id))
            .filter(|h| h.attacks.iter().any(|a| a.kind == WeaponKind::Melee))
            .filter(|h| {
                let reach = h.melee_reach_ft() as i32;
                grid_distance_ft(h.pos, from) <= reach && grid_distance_ft(h.pos, to) > reach
            })
            .find(|h| self.can_see(h, m, lights))
            .map(|h| h.id)
    }

    // ------------------------------------------------------------------ actions

    pub(crate) fn active_turn_check(&self, who: Principal, entity: EntityId) -> Res<()> {
        let c = self.creature(entity)?;
        if !Self::controls(who, c) {
            return Err(CommandError::NotAuthorized);
        }
        let Some(cb) = self.combat_in(c.room) else {
            return rules("this needs combat — the DM can start an encounter");
        };
        if cb.paused {
            return rules("combat is paused");
        }
        if cb.pending.is_some() {
            return rules("waiting for a reaction decision");
        }
        if cb.active() != entity {
            return rules("it isn't this creature's turn");
        }
        if !c.conditions.can_act() {
            return rules(format!("{} is Incapacitated", c.name));
        }
        Ok(())
    }

    pub(crate) fn cmd_simple_action(
        &mut self,
        who: Principal,
        entity: EntityId,
        action: &str,
    ) -> Res<()> {
        self.active_turn_check(who, entity)?;
        let t = &self.state.combat.as_ref().expect("checked").turn;
        if t.action_used && t.extra_actions == 0 {
            return rules("action already used this turn");
        }
        let speed = self.state.creatures[&entity].effective_speed();
        let name = self.state.creatures[&entity].name.clone();
        let cb = self.state.combat.as_mut().expect("checked");
        if cb.turn.action_used {
            cb.turn.extra_actions -= 1;
        } else {
            cb.turn.action_used = true;
        }
        match action {
            // Dash: extra movement equal to your Speed after modifiers.
            "Dash" => cb.turn.movement_left_ft += speed,
            "Disengage" => cb.turn.disengaged = true,
            "Dodge" => self.creature_mut(entity).effects.dodging = true,
            _ => unreachable!(),
        }
        self.log(Audience::All, format!("{name} takes the {action} action."));
        self.emit(
            Audience::All,
            Event::Floating {
                entity,
                text: action.to_string(),
                kind: FloatKind::Info,
            },
        );
        Ok(())
    }

    pub(crate) fn cmd_attack(
        &mut self,
        who: Principal,
        attacker: EntityId,
        target: EntityId,
        attack: usize,
    ) -> Res<()> {
        self.active_turn_check(who, attacker)?;
        let a = self.creature(attacker)?;
        let per_action = a.attacks_per_action.max(1);
        let unarmed = a
            .attacks
            .get(attack)
            .is_some_and(|p| p.name == "Unarmed Strike");
        let turn = &self.state.combat.as_ref().expect("checked").turn;
        // Which resource pays for this attack: the running Attack action, a new action, or a bonus attack.
        let source = if turn.attacks_left > 0 {
            0
        } else if !turn.action_used || turn.extra_actions > 0 {
            1
        } else if turn.bonus_attacks > 0 && unarmed {
            2
        } else if turn.bonus_attacks > 0 {
            return rules("bonus-action attacks must be Unarmed Strikes");
        } else {
            return rules("no attacks left this turn");
        };
        self.validate_attack(attacker, target, attack)?;
        let turn = &mut self.state.combat.as_mut().expect("checked").turn;
        match source {
            0 => turn.attacks_left -= 1,
            1 => {
                if !turn.action_used {
                    turn.action_used = true;
                } else {
                    turn.extra_actions -= 1;
                }
                turn.attacks_left = per_action - 1;
            }
            _ => turn.bonus_attacks -= 1,
        }
        self.perform_attack(attacker, target, attack);
        Ok(())
    }

    pub(crate) fn validate_attack(
        &self,
        attacker: EntityId,
        target: EntityId,
        attack: usize,
    ) -> Res<()> {
        let a = self.creature(attacker)?;
        let t = self.creature(target)?;
        if attacker == target {
            return invalid("can't attack yourself");
        }
        if a.room != t.room || !t.is_alive() {
            return rules("target isn't a living creature in this room");
        }
        let profile = a
            .attacks
            .get(attack)
            .ok_or_else(|| CommandError::NotFound("attack".into()))?;
        let dist = grid_distance_ft(a.pos, t.pos);
        match profile.kind {
            WeaponKind::Melee if dist > profile.reach_ft as i32 => {
                return rules(format!(
                    "target is {dist} ft away; reach is {} ft",
                    profile.reach_ft
                ));
            }
            WeaponKind::Ranged => {
                let (_, long) = profile.range_ft.unwrap_or((5, 5));
                if dist > long as i32 {
                    return rules(format!("target is beyond long range ({long} ft)"));
                }
            }
            _ => {}
        }
        if !line_of_sight(&self.room(a.room)?.map, a.pos, t.pos) {
            return rules("no line of sight (Total Cover)");
        }
        Ok(())
    }

    /// Resolves an already-validated attack and applies its results.
    /// Resolves an already-validated attack. If the target can react to the hit (Shield, Uncanny Dodge,
    /// Deflect Attacks) resolution pauses until it decides; otherwise the hit is applied immediately.
    pub(crate) fn perform_attack(&mut self, attacker: EntityId, target: EntityId, attack: usize) {
        let room = self.state.creatures[&attacker].room;
        let lights = self.light_levels(room);
        let a = self.state.creatures[&attacker].clone();
        let t = self.state.creatures[&target].clone();
        let profile = a.attacks[attack].clone();
        let dist = grid_distance_ft(a.pos, t.pos);
        let round = self.state.combat.as_ref().map(|c| c.round).unwrap_or(0);
        let own_turn = self
            .state
            .combat
            .as_ref()
            .is_some_and(|c| c.turn.entity == attacker);

        let mut sources = ModeSources::default();
        let situation = AttackSituation {
            attacker_within_5ft: dist <= 5,
            attacker_can_see_target: self.can_see(&a, &t, &lights),
            target_can_see_attacker: self.can_see(&t, &a, &lights),
            target_is_attackers_grappler: false,
            // Conservative: assume the source of fear is in sight (DM can clear the condition).
            attacker_fear_source_in_sight: true,
            target_dodging: t.effects.dodging,
        };
        attack_roll_sources(&a.conditions, &t.conditions, situation, &mut sources);
        if profile.kind == WeaponKind::Ranged {
            if let Some((normal, _)) = profile.range_ft
                && dist > normal as i32
            {
                sources.dis("beyond normal range");
            }
            if self.threatened_in_melee(&a, &lights) {
                sources.dis("ranged attack with an enemy within 5 ft");
            }
        }
        if a.effects.sapped_by.is_some() {
            sources.dis("Sap");
        }
        if a.effects.vex.iter().any(|(v, _)| *v == target) {
            sources.adv("Vex");
        }
        let strength_based = profile.ability == Some(dnd_rules::Ability::Strength);
        if t.status.exposed.is_some() {
            sources.adv("target exposed");
        }
        if t.status.reckless {
            sources.adv("target attacked recklessly");
        }
        if a.status.reckless && strength_based {
            sources.adv("Reckless Attack");
        }
        if own_turn
            && self
                .state
                .combat
                .as_ref()
                .is_some_and(|c| c.turn.steady_aim)
        {
            sources.adv("Steady Aim");
        }
        let mut mods = AttackModifiers::new();
        mods.exhaustion_level = a.conditions.exhaustion;
        mods.hit_is_auto_critical = hit_becomes_critical(&t.conditions, dist <= 5);
        let weapon_or_unarmed = profile.weapon_id.is_some() || profile.name == "Unarmed Strike";
        if let Some(sheet) = a.sheet() {
            if weapon_or_unarmed {
                mods.crit_threshold = sheet.crit_threshold();
            }
            if sheet.fighting_style
                == Some(dnd_rules::character::FightingStyle::GreatWeaponFighting)
                && profile.two_handed_melee
            {
                mods.min_damage_die = 3;
            }
        }
        if self.has_bless(attacker) {
            mods.to_hit_dice
                .push(("Bless".into(), DiceExpr::dice(1, 4)));
        }
        if let (true, Some(die)) = (a.status.inspiration_armed, a.status.inspiration) {
            mods.on_miss_bonus = Some(("Bardic Inspiration".into(), DiceExpr::dice(1, die)));
        }
        let dtype = profile
            .damage
            .first()
            .map(|d| d.damage_type)
            .unwrap_or(DamageType::Bludgeoning);
        let turn_flags = self
            .state
            .combat
            .as_ref()
            .filter(|_| own_turn)
            .map(|c| (c.turn.sneak_attack_used, c.turn.frenzy_used));
        // Rage Damage: Strength-based weapon attacks and Unarmed Strikes.
        if a.status.raging && strength_based {
            let bonus = dnd_rules::classes::rage(a.level().max(1)).1;
            mods.extra_damage_on_hit.push((
                "Rage".into(),
                DamagePart {
                    dice: DiceExpr::default().with_modifier(bonus),
                    damage_type: dtype,
                },
            ));
            // Frenzy (Berserker 3): Reckless + Rage: first target hit each turn takes (rage bonus)d6.
            if a.status.reckless && a.has_feature("Frenzy") && turn_flags.is_some_and(|f| !f.1) {
                mods.extra_damage_on_hit.push((
                    "Frenzy".into(),
                    DamagePart {
                        dice: DiceExpr::dice(bonus as u32, 6),
                        damage_type: dtype,
                    },
                ));
            }
        }
        // Sneak Attack: once per turn, Finesse or Ranged weapon, Advantage — or an ally within 5 ft of the
        // target that isn't Incapacitated, as long as the attack doesn't have Disadvantage.
        if a.is_class(dnd_rules::content::ClassId::Rogue)
            && profile.finesse_or_ranged
            && profile.weapon_id.is_some()
            && turn_flags.is_some_and(|f| !f.0)
        {
            let mode = sources.mode();
            let ally_adjacent = self.state.creatures.values().any(|c| {
                c.id != attacker
                    && c.room == room
                    && c.is_alive()
                    && c.faction == a.faction
                    && grid_distance_ft(c.pos, t.pos) <= 5
                    && !c.conditions.has(Condition::Incapacitated)
            });
            if mode == RollMode::Advantage || (ally_adjacent && mode != RollMode::Disadvantage) {
                let dice = dnd_rules::classes::sneak_attack_dice(a.level().max(1));
                mods.extra_damage_on_hit.push((
                    "Sneak Attack".into(),
                    DamagePart {
                        dice,
                        damage_type: dtype,
                    },
                ));
            }
        }
        mods.sources = sources;
        let (target_ac, target_def) = (self.effective_ac(&t), self.defenses_of(&t));
        let result = resolve_attack(self.rng.as_mut(), &profile, target_ac, &target_def, &mods);

        // One-shot modifiers are consumed by the attack roll they affect.
        {
            let ac = self.creature_mut(attacker);
            ac.effects.sapped_by = None;
            ac.effects.vex.retain(|(v, _)| *v != target);
            if result.on_miss_bonus_used {
                ac.status.inspiration = None;
                ac.status.inspiration_armed = false;
            }
            // Attacking extends Rage, and reveals a DM-hidden creature.
            if ac.status.raging && Self::are_hostile(&a, &t) {
                ac.status.rage_extended = true;
            }
            ac.hidden = false;
        }
        if t.status.exposed.is_some() {
            self.creature_mut(target).status.exposed = None;
        }
        if own_turn && let Some(cb) = self.state.combat.as_mut() {
            cb.turn.steady_aim = false;
        }
        let vfx = vfx_for(profile.damage.first().map(|d| d.damage_type), profile.kind);
        self.emit(
            Audience::All,
            Event::AttackVisual {
                attacker,
                target,
                vfx,
                ranged: profile.kind == WeaponKind::Ranged,
                hit: result.hit,
            },
        );
        self.log(Audience::All, result.log_line(&a.name, &t.name));
        let _ = round;

        let melee_weapon_hit = result.hit && profile.kind == WeaponKind::Melee && weapon_or_unarmed;
        let ranged = profile.kind == WeaponKind::Ranged;
        if result.hit {
            let options = self.hit_reaction_options(&a, &t, &result);
            if !options.is_empty() && self.state.combat.is_some() {
                self.state.combat.as_mut().expect("in combat").pending_hit = Some(PendingHit {
                    attacker,
                    target,
                    result,
                    ranged,
                    melee_weapon_hit,
                    options,
                    vfx,
                });
                self.emit(
                    Audience::All,
                    Event::ReactionPrompt {
                        reactor: target,
                        mover: attacker,
                    },
                );
                return;
            }
        }
        self.finalize_attack(
            attacker,
            target,
            result,
            melee_weapon_hit,
            profile.ability_mod,
        );
    }

    /// Reactions the target could take against a hit.
    pub(crate) fn hit_reaction_options(
        &self,
        a: &Creature,
        t: &Creature,
        result: &dnd_rules::attack::AttackResult,
    ) -> Vec<HitReaction> {
        let mut v = Vec::new();
        let Some(cb) = self.state.combat.as_ref() else {
            return v;
        };
        if cb.reactions_used.contains(&t.id)
            || t.status.no_reactions
            || !t.conditions.can_act()
            || t.hp.current == 0
        {
            return v;
        }
        if let Some(sheet) = t.sheet() {
            let ac = self.effective_ac(t);
            if sheet.knows_spell("shield")
                && !result.d20.is_natural_20()
                && result.d20.total < ac + 5
                && self.lowest_free_slot(t, 1).is_some()
            {
                v.push(HitReaction::Shield);
            }
            let lights = self.light_levels(t.room);
            if t.has_feature("Uncanny Dodge") && self.can_see(t, a, &lights) {
                v.push(HitReaction::UncannyDodge);
            }
            let physical = result.damage.iter().any(|d| {
                matches!(
                    d.outcome.damage_type,
                    DamageType::Bludgeoning | DamageType::Piercing | DamageType::Slashing
                )
            });
            if t.has_feature("Deflect Attacks") && physical {
                v.push(HitReaction::DeflectAttacks);
            }
        }
        v
    }

    /// Applies an attack's results (masteries, features, damage) after any reaction.
    pub(crate) fn finalize_attack(
        &mut self,
        attacker: EntityId,
        target: EntityId,
        result: dnd_rules::attack::AttackResult,
        melee_weapon_hit: bool,
        ability_mod: i32,
    ) {
        let own_turn = self
            .state
            .combat
            .as_ref()
            .is_some_and(|c| c.turn.entity == attacker);
        if result.hit {
            let labels: Vec<&str> = result
                .damage
                .iter()
                .flat_map(|d| d.rolls.iter().map(|(l, _)| l.as_str()))
                .collect();
            if let Some(cb) = self.state.combat.as_mut().filter(|_| own_turn) {
                if labels.contains(&"Sneak Attack") {
                    cb.turn.sneak_attack_used = true;
                }
                if labels.contains(&"Frenzy") {
                    cb.turn.frenzy_used = true;
                }
                if melee_weapon_hit {
                    cb.turn.last_melee_hit = Some((target, result.critical));
                }
            }
        }
        let a = self.state.creatures[&attacker].clone();
        match result.mastery_triggered {
            Some(dnd_rules::equipment::Mastery::Sap) => {
                self.creature_mut(target).effects.sapped_by = Some(attacker)
            }
            Some(dnd_rules::equipment::Mastery::Vex) => {
                // Until the end of the attacker's *next* turn.
                self.creature_mut(attacker)
                    .effects
                    .vex
                    .push((target, if own_turn { 2 } else { 1 }));
            }
            Some(dnd_rules::equipment::Mastery::Slow) if result.total_damage > 0 => {
                // Slow: Speed −10 ft until the start of the attacker's next turn (doesn't stack beyond 10).
                self.creature_mut(target).status.speed_penalty = Some((attacker, 10));
                self.log(Audience::All, "Slow mastery: Speed reduced by 10 ft.");
            }
            Some(dnd_rules::equipment::Mastery::Topple) => {
                // Topple: Con save DC 8 + ability mod + PB or Prone (optional for the wielder; applied, logged).
                let pb = a.sheet().map(|s| s.proficiency_bonus()).unwrap_or(2);
                let dc = 8 + ability_mod + pb;
                if !self
                    .saving_throw(target, dnd_rules::Ability::Constitution, dc, "Topple")
                    .0
                {
                    self.creature_mut(target).conditions.add(Condition::Prone);
                    self.emit(
                        Audience::All,
                        Event::Floating {
                            entity: target,
                            text: "Prone".into(),
                            kind: FloatKind::Condition,
                        },
                    );
                }
            }
            Some(dnd_rules::equipment::Mastery::Push)
                if self.state.creatures[&target].size as u8
                    <= dnd_rules::content::Size::Large as u8 =>
            {
                self.push_away(attacker, target, 10);
            }
            _ => {}
        }
        // Stunning Strike (Monk 5): prepared before the attack, used on the next hit with a Monk weapon or Unarmed Strike.
        if result.hit && melee_weapon_hit && a.status.stunning_strike_ready {
            self.creature_mut(attacker).status.stunning_strike_ready = false;
            self.creature_mut(attacker).resources.focus_used += 1;
            let dc = 8
                + a.mods[dnd_rules::Ability::Wisdom.index()]
                + a.sheet().map(|s| s.proficiency_bonus()).unwrap_or(2);
            if self
                .saving_throw(
                    target,
                    dnd_rules::Ability::Constitution,
                    dc,
                    "Stunning Strike",
                )
                .0
            {
                let t = self.creature_mut(target);
                t.status.speed_penalty = Some((attacker, t.speed_ft / 2));
                t.status.exposed = Some((attacker, 0));
            } else {
                let id = self.add_effect(
                    attacker,
                    "Stunning Strike",
                    EffectKind::Condition {
                        target,
                        condition: Condition::Stunned,
                        save: None,
                        repeat_save: false,
                        ends_at_source_turn: true,
                    },
                    Some(1),
                    false,
                );
                let _ = id;
                self.creature_mut(target).conditions.add(Condition::Stunned);
                self.emit(
                    Audience::All,
                    Event::Floating {
                        entity: target,
                        text: "Stunned".into(),
                        kind: FloatKind::Condition,
                    },
                );
            }
        }

        if !result.hit && result.total_damage == 0 {
            self.emit(
                Audience::All,
                Event::Floating {
                    entity: target,
                    text: "Miss".into(),
                    kind: FloatKind::Miss,
                },
            );
            return;
        }
        let immune = !result.damage.is_empty() && result.damage.iter().all(|d| d.outcome.immune);
        let resisted = result.damage.iter().any(|d| d.outcome.resisted);
        let kind = if immune {
            FloatKind::Immune
        } else if result.critical {
            FloatKind::CriticalDamage
        } else if resisted {
            FloatKind::Resisted
        } else {
            FloatKind::Damage
        };
        let text = if immune {
            "Immune".into()
        } else if result.critical {
            format!("-{} CRIT!", result.total_damage)
        } else if !result.hit {
            format!("-{} (graze)", result.total_damage)
        } else {
            format!("-{}", result.total_damage)
        };
        self.emit(
            Audience::All,
            Event::Floating {
                entity: target,
                text,
                kind,
            },
        );
        self.apply_damage_from(target, result.total_damage, result.critical, Some(attacker));
    }

    /// Answer to a pending hit: Shield (+5 AC, may turn the hit into a miss), Uncanny Dodge (halve),
    /// Deflect Attacks (reduce by 1d10 + Dex + Monk level), or take it.
    pub(crate) fn cmd_hit_reaction(
        &mut self,
        who: Principal,
        choice: Option<HitReaction>,
    ) -> Res<()> {
        let pending = self
            .state
            .combat
            .as_ref()
            .and_then(|c| c.pending_hit.clone())
            .ok_or_else(|| CommandError::Invalid("no hit is waiting for a reaction".into()))?;
        let t = self.creature(pending.target)?.clone();
        if !Self::controls(who, &t) {
            return Err(CommandError::NotAuthorized);
        }
        if let Some(c) = choice
            && !pending.options.contains(&c)
        {
            return rules("that reaction isn't available");
        }
        let mut result = pending.result.clone();
        let cb = self.state.combat.as_mut().expect("checked");
        cb.pending_hit = None;
        if choice.is_some() {
            cb.reactions_used.insert(pending.target);
        }
        match choice {
            Some(HitReaction::Shield) => {
                let slot = self.lowest_free_slot(&t, 1).expect("checked when offered");
                self.spend_slot(pending.target, slot);
                self.add_effect(
                    pending.target,
                    "shield",
                    EffectKind::Shield {
                        target: pending.target,
                    },
                    Some(1),
                    false,
                );
                let ac = self.effective_ac(&self.state.creatures[&pending.target].clone());
                self.emit(
                    Audience::All,
                    Event::Floating {
                        entity: pending.target,
                        text: "Shield!".into(),
                        kind: FloatKind::Info,
                    },
                );
                self.emit(
                    Audience::All,
                    Event::SpellVisual {
                        caster: pending.target,
                        name: "Shield".into(),
                        vfx: Vfx::Force,
                        from: t.pos,
                        targets: vec![t.pos],
                        area: vec![],
                    },
                );
                if !result.d20.is_natural_20() && result.d20.total < ac {
                    result.hit = false;
                    result.critical = false;
                    result.total_damage = 0;
                    result.damage.clear();
                    self.log(
                        Audience::All,
                        format!("{} casts Shield (AC {ac}): the attack misses!", t.name),
                    );
                } else {
                    self.log(
                        Audience::All,
                        format!("{} casts Shield (AC {ac}) but is still hit.", t.name),
                    );
                }
            }
            Some(HitReaction::UncannyDodge) => {
                result.total_damage /= 2;
                self.log(
                    Audience::All,
                    format!(
                        "{} uses Uncanny Dodge: damage halved to {}.",
                        t.name, result.total_damage
                    ),
                );
            }
            Some(HitReaction::DeflectAttacks) => {
                let r = DiceExpr::dice(1, 10).roll(self.rng.as_mut(), false).total
                    + t.mods[dnd_rules::Ability::Dexterity.index()]
                    + t.level() as i32;
                result.total_damage = (result.total_damage - r).max(0);
                self.log(
                    Audience::All,
                    format!(
                        "{} uses Deflect Attacks: damage reduced by {r} to {}.",
                        t.name, result.total_damage
                    ),
                );
            }
            None => {}
        }
        let ability_mod = self
            .state
            .creatures
            .get(&pending.attacker)
            .map(|a| a.mods[0].max(a.mods[1]))
            .unwrap_or(0);
        self.finalize_attack(
            pending.attacker,
            pending.target,
            result,
            pending.melee_weapon_hit,
            ability_mod,
        );
        Ok(())
    }

    pub(crate) fn threatened_in_melee(&self, a: &Creature, lights: &[LightLevel]) -> bool {
        self.state.creatures.values().any(|e| {
            e.room == a.room
                && e.is_alive()
                && Self::are_hostile(e, a)
                && grid_distance_ft(e.pos, a.pos) <= 5
                && !e.conditions.has(Condition::Incapacitated)
                && self.can_see(e, a, lights)
        })
    }

    /// Pushes `target` up to `feet` straight away from `source` (stops at walls/creatures).
    pub(crate) fn push_away(&mut self, source: EntityId, target: EntityId, feet: i32) {
        let (sp, tp, room) = {
            let s = &self.state.creatures[&source];
            let t = &self.state.creatures[&target];
            (s.pos, t.pos, t.room)
        };
        let (dx, dy) = ((tp.0 - sp.0).signum(), (tp.1 - sp.1).signum());
        if dx == 0 && dy == 0 {
            return;
        }
        let mut pos = tp;
        for _ in 0..(feet / 5) {
            let next = (pos.0 + dx, pos.1 + dy);
            let blocked = self
                .state
                .rooms
                .get(&room)
                .is_none_or(|r| !r.map.in_bounds(next) || r.map.flags(next).blocks_movement)
                || self
                    .state
                    .creatures
                    .values()
                    .any(|c| c.room == room && c.is_alive() && c.pos == next);
            if blocked {
                break;
            }
            pos = next;
        }
        if pos != tp {
            self.creature_mut(target).pos = pos;
            self.emit(
                Audience::All,
                Event::Moved {
                    entity: target,
                    path: vec![tp, pos],
                },
            );
        }
    }

    /// Applies final damage and handles 0 HP, death and initiative cleanup.
    pub(crate) fn apply_damage(&mut self, target: EntityId, amount: i32, critical: bool) {
        self.apply_damage_from(target, amount, critical, None);
    }

    /// Applies final damage and handles 0 HP, death, Concentration, initiative cleanup and triggers.
    pub(crate) fn apply_damage_from(
        &mut self,
        target: EntityId,
        amount: i32,
        critical: bool,
        source: Option<EntityId>,
    ) {
        if amount <= 0 || !self.state.creatures.contains_key(&target) {
            return;
        }
        let applied = self.creature_mut(target).hp.take_damage(amount, critical);
        let name = self.state.creatures[&target].name.clone();
        let hostile_downed = (applied.died || applied.dropped_to_zero)
            && source.is_some_and(|s| {
                self.state
                    .creatures
                    .get(&s)
                    .is_some_and(|a| Self::are_hostile(a, &self.state.creatures[&target]))
            });
        if applied.died {
            self.log(Audience::All, format!("{name} dies."));
            self.emit(
                Audience::All,
                Event::Floating {
                    entity: target,
                    text: "Dead".into(),
                    kind: FloatKind::Condition,
                },
            );
            self.creature_mut(target)
                .conditions
                .remove(Condition::Unconscious);
            self.end_concentration(target, "death");
            self.creature_mut(target).status.raging = false;
            self.remove_from_initiative(target);
            self.fire_scripts_for(
                crate::scripts::Trigger::CreatureDefeated { entity: target },
                source,
            );
        } else if applied.dropped_to_zero {
            self.creature_mut(target)
                .conditions
                .add(Condition::Unconscious);
            self.log(
                Audience::All,
                format!("{name} drops to 0 Hit Points and falls Unconscious."),
            );
            self.emit(
                Audience::All,
                Event::Floating {
                    entity: target,
                    text: "Unconscious".into(),
                    kind: FloatKind::Condition,
                },
            );
            // Incapacitated: Concentration and Rage end.
            self.end_concentration(target, "incapacitated");
            self.creature_mut(target).status.raging = false;
        } else if applied.death_save_failures_added > 0 {
            let f = self.state.creatures[&target].hp.death_saves.failures;
            self.log(
                Audience::All,
                format!(
                    "{name} suffers {} Death Saving Throw failure(s) ({f}/3).",
                    applied.death_save_failures_added
                ),
            );
        } else if self.state.creatures[&target].concentration.is_some()
            && applied.hp_lost + applied.absorbed_by_temp > 0
        {
            // Concentration: Con save DC max(10, half the damage), up to 30.
            let dc = (amount / 2).clamp(10, 30);
            let (ok, _) = self.saving_throw(
                target,
                dnd_rules::Ability::Constitution,
                dc,
                "Concentration",
            );
            if !ok {
                self.end_concentration(target, "failed Concentration save");
            }
        }
        // Dark One's Blessing (Fiend Warlock 3): reducing an enemy to 0 HP grants temp HP = Cha mod + Warlock level.
        if hostile_downed && let Some(src) = source {
            let killer = self.state.creatures[&src].clone();
            if killer.has_feature("Dark One’s Blessing") {
                let thp = (killer.mods[dnd_rules::Ability::Charisma.index()]
                    + killer.level() as i32)
                    .max(1);
                if thp > killer.hp.temp {
                    self.creature_mut(src).hp.offer_temp_hp(thp, true);
                    self.emit(
                        Audience::All,
                        Event::Floating {
                            entity: src,
                            text: format!("+{thp} temp"),
                            kind: FloatKind::TempHp,
                        },
                    );
                }
            }
        }
    }

    pub(crate) fn heal(&mut self, target: EntityId, amount: i32) {
        // Chill Touch: "can't regain Hit Points until the end of your next turn".
        if self.state.creatures[&target]
            .status
            .no_healing_from
            .is_some()
        {
            let name = self.state.creatures[&target].name.clone();
            self.log(
                Audience::All,
                format!("{name} can't regain Hit Points right now (Chill Touch)."),
            );
            return;
        }
        let was_zero = self.state.creatures[&target].hp.current == 0;
        let gained = self.creature_mut(target).hp.heal(amount);
        if gained > 0 {
            if was_zero {
                // Unconscious ends on regaining HP; "When this condition ends, you remain Prone."
                let c = self.creature_mut(target);
                c.conditions.remove(Condition::Unconscious);
                c.conditions.add(Condition::Prone);
            }
            self.emit(
                Audience::All,
                Event::Floating {
                    entity: target,
                    text: format!("+{gained}"),
                    kind: FloatKind::Healing,
                },
            );
        }
    }

    pub(crate) fn remove_from_initiative(&mut self, id: EntityId) {
        let Some(cb) = self.state.combat.as_mut() else {
            return;
        };
        let Some(pos) = cb.order.iter().position(|e| e.entity == id) else {
            return;
        };
        let was_active = cb.turn.entity == id;
        cb.order.remove(pos);
        if cb.order.is_empty() {
            self.state.combat = None;
            self.log(Audience::All, "Combat ends: no combatants remain.");
            self.emit(Audience::All, Event::CombatEnded);
            return;
        }
        if pos < cb.index {
            cb.index -= 1;
        }
        if was_active {
            // The next creature in order takes its turn (index already points at it).
            if cb.index >= cb.order.len() {
                cb.index = 0;
                cb.round += 1;
            }
            let next = cb.order[cb.index].entity;
            self.start_turn(next);
        }
    }

    pub(crate) fn cmd_reaction(&mut self, who: Principal, accept: bool, attack: usize) -> Res<()> {
        let pending = self
            .state
            .combat
            .as_ref()
            .and_then(|c| c.pending.clone())
            .ok_or_else(|| CommandError::Invalid("no reaction is pending".into()))?;
        let reactor = self.creature(pending.reactor)?;
        if !Self::controls(who, reactor) {
            return Err(CommandError::NotAuthorized);
        }
        if accept {
            let profile = reactor
                .attacks
                .get(attack)
                .ok_or_else(|| CommandError::NotFound("attack".into()))?;
            if profile.kind != WeaponKind::Melee {
                return rules("an Opportunity Attack is a melee attack");
            }
            // The mover is still within reach (it hasn't left yet).
            self.validate_attack(pending.reactor, pending.mover, attack)?;
        }
        self.state.combat.as_mut().expect("checked").pending = None;
        if accept {
            self.state
                .combat
                .as_mut()
                .expect("checked")
                .reactions_used
                .insert(pending.reactor);
            let name = self.state.creatures[&pending.reactor].name.clone();
            self.log(
                Audience::All,
                format!("{name} makes an Opportunity Attack!"),
            );
            self.perform_attack(pending.reactor, pending.mover, attack);
        } else {
            let name = self.state.creatures[&pending.reactor].name.clone();
            self.log(Audience::All, format!("{name} lets them go."));
        }
        // Continue the interrupted movement if the mover still can.
        let can_continue = self.state.combat.is_some()
            && self
                .state
                .creatures
                .get(&pending.mover)
                .is_some_and(|m| m.is_alive() && m.hp.current > 0 && m.effective_speed() > 0);
        if can_continue {
            self.walk(pending.mover, pending.remaining_path);
        }
        Ok(())
    }

    pub(crate) fn cmd_end_turn(&mut self, who: Principal) -> Res<()> {
        let cb = self
            .state
            .combat
            .as_ref()
            .ok_or_else(|| CommandError::Rules("not in combat".into()))?;
        if cb.pending.is_some() {
            return rules("waiting for a reaction decision");
        }
        let active = self.creature(cb.active())?;
        if !Self::controls(who, active) {
            return Err(CommandError::NotAuthorized);
        }
        self.advance_turn();
        Ok(())
    }

    pub(crate) fn advance_turn(&mut self) {
        if let Some(cb) = &self.state.combat {
            let ending = cb.turn.entity;
            self.end_of_turn(ending);
        }
        let Some(cb) = self.state.combat.as_mut() else {
            return;
        };
        cb.pending = None;
        cb.pending_hit = None;
        cb.index += 1;
        if cb.index >= cb.order.len() {
            cb.index = 0;
            cb.round += 1;
        }
        let next = cb.order[cb.index].entity;
        self.start_turn(next);
    }

    /// End-of-turn bookkeeping for `entity`.
    pub(crate) fn end_of_turn(&mut self, entity: EntityId) {
        let round = self.state.combat.as_ref().map(|c| c.round).unwrap_or(0);
        if let Some(c) = self.state.creatures.get_mut(&entity) {
            // Vex windows tick down.
            for v in &mut c.effects.vex {
                v.1 = v.1.saturating_sub(1);
            }
            c.effects.vex.retain(|v| v.1 > 0);
        }
        // Rage ends at the end of the turn unless it was extended (attack, forced save, or Bonus Action),
        // and after 10 minutes.
        if let Some(c) = self.state.creatures.get_mut(&entity)
            && c.status.raging
        {
            c.status.rage_rounds += 1;
            if !c.status.rage_extended || c.status.rage_rounds >= 100 {
                c.status.raging = false;
                let name = c.name.clone();
                self.log(Audience::All, format!("{name}'s Rage ends."));
            }
        }
        // "Until the end of your next turn" windows (Guiding Bolt, Chill Touch) set in an earlier round expire.
        for c in self.state.creatures.values_mut() {
            if c.status
                .exposed
                .is_some_and(|(src, r)| src == entity && r < round)
            {
                c.status.exposed = None;
            }
            if c.status
                .no_healing_from
                .is_some_and(|(src, r)| src == entity && r < round)
            {
                c.status.no_healing_from = None;
            }
        }
        // Repeated saves at the end of the affected creature's turn.
        let repeats: Vec<(u64, dnd_rules::Ability, i32, String)> = self
            .state
            .effects
            .iter()
            .filter_map(|e| match &e.kind {
                EffectKind::Condition {
                    target,
                    save: Some((ab, dc)),
                    repeat_save: true,
                    ..
                } if *target == entity => Some((e.id, *ab, *dc, e.label())),
                _ => None,
            })
            .collect();
        for (id, ability, dc, label) in repeats {
            if self
                .saving_throw(entity, ability, dc, &format!("repeat save vs {label}"))
                .0
            {
                self.remove_effect(id);
            }
        }
    }

    pub(crate) fn start_turn(&mut self, entity: EntityId) {
        // Effects created by this creature tick down; "until the start of your next turn" effects end.
        self.tick_effects(entity);
        let Some(c) = self.state.creatures.get(&entity) else {
            return;
        };
        let name = c.name.clone();
        let hidden = c.hidden;
        for other in self.state.creatures.values_mut() {
            // Sap and source-bound speed penalties last until the start of the source's next turn.
            if other.effects.sapped_by == Some(entity) {
                other.effects.sapped_by = None;
            }
            if other
                .status
                .speed_penalty
                .is_some_and(|(src, _)| src == entity)
            {
                other.status.speed_penalty = None;
            }
        }
        {
            let c = self.creature_mut(entity);
            // Dodge, Reckless Attack and Shocking Grasp last until the start of your next turn.
            c.effects.dodging = false;
            c.status.reckless = false;
            c.status.no_reactions = false;
            c.status.rage_extended = false;
            c.status.stunning_strike_ready = false;
        }
        let speed = self.state.creatures[&entity].effective_speed();
        let Some(cb) = self.state.combat.as_mut() else {
            return;
        };
        cb.reactions_used.remove(&entity);
        cb.turn = TurnState {
            entity,
            movement_left_ft: speed,
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
        };
        let round = cb.round;
        // Monster abilities with "Recharge X–6": roll a d6 for each expended one.
        let recharging = self.state.creatures[&entity].resources.recharging.clone();
        for idx in recharging {
            let need = self.state.creatures[&entity]
                .save_actions
                .get(idx)
                .and_then(|a| a.recharge)
                .unwrap_or(6) as i32;
            let r = DiceExpr::dice(1, 6).roll(self.rng.as_mut(), false).total;
            if r >= need {
                let c = self.creature_mut(entity);
                c.resources.recharging.retain(|i| *i != idx);
                let action = c.save_actions[idx].name.clone();
                self.log(
                    Audience::DmOnly,
                    format!("{name}: {action} recharges (d6 = {r})."),
                );
            }
        }
        let audience = if hidden {
            Audience::DmOnly
        } else {
            Audience::All
        };
        self.log(audience, format!("Round {round}: {name}'s turn."));
        self.emit(audience, Event::TurnStarted { entity, round });

        // Death Saving Throw at the start of the turn for a dying character.
        if self.state.creatures[&entity].hp.life_state() == LifeState::Dying {
            let exhaustion = self.state.creatures[&entity].conditions.exhaustion;
            let roll = D20Roll::roll(self.rng.as_mut(), RollMode::Normal, 0, exhaustion);
            let res = self
                .creature_mut(entity)
                .hp
                .death_saving_throw_total(roll.natural, roll.total);
            self.log(
                Audience::All,
                format!(
                    "{name} makes a Death Saving Throw: {} → {res:?}",
                    roll.breakdown()
                ),
            );
            match res {
                DeathSaveResult::RegainedHp => {
                    let c = self.creature_mut(entity);
                    c.conditions.remove(Condition::Unconscious);
                    c.conditions.add(Condition::Prone);
                    self.emit(
                        Audience::All,
                        Event::Floating {
                            entity,
                            text: "+1".into(),
                            kind: FloatKind::Healing,
                        },
                    );
                }
                DeathSaveResult::Died => {
                    self.log(Audience::All, format!("{name} dies."));
                    self.remove_from_initiative(entity);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn cmd_interact(
        &mut self,
        who: Principal,
        entity: EntityId,
        object: ObjectId,
    ) -> Res<()> {
        let c = self.creature(entity)?;
        if !Self::controls(who, c) {
            return Err(CommandError::NotAuthorized);
        }
        if !c.conditions.can_act() {
            return rules("Incapacitated");
        }
        let room = c.room;
        let pos = c.pos;
        let name = c.name.clone();
        if let Some(cb) = self.combat_in(room)
            && cb.active() != entity
        {
            return rules("it isn't this creature's turn");
        }
        let r = self.room(room)?;
        let o = r
            .map
            .object(object)
            .filter(|o| !o.hidden)
            .ok_or_else(|| CommandError::NotFound("object".into()))?;
        if !o.interactable {
            return rules("nothing to interact with");
        }
        if !o.footprint().any(|f| grid_distance_ft(f, pos) <= 5) {
            return rules("too far away: move next to it");
        }
        let closing = o.open;
        if closing
            && o.asset_id == "door"
            && self
                .state
                .creatures
                .values()
                .any(|cr| cr.room == room && cr.is_alive() && o.covers(cr.pos))
        {
            return rules("something is in the doorway");
        }
        let label = self
            .catalog
            .get(&o.asset_id)
            .map(|a| a.name.clone())
            .unwrap_or_default();
        let rs = self.state.rooms.get_mut(&room).expect("checked");
        let o = rs.map.object_mut(object).expect("checked");
        o.open = !o.open;
        rs.map_rev += 1;
        let verb = if closing { "closes" } else { "opens" };
        self.log(
            Audience::All,
            format!("{name} {verb} the {}.", label.to_lowercase()),
        );
        if !closing {
            let contents = self.state.rooms[&room]
                .containers
                .get(&object)
                .cloned()
                .unwrap_or_default();
            if !contents.is_empty() {
                let list: Vec<String> = contents
                    .iter()
                    .map(|i| format!("{}× {}", i.quantity, i.name))
                    .collect();
                self.log(Audience::All, format!("Inside: {}", list.join(", ")));
            }
        }
        self.fire_scripts_for(
            crate::scripts::Trigger::Interact { room, object },
            Some(entity),
        );
        Ok(())
    }

    pub(crate) fn cmd_add_drawing(
        &mut self,
        who: Principal,
        room: RoomId,
        mut drawing: Drawing,
    ) -> Res<()> {
        if who != Principal::Dm && !self.state.settings.players_can_draw {
            return Err(CommandError::NotAuthorized);
        }
        self.room(room)?;
        if who != Principal::Dm {
            // Player drawings are always temporary and visible to everyone, in the party's room.
            if room != self.state.party_room {
                return Err(CommandError::NotAuthorized);
            }
            drawing.temporary = true;
            drawing.visibility = DrawVisibility::All;
        }
        let too_big = match &drawing.shape {
            DrawShape::Stroke(p) => p.len() > 2000,
            DrawShape::Text(_, t) => t.chars().count() > 120,
            _ => false,
        };
        if too_big || !(0.5..=20.0).contains(&drawing.width) {
            return invalid("drawing too large");
        }
        drawing.id = self.next_id();
        drawing.author = self.display_name(who);
        let rs = self.state.rooms.get_mut(&room).expect("checked");
        rs.drawings.push(drawing);
        if rs.drawings.len() > 500 {
            rs.drawings.remove(0);
        }
        Ok(())
    }

    // ------------------------------------------------------------------ DM

    pub(crate) fn asset(&self, id: &Option<String>) -> Res<Option<dnd_world::catalog::AssetDef>> {
        match id {
            None => Ok(None),
            Some(id) => self
                .catalog
                .get(id)
                .cloned()
                .map(Some)
                .ok_or_else(|| CommandError::NotFound(format!("asset {id}"))),
        }
    }

    pub(crate) fn dm(&mut self, d: DmCommand) -> Res<()> {
        use DmCommand::*;
        match d {
            StartSession => {
                self.state.session_started = true;
                self.state
                    .rooms
                    .get_mut(&self.state.party_room)
                    .expect("party room exists")
                    .visited = true;
                self.log(Audience::All, "The adventure begins!");
            }
            ViewRoom { room } => {
                self.room(room)?;
                self.state.dm_view_room = room;
            }
            CreateRoom {
                name,
                width,
                height,
                ambient,
            } => {
                let map = GridMap::new(width, height, ambient)
                    .map_err(|e| CommandError::Invalid(e.to_string()))?;
                let name: String = name.trim().chars().take(60).collect();
                let id = self.state.graph.add_room(if name.is_empty() {
                    "New Room".to_string()
                } else {
                    name.clone()
                });
                self.state.rooms.insert(id, RoomState::new(map));
                self.state.dm_view_room = id;
                self.log(Audience::DmOnly, format!("Room created: {name}"));
            }
            RenameRoom {
                room,
                name,
                description,
            } => {
                let n = self
                    .state
                    .graph
                    .nodes
                    .get_mut(&room)
                    .ok_or_else(|| CommandError::NotFound("room".into()))?;
                n.name = name.chars().take(60).collect();
                n.description = description.chars().take(4000).collect();
            }
            ConnectRooms { from, to, one_way } => {
                self.state
                    .graph
                    .connect(from, to, one_way)
                    .ok_or_else(|| CommandError::Invalid("can't connect those rooms".into()))?;
            }
            SetEdge {
                edge,
                locked,
                secret,
                arrival,
            } => {
                let e = self
                    .state
                    .graph
                    .edge_mut(edge)
                    .ok_or_else(|| CommandError::NotFound("edge".into()))?;
                e.locked = locked;
                e.secret = secret;
                e.arrival = arrival;
            }
            DeleteEdge { edge } => {
                if !self.state.graph.remove_edge(edge) {
                    return Err(CommandError::NotFound("edge".into()));
                }
            }
            PaintTiles {
                room,
                layer,
                cells,
                asset,
            } => {
                if cells.len() > 65_536 {
                    return invalid("too many cells");
                }
                let a = self.asset(&asset)?;
                let r = self.room(room)?;
                let mut map = r.map.clone();
                for c in cells {
                    if map.in_bounds(c) {
                        map.set_tile(layer, c, a.as_ref())
                            .map_err(|e| CommandError::Invalid(e.to_string()))?;
                    }
                }
                self.state.rooms.get_mut(&room).expect("checked").map = map;
            }
            FillRect {
                room,
                layer,
                a,
                b,
                asset,
            } => {
                let def = self.asset(&asset)?;
                let mut map = self.room(room)?.map.clone();
                map.fill_rect(layer, a, b, def.as_ref())
                    .map_err(|e| CommandError::Invalid(e.to_string()))?;
                self.state.rooms.get_mut(&room).expect("checked").map = map;
            }
            FloodFill {
                room,
                layer,
                start,
                asset,
            } => {
                let def = self.asset(&asset)?;
                let mut map = self.room(room)?.map.clone();
                map.flood_fill(layer, start, def.as_ref())
                    .map_err(|e| CommandError::Invalid(e.to_string()))?;
                self.state.rooms.get_mut(&room).expect("checked").map = map;
            }
            SetLayer {
                room,
                layer,
                visible,
                locked,
            } => {
                self.room(room)?;
                let l = self
                    .state
                    .rooms
                    .get_mut(&room)
                    .expect("checked")
                    .map
                    .layer_mut(layer);
                l.visible = visible;
                l.locked = locked;
            }
            PlaceObject {
                room,
                asset,
                cell,
                hidden,
            } => {
                let def = self
                    .catalog
                    .get(&asset)
                    .cloned()
                    .ok_or_else(|| CommandError::NotFound(format!("asset {asset}")))?;
                if !def.is_object {
                    return invalid("that asset is a tile; paint it instead");
                }
                let map = &self.room(room)?.map;
                let mut obj = MapObject::from_asset(ObjectId(0), &def, cell.0, cell.1);
                if !obj.footprint().all(|c| map.in_bounds(c)) {
                    return invalid("object doesn't fit on the map there");
                }
                obj.id = ObjectId(self.next_id());
                obj.hidden = hidden;
                self.state
                    .rooms
                    .get_mut(&room)
                    .expect("checked")
                    .map
                    .objects
                    .push(obj);
            }
            RemoveObject { room, object } => {
                let map = &mut self
                    .state
                    .rooms
                    .get_mut(&room)
                    .ok_or_else(|| CommandError::NotFound("room".into()))?
                    .map;
                let n = map.objects.len();
                map.objects.retain(|o| o.id != object);
                if n == map.objects.len() {
                    return Err(CommandError::NotFound("object".into()));
                }
            }
            SetObject {
                room,
                object,
                hidden,
                open,
            } => {
                let rs = self
                    .state
                    .rooms
                    .get_mut(&room)
                    .ok_or_else(|| CommandError::NotFound("room".into()))?;
                let o = rs
                    .map
                    .object_mut(object)
                    .ok_or_else(|| CommandError::NotFound("object".into()))?;
                let revealed = o.hidden && !hidden;
                o.hidden = hidden;
                o.open = open;
                if revealed {
                    self.log(Audience::All, "Something hidden is revealed!");
                }
            }
            SetAmbient { room, ambient } => {
                self.room(room)?;
                self.state
                    .rooms
                    .get_mut(&room)
                    .expect("checked")
                    .map
                    .ambient = ambient;
            }
            SetEntryPoint { room, cell } => {
                let r = self.room(room)?;
                if !r.map.in_bounds(cell) {
                    return invalid("out of bounds");
                }
                self.state.rooms.get_mut(&room).expect("checked").entry = cell;
            }
            AddLight {
                room,
                cell,
                bright_ft,
                dim_ft,
            } => {
                let r = self.room(room)?;
                if !r.map.in_bounds(cell) || bright_ft > 300 || dim_ft > 300 {
                    return invalid("bad light");
                }
                self.state
                    .rooms
                    .get_mut(&room)
                    .expect("checked")
                    .extra_lights
                    .push(PointLight {
                        cell,
                        bright_ft,
                        dim_ft,
                    });
            }
            ClearLights { room } => {
                self.room(room)?;
                self.state
                    .rooms
                    .get_mut(&room)
                    .expect("checked")
                    .extra_lights
                    .clear();
            }
            Undo { room } | Redo { room } => {
                let is_undo = matches!(d, Undo { .. });
                let current = {
                    let r = self.room(room)?;
                    (r.map.clone(), r.extra_lights.clone())
                };
                let (undo, redo) = self.undo.entry(room).or_default();
                let (from, to) = if is_undo { (undo, redo) } else { (redo, undo) };
                let snap = from
                    .pop()
                    .ok_or_else(|| CommandError::Invalid("nothing to undo/redo".into()))?;
                to.push(current);
                let rs = self.state.rooms.get_mut(&room).expect("checked");
                rs.map = snap.0;
                rs.extra_lights = snap.1;
                rs.map_rev += 1;
            }
            SpawnMonster {
                template,
                room,
                cell,
                hidden,
            } => {
                let t = self
                    .bestiary()
                    .into_iter()
                    .find(|t| t.id == template)
                    .ok_or_else(|| CommandError::NotFound(format!("monster {template}")))?;
                let r = self.room(room)?;
                if !r.map.in_bounds(cell) || r.map.flags(cell).blocks_movement {
                    return rules("that space is blocked");
                }
                if self
                    .state
                    .creatures
                    .values()
                    .any(|c| c.room == room && c.is_alive() && c.pos == cell)
                {
                    return rules("that space is occupied");
                }
                let n = self.state.creatures.values().filter(|c| matches!(&c.kind, CreatureKind::Monster{template_id} if *template_id == t.id)).count();
                let id = EntityId(self.next_id());
                let mut c =
                    Creature::from_template(id, &t, format!("{} {}", t.name, n + 1), room, cell);
                c.hidden = hidden;
                let name = c.name.clone();
                self.state.creatures.insert(id, c);
                let audience = if hidden || room != self.state.party_room {
                    Audience::DmOnly
                } else {
                    Audience::All
                };
                self.log(audience, format!("{name} appears!"));
                // Late joiners roll initiative into a running combat in that room.
                if self.combat_in(room).is_some() {
                    self.roll_into_initiative(id, false);
                }
            }
            RemoveCreature { entity } => {
                self.creature(entity)?;
                self.remove_from_initiative(entity);
                self.state.creatures.remove(&entity);
                for p in self.state.players.values_mut() {
                    if p.character == Some(entity) {
                        p.character = None;
                    }
                }
            }
            SetCreatureHidden { entity, hidden } => {
                self.creature(entity)?;
                self.creature_mut(entity).hidden = hidden;
            }
            Teleport { entity, cell } => {
                let c = self.creature(entity)?;
                let r = self.room(c.room)?;
                if !r.map.in_bounds(cell) {
                    return invalid("out of bounds");
                }
                let from = c.pos;
                self.creature_mut(entity).pos = cell;
                self.emit(
                    Audience::All,
                    Event::Moved {
                        entity,
                        path: vec![from, cell],
                    },
                );
            }
            AdjustHp { entity, delta } => {
                let name = self.creature(entity)?.name.clone();
                if delta < 0 {
                    self.emit(
                        Audience::All,
                        Event::Floating {
                            entity,
                            text: format!("{delta}"),
                            kind: FloatKind::Damage,
                        },
                    );
                    self.apply_damage(entity, -delta, false);
                } else {
                    self.heal(entity, delta);
                }
                self.log(
                    Audience::DmOnly,
                    format!("DM adjusted {name}'s HP by {delta:+} (override)."),
                );
            }
            SetCondition {
                entity,
                condition,
                present,
            } => {
                let name = self.creature(entity)?.name.clone();
                let c = self.creature_mut(entity);
                if present {
                    c.conditions.add(condition);
                } else {
                    c.conditions.remove(condition);
                }
                let text = format!("{}{condition:?}", if present { "+" } else { "-" });
                self.emit(
                    Audience::All,
                    Event::Floating {
                        entity,
                        text: text.clone(),
                        kind: FloatKind::Condition,
                    },
                );
                self.log(Audience::All, format!("{name}: {text}"));
            }
            StartCombat { surprised } => self.start_combat(surprised)?,
            EndCombat => {
                if self.state.combat.take().is_none() {
                    return rules("not in combat");
                }
                for c in self.state.creatures.values_mut() {
                    c.effects = ActiveEffects::default();
                }
                self.log(Audience::All, "Combat ends.");
                self.emit(Audience::All, Event::CombatEnded);
            }
            NextTurn => {
                if self.state.combat.is_none() {
                    return rules("not in combat");
                }
                self.advance_turn();
            }
            ReorderInitiative { order } => {
                let cb = self
                    .state
                    .combat
                    .as_ref()
                    .ok_or_else(|| CommandError::Rules("not in combat".into()))?;
                let mut a: Vec<EntityId> = cb.order.iter().map(|e| e.entity).collect();
                let mut b = order.clone();
                a.sort();
                b.sort();
                if a != b {
                    return invalid("order must contain exactly the current combatants");
                }
                let cb = self.state.combat.as_mut().expect("checked");
                let active = cb.turn.entity;
                let mut new_order = Vec::new();
                for id in order {
                    new_order.push(
                        cb.order
                            .iter()
                            .find(|e| e.entity == id)
                            .expect("checked")
                            .clone(),
                    );
                }
                cb.order = new_order;
                cb.index = cb
                    .order
                    .iter()
                    .position(|e| e.entity == active)
                    .unwrap_or(0);
                self.log(Audience::All, "The DM reorders the initiative.");
            }
            TransitionParty { to, override_rules } => self.transition(to, override_rules)?,
            RevealRoom { room, explored } => {
                let r = self
                    .state
                    .rooms
                    .get_mut(&room)
                    .ok_or_else(|| CommandError::NotFound("room".into()))?;
                r.explored.iter_mut().for_each(|e| *e = explored);
            }
            SetExplorationMovement { allowed } => {
                self.state.settings.exploration_movement = allowed;
            }
            UpdateObject { room, object } => {
                let mut obj = *object;
                let rs = self
                    .state
                    .rooms
                    .get(&room)
                    .ok_or_else(|| CommandError::NotFound("room".into()))?;
                if rs.map.object(obj.id).is_none() {
                    return Err(CommandError::NotFound("object".into()));
                }
                if !obj.footprint().all(|c| rs.map.in_bounds(c))
                    || obj.w == 0
                    || obj.h == 0
                    || obj.w > 16
                    || obj.h > 16
                {
                    return invalid("object must fit on the map (1–16 cells per side)");
                }
                obj.label = obj.label.chars().take(60).collect();
                obj.dm_notes = obj.dm_notes.chars().take(2000).collect();
                if let Some(l) = &mut obj.light {
                    l.bright_ft = l.bright_ft.min(300);
                    l.dim_ft = l.dim_ft.min(300);
                }
                let rs = self.state.rooms.get_mut(&room).expect("checked");
                let id = obj.id;
                *rs.map.object_mut(id).expect("checked") = obj;
            }
            AddObjects { room, objects } => {
                if objects.len() > 500 {
                    return invalid("too many objects at once");
                }
                let map = &self.room(room)?.map;
                for o in &objects {
                    if self.catalog.get(&o.asset_id).is_none()
                        || !o.footprint().all(|c| map.in_bounds(c))
                    {
                        return invalid("pasted objects must fit on the map");
                    }
                }
                for mut o in objects {
                    o.id = dnd_world::ObjectId(self.next_id());
                    self.state
                        .rooms
                        .get_mut(&room)
                        .expect("checked")
                        .map
                        .objects
                        .push(o);
                }
            }
            RemoveObjects { room, objects } => {
                let rs = self
                    .state
                    .rooms
                    .get_mut(&room)
                    .ok_or_else(|| CommandError::NotFound("room".into()))?;
                rs.map.objects.retain(|o| !objects.contains(&o.id));
                for id in &objects {
                    rs.containers.remove(id);
                }
            }
            MoveObjects {
                room,
                objects,
                dx,
                dy,
            } => {
                let map = &self.room(room)?.map;
                for id in &objects {
                    let o = map
                        .object(*id)
                        .ok_or_else(|| CommandError::NotFound("object".into()))?;
                    let mut moved = o.clone();
                    moved.x += dx;
                    moved.y += dy;
                    if !moved.footprint().all(|c| map.in_bounds(c)) {
                        return invalid("objects would leave the map");
                    }
                }
                let rs = self.state.rooms.get_mut(&room).expect("checked");
                for id in objects {
                    let o = rs.map.object_mut(id).expect("checked");
                    o.x += dx;
                    o.y += dy;
                }
            }
            MoveLayer { room, layer, up } => {
                let rs = self
                    .state
                    .rooms
                    .get_mut(&room)
                    .ok_or_else(|| CommandError::NotFound("room".into()))?;
                let i = rs
                    .map
                    .layers
                    .iter()
                    .position(|l| l.kind == layer)
                    .expect("all layers exist");
                let j = if up { i + 1 } else { i.wrapping_sub(1) };
                if j < rs.map.layers.len() {
                    rs.map.layers.swap(i, j);
                }
            }
            SetRoomPos { room, pos } => {
                let n = self
                    .state
                    .graph
                    .nodes
                    .get_mut(&room)
                    .ok_or_else(|| CommandError::NotFound("room".into()))?;
                n.editor_pos = (
                    pos.0.clamp(-10_000.0, 10_000.0),
                    pos.1.clamp(-10_000.0, 10_000.0),
                );
            }
            DeleteRoom { room } => {
                if room == self.state.party_room {
                    return rules("the party is in that room");
                }
                if self.state.rooms.len() <= 1 {
                    return rules("a campaign needs at least one room");
                }
                self.state.graph.remove_room(room);
                self.state.rooms.remove(&room);
                self.state
                    .creatures
                    .retain(|_, c| c.room != room || c.faction == Faction::Party);
                if self.state.dm_view_room == room {
                    self.state.dm_view_room = self.state.party_room;
                }
            }
            SetRoomAudio {
                room,
                music,
                ambience,
            } => {
                let rs = self
                    .state
                    .rooms
                    .get_mut(&room)
                    .ok_or_else(|| CommandError::NotFound("room".into()))?;
                rs.music = music.clone();
                rs.ambience = ambience.clone();
                if room == self.state.party_room {
                    self.emit(
                        Audience::All,
                        Event::Sound {
                            id: format!("music:{}", music.unwrap_or_default()),
                        },
                    );
                    self.emit(
                        Audience::All,
                        Event::Sound {
                            id: format!("ambience:{}", ambience.unwrap_or_default()),
                        },
                    );
                }
            }
            SaveCustomMonster { template } => {
                let mut t = *template;
                t.validate_custom().map_err(CommandError::Invalid)?;
                t.source = "homebrew".into();
                if !t.id.starts_with("custom_") {
                    t.id = format!("custom_{}", self.next_id());
                }
                t.hit_dice = dnd_rules::DiceExpr::default().with_modifier(t.hit_points);
                match self.state.custom_monsters.iter_mut().find(|m| m.id == t.id) {
                    Some(m) => *m = t,
                    None => self.state.custom_monsters.push(t),
                }
            }
            DeleteCustomMonster { id } => self.state.custom_monsters.retain(|m| m.id != id),
            AwardXp { entities, xp } => self.award_xp(entities, xp)?,
            GrantLevel { entity } => {
                let c = self.creature_mut(entity);
                c.status.milestone_levels += 1;
                let n = c.name.clone();
                self.log(
                    Audience::All,
                    format!("{n} may advance a level (milestone)."),
                );
            }
            Rest { long } => self.rest(long)?,
            EndEffect { effect } => {
                if !self.state.effects.iter().any(|e| e.id == effect) {
                    return Err(CommandError::NotFound("effect".into()));
                }
                self.remove_effect(effect);
            }
            SetContainer {
                room,
                object,
                items,
            } => {
                let rs = self
                    .state
                    .rooms
                    .get_mut(&room)
                    .ok_or_else(|| CommandError::NotFound("room".into()))?;
                if rs.map.object(object).is_none() || items.len() > 100 {
                    return invalid("unknown container or too many items");
                }
                rs.containers.insert(object, items);
            }
            GiveItemTo {
                entity,
                item,
                quantity,
            } => {
                let mut it = dnd_rules::items::catalog_item(&item)
                    .ok_or_else(|| CommandError::NotFound(item.clone()))?;
                it.quantity = quantity.clamp(1, 999);
                if self.creature(entity)?.sheet().is_none() {
                    return rules("only characters carry items");
                }
                self.give_item_to(entity, it);
            }
            SetCurrency { entity, currency } => {
                self.creature(entity)?;
                if let Some(s) = self.creature_mut(entity).sheet_mut() {
                    s.currency = currency;
                }
            }
            SaveScript { script } => {
                let mut sc = *script;
                sc.name = sc.name.chars().take(80).collect();
                if sc.actions.len() > 50 {
                    return invalid("too many actions");
                }
                if sc.id == 0 {
                    sc.id = self.next_id();
                }
                match self.state.scripts.iter_mut().find(|x| x.id == sc.id) {
                    Some(x) => *x = sc,
                    None => self.state.scripts.push(sc),
                }
            }
            DeleteScript { id } => self.state.scripts.retain(|x| x.id != id),
            FireScript { id, actor } => {
                if !self.state.scripts.iter().any(|x| x.id == id) {
                    return Err(CommandError::NotFound("event".into()));
                }
                self.run_script(id, actor);
            }
            ClearDrawings {
                room,
                temporary_only,
            } => {
                let rs = self
                    .state
                    .rooms
                    .get_mut(&room)
                    .ok_or_else(|| CommandError::NotFound("room".into()))?;
                rs.drawings.retain(|d| temporary_only && !d.temporary);
            }
            DeleteDrawing { room, id } => {
                let rs = self
                    .state
                    .rooms
                    .get_mut(&room)
                    .ok_or_else(|| CommandError::NotFound("room".into()))?;
                rs.drawings.retain(|d| d.id != id);
            }
            SetPlayersCanDraw { allowed } => self.state.settings.players_can_draw = allowed,
        }
        Ok(())
    }

    pub(crate) fn initiative_roll(&mut self, id: EntityId, surprised: bool) -> InitiativeEntry {
        let c = &self.state.creatures[&id];
        let mut s = ModeSources::default();
        if surprised {
            s.dis("Surprised");
        }
        if c.conditions.has(Condition::Invisible) {
            s.adv("Invisible");
        }
        if c.conditions.has(Condition::Incapacitated) {
            s.dis("Incapacitated");
        }
        let (bonus, exh) = (c.initiative_bonus, c.conditions.exhaustion);
        let roll = D20Roll::roll(self.rng.as_mut(), s.mode(), bonus, exh);
        InitiativeEntry {
            entity: id,
            total: roll.total,
            roll,
        }
    }

    pub(crate) fn sort_initiative(&self, order: &mut [InitiativeEntry]) {
        // Highest first. Ties are the GM's/players' call; default tiebreak is Dexterity, then entity id,
        // and the DM can ReorderInitiative.
        order.sort_by(|a, b| {
            b.total
                .cmp(&a.total)
                .then(
                    self.state.creatures[&b.entity]
                        .dex_score
                        .cmp(&self.state.creatures[&a.entity].dex_score),
                )
                .then(a.entity.cmp(&b.entity))
        });
    }

    pub(crate) fn roll_into_initiative(&mut self, id: EntityId, surprised: bool) {
        let e = self.initiative_roll(id, surprised);
        let name = self.state.creatures[&id].name.clone();
        let hidden = self.state.creatures[&id].hidden;
        let mut order = self.state.combat.as_ref().expect("in combat").order.clone();
        let active = self.state.combat.as_ref().expect("in combat").turn.entity;
        order.push(e.clone());
        self.sort_initiative(&mut order);
        let cb = self.state.combat.as_mut().expect("in combat");
        cb.order = order;
        cb.index = cb
            .order
            .iter()
            .position(|x| x.entity == active)
            .unwrap_or(0);
        self.log(
            if hidden {
                Audience::DmOnly
            } else {
                Audience::All
            },
            format!("{name} rolls Initiative: {}", e.roll.breakdown()),
        );
    }

    pub(crate) fn start_combat(&mut self, surprised: Vec<EntityId>) -> Res<()> {
        if self.state.combat.is_some() {
            return rules("combat is already running");
        }
        let room = self.state.party_room;
        let ids: Vec<EntityId> = self
            .state
            .creatures
            .values()
            .filter(|c| c.room == room && c.is_alive())
            .map(|c| c.id)
            .collect();
        if ids.is_empty() {
            return rules("no creatures in the room");
        }
        let mut order = Vec::new();
        for id in &ids {
            let e = self.initiative_roll(*id, surprised.contains(id));
            let c = &self.state.creatures[id];
            let audience = if c.hidden {
                Audience::DmOnly
            } else {
                Audience::All
            };
            let text = format!(
                "{} rolls Initiative: {}{}",
                c.name,
                e.roll.breakdown(),
                if surprised.contains(id) {
                    " (Surprised)"
                } else {
                    ""
                }
            );
            self.log(audience, text);
            order.push(e);
        }
        self.sort_initiative(&mut order);
        for w in order.windows(2) {
            if w[0].total == w[1].total {
                let (a, b) = (
                    &self.state.creatures[&w[0].entity].name,
                    &self.state.creatures[&w[1].entity].name,
                );
                let t = format!("Initiative tie between {a} and {b} — the DM may reorder.");
                self.log(Audience::DmOnly, t);
            }
        }
        let first = order[0].entity;
        self.state.combat = Some(Combat {
            room,
            round: 1,
            order,
            index: 0,
            turn: TurnState::fresh(first, 0),
            reactions_used: Default::default(),
            pending: None,
            pending_hit: None,
            paused: false,
        });
        self.log(Audience::All, "Roll for initiative! Combat begins.");
        self.emit(Audience::All, Event::CombatStarted);
        self.start_turn(first);
        Ok(())
    }

    pub(crate) fn transition(&mut self, to: RoomId, override_rules: bool) -> Res<()> {
        if self.state.combat.is_some() && !override_rules {
            return rules("end combat before travelling (or override)");
        }
        let from = self.state.party_room;
        let edge = self
            .state
            .graph
            .validate_travel(from, to, override_rules)
            .map_err(|e| CommandError::Rules(e.to_string()))?;
        let arrival = edge
            .and_then(|e| e.arrival_from(from))
            .unwrap_or(self.room(to)?.entry);
        let party: Vec<EntityId> = self
            .state
            .creatures
            .values()
            .filter(|c| c.room == from && c.faction == Faction::Party && c.is_alive())
            .map(|c| c.id)
            .collect();
        // Compute all positions before mutating (so a failure leaves state untouched).
        let mut placed: Vec<(EntityId, Cell)> = Vec::new();
        for id in &party {
            let cell = {
                let r = self.room(to)?;
                let taken = |c: Cell| {
                    placed.iter().any(|(_, p)| *p == c)
                        || self
                            .state
                            .creatures
                            .values()
                            .any(|cr| cr.room == to && cr.is_alive() && cr.pos == c)
                };
                let mut found = None;
                'outer: for radius in 0..r.map.width.max(r.map.height) {
                    for dy in -radius..=radius {
                        for dx in -radius..=radius {
                            let c = (arrival.0 + dx, arrival.1 + dy);
                            if dx.abs().max(dy.abs()) == radius
                                && r.map.in_bounds(c)
                                && !r.map.flags(c).blocks_movement
                                && !taken(c)
                            {
                                found = Some(c);
                                break 'outer;
                            }
                        }
                    }
                }
                found.ok_or_else(|| {
                    CommandError::Rules("no room for the party at the arrival point".into())
                })?
            };
            placed.push((*id, cell));
        }
        if override_rules && self.state.combat.is_some() {
            self.state.combat = None;
            self.emit(Audience::All, Event::CombatEnded);
        }
        for (id, cell) in placed {
            let c = self.creature_mut(id);
            c.room = to;
            c.pos = cell;
            c.effects = ActiveEffects::default();
        }
        self.state.party_room = to;
        self.state.dm_view_room = to;
        self.state.rooms.get_mut(&to).expect("checked").visited = true;
        let name = self.state.graph.nodes[&to].name.clone();
        self.log(Audience::All, format!("The party travels to {name}."));
        self.emit(Audience::All, Event::RoomChanged { room: to, name });
        if let Some(m) = self.state.rooms[&to].music.clone() {
            self.emit(
                Audience::All,
                Event::Sound {
                    id: format!("music:{m}"),
                },
            );
        }
        self.fire_scripts_for(crate::scripts::Trigger::EnterRoom { room: to }, None);
        Ok(())
    }

    // ------------------------------------------------------------------ fog of war

    /// Party members whose vision a player shares.
    pub fn viewers_for(&self, who: Principal) -> Vec<&Creature> {
        let room = self.state.party_room;
        self.state
            .creatures
            .values()
            .filter(|c| {
                c.room == room && c.is_alive() && c.faction == Faction::Party && c.hp.current > 0
            })
            .filter(|c| match who {
                Principal::Dm => true,
                Principal::Player(p) => {
                    self.state.settings.shared_party_vision || c.controller == Controller::Player(p)
                }
            })
            .collect()
    }

    /// Per-cell visibility for a set of viewers: 0 none, 1 gray (darkvision in darkness), 2 dim, 3 bright.
    pub fn visibility(
        &self,
        room: RoomId,
        viewers: &[&Creature],
        lights: &[LightLevel],
    ) -> Vec<u8> {
        let Ok(r) = self.room(room) else {
            return vec![];
        };
        let mut vis = vec![0u8; (r.map.width * r.map.height) as usize];
        for v in viewers {
            let mut senses = v.senses;
            senses.blinded = v.conditions.has(Condition::Blinded);
            let seen = dnd_world::vision::visibility_map(&r.map, lights, v.pos, senses);
            for (i, s) in seen.iter().enumerate() {
                let val = match s {
                    Seen::NotVisible => 0,
                    Seen::Visible {
                        grayscale: true, ..
                    } => 1,
                    Seen::Visible {
                        effective: LightLevel::Dim,
                        ..
                    } => 2,
                    Seen::Visible { .. } => 3,
                };
                vis[i] = vis[i].max(val);
            }
        }
        vis
    }

    pub(crate) fn update_fog(&mut self) {
        let room = self.state.party_room;
        let lights = self.light_levels(room);
        let vis = self.visibility(room, &self.viewers_for(Principal::Dm), &lights);
        if let Some(r) = self.state.rooms.get_mut(&room) {
            for (e, v) in r.explored.iter_mut().zip(vis) {
                *e |= v > 0;
            }
        }
    }
}

fn vfx_for(t: Option<DamageType>, kind: WeaponKind) -> Vfx {
    use DamageType::*;
    match t {
        Some(Piercing) if kind == WeaponKind::Ranged => Vfx::Arrow,
        Some(Piercing) => Vfx::Pierce,
        Some(Slashing) => Vfx::Slash,
        Some(Bludgeoning) | None => Vfx::Bludgeon,
        Some(Fire) => Vfx::Fire,
        Some(Cold) => Vfx::Cold,
        Some(Lightning) => Vfx::Lightning,
        Some(Poison) => Vfx::Poison,
        Some(Acid) => Vfx::Acid,
        Some(Radiant) => Vfx::Radiant,
        Some(Necrotic) => Vfx::Necrotic,
        Some(Force) => Vfx::Force,
        Some(Thunder) => Vfx::Thunder,
        Some(Psychic) => Vfx::Psychic,
    }
}

/// Movement context: other creatures' spaces ("Moving around Other Creatures").
struct Occupancy {
    /// cell -> (passable for mover, difficult for mover)
    cells: HashMap<Cell, (bool, bool)>,
}

impl Occupancy {
    pub(crate) fn new(s: &Session, mover: EntityId) -> Self {
        let m = &s.state.creatures[&mover];
        let size_idx = |sz: dnd_rules::content::Size| sz as i32;
        let mut cells = HashMap::new();
        for c in s
            .state
            .creatures
            .values()
            .filter(|c| c.room == m.room && c.id != mover && c.is_alive())
        {
            let ally = !Session::are_hostile(c, m);
            let tiny = c.size == dnd_rules::content::Size::Tiny;
            let passable = ally
                || tiny
                || c.conditions.has(Condition::Incapacitated)
                || (size_idx(c.size) - size_idx(m.size)).abs() >= 2;
            let difficult = !(ally || tiny);
            cells.insert(c.pos, (passable, difficult));
        }
        Occupancy { cells }
    }
}

impl MoveContext for Occupancy {
    fn passable(&self, c: Cell) -> bool {
        self.cells.get(&c).is_none_or(|v| v.0)
    }
    fn difficult(&self, c: Cell) -> bool {
        self.cells.get(&c).is_some_and(|v| v.1)
    }
    fn can_end(&self, c: Cell) -> bool {
        !self.cells.contains_key(&c)
    }
}
