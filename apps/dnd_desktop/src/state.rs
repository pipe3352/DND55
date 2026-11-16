//! Client-side resources. Nothing here is authoritative: the host's `ClientView` is the source of truth.

use bevy::prelude::*;
use dnd_net::{ClientMsg, HostHandle, HostRequest, NetClient};
use dnd_rules::character::CharacterSheet;
use dnd_rules::d20::RollMode;
use dnd_session::command::Command;
use dnd_session::event::Event;
use dnd_session::model::Principal;
use dnd_session::view::ClientView;
use dnd_world::map::{AmbientLight, GridMap, LayerKind};
use dnd_world::{EntityId, RoomId};

#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
pub enum AppState {
    #[default]
    MainMenu,
    InGame,
}

/// Connection to the (possibly local) host.
#[derive(Resource, Default)]
pub struct Net {
    pub client: Option<NetClient>,
    pub seq: u64,
    pub status: String,
    pub you: Option<Principal>,
    pub resume_token: Option<String>,
    /// Character to submit right after joining (from the local library).
    pub pending_character: Option<(CharacterSheet, Option<Vec<u8>>)>,
    pub address: String,
    pub hello: Option<dnd_net::Hello>,
    /// Where we connect (direct address or relay + invite code).
    pub target: Option<dnd_net::Target>,
    /// Automatic reconnection: next attempt time (seconds since startup) and attempts so far.
    pub retry_at: Option<f64>,
    pub retries: u32,
    /// The host refused us (wrong code, version…): don't retry automatically.
    pub rejected: bool,
    pub rtt_ms: Option<u64>,
    pub last_ping: f64,
}

/// Maximum automatic reconnection attempts before asking the user.
pub const MAX_RETRIES: u32 = 10;

impl Net {
    pub fn cmd(&mut self, command: Command) -> u64 {
        self.seq += 1;
        if let Some(c) = &self.client {
            c.send(ClientMsg::Cmd {
                seq: self.seq,
                command,
            });
        }
        self.seq
    }
    pub fn save(&mut self, label: String) {
        self.seq += 1;
        if let Some(c) = &self.client {
            c.send(ClientMsg::Host {
                seq: self.seq,
                request: HostRequest::Save { label },
            });
        }
    }
    pub fn is_dm(&self) -> bool {
        self.you == Some(Principal::Dm)
    }
}

/// The DM's in-process authoritative host.
#[derive(Resource)]
pub struct HostRuntime {
    /// Owns the host's Tokio runtime; dropping it stops the host.
    #[allow(dead_code)]
    pub runtime: tokio::runtime::Runtime,
    pub handle: HostHandle,
    #[allow(dead_code)]
    pub campaign_id: i64,
    /// Result of the optional UPnP port mapping (external address or error).
    pub upnp: std::sync::Arc<std::sync::Mutex<Option<Result<std::net::SocketAddr, String>>>>,
}

impl Drop for HostRuntime {
    fn drop(&mut self) {
        self.handle.shutdown();
    }
}

/// Latest authoritative view plus the cached map.
#[derive(Resource, Default)]
pub struct GameView {
    pub view: Option<ClientView>,
    pub map: Option<GridMap>,
    pub map_key: Option<(RoomId, u64)>,
    pub map_dirty: bool,
    pub view_dirty: bool,
    /// Events not yet turned into animations/sounds.
    pub events: Vec<Event>,
    pub toasts: Vec<(String, f32)>,
}

impl GameView {
    pub fn toast(&mut self, s: impl Into<String>) {
        self.toasts.push((s.into(), 5.0));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Play,
    Pencil,
    Eraser,
    Rect,
    Fill,
    Object,
    Spawn,
    Entry,
    Light,
    RemoveObject,
    /// Select/move/copy map objects (box or Shift multi-select).
    Select,
    /// Freehand drawing / annotations.
    Draw,
}

/// How a pending spell or monster action wants its targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetMode {
    /// Click a point (area spells, directions for cones/lines).
    Point,
    /// Click up to `max_targets` creatures (repeats allowed for darts/rays/beams).
    Creatures,
    /// Light-style: a creature (its gear) or a point.
    PointOrCreature,
}

