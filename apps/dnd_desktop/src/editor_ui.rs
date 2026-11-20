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
        crate::pixel_objects::image(&def.id, def.size.0, def.size.1, def.color)
    } else {
        crate::pixel_tiles::tile_image(&def.id, 0, def.color)
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
    let tex = ctx.load_texture(format!("palette-{id}"), ci, egui::TextureOptions::NEAREST);
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
            let (rect, resp) = ui.allocate_exact_size(vec2(62.0, 62.0), egui::Sense::click());
            ui.painter()
                .rect_filled(rect.shrink(1.0), 4.0, Color32::from_black_alpha(60));
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
            p.rect_stroke(rect.shrink(1.5), 4.0, stroke, egui::StrokeKind::Inside);
            if selected {
                p.rect_stroke(
                    rect.expand(1.0),
                    5.0,
                    egui::Stroke::new(2.0, theme::GOLD_BRIGHT.gamma_multiply(0.35)),
                    egui::StrokeKind::Outside,
                );
            }
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
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{}: {}°",
                    g.i18n.t("editor.place_rotation"),
                    g.ui.place_rotation as u32 * 90
                ));
                if ui
                    .small_button("-90°")
                    .on_hover_text(g.i18n.t("props.rotate_ccw"))
                    .clicked()
                {
                    g.ui.place_rotation = (g.ui.place_rotation + 3) % 4;
                }
                if ui
                    .small_button("+90°")
                    .on_hover_text(g.i18n.t("props.rotate"))
                    .clicked()
                {
                    g.ui.place_rotation = (g.ui.place_rotation + 1) % 4;
                }
            });
        }
        Tool::Eraser => {
            crate::chrome::center_heading(ui, g.i18n.t("eraser.title"), theme::GOLD_BRIGHT);
            ui.horizontal_wrapped(|ui| {
                for (k, key) in [
                    "eraser.layer",
                    "eraser.category",
                    "eraser.objects",
                    "eraser.creatures",
                    "eraser.lights",
                    "eraser.drawings",
                    "eraser.all",
                ]
                .iter()
                .enumerate()
                {
                    ui.selectable_value(&mut g.ui.erase_kind, k as u8, g.i18n.t(key));
                }
            });
            match g.ui.erase_kind {
                0 => {
                    ui.horizontal_wrapped(|ui| {
                        for k in [
                            LayerKind::Terrain,
                            LayerKind::FloorDecoration,
                            LayerKind::Walls,
                            LayerKind::Furniture,
                            LayerKind::Overhead,
                        ] {
                            let name = crate::ui_dm::layer_label(g, k);
                            ui.selectable_value(&mut g.ui.layer, k, name);
                        }
                    });
                }
                1 => {
                    ui.horizontal_wrapped(|ui| {
                        for (cat, key) in [
                            (Category::Furniture, "cat.furniture"),
                            (Category::Natural, "cat.natural"),
                            (Category::Dungeon, "cat.dungeon"),
                            (Category::Architecture, "cat.architecture"),
                            (Category::Fantasy, "cat.fantasy"),
                            (Category::Vegetation, "cat.vegetation"),
                        ] {
                            ui.selectable_value(&mut g.ui.erase_cat, cat, g.i18n.t(key));
                        }
                    });
                }
                _ => {}
            }
            ui.add(egui::Slider::new(&mut g.ui.erase_brush, 1..=7).text(g.i18n.t("eraser.brush")));
            ui.checkbox(&mut g.ui.erase_protect, g.i18n.t("eraser.protect"))
                .on_hover_text(g.i18n.t("eraser.protect_help"));
            ui.label(RichText::new(g.i18n.t("eraser.help")).small().italics());
        }
        Tool::Draw => crate::ui_dm::draw_options(ui, g, room, true),
        _ => {}
    }
    // Paint mode: which map layer the brush tools paint.
    crate::chrome::center_heading(ui, g.i18n.t("editor.paint_mode"), theme::GOLD_BRIGHT);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let tw = ((ui.available_width() - 18.0) / 4.0).clamp(60.0, 110.0);
        let painting = matches!(
            g.ui.tool,
            Tool::Pencil | Tool::Rect | Tool::Fill | Tool::Eraser
        );
        for (k, icon) in [
            (LayerKind::Terrain, Icon::Map),
            (LayerKind::FloorDecoration, Icon::Leaf),
            (LayerKind::Walls, Icon::Square),
            (LayerKind::Overhead, Icon::Layers),
        ] {
            let name = crate::ui_dm::layer_label(g, k);
            if crate::chrome::tile_button(
                ui,
                icon,
                &name,
                painting && g.ui.layer == k,
                vec2(tw, 62.0),
            )
            .clicked()
            {
                g.ui.layer = k;
                if !painting {
                    g.ui.tool = Tool::Pencil;
                }
            }
        }
    });
    ui.add_space(6.0);
    crate::chrome::center_heading(ui, g.i18n.t("editor.assets"), theme::GOLD_BRIGHT);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut g.ui.asset_search)
                .hint_text(g.i18n.t("editor.search_assets"))
                .font(egui::FontId::new(16.0, egui::FontFamily::Proportional))
                .desired_width(ui.available_width() - 44.0),
        );
        let fav = g.ui.asset_special == 1;
        if theme::icon_button(ui, Icon::Star, fav, g.i18n.t("editor.favorites")).clicked() {
            g.ui.asset_special = if fav { 0 } else { 1 };
        }
    });
    ui.add_space(4.0);
    const CATS: [(Category, &str, Icon); 7] = [
        (Category::Terrain, "cat.terrain", Icon::Map),
        (Category::Architecture, "cat.architecture", Icon::Door),
        (Category::Vegetation, "cat.vegetation", Icon::Leaf),
        (Category::Furniture, "cat.furniture", Icon::Chest),
        (Category::Natural, "cat.natural", Icon::Tree),
        (Category::Dungeon, "cat.dungeon", Icon::Skull),
        (Category::Fantasy, "cat.fantasy", Icon::Star),
    ];
    let search = g.ui.asset_search.clone();
    if g.ui.asset_special == 0 && g.ui.asset_cat.is_none() {
        g.ui.asset_cat = Some(Category::Terrain);
    }
    ui.horizontal_top(|ui| {
        // Left column: favorites, recent and the categories with their counts.
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() * 0.38).clamp(140.0, 190.0));
            if crate::chrome::list_row(
                ui,
                Icon::Star,
                g.i18n.t("editor.favorites"),
                g.ui.asset_special == 1,
                false,
            )
            .clicked()
            {
                g.ui.asset_special = 1;
            }
            if crate::chrome::list_row(
                ui,
                Icon::Hourglass,
                g.i18n.t("editor.recent"),
                g.ui.asset_special == 2,
                false,
            )
            .clicked()
            {
                g.ui.asset_special = 2;
            }
            for (cat, key, icon) in CATS {
                let n = g.catalog.0.search("", Some(cat)).count();
                if n == 0 {
                    continue;
                }
                let sel = g.ui.asset_special == 0 && g.ui.asset_cat == Some(cat);
                if crate::chrome::list_row(ui, icon, &format!("{} ({n})", g.i18n.t(key)), sel, true)
                    .clicked()
                {
                    g.ui.asset_special = 0;
                    g.ui.asset_cat = Some(cat);
                    g.ui.asset_group = None;
                }
            }
        });
        ui.add_space(6.0);
        // Right column: the assets of the selected list (or every search match).
        ui.vertical(|ui| {
            let ids: Vec<String> = if !search.is_empty() {
                g.catalog
                    .0
                    .search(&search, None)
                    .map(|a| a.id.clone())
                    .collect()
            } else {
                match g.ui.asset_special {
                    1 => g.ui.favorites.clone(),
                    2 => g.ui.recent_assets.clone(),
                    _ => g
                        .catalog
                        .0
                        .search("", g.ui.asset_cat)
                        .map(|a| a.id.clone())
                        .collect(),
                }
            };
            if ids.is_empty() {
                ui.label(
                    theme::italic(g.i18n.t(if g.ui.asset_special == 1 {
                        "editor.favorites_help"
                    } else {
                        "chars.none"
                    }))
                    .size(14.0),
                );
                return;
            }
            // Sub-group chips (Trees, Plants, Rocks…) for the selected category.
            let mut groups: Vec<String> = Vec::new();
            for id in &ids {
                if let Some(a) = g.catalog.0.get(id) {
                    let grp = a.tags.first().cloned().unwrap_or_default();
                    if !groups.contains(&grp) {
                        groups.push(grp);
                    }
                }
            }
            let show_groups = search.is_empty() && g.ui.asset_special == 0 && groups.len() > 1;
            if show_groups {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    if ui
                        .selectable_label(
                            g.ui.asset_group.is_none(),
                            RichText::new(g.i18n.t("editor.all")).small(),
                        )
                        .clicked()
                    {
                        g.ui.asset_group = None;
                    }
                    for grp in &groups {
                        let sel = g.ui.asset_group.as_deref() == Some(grp.as_str());
                        if ui
                            .selectable_label(sel, RichText::new(group_label(g, grp)).small())
                            .clicked()
                        {
                            g.ui.asset_group = (!sel).then(|| grp.clone());
                        }
                    }
                });
                ui.add_space(4.0);
            }
            let ids: Vec<String> = ids
                .into_iter()
                .filter(|id| {
                    !show_groups
                        || g.ui.asset_group.as_ref().is_none_or(|sel| {
                            g.catalog.0.get(id).and_then(|a| a.tags.first()) == Some(sel)
                        })
                })
                .collect();
            asset_grid(ui, g, &ids);
        });
    });
}

