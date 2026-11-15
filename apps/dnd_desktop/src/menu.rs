//! Main menu: Play (quick demo), Create, Join, Load, Character Manager, Settings, Exit.

use crate::character_ui::{CharacterDraft, creator_ui};
use crate::hosting::{self, open_store};
use crate::i18n::I18n;
use crate::state::{HostRuntime, MenuPage, Net, UiState};
use crate::theme;
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use bevy_egui::{EguiContexts, egui::Ui};
use dnd_session::Session;
use dnd_session::demo::demo_campaign;

#[derive(Resource, Default)]
pub struct MenuCache {
    pub campaigns: Vec<dnd_persist::CampaignInfo>,
    pub characters: Vec<dnd_persist::StoredCharacter>,
    pub error: String,
    pub refreshed: bool,
}

impl MenuCache {
    pub fn refresh(&mut self) {
        match open_store() {
            Ok(s) => {
                self.campaigns = s.list_campaigns().unwrap_or_default();
                self.characters = s.list_characters().unwrap_or_default();
            }
            Err(e) => self.error = e,
        }
        self.refreshed = true;
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct MenuParams<'w> {
    pub ui: ResMut<'w, UiState>,
    pub net: ResMut<'w, Net>,
    pub cache: ResMut<'w, MenuCache>,
    pub draft: ResMut<'w, CharacterDraft>,
    pub i18n: ResMut<'w, I18n>,
    pub settings: ResMut<'w, crate::settings::GameSettings>,
    pub audio: Option<Res<'w, crate::audio::AudioStatus>>,
}

pub fn menu_ui(
    mut contexts: EguiContexts,
    mut p: MenuParams,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut themed: Local<Option<f32>>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    if *themed != Some(p.ui.ui_scale) {
        theme::apply(&ctx, p.ui.ui_scale);
        *themed = Some(p.ui.ui_scale);
    }
    if !p.cache.refreshed {
        p.cache.refresh();
    }
    // Dark backdrop with a title.
    let mut root = Ui::new(
        ctx.clone(),
        "menu-root".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(egui::Color32::from_rgb(20, 14, 10)))
        .show(&mut root, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new(p.i18n.t("app.title"))
                        .size(44.0)
                        .color(theme::GOLD)
                        .strong(),
                );
                ui.label(
                    RichText::new(p.i18n.t("app.subtitle"))
                        .size(16.0)
                        .color(theme::PARCHMENT_DARK)
                        .italics(),
                );
            });
        });

    let i18n_lang = p.i18n.lang;
    egui::Window::new(p.i18n.t("menu.title"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 40.0))
        .resizable(false)
        .collapsible(false)
        .min_width(if p.ui.menu == MenuPage::Characters {
            900.0
        } else {
            420.0
        })
        .show(&ctx, |ui| {
            let page = p.ui.menu;
            match page {
                MenuPage::Main => main_page(ui, &mut p, &mut commands, &mut exit),
                MenuPage::Create => create_page(ui, &mut p, &mut commands),
                MenuPage::Join => join_page(ui, &mut p),
                MenuPage::Load => load_page(ui, &mut p, &mut commands),
                MenuPage::Characters => characters_page(ui, &mut p),
                MenuPage::Settings => settings_page(ui, &mut p),
            }
            if page != MenuPage::Main {
                ui.separator();
                if ui.button(p.i18n.t("menu.back")).clicked() {
                    p.ui.menu = MenuPage::Main;
                }
            }
            if !p.net.status.is_empty() {
                ui.label(RichText::new(&p.net.status).italics());
            }
            if !p.cache.error.is_empty() {
                ui.label(RichText::new(&p.cache.error).color(theme::BLOOD));
            }
        });
    let _ = i18n_lang;
    egui::Area::new("attribution".into())
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -8.0))
        .show(&ctx, |ui| {
            ui.label(
                RichText::new(dnd_rules::SRD_ATTRIBUTION)
                    .size(10.0)
                    .color(theme::PARCHMENT_DARK),
            );
        });
    Ok(())
}

