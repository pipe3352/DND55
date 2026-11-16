//! In-game HUD. The map stays the focus; panels are collapsible.

use crate::character_ui::{CharacterDraft, creator_ui, sheet_ui};
use crate::fx::Presentation;
use crate::i18n::I18n;
use crate::map_render::CatalogRes;
use crate::state::{AppState, DmTab, GameView, HostRuntime, Net, Tool, UiState};
use crate::theme;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::EguiContexts;
use bevy_egui::egui::{self, Color32, RichText, Ui};
use dnd_rules::d20::RollMode;
use dnd_session::command::{Command, DmCommand};
use dnd_session::model::Principal;
use dnd_session::view::{ClientView, HpView};
use dnd_world::map::AmbientLight;

#[derive(SystemParam)]
pub struct GameUi<'w> {
    pub ui: ResMut<'w, UiState>,
    pub net: ResMut<'w, Net>,
    pub gv: ResMut<'w, GameView>,
    pub i18n: Res<'w, I18n>,
    pub pres: ResMut<'w, Presentation>,
    pub host: Option<Res<'w, HostRuntime>>,
    pub draft: ResMut<'w, CharacterDraft>,
    pub catalog: Res<'w, CatalogRes>,
    pub next: ResMut<'w, NextState<AppState>>,
    pub hover: Res<'w, crate::input::Hover>,
    pub graph: ResMut<'w, crate::graph_editor::GraphEditor>,
    pub thumbs: ResMut<'w, crate::editor_ui::PaletteThumbs>,
    pub fit: ResMut<'w, crate::input::CameraFit>,
    pub settings: ResMut<'w, crate::settings::GameSettings>,
    pub audio: Option<Res<'w, crate::audio::AudioStatus>>,
    pub map_ptr: ResMut<'w, crate::pointer::MapPointer>,
}