/// A spell (or monster save action) waiting for target clicks.
#[derive(Debug, Clone, PartialEq)]
pub struct CastIntent {
    pub caster: EntityId,
    pub spell: String,
    pub label: String,
    pub slot: Option<u8>,
    pub targets: Vec<EntityId>,
    pub max_targets: usize,
    pub repeat: bool,
    pub mode: TargetMode,
    /// Area preview (shape, size) for point targeting.
    pub area: Option<dnd_rules::spells::Area>,
    pub self_origin: bool,
    pub range_ft: u32,
    pub monster_action: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawKind {
    Stroke,
    Line,
    Arrow,
    Circle,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuPage {
    #[default]
    Main,
    Create,
    Join,
    Load,
    Characters,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DmTab {
    #[default]
    Editor,
    World,
    Monsters,
    Events,
    Session,
}

#[derive(Resource)]
pub struct UiState {
    pub menu: MenuPage,
    pub dm_tab: DmTab,
    pub tool: Tool,
    pub layer: LayerKind,
    pub asset: String,
    pub asset_search: String,
    pub favorites: Vec<String>,
    pub monster: String,
    pub spawn_hidden: bool,
    pub object_hidden: bool,
    pub selected: Option<EntityId>,
    /// Attack index chosen; next click on a creature attacks it.
    pub targeting: Option<usize>,
    pub dice_expr: String,
    pub dice_mode: RollMode,
    pub private_roll: bool,
    pub chat: String,
    pub show_grid: bool,
    pub show_sheet: bool,
    pub show_creator: bool,
    pub new_room_name: String,
    pub new_room_w: i32,
    pub new_room_h: i32,
    pub new_room_ambient: AmbientLight,
    pub connect_to: Option<RoomId>,
    pub connect_one_way: bool,
    pub light_bright: u32,
    pub light_dim: u32,
    pub rect_start: Option<(i32, i32)>,
    pub save_label: String,
    pub reaction_attack: usize,
    pub surprised: Vec<EntityId>,
    pub room_desc_edit: Option<(RoomId, String, String)>,
    // Menu forms
    pub form_campaign: String,
    pub form_port: String,
    pub form_address: String,
    pub form_code: String,
    pub form_name: String,
    pub form_character: Option<i64>,
    pub ui_scale: f32,
    // ---- play
    pub casting: Option<CastIntent>,
    /// Feature waiting for a creature click (Bardic Inspiration, Divine Spark, Lay On Hands).
    pub feature_target: Option<(EntityId, dnd_session::command::Feature)>,
    pub loh_amount: i32,
    pub show_spellbook: bool,
    pub spellbook_prepare: bool,
    pub prep_draft: Option<(Vec<String>, Vec<String>)>,
    pub spell_search: String,
    pub show_inventory: bool,
    pub show_levelup: bool,
    pub levelup: dnd_session::command::LevelUpChoices,
    pub show_settings: bool,
    pub recent_assets: Vec<String>,
    pub show_collision: bool,
    pub show_light_radii: bool,
    /// Editor preview: hide the editor chrome to see the map as it plays.
    pub editor_preview: bool,
    /// Object context menu on the map: screen position (egui points) and object.
    pub map_context: Option<((f32, f32), dnd_world::ObjectId)>,
    pub show_invite: bool,
    pub dismissed_message: u64,
    // ---- DM editor
    pub sel_objects: Vec<dnd_world::ObjectId>,
    pub clipboard: Vec<dnd_world::map::MapObject>,
    /// Drag in progress: start cell, and whether it is moving selected objects (vs box select).
    pub drag: Option<((i32, i32), bool)>,
    pub draw_kind: DrawKind,
    pub draw_color: [u8; 4],
    pub draw_width: f32,
    pub draw_dm_only: bool,
    pub draw_temporary: bool,
    pub draw_text: String,
    pub draw_points: Vec<(f32, f32)>,
    // ---- DM windows
    pub show_bestiary: bool,
    pub bestiary_search: String,
    pub bestiary_type: String,
    pub bestiary_cr: (f32, f32),
    pub bestiary_preview: Option<String>,
    /// Monster being dragged from the bestiary onto the map.
    pub dragging_monster: Option<String>,
    pub custom_monster: Option<dnd_rules::monster::MonsterTemplate>,
    pub show_graph: bool,
    pub graph_link_from: Option<RoomId>,
    pub graph_drag: Option<(RoomId, egui_pos::Pos)>,
    pub script_draft: Option<dnd_session::scripts::ScriptedEvent>,
    pub script_pick: Option<ScriptPick>,
    pub xp_award: u32,
    pub container_add: String,
}

/// Small helper so state.rs doesn't depend on egui types.
pub mod egui_pos {
    #[derive(Debug, Clone, Copy, PartialEq, Default)]
    pub struct Pos(pub f32, pub f32);
}

/// The script editor is waiting for a map click to fill in a trigger/action field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptPick {
    AreaStart,
    AreaEnd,
    Object,
    SpawnCell,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            menu: MenuPage::Main,
            dm_tab: DmTab::Editor,
            tool: Tool::Play,
            layer: LayerKind::Terrain,
            asset: "stone_floor".into(),
            asset_search: String::new(),
            favorites: vec![
                "stone_floor".into(),
                "stone_wall".into(),
                "grass".into(),
                "door".into(),
                "torch".into(),
            ],
            monster: "goblin_warrior".into(),
            spawn_hidden: false,
            object_hidden: false,
            selected: None,
            targeting: None,
            dice_expr: "1d20".into(),
            dice_mode: RollMode::Normal,
            private_roll: false,
            chat: String::new(),
            show_grid: true,
            show_sheet: true,
            show_creator: false,
            new_room_name: String::new(),
            new_room_w: 20,
            new_room_h: 15,
            new_room_ambient: AmbientLight::Bright,
            connect_to: None,
            connect_one_way: false,
            light_bright: 20,
            light_dim: 20,
            rect_start: None,
            save_label: String::new(),
            reaction_attack: 0,
            surprised: vec![],
            room_desc_edit: None,
            form_campaign: String::new(),
            form_port: "7878".into(),
            form_address: "127.0.0.1:7878".into(),
            form_code: String::new(),
            form_name: String::new(),
            form_character: None,
            ui_scale: 1.0,
            casting: None,
            feature_target: None,
            loh_amount: 5,
            show_spellbook: false,
            spellbook_prepare: false,
            prep_draft: None,
            spell_search: String::new(),
            show_inventory: false,
            show_levelup: false,
            levelup: Default::default(),
            show_settings: false,
            recent_assets: vec![],
            show_collision: false,
            show_light_radii: false,
            editor_preview: false,
            map_context: None,
            show_invite: false,
            dismissed_message: 0,
            sel_objects: vec![],
            clipboard: vec![],
            drag: None,
            draw_kind: DrawKind::Stroke,
            draw_color: [200, 40, 30, 255],
            draw_width: 3.0,
            draw_dm_only: false,
            draw_temporary: true,
            draw_text: String::new(),
            draw_points: vec![],
            show_bestiary: false,
            bestiary_search: String::new(),
            bestiary_type: String::new(),
            bestiary_cr: (0.0, 30.0),
            bestiary_preview: None,
            dragging_monster: None,
            custom_monster: None,
            show_graph: false,
            graph_link_from: None,
            graph_drag: None,
            script_draft: None,
            script_pick: None,
            xp_award: 100,
            container_add: String::new(),
        }
    }
}

/// Audio volumes (0..1).
#[derive(Resource)]
pub struct AudioSettings {
    pub master: f32,
    pub effects: f32,
    pub music: f32,
    pub ambience: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        AudioSettings {
            master: 0.8,
            effects: 0.8,
            music: 0.6,
            ambience: 0.6,
        }
    }
}

/// World units per grid cell.
pub const CELL: f32 = 48.0;

pub fn cell_to_world(c: (i32, i32)) -> Vec2 {
    Vec2::new(
        c.0 as f32 * CELL + CELL / 2.0,
        -(c.1 as f32 * CELL + CELL / 2.0),
    )
}

pub fn world_to_cell(p: Vec2) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (-p.y / CELL).floor() as i32)
}