fn big_button(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add_sized(
        [360.0, 38.0],
        egui::Button::new(RichText::new(text).size(18.0)),
    )
    .clicked()
}

fn main_page(
    ui: &mut egui::Ui,
    p: &mut MenuParams,
    commands: &mut Commands,
    exit: &mut MessageWriter<AppExit>,
) {
    ui.vertical_centered(|ui| {
        if big_button(ui, p.i18n.t("menu.play")) {
            start_hosting(p, commands, demo_campaign(), None);
        }
        if big_button(ui, p.i18n.t("menu.create")) {
            p.ui.menu = MenuPage::Create;
        }
        if big_button(ui, p.i18n.t("menu.join")) {
            p.ui.menu = MenuPage::Join;
        }
        if big_button(ui, p.i18n.t("menu.load")) {
            p.cache.refresh();
            p.ui.menu = MenuPage::Load;
        }
        if big_button(ui, p.i18n.t("menu.characters")) {
            p.cache.refresh();
            p.ui.menu = MenuPage::Characters;
        }
        if big_button(ui, p.i18n.t("menu.settings")) {
            p.ui.menu = MenuPage::Settings;
        }
        if big_button(ui, p.i18n.t("menu.exit")) {
            exit.write(AppExit::Success);
        }
    });
}

fn start_hosting(
    p: &mut MenuParams,
    commands: &mut Commands,
    session: Session,
    existing: Option<i64>,
) {
    let port: u16 = p.ui.form_port.trim().parse().unwrap_or(7878);
    let campaign_id = match existing {
        Some(id) => id,
        None => match open_store().and_then(|s| {
            s.create_campaign(&session.state.campaign_name)
                .map_err(|e| e.to_string())
        }) {
            Ok(id) => id,
            Err(e) => {
                p.cache.error = e;
                return;
            }
        },
    };
    match hosting::host_session(session, campaign_id, port, &mut p.net) {
        Ok(host) => commands.insert_resource::<HostRuntime>(host),
        Err(e) => p.cache.error = e,
    }
}

fn create_page(ui: &mut egui::Ui, p: &mut MenuParams, commands: &mut Commands) {
    theme::heading(ui, p.i18n.t("menu.create"));
    egui::Grid::new("create").num_columns(2).show(ui, |ui| {
        ui.label(p.i18n.t("form.campaign_name"));
        ui.text_edit_singleline(&mut p.ui.form_campaign);
        ui.end_row();
        ui.label(p.i18n.t("form.port"));
        ui.text_edit_singleline(&mut p.ui.form_port);
        ui.end_row();
    });
    ui.label(RichText::new(p.i18n.t("form.port_help")).small());
    if ui.button(p.i18n.t("form.start_hosting")).clicked() {
        let name = if p.ui.form_campaign.trim().is_empty() {
            "New Campaign".to_string()
        } else {
            p.ui.form_campaign.trim().to_string()
        };
        start_hosting(p, commands, Session::new_campaign(&name), None);
    }
}

