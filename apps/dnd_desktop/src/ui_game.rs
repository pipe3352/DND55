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
use dnd_world::catalog::Category;
use dnd_world::map::{AmbientLight, LayerKind};

#[derive(SystemParam)]
pub struct GameUi<'w> {
    pub ui: ResMut<'w, UiState>,
    pub net: ResMut<'w, Net>,
    pub gv: ResMut<'w, GameView>,
    pub i18n: Res<'w, I18n>,
    pub pres: Res<'w, Presentation>,
    pub host: Option<Res<'w, HostRuntime>>,
    pub draft: ResMut<'w, CharacterDraft>,
    pub catalog: Res<'w, CatalogRes>,
    pub next: ResMut<'w, NextState<AppState>>,
}

pub fn game_ui(
    mut contexts: EguiContexts,
    mut g: GameUi,
    mut commands: Commands,
    mut themed: Local<Option<f32>>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    if *themed != Some(g.ui.ui_scale) {
        theme::apply(&ctx, g.ui.ui_scale);
        *themed = Some(g.ui.ui_scale);
    }
    let Some(view) = g.gv.view.clone() else {
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

    // ---------------- top bar
    egui::Panel::top("top")
        .frame(theme::panel_frame().inner_margin(egui::Margin::symmetric(10, 4)))
        .show(&mut root, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&view.campaign_name)
                        .strong()
                        .color(theme::WOOD)
                        .size(17.0),
                );
                if let Some(r) = &view.room {
                    ui.label(format!("· {}", r.name));
                    if dm && !r.is_party_room {
                        ui.label(
                            RichText::new(g.i18n.t("game.editing_other_room")).color(theme::BLOOD),
                        );
                    }
                }
                if let Some(c) = &view.combat {
                    ui.label(
                        RichText::new(format!("⚔ {} {}", g.i18n.t("combat.round"), c.round))
                            .color(theme::BLOOD)
                            .strong(),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(g.i18n.t("game.leave")).clicked() {
                        commands.remove_resource::<HostRuntime>();
                        g.net.client = None;
                        g.net.you = None;
                        g.net.status.clear();
                        *g.gv = GameView::default();
                        g.next.set(AppState::MainMenu);
                    }
                    if g.net.client.is_none() && ui.button(g.i18n.t("game.reconnect")).clicked() {
                        crate::hosting::reconnect(&mut g.net);
                    }
                    if dm {
                        if ui.button(g.i18n.t("game.save")).clicked() {
                            let label = std::mem::take(&mut g.ui.save_label);
                            g.net.save(label);
                            g.gv.toast(g.i18n.t("game.saving"));
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut g.ui.save_label)
                                .hint_text(g.i18n.t("game.save_label"))
                                .desired_width(120.0),
                        );
                        if let Some(h) = &g.host {
                            ui.label(
                                RichText::new(format!(
                                    "{} {} · {} {}",
                                    g.i18n.t("game.code"),
                                    h.handle.session_code,
                                    g.i18n.t("game.port"),
                                    h.handle.local_addr.port()
                                ))
                                .strong()
                                .color(theme::BLOOD),
                            );
                        }
                    }
                    ui.checkbox(&mut g.ui.show_grid, g.i18n.t("game.grid"));
                });
            });
        });

    // ---------------- bottom: dice + log + chat
    egui::Panel::bottom("bottom")
        .resizable(true)
        .default_size(170.0)
        .frame(theme::panel_frame())
        .show(&mut root, |ui| {
            ui.columns(2, |cols| {
                dice_panel(&mut cols[0], &mut g);
                log_panel(&mut cols[1], &mut g, &view);
            });
        });

    // ---------------- left: DM tools
    if dm {
        egui::Panel::left("dm")
            .resizable(true)
            .default_size(290.0)
            .frame(theme::panel_frame())
            .show(&mut root, |ui| {
                ui.horizontal(|ui| {
                    for (tab, key) in [
                        (DmTab::Editor, "dm.editor"),
                        (DmTab::World, "dm.world"),
                        (DmTab::Monsters, "dm.monsters"),
                        (DmTab::Session, "dm.session"),
                    ] {
                        ui.selectable_value(&mut g.ui.dm_tab, tab, g.i18n.t(key));
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| match g.ui.dm_tab {
                    DmTab::Editor => editor_tab(ui, &mut g, &view),
                    DmTab::World => world_tab(ui, &mut g, &view),
                    DmTab::Monsters => monsters_tab(ui, &mut g, &view),
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
            egui::ScrollArea::vertical().show(ui, |ui| right_panel(ui, &mut g, &view));
        });

    overlays(&ctx, &mut g, &view);
    Ok(())
}

fn dice_panel(ui: &mut Ui, g: &mut GameUi) {
    theme::heading(ui, g.i18n.t("dice.title"));
    ui.horizontal_wrapped(|ui| {
        for s in [4, 6, 8, 10, 12, 20, 100] {
            if ui.button(format!("d{s}")).clicked() {
                let (mode, private) = (
                    if s == 20 {
                        g.ui.dice_mode
                    } else {
                        RollMode::Normal
                    },
                    g.ui.private_roll,
                );
                g.net.cmd(Command::RollDice {
                    expr: format!("1d{s}"),
                    mode,
                    private,
                    label: String::new(),
                });
            }
        }
    });
    ui.horizontal(|ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(&mut g.ui.dice_expr)
                .desired_width(110.0)
                .hint_text("2d6+3"),
        );
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button(g.i18n.t("dice.roll")).clicked() || enter {
            let (expr, mode, private) = (g.ui.dice_expr.clone(), g.ui.dice_mode, g.ui.private_roll);
            g.net.cmd(Command::RollDice {
                expr,
                mode,
                private,
                label: String::new(),
            });
        }
    });
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut g.ui.dice_mode,
            RollMode::Normal,
            g.i18n.t("dice.normal"),
        );
        ui.selectable_value(
            &mut g.ui.dice_mode,
            RollMode::Advantage,
            g.i18n.t("dice.advantage"),
        );
        ui.selectable_value(
            &mut g.ui.dice_mode,
            RollMode::Disadvantage,
            g.i18n.t("dice.disadvantage"),
        );
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
            ui.label(
                RichText::new(format!("🎲 {shown}"))
                    .size(28.0)
                    .strong()
                    .color(theme::BLOOD),
            );
            ui.vertical(|ui| {
                ui.label(RichText::new(who).strong());
                ui.label(RichText::new(detail).small());
            });
        });
    }
}

