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
}

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
