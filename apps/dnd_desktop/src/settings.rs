//! Persistent user settings (`settings.json` in the data folder).

use crate::i18n::{I18n, Lang};
use crate::state::{AudioSettings, UiState};
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, VideoModeSelection, WindowMode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayMode {
    Windowed,
    Borderless,
    Fullscreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsBackend {
    Auto,
    Dx12,
    Vulkan,
    OpenGl,
}

impl GraphicsBackend {
    /// Value for the `WGPU_BACKEND` variable read by Bevy's renderer at startup.
    pub fn env_value(self) -> Option<&'static str> {
        match self {
            GraphicsBackend::Auto => None,
            GraphicsBackend::Dx12 => Some("dx12"),
            GraphicsBackend::Vulkan => Some("vulkan"),
            GraphicsBackend::OpenGl => Some("gl"),
        }
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSettings {
    pub language: String,
    pub ui_scale: f32,
    pub resolution: (u32, u32),
    pub display: DisplayMode,
    pub vsync: bool,
    pub backend: GraphicsBackend,
    pub master: f32,
    pub effects: f32,
    pub music: f32,
    pub ambience: f32,
    pub last_address: String,
    pub last_name: String,
    pub relay_address: String,
    pub host_port: u16,
    pub try_upnp: bool,
    pub audio_enabled: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        GameSettings {
            language: "en".into(),
            ui_scale: 1.0,
            resolution: (1600, 900),
            display: DisplayMode::Windowed,
            vsync: true,
            backend: GraphicsBackend::Auto,
            master: 0.8,
            effects: 0.8,
            music: 0.5,
            ambience: 0.6,
            last_address: "127.0.0.1:7878".into(),
            last_name: String::new(),
            relay_address: String::new(),
            host_port: 7878,
            try_upnp: false,
            audio_enabled: true,
        }
    }
}

pub const RESOLUTIONS: [(u32, u32); 6] = [
    (1280, 720),
    (1366, 768),
    (1600, 900),
    (1920, 1080),
    (2560, 1440),
    (3840, 2160),
];

impl GameSettings {
    pub fn path() -> std::path::PathBuf {
        crate::platform::data_dir().join("settings.json")
    }

    /// Loads settings; a missing or corrupt file falls back to defaults (never blocks startup).
    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str::<GameSettings>(&s).ok())
            .unwrap_or_default()
            .sanitized()
    }

    pub fn sanitized(mut self) -> Self {
        self.ui_scale = self.ui_scale.clamp(0.75, 1.75);
        self.resolution = (
            self.resolution.0.clamp(800, 7680),
            self.resolution.1.clamp(600, 4320),
        );
        for v in [
            &mut self.master,
            &mut self.effects,
            &mut self.music,
            &mut self.ambience,
        ] {
            *v = v.clamp(0.0, 1.0);
        }
        if self.host_port == 0 {
            self.host_port = 7878;
        }
        self
    }

    /// Atomic save: write a temp file then rename, so a crash never leaves a half-written file.
    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(tmp, path)
    }

    pub fn window_mode(&self) -> WindowMode {
        match self.display {
            DisplayMode::Windowed => WindowMode::Windowed,
            DisplayMode::Borderless => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            DisplayMode::Fullscreen => {
                WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
            }
        }
    }

    pub fn present_mode(&self) -> PresentMode {
        if self.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        }
    }
}

/// Applies changed settings to the window, UI, audio and language, and saves them.
pub fn apply_settings(
    settings: Res<GameSettings>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    mut ui: ResMut<UiState>,
    mut audio: ResMut<AudioSettings>,
    mut i18n: ResMut<I18n>,
    mut first: Local<bool>,
) {
    if !settings.is_changed() && *first {
        return;
    }
    let initial = !*first;
    *first = true;
    if let Ok(mut w) = window.single_mut() {
        if w.mode != settings.window_mode() {
            w.mode = settings.window_mode();
        }
        if w.present_mode != settings.present_mode() {
            w.present_mode = settings.present_mode();
        }
        if settings.display == DisplayMode::Windowed {
            let (rw, rh) = settings.resolution;
            if (w.resolution.width() as u32, w.resolution.height() as u32) != (rw, rh) {
                w.resolution.set(rw as f32, rh as f32);
            }
        }
    }
    ui.ui_scale = settings.ui_scale;
    *audio = AudioSettings {
        master: settings.master,
        effects: settings.effects,
        music: settings.music,
        ambience: settings.ambience,
    };
    i18n.lang = if settings.language == "it" {
        Lang::It
    } else {
        Lang::En
    };
    if !initial && let Err(e) = settings.save() {
        warn!("could not save settings: {e}");
    }
}