fn log_panel(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    theme::heading(ui, g.i18n.t("log.title"));
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .max_height(ui.available_height() - 34.0)
        .show(ui, |ui| {
            for l in view.log.iter().rev().take(80).rev() {
                ui.label(RichText::new(l).small());
            }
            for (from, text) in g.pres.chat.iter().rev().take(20).rev() {
                ui.label(
                    RichText::new(format!("💬 {from}: {text}"))
                        .small()
                        .color(theme::MANA),
                );
            }
        });
    ui.horizontal(|ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(&mut g.ui.chat)
                .hint_text(g.i18n.t("chat.hint"))
                .desired_width(ui.available_width() - 60.0),
        );
        if (ui.button(g.i18n.t("chat.send")).clicked()
            || resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            && !g.ui.chat.trim().is_empty()
        {
            let text = std::mem::take(&mut g.ui.chat);
            g.net.cmd(Command::Chat { text });
        }
    });
}

fn tool_button(ui: &mut Ui, g: &mut GameUi, tool: Tool, key: &str) {
    if ui
        .selectable_label(g.ui.tool == tool, g.i18n.t(key))
        .clicked()
    {
        g.ui.tool = tool;
    }
}

fn editor_tab(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let Some(room) = &view.room else { return };
    let room_id = room.id;
    theme::heading(ui, g.i18n.t("editor.tools"));
    ui.horizontal_wrapped(|ui| {
        tool_button(ui, g, Tool::Play, "tool.play");
        tool_button(ui, g, Tool::Pencil, "tool.pencil");
        tool_button(ui, g, Tool::Eraser, "tool.eraser");
        tool_button(ui, g, Tool::Rect, "tool.rect");
        tool_button(ui, g, Tool::Fill, "tool.fill");
        tool_button(ui, g, Tool::Object, "tool.object");
        tool_button(ui, g, Tool::RemoveObject, "tool.remove_object");
        tool_button(ui, g, Tool::Entry, "tool.entry");
        tool_button(ui, g, Tool::Light, "tool.light");
    });
    ui.horizontal(|ui| {
        if ui.button(g.i18n.t("editor.undo")).clicked() {
            g.net.cmd(Command::Dm(DmCommand::Undo { room: room_id }));
        }
        if ui.button(g.i18n.t("editor.redo")).clicked() {
            g.net.cmd(Command::Dm(DmCommand::Redo { room: room_id }));
        }
    });
    theme::heading(ui, g.i18n.t("editor.layers"));
    if let Some(map) = g.gv.map.clone() {
        for l in &map.layers {
            ui.horizontal(|ui| {
                ui.radio_value(&mut g.ui.layer, l.kind, format!("{:?}", l.kind));
                let (mut vis, mut lock) = (l.visible, l.locked);
                let a = ui.checkbox(&mut vis, "👁").changed();
                let b = ui.checkbox(&mut lock, "🔒").changed();
                if a || b {
                    g.net.cmd(Command::Dm(DmCommand::SetLayer {
                        room: room_id,
                        layer: l.kind,
                        visible: vis,
                        locked: lock,
                    }));
                }
            });
        }
        ui.horizontal(|ui| {
            ui.label(g.i18n.t("editor.ambient"));
            for (amb, key) in [
                (AmbientLight::Bright, "light.bright"),
                (AmbientLight::Dim, "light.dim"),
                (AmbientLight::Darkness, "light.dark"),
            ] {
                if ui
                    .selectable_label(map.ambient == amb, g.i18n.t(key))
                    .clicked()
                {
                    g.net.cmd(Command::Dm(DmCommand::SetAmbient {
                        room: room_id,
                        ambient: amb,
                    }));
                }
            }
        });
    }
    if g.ui.tool == Tool::Light {
        ui.add(
            egui::Slider::new(&mut g.ui.light_bright, 0..=60)
                .step_by(5.0)
                .text(g.i18n.t("light.bright_ft")),
        );
        ui.add(
            egui::Slider::new(&mut g.ui.light_dim, 0..=60)
                .step_by(5.0)
                .text(g.i18n.t("light.dim_ft")),
        );
        if ui.button(g.i18n.t("light.clear")).clicked() {
            g.net
                .cmd(Command::Dm(DmCommand::ClearLights { room: room_id }));
        }
    }
    if g.ui.tool == Tool::Object {
        ui.checkbox(&mut g.ui.object_hidden, g.i18n.t("editor.hidden_object"));
    }
    theme::heading(ui, g.i18n.t("editor.assets"));
    ui.add(egui::TextEdit::singleline(&mut g.ui.asset_search).hint_text(g.i18n.t("editor.search")));
    let favorites = g.ui.favorites.clone();
    ui.horizontal_wrapped(|ui| {
        ui.label("★");
        for f in &favorites {
            if let Some(a) = g.catalog.0.get(f)
                && ui.selectable_label(g.ui.asset == a.id, &a.name).clicked()
            {
                select_asset(g, a.id.clone(), a.is_object);
            }
        }
    });
    let search = g.ui.asset_search.clone();
    for cat in [
        Category::Terrain,
        Category::Architecture,
        Category::Vegetation,
        Category::Furniture,
        Category::Natural,
        Category::Dungeon,
        Category::Fantasy,
    ] {
        let items: Vec<_> = g.catalog.0.search(&search, Some(cat)).cloned().collect();
        if items.is_empty() {
            continue;
        }
        egui::CollapsingHeader::new(format!("{cat:?} ({})", items.len()))
            .default_open(!search.is_empty())
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for a in items {
                        let [r, gg, b] = a.color;
                        let resp = ui.add(
                            egui::Button::new(RichText::new(&a.name).color(Color32::WHITE))
                                .fill(Color32::from_rgb(r, gg, b))
                                .selected(g.ui.asset == a.id),
                        );
                        if resp.clicked() {
                            select_asset(g, a.id.clone(), a.is_object);
                        }
                        if resp.secondary_clicked() {
                            if g.ui.favorites.contains(&a.id) {
                                g.ui.favorites.retain(|f| *f != a.id);
                            } else {
                                g.ui.favorites.push(a.id.clone());
                            }
                        }
                    }
                });
            });
    }
    ui.label(RichText::new(g.i18n.t("editor.favorites_help")).small());
}

