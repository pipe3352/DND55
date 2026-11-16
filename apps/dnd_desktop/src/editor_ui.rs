//! Map editor workspace (Dungeon Master only):
//! * **top toolbar** — tools with icons and number-key shortcuts, clipboard, undo/redo, save,
//!   preview, collision/light overlays, fit;
//! * **left palette** — real art thumbnails, search, favorites (right-click), recently used,
//!   categories, tool options;
//! * **right inspector** — the selected object's properties, or the room's properties;
//! * **bottom status bar** — layers (visibility, lock, order) and map information;
//! * on the map — placement ghost, minimap and an object context menu.

use crate::icons::Icon;
use crate::state::Tool;
use crate::theme;
use crate::ui_game::GameUi;
use bevy::prelude::Resource;
use bevy_egui::egui::{self, Color32, RichText, Ui, vec2};
use dnd_session::command::{Command, DmCommand};
use dnd_session::view::ClientView;
use dnd_world::catalog::Category;
use dnd_world::map::{AmbientLight, LayerKind};
use std::collections::HashMap;

/// Palette thumbnails generated from the same procedural art the map uses.
#[derive(Resource, Default)]
pub struct PaletteThumbs(HashMap<String, egui::TextureHandle>);

fn thumb(g: &mut GameUi, ctx: &egui::Context, id: &str) -> Option<egui::TextureHandle> {
    if let Some(t) = g.thumbs.0.get(id) {
        return Some(t.clone());
    }
    let def = g.catalog.0.get(id)?.clone();
    let img = if def.is_object {
        crate::terrain_art::object(&def.id, def.size.0, def.size.1, def.color)
    } else {
        crate::terrain_art::tile(&def.id, 0, def.color)
    };
    let (w, h) = (img.width() as usize, img.height() as usize);
    let data = img.data?;
    let mut ci = egui::ColorImage::from_rgba_unmultiplied([w, h], &data);
    if def.is_object {
        // Show objects on a parchment-dark tile so their outline reads.
        for p in ci.pixels.iter_mut() {
            if p.a() < 255 {
                let a = p.a() as f32 / 255.0;
                let bg = theme::PARCHMENT_DARK;
                *p = Color32::from_rgb(
                    (p.r() as f32 + bg.r() as f32 * (1.0 - a)) as u8,
                    (p.g() as f32 + bg.g() as f32 * (1.0 - a)) as u8,
                    (p.b() as f32 + bg.b() as f32 * (1.0 - a)) as u8,
                );
            }
        }
    }
    let tex = ctx.load_texture(format!("palette-{id}"), ci, egui::TextureOptions::LINEAR);
    g.thumbs.0.insert(id.to_string(), tex.clone());
    Some(tex)
}

pub const TOOLS: [(Tool, Icon, &str, &str); 10] = [
    (Tool::Select, Icon::Pointer, "tool.select", "1"),
    (Tool::Pencil, Icon::Brush, "tool.pencil", "2"),
    (Tool::Eraser, Icon::Eraser, "tool.eraser", "3"),
    (Tool::Rect, Icon::Square, "tool.rect", "4"),
    (Tool::Fill, Icon::Bucket, "tool.fill", "5"),
    (Tool::Object, Icon::Tree, "tool.object", "6"),
    (Tool::RemoveObject, Icon::Trash, "tool.remove_object", "7"),
    (Tool::Entry, Icon::Flag, "tool.entry", "8"),
    (Tool::Light, Icon::Torch, "tool.light", "9"),
    (Tool::Draw, Icon::Quill, "tool.draw", "0"),
];