pub fn game_ui(
    mut contexts: EguiContexts,
    mut g: GameUi,
    mut commands: Commands,
    mut themed: Local<Option<(f32, bool)>>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    theme::set_reduced_motion(&ctx, g.settings.reduced_motion);
    if *themed != Some((g.ui.ui_scale, g.settings.reduced_motion)) {
        theme::apply(&ctx, g.ui.ui_scale);
        *themed = Some((g.ui.ui_scale, g.settings.reduced_motion));
        // Named font families become usable from the next frame.
        ctx.request_repaint();
        return Ok(());
    }
    let Some(view) = g.gv.view.clone() else {
        *g.map_ptr = Default::default();
        egui::Area::new("waiting".into())
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(&ctx, |ui| {
                ui.label(
                    RichText::new(g.i18n.t("game.waiting"))
                        .size(22.0)
                        .color(theme::GOLD),
                );
            });
        return Ok(());
    };
    let dm = g.net.is_dm();
    let mut root = Ui::new(
        ctx.clone(),
        "game-root".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    // ---------------- top bar: crest + title, view tabs, session actions
    egui::Panel::top("top")
        .exact_size(58.0)
        .frame(theme::toolbar_frame())
        .show(&mut root, |ui| {
            theme::wood_bg(ui);
            theme::dark_scope(ui);
            let full = ui.max_rect();
            ui.horizontal_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(50.0, 48.0), egui::Sense::hover());
                crate::chrome::dragon(ui.painter(), r);
                ui.label(theme::display(g.i18n.t("app.title"), 28.0).color(theme::GOLD_BRIGHT));
                let tabs_w = if dm { 640.0 } else { 440.0 };
                let tabs_rect =
                    egui::Rect::from_center_size(full.center(), egui::vec2(tabs_w, 44.0));
                if full.width() >= 1380.0 {
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(tabs_rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        |ui| view_tabs(ui, &mut g, dm),
                    );
                } else {
                    ui.add_space(12.0);
                    view_tabs(ui, &mut g, dm);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    use crate::icons::Icon;
                    if crate::chrome::bar_button(ui, None, g.i18n.t("game.leave"), true).clicked() {
                        commands.remove_resource::<HostRuntime>();
                        g.net.client = None;
                        g.net.you = None;
                        g.net.status.clear();
                        *g.gv = GameView::default();
                        g.next.set(AppState::MainMenu);
                    }
                    if crate::chrome::bar_button(ui, Some(Icon::Gear), "", false)
                        .on_hover_text(g.i18n.t("menu.settings"))
                        .clicked()
                    {
                        g.ui.show_settings = !g.ui.show_settings;
                    }
                    if dm {
                        let save = crate::chrome::bar_button(
                            ui,
                            Some(Icon::Seal),
                            g.i18n.t("game.save"),
                            false,
                        )
                        .on_hover_text(g.i18n.t("game.save_help"));
                        if save.clicked() {
                            let label = std::mem::take(&mut g.ui.save_label);
                            g.net.save(label);
                            g.gv.toast(g.i18n.t("game.saving"));
                        }
                        save.context_menu(|ui| {
                            ui.label(g.i18n.t("game.save_label"));
                            ui.text_edit_singleline(&mut g.ui.save_label);
                        });
                        if let Some(h) = &g.host {
                            let code = h.handle.session_code.clone();
                            if crate::chrome::bar_button(
                                ui,
                                Some(Icon::Link),
                                g.i18n.t("invite.players"),
                                false,
                            )
                            .on_hover_text(format!("{} {code}", g.i18n.t("game.code")))
                            .clicked()
                            {
                                g.ui.show_invite = !g.ui.show_invite;
                            }
                        }
                    }
                    // Session status pill (ping in the tooltip) or reconnect.
                    match (&g.net.client, g.net.rtt_ms) {
                        (Some(_), ms) => {
                            let c = match ms {
                                Some(ms) if ms >= 400 => theme::BLOOD,
                                Some(ms) if ms >= 120 => theme::GOLD,
                                _ => egui::Color32::from_rgb(90, 200, 90),
                            };
                            crate::chrome::pill(ui, c, g.i18n.t("menu.session_active"))
                                .on_hover_text(ms.map(|m| format!("{m} ms")).unwrap_or_default());
                        }
                        (None, _) => {
                            if crate::chrome::bar_button(
                                ui,
                                None,
                                g.i18n.t("game.reconnect"),
                                false,
                            )
                            .clicked()
                            {
                                crate::hosting::reconnect(&mut g.net);
                            }
                            crate::chrome::pill(ui, theme::BLOOD, &g.net.status);
                        }
                    }
                });
            });
        });

    let editing = dm && g.ui.dm_tab == DmTab::Editor && view.room.is_some() && !g.ui.show_party;
    let preview = editing && g.ui.editor_preview;

    // ---------------- second row: banner, campaign – room (DM: room switcher), editor tools
    egui::Panel::top("room-row")
        .exact_size(52.0)
        .frame(theme::toolbar_frame())
        .show(&mut root, |ui| {
            theme::wood_bg(ui);
            theme::dark_scope(ui);
            ui.horizontal_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(36.0, 50.0), egui::Sense::hover());
                crate::chrome::banner(ui.painter(), r, crate::icons::Icon::Sword);
                ui.label(theme::display(&view.campaign_name, 25.0).color(theme::GOLD_BRIGHT));
                if let Some(r) = &view.room {
                    let room_txt =
                        theme::gothic(format!("– {} ⏷", r.name), 20.0).color(theme::IVORY);
                    if dm {
                        let resp = ui.add(egui::Label::new(room_txt).sense(egui::Sense::click()));
                        egui::Popup::menu(&resp).show(|ui| {
                            theme::dark_scope(ui);
                            for n in view.graph.nodes.values() {
                                if ui.selectable_label(n.id == r.id, &n.name).clicked() {
                                    g.net.cmd(Command::Dm(DmCommand::ViewRoom { room: n.id }));
                                }
                            }
                            ui.separator();
                            if ui.button(g.i18n.t("graph.open")).clicked() {
                                g.ui.show_graph = true;
                            }
                        });
                    } else {
                        ui.label(theme::gothic(format!("– {}", r.name), 20.0).color(theme::IVORY));
                    }
                    if dm && !r.is_party_room {
                        ui.label(
                            theme::italic(g.i18n.t("game.editing_other_room"))
                                .color(egui::Color32::from_rgb(235, 150, 120)),
                        );
                    }
                }
                if let Some(c) = &view.combat {
                    ui.label(
                        theme::gothic(format!("{} {}", g.i18n.t("combat.round"), c.round), 19.0)
                            .color(egui::Color32::from_rgb(240, 120, 100)),
                    );
                }
                ui.add_space(16.0);
                if editing && !preview {
                    crate::editor_ui::toolbar(ui, &mut g, &view);
                } else {
                    let grid = g.ui.show_grid;
                    if theme::icon_button(ui, crate::icons::Icon::Grid, grid, g.i18n.t("game.grid"))
                        .clicked()
                    {
                        g.ui.show_grid = !grid;
                    }
                }
            });
        });

    // ---------------- bottom: dice + log + chat (dark), with candle and skull decorations
    egui::Panel::bottom("bottom")
        .resizable(true)
        .default_size(205.0)
        .min_size(170.0)
        .frame(theme::panel_frame().fill(crate::chrome::DARK))
        .show(&mut root, |ui| {
            crate::chrome::dark_panel_bg(ui);
            theme::dark_scope(ui);
            let r = ui.max_rect();
            let t = ui.input(|i| i.time) as f32;
            let t = if theme::reduced_motion(ui.ctx()) {
                1.0
            } else {
                t
            };
            let deco = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("bottom-deco"),
            ));
            crate::chrome::candle(
                &deco,
                egui::pos2(r.left() + 44.0, r.bottom() + 8.0),
                0.62,
                t,
            );
            crate::chrome::skull(&deco, egui::pos2(r.right() - 34.0, r.bottom() - 26.0), 0.9);
            if !theme::reduced_motion(ui.ctx()) {
                ui.ctx().request_repaint();
            }
            ui.horizontal_top(|ui| {
                ui.add_space(78.0);
                let w = (ui.available_width() - 92.0) / 2.0;
                ui.vertical(|ui| {
                    ui.set_width(w - 12.0);
                    dice_panel(ui, &mut g);
                });
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.set_width(w);
                    log_panel(ui, &mut g, &view);
                });
            });
        });
    if editing && !preview {
        egui::Panel::bottom("editor-status")
            .frame(theme::toolbar_frame().inner_margin(egui::Margin::symmetric(10, 4)))
            .show(&mut root, |ui| {
                theme::wood_bg(ui);
                theme::dark_scope(ui);
                crate::editor_ui::status_bar(ui, &mut g, &view);
            });
    }

    // ---------------- left: DM tools (dark)
    if dm && !preview {
        egui::Panel::left("dm")
            .resizable(true)
            .default_size(440.0)
            .min_size(300.0)
            .frame(theme::panel_frame().fill(crate::chrome::DARK))
            .show(&mut root, |ui| {
                crate::chrome::dark_panel_bg(ui);
                theme::dark_scope(ui);
                use crate::icons::Icon;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let tw = ((ui.available_width() - 24.0) / 5.0).clamp(54.0, 86.0);
                    for (tab, key, icon) in [
                        (DmTab::Editor, "dm.editor", Icon::Hammer),
                        (DmTab::World, "dm.world", Icon::Map),
                        (DmTab::Monsters, "dm.monsters", Icon::Skull),
                        (DmTab::Events, "dm.events", Icon::Scroll),
                        (DmTab::Session, "dm.session", Icon::Banner),
                    ] {
                        if crate::chrome::tile_button(
                            ui,
                            icon,
                            g.i18n.t(key),
                            g.ui.dm_tab == tab,
                            egui::vec2(tw, 66.0),
                        )
                        .clicked()
                        {
                            g.ui.dm_tab = tab;
                            g.ui.show_party = false;
                        }
                    }
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical().show(ui, |ui| match g.ui.dm_tab {
                    DmTab::Editor => crate::editor_ui::palette(ui, &mut g, &view),
                    DmTab::World => world_tab(ui, &mut g, &view),
                    DmTab::Monsters => monsters_tab(ui, &mut g, &view),
                    DmTab::Events => crate::ui_dm::events_tab(ui, &mut g, &view),
                    DmTab::Session => session_tab(ui, &mut g, &view),
                });
            });
    }

    // ---------------- right: party, initiative, sheet
    egui::Panel::right("party")
        .resizable(true)
        .default_size(320.0)
        .frame(theme::panel_frame())
        .show(&mut root, |ui| {
            theme::panel_bg(ui);
            egui::ScrollArea::vertical().show(ui, |ui| {
                if editing && !preview {
                    crate::editor_ui::inspector(ui, &mut g, &view);
                } else {
                    right_panel(ui, &mut g, &view);
                }
            });
        });

    // Whatever the panels left free is the map canvas: only there do wheel/clicks reach the map.
    let map_rect = root.available_rect_before_wrap();
    *g.map_ptr = crate::pointer::compute(&ctx, map_rect);
    crate::chrome::map_frame(&ctx.layer_painter(egui::LayerId::background()), map_rect);
    overlays(&ctx, &mut g, &view);
    if editing && !preview {
        crate::editor_ui::minimap(&ctx, &mut g, &view);
        crate::editor_ui::context_menu(&ctx, &mut g, &view);
    }
    if preview {
        egui::Area::new("exit-preview".into())
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 48.0))
            .show(&ctx, |ui| {
                if theme::plaque_button(
                    ui,
                    crate::icons::Icon::Brush,
                    g.i18n.t("editor.exit_preview"),
                    260.0,
                )
                .clicked()
                {
                    g.ui.editor_preview = false;
                }
            });
    }
    if g.ui.show_invite {
        invite_window(&ctx, &mut g);
    }
    if g.ui.show_settings {
        let mut open = true;
        theme::win(format!("⚙ {}", g.i18n.t("menu.settings")))
            .open(&mut open)
            .show(&ctx, |ui| {
                theme::window_bg(ui);
                if g.audio.as_ref().is_some_and(|a| !a.available) {
                    ui.label(
                        RichText::new(g.i18n.t("settings.no_audio_device")).color(theme::BLOOD),
                    );
                }
                crate::settings::settings_ui(ui, &mut g.settings, &g.i18n);
            });
        g.ui.show_settings &= open;
    }
    Ok(())
}