fn select_asset(g: &mut GameUi, id: String, is_object: bool) {
    g.ui.asset = id;
    if is_object {
        g.ui.tool = Tool::Object;
    } else if matches!(
        g.ui.tool,
        Tool::Play | Tool::Object | Tool::RemoveObject | Tool::Spawn | Tool::Entry | Tool::Light
    ) {
        g.ui.tool = Tool::Pencil;
    }
    if !is_object && g.ui.layer == LayerKind::Furniture {
        g.ui.layer = LayerKind::Terrain;
    }
}

fn world_tab(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let party_room = view.dm.as_ref().map(|d| d.party_room);
    let viewing = view.room.as_ref().map(|r| r.id);
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
                        if e.from == v { "→" } else { "←" }
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
                .checkbox(&mut hidden, "👁‍🗨")
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
            if p.connected { "🟢" } else { "⚪" },
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
        theme::heading(ui, g.i18n.t("combat.initiative"));
        for e in &c.order {
            let active = c.active == Some(e.entity);
            let text = format!(
                "{} {} ({})",
                if active { "▶" } else { "  " },
                e.name,
                e.total
            );
            ui.label(if active {
                RichText::new(text).strong().color(theme::BLOOD)
            } else {
                RichText::new(text)
            });
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
    // Combat action bar for your active creature.
    if let Some(c) = &view.combat {
        if let (Some(turn), Some(active)) = (&c.turn, c.active) {
            let me = view.creatures.iter().find(|x| x.id == active).cloned();
            egui::Window::new(g.i18n.t("combat.your_turn"))
                .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -190.0))
                .collapsible(false)
                .resizable(false)
                .title_bar(true)
                .show(ctx, |ui| {
                    if let Some(me) = &me {
                        ui.label(RichText::new(&me.name).strong());
                        ui.label(format!(
                            "{}: {} ft · {}: {} · {}: {}",
                            g.i18n.t("combat.movement"),
                            turn.movement_left_ft,
                            g.i18n.t("combat.action"),
                            if turn.action_used { "✖" } else { "✔" },
                            g.i18n.t("combat.reaction"),
                            "✔"
                        ));
                        ui.horizontal_wrapped(|ui| {
                            for (i, a) in me.attacks.iter().enumerate() {
                                if ui
                                    .add_enabled(
                                        !turn.action_used,
                                        egui::Button::new(format!("⚔ {}", a.name)),
                                    )
                                    .on_hover_text(a.summary())
                                    .clicked()
                                {
                                    g.ui.targeting = Some(i);
                                    g.ui.selected = Some(me.id);
                                    g.ui.tool = Tool::Play;
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    !turn.action_used,
                                    egui::Button::new(g.i18n.t("action.dash")),
                                )
                                .clicked()
                            {
                                g.net.cmd(Command::Dash { entity: me.id });
                            }
                            if ui
                                .add_enabled(
                                    !turn.action_used,
                                    egui::Button::new(g.i18n.t("action.dodge")),
                                )
                                .clicked()
                            {
                                g.net.cmd(Command::Dodge { entity: me.id });
                            }
                            if ui
                                .add_enabled(
                                    !turn.action_used,
                                    egui::Button::new(g.i18n.t("action.disengage")),
                                )
                                .clicked()
                            {
                                g.net.cmd(Command::Disengage { entity: me.id });
                            }
                            if ui
                                .button(RichText::new(g.i18n.t("action.end_turn")).strong())
                                .clicked()
                            {
                                g.net.cmd(Command::EndTurn);
                                g.ui.targeting = None;
                            }
                        });
                        if g.ui.targeting.is_some() {
                            ui.label(
                                RichText::new(g.i18n.t("combat.choose_target")).color(theme::BLOOD),
                            );
                        }
                    }
                });
        }
        if let Some(p) = &c.your_reaction {
            let reactor = view.creatures.iter().find(|x| x.id == p.reactor).cloned();
            let mover = view
                .creatures
                .iter()
                .find(|x| x.id == p.mover)
                .map(|m| m.name.clone())
                .unwrap_or_default();
            egui::Window::new(g.i18n.t("reaction.title"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .collapsible(false)
                .show(ctx, |ui| {
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
    }
    // Character creation inside the game (player without a character).
    if g.ui.show_creator && !g.net.is_dm() {
        let mut open = true;
        let mut submit = false;
        egui::Window::new(g.i18n.t("game.create_character"))
            .open(&mut open)
            .default_width(860.0)
            .show(ctx, |ui| {
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
                            egui::Label::new(RichText::new(t).color(theme::PARCHMENT))
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
    if let Some((name, t)) = &g.pres.room_title {
        let a = (t.min(1.0) * 255.0) as u8;
        egui::Area::new("room-title".into())
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -120.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(name)
                        .size(40.0)
                        .color(Color32::from_rgba_unmultiplied(201, 162, 72, a))
                        .strong(),
                );
            });
    }
    if g.pres.your_turn_banner > 0.0 {
        egui::Area::new("turn-banner".into())
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -60.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(g.i18n.t("combat.your_turn"))
                        .size(34.0)
                        .color(theme::GOLD)
                        .strong(),
                );
            });
    }
    if g.net.you == Some(Principal::Dm) && g.ui.tool != Tool::Play {
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