pub fn toolbar(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let Some(room) = view.room.as_ref().map(|r| r.id) else {
        return;
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let play = g.ui.tool == Tool::Play;
        if theme::icon_button(
            ui,
            Icon::Sword,
            play,
            &format!("{} (Esc)", g.i18n.t("tool.play")),
        )
        .clicked()
        {
            g.ui.tool = Tool::Play;
        }
        ui.add_space(6.0);
        for (tool, icon, key, sc) in TOOLS {
            let sel = g.ui.tool == tool;
            if theme::icon_button(ui, icon, sel, &format!("{} ({sc})", g.i18n.t(key))).clicked() {
                g.ui.tool = tool;
            }
        }
        ui.separator();
        let has_sel = !g.ui.sel_objects.is_empty();
        if theme::icon_button(
            ui,
            Icon::Scroll,
            false,
            &format!("{} (Ctrl+C)", g.i18n.t("select.copy")),
        )
        .clicked()
            && has_sel
        {
            crate::ui_dm::copy_selection(g);
        }
        if theme::icon_button(
            ui,
            Icon::Plus,
            false,
            &format!("{} (Ctrl+V)", g.i18n.t("select.paste")),
        )
        .clicked()
        {
            let at = g.hover.cell.unwrap_or((1, 1));
            crate::ui_dm::paste_at(g, room, at);
        }
        if theme::icon_button(
            ui,
            Icon::Undo,
            false,
            &format!("{} (Ctrl+Z)", g.i18n.t("editor.undo")),
        )
        .clicked()
        {
            g.net.cmd(Command::Dm(DmCommand::Undo { room }));
        }
        if theme::icon_button(
            ui,
            Icon::Redo,
            false,
            &format!("{} (Ctrl+Y)", g.i18n.t("editor.redo")),
        )
        .clicked()
        {
            g.net.cmd(Command::Dm(DmCommand::Redo { room }));
        }
        if theme::icon_button(
            ui,
            Icon::Seal,
            false,
            &format!("{} (Ctrl+S)", g.i18n.t("game.save")),
        )
        .clicked()
        {
            g.net.save(String::new());
            g.gv.toast(g.i18n.t("game.saving"));
        }
        ui.separator();
        let grid = g.ui.show_grid;
        if theme::icon_button(ui, Icon::Grid, grid, g.i18n.t("game.grid")).clicked() {
            g.ui.show_grid = !grid;
        }
        let col = g.ui.show_collision;
        if theme::icon_button(ui, Icon::Shield, col, g.i18n.t("editor.collision_overlay")).clicked()
        {
            g.ui.show_collision = !col;
        }
        let lr = g.ui.show_light_radii;
        if theme::icon_button(ui, Icon::Sun, lr, g.i18n.t("editor.light_overlay")).clicked() {
            g.ui.show_light_radii = !lr;
        }
        if theme::icon_button(ui, Icon::Fit, false, g.i18n.t("graph.fit")).clicked() {
            g.fit.request = true;
        }
        ui.separator();
        if theme::icon_button(
            ui,
            Icon::Play,
            g.ui.editor_preview,
            g.i18n.t("editor.preview"),
        )
        .clicked()
        {
            g.ui.editor_preview = !g.ui.editor_preview;
            if g.ui.editor_preview {
                g.ui.tool = Tool::Play;
            }
        }
        ui.label(RichText::new(g.i18n.t(tool_label(g.ui.tool))).color(theme::GOLD_BRIGHT));
    });
}

fn tool_label(t: Tool) -> &'static str {
    TOOLS
        .iter()
        .find(|x| x.0 == t)
        .map(|x| x.2)
        .unwrap_or("tool.play")
}

fn select_asset(g: &mut GameUi, id: &str, is_object: bool) {
    g.ui.asset = id.to_string();
    g.ui.recent_assets.retain(|r| r != id);
    g.ui.recent_assets.insert(0, id.to_string());
    g.ui.recent_assets.truncate(8);
    if is_object {
        g.ui.tool = Tool::Object;
    } else {
        if !matches!(g.ui.tool, Tool::Pencil | Tool::Rect | Tool::Fill) {
            g.ui.tool = Tool::Pencil;
        }
        if g.ui.layer == LayerKind::Furniture {
            g.ui.layer = LayerKind::Terrain;
        }
    }
}