/// Top-bar view tabs. DM: Campaign · Map Editor · Character Manager; players: Campaign · Character.
fn view_tabs(ui: &mut Ui, g: &mut GameUi, dm: bool) {
    use crate::icons::Icon;
    ui.spacing_mut().item_spacing.x = 4.0;
    let editor = dm && g.ui.dm_tab == DmTab::Editor && !g.ui.show_party;
    let chars = if dm { g.ui.show_party } else { g.ui.show_sheet };
    if crate::chrome::top_tab(ui, Icon::Map, g.i18n.t("tab.campaign"), !editor && !chars).clicked()
    {
        g.ui.show_party = false;
        if dm {
            if g.ui.dm_tab == DmTab::Editor {
                g.ui.dm_tab = DmTab::World;
            }
            g.ui.tool = crate::state::Tool::Play;
        } else {
            g.ui.show_sheet = false;
        }
    }
    if dm && crate::chrome::top_tab(ui, Icon::Scroll, g.i18n.t("tab.map_editor"), editor).clicked()
    {
        g.ui.dm_tab = DmTab::Editor;
        g.ui.show_party = false;
    }
    if crate::chrome::top_tab(ui, Icon::Helm, g.i18n.t("tab.characters"), chars).clicked() {
        if dm {
            g.ui.show_party = true;
        } else {
            g.ui.show_sheet = true;
        }
    }
}

fn dice_panel(ui: &mut Ui, g: &mut GameUi) {
    crate::chrome::heading(ui, Some(crate::icons::Icon::D20), g.i18n.t("dice.title"));
    let roll = |g: &mut GameUi, expr: String| {
        let d20 = expr.ends_with("d20");
        let (mode, private) = (
            if d20 {
                g.ui.dice_mode
            } else {
                RollMode::Normal
            },
            g.ui.private_roll,
        );
        g.net.cmd(Command::RollDice {
            expr,
            mode,
            private,
            label: String::new(),
        });
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for s in [4, 6, 8, 10, 12, 20] {
            if crate::chrome::die(ui, s, 54.0)
                .on_hover_text(format!("1d{s}"))
                .clicked()
            {
                roll(g, format!("1d{s}"));
            }
        }
        if ui.small_button("d100").clicked() {
            roll(g, "1d100".into());
        }
    });
    ui.horizontal(|ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(&mut g.ui.dice_expr)
                .desired_width(150.0)
                .font(egui::FontId::new(18.0, egui::FontFamily::Proportional))
                .hint_text("2d6+3"),
        );
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if crate::chrome::gold_button(
            ui,
            Some(crate::icons::Icon::D20),
            g.i18n.t("dice.roll"),
            110.0,
        )
        .clicked()
            || enter
        {
            let expr = g.ui.dice_expr.clone();
            roll(g, expr);
        }
    });
    ui.horizontal(|ui| {
        let mut adv = g.ui.dice_mode == RollMode::Advantage;
        let mut dis = g.ui.dice_mode == RollMode::Disadvantage;
        if ui.checkbox(&mut adv, g.i18n.t("dice.advantage")).changed() {
            g.ui.dice_mode = if adv {
                RollMode::Advantage
            } else {
                RollMode::Normal
            };
        }
        if ui
            .checkbox(&mut dis, g.i18n.t("dice.disadvantage"))
            .changed()
        {
            g.ui.dice_mode = if dis {
                RollMode::Disadvantage
            } else {
                RollMode::Normal
            };
        }
        ui.checkbox(
            &mut g.ui.private_roll,
            g.i18n.t(if g.net.is_dm() {
                "dice.hidden"
            } else {
                "dice.private"
            }),
        );
    });
    if let Some((who, detail, total, t)) = &g.pres.last_roll {
        let shown = if *t > 0.3 {
            ((t * 97.0) as i32 % 20 + 1).to_string()
        } else {
            total.to_string()
        };
        ui.horizontal(|ui| {
            // Result on a wax seal; the number tumbles while the dice "roll".
            let (r, _) = ui.allocate_exact_size(egui::vec2(52.0, 52.0), egui::Sense::hover());
            let p = ui.painter();
            let wobble = if *t > 0.3 && !theme::reduced_motion(ui.ctx()) {
                (t * 40.0).sin() * 0.25
            } else {
                0.0
            };
            p.circle_filled(
                r.center() + egui::vec2(1.5, 2.5),
                25.0,
                egui::Color32::from_black_alpha(70),
            );
            p.circle_filled(r.center(), 25.0, theme::BURGUNDY);
            p.circle_stroke(r.center(), 21.0, egui::Stroke::new(1.2, theme::GOLD));
            let galley = p.layout_no_wrap(
                shown.clone(),
                egui::FontId::new(26.0, theme::bold_family()),
                theme::IVORY,
            );
            let pos = r.center() - galley.size() / 2.0;
            p.add(egui::epaint::TextShape::new(pos, galley, theme::IVORY).with_angle(wobble));
            ui.vertical(|ui| {
                ui.label(RichText::new(who).strong().color(theme::GOLD_BRIGHT));
                ui.label(RichText::new(detail).small());
            });
        });
    }
}

