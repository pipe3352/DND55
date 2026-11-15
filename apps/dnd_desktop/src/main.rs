//! D&D 5.5 — desktop RPG client and DM host.
//!
//! The DM's copy runs the authoritative host in-process (see `dnd_net`) and connects to it like any
//! player; all game rules live in `dnd_rules`/`dnd_session`. This crate only renders and collects input.

// Bevy systems naturally take many params and complex query types.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]
// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod audio;
mod autotest;
mod character_ui;
mod fx;
mod hosting;
mod i18n;
mod input;
mod map_render;
mod menu;
mod net_sync;
mod state;
mod textures;
mod theme;
mod tokens;
mod ui_game;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use state::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "D&D 5.5 — Tabletop RPG".into(),
                resolution: WindowResolution::new(1600, 900),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.07, 0.055, 0.045)))
        .init_state::<AppState>()
        .init_resource::<Net>()
        .init_resource::<GameView>()
        .init_resource::<UiState>()
        .init_resource::<AudioSettings>()
        .init_resource::<i18n::I18n>()
        .init_resource::<textures::TextureCache>()
        .init_resource::<tokens::TokenAnims>()
        .init_resource::<fx::Presentation>()
        .init_resource::<input::Hover>()
        .init_resource::<menu::MenuCache>()
        .init_resource::<character_ui::CharacterDraft>()
        .init_resource::<audio::SfxBank>()
        .insert_resource(map_render::CatalogRes(
            dnd_world::catalog::Catalog::builtin(),
        ))
        .init_resource::<autotest::AutoState>()
        .add_systems(
            Startup,
            (setup_camera, audio::build_bank, autotest::auto_start),
        )
        .add_systems(Update, autotest::auto_script.after(net_sync::poll_network))
        .add_systems(
            EguiPrimaryContextPass,
            menu::menu_ui.run_if(in_state(AppState::MainMenu)),
        )
        .add_systems(
            EguiPrimaryContextPass,
            ui_game::game_ui.run_if(in_state(AppState::InGame)),
        )
        .add_systems(Update, net_sync::poll_network)
        .add_systems(
            Update,
            (
                (
                    map_render::rebuild_map,
                    map_render::update_fog_and_lights,
                    tokens::sync_tokens,
                )
                    .chain(),
                fx::process_events,
                (
                    tokens::animate_tokens,
                    fx::animate_fx,
                    map_render::flicker_lights,
                ),
                (input::camera_controls, input::track_hover, input::map_click).chain(),
                input::draw_overlays,
            )
                .chain()
                .after(net_sync::poll_network)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(OnExit(AppState::InGame), cleanup_world)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Transform::from_xyz(400.0, -300.0, 0.0)));
}

fn cleanup_world(
    mut commands: Commands,
    q: Query<
        Entity,
        Or<(
            With<map_render::MapSprite>,
            With<map_render::FogCell>,
            With<map_render::LightGlow>,
            With<tokens::Token>,
            With<fx::FloatText>,
        )>,
    >,
    mut anims: ResMut<tokens::TokenAnims>,
) {
    for e in &q {
        commands.entity(e).despawn();
    }
    *anims = tokens::TokenAnims::default();
}