fn asset_grid(ui: &mut Ui, g: &mut GameUi, ids: &[String]) {
    let ctx = ui.ctx().clone();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        for id in ids {
            let Some(def) = g.catalog.0.get(id).cloned() else {
                continue;
            };
            let Some(tex) = thumb(g, &ctx, id) else {
                continue;
            };
            let selected = g.ui.asset == *id;
            let (rect, resp) = ui.allocate_exact_size(vec2(50.0, 50.0), egui::Sense::click());
            let p = ui.painter();
            p.image(
                tex.id(),
                rect.shrink(3.0),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            let stroke = if selected {
                egui::Stroke::new(3.0, theme::GOLD_BRIGHT)
            } else if resp.hovered() {
                egui::Stroke::new(2.0, theme::GOLD)
            } else {
                egui::Stroke::new(1.0, theme::BRONZE)
            };
            p.rect_stroke(rect.shrink(1.5), 3.0, stroke, egui::StrokeKind::Inside);
            if g.ui.favorites.contains(id) {
                crate::icons::paint(
                    p,
                    egui::Rect::from_min_size(
                        rect.right_top() + vec2(-15.0, 2.0),
                        vec2(13.0, 13.0),
                    ),
                    Icon::Star,
                );
            }
            let resp = resp.on_hover_text(format!(
                "{}\n{}",
                def.name,
                g.i18n.t("editor.favorites_help")
            ));
            if resp.clicked() {
                select_asset(g, id, def.is_object);
            }
            if resp.secondary_clicked() {
                if g.ui.favorites.contains(id) {
                    g.ui.favorites.retain(|f| f != id);
                } else {
                    g.ui.favorites.push(id.clone());
                }
            }
        }
    });
}

/// Left sidebar in the Editor tab.
pub fn palette(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let Some(room) = view.room.as_ref().map(|r| r.id) else {
        return;
    };
    // Tool options first: they change with the active tool.
    match g.ui.tool {
        Tool::Light => {
            theme::heading(ui, g.i18n.t("tool.light"));
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
                g.net.cmd(Command::Dm(DmCommand::ClearLights { room }));
            }
        }
        Tool::Object => {
            ui.checkbox(&mut g.ui.object_hidden, g.i18n.t("editor.hidden_object"));
        }
        Tool::Draw => crate::ui_dm::draw_options(ui, g, room, true),
        Tool::Pencil | Tool::Rect | Tool::Fill | Tool::Eraser => {
            ui.horizontal_wrapped(|ui| {
                ui.label(g.i18n.t("editor.paint_layer"));
                for k in [
                    LayerKind::Terrain,
                    LayerKind::FloorDecoration,
                    LayerKind::Walls,
                    LayerKind::Overhead,
                ] {
                    let name = crate::ui_dm::layer_label(g, k);
                    ui.selectable_value(&mut g.ui.layer, k, name);
                }
            });
        }
        _ => {}
    }
    theme::heading(ui, g.i18n.t("editor.assets"));
    ui.add(
        egui::TextEdit::singleline(&mut g.ui.asset_search)
            .hint_text(g.i18n.t("editor.search"))
            .desired_width(f32::INFINITY),
    );
    let search = g.ui.asset_search.clone();
    if search.is_empty() {
        if !g.ui.favorites.is_empty() {
            ui.label(theme::bold(format!("★ {}", g.i18n.t("editor.favorites"))));
            let favs = g.ui.favorites.clone();
            asset_grid(ui, g, &favs);
        }
        if !g.ui.recent_assets.is_empty() {
            ui.label(theme::bold(g.i18n.t("editor.recent")));
            let recent = g.ui.recent_assets.clone();
            asset_grid(ui, g, &recent);
        }
    }
    for (cat, key) in [
        (Category::Terrain, "cat.terrain"),
        (Category::Architecture, "cat.architecture"),
        (Category::Vegetation, "cat.vegetation"),
        (Category::Furniture, "cat.furniture"),
        (Category::Natural, "cat.natural"),
        (Category::Dungeon, "cat.dungeon"),
        (Category::Fantasy, "cat.fantasy"),
    ] {
        let items: Vec<String> = g
            .catalog
            .0
            .search(&search, Some(cat))
            .map(|a| a.id.clone())
            .collect();
        if items.is_empty() {
            continue;
        }
        egui::CollapsingHeader::new(theme::gothic(
            format!("{} ({})", g.i18n.t(key), items.len()),
            17.0,
        ))
        .id_salt(key)
        .default_open(!search.is_empty() || cat == Category::Terrain)
        .show(ui, |ui| asset_grid(ui, g, &items));
    }
}