fn log_panel(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    crate::chrome::heading(ui, Some(crate::icons::Icon::Book), g.i18n.t("log.title"));
    // Arrival times: new lines get the current local time; the view's log may drop old lines.
    let stamps = &mut g.pres.log_stamps;
    if stamps.len() > view.log.len() {
        let drop = stamps.len() - view.log.len();
        stamps.drain(..drop);
    }
    while stamps.len() < view.log.len() {
        stamps.push(crate::platform::local_hhmm());
    }
    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(0x16, 0x11, 0x0E))
        .stroke(egui::Stroke::new(1.0, theme::BRONZE))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(10, 6));
    frame.show(ui, |ui| {
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .max_height((ui.available_height() - 52.0).max(40.0))
            .show(ui, |ui| {
                let n = view.log.len();
                for (i, l) in view.log.iter().enumerate().skip(n.saturating_sub(80)) {
                    let stamp = g.pres.log_stamps.get(i).cloned().unwrap_or_default();
                    log_row(ui, &theme::glyph_safe(l), &stamp, theme::PARCHMENT);
                }
                for (from, text) in g.pres.chat.iter().rev().take(20).rev() {
                    log_row(
                        ui,
                        &format!("{from}: {text}"),
                        "",
                        egui::Color32::from_rgb(150, 190, 230),
                    );
                }
            });
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let send_w = 110.0;
        let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, 30.0), egui::Sense::hover());
        crate::chrome::bubble(ui.painter(), r.center());
        let resp = ui.add(
            egui::TextEdit::singleline(&mut g.ui.chat)
                .hint_text(g.i18n.t("chat.hint"))
                .font(egui::FontId::new(16.0, egui::FontFamily::Proportional))
                .desired_width((ui.available_width() - send_w - 18.0).max(80.0)),
        );
        if (crate::chrome::gold_button(
            ui,
            Some(crate::icons::Icon::Pointer),
            g.i18n.t("chat.send"),
            send_w,
        )
        .clicked()
            || resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            && !g.ui.chat.trim().is_empty()
        {
            let text = std::mem::take(&mut g.ui.chat);
            g.net.cmd(Command::Chat { text });
        }
    });
}

/// One adventure-log line: bullet, text (truncated, full text on hover) and its time on the right.
fn log_row(ui: &mut Ui, text: &str, stamp: &str, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("•").color(theme::GOLD));
        let w = (ui.available_width() - 56.0).max(60.0);
        ui.scope(|ui| {
            ui.set_max_width(w);
            ui.add(egui::Label::new(RichText::new(text).color(color)).truncate());
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(stamp).small().color(theme::PARCHMENT_DARK));
        });
    });
}

fn world_tab(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let party_room = view.dm.as_ref().map(|d| d.party_room);
    let viewing = view.room.as_ref().map(|r| r.id);
    if ui
        .button(RichText::new(format!("🗺 {}", g.i18n.t("graph.open"))).strong())
        .clicked()
    {
        g.ui.show_graph = !g.ui.show_graph;
    }
    theme::heading(ui, g.i18n.t("world.rooms"));
    for (id, node) in &view.graph.nodes {
        ui.horizontal(|ui| {
            let mut label = RichText::new(&node.name);
            if Some(*id) == party_room {
                label = label.strong().color(theme::BLOOD);
            }
            ui.label(label);
            if Some(*id) != viewing && ui.small_button(g.i18n.t("world.view")).clicked() {
                g.net.cmd(Command::Dm(DmCommand::ViewRoom { room: *id }));
            }
            if Some(*id) != party_room {
                let connected =
                    party_room.is_some_and(|p| view.graph.edges.iter().any(|e| e.connects(p, *id)));
                let text = if connected {
                    g.i18n.t("world.travel")
                } else {
                    g.i18n.t("world.travel_override")
                };
                if ui.small_button(text).clicked() {
                    g.net.cmd(Command::Dm(DmCommand::TransitionParty {
                        to: *id,
                        override_rules: !connected,
                    }));
                }
            }
        });
    }
    if let Some(v) = viewing {
        let edit = g.ui.room_desc_edit.get_or_insert_with(|| {
            let n = &view.graph.nodes[&v];
            (v, n.name.clone(), n.description.clone())
        });
        if edit.0 != v {
            let n = &view.graph.nodes[&v];
            *edit = (v, n.name.clone(), n.description.clone());
        }
        theme::heading(ui, g.i18n.t("world.this_room"));
        ui.text_edit_singleline(&mut edit.1);
        ui.add(
            egui::TextEdit::multiline(&mut edit.2)
                .desired_rows(3)
                .hint_text(g.i18n.t("world.description")),
        );
        let (room, name, description) = edit.clone();
        if ui.button(g.i18n.t("world.apply")).clicked() {
            g.net.cmd(Command::Dm(DmCommand::RenameRoom {
                room,
                name,
                description,
            }));
        }
        theme::heading(ui, g.i18n.t("world.connections"));
        for e in view.graph.edges.iter().filter(|e| e.from == v || e.to == v) {
            let other = if e.from == v { e.to } else { e.from };
            let name = view
                .graph
                .nodes
                .get(&other)
                .map(|n| n.name.clone())
                .unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} {name}",
                    if e.one_way {
                        if e.from == v { "▶" } else { "◀" }
                    } else {
                        "↔"
                    }
                ));
                let (mut locked, mut secret) = (e.locked, e.secret);
                let a = ui.checkbox(&mut locked, g.i18n.t("world.locked")).changed();
                let b = ui.checkbox(&mut secret, g.i18n.t("world.secret")).changed();
                if a || b {
                    g.net.cmd(Command::Dm(DmCommand::SetEdge {
                        edge: e.id,
                        locked,
                        secret,
                        arrival: e.arrival,
                    }));
                }
                if ui.small_button("✖").clicked() {
                    g.net.cmd(Command::Dm(DmCommand::DeleteEdge { edge: e.id }));
                }
            });
        }
        ui.horizontal(|ui| {
            let target =
                g.ui.connect_to
                    .and_then(|t| view.graph.nodes.get(&t))
                    .map(|n| n.name.clone())
                    .unwrap_or_else(|| "—".into());
            egui::ComboBox::from_id_salt("connect")
                .selected_text(target)
                .show_ui(ui, |ui| {
                    for (id, n) in &view.graph.nodes {
                        if *id != v {
                            ui.selectable_value(&mut g.ui.connect_to, Some(*id), &n.name);
                        }
                    }
                });
            ui.checkbox(&mut g.ui.connect_one_way, g.i18n.t("world.one_way"));
            if let Some(to) = g.ui.connect_to
                && ui.button(g.i18n.t("world.connect")).clicked()
            {
                let one_way = g.ui.connect_one_way;
                g.net.cmd(Command::Dm(DmCommand::ConnectRooms {
                    from: v,
                    to,
                    one_way,
                }));
            }
        });
    }
    theme::heading(ui, g.i18n.t("world.new_room"));
    ui.text_edit_singleline(&mut g.ui.new_room_name);
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(&mut g.ui.new_room_w)
                .range(4..=128)
                .prefix("W "),
        );
        ui.add(
            egui::DragValue::new(&mut g.ui.new_room_h)
                .range(4..=128)
                .prefix("H "),
        );
        for (amb, key) in [
            (AmbientLight::Bright, "light.bright"),
            (AmbientLight::Dim, "light.dim"),
            (AmbientLight::Darkness, "light.dark"),
        ] {
            ui.selectable_value(&mut g.ui.new_room_ambient, amb, g.i18n.t(key));
        }
    });
    if ui.button(g.i18n.t("world.create_room")).clicked() {
        let (name, width, height, ambient) = (
            std::mem::take(&mut g.ui.new_room_name),
            g.ui.new_room_w,
            g.ui.new_room_h,
            g.ui.new_room_ambient,
        );
        g.net.cmd(Command::Dm(DmCommand::CreateRoom {
            name,
            width,
            height,
            ambient,
        }));
    }
}