/// Sub-group name ("tree" → "Trees"); unknown groups show the tag itself.
fn group_label(g: &GameUi, grp: &str) -> String {
    if grp.is_empty() {
        return g.i18n.t("group.other").to_string();
    }
    let key = format!("group.{grp}");
    let t = g.i18n.t(&key);
    if t == key {
        grp.to_string()
    } else {
        t.to_string()
    }
}

/// Right sidebar in the Editor tab: selection properties or room properties.
pub fn inspector(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let Some(r) = &view.room else { return };
    let room = r.id;
    if g.ui.tool == Tool::Select {
        crate::ui_dm::selection_panel(ui, g, room);
        if !g.ui.sel_objects.is_empty() {
            return;
        }
    }
    let panel = ui.max_rect();
    crate::chrome::archway(
        ui.painter(),
        egui::Rect::from_min_size(
            egui::pos2(panel.right() - 110.0, panel.top() - 4.0),
            vec2(100.0, 90.0),
        ),
    );
    ui.horizontal(|ui| {
        crate::chrome::diamond(ui.painter(), ui.cursor().min + vec2(6.0, 14.0), 4.0);
        ui.add_space(16.0);
        ui.label(theme::gothic(g.i18n.t("editor.room_details"), 26.0).color(theme::WOOD));
    });
    ui.add_space(6.0);
    let row = |ui: &mut Ui, icon: Icon, label: &str| {
        let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), egui::Sense::hover());
        crate::icons::paint(ui.painter(), r, icon);
        let label = label.trim_start_matches(|c: char| !c.is_alphanumeric());
        ui.label(RichText::new(label).size(16.0).color(theme::INK));
    };
    // Room name (Enter or leaving the field renames the room).
    ui.horizontal(|ui| row(ui, Icon::Quill, g.i18n.t("editor.room_name")));
    let mut name =
        g.ui.room_name_edit
            .clone()
            .unwrap_or_else(|| r.name.clone());
    let resp = ui.add(
        egui::TextEdit::singleline(&mut name)
            .font(egui::FontId::new(18.0, egui::FontFamily::Proportional))
            .desired_width(f32::INFINITY),
    );
    if resp.changed() {
        g.ui.room_name_edit = Some(name.clone());
    }
    if resp.lost_focus()
        && let Some(n) = g.ui.room_name_edit.take()
        && !n.trim().is_empty()
        && n != r.name
    {
        g.net.cmd(Command::Dm(DmCommand::RenameRoom {
            room,
            name: n.trim().to_string(),
            description: r.description.clone(),
        }));
    }
    ui.add_space(6.0);
    if let Some(map) = g.gv.map.clone() {
        ui.horizontal(|ui| {
            row(ui, Icon::Grid, g.i18n.t("editor.size"));
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("{} × {}", map.width, map.height))
                    .size(17.0)
                    .color(theme::INK),
            );
            ui.label(
                theme::italic(format!(
                    "({} {})",
                    map.objects.len(),
                    g.i18n.t("editor.objects")
                ))
                .color(theme::INK_SOFT),
            );
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| row(ui, Icon::Sun, g.i18n.t("editor.ambient")));
        let cur = match map.ambient {
            AmbientLight::Bright => 0,
            AmbientLight::Dim => 1,
            AmbientLight::Darkness => 2,
        };
        if let Some(i) = crate::chrome::segmented(
            ui,
            &[
                g.i18n.t("light.bright"),
                g.i18n.t("light.dim"),
                g.i18n.t("light.dark"),
            ],
            cur,
        ) {
            let ambient = [
                AmbientLight::Bright,
                AmbientLight::Dim,
                AmbientLight::Darkness,
            ][i];
            g.net
                .cmd(Command::Dm(DmCommand::SetAmbient { room, ambient }));
        }
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        row(ui, Icon::Flag, g.i18n.t("tool.entry"));
        ui.add_space(8.0);
        egui::Frame::new()
            .fill(theme::IVORY)
            .stroke(egui::Stroke::new(1.0, theme::BRONZE))
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(10, 3))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("({}, {})", r.entry.0, r.entry.1))
                        .size(16.0)
                        .color(theme::INK),
                )
            });
        if theme::icon_text_button(ui, Icon::Target, g.i18n.t("editor.set_entry"), true).clicked() {
            g.ui.tool = Tool::Entry;
        }
    });
    resize_section(ui, g, room);
    crate::journal_ui::room_notes(ui, g, view, room);
    ui.add_space(10.0);
    crate::chrome::center_heading(ui, g.i18n.t("audio.room"), theme::WOOD);
    room_audio_rows(ui, g, view);
    ui.add_space(6.0);
    egui::Frame::new()
        .fill(theme::PARCHMENT_DARK.gamma_multiply(0.5))
        .stroke(egui::Stroke::new(1.0, theme::BRONZE.gamma_multiply(0.6)))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), egui::Sense::hover());
                crate::chrome::info_badge(ui.painter(), r.center());
                ui.label(
                    theme::italic(g.i18n.t("audio.room_help"))
                        .size(14.0)
                        .color(theme::INK_SOFT),
                );
            });
        });
    ui.add_space(6.0);
    ui.label(
        RichText::new(g.i18n.t("editor.inspector_help"))
            .small()
            .italics(),
    );
    // Wax seal after the content (never over it).
    let (sr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), egui::Sense::hover());
    crate::chrome::wax_seal(
        ui.painter(),
        egui::pos2(sr.right() - 30.0, sr.top() + 26.0),
        22.0,
    );
}