/// Right sidebar in the Editor tab: selection properties or room properties.
pub fn inspector(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let Some(r) = &view.room else { return };
    let room = r.id;
    if g.ui.tool == Tool::Select && !g.ui.sel_objects.is_empty() {
        crate::ui_dm::selection_panel(ui, g, room);
        return;
    }
    if g.ui.tool == Tool::Select {
        crate::ui_dm::selection_panel(ui, g, room);
    }
    theme::heading(ui, g.i18n.t("editor.room_props"));
    ui.label(theme::display(&r.name, 24.0).color(theme::WOOD));
    if let Some(map) = g.gv.map.clone() {
        ui.label(
            RichText::new(format!(
                "{} × {} · {} {} · {} {}",
                map.width,
                map.height,
                map.objects.len(),
                g.i18n.t("editor.objects"),
                r.lights.len(),
                g.i18n.t("editor.lights")
            ))
            .small(),
        );
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
                    g.net
                        .cmd(Command::Dm(DmCommand::SetAmbient { room, ambient: amb }));
                }
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label(format!(
            "{}: ({}, {})",
            g.i18n.t("tool.entry"),
            r.entry.0,
            r.entry.1
        ));
        if ui.small_button(g.i18n.t("editor.set_entry")).clicked() {
            g.ui.tool = Tool::Entry;
        }
    });
    crate::ui_dm::room_audio(ui, g, view);
    ui.add_space(6.0);
    ui.label(
        RichText::new(g.i18n.t("editor.inspector_help"))
            .small()
            .italics(),
    );
}

/// Bottom status strip: layers and map information.
pub fn status_bar(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let Some(r) = &view.room else { return };
    let room = r.id;
    let Some(map) = g.gv.map.clone() else { return };
    ui.horizontal(|ui| {
        crate::icons::paint(
            ui.painter(),
            egui::Rect::from_min_size(ui.cursor().min, vec2(20.0, 20.0)),
            Icon::Layers,
        );
        ui.add_space(24.0);
        let n = map.layers.len();
        for (i, l) in map.layers.iter().enumerate() {
            let name = crate::ui_dm::layer_label(g, l.kind);
            let active = g.ui.layer == l.kind;
            egui::Frame::new()
                .fill(if active {
                    theme::GOLD.gamma_multiply(0.45)
                } else {
                    Color32::TRANSPARENT
                })
                .stroke(egui::Stroke::new(1.0, theme::BRONZE))
                .corner_radius(3)
                .inner_margin(egui::Margin::symmetric(4, 1))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(active, RichText::new(&name).small())
                            .clicked()
                        {
                            g.ui.layer = l.kind;
                        }
                        let (mut vis, mut lock) = (l.visible, l.locked);
                        let a = ui
                            .checkbox(&mut vis, "")
                            .on_hover_text(g.i18n.t("layer.visible"))
                            .changed();
                        let b = ui
                            .checkbox(&mut lock, "")
                            .on_hover_text(g.i18n.t("layer.locked"))
                            .changed();
                        if a || b {
                            g.net.cmd(Command::Dm(DmCommand::SetLayer {
                                room,
                                layer: l.kind,
                                visible: vis,
                                locked: lock,
                            }));
                        }
                        if i > 0
                            && ui
                                .small_button("⏶")
                                .on_hover_text(g.i18n.t("layer.up"))
                                .clicked()
                        {
                            g.net.cmd(Command::Dm(DmCommand::MoveLayer {
                                room,
                                layer: l.kind,
                                up: true,
                            }));
                        }
                        if i + 1 < n
                            && ui
                                .small_button("⏷")
                                .on_hover_text(g.i18n.t("layer.down"))
                                .clicked()
                        {
                            g.net.cmd(Command::Dm(DmCommand::MoveLayer {
                                room,
                                layer: l.kind,
                                up: false,
                            }));
                        }
                    });
                });
        }
        ui.separator();
        let cursor = g.hover.cell.map(|c| {
            let terrain = map
                .layers
                .iter()
                .find(|l| l.kind == LayerKind::Terrain)
                .and_then(|l| {
                    let v = l.tiles[(c.1 * map.width + c.0) as usize];
                    (v != 0)
                        .then(|| map.palette.get(v as usize - 1))
                        .flatten()
                        .and_then(|p| g.catalog.0.get(&p.asset_id))
                        .map(|a| a.name.clone())
                });
            format!("({}, {}) {}", c.0, c.1, terrain.unwrap_or_default())
        });
        ui.label(
            RichText::new(format!(
                "{} × {} · {}",
                map.width,
                map.height,
                cursor.unwrap_or_default()
            ))
            .small(),
        );
        ui.label(
            RichText::new(g.i18n.t("editor.art_status"))
                .small()
                .italics(),
        )
        .on_hover_text(g.i18n.t("editor.art_status_help"));
    });
}

