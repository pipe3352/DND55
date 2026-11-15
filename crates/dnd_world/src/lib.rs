//! # dnd_world — maps, rooms and the campaign graph
//!
//! Renderer-independent world model. The Bevy client renders these structures; the authoritative
//! host uses the same code for collision, pathfinding and visibility so that clients can never
//! disagree with the server about what is walkable or visible.

pub mod catalog;
pub mod graph;
pub mod ids;
pub mod map;
pub mod path;
pub mod vision;

pub use graph::{RoomEdge, RoomNode, TravelError, WorldGraph};
pub use ids::{EdgeId, EntityId, ObjectId, RoomId};
pub use map::{Cell, GridMap, LayerKind, MapObject};

/// Feet per grid square ("Playing on a Grid": each square represents 5 feet).
pub const FEET_PER_SQUARE: i32 = 5;