fn join_page(ui: &mut egui::Ui, p: &mut MenuParams) {
    theme::heading(ui, p.i18n.t("menu.join"));
    egui::Grid::new("join").num_columns(2).show(ui, |ui| {
        ui.label(p.i18n.t("form.address"));
        ui.text_edit_singleline(&mut p.ui.form_address);
        ui.end_row();
        ui.label(p.i18n.t("form.code"));
        ui.text_edit_singleline(&mut p.ui.form_code);
        ui.end_row();
        ui.label(p.i18n.t("form.player_name"));
        ui.text_edit_singleline(&mut p.ui.form_name);
        ui.end_row();
        ui.label(p.i18n.t("form.character"));
        let current =
            p.ui.form_character
                .and_then(|id| p.cache.characters.iter().find(|c| c.id == id))
                .map(|c| c.name.clone())
                .unwrap_or_else(|| p.i18n.t("form.create_later").to_string());
        egui::ComboBox::from_id_salt("join-char")
            .selected_text(current)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut p.ui.form_character,
                    None,
                    p.i18n.t("form.create_later"),
                );
                for c in &p.cache.characters {
                    ui.selectable_value(
                        &mut p.ui.form_character,
                        Some(c.id),
                        format!(
                            "{} ({} {})",
                            c.name,
                            c.sheet.class_def().name,
                            c.sheet.level
                        ),
                    );
                }
            });
        ui.end_row();
    });
    ui.label(RichText::new(p.i18n.t("form.join_help")).small());
    let is_invite = dnd_net::Invite::parse(&p.ui.form_address).is_some();
    if is_invite {
        ui.label(RichText::new(p.i18n.t("form.invite_ok")).color(theme::HEALTH));
    }
    let ready = !p.ui.form_name.trim().is_empty()
        && (is_invite || !p.ui.form_code.trim().is_empty())
        && !p.ui.form_address.trim().is_empty();
    if ui
        .add_enabled(ready, egui::Button::new(p.i18n.t("form.connect")))
        .clicked()
    {
        p.net.pending_character =
            p.ui.form_character
                .and_then(|id| p.cache.characters.iter().find(|c| c.id == id))
                .map(|c| (c.sheet.clone(), c.portrait_png.clone()));
        let (a, c, n) = (
            p.ui.form_address.clone(),
            p.ui.form_code.clone(),
            p.ui.form_name.clone(),
        );
        if let Err(e) = hosting::join(&mut p.net, &a, &c, &n) {
            p.cache.error = e;
        }
    }
}

fn load_page(ui: &mut egui::Ui, p: &mut MenuParams, commands: &mut Commands) {
    theme::heading(ui, p.i18n.t("menu.load"));
    if p.cache.campaigns.is_empty() {
        ui.label(p.i18n.t("load.none"));
    }
    let mut chosen = None;
    egui::ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            for c in &p.cache.campaigns {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&c.name).strong());
                    if ui.button(p.i18n.t("load.host")).clicked() {
                        chosen = Some(c.id);
                    }
                });
            }
        });
    if let Some(id) = chosen {
        match open_store().and_then(|s| s.load_latest_valid(id).map_err(|e| e.to_string())) {
            Ok((info, state, assets)) => {
                p.net.status = format!("{} — {}", p.i18n.t("load.loaded"), info.label);
                let session = Session::from_state(
                    state,
                    assets,
                    Box::new(dnd_rules::rng::GameRng::from_entropy()),
                );
                start_hosting(p, commands, session, Some(id));
            }
            Err(e) => p.cache.error = e,
        }
    }
}

fn characters_page(ui: &mut egui::Ui, p: &mut MenuParams) {
    theme::heading(ui, p.i18n.t("menu.characters"));
    ui.horizontal(|ui| {
        ui.label(p.i18n.t("chars.library"));
        if ui.button(p.i18n.t("chars.new")).clicked() {
            *p.draft = CharacterDraft::default();
        }
        let mut load = None;
        for c in &p.cache.characters {
            if ui.button(&c.name).clicked() {
                load = Some(c.clone());
            }
        }
        if let Some(c) = load {
            p.draft.sheet = c.sheet;
            p.draft.portrait_tex = None;
            p.draft.portrait = c.portrait_png;
            p.draft.library_id = Some(c.id);
        }
    });
    ui.separator();
    if creator_ui(ui, &mut p.draft, &p.i18n) {
        match open_store().and_then(|s| {
            s.save_character(
                p.draft.library_id,
                &p.draft.sheet,
                p.draft.portrait.as_deref(),
            )
            .map_err(|e| e.to_string())
        }) {
            Ok(id) => {
                p.draft.library_id = Some(id);
                p.draft.message = p.i18n.t("chars.saved").into();
                p.cache.refresh();
            }
            Err(e) => p.draft.message = e,
        }
    }
}

fn settings_page(ui: &mut egui::Ui, p: &mut MenuParams) {
    theme::heading(ui, p.i18n.t("menu.settings"));
    if p.audio.as_ref().is_some_and(|a| !a.available) {
        ui.label(egui::RichText::new(p.i18n.t("settings.no_audio_device")).color(theme::BLOOD));
    }
    crate::settings::settings_ui(ui, &mut p.settings, &p.i18n);
}