/// Room minimap (DM): click to move the camera there.
pub fn minimap(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    let (Some(r), Some(dm)) = (&view.room, &view.dm) else {
        return;
    };
    let Some(card) = dm.room_cards.iter().find(|c| c.id == r.id) else {
        return;
    };
    let tex = crate::graph_editor::thumbnail_tex(&mut g.graph, ctx, card);
    let w = 180.0;
    let h = w * card.thumb_h as f32 / card.thumb_w.max(1) as f32;
    egui::Area::new("editor-minimap".into())
        .anchor(egui::Align2::RIGHT_BOTTOM, vec2(-340.0, -190.0))
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(vec2(w, h.min(140.0)), egui::Sense::click());
            let p = ui.painter();
            p.rect_filled(rect.expand(4.0), 4.0, theme::WOOD);
            p.image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            p.rect_stroke(
                rect.expand(4.0),
                4.0,
                egui::Stroke::new(1.5, theme::GOLD),
                egui::StrokeKind::Inside,
            );
            if let Some(cam) = g.fit.camera_view {
                let (mw, mh) = (
                    card.size.0 as f32 * crate::state::CELL,
                    card.size.1 as f32 * crate::state::CELL,
                );
                let to = |x: f32, y: f32| {
                    egui::pos2(
                        rect.left() + x / mw * rect.width(),
                        rect.top() + -y / mh * rect.height(),
                    )
                };
                let v =
                    egui::Rect::from_two_pos(to(cam.0, cam.1), to(cam.2, cam.3)).intersect(rect);
                p.rect_stroke(
                    v,
                    0.0,
                    egui::Stroke::new(1.5, theme::IVORY),
                    egui::StrokeKind::Inside,
                );
            }
            if resp.clicked()
                && let Some(pos) = resp.interact_pointer_pos()
            {
                let (mw, mh) = (
                    card.size.0 as f32 * crate::state::CELL,
                    card.size.1 as f32 * crate::state::CELL,
                );
                let fx = (pos.x - rect.left()) / rect.width();
                let fy = (pos.y - rect.top()) / rect.height();
                g.fit.goto = Some((fx * mw, -fy * mh));
            }
            resp.on_hover_text(g.i18n.t("editor.minimap_help"));
        });
}

/// Right-click menu on map objects (Select tool).
pub fn context_menu(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    let Some(((x, y), obj)) = g.ui.map_context else {
        return;
    };
    let Some(room) = view.room.as_ref().map(|r| r.id) else {
        return;
    };
    let mut close = false;
    egui::Area::new("map-context".into())
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(x, y))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(170.0);
                if !g.ui.sel_objects.contains(&obj) {
                    g.ui.sel_objects = vec![obj];
                }
                for (key, action) in [
                    ("select.copy", 0),
                    ("select.duplicate", 1),
                    ("select.rotate", 2),
                    ("props.title", 3),
                    ("select.delete", 4),
                ] {
                    if ui.button(g.i18n.t(key)).clicked() {
                        match action {
                            0 => crate::ui_dm::copy_selection(g),
                            1 => crate::ui_dm::duplicate(g, room),
                            2 => crate::ui_dm::rotate(g, room),
                            3 => {}
                            _ => {
                                let objects = std::mem::take(&mut g.ui.sel_objects);
                                g.net
                                    .cmd(Command::Dm(DmCommand::RemoveObjects { room, objects }));
                            }
                        }
                        close = true;
                    }
                }
            });
        });
    if close
        || ctx.input(|i| i.pointer.any_pressed() && !i.pointer.secondary_pressed())
            && !ctx.is_pointer_over_egui()
    {
        g.ui.map_context = None;
    }
}