fn monsters_tab(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    theme::heading(ui, g.i18n.t("monsters.bestiary"));
    if ui
        .button(RichText::new(format!("🐉 {}", g.i18n.t("bestiary.open"))).strong())
        .clicked()
    {
        g.ui.show_bestiary = !g.ui.show_bestiary;
    }
    ui.checkbox(&mut g.ui.spawn_hidden, g.i18n.t("monsters.spawn_hidden"));
    if let Some(dm) = &view.dm {
        for m in &dm.bestiary {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(
                        g.ui.tool == Tool::Spawn && g.ui.monster == m.id,
                        RichText::new(&m.name).strong(),
                    )
                    .clicked()
                {
                    g.ui.monster = m.id.clone();
                    g.ui.tool = Tool::Spawn;
                }
                ui.label(format!(
                    "CR {} · AC {} · HP {}",
                    m.challenge, m.armor_class, m.hit_points
                ));
            });
        }
    }
    ui.label(RichText::new(g.i18n.t("monsters.help")).small());
    theme::heading(ui, g.i18n.t("monsters.in_room"));
    for c in view.creatures.iter().filter(|c| c.is_monster) {
        ui.horizontal(|ui| {
            let hp = match c.hp {
                HpView::Exact { current, max, .. } => format!("{current}/{max}"),
                HpView::Status(s) => format!("{s:?}"),
            };
            if ui
                .selectable_label(g.ui.selected == Some(c.id), format!("{} ({hp})", c.name))
                .clicked()
            {
                g.ui.selected = Some(c.id);
            }
            let mut hidden = c.hidden;
            if ui
                .checkbox(&mut hidden, "👁")
                .on_hover_text(g.i18n.t("monsters.hidden_toggle"))
                .changed()
            {
                g.net.cmd(Command::Dm(DmCommand::SetCreatureHidden {
                    entity: c.id,
                    hidden,
                }));
            }
            if ui.small_button("✖").clicked() {
                g.net
                    .cmd(Command::Dm(DmCommand::RemoveCreature { entity: c.id }));
            }
        });
    }
}

