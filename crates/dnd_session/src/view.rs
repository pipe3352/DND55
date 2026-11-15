//! Per-client views. **This is the hidden-information boundary**: a player's view only ever contains
//! what that player is allowed to know. The DM's view contains everything.

use crate::event::Routed;
use crate::model::*;
use crate::session::Session;
use dnd_rules::attack::AttackProfile;
use dnd_rules::character::CharacterSheet;
use dnd_rules::conditions::Condition;
use dnd_rules::content::Size;
use dnd_rules::hp::LifeState;
use dnd_world::map::{Cell, GridMap};
use dnd_world::vision::PointLight;
use dnd_world::{EntityId, RoomId, WorldGraph};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HpView {
    Exact { current: i32, max: i32, temp: i32 },
    /// What players see of non-party creatures.
    Status(HealthStatus),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Bloodied,
    Down,
    Dead,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatureView {
    pub id: EntityId,
    pub name: String,
    pub pos: Cell,
    pub faction: Faction,
    pub size: Size,
    pub yours: bool,
    pub hp: HpView,
    pub conditions: Vec<Condition>,
    pub portrait: Option<String>,
    pub hidden: bool,
    /// Only for creatures you control (or the DM).
    pub armor_class: Option<i32>,
    pub attacks: Vec<AttackProfile>,
    pub speed_ft: i32,
    pub death_saves: (u8, u8),
    pub is_monster: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomView {
    pub id: RoomId,
    pub name: String,
    pub description: String,
    pub map_rev: u64,
    /// Present only when the client's cached revision is stale.
    pub map: Option<GridMap>,
    pub width: i32,
    pub height: i32,
    /// Per-cell: 0 unseen, 1 gray (darkvision), 2 dim, 3 bright. `None` = DM sees everything.
    pub visible: Option<Vec<u8>>,
    pub explored: Vec<bool>,
    /// Cosmetic light sources the viewer can see (for glow rendering).
    pub lights: Vec<PointLight>,
    pub entry: Cell,
    pub is_party_room: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InitiativeView {
    pub entity: EntityId,
    pub name: String,
    pub total: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombatView {
    pub round: u32,
    pub order: Vec<InitiativeView>,
    /// `None` when the active creature is hidden from you.
    pub active: Option<EntityId>,
    /// Turn resources, if the active creature is yours.
    pub turn: Option<TurnState>,
    /// A reaction you must decide on.
    pub your_reaction: Option<PendingReaction>,
    pub waiting_on_reaction: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerPublic {
    pub id: PlayerId,
    pub name: String,
    pub character: Option<String>,
    pub ready: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonsterSummary {
    pub id: String,
    pub name: String,
    pub challenge: String,
    pub armor_class: i32,
    pub hit_points: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DmExtras {
    pub party_room: RoomId,
    pub bestiary: Vec<MonsterSummary>,
    pub exploration_movement: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientView {
    pub you: Principal,
    pub campaign_name: String,
    pub session_started: bool,
    pub players: Vec<PlayerPublic>,
    pub room: Option<RoomView>,
    pub creatures: Vec<CreatureView>,
    pub my_characters: Vec<(EntityId, CharacterSheet)>,
    pub combat: Option<CombatView>,
    pub graph: WorldGraph,
    pub log: Vec<String>,
    pub dm: Option<DmExtras>,
}

pub fn audience_includes(a: Audience, who: Principal) -> bool {
    match (a, who) {
        (Audience::All, _) => true,
        (_, Principal::Dm) => true,
        (Audience::DmOnly, Principal::Player(_)) => false,
        (Audience::DmAnd(p), Principal::Player(q)) => p == q,
    }
}

impl Session {
    /// Ids of creatures the principal may know about right now.
    pub fn visible_creature_ids(&self, who: Principal) -> HashSet<EntityId> {
        match who {
            Principal::Dm => self.state.creatures.keys().copied().collect(),
            Principal::Player(_) => {
                let room = self.state.party_room;
                let lights = self.light_levels(room);
                let viewers = self.viewers_for(who);
                self.state
                    .creatures
                    .values()
                    .filter(|c| c.room == room)
                    .filter(|c| {
                        (c.faction == Faction::Party && !c.hidden)
                            || Self::controls(who, c)
                            || (!c.hidden && viewers.iter().any(|v| v.id == c.id || self.can_see(v, c, &lights)))
                    })
                    .map(|c| c.id)
                    .collect()
            }
        }
    }

    /// Filters routed events for one recipient. Events about creatures the recipient can't see are dropped.
    pub fn events_for(&self, who: Principal, events: &[Routed], visible: &HashSet<EntityId>) -> Vec<crate::event::Event> {
        events
            .iter()
            .filter(|r| audience_includes(r.audience, who))
            .filter(|r| who == Principal::Dm || r.event.subject().is_none_or(|e| visible.contains(&e)))
            .map(|r| r.event.clone())
            .collect()
    }

    pub fn build_view(&self, who: Principal, known_map: Option<(RoomId, u64)>) -> ClientView {
        let s = &self.state;
        let visible_ids = self.visible_creature_ids(who);
        let room_id = match who {
            Principal::Dm => s.dm_view_room,
            Principal::Player(_) => s.party_room,
        };
        let lights = self.light_levels(room_id);
        let room = s.rooms.get(&room_id).map(|r| {
            let (visible, lights_out) = match who {
                Principal::Dm => (None, dnd_world::vision::collect_lights(&r.map, &r.extra_lights)),
                Principal::Player(_) => {
                    let vis = self.visibility(room_id, &self.viewers_for(who), &lights);
                    let ls = dnd_world::vision::collect_lights(&r.map, &r.extra_lights)
                        .into_iter()
                        .filter(|l| vis.get((l.cell.1 * r.map.width + l.cell.0) as usize).is_some_and(|v| *v > 0))
                        .collect();
                    (Some(vis), ls)
                }
            };
            let stale = known_map != Some((room_id, r.map_rev));
            let node = s.graph.nodes.get(&room_id);
            RoomView {
                id: room_id,
                name: node.map(|n| n.name.clone()).unwrap_or_default(),
                description: node.map(|n| n.description.clone()).unwrap_or_default(),
                map_rev: r.map_rev,
                map: stale.then(|| if who == Principal::Dm { r.map.clone() } else { r.map.player_view() }),
                width: r.map.width,
                height: r.map.height,
                visible,
                explored: r.explored.clone(),
                lights: lights_out,
                entry: if who == Principal::Dm { r.entry } else { (0, 0) },
                is_party_room: room_id == s.party_room,
            }
        });

        let creatures = s
            .creatures
            .values()
            .filter(|c| c.room == room_id && visible_ids.contains(&c.id))
            .map(|c| {
                let full = who == Principal::Dm || c.faction == Faction::Party || Self::controls(who, c);
                let mine = Self::controls(who, c);
                let hp = if full {
                    HpView::Exact { current: c.hp.current, max: c.hp.max, temp: c.hp.temp }
                } else {
                    HpView::Status(match c.hp.life_state() {
                        LifeState::Dead => HealthStatus::Dead,
                        LifeState::Dying | LifeState::Stable => HealthStatus::Down,
                        LifeState::Conscious if c.hp.is_bloodied() => HealthStatus::Bloodied,
                        LifeState::Conscious => HealthStatus::Healthy,
                    })
                };
                CreatureView {
                    id: c.id,
                    name: c.name.clone(),
                    pos: c.pos,
                    faction: c.faction,
                    size: c.size,
                    yours: match who {
                        Principal::Dm => c.controller == Controller::Dm,
                        Principal::Player(p) => c.controller == Controller::Player(p),
                    },
                    hp,
                    conditions: c.conditions.conditions.iter().copied().collect(),
                    portrait: c.portrait.clone(),
                    hidden: c.hidden && who == Principal::Dm,
                    armor_class: mine.then_some(c.armor_class),
                    attacks: if mine { c.attacks.clone() } else { vec![] },
                    speed_ft: c.effective_speed(),
                    death_saves: (c.hp.death_saves.successes, c.hp.death_saves.failures),
                    is_monster: matches!(c.kind, CreatureKind::Monster { .. }),
                }
            })
            .collect();

        let my_characters = s
            .creatures
            .values()
            .filter(|c| matches!(who, Principal::Player(p) if c.controller == Controller::Player(p)))
            .filter_map(|c| c.sheet().map(|sh| (c.id, sh.clone())))
            .collect();

        let combat = s.combat.as_ref().filter(|cb| cb.room == room_id || who != Principal::Dm).map(|cb| {
            let order = cb
                .order
                .iter()
                .filter(|e| visible_ids.contains(&e.entity))
                .map(|e| InitiativeView {
                    entity: e.entity,
                    name: s.creatures.get(&e.entity).map(|c| c.name.clone()).unwrap_or_default(),
                    total: e.total,
                })
                .collect();
            let active_c = s.creatures.get(&cb.turn.entity);
            let active_visible = visible_ids.contains(&cb.turn.entity);
            let your_turn = active_c.is_some_and(|c| Self::controls(who, c));
            let your_reaction = cb
                .pending
                .as_ref()
                .filter(|p| s.creatures.get(&p.reactor).is_some_and(|r| Self::controls(who, r)))
                .cloned();
            CombatView {
                round: cb.round,
                order,
                active: active_visible.then_some(cb.turn.entity),
                turn: your_turn.then(|| cb.turn.clone()),
                your_reaction,
                waiting_on_reaction: cb.pending.is_some(),
            }
        });

        let graph = match who {
            Principal::Dm => s.graph.clone(),
            Principal::Player(_) => {
                let visited: BTreeSet<RoomId> = s.rooms.iter().filter(|(_, r)| r.visited).map(|(id, _)| *id).collect();
                let mut g = s.graph.player_view(&visited);
                g.nodes.retain(|id, _| visited.contains(id));
                g.edges.retain(|e| visited.contains(&e.from) && visited.contains(&e.to));
                g
            }
        };

        let mut log: Vec<String> = s.log.iter().rev().filter(|l| audience_includes(l.audience, who)).take(200).map(|l| l.text.clone()).collect();
        log.reverse();

        let players = s
            .players
            .values()
            .map(|p| PlayerPublic {
                id: p.id,
                name: p.name.clone(),
                character: p.character.and_then(|id| s.creatures.get(&id)).map(|c| c.name.clone()),
                ready: p.ready,
                connected: p.connected,
            })
            .collect();

        let dm = (who == Principal::Dm).then(|| DmExtras {
            party_room: s.party_room,
            bestiary: self
                .bestiary()
                .into_iter()
                .map(|m| MonsterSummary { id: m.id, name: m.name, challenge: m.challenge, armor_class: m.armor_class, hit_points: m.hit_points })
                .collect(),
            exploration_movement: s.settings.exploration_movement,
        });

        ClientView {
            you: who,
            campaign_name: s.campaign_name.clone(),
            session_started: s.session_started,
            players,
            room,
            creatures,
            my_characters,
            combat,
            graph,
            log,
            dm,
        }
    }
}