/// The settings form (main menu and in-game). Edits apply immediately and are saved.
pub fn settings_ui(ui: &mut bevy_egui::egui::Ui, s: &mut GameSettings, i18n: &I18n) {
    use bevy_egui::egui;
    let mut v = s.clone();
    egui::Grid::new("settings-grid")
        .num_columns(2)
        .spacing([16.0, 8.0])
        .show(ui, |ui| {
            ui.label(i18n.t("settings.language"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut v.language, "en".to_string(), "English");
                ui.selectable_value(&mut v.language, "it".to_string(), "Italiano");
            });
            ui.end_row();
            ui.label(i18n.t("settings.display"));
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut v.display,
                    DisplayMode::Windowed,
                    i18n.t("settings.windowed"),
                );
                ui.selectable_value(
                    &mut v.display,
                    DisplayMode::Borderless,
                    i18n.t("settings.borderless"),
                );
                ui.selectable_value(
                    &mut v.display,
                    DisplayMode::Fullscreen,
                    i18n.t("settings.fullscreen"),
                );
            });
            ui.end_row();
            ui.label(i18n.t("settings.resolution"));
            egui::ComboBox::from_id_salt("res")
                .selected_text(format!("{} × {}", v.resolution.0, v.resolution.1))
                .show_ui(ui, |ui| {
                    for r in RESOLUTIONS {
                        ui.selectable_value(&mut v.resolution, r, format!("{} × {}", r.0, r.1));
                    }
                });
            ui.end_row();
            ui.label(i18n.t("settings.vsync"));
            ui.checkbox(&mut v.vsync, i18n.t("settings.vsync_help"));
            ui.end_row();
            ui.label(i18n.t("settings.backend"));
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut v.backend,
                    GraphicsBackend::Auto,
                    i18n.t("settings.auto"),
                );
                ui.selectable_value(&mut v.backend, GraphicsBackend::Dx12, "DirectX 12");
                ui.selectable_value(&mut v.backend, GraphicsBackend::Vulkan, "Vulkan");
                ui.selectable_value(&mut v.backend, GraphicsBackend::OpenGl, "OpenGL");
            });
            ui.end_row();
            ui.label(i18n.t("settings.ui_scale"));
            ui.add(egui::Slider::new(&mut v.ui_scale, 0.75..=1.75));
            ui.end_row();
            for (label, val) in [
                ("settings.master", &mut v.master),
                ("settings.effects", &mut v.effects),
                ("settings.music", &mut v.music),
                ("settings.ambience", &mut v.ambience),
            ] {
                ui.label(i18n.t(label));
                ui.add(egui::Slider::new(val, 0.0..=1.0).show_value(false));
                ui.end_row();
            }
        });
    ui.checkbox(&mut v.audio_enabled, i18n.t("settings.audio_enabled"));
    if v.backend != s.backend || v.audio_enabled != s.audio_enabled {
        ui.label(egui::RichText::new(i18n.t("settings.restart_needed")).italics());
    }
    ui.label(format!(
        "{}: {}",
        i18n.t("settings.data_dir"),
        crate::platform::data_dir().display()
    ));
    ui.horizontal(|ui| {
        if ui.button(i18n.t("settings.open_data")).clicked() {
            let _ = std::fs::create_dir_all(crate::platform::data_dir());
            open_folder(&crate::platform::data_dir());
        }
        if ui.button(i18n.t("settings.reset")).clicked() {
            v = GameSettings {
                last_address: v.last_address.clone(),
                last_name: v.last_name.clone(),
                ..GameSettings::default()
            };
        }
    });
    if v != *s {
        *s = v;
    }
}

/// Opens a folder in Explorer (Windows) / the file manager.
pub fn open_folder(path: &std::path::Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_values_are_sanitized() {
        let s = GameSettings {
            ui_scale: 9.0,
            master: -1.0,
            resolution: (10, 10),
            host_port: 0,
            ..Default::default()
        }
        .sanitized();
        assert_eq!(s.ui_scale, 1.75);
        assert_eq!(s.master, 0.0);
        assert_eq!(s.resolution, (800, 600));
        assert_eq!(s.host_port, 7878);
    }

    #[test]
    fn unknown_or_missing_fields_use_defaults() {
        let s: GameSettings =
            serde_json::from_str(r#"{"language":"it","new_field_from_future":1}"#).unwrap();
        assert_eq!(s.language, "it");
        assert_eq!(s.resolution, (1600, 900));
    }
}