fn session_tab(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    theme::heading(ui, g.i18n.t("session.players"));
    for p in &view.players {
        ui.label(format!(
            "{} {} — {}{}",
            if p.connected { "⚫" } else { "⚪" },
            p.name,
            p.character
                .clone()
                .unwrap_or_else(|| g.i18n.t("session.no_character").into()),
            if p.ready { " ✔" } else { "" }
        ));
    }
    if !view.session_started && ui.button(g.i18n.t("session.start")).clicked() {
        g.net.cmd(Command::Dm(DmCommand::StartSession));
    }
    if let Some(dm) = &view.dm {
        let mut allowed = dm.exploration_movement;
        if ui
            .checkbox(&mut allowed, g.i18n.t("session.free_movement"))
            .changed()
        {
            g.net
                .cmd(Command::Dm(DmCommand::SetExplorationMovement { allowed }));
        }
    }
    theme::heading(ui, g.i18n.t("combat.title"));
    if view.combat.is_none() {
        ui.label(g.i18n.t("combat.surprised_help"));
        for c in &view.creatures {
            let mut s = g.ui.surprised.contains(&c.id);
            if ui.checkbox(&mut s, &c.name).changed() {
                if s {
                    g.ui.surprised.push(c.id);
                } else {
                    g.ui.surprised.retain(|x| *x != c.id);
                }
            }
        }
        if ui
            .button(RichText::new(g.i18n.t("combat.start")).strong())
            .clicked()
        {
            let surprised = std::mem::take(&mut g.ui.surprised);
            g.net.cmd(Command::Dm(DmCommand::StartCombat { surprised }));
        }
    } else {
        ui.horizontal(|ui| {
            if ui.button(g.i18n.t("combat.next_turn")).clicked() {
                g.net.cmd(Command::Dm(DmCommand::NextTurn));
            }
            if ui.button(g.i18n.t("combat.end")).clicked() {
                g.net.cmd(Command::Dm(DmCommand::EndCombat));
            }
        });
    }
    theme::heading(ui, g.i18n.t("session.selected"));
    if let Some(c) =
        g.ui.selected
            .and_then(|id| view.creatures.iter().find(|c| c.id == id))
            .cloned()
    {
        ui.label(RichText::new(&c.name).strong());
        ui.horizontal(|ui| {
            for d in [-5, -1, 1, 5] {
                if ui.button(format!("{d:+} HP")).clicked() {
                    g.net.cmd(Command::Dm(DmCommand::AdjustHp {
                        entity: c.id,
                        delta: d,
                    }));
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            use dnd_rules::conditions::Condition::*;
            for cond in [
                Blinded,
                Charmed,
                Deafened,
                Frightened,
                Grappled,
                Incapacitated,
                Invisible,
                Paralyzed,
                Petrified,
                Poisoned,
                Prone,
                Restrained,
                Stunned,
                Unconscious,
            ] {
                let mut on = c.conditions.contains(&cond);
                if ui.toggle_value(&mut on, format!("{cond:?}")).changed() {
                    g.net.cmd(Command::Dm(DmCommand::SetCondition {
                        entity: c.id,
                        condition: cond,
                        present: on,
                    }));
                }
            }
        });
        ui.label(RichText::new(g.i18n.t("session.teleport_help")).small());
    }
    crate::ui_dm::session_tools(ui, g, view);
    if let Some(r) = &view.room {
        ui.horizontal(|ui| {
            if ui.button(g.i18n.t("session.reveal")).clicked() {
                g.net.cmd(Command::Dm(DmCommand::RevealRoom {
                    room: r.id,
                    explored: true,
                }));
            }
            if ui.button(g.i18n.t("session.hide_all")).clicked() {
                g.net.cmd(Command::Dm(DmCommand::RevealRoom {
                    room: r.id,
                    explored: false,
                }));
            }
        });
    }
}

fn right_panel(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    if let Some(c) = &view.combat {
        theme::heading(
            ui,
            &format!(
                "{} · {} {}",
                g.i18n.t("combat.initiative"),
                g.i18n.t("combat.round"),
                c.round
            ),
        );
        for e in &c.order {
            let active = c.active == Some(e.entity);
            let cv = view.creatures.iter().find(|x| x.id == e.entity);
            initiative_card(ui, e, cv, active);
        }
        if c.waiting_on_reaction {
            ui.label(RichText::new(g.i18n.t("combat.waiting_reaction")).italics());
        }
    }
    theme::heading(ui, g.i18n.t("party.title"));
    for c in view
        .creatures
        .iter()
        .filter(|c| c.faction == dnd_session::model::Faction::Party)
    {
        if let HpView::Exact { current, max, .. } = c.hp {
            theme::bar(
                ui,
                &c.name,
                current,
                max,
                if current * 2 <= max {
                    theme::BLOOD
                } else {
                    theme::HEALTH
                },
            );
        }
    }
    if !g.net.is_dm() && !view.session_started {
        // Lobby: tell the DM you're ready.
        let me = view
            .players
            .iter()
            .find(|p| Some(Principal::Player(p.id)) == g.net.you);
        if let Some(p) = me {
            let mut ready = p.ready;
            if ui
                .toggle_value(&mut ready, RichText::new(g.i18n.t("lobby.ready")).strong())
                .on_hover_text(g.i18n.t("lobby.ready_help"))
                .changed()
            {
                g.net.cmd(Command::SetReady(ready));
            }
        }
    }
    if !g.net.is_dm() {
        if let Some((id, sheet)) = view.my_characters.first().cloned() {
            g.ui.selected.get_or_insert(id);
            let me = view.creatures.iter().find(|c| c.id == id);
            ui.add_space(6.0);
            ui.checkbox(&mut g.ui.show_sheet, g.i18n.t("sheet.show"));
            if g.ui.show_sheet {
                theme::heading(ui, g.i18n.t("sheet.title"));
                sheet_ui(ui, &sheet, me, &mut g.net, &mut g.ui, &g.i18n);
            }
            crate::ui_play::sheet_extras(ui, g, view);
        } else {
            ui.label(g.i18n.t("game.no_character"));
            if ui.button(g.i18n.t("game.create_character")).clicked() {
                g.ui.show_creator = true;
            }
        }
    }
    if let Some(r) = &view.room
        && !r.description.is_empty()
    {
        theme::heading(ui, &r.name);
        ui.label(RichText::new(&r.description).italics());
    }
}

fn overlays(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    crate::ui_play::action_bar(ctx, g, view);
    crate::ui_play::reaction_prompts(ctx, g, view);
    crate::ui_play::targeting_window(ctx, g, view);
    crate::ui_play::spellbook_window(ctx, g, view);
    crate::ui_play::levelup_window(ctx, g, view);
    crate::ui_play::inventory_window(ctx, g, view);
    crate::ui_play::loot_window(ctx, g, view);
    crate::ui_play::dialogue_window(ctx, g, view);
    if g.net.is_dm() {
        crate::ui_dm::bestiary_window(ctx, g, view);
        crate::graph_editor::window(ctx, g, view);
        crate::ui_dm::script_editor_window(ctx, g, view);
    } else if view.players_can_draw || g.ui.tool == Tool::Draw {
        player_draw_toggle(ctx, g, view);
    }
    if let Some(c) = &view.combat
        && let Some(p) = &c.your_reaction
    {
        let reactor = view.creatures.iter().find(|x| x.id == p.reactor).cloned();
        let mover = view
            .creatures
            .iter()
            .find(|x| x.id == p.mover)
            .map(|m| m.name.clone())
            .unwrap_or_default();
        theme::win(g.i18n.t("reaction.title"))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .collapsible(false)
            .show(ctx, |ui| {
                theme::window_bg(ui);
                ui.label(format!("{mover} {}", g.i18n.t("reaction.leaving")));
                if let Some(r) = &reactor {
                    let melee: Vec<(usize, String)> = r
                        .attacks
                        .iter()
                        .enumerate()
                        .filter(|(_, a)| a.kind == dnd_rules::equipment::WeaponKind::Melee)
                        .map(|(i, a)| (i, a.name.clone()))
                        .collect();
                    if !melee.iter().any(|(i, _)| *i == g.ui.reaction_attack) {
                        g.ui.reaction_attack = melee.first().map(|m| m.0).unwrap_or(0);
                    }
                    egui::ComboBox::from_id_salt("oa")
                        .selected_text(
                            melee
                                .iter()
                                .find(|m| m.0 == g.ui.reaction_attack)
                                .map(|m| m.1.clone())
                                .unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for (i, n) in &melee {
                                ui.selectable_value(&mut g.ui.reaction_attack, *i, n);
                            }
                        });
                }
                ui.horizontal(|ui| {
                    if ui.button(g.i18n.t("reaction.attack")).clicked() {
                        let attack = g.ui.reaction_attack;
                        g.net.cmd(Command::ReactionDecision {
                            accept: true,
                            attack,
                        });
                    }
                    if ui.button(g.i18n.t("reaction.decline")).clicked() {
                        g.net.cmd(Command::ReactionDecision {
                            accept: false,
                            attack: 0,
                        });
                    }
                });
            });
    }
    // Character creation inside the game (player without a character).
    if g.ui.show_creator && !g.net.is_dm() {
        let mut open = true;
        let mut submit = false;
        theme::win(g.i18n.t("game.create_character"))
            .open(&mut open)
            .default_width(860.0)
            .show(ctx, |ui| {
                theme::window_bg(ui);
                submit = creator_ui(ui, &mut g.draft, &g.i18n);
            });
        if submit {
            let (sheet, png) = (g.draft.sheet.clone(), g.draft.portrait.clone());
            g.net.cmd(Command::SubmitCharacter {
                sheet: Box::new(sheet.clone()),
                portrait_png: png.clone(),
            });
            if let Ok(store) = crate::hosting::open_store() {
                let _ = store.save_character(g.draft.library_id, &sheet, png.as_deref());
            }
            g.ui.show_creator = false;
        }
        if !open {
            g.ui.show_creator = false;
        }
    }
    // Toasts.
    egui::Area::new("toasts".into())
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 60.0))
        .show(ctx, |ui| {
            for (t, _) in &g.gv.toasts {
                egui::Frame::new()
                    .fill(theme::WOOD)
                    .corner_radius(4)
                    .inner_margin(6)
                    .show(ui, |ui| {
                        ui.set_max_width(520.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(theme::glyph_safe(t)).color(theme::PARCHMENT),
                            )
                            .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    });
            }
        });
    // Room transition fade and title.
    if g.pres.fade > 0.0 {
        let a = (g.pres.fade.min(1.0) * 255.0) as u8;
        egui::Area::new("fade".into())
            .order(egui::Order::Foreground)
            .fixed_pos(egui::Pos2::ZERO)
            .interactable(false)
            .show(ctx, |ui| {
                ui.painter()
                    .rect_filled(ctx.viewport_rect(), 0.0, Color32::from_black_alpha(a));
            });
    }
    // Chapter-style reveal of the room name: Blackletter title rising in, gold rules unfolding.
    if let Some((name, t)) = &g.pres.room_title {
        let life = 3.0 - t; // seconds since it appeared
        let fade_in = (life / 0.8).clamp(0.0, 1.0);
        let fade_out = t.min(1.0);
        let a = fade_in * fade_out;
        let rise = if theme::reduced_motion(ctx) {
            0.0
        } else {
            (1.0 - fade_in) * 18.0
        };
        let p = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            "room-title".into(),
        ));
        let c = ctx.viewport_rect().center() + egui::vec2(0.0, -130.0 + rise);
        let galley = p.layout_no_wrap(
            name.clone(),
            egui::FontId::new(64.0, theme::display_family()),
            theme::GOLD_BRIGHT.gamma_multiply(a),
        );
        let shadow = p.layout_no_wrap(
            name.clone(),
            egui::FontId::new(64.0, theme::display_family()),
            Color32::from_black_alpha((200.0 * a) as u8),
        );
        let pos = c - galley.size() / 2.0;
        p.rect_filled(
            egui::Rect::from_center_size(
                c,
                egui::vec2(galley.size().x + 160.0, galley.size().y + 30.0),
            ),
            30.0,
            Color32::from_black_alpha((120.0 * a) as u8),
        );
        p.galley(pos + egui::vec2(3.0, 4.0), shadow, Color32::BLACK);
        let gsize = galley.size();
        p.galley(pos, galley, theme::GOLD_BRIGHT);
        let half = (gsize.x / 2.0 + 60.0) * fade_in;
        let dy = gsize.y / 2.0 + 6.0;
        p.line_segment(
            [c + egui::vec2(-half, dy), c + egui::vec2(half, dy)],
            egui::Stroke::new(1.5, theme::GOLD.gamma_multiply(a)),
        );
        if !theme::reduced_motion(ctx) {
            ctx.request_repaint();
        }
    }
    if g.pres.your_turn_banner > 0.0 {
        let a = g.pres.your_turn_banner.min(1.0);
        let p = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            "turn-banner".into(),
        ));
        theme::banner(
            &p,
            ctx.viewport_rect().center() + egui::vec2(0.0, -60.0),
            g.i18n.t("combat.your_turn"),
            40.0,
            a,
        );
    }
    if g.net.you == Some(Principal::Dm) && g.ui.tool != Tool::Play && g.ui.dm_tab != DmTab::Editor {
        egui::Area::new("tool-hint".into())
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 36.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(g.i18n.t("editor.tool_hint"))
                        .color(theme::PARCHMENT)
                        .small(),
                );
            });
    }
}

