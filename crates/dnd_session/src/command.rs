//! Commands are the only way to change the game state. Clients send them; the host validates them.

use dnd_rules::character::CharacterSheet;
use dnd_rules::conditions::Condition;
use dnd_rules::d20::RollMode;
use dnd_world::map::{AmbientLight, Cell, LayerKind};
use dnd_world::{EdgeId, EntityId, ObjectId, RoomId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Chat { text: String },
    RollDice { expr: String, mode: RollMode, private: bool, label: String },
    /// Create or replace the player's character. Portrait is a PNG (validated and re-encoded by the host).
    SubmitCharacter {
        sheet: Box<CharacterSheet>,
        #[serde(with = "crate::hexbytes::opt")]
        portrait_png: Option<Vec<u8>>,
    },
    SetReady(bool),
    Move { entity: EntityId, to: Cell },
    Attack { attacker: EntityId, target: EntityId, attack: usize },
    Dash { entity: EntityId },
    Disengage { entity: EntityId },
    Dodge { entity: EntityId },
    EndTurn,
    /// Answer to a pending Opportunity Attack prompt.
    ReactionDecision { accept: bool, attack: usize },
    Interact { entity: EntityId, object: ObjectId },
    Dm(DmCommand),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DmCommand {
    StartSession,
    ViewRoom { room: RoomId },
    CreateRoom { name: String, width: i32, height: i32, ambient: AmbientLight },
    RenameRoom { room: RoomId, name: String, description: String },
    ConnectRooms { from: RoomId, to: RoomId, one_way: bool },
    SetEdge { edge: EdgeId, locked: bool, secret: bool, arrival: Option<Cell> },
    DeleteEdge { edge: EdgeId },
    PaintTiles { room: RoomId, layer: LayerKind, cells: Vec<Cell>, asset: Option<String> },
    FillRect { room: RoomId, layer: LayerKind, a: Cell, b: Cell, asset: Option<String> },
    FloodFill { room: RoomId, layer: LayerKind, start: Cell, asset: Option<String> },
    SetLayer { room: RoomId, layer: LayerKind, visible: bool, locked: bool },
    PlaceObject { room: RoomId, asset: String, cell: Cell, hidden: bool },
    RemoveObject { room: RoomId, object: ObjectId },
    SetObject { room: RoomId, object: ObjectId, hidden: bool, open: bool },
    SetAmbient { room: RoomId, ambient: AmbientLight },
    SetEntryPoint { room: RoomId, cell: Cell },
    AddLight { room: RoomId, cell: Cell, bright_ft: u32, dim_ft: u32 },
    ClearLights { room: RoomId },
    Undo { room: RoomId },
    Redo { room: RoomId },
    SpawnMonster { template: String, room: RoomId, cell: Cell, hidden: bool },
    RemoveCreature { entity: EntityId },
    SetCreatureHidden { entity: EntityId, hidden: bool },
    /// Moves a creature without movement rules (DM override).
    Teleport { entity: EntityId, cell: Cell },
    AdjustHp { entity: EntityId, delta: i32 },
    SetCondition { entity: EntityId, condition: Condition, present: bool },
    StartCombat { surprised: Vec<EntityId> },
    EndCombat,
    /// Forces the current turn to end (skip).
    NextTurn,
    ReorderInitiative { order: Vec<EntityId> },
    TransitionParty { to: RoomId, override_rules: bool },
    RevealRoom { room: RoomId, explored: bool },
    SetExplorationMovement { allowed: bool },
}

impl Command {
    /// Map-editing commands snapshot the room for undo.
    pub fn edited_room(&self) -> Option<RoomId> {
        match self {
            Command::Dm(
                DmCommand::PaintTiles { room, .. }
                | DmCommand::FillRect { room, .. }
                | DmCommand::FloodFill { room, .. }
                | DmCommand::PlaceObject { room, .. }
                | DmCommand::RemoveObject { room, .. }
                | DmCommand::SetObject { room, .. }
                | DmCommand::SetAmbient { room, .. }
                | DmCommand::SetLayer { room, .. }
                | DmCommand::AddLight { room, .. }
                | DmCommand::ClearLights { room },
            ) => Some(*room),
            _ => None,
        }
    }
}
