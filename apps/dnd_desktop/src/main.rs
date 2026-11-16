//! DND55 — desktop fantasy RPG client and DM host (5e 2024 / SRD 5.2.1 rules).
//!
//! The DM's copy runs the authoritative host in-process (see `dnd_net`) and connects to it like any
//! player; all game rules live in `dnd_rules`/`dnd_session`. This crate only renders and collects input.

// Bevy systems naturally take many params and complex query types.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]
// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod asset_packs;
mod audio;
mod autotest;
mod char_manager;
mod character_ui;
mod editor_ui;
mod fx;
mod graph_editor;
mod hosting;
mod i18n;
mod icons;
mod input;
mod liquid;
mod map_render;
mod menu;
mod menu_scene;
mod net_sync;
mod platform;
mod pointer;
mod settings;
mod state;
mod terrain_art;
mod textures;
mod theme;
mod tokens;
mod ui_dm;
mod ui_game;
mod ui_play;
mod vfx;
mod weather;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use state::*;

fn main() {
    platform::install_crash_handler();
    let game_settings = settings::GameSettings::load();
    if let Some(backend) = game_settings.backend.env_value()
        && std::env::var("WGPU_BACKEND").is_err()
    {
        // SAFETY: called before any other thread exists.
        unsafe { std::env::set_var("WGPU_BACKEND", backend) };
    }
    let (w, h) = game_settings.resolution;
    let audio_ok = game_settings.audio_enabled && platform::audio_available();
    let mut plugins = DefaultPlugins.build();
    if !audio_ok {
        plugins = plugins.disable::<bevy::audio::AudioPlugin>();
    }
    App::new()
        .insert_resource(audio::AudioStatus {
            available: audio_ok,
        })
        .add_plugins(
            plugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "DND55 — Fantasy Tabletop RPG".into(),
                        resolution: WindowResolution::new(w, h),
                        mode: game_settings.window_mode(),
                        present_mode: game_settings.present_mode(),
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    custom_layer: platform::file_log_layer,
                    ..default()
                }),
        )
        .insert_resource(game_settings)
        .init_resource::<liquid::LiquidSettings>()
        .add_plugins(liquid::LiquidPlugin)
        .add_systems(
            Update,
            (settings::apply_settings, platform::set_window_icon),
        )
        .add_systems(Last, settings::frame_limiter)
        .add_plugins(EguiPlugin::default())
        .insert_resource(ClearColor(Color::srgb_u8(
            theme::BG.r(),
            theme::BG.g(),
            theme::BG.b(),
        )))
        .init_state::<AppState>()
        .init_resource::<Net>()
        .init_resource::<GameView>()
        .init_resource::<UiState>()
        .init_resource::<AudioSettings>()
        .init_resource::<i18n::I18n>()
        .init_resource::<textures::TextureCache>()
        .init_resource::<map_render::ArtCache>()
        .init_resource::<weather::WeatherSprites>()
        .init_resource::<vfx::CameraShake>()
        .init_resource::<tokens::TokenAnims>()
        .init_resource::<tokens::TokenArt>()
        .init_resource::<fx::Presentation>()
        .init_resource::<input::Hover>()
        .init_resource::<pointer::MapPointer>()
        .init_resource::<pointer::MapPointer>()
        .init_resource::<input::CameraFit>()
        .init_resource::<graph_editor::GraphEditor>()
        .init_resource::<editor_ui::PaletteThumbs>()
        .init_resource::<menu::MenuCache>()
        .init_resource::<character_ui::CharacterDraft>()
        .init_resource::<audio::SfxBank>()
        .init_resource::<audio::Soundscape>()
        .add_systems(Update, audio::update_soundscape)
        .insert_resource(map_render::CatalogRes(
            dnd_world::catalog::Catalog::builtin(),
        ))
        .init_resource::<autotest::AutoState>()
        .add_systems(
            Startup,
            (
                setup_camera,
                audio::build_bank,
                autotest::auto_start,
                vfx::setup_fx,
            ),
        )
        .add_systems(Update, autotest::auto_script.after(net_sync::poll_network))
        .add_systems(Update, autotest::log_camera)
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
                weather::update_weather,
                vfx::animate_particles,
                (
                    tokens::animate_tokens,
                    tokens::token_indicators,
                    fx::animate_fx,
                    map_render::flicker_lights,
                ),
                (
                    vfx::apply_shake,
                    input::camera_controls,
                    input::track_hover,
                    input::editor_keys,
                    input::map_click,
                )
                    .chain(),
                (
                    input::draw_overlays,
                    input::sync_drawing_labels,
                    map_render::placement_ghost,
                ),
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
            With<weather::Particle>,
            With<vfx::FxParticle>,
            With<map_render::PlacementGhost>,
            With<input::DrawingLabel>,
        )>,
    >,
    mut anims: ResMut<tokens::TokenAnims>,
) {
    for e in &q {
        commands.entity(e).despawn();
    }
    *anims = tokens::TokenAnims::default();
}