/// Players can annotate the map when the DM allows it.
fn player_draw_toggle(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    if !view.players_can_draw {
        if g.ui.tool == Tool::Draw {
            g.ui.tool = Tool::Play;
        }
        return;
    }
    theme::win(g.i18n.t("draw.title"))
        .id("player-draw".into())
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(10.0, 50.0))
        .collapsible(true)
        .default_open(false)
        .resizable(false)
        .show(ctx, |ui| {
            theme::window_bg(ui);
            let mut on = g.ui.tool == Tool::Draw;
            if ui.toggle_value(&mut on, g.i18n.t("tool.draw")).changed() {
                g.ui.tool = if on { Tool::Draw } else { Tool::Play };
            }
            if let Some(r) = &view.room {
                crate::ui_dm::draw_options(ui, g, r.id, false);
            }
        });
}

/// How the DM shares the game: invite links, relay, UPnP and plain instructions.
fn invite_window(ctx: &egui::Context, g: &mut GameUi) {
    let Some(host) = g.host.as_ref() else { return };
    let mut open = true;
    let invites = crate::hosting::invites(host);
    let relay = host.handle.relay.lock().expect("relay").clone();
    let upnp = host.upnp.lock().expect("upnp").clone();
    let port = host.handle.local_addr.port();
    let mut go_online = None;
    let mut try_upnp = false;
    theme::win(format!("✉ {}", g.i18n.t("invite.title")))
        .open(&mut open)
        .default_width(560.0)
        .show(ctx, |ui| {
            theme::window_bg(ui);
            ui.label(g.i18n.t("invite.intro"));
            for (key, link) in &invites {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(g.i18n.t(key)).strong());
                    ui.add(egui::TextEdit::singleline(&mut link.clone()).desired_width(330.0));
                    if ui.button(g.i18n.t("invite.copy")).clicked() {
                        ui.ctx().copy_text(link.clone());
                        g.gv.toast(g.i18n.t("invite.copied"));
                    }
                });
            }
            theme::heading(ui, g.i18n.t("invite.online_title"));
            ui.label(RichText::new(g.i18n.t("invite.online_help")).small());
            ui.horizontal(|ui| {
                ui.label(g.i18n.t("invite.relay_address"));
                ui.add(
                    egui::TextEdit::singleline(&mut g.settings.relay_address)
                        .hint_text("relay.example.com:7979")
                        .desired_width(220.0),
                );
                let can = !g.settings.relay_address.trim().is_empty()
                    && !matches!(
                        relay,
                        dnd_net::RelayStatus::Online { .. } | dnd_net::RelayStatus::Connecting
                    );
                if ui
                    .add_enabled(can, egui::Button::new(g.i18n.t("invite.go_online")))
                    .clicked()
                {
                    go_online = Some(g.settings.relay_address.trim().to_string());
                }
            });
            let (txt, color) = match &relay {
                dnd_net::RelayStatus::Off => {
                    (g.i18n.t("invite.relay_off").to_string(), theme::WOOD_LIGHT)
                }
                dnd_net::RelayStatus::Connecting => {
                    (g.i18n.t("invite.relay_connecting").to_string(), theme::GOLD)
                }
                dnd_net::RelayStatus::Online { relay, .. } => (
                    format!("{} {relay}", g.i18n.t("invite.relay_online")),
                    theme::HEALTH,
                ),
                dnd_net::RelayStatus::Failed(e) => (
                    format!(
                        "{} {}",
                        g.i18n.t("invite.relay_failed"),
                        crate::net_sync::explain_network(e)
                    ),
                    theme::BLOOD,
                ),
            };
            ui.label(RichText::new(txt).color(color));
            theme::heading(ui, g.i18n.t("invite.direct_title"));
            ui.label(RichText::new(format!("{} {port}", g.i18n.t("invite.direct_help"))).small());
            ui.horizontal(|ui| {
                if ui
                    .button(g.i18n.t("invite.try_upnp"))
                    .on_hover_text(g.i18n.t("invite.upnp_help"))
                    .clicked()
                {
                    try_upnp = true;
                }
                match &upnp {
                    None => {}
                    Some(Ok(a)) => {
                        ui.label(
                            RichText::new(format!("{} {a}", g.i18n.t("invite.upnp_ok")))
                                .color(theme::HEALTH),
                        );
                    }
                    Some(Err(e)) => {
                        ui.label(
                            RichText::new(format!("{} {e}", g.i18n.t("invite.upnp_failed")))
                                .color(theme::BLOOD),
                        );
                    }
                }
            });
            ui.label(RichText::new(g.i18n.t("invite.security")).small().italics());
        });
    if let Some(r) = go_online
        && let Some(h) = g.host.as_ref()
    {
        crate::hosting::go_online(h, &r);
        let _ = g.settings.save();
    }
    if try_upnp && let Some(h) = g.host.as_ref() {
        crate::hosting::start_upnp(h);
    }
    g.ui.show_invite &= open;
}

