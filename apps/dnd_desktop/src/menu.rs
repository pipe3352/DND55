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
    let vp = ctx.viewport_rect();
    {
        // Hanging banner on the left edge and the candle on its books (bottom-left).
        let bg = ctx.layer_painter(egui::LayerId::background());
        crate::chrome::banner(
            &bg,
            egui::Rect::from_min_size(egui::pos2(22.0, 0.0), egui::vec2(82.0, 270.0)),
            crate::icons::Icon::Sword,
        );
        crate::chrome::candle(&bg, egui::pos2(78.0, vp.bottom() - 66.0), 0.95, t);
    }

    // Left column: dragon crest, Blackletter title, subtitle, divider and the menu plaques.
    let page = p.ui.menu;
    egui::Area::new("menu-left".into())
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(112.0, 22.0))
        .show(&ctx, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(124.0, 112.0), egui::Sense::hover());
                crate::chrome::dragon(ui.painter(), r);
                ui.vertical(|ui| {
                    ui.add_space(14.0);
                    theme::title(ui, p.i18n.t("app.title"), 62.0, theme::GOLD_BRIGHT);
                    ui.label(
                        theme::italic(p.i18n.t("app.subtitle"))
                            .size(19.0)
                            .color(theme::PARCHMENT),
                    );
                });
            });
            let (r, _) = ui.allocate_exact_size(egui::vec2(600.0, 18.0), egui::Sense::hover());
            let pnt = ui.painter();
            pnt.hline(
                r.x_range(),
                r.center().y,
                egui::Stroke::new(1.0, theme::GOLD.gamma_multiply(0.7)),
            );
            crate::chrome::diamond(pnt, egui::pos2(r.left() + 280.0, r.center().y), 5.0);
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.add_space(42.0);
                ui.vertical(|ui| main_page(ui, &mut p, &mut commands, &mut exit));
            });
        });
    top_right(&ctx, &mut p);
    bottom_bar(&ctx, &mut p);
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
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -50.0))
        .show(&ctx, |ui| {
            ui.label(
                RichText::new(dnd_rules::SRD_ATTRIBUTION)
                    .size(11.0)
                    .color(theme::PARCHMENT_DARK.gamma_multiply(0.8)),
            );
        });
    Ok(())
}

/// Top-right cluster: session status, player name, settings, help.
fn top_right(ctx: &egui::Context, p: &mut MenuParams) {
    use crate::chrome;
    use crate::icons::Icon;
    egui::Area::new("menu-top-right".into())
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-36.0, 24.0))
        .show(ctx, |ui| {
            theme::dark_scope(ui);
            ui.horizontal(|ui| {
                let active = p.net.client.is_some();
                chrome::pill(
                    ui,
                    if active {
                        theme::HEALTH.gamma_multiply(1.6)
                    } else {
                        theme::SLATE
                    },
                    p.i18n.t(if active {
                        "menu.session_active"
                    } else {
                        "menu.offline"
                    }),
                );
                let name = if p.settings.last_name.trim().is_empty() {
                    p.i18n.t("menu.adventurer").to_string()
                } else {
                    p.settings.last_name.clone()
                };
                let r = chrome::bar_button(ui, Some(Icon::Helm), &format!("{name} ⏷"), false);
                egui::Popup::from_toggle_button_response(&r).show(|ui| {
                    theme::dark_scope(ui);
                    ui.label(p.i18n.t("menu.player_name"));
                    ui.text_edit_singleline(&mut p.settings.last_name);
                });
                if chrome::bar_button(ui, Some(Icon::Gear), "", false)
                    .on_hover_text(p.i18n.t("menu.settings"))
                    .clicked()
                {
                    p.ui.menu = if p.ui.menu == MenuPage::Settings {
                        MenuPage::Main
                    } else {
                        MenuPage::Settings
                    };
                }
                if chrome::bar_button(ui, None, "?", false)
                    .on_hover_text(p.i18n.t("menu.help"))
                    .clicked()
                {
                    p.ui.menu_info = if p.ui.menu_info == 1 { 0 } else { 1 };
                }
            });
        });
    // Help / licenses window.
    if p.ui.menu_info != 0 {
        let mut open = true;
        let help = p.ui.menu_info == 1;
        theme::win(p.i18n.t(if help { "menu.help" } else { "menu.licenses" }))
            .open(&mut open)
            .collapsible(false)
            .default_width(520.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                theme::window_bg(ui);
                egui::ScrollArea::vertical()
                    .max_height(420.0)
                    .show(ui, |ui| {
                        if help {
                            ui.label(p.i18n.t("menu.help_text"));
                        } else {
                            ui.label(RichText::new(dnd_rules::SRD_ATTRIBUTION).small());
                            ui.add_space(6.0);
                            ui.label(p.i18n.t("menu.licenses_text"));
                        }
                    });
            });
        if !open {
            p.ui.menu_info = 0;
        }
    }
}

/// Bottom strip: version, tagline and links.
fn bottom_bar(ctx: &egui::Context, p: &mut MenuParams) {
    let vp = ctx.viewport_rect();
    let bar = egui::Rect::from_min_max(egui::pos2(vp.left(), vp.bottom() - 40.0), vp.max);
    let bg = ctx.layer_painter(egui::LayerId::background());
    bg.rect_filled(
        bar,
        0.0,
        egui::Color32::from_rgba_unmultiplied(14, 10, 8, 235),
    );
    bg.hline(
        bar.x_range(),
        bar.top(),
        egui::Stroke::new(1.5, theme::GOLD.gamma_multiply(0.7)),
    );
    crate::chrome::diamond(&bg, egui::pos2(bar.center().x, bar.top()), 6.0);
    egui::Area::new("menu-version".into())
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(60.0, -11.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .color(theme::PARCHMENT_DARK),
            );
        });
    egui::Area::new("menu-links".into())
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -10.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::italic(p.i18n.t("menu.tagline")).color(theme::PARCHMENT));
                let link = |ui: &mut egui::Ui, text: &str| {
                    ui.label(RichText::new("|").color(theme::GOLD));
                    ui.add(
                        egui::Label::new(RichText::new(text).underline().color(theme::PARCHMENT))
                            .sense(egui::Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                };
                if link(ui, p.i18n.t("menu.srd_docs")) {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab("https://www.dndbeyond.com/srd"));
                }
                if link(ui, p.i18n.t("menu.licenses")) {
                    p.ui.menu_info = 2;
                }
                if link(ui, p.i18n.t("menu.support")) {
                    p.ui.menu_info = 1;
                }
            });
        });
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
    let (w, h) = (470.0, 58.0);
    ui.spacing_mut().item_spacing.y = 13.0;
    // Continue the most recently played campaign.
    if let Some(last) = p
        .cache
        .campaigns
        .iter()
        .max_by_key(|c| c.updated_at)
        .cloned()
    {
        let label = format!("{} — {}", p.i18n.t("menu.continue"), last.name);
        if theme::plaque_button_ex(ui, Icon::Book, &label, w, h, true)
            .on_hover_text(&last.name)
            .clicked()
        {
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
        let open = target.is_some_and(|t| p.ui.menu == t);
        if theme::plaque_button_ex(ui, icon, p.i18n.t(key), w, h, open).clicked() {
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
