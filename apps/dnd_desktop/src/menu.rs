//! Main menu: Play (quick demo), Create, Join, Load, Character Manager, Settings, Exit.

use crate::character_ui::CharacterDraft;
use crate::hosting::{self, open_store};
use crate::i18n::I18n;
use crate::state::{HostRuntime, MenuPage, Net, UiState};
use crate::theme;
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy_egui::EguiContexts;
use bevy_egui::egui::{self, RichText};
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
    mut themed: Local<Option<(f32, bool)>>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    theme::set_reduced_motion(&ctx, p.settings.reduced_motion);
    if *themed != Some((p.ui.ui_scale, p.settings.reduced_motion)) {
        theme::apply(&ctx, p.ui.ui_scale);
        *themed = Some((p.ui.ui_scale, p.settings.reduced_motion));
        // Named font families become usable from the next frame.
        ctx.request_repaint();
        return Ok(());
    }
    if !p.cache.refreshed {
        p.cache.refresh();
    }
    // Animated painted backdrop (still when Reduced Motion is on).
    let reduced = theme::reduced_motion(&ctx);
    let t = if reduced {
        12.0
    } else {
        ctx.input(|i| i.time) as f32
    };
    if !reduced {
        ctx.request_repaint();
    }
    crate::menu_scene::paint(
        &ctx.layer_painter(egui::LayerId::background()),
        ctx.viewport_rect(),
        t,
    );

    // Left column: Blackletter title and the illustrated menu plaques.
    let page = p.ui.menu;
    egui::Area::new("menu-left".into())
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(64.0, 48.0))
        .show(&ctx, |ui| {
            theme::title(ui, p.i18n.t("app.title"), 66.0, theme::GOLD_BRIGHT);
            ui.label(
                theme::italic(p.i18n.t("app.subtitle"))
                    .size(18.0)
                    .color(theme::PARCHMENT),
            );
            ui.add_space(26.0);
            main_page(ui, &mut p, &mut commands, &mut exit);
        });

    // Sub-pages open as a parchment window on the right, fading in.
    if page == MenuPage::Characters {
        crate::char_manager::window(&ctx, &mut p);
    } else if page != MenuPage::Main {
        let fade = if reduced {
            1.0
        } else {
            ctx.animate_bool_with_time(egui::Id::new(("menu-page", page as u8)), true, 0.25)
        };
        theme::win(page_title(&p, page))
            .id(egui::Id::new("menu-page"))
            .anchor(
                egui::Align2::RIGHT_CENTER,
                egui::vec2(-48.0 + (1.0 - fade) * 40.0, 10.0),
            )
            .resizable(false)
            .collapsible(false)
            .min_width(440.0)
            .max_height(ctx.viewport_rect().height() - 120.0)
            .show(&ctx, |ui| {
                theme::window_bg(ui);
                ui.set_opacity(fade);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    match page {
                        MenuPage::Main => {}
                        MenuPage::Create => create_page(ui, &mut p, &mut commands),
                        MenuPage::Join => join_page(ui, &mut p),
                        MenuPage::Load => load_page(ui, &mut p, &mut commands),
                        MenuPage::Characters => {}
                        MenuPage::Settings => settings_page(ui, &mut p),
                    }
                    ui.separator();
                    if ui.button(p.i18n.t("menu.back")).clicked() {
                        p.ui.menu = MenuPage::Main;
                    }
                });
            });
    }
    // Status and errors as a banner-like strip at the bottom of the menu column.
    if !p.net.status.is_empty() || !p.cache.error.is_empty() {
        egui::Area::new("menu-status".into())
            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(64.0, -40.0))
            .show(&ctx, |ui| {
                ui.set_max_width(520.0);
                if !p.net.status.is_empty() {
                    ui.label(theme::italic(&p.net.status).color(theme::PARCHMENT));
                }
                if !p.cache.error.is_empty() {
                    ui.label(
                        theme::bold(&p.cache.error).color(egui::Color32::from_rgb(230, 120, 100)),
                    );
                }
            });
    }
    egui::Area::new("attribution".into())
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -8.0))
        .show(&ctx, |ui| {
            ui.label(
                RichText::new(dnd_rules::SRD_ATTRIBUTION)
                    .size(11.0)
                    .color(theme::PARCHMENT_DARK.gamma_multiply(0.8)),
            );
        });
    Ok(())
}

fn page_title(p: &MenuParams, page: MenuPage) -> String {
    p.i18n
        .t(match page {
            MenuPage::Main => "menu.title",
            MenuPage::Create => "menu.create",
            MenuPage::Join => "menu.join",
            MenuPage::Load => "menu.load",
            MenuPage::Characters => "menu.characters",
            MenuPage::Settings => "menu.settings",
        })
        .to_string()
}

/// Loads the latest valid save of a campaign and hosts it.
fn load_campaign(p: &mut MenuParams, commands: &mut Commands, id: i64) {
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

fn main_page(
    ui: &mut egui::Ui,
    p: &mut MenuParams,
    commands: &mut Commands,
    exit: &mut MessageWriter<AppExit>,
) {
    use crate::icons::Icon;
    let w = 360.0;
    ui.spacing_mut().item_spacing.y = 10.0;
    // Continue the most recently played campaign.
    if let Some(last) = p
        .cache
        .campaigns
        .iter()
        .max_by_key(|c| c.updated_at)
        .cloned()
    {
        let label = format!("{} — {}", p.i18n.t("menu.continue"), last.name);
        if theme::plaque_button(ui, Icon::Book, &label, w).clicked() {
            load_campaign(p, commands, last.id);
        }
    }
    let items: [(Icon, &str, Option<MenuPage>); 7] = [
        (Icon::D20, "menu.play", None),
        (Icon::Quill, "menu.create", Some(MenuPage::Create)),
        (Icon::Banner, "menu.join", Some(MenuPage::Join)),
        (Icon::Chest, "menu.load", Some(MenuPage::Load)),
        (Icon::Helm, "menu.characters", Some(MenuPage::Characters)),
        (Icon::Gear, "menu.settings", Some(MenuPage::Settings)),
        (Icon::Door, "menu.exit", None),
    ];
    for (icon, key, target) in items {
        if theme::plaque_button(ui, icon, p.i18n.t(key), w).clicked() {
            match (key, target) {
                ("menu.play", _) => start_hosting(p, commands, demo_campaign(), None),
                ("menu.exit", _) => {
                    exit.write(AppExit::Success);
                }
                (_, Some(page)) => {
                    if matches!(page, MenuPage::Load | MenuPage::Characters) {
                        p.cache.refresh();
                    }
                    p.ui.menu = if p.ui.menu == page {
                        MenuPage::Main
                    } else {
                        page
                    };
                }
                _ => {}
            }
        }
    }
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
        load_campaign(p, commands, id);
    }
}

fn settings_page(ui: &mut egui::Ui, p: &mut MenuParams) {
    theme::heading(ui, p.i18n.t("menu.settings"));
    if p.audio.as_ref().is_some_and(|a| !a.available) {
        ui.label(egui::RichText::new(p.i18n.t("settings.no_audio_device")).color(theme::BLOOD));
    }
    crate::settings::settings_ui(ui, &mut p.settings, &p.i18n);
}
