//! The campaign world graph: rooms (nodes) and connections (edges).
//!
//! Supports cycles, branches, one-way and secret routes, locked routes with prerequisites, and
//! regions/chapters. Room transitions are **only** performed by the DM; this module validates them.

use crate::ids::{EdgeId, RoomId};
use crate::map::Cell;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomNode {
    pub id: RoomId,
    pub name: String,
    pub description: String,
    pub dm_notes: String,
    pub region: String,
    pub chapter: String,
    /// Secret rooms are not shown to players on any map/graph until revealed.
    pub secret: bool,
    /// Position in the graph editor canvas.
    pub editor_pos: (f32, f32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomEdge {
    pub id: EdgeId,
    pub from: RoomId,
    pub to: RoomId,
    pub one_way: bool,
    pub secret: bool,
    pub locked: bool,
    /// Free-text prerequisite shown to the DM (e.g. "Requires the iron key").
    pub prerequisite: String,
    pub label: String,
    /// Where the party appears when arriving through this edge (in the destination room).
    pub arrival: Option<Cell>,
    /// Where the party appears when travelling the edge backwards (in `from`), for two-way edges.
    pub return_arrival: Option<Cell>,
}

impl RoomEdge {
    pub fn connects(&self, a: RoomId, b: RoomId) -> bool {
        (self.from == a && self.to == b) || (!self.one_way && self.from == b && self.to == a)
    }
    /// Arrival cell when travelling from `origin` across this edge.
    pub fn arrival_from(&self, origin: RoomId) -> Option<Cell> {
        if origin == self.from { self.arrival } else { self.return_arrival }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldGraph {
    pub nodes: BTreeMap<RoomId, RoomNode>,
    pub edges: Vec<RoomEdge>,
    next_room: u32,
    next_edge: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum TravelError {
    #[error("unknown room")]
    UnknownRoom,
    #[error("rooms are not connected")]
    NotConnected,
    #[error("the route is locked")]
    Locked,
    #[error("already in that room")]
    SameRoom,
}

impl WorldGraph {
    pub fn add_room(&mut self, name: impl Into<String>) -> RoomId {
        self.next_room += 1;
        let id = RoomId(self.next_room);
        let n = self.nodes.len() as f32;
        self.nodes.insert(
            id,
            RoomNode {
                id,
                name: name.into(),
                description: String::new(),
                dm_notes: String::new(),
                region: String::new(),
                chapter: String::new(),
                secret: false,
                editor_pos: (n * 160.0, 0.0),
            },
        );
        id
    }

    pub fn connect(&mut self, from: RoomId, to: RoomId, one_way: bool) -> Option<EdgeId> {
        if !self.nodes.contains_key(&from) || !self.nodes.contains_key(&to) || from == to {
            return None;
        }
        self.next_edge += 1;
        let id = EdgeId(self.next_edge);
        self.edges.push(RoomEdge {
            id,
            from,
            to,
            one_way,
            secret: false,
            locked: false,
            prerequisite: String::new(),
            label: String::new(),
            arrival: None,
            return_arrival: None,
        });
        Some(id)
    }

    pub fn edge_mut(&mut self, id: EdgeId) -> Option<&mut RoomEdge> {
        self.edges.iter_mut().find(|e| e.id == id)
    }

    pub fn remove_room(&mut self, id: RoomId) -> bool {
        self.edges.retain(|e| e.from != id && e.to != id);
        self.nodes.remove(&id).is_some()
    }

    pub fn remove_edge(&mut self, id: EdgeId) -> bool {
        let n = self.edges.len();
        self.edges.retain(|e| e.id != id);
        n != self.edges.len()
    }

    /// Edges that can be travelled from `room` (respecting one-way direction).
    pub fn exits(&self, room: RoomId) -> impl Iterator<Item = &RoomEdge> {
        self.edges.iter().filter(move |e| e.from == room || (!e.one_way && e.to == room))
    }

    /// Destination when leaving `room` through `edge`.
    pub fn other_end(edge: &RoomEdge, room: RoomId) -> RoomId {
        if edge.from == room { edge.to } else { edge.from }
    }

    /// Validates a transition. With `dm_override` the DM may ignore locks or move to any room.
    pub fn validate_travel(&self, from: RoomId, to: RoomId, dm_override: bool) -> Result<Option<&RoomEdge>, TravelError> {
        if !self.nodes.contains_key(&from) || !self.nodes.contains_key(&to) {
            return Err(TravelError::UnknownRoom);
        }
        if from == to {
            return Err(TravelError::SameRoom);
        }
        let edge = self.edges.iter().find(|e| e.connects(from, to));
        match edge {
            None if dm_override => Ok(None),
            None => Err(TravelError::NotConnected),
            Some(e) if e.locked && !dm_override => Err(TravelError::Locked),
            Some(e) => Ok(Some(e)),
        }
    }

    /// Rooms reachable from `start` following travellable edges (ignoring locks if `ignore_locks`).
    pub fn reachable(&self, start: RoomId, ignore_locks: bool) -> BTreeSet<RoomId> {
        let mut seen = BTreeSet::from([start]);
        let mut q = VecDeque::from([start]);
        while let Some(r) = q.pop_front() {
            for e in self.exits(r) {
                if e.locked && !ignore_locks {
                    continue;
                }
                let n = Self::other_end(e, r);
                if seen.insert(n) {
                    q.push_back(n);
                }
            }
        }
        seen
    }

    /// Graph as players may see it: secret rooms and secret edges removed, notes stripped.
    pub fn player_view(&self, revealed: &BTreeSet<RoomId>) -> WorldGraph {
        let mut g = self.clone();
        g.nodes.retain(|id, n| !n.secret || revealed.contains(id));
        for n in g.nodes.values_mut() {
            n.dm_notes.clear();
        }
        let keep: BTreeSet<RoomId> = g.nodes.keys().copied().collect();
        g.edges.retain(|e| !e.secret && keep.contains(&e.from) && keep.contains(&e.to));
        for e in &mut g.edges {
            e.prerequisite.clear();
        }
        g
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain() -> (WorldGraph, Vec<RoomId>) {
        let mut g = WorldGraph::default();
        let ids: Vec<RoomId> = ["Entrance", "Forest", "Cave", "Temple", "Boss"].iter().map(|n| g.add_room(*n)).collect();
        for w in ids.windows(2) {
            g.connect(w[0], w[1], false);
        }
        (g, ids)
    }

    #[test]
    fn travel_validation() {
        let (mut g, r) = chain();
        assert!(g.validate_travel(r[0], r[1], false).is_ok());
        assert!(g.validate_travel(r[1], r[0], false).is_ok(), "two-way");
        assert_eq!(g.validate_travel(r[0], r[2], false), Err(TravelError::NotConnected));
        assert!(g.validate_travel(r[0], r[2], true).is_ok(), "DM override");
        let e = g.edges[1].id;
        g.edge_mut(e).unwrap().locked = true;
        assert_eq!(g.validate_travel(r[1], r[2], false), Err(TravelError::Locked));
        assert!(g.validate_travel(r[1], r[2], true).is_ok());
    }

    #[test]
    fn one_way_edges_and_cycles() {
        let (mut g, r) = chain();
        let back = g.connect(r[4], r[0], true).unwrap(); // portal back to entrance: creates a cycle
        assert!(g.validate_travel(r[4], r[0], false).is_ok());
        assert_eq!(g.validate_travel(r[0], r[4], false), Err(TravelError::NotConnected));
        assert_eq!(g.reachable(r[2], false).len(), 5);
        g.remove_edge(back);
        g.edges[3].locked = true;
        assert_eq!(g.reachable(r[0], false).len(), 4);
        assert_eq!(g.reachable(r[0], true).len(), 5);
    }

    #[test]
    fn player_view_hides_secrets() {
        let (mut g, r) = chain();
        let secret = g.add_room("Hidden Vault");
        g.nodes.get_mut(&secret).unwrap().secret = true;
        g.nodes.get_mut(&r[0]).unwrap().dm_notes = "trap at door".into();
        g.connect(r[2], secret, false);
        let pv = g.player_view(&BTreeSet::new());
        assert!(!pv.nodes.contains_key(&secret));
        assert!(pv.edges.iter().all(|e| e.to != secret));
        assert!(pv.nodes[&r[0]].dm_notes.is_empty());
        let pv = g.player_view(&[secret].into());
        assert!(pv.nodes.contains_key(&secret));
    }

    #[test]
    fn removing_room_removes_edges() {
        let (mut g, r) = chain();
        g.remove_room(r[2]);
        assert_eq!(g.edges.len(), 2);
    }
}
