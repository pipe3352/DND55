//! Scripted events: "when <trigger> then <actions>", authored in the DM's Events panel (no code).
//!
//! Examples: stepping on a pressure plate fires a trap (Dex save, 2d6 Piercing); pulling a lever opens
//! a gate; defeating the boss unlocks a route; opening a chest gives a potion; entering a room starts a
//! dialogue; inspecting an altar asks for a Religion check with success/failure branches.

use dnd_rules::ability::{Ability, Skill};
use dnd_rules::damage::DamageType;
use dnd_world::map::{AmbientLight, Cell};
use dnd_world::{EdgeId, EntityId, ObjectId, RoomId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Trigger {
    /// A player character enters a cell inside the rectangle (inclusive).
    EnterArea { room: RoomId, a: Cell, b: Cell },
    /// A character opens/uses an interactable object (door, chest, lever).
    Interact { room: RoomId, object: ObjectId },
    /// A creature is defeated (dies).
    CreatureDefeated { entity: EntityId },
    /// The party travels into a room.
    EnterRoom { room: RoomId },
    /// Only fired by the DM's "Fire now" button.
    Manual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DialogueChoice {
    pub text: String,
    pub actions: Vec<ScriptAction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScriptAction {
    /// Narration shown to everyone.
    Message { text: String },
    /// Opens (true), closes (false) or toggles (None) a door/gate/lever/chest.
    SetObjectOpen { room: RoomId, object: ObjectId, open: Option<bool> },
    /// Reveals a hidden object (secret door, trap, treasure).
    RevealObject { room: RoomId, object: ObjectId },
    /// Reveals a hidden creature (ambush).
    RevealCreature { entity: EntityId },
    /// Damage to the triggering character, optionally halved/avoided by a saving throw.
    Trap { dice: String, damage_type: DamageType, save: Option<Ability>, dc: i32, half_on_success: bool },
    /// Adds an item from the item catalog to the triggering character's inventory.
    GiveItem { item_id: String, quantity: u32 },
    /// The triggering character makes a skill check rolled by the host.
    SkillCheck { skill: Skill, dc: i32, on_success: Vec<ScriptAction>, on_failure: Vec<ScriptAction> },
    /// Unlocks (or locks) a connection in the world graph.
    SetEdgeLocked { edge: EdgeId, locked: bool },
    SpawnMonster { template: String, room: RoomId, cell: Cell, hidden: bool },
    /// NPC dialogue with optional choices; players pick, the DM can override.
    Dialogue { speaker: String, text: String, choices: Vec<DialogueChoice> },
    SetAmbient { room: RoomId, ambient: AmbientLight },
    AwardXp { xp: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptedEvent {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    /// Fire only the first time.
    pub once: bool,
    pub fired: bool,
    pub trigger: Trigger,
    pub actions: Vec<ScriptAction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingDialogue {
    pub event_id: u64,
    pub speaker: String,
    pub text: String,
    pub choices: Vec<DialogueChoice>,
    /// Character whose action triggered the dialogue (receives items/checks from choices).
    pub actor: Option<EntityId>,
}

impl ScriptAction {
    /// One-line summary for the DM's event list.
    pub fn summary(&self) -> String {
        match self {
            ScriptAction::Message { text } => format!("Message: {}", text.chars().take(40).collect::<String>()),
            ScriptAction::SetObjectOpen { open, .. } => format!("{} object", match open { Some(true) => "Open", Some(false) => "Close", None => "Toggle" }),
            ScriptAction::RevealObject { .. } => "Reveal object".into(),
            ScriptAction::RevealCreature { .. } => "Reveal creature".into(),
            ScriptAction::Trap { dice, damage_type, save, dc, .. } => match save {
                Some(a) => format!("Trap {dice} {damage_type:?} (DC {dc} {a:?} save)"),
                None => format!("Trap {dice} {damage_type:?}"),
            },
            ScriptAction::GiveItem { item_id, quantity } => format!("Give {quantity}× {item_id}"),
            ScriptAction::SkillCheck { skill, dc, .. } => format!("{skill:?} check DC {dc}"),
            ScriptAction::SetEdgeLocked { locked, .. } => (if *locked { "Lock route" } else { "Unlock route" }).into(),
            ScriptAction::SpawnMonster { template, .. } => format!("Spawn {template}"),
            ScriptAction::Dialogue { speaker, .. } => format!("Dialogue: {speaker}"),
            ScriptAction::SetAmbient { ambient, .. } => format!("Light → {ambient:?}"),
            ScriptAction::AwardXp { xp } => format!("Award {xp} XP"),
        }
    }
}

// ---------------------------------------------------------------------- execution

use crate::event::{Event, FloatKind};
use crate::model::Audience;
use crate::session::Session;

impl Session {
    /// Fires every enabled script whose trigger matches. `actor` is the character responsible (if any).
    pub(crate) fn fire_scripts_for(&mut self, trigger: Trigger, actor: Option<EntityId>) {
        let ids: Vec<u64> = self
            .state
            .scripts
            .iter()
            .filter(|s| s.enabled && !(s.once && s.fired) && trigger_matches(&s.trigger, &trigger))
            .map(|s| s.id)
            .collect();
        for id in ids {
            self.run_script(id, actor);
        }
    }

    pub(crate) fn run_script(&mut self, id: u64, actor: Option<EntityId>) {
        let Some(s) = self.state.scripts.iter_mut().find(|s| s.id == id) else { return };
        s.fired = true;
        let (name, actions) = (s.name.clone(), s.actions.clone());
        self.log(Audience::DmOnly, format!("Event fired: {name}"));
        self.run_actions(id, &actions, actor, 0);
    }

    fn run_actions(&mut self, script: u64, actions: &[ScriptAction], actor: Option<EntityId>, depth: u8) {
        if depth > 4 {
            return; // nested checks are bounded
        }
        for a in actions {
            match a {
                ScriptAction::Message { text } => {
                    self.log(Audience::All, text.clone());
                    self.emit(Audience::All, Event::Dialogue { speaker: String::new(), text: text.clone(), choices: vec![] });
                }
                ScriptAction::SetObjectOpen { room, object, open } => {
                    if let Some(rs) = self.state.rooms.get_mut(room)
                        && let Some(o) = rs.map.object_mut(*object)
                    {
                        o.open = open.unwrap_or(!o.open);
                        rs.map_rev += 1;
                    }
                }
                ScriptAction::RevealObject { room, object } => {
                    if let Some(rs) = self.state.rooms.get_mut(room)
                        && let Some(o) = rs.map.object_mut(*object)
                    {
                        o.hidden = false;
                        rs.map_rev += 1;
                        self.log(Audience::All, "Something hidden is revealed!");
                    }
                }
                ScriptAction::RevealCreature { entity } => {
                    if let Some(c) = self.state.creatures.get_mut(entity) {
                        c.hidden = false;
                        let n = c.name.clone();
                        self.log(Audience::All, format!("{n} appears!"));
                    }
                }
                ScriptAction::Trap { dice, damage_type, save, dc, half_on_success } => {
                    let Some(t) = actor.filter(|t| self.state.creatures.contains_key(t)) else { continue };
                    let ok = match save {
                        Some(ab) => self.saving_throw(t, *ab, *dc, "trap").0,
                        None => false,
                    };
                    let r = dnd_rules::DiceExpr::parse(dice).unwrap_or_default().roll(self.rng.as_mut(), false);
                    let base = if ok { if *half_on_success { r.total / 2 } else { 0 } } else { r.total };
                    let tc = self.state.creatures[&t].clone();
                    let o = self.defenses_of(&tc).apply(*damage_type, base.max(0), 0);
                    self.log(Audience::All, format!("A trap strikes {}: {} → {} {damage_type:?}", tc.name, r.breakdown(), o.final_amount));
                    if o.final_amount > 0 {
                        self.emit(Audience::All, Event::Floating { entity: t, text: format!("-{}", o.final_amount), kind: FloatKind::Damage });
                        self.apply_damage(t, o.final_amount, false);
                    }
                }
                ScriptAction::GiveItem { item_id, quantity } => {
                    if let (Some(t), Some(mut it)) = (actor, dnd_rules::items::catalog_item(item_id)) {
                        it.quantity = (*quantity).max(1);
                        if self.state.creatures.get(&t).is_some_and(|c| c.sheet().is_some()) {
                            self.give_item_to(t, it);
                        }
                    }
                }
                ScriptAction::SkillCheck { skill, dc, on_success, on_failure } => {
                    let Some(t) = actor.filter(|t| self.state.creatures.contains_key(t)) else { continue };
                    let c = self.state.creatures[&t].clone();
                    let bonus = c.sheet().map(|s| s.skill_bonus(*skill)).unwrap_or(c.mods[skill.ability().index()]);
                    let roll = dnd_rules::D20Roll::roll(self.rng.as_mut(), dnd_rules::RollMode::Normal, bonus, c.conditions.exhaustion);
                    let ok = roll.total >= *dc;
                    self.log(Audience::All, format!("{} makes a {skill:?} check (DC {dc}): {} → {}", c.name, roll.breakdown(), if ok { "success" } else { "failure" }));
                    let branch = if ok { on_success.clone() } else { on_failure.clone() };
                    self.run_actions(script, &branch, actor, depth + 1);
                }
                ScriptAction::SetEdgeLocked { edge, locked } => {
                    if let Some(e) = self.state.graph.edge_mut(*edge) {
                        e.locked = *locked;
                        self.log(Audience::All, if *locked { "A way is barred." } else { "A way opens." });
                    }
                }
                ScriptAction::SpawnMonster { template, room, cell, hidden } => {
                    let _ = self.dm(crate::command::DmCommand::SpawnMonster { template: template.clone(), room: *room, cell: *cell, hidden: *hidden });
                }
                ScriptAction::Dialogue { speaker, text, choices } => {
                    self.state.pending_dialogue = (!choices.is_empty()).then(|| PendingDialogue { event_id: script, speaker: speaker.clone(), text: text.clone(), choices: choices.clone(), actor });
                    self.log(Audience::All, format!("{speaker}: \u{201c}{text}\u{201d}"));
                    self.emit(Audience::All, Event::Dialogue { speaker: speaker.clone(), text: text.clone(), choices: choices.iter().map(|c| c.text.clone()).collect() });
                }
                ScriptAction::SetAmbient { room, ambient } => {
                    if let Some(rs) = self.state.rooms.get_mut(room) {
                        rs.map.ambient = *ambient;
                        rs.map_rev += 1;
                    }
                }
                ScriptAction::AwardXp { xp } => {
                    let party: Vec<EntityId> = self.state.creatures.values().filter(|c| c.sheet().is_some() && c.is_alive()).map(|c| c.id).collect();
                    let _ = self.award_xp(party, *xp);
                }
            }
        }
    }

    pub(crate) fn cmd_dialogue_choice(&mut self, choice: usize) -> Result<(), crate::session::CommandError> {
        let d = self.state.pending_dialogue.take().ok_or_else(|| crate::session::CommandError::Invalid("no dialogue is waiting for a choice".into()))?;
        let Some(c) = d.choices.get(choice).cloned() else {
            self.state.pending_dialogue = Some(d);
            return Err(crate::session::CommandError::Invalid("no such choice".into()));
        };
        self.log(Audience::All, format!("\u{2192} {}", c.text));
        self.run_actions(d.event_id, &c.actions, d.actor, 1);
        Ok(())
    }
}

fn trigger_matches(rule: &Trigger, happened: &Trigger) -> bool {
    match (rule, happened) {
        (Trigger::EnterArea { room, a, b }, Trigger::EnterArea { room: r2, a: cell, .. }) => {
            room == r2 && (a.0.min(b.0)..=a.0.max(b.0)).contains(&cell.0) && (a.1.min(b.1)..=a.1.max(b.1)).contains(&cell.1)
        }
        (Trigger::Interact { room, object }, Trigger::Interact { room: r2, object: o2 }) => room == r2 && object == o2,
        (Trigger::CreatureDefeated { entity }, Trigger::CreatureDefeated { entity: e2 }) => entity == e2,
        (Trigger::EnterRoom { room }, Trigger::EnterRoom { room: r2 }) => room == r2,
        _ => false,
    }
}