/// Resize the map after creation: new size, anchor (3×3 grid), loss preview and confirmation.
fn resize_section(ui: &mut Ui, g: &mut GameUi, room: dnd_world::RoomId) {
    use dnd_world::map::Anchor;
    let Some(map) = g.gv.map.clone() else { return };
    egui::CollapsingHeader::new(theme::gothic(g.i18n.t("resize.title"), 18.0).color(theme::WOOD))
        .id_salt("resize-map")
        .show(ui, |ui| {
            if g.ui.resize_w == 0 {
                g.ui.resize_w = map.width;
                g.ui.resize_h = map.height;
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut g.ui.resize_w)
                        .range(1..=dnd_world::map::MAX_MAP_SIDE)
                        .prefix("w "),
                );
                ui.label("×");
                ui.add(
                    egui::DragValue::new(&mut g.ui.resize_h)
                        .range(1..=dnd_world::map::MAX_MAP_SIDE)
                        .prefix("h "),
                );
                ui.label(
                    RichText::new(format!(
                        "({} × {} → {} × {})",
                        map.width, map.height, g.ui.resize_w, g.ui.resize_h
                    ))
                    .small(),
                );
            });
            ui.horizontal(|ui| {
                ui.label(g.i18n.t("resize.anchor"));
                egui::Grid::new("anchor-grid")
                    .spacing([2.0, 2.0])
                    .show(ui, |ui| {
                        for (i, a) in Anchor::ALL.iter().enumerate() {
                            let sel = g.ui.resize_anchor == *a;
                            if ui
                                .add(
                                    egui::Button::new(if sel { "•" } else { " " })
                                        .selected(sel)
                                        .min_size(vec2(22.0, 20.0)),
                                )
                                .clicked()
                            {
                                g.ui.resize_anchor = *a;
                            }
                            if i % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
            });
            let rep = map.resize_report(g.ui.resize_w, g.ui.resize_h, g.ui.resize_anchor);
            let loses = rep.tiles_lost > 0 || !rep.objects_lost.is_empty();
            if loses {
                ui.label(
                    RichText::new(format!(
                        "{} {} · {} {}",
                        rep.tiles_lost,
                        g.i18n.t("resize.tiles_lost"),
                        rep.objects_lost.len(),
                        g.i18n.t("resize.objects_lost")
                    ))
                    .color(egui::Color32::from_rgb(170, 40, 40))
                    .strong(),
                );
                ui.checkbox(&mut g.ui.resize_confirm, g.i18n.t("resize.confirm"));
            }
            ui.horizontal(|ui| {
                let changed = (g.ui.resize_w, g.ui.resize_h) != (map.width, map.height);
                if ui
                    .add_enabled(
                        changed && (!loses || g.ui.resize_confirm),
                        egui::Button::new(g.i18n.t("resize.apply")),
                    )
                    .clicked()
                {
                    g.net.cmd(Command::Dm(DmCommand::ResizeMap {
                        room,
                        width: g.ui.resize_w,
                        height: g.ui.resize_h,
                        anchor: g.ui.resize_anchor,
                        confirm_loss: g.ui.resize_confirm,
                    }));
                    g.ui.resize_confirm = false;
                    g.ui.resize_w = 0;
                    g.fit.request = true;
                }
                if ui.small_button(g.i18n.t("resize.reset")).clicked() {
                    g.ui.resize_w = 0;
                }
            });
            ui.label(RichText::new(g.i18n.t("resize.help")).small().italics());
        });
}