/// One initiative tracker entry: faction token, name, health, initiative seal; the active
/// creature's card is lit and marked.
fn initiative_card(
    ui: &mut Ui,
    e: &dnd_session::view::InitiativeView,
    c: Option<&dnd_session::view::CreatureView>,
    active: bool,
) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::hover());
    let p = ui.painter();
    let t = if theme::reduced_motion(ui.ctx()) {
        0.0
    } else {
        ui.input(|i| i.time) as f32
    };
    if active {
        let pulse = 0.75 + 0.25 * (t * 3.0).sin();
        p.rect_filled(
            rect.expand(2.0),
            5.0,
            theme::GOLD_BRIGHT.gamma_multiply(0.35 * pulse),
        );
        ui.ctx().request_repaint();
    }
    p.rect_filled(
        rect,
        4.0,
        if active {
            egui::Color32::from_rgb(0xEC, 0xD6, 0xA0)
        } else {
            egui::Color32::from_rgb(0xDB, 0xC6, 0x9A)
        },
    );
    p.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(
            if active { 2.0 } else { 1.0 },
            if active {
                theme::BURGUNDY
            } else {
                theme::BRONZE
            },
        ),
        egui::StrokeKind::Inside,
    );
    // Illustrated token (emblem + side frame): sides differ by frame shape, not only colour.
    let token = egui::pos2(rect.left() + 20.0, rect.center().y);
    if let Some(c) = c {
        let tex = crate::tokens::token_egui(
            ui.ctx(),
            &crate::tokens::emblem(c),
            crate::tokens::side(c.faction),
        );
        p.image(
            tex.id(),
            egui::Rect::from_center_size(token, egui::vec2(32.0, 32.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        p.circle_filled(token, 14.0, theme::SLATE);
        let initial: String = e
            .name
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        p.text(
            token,
            egui::Align2::CENTER_CENTER,
            initial,
            egui::FontId::new(16.0, theme::gothic_family()),
            theme::IVORY,
        );
    }
    p.text(
        egui::pos2(rect.left() + 42.0, rect.top() + 11.0),
        egui::Align2::LEFT_CENTER,
        &e.name,
        egui::FontId::new(15.0, theme::bold_family()),
        theme::INK,
    );
    if let Some(c) = c {
        let bar = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 42.0, rect.top() + 23.0),
            egui::vec2((rect.width() - 100.0).max(40.0), 7.0),
        );
        match c.hp {
            HpView::Exact { current, max, .. } => {
                let frac = if max > 0 {
                    current.max(0) as f32 / max as f32
                } else {
                    0.0
                };
                theme::paint_bar(
                    p,
                    bar,
                    frac,
                    if frac <= 0.5 {
                        theme::BURGUNDY
                    } else {
                        theme::HEALTH
                    },
                );
            }
            HpView::Status(s) => {
                p.text(
                    bar.left_center(),
                    egui::Align2::LEFT_CENTER,
                    format!("{s:?}"),
                    egui::FontId::new(11.5, theme::italic_family()),
                    theme::INK_SOFT,
                );
            }
        }
    }
    let seal = egui::pos2(rect.right() - 20.0, rect.center().y);
    p.circle_filled(seal, 14.0, theme::WOOD);
    p.circle_stroke(seal, 14.0, egui::Stroke::new(1.0, theme::GOLD));
    p.text(
        seal,
        egui::Align2::CENTER_CENTER,
        e.total.to_string(),
        egui::FontId::new(14.0, theme::bold_family()),
        theme::GOLD_BRIGHT,
    );
    if active {
        let m = egui::pos2(rect.left() - 2.0, rect.center().y);
        p.add(egui::Shape::convex_polygon(
            vec![
                m + egui::vec2(-6.0, -7.0),
                m + egui::vec2(3.0, 0.0),
                m + egui::vec2(-6.0, 7.0),
            ],
            theme::BURGUNDY,
            egui::Stroke::NONE,
        ));
    }
}