/// Music and ambience pickers with a local preview button each.
fn room_audio_rows(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let Some(r) = &view.room else { return };
    let (mut music, mut amb) = (r.music.clone(), r.ambience.clone());
    let none = g.i18n.t("audio.none").to_string();
    let mut changed = false;
    for is_music in [true, false] {
        ui.horizontal(|ui| {
            let (icon, label, id) = if is_music {
                (Icon::Lute, "settings.music", "music")
            } else {
                (Icon::Leaf, "settings.ambience", "amb")
            };
            let (rr, _) = ui.allocate_exact_size(vec2(22.0, 22.0), egui::Sense::hover());
            crate::icons::paint(ui.painter(), rr, icon);
            ui.add_sized(
                vec2(84.0, 22.0),
                egui::Label::new(RichText::new(g.i18n.t(label)).size(16.0).color(theme::INK)),
            );
            let (value, tracks) = if is_music {
                (&mut music, crate::audio::MUSIC_TRACKS)
            } else {
                (&mut amb, crate::audio::AMBIENCE_TRACKS)
            };
            egui::ComboBox::from_id_salt(id)
                .width(ui.available_width() - 44.0)
                .selected_text(value.clone().unwrap_or(none.clone()))
                .show_ui(ui, |ui| {
                    changed |= ui.selectable_value(value, None, &none).changed();
                    for t in tracks {
                        changed |= ui
                            .selectable_value(value, Some(t.to_string()), *t)
                            .changed();
                    }
                });
            let slot = if is_music {
                &mut g.ui.audio_preview.0
            } else {
                &mut g.ui.audio_preview.1
            };
            let playing = slot.is_some();
            if theme::icon_button(ui, Icon::Play, playing, g.i18n.t("audio.preview")).clicked() {
                *slot = if playing { None } else { value.clone() };
            }
        });
    }
    if changed {
        g.ui.audio_preview = (None, None);
        g.net.cmd(Command::Dm(DmCommand::SetRoomAudio {
            room: r.id,
            music,
            ambience: amb,
        }));
    }
}

/// Status bar under the map: active layer, grid/snap, local object/effect filters, scale, cursor.
pub fn status_bar(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let Some(r) = &view.room else { return };
    let room = r.id;
    let Some(map) = g.gv.map.clone() else { return };
    ui.horizontal(|ui| {
        let (ir, _) = ui.allocate_exact_size(vec2(20.0, 20.0), egui::Sense::hover());
        crate::icons::paint(ui.painter(), ir, Icon::Layers);
        ui.label(g.i18n.t("editor.layers"));
        egui::ComboBox::from_id_salt("status-layer")
            .width(150.0)
            .selected_text(crate::ui_dm::layer_label(g, g.ui.layer))
            .show_ui(ui, |ui| {
                let n = map.layers.len();
                for (i, l) in map.layers.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let name = crate::ui_dm::layer_label(g, l.kind);
                        ui.selectable_value(&mut g.ui.layer, l.kind, name);
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
                }
            });
        ui.separator();
        ui.checkbox(&mut g.ui.show_grid, g.i18n.t("game.grid"));
        ui.checkbox(&mut g.ui.snap, g.i18n.t("editor.snap"))
            .on_hover_text(g.i18n.t("editor.snap_help"));
        ui.separator();
        let filter =
            |ui: &mut Ui, label: &str, id: &str, value: &mut u8, opts: [&str; 3]| -> bool {
                ui.label(label);
                let before = *value;
                egui::ComboBox::from_id_salt(id)
                    .width(130.0)
                    .selected_text(opts[*value as usize])
                    .show_ui(ui, |ui| {
                        for (i, o) in opts.iter().enumerate() {
                            ui.selectable_value(value, i as u8, *o);
                        }
                    });
                before != *value
            };
        let (mut of, mut ff) = (g.ui.obj_filter, g.ui.fx_filter);
        let oc = filter(
            ui,
            g.i18n.t("editor.objects_filter"),
            "obj-filter",
            &mut of,
            [
                g.i18n.t("editor.all"),
                g.i18n.t("editor.player_visible"),
                g.i18n.t("editor.none"),
            ],
        );
        let fc = filter(
            ui,
            g.i18n.t("editor.effects_filter"),
            "fx-filter",
            &mut ff,
            [
                g.i18n.t("editor.all"),
                g.i18n.t("editor.lights_only"),
                g.i18n.t("editor.none"),
            ],
        );
        g.ui.obj_filter = of;
        g.ui.fx_filter = ff;
        if oc || fc {
            g.gv.map_dirty = true;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(g.i18n.t("editor.scale")).small());
            let (gr, _) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::hover());
            crate::icons::paint(ui.painter(), gr, Icon::Grid);
            if let Some(c) = g.hover.cell {
                ui.label(
                    RichText::new(format!("({}, {})", c.0, c.1))
                        .small()
                        .color(theme::PARCHMENT_DARK),
                );
            }
        });
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
    let h = h.min(140.0);
    let Some(mr) = g.map_ptr.map_rect else { return };
    let pos = egui::pos2(mr[2] - w - 22.0, mr[3] - h - 22.0);
    egui::Area::new("editor-minimap".into())
        .fixed_pos(pos - vec2(40.0, 18.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Zoom buttons.
                ui.vertical(|ui| {
                    ui.add_space(h - 56.0 + 18.0);
                    for (txt, k) in [("+", 0.85f32), ("−", 1.18)] {
                        if crate::chrome::bar_button(ui, None, txt, false).clicked() {
                            let cur = g.fit.target_scale.unwrap_or(1.0);
                            g.fit.target_scale = Some((cur * k).clamp(0.25, 4.0));
                        }
                    }
                });
                ui.vertical(|ui| {
                    ui.add_space(18.0);
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), egui::Sense::click());
                    let p = ui.painter();
                    p.rect_filled(rect.expand(6.0), 4.0, crate::chrome::DARK);
                    p.image(
                        tex.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    crate::chrome::gold_frame(p, rect.expand(6.0), 0.8);
                    crate::chrome::compass(p, egui::pos2(rect.center().x, rect.top() - 6.0), 11.0);
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
                        let v = egui::Rect::from_two_pos(to(cam.0, cam.1), to(cam.2, cam.3))
                            .intersect(rect);
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
            });
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
