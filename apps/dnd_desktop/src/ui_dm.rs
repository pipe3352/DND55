//! Dungeon Master windows and panels: object selection & properties, drawing tools, the bestiary
//! (search, filters, stat blocks, drag-to-map, custom monsters), the visual room graph, scripted
//! events, rests/XP and room audio. None of these are shown to players.

use crate::state::{DrawKind, ScriptPick, Tool};
use crate::theme;
use crate::ui_game::GameUi;
use bevy_egui::egui::{self, Color32, RichText, Ui};
use dnd_rules::ability::{Ability, Skill};
use dnd_rules::attack::{AttackProfile, DamagePart};
use dnd_rules::content::Size;
use dnd_rules::damage::DamageType;
use dnd_rules::monster::{self, MonsterTemplate};
use dnd_session::command::{Command, DmCommand};
use dnd_session::scripts::{DialogueChoice, ScriptAction, ScriptedEvent, Trigger};
use dnd_session::view::ClientView;
use dnd_world::RoomId;
use dnd_world::map::{AmbientLight, LayerKind, MapObject};

// ------------------------------------------------------------------ editor: selection & properties

pub fn selection_panel(ui: &mut Ui, g: &mut GameUi, room: RoomId) {
    let Some(map) = g.gv.map.clone() else { return };
    g.ui.sel_objects.retain(|id| map.object(*id).is_some());
    theme::heading(ui, g.i18n.t("select.title"));
    ui.label(RichText::new(g.i18n.t("select.help")).small());
    let n = g.ui.sel_objects.len();
    ui.label(format!("{}: {n}", g.i18n.t("select.count")));
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(n > 0, egui::Button::new(g.i18n.t("select.copy")))
            .on_hover_text("Ctrl+C")
            .clicked()
        {
            copy_selection(g);
        }
        if ui
            .add_enabled(
                !g.ui.clipboard.is_empty(),
                egui::Button::new(g.i18n.t("select.paste")),
            )
            .on_hover_text("Ctrl+V")
            .clicked()
        {
            let at = g.hover.cell.unwrap_or((1, 1));
            paste_at(g, room, at);
        }
        if ui
            .add_enabled(n > 0, egui::Button::new(g.i18n.t("select.duplicate")))
            .on_hover_text("Ctrl+D")
            .clicked()
        {
            duplicate(g, room);
        }
        if ui
            .add_enabled(n > 0, egui::Button::new(g.i18n.t("select.rotate")))
            .on_hover_text("R")
            .clicked()
        {
            rotate(g, room);
        }
        if ui
            .add_enabled(
                n > 0,
                egui::Button::new(RichText::new(g.i18n.t("select.delete")).color(theme::BLOOD)),
            )
            .on_hover_text("Del")
            .clicked()
        {
            let objects = std::mem::take(&mut g.ui.sel_objects);
            g.net
                .cmd(Command::Dm(DmCommand::RemoveObjects { room, objects }));
        }
        if ui
            .add_enabled(n > 0, egui::Button::new(g.i18n.t("select.clear")))
            .on_hover_text("Esc")
            .clicked()
        {
            g.ui.sel_objects.clear();
        }
    });
    ui.horizontal(|ui| {
        for (label, dx, dy) in [("◀", -1, 0), ("▶", 1, 0), ("⬆", 0, -1), ("⬇", 0, 1)] {
            if ui.add_enabled(n > 0, egui::Button::new(label)).clicked() {
                let objects = g.ui.sel_objects.clone();
                g.net.cmd(Command::Dm(DmCommand::MoveObjects {
                    room,
                    objects,
                    dx,
                    dy,
                }));
            }
        }
    });
    if n == 1
        && let Some(o) = map.object(g.ui.sel_objects[0]).cloned()
    {
        object_properties(ui, g, room, o);
    }
}

pub fn copy_selection(g: &mut GameUi) {
    let Some(map) = &g.gv.map else { return };
    g.ui.clipboard =
        g.ui.sel_objects
            .iter()
            .filter_map(|id| map.object(*id).cloned())
            .collect();
    let n = g.ui.clipboard.len();
    g.gv.toast(format!("{} {n}", g.i18n.t("select.copied")));
}

/// Pastes the clipboard so its top-left object lands on `at`.
pub fn paste_at(g: &mut GameUi, room: RoomId, at: (i32, i32)) {
    if g.ui.clipboard.is_empty() {
        return;
    }
    let min_x = g.ui.clipboard.iter().map(|o| o.x).min().unwrap_or(0);
    let min_y = g.ui.clipboard.iter().map(|o| o.y).min().unwrap_or(0);
    let objects: Vec<MapObject> =
        g.ui.clipboard
            .iter()
            .cloned()
            .map(|mut o| {
                o.x = o.x - min_x + at.0;
                o.y = o.y - min_y + at.1;
                o
            })
            .collect();
    g.net
        .cmd(Command::Dm(DmCommand::AddObjects { room, objects }));
}

pub fn duplicate(g: &mut GameUi, room: RoomId) {
    let Some(map) = &g.gv.map else { return };
    let objects: Vec<MapObject> =
        g.ui.sel_objects
            .iter()
            .filter_map(|id| map.object(*id).cloned())
            .map(|mut o| {
                o.x += 1;
                o.y += 1;
                o
            })
            .collect();
    g.net
        .cmd(Command::Dm(DmCommand::AddObjects { room, objects }));
}

pub fn rotate(g: &mut GameUi, room: RoomId) {
    let Some(map) = g.gv.map.clone() else { return };
    for id in g.ui.sel_objects.clone() {
        if let Some(mut o) = map.object(id).cloned() {
            o.rotation = (o.rotation + 1) % 4;
            std::mem::swap(&mut o.w, &mut o.h);
            g.net.cmd(Command::Dm(DmCommand::UpdateObject {
                room,
                object: Box::new(o),
            }));
        }
    }
}

fn object_properties(ui: &mut Ui, g: &mut GameUi, room: RoomId, o: MapObject) {
    theme::heading(ui, g.i18n.t("props.title"));
    let asset_name = g
        .catalog
        .0
        .get(&o.asset_id)
        .map(|a| a.name.clone())
        .unwrap_or_else(|| o.asset_id.clone());
    ui.label(format!(
        "{asset_name} @ ({}, {}) · {}×{}",
        o.x, o.y, o.w, o.h
    ));
    let mut e = o.clone();
    ui.horizontal(|ui| {
        ui.label(g.i18n.t("props.label"));
        ui.text_edit_singleline(&mut e.label);
    });
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut e.hidden, g.i18n.t("props.hidden"));
        ui.checkbox(&mut e.open, g.i18n.t("props.open"));
        ui.checkbox(&mut e.interactable, g.i18n.t("props.interactable"));
        ui.checkbox(&mut e.blocks_movement, g.i18n.t("props.blocks_movement"));
        ui.checkbox(&mut e.blocks_sight, g.i18n.t("props.blocks_sight"));
        ui.checkbox(&mut e.difficult, g.i18n.t("props.difficult"));
    });
    // Size (scaling in whole cells), rotation and draw layer.
    ui.horizontal_wrapped(|ui| {
        ui.label(g.i18n.t("props.size"));
        ui.add(egui::DragValue::new(&mut e.w).range(1..=8).prefix("w "));
        ui.add(egui::DragValue::new(&mut e.h).range(1..=8).prefix("h "));
        if ui
            .small_button("+90°")
            .on_hover_text(g.i18n.t("props.rotate"))
            .clicked()
        {
            e.rotation = (e.rotation + 1) % 4;
        }
    });
    ui.horizontal(|ui| {
        ui.label(g.i18n.t("props.layer"));
        for (l, key) in [
            (dnd_world::map::ObjectLayer::Floor, "props.layer_floor"),
            (
                dnd_world::map::ObjectLayer::Standard,
                "props.layer_standard",
            ),
            (
                dnd_world::map::ObjectLayer::Overhead,
                "props.layer_overhead",
            ),
        ] {
            ui.selectable_value(&mut e.layer, l, g.i18n.t(key));
        }
    });
    let mut has_light = e.light.is_some();
    ui.horizontal(|ui| {
        ui.checkbox(&mut has_light, g.i18n.t("props.light"));
        if has_light {
            let l = e.light.get_or_insert(dnd_world::catalog::LightEmission {
                bright_ft: 20,
                dim_ft: 20,
                color: [255, 200, 120],
                flicker: true,
            });
            ui.add(
                egui::DragValue::new(&mut l.bright_ft)
                    .range(0..=120)
                    .suffix(" ft"),
            );
            ui.add(
                egui::DragValue::new(&mut l.dim_ft)
                    .range(0..=120)
                    .suffix(" ft"),
            );
            ui.checkbox(&mut l.flicker, g.i18n.t("props.flicker"));
        } else {
            e.light = None;
        }
    });
    ui.label(g.i18n.t("props.dm_notes"));
    ui.add(egui::TextEdit::multiline(&mut e.dm_notes).desired_rows(2));
    if e != o {
        g.net.cmd(Command::Dm(DmCommand::UpdateObject {
            room,
            object: Box::new(e.clone()),
        }));
    }
    // Container contents (chests, crates, corpses…): anything can hold loot.
    let contents =
        g.gv.view
            .as_ref()
            .and_then(|v| v.room.as_ref())
            .and_then(|r| {
                r.containers
                    .iter()
                    .find(|(id, _)| *id == o.id)
                    .map(|(_, items)| items.clone())
            })
            .unwrap_or_default();
    egui::CollapsingHeader::new(format!(
        "{} ({})",
        g.i18n.t("props.contents"),
        contents.len()
    ))
    .id_salt("contents")
    .show(ui, |ui| {
        let mut items = contents.clone();
        let mut changed = false;
        let mut remove = None;
        for (i, it) in items.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                changed |= ui
                    .add(egui::DragValue::new(&mut it.quantity).range(1..=999))
                    .changed();
                ui.label(&it.name);
                if ui.small_button("✖").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            items.remove(i);
            changed = true;
        }
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("add-item")
                .selected_text(
                    dnd_rules::items::catalog_item(&g.ui.container_add)
                        .map(|i| i.name)
                        .unwrap_or_else(|| g.i18n.t("props.add_item").into()),
                )
                .show_ui(ui, |ui| {
                    for it in dnd_rules::items::catalog() {
                        ui.selectable_value(&mut g.ui.container_add, it.id.clone(), &it.name);
                    }
                });
            if ui.button("+").clicked()
                && let Some(it) = dnd_rules::items::catalog_item(&g.ui.container_add)
            {
                dnd_rules::items::add_to(&mut items, it);
                changed = true;
            }
        });
        if changed {
            g.net.cmd(Command::Dm(DmCommand::SetContainer {
                room,
                object: o.id,
                items,
            }));
        }
        ui.label(RichText::new(g.i18n.t("props.contents_help")).small());
    });
}

pub fn layer_label(g: &GameUi, k: LayerKind) -> String {
    let key = match k {
        LayerKind::Terrain => "layer.terrain",
        LayerKind::FloorDecoration => "layer.floor",
        LayerKind::Walls => "layer.walls",
        LayerKind::Furniture => "layer.furniture",
        LayerKind::Overhead => "layer.overhead",
        LayerKind::Effects => "layer.effects",
    };
    g.i18n.t(key).to_string()
}

/// Drawing tool options (shared by the DM and, when allowed, players).
pub fn draw_options(ui: &mut Ui, g: &mut GameUi, room: RoomId, dm: bool) {
    theme::heading(ui, g.i18n.t("draw.title"));
    ui.horizontal_wrapped(|ui| {
        for (k, key) in [
            (DrawKind::Stroke, "draw.stroke"),
            (DrawKind::Line, "draw.line"),
            (DrawKind::Arrow, "draw.arrow"),
            (DrawKind::Circle, "draw.circle"),
            (DrawKind::Text, "draw.text"),
        ] {
            ui.selectable_value(&mut g.ui.draw_kind, k, g.i18n.t(key));
        }
    });
    ui.horizontal(|ui| {
        let [r, gg, b, a] = g.ui.draw_color;
        let mut c = Color32::from_rgba_unmultiplied(r, gg, b, a);
        if ui.color_edit_button_srgba(&mut c).changed() {
            g.ui.draw_color = c.to_srgba_unmultiplied();
        }
        ui.add(egui::Slider::new(&mut g.ui.draw_width, 1.0..=12.0).text(g.i18n.t("draw.width")));
    });
    if g.ui.draw_kind == DrawKind::Text {
        ui.add(
            egui::TextEdit::singleline(&mut g.ui.draw_text).hint_text(g.i18n.t("draw.text_hint")),
        );
    }
    ui.checkbox(&mut g.ui.draw_temporary, g.i18n.t("draw.temporary"));
    if dm {
        ui.checkbox(&mut g.ui.draw_dm_only, g.i18n.t("draw.dm_only"));
        ui.horizontal(|ui| {
            if ui.button(g.i18n.t("draw.clear_temp")).clicked() {
                g.net.cmd(Command::Dm(DmCommand::ClearDrawings {
                    room,
                    temporary_only: true,
                }));
            }
            if ui.button(g.i18n.t("draw.clear_all")).clicked() {
                g.net.cmd(Command::Dm(DmCommand::ClearDrawings {
                    room,
                    temporary_only: false,
                }));
            }
        });
        if let Some(v) = &g.gv.view {
            let mut allowed = v.players_can_draw;
            if ui
                .checkbox(&mut allowed, g.i18n.t("draw.players_can_draw"))
                .changed()
            {
                g.net
                    .cmd(Command::Dm(DmCommand::SetPlayersCanDraw { allowed }));
            }
        }
    }
    ui.label(RichText::new(g.i18n.t("draw.help")).small());
}

/// Music and ambience for the viewed room.
pub fn room_audio(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let Some(r) = &view.room else { return };
    theme::heading(ui, g.i18n.t("audio.room"));
    let (mut music, mut amb) = (r.music.clone(), r.ambience.clone());
    let none = g.i18n.t("audio.none").to_string();
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(g.i18n.t("settings.music"));
        egui::ComboBox::from_id_salt("music")
            .selected_text(music.clone().unwrap_or(none.clone()))
            .show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut music, None, &none).changed();
                for t in crate::audio::MUSIC_TRACKS {
                    changed |= ui
                        .selectable_value(&mut music, Some(t.to_string()), *t)
                        .changed();
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label(g.i18n.t("settings.ambience"));
        egui::ComboBox::from_id_salt("amb")
            .selected_text(amb.clone().unwrap_or(none.clone()))
            .show_ui(ui, |ui| {
                changed |= ui.selectable_value(&mut amb, None, &none).changed();
                for t in crate::audio::AMBIENCE_TRACKS {
                    changed |= ui
                        .selectable_value(&mut amb, Some(t.to_string()), *t)
                        .changed();
                }
            });
    });
    if changed {
        g.net.cmd(Command::Dm(DmCommand::SetRoomAudio {
            room: r.id,
            music,
            ambience: amb,
        }));
    }
    ui.label(RichText::new(g.i18n.t("audio.room_help")).small());
}

// ------------------------------------------------------------------ bestiary

fn all_templates(view: &ClientView) -> Vec<MonsterTemplate> {
    let mut v: Vec<MonsterTemplate> = monster::srd_bestiary_ref().to_vec();
    if let Some(dm) = &view.dm {
        v.extend(dm.custom_monsters.iter().cloned());
    }
    v
}

pub fn bestiary_window(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    // Drop a dragged monster onto the map.
    if let Some(id) = g.ui.dragging_monster.clone() {
        if let Some(p) = ctx.pointer_latest_pos() {
            egui::Area::new("drag-monster".into())
                .order(egui::Order::Tooltip)
                .fixed_pos(p + egui::vec2(12.0, 12.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(format!("🐉 {id}"))
                            .strong()
                            .background_color(theme::WOOD)
                            .color(theme::PARCHMENT),
                    );
                });
        }
        if ctx.input(|i| i.pointer.any_released()) {
            g.ui.dragging_monster = None;
            let over_map = g
                .map_ptr
                .map_rect
                .zip(ctx.pointer_latest_pos())
                .is_some_and(|(r, p)| {
                    crate::pointer::over_map_at(
                        ctx,
                        egui::Rect::from_min_max(egui::pos2(r[0], r[1]), egui::pos2(r[2], r[3])),
                        p,
                    )
                });
            if over_map && let (Some(cell), Some(r)) = (g.hover.cell, &view.room) {
                let hidden = g.ui.spawn_hidden;
                g.net.cmd(Command::Dm(DmCommand::SpawnMonster {
                    template: id,
                    room: r.id,
                    cell,
                    hidden,
                }));
            }
        }
    }
    if !g.ui.show_bestiary {
        return;
    }
    let templates = all_templates(view);
    let mut open = true;
    theme::win(format!("🐉 {}", g.i18n.t("bestiary.title")))
        .open(&mut open)
        .default_width(720.0)
        .default_height(560.0)
        .show(ctx, |ui| {
            theme::window_bg(ui);
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut g.ui.bestiary_search)
                        .hint_text(g.i18n.t("editor.search"))
                        .desired_width(160.0),
                );
                let mut types: Vec<String> = templates
                    .iter()
                    .map(|t| {
                        t.creature_type
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_string()
                    })
                    .collect();
                types.sort();
                types.dedup();
                let any = g.i18n.t("bestiary.any_type").to_string();
                egui::ComboBox::from_id_salt("btype")
                    .selected_text(if g.ui.bestiary_type.is_empty() {
                        any.clone()
                    } else {
                        g.ui.bestiary_type.clone()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut g.ui.bestiary_type, String::new(), &any);
                        for t in types {
                            ui.selectable_value(&mut g.ui.bestiary_type, t.clone(), t);
                        }
                    });
                ui.label("CR");
                ui.add(
                    egui::DragValue::new(&mut g.ui.bestiary_cr.0)
                        .range(0.0..=30.0)
                        .speed(0.25),
                );
                ui.label("–");
                ui.add(
                    egui::DragValue::new(&mut g.ui.bestiary_cr.1)
                        .range(0.0..=30.0)
                        .speed(0.25),
                );
                ui.checkbox(&mut g.ui.spawn_hidden, g.i18n.t("monsters.spawn_hidden"));
                if ui.button(g.i18n.t("bestiary.new_custom")).clicked() {
                    g.ui.custom_monster = Some(MonsterTemplate::blank_custom("custom_new"));
                }
            });
            ui.label(RichText::new(g.i18n.t("bestiary.drag_help")).small());
            ui.separator();
            let q = g.ui.bestiary_search.to_lowercase();
            let filtered: Vec<&MonsterTemplate> = templates
                .iter()
                .filter(|t| {
                    q.is_empty()
                        || t.name.to_lowercase().contains(&q)
                        || t.tags.to_lowercase().contains(&q)
                })
                .filter(|t| {
                    g.ui.bestiary_type.is_empty()
                        || t.creature_type.starts_with(&g.ui.bestiary_type)
                })
                .filter(|t| {
                    let cr = t.cr_value();
                    cr >= g.ui.bestiary_cr.0 && cr <= g.ui.bestiary_cr.1
                })
                .collect();
            ui.columns(2, |cols| {
                cols[0].label(format!(
                    "{} {}",
                    filtered.len(),
                    g.i18n.t("bestiary.results")
                ));
                egui::ScrollArea::vertical()
                    .id_salt("blist")
                    .show(&mut cols[0], |ui| {
                        for t in &filtered {
                            let custom = t.id.starts_with("custom_");
                            let text = format!(
                                "{}{} · CR {} · AC {} · HP {}",
                                if custom { "★ " } else { "" },
                                t.name,
                                t.challenge,
                                t.armor_class,
                                t.hit_points
                            );
                            // Pixel-art portrait of the monster (same sprite as on the map).
                            let key = format!(
                                "mon:{}:{}",
                                t.creature_type.to_lowercase(),
                                t.name.to_lowercase()
                            );
                            let tex = crate::tokens::token_egui(
                                ui.ctx(),
                                &key,
                                crate::terrain_art::token_art::Side::Hostile,
                            );
                            let resp = ui.add(
                                egui::Button::image_and_text(
                                    egui::Image::new((tex.id(), egui::vec2(24.0, 24.0))),
                                    text,
                                )
                                .selected(g.ui.bestiary_preview.as_deref() == Some(&t.id))
                                .sense(egui::Sense::click_and_drag()),
                            );
                            if resp.clicked() {
                                g.ui.bestiary_preview = Some(t.id.clone());
                            }
                            if resp.drag_started() {
                                g.ui.dragging_monster = Some(t.id.clone());
                            }
                            if resp.double_clicked() {
                                g.ui.monster = t.id.clone();
                                g.ui.tool = Tool::Spawn;
                            }
                        }
                    });
                let preview =
                    g.ui.bestiary_preview
                        .clone()
                        .and_then(|id| templates.iter().find(|t| t.id == id).cloned());
                egui::ScrollArea::vertical().id_salt("bprev").show(
                    &mut cols[1],
                    |ui| match preview {
                        Some(t) => {
                            ui.horizontal(|ui| {
                                if ui
                                    .button(g.i18n.t("bestiary.place"))
                                    .on_hover_text(g.i18n.t("bestiary.place_help"))
                                    .clicked()
                                {
                                    g.ui.monster = t.id.clone();
                                    g.ui.tool = Tool::Spawn;
                                }
                                if ui.button(g.i18n.t("bestiary.copy_custom")).clicked() {
                                    let mut c = t.clone();
                                    c.id = "custom_new".into();
                                    c.name = format!("{} (variant)", t.name);
                                    g.ui.custom_monster = Some(c);
                                }
                                if t.id.starts_with("custom_") {
                                    if ui.button(g.i18n.t("bestiary.edit")).clicked() {
                                        g.ui.custom_monster = Some(t.clone());
                                    }
                                    if ui
                                        .button(
                                            RichText::new(g.i18n.t("bestiary.delete"))
                                                .color(theme::BLOOD),
                                        )
                                        .clicked()
                                    {
                                        g.net.cmd(Command::Dm(DmCommand::DeleteCustomMonster {
                                            id: t.id.clone(),
                                        }));
                                        g.ui.bestiary_preview = None;
                                    }
                                }
                            });
                            stat_block(ui, &t);
                        }
                        None => {
                            ui.label(g.i18n.t("bestiary.pick"));
                        }
                    },
                );
            });
            ui.label(RichText::new(g.i18n.t("bestiary.legal")).small().italics());
        });
    if !open {
        g.ui.show_bestiary = false;
    }
    custom_monster_window(ctx, g);
}

pub fn stat_block(ui: &mut Ui, t: &MonsterTemplate) {
    ui.label(
        RichText::new(&t.name)
            .size(20.0)
            .strong()
            .color(theme::BLOOD),
    );
    ui.label(RichText::new(format!("{:?} {}, {}", t.size, t.creature_type, t.alignment)).italics());
    ui.separator();
    ui.label(format!(
        "AC {} · HP {} ({}) · Speed {}",
        t.armor_class,
        t.hit_points,
        t.hit_dice,
        if t.speeds.is_empty() {
            format!("{} ft", t.speed_ft)
        } else {
            t.speeds.clone()
        }
    ));
    ui.label(format!("Initiative {:+}", t.initiative_bonus));
    egui::Grid::new(("abilities", &t.id)).show(ui, |ui| {
        for a in Ability::ALL {
            ui.label(
                RichText::new(
                    format!("{a:?}")
                        .chars()
                        .take(3)
                        .collect::<String>()
                        .to_uppercase(),
                )
                .strong(),
            );
        }
        ui.end_row();
        for a in Ability::ALL {
            let s = t.scores.get(a) as i32;
            ui.label(format!("{s} ({:+})", (s - 10).div_euclid(2)));
        }
        ui.end_row();
    });
    for (k, v) in [
        ("Skills", &t.skills),
        ("Senses", &t.senses),
        ("Languages", &t.languages),
    ] {
        if !v.is_empty() {
            ui.label(format!("{k} {v}"));
        }
    }
    ui.label(format!(
        "CR {} ({} XP; PB {:+})",
        t.challenge, t.xp, t.proficiency_bonus
    ));
    let section = |ui: &mut Ui, title: &str, list: &[monster::TextEntry]| {
        if list.is_empty() {
            return;
        }
        ui.label(RichText::new(title).strong().color(theme::BLOOD));
        for e in list {
            ui.label(RichText::new(&e.name).strong().italics());
            ui.label(&e.text);
        }
    };
    section(ui, "Traits", &t.traits);
    section(ui, "Actions", &t.actions);
    if t.actions.is_empty() {
        ui.label(RichText::new("Actions").strong().color(theme::BLOOD));
        for a in &t.attacks {
            ui.label(a.summary());
        }
    }
    section(ui, "Bonus Actions", &t.bonus_actions);
    section(ui, "Reactions", &t.reactions);
    section(ui, "Legendary Actions", &t.legendary_actions);
    ui.label(RichText::new(format!("Source: {}", t.source)).small());
}

fn custom_monster_window(ctx: &egui::Context, g: &mut GameUi) {
    let Some(mut m) = g.ui.custom_monster.clone() else {
        return;
    };
    let mut open = true;
    let mut save = false;
    theme::win(format!("★ {}", g.i18n.t("custom.title")))
        .open(&mut open)
        .default_width(480.0)
        .show(ctx, |ui| {
            theme::window_bg(ui);
            egui::ScrollArea::vertical()
                .max_height(520.0)
                .show(ui, |ui| {
                    egui::Grid::new("cm").num_columns(2).show(ui, |ui| {
                        ui.label(g.i18n.t("custom.name"));
                        ui.text_edit_singleline(&mut m.name);
                        ui.end_row();
                        ui.label(g.i18n.t("custom.type"));
                        ui.text_edit_singleline(&mut m.creature_type);
                        ui.end_row();
                        ui.label(g.i18n.t("custom.size"));
                        egui::ComboBox::from_id_salt("size")
                            .selected_text(format!("{:?}", m.size))
                            .show_ui(ui, |ui| {
                                for s in [
                                    Size::Tiny,
                                    Size::Small,
                                    Size::Medium,
                                    Size::Large,
                                    Size::Huge,
                                    Size::Gargantuan,
                                ] {
                                    ui.selectable_value(&mut m.size, s, format!("{s:?}"));
                                }
                            });
                        ui.end_row();
                        ui.label("AC");
                        ui.add(egui::DragValue::new(&mut m.armor_class).range(1..=30));
                        ui.end_row();
                        ui.label("HP");
                        ui.add(egui::DragValue::new(&mut m.hit_points).range(1..=999));
                        ui.end_row();
                        ui.label(g.i18n.t("custom.speed"));
                        ui.add(
                            egui::DragValue::new(&mut m.speed_ft)
                                .range(0..=120)
                                .suffix(" ft"),
                        );
                        ui.end_row();
                        ui.label("CR");
                        ui.text_edit_singleline(&mut m.challenge);
                        ui.end_row();
                        ui.label("XP");
                        ui.add(egui::DragValue::new(&mut m.xp).range(0..=155000));
                        ui.end_row();
                        ui.label(g.i18n.t("custom.initiative"));
                        ui.add(egui::DragValue::new(&mut m.initiative_bonus).range(-5..=15));
                        ui.end_row();
                        ui.label(g.i18n.t("custom.darkvision"));
                        ui.add(
                            egui::DragValue::new(&mut m.darkvision_ft)
                                .range(0..=120)
                                .suffix(" ft"),
                        );
                        ui.end_row();
                    });
                    ui.horizontal_wrapped(|ui| {
                        for a in Ability::ALL {
                            let mut v = m.scores.get(a);
                            ui.label(format!("{a:?}").chars().take(3).collect::<String>());
                            if ui.add(egui::DragValue::new(&mut v).range(1..=30)).changed() {
                                m.scores.set(a, v);
                            }
                        }
                    });
                    theme::heading(ui, g.i18n.t("custom.attacks"));
                    let mut remove = None;
                    for (i, a) in m.attacks.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut a.name).desired_width(110.0));
                            ui.add(
                                egui::DragValue::new(&mut a.to_hit)
                                    .range(-5..=20)
                                    .prefix("+"),
                            );
                            if let Some(d) = a.damage.first_mut() {
                                let mut txt = d.dice.to_string();
                                if ui
                                    .add(egui::TextEdit::singleline(&mut txt).desired_width(70.0))
                                    .changed()
                                    && let Ok(p) = dnd_rules::DiceExpr::parse(&txt)
                                {
                                    d.dice = p;
                                }
                                egui::ComboBox::from_id_salt(("dt", i))
                                    .selected_text(format!("{:?}", d.damage_type))
                                    .width(90.0)
                                    .show_ui(ui, |ui| {
                                        for t in DamageType::ALL {
                                            ui.selectable_value(
                                                &mut d.damage_type,
                                                t,
                                                format!("{t:?}"),
                                            );
                                        }
                                    });
                            }
                            ui.add(
                                egui::DragValue::new(&mut a.reach_ft)
                                    .range(5..=30)
                                    .suffix(" ft"),
                            );
                            if ui.small_button("✖").clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        m.attacks.remove(i);
                    }
                    if ui.button(g.i18n.t("custom.add_attack")).clicked() {
                        m.attacks.push(AttackProfile {
                            name: "Claw".into(),
                            to_hit: 4,
                            reach_ft: 5,
                            damage: vec![DamagePart {
                                dice: dnd_rules::DiceExpr::parse("1d6+2").expect("valid"),
                                damage_type: DamageType::Slashing,
                            }],
                            ..Default::default()
                        });
                    }
                    ui.add(
                        egui::Slider::new(&mut m.attacks_per_action, 1..=4)
                            .text(g.i18n.t("custom.multiattack")),
                    );
                    theme::heading(ui, g.i18n.t("custom.traits"));
                    let mut remove = None;
                    for (i, t) in m.traits.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut t.name).desired_width(120.0));
                            ui.add(egui::TextEdit::multiline(&mut t.text).desired_rows(1));
                            if ui.small_button("✖").clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        m.traits.remove(i);
                    }
                    if ui.button(g.i18n.t("custom.add_trait")).clicked() {
                        m.traits.push(monster::TextEntry {
                            name: "Trait".into(),
                            text: String::new(),
                        });
                    }
                });
            match m.validate_custom() {
                Ok(()) => {
                    if ui
                        .button(RichText::new(g.i18n.t("custom.save")).strong())
                        .clicked()
                    {
                        save = true;
                    }
                }
                Err(e) => {
                    ui.label(RichText::new(e).color(theme::BLOOD));
                }
            }
            ui.label(RichText::new(g.i18n.t("custom.help")).small());
        });
    if save {
        g.net.cmd(Command::Dm(DmCommand::SaveCustomMonster {
            template: Box::new(m.clone()),
        }));
        g.gv.toast(g.i18n.t("custom.saved"));
        g.ui.custom_monster = None;
    } else if !open {
        g.ui.custom_monster = None;
    } else {
        g.ui.custom_monster = Some(m);
    }
}

// ------------------------------------------------------------------ scripted events

fn trigger_summary(t: &Trigger, view: &ClientView) -> String {
    match t {
        Trigger::EnterArea { a, b, .. } => {
            format!("Enter area ({},{})–({},{})", a.0, a.1, b.0, b.1)
        }
        Trigger::Interact { object, .. } => format!("Interact with object #{}", object.0),
        Trigger::CreatureDefeated { entity } => format!(
            "{} defeated",
            view.creatures
                .iter()
                .find(|c| c.id == *entity)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| format!("#{}", entity.0))
        ),
        Trigger::EnterRoom { room } => format!(
            "Party enters {}",
            view.graph
                .nodes
                .get(room)
                .map(|n| n.name.clone())
                .unwrap_or_default()
        ),
        Trigger::Manual => "Manual (DM fires it)".into(),
    }
}

pub fn events_tab(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let Some(dm) = &view.dm else { return };
    let room = view
        .room
        .as_ref()
        .map(|r| r.id)
        .unwrap_or(dnd_world::RoomId(0));
    theme::heading(ui, g.i18n.t("events.title"));
    ui.label(RichText::new(g.i18n.t("events.help")).small());
    for s in &dm.scripts {
        ui.horizontal_wrapped(|ui| {
            let mut en = s.enabled;
            if ui
                .checkbox(&mut en, "")
                .on_hover_text(g.i18n.t("events.enabled"))
                .changed()
            {
                let mut c = s.clone();
                c.enabled = en;
                g.net.cmd(Command::Dm(DmCommand::SaveScript {
                    script: Box::new(c),
                }));
            }
            ui.label(RichText::new(&s.name).strong())
                .on_hover_text(format!(
                    "{}\n{}",
                    trigger_summary(&s.trigger, view),
                    s.actions
                        .iter()
                        .map(|a| a.summary())
                        .collect::<Vec<_>>()
                        .join("\n")
                ));
            if s.fired {
                ui.label(RichText::new("✔").small())
                    .on_hover_text(g.i18n.t("events.fired"));
            }
            if ui
                .small_button("▶")
                .on_hover_text(g.i18n.t("events.fire"))
                .clicked()
            {
                g.net.cmd(Command::Dm(DmCommand::FireScript {
                    id: s.id,
                    actor: g.ui.selected,
                }));
            }
            if ui
                .small_button("✏")
                .on_hover_text(g.i18n.t("events.edit"))
                .clicked()
            {
                g.ui.script_draft = Some(s.clone());
            }
            if ui.small_button("✖").clicked() {
                g.net.cmd(Command::Dm(DmCommand::DeleteScript { id: s.id }));
            }
        });
    }
    theme::heading(ui, g.i18n.t("events.templates"));
    ui.horizontal_wrapped(|ui| {
        let new = |name: &str, trigger: Trigger, actions: Vec<ScriptAction>| ScriptedEvent {
            id: 0,
            name: name.into(),
            enabled: true,
            once: true,
            fired: false,
            trigger,
            actions,
        };
        if ui.button(g.i18n.t("events.tpl_trap")).clicked() {
            g.ui.script_draft = Some(new(
                "Pressure plate",
                Trigger::EnterArea {
                    room,
                    a: (1, 1),
                    b: (1, 1),
                },
                vec![ScriptAction::Trap {
                    dice: "2d6".into(),
                    damage_type: DamageType::Piercing,
                    save: Some(Ability::Dexterity),
                    dc: 13,
                    half_on_success: true,
                }],
            ));
            g.ui.script_pick = Some(ScriptPick::AreaStart);
        }
        if ui.button(g.i18n.t("events.tpl_lever")).clicked() {
            g.ui.script_draft = Some(new(
                "Lever opens the gate",
                Trigger::Interact {
                    room,
                    object: dnd_world::ObjectId(0),
                },
                vec![ScriptAction::Message {
                    text: "With a grinding of chains, the gate rises.".into(),
                }],
            ));
            g.ui.script_pick = Some(ScriptPick::Object);
        }
        if ui.button(g.i18n.t("events.tpl_chest")).clicked() {
            g.ui.script_draft = Some(new(
                "Chest reward",
                Trigger::Interact {
                    room,
                    object: dnd_world::ObjectId(0),
                },
                vec![ScriptAction::GiveItem {
                    item_id: "potion_of_healing".into(),
                    quantity: 1,
                }],
            ));
            g.ui.script_pick = Some(ScriptPick::Object);
        }
        if ui.button(g.i18n.t("events.tpl_boss")).clicked() {
            let first_monster = view
                .creatures
                .iter()
                .find(|c| c.is_monster)
                .map(|c| c.id)
                .unwrap_or(dnd_world::EntityId(0));
            g.ui.script_draft = Some(new(
                "Boss defeated",
                Trigger::CreatureDefeated {
                    entity: first_monster,
                },
                vec![
                    ScriptAction::Message {
                        text: "The way forward is open.".into(),
                    },
                    ScriptAction::AwardXp { xp: 200 },
                ],
            ));
        }
        if ui.button(g.i18n.t("events.tpl_dialogue")).clicked() {
            g.ui.script_draft = Some(new(
                "Old hermit",
                Trigger::EnterRoom { room },
                vec![ScriptAction::Dialogue {
                    speaker: "Old Hermit".into(),
                    text: "Travelers! Will you help an old man?".into(),
                    choices: vec![
                        DialogueChoice {
                            text: "Of course.".into(),
                            actions: vec![
                                ScriptAction::Message {
                                    text: "The hermit smiles and hands you a vial.".into(),
                                },
                                ScriptAction::GiveItem {
                                    item_id: "potion_of_healing".into(),
                                    quantity: 1,
                                },
                            ],
                        },
                        DialogueChoice {
                            text: "Not today.".into(),
                            actions: vec![],
                        },
                    ],
                }],
            ));
        }
        if ui.button(g.i18n.t("events.tpl_check")).clicked() {
            g.ui.script_draft = Some(new(
                "Inspect the altar",
                Trigger::Interact {
                    room,
                    object: dnd_world::ObjectId(0),
                },
                vec![ScriptAction::SkillCheck {
                    skill: Skill::Religion,
                    dc: 12,
                    on_success: vec![ScriptAction::Message {
                        text: "You recognize the sigil of a forgotten dawn god.".into(),
                    }],
                    on_failure: vec![ScriptAction::Message {
                        text: "The carvings mean nothing to you.".into(),
                    }],
                }],
            ));
            g.ui.script_pick = Some(ScriptPick::Object);
        }
        if ui.button(g.i18n.t("events.blank")).clicked() {
            g.ui.script_draft = Some(new("New event", Trigger::Manual, vec![]));
        }
    });
}

pub fn script_editor_window(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    let Some(mut s) = g.ui.script_draft.clone() else {
        return;
    };
    let room = view
        .room
        .as_ref()
        .map(|r| r.id)
        .unwrap_or(dnd_world::RoomId(0));
    let mut open = true;
    let mut save = false;
    theme::win(format!("⚙ {}", g.i18n.t("events.editor")))
        .open(&mut open)
        .default_width(520.0)
        .show(ctx, |ui| {
            theme::window_bg(ui);
            egui::ScrollArea::vertical()
                .max_height(560.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(g.i18n.t("events.name"));
                        ui.text_edit_singleline(&mut s.name);
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut s.enabled, g.i18n.t("events.enabled"));
                        ui.checkbox(&mut s.once, g.i18n.t("events.once"));
                        if s.fired && ui.button(g.i18n.t("events.rearm")).clicked() {
                            s.fired = false;
                        }
                    });
                    theme::heading(ui, g.i18n.t("events.when"));
                    let kinds = [
                        (0, g.i18n.t("events.trigger_area")),
                        (1, g.i18n.t("events.trigger_interact")),
                        (2, g.i18n.t("events.trigger_defeated")),
                        (3, g.i18n.t("events.trigger_room")),
                        (4, g.i18n.t("events.trigger_manual")),
                    ];
                    let cur = match s.trigger {
                        Trigger::EnterArea { .. } => 0,
                        Trigger::Interact { .. } => 1,
                        Trigger::CreatureDefeated { .. } => 2,
                        Trigger::EnterRoom { .. } => 3,
                        Trigger::Manual => 4,
                    };
                    let mut sel = cur;
                    egui::ComboBox::from_id_salt("trig")
                        .selected_text(kinds[cur].1)
                        .show_ui(ui, |ui| {
                            for (k, n) in kinds {
                                ui.selectable_value(&mut sel, k, n);
                            }
                        });
                    if sel != cur {
                        s.trigger = match sel {
                            0 => Trigger::EnterArea {
                                room,
                                a: (0, 0),
                                b: (0, 0),
                            },
                            1 => Trigger::Interact {
                                room,
                                object: dnd_world::ObjectId(0),
                            },
                            2 => Trigger::CreatureDefeated {
                                entity: view
                                    .creatures
                                    .iter()
                                    .find(|c| c.is_monster)
                                    .map(|c| c.id)
                                    .unwrap_or(dnd_world::EntityId(0)),
                            },
                            3 => Trigger::EnterRoom { room },
                            _ => Trigger::Manual,
                        };
                    }
                    match &mut s.trigger {
                        Trigger::EnterArea { a, b, .. } => {
                            ui.label(format!("({}, {}) – ({}, {})", a.0, a.1, b.0, b.1));
                            if ui.button(g.i18n.t("events.pick_area")).clicked() {
                                g.ui.script_pick = Some(ScriptPick::AreaStart);
                            }
                        }
                        Trigger::Interact { object, .. } => {
                            let label =
                                g.gv.map
                                    .as_ref()
                                    .and_then(|m| m.object(*object))
                                    .map(|o| {
                                        format!(
                                            "{} #{}",
                                            if o.label.is_empty() {
                                                &o.asset_id
                                            } else {
                                                &o.label
                                            },
                                            o.id.0
                                        )
                                    })
                                    .unwrap_or_else(|| g.i18n.t("events.none_picked").into());
                            ui.label(label);
                            if ui.button(g.i18n.t("events.pick_object")).clicked() {
                                g.ui.script_pick = Some(ScriptPick::Object);
                            }
                        }
                        Trigger::CreatureDefeated { entity } => {
                            egui::ComboBox::from_id_salt("def")
                                .selected_text(
                                    view.creatures
                                        .iter()
                                        .find(|c| c.id == *entity)
                                        .map(|c| c.name.clone())
                                        .unwrap_or_default(),
                                )
                                .show_ui(ui, |ui| {
                                    for c in &view.creatures {
                                        ui.selectable_value(entity, c.id, &c.name);
                                    }
                                });
                        }
                        Trigger::EnterRoom { room } => {
                            egui::ComboBox::from_id_salt("er")
                                .selected_text(
                                    view.graph
                                        .nodes
                                        .get(room)
                                        .map(|n| n.name.clone())
                                        .unwrap_or_default(),
                                )
                                .show_ui(ui, |ui| {
                                    for (id, n) in &view.graph.nodes {
                                        ui.selectable_value(room, *id, &n.name);
                                    }
                                });
                        }
                        Trigger::Manual => {}
                    }
                    if g.ui.script_pick.is_some() {
                        ui.label(RichText::new(g.i18n.t("events.picking")).color(theme::GOLD));
                    }
                    theme::heading(ui, g.i18n.t("events.then"));
                    action_list(ui, g, view, &mut s.actions, room, "root", 0);
                });
            ui.separator();
            if ui
                .button(RichText::new(g.i18n.t("events.save")).strong())
                .clicked()
            {
                save = true;
            }
        });
    if save {
        g.net.cmd(Command::Dm(DmCommand::SaveScript {
            script: Box::new(s),
        }));
        g.ui.script_draft = None;
        g.ui.script_pick = None;
    } else if !open {
        g.ui.script_draft = None;
        g.ui.script_pick = None;
    } else {
        g.ui.script_draft = Some(s);
    }
}

fn action_list(
    ui: &mut Ui,
    g: &mut GameUi,
    view: &ClientView,
    actions: &mut Vec<ScriptAction>,
    room: RoomId,
    salt: &str,
    depth: u8,
) {
    let mut remove = None;
    for (i, a) in actions.iter_mut().enumerate() {
        let id = format!("{salt}-{i}");
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(a.summary()).strong());
                if ui.small_button("✖").clicked() {
                    remove = Some(i);
                }
            });
            action_fields(ui, g, view, a, room, &id, depth);
        });
    }
    if let Some(i) = remove {
        actions.remove(i);
    }
    if depth > 2 {
        return;
    }
    egui::ComboBox::from_id_salt(format!("add-{salt}"))
        .selected_text(g.i18n.t("events.add_action"))
        .show_ui(ui, |ui| {
            let opts: Vec<(&str, ScriptAction)> = vec![
                (
                    "events.act_message",
                    ScriptAction::Message {
                        text: String::new(),
                    },
                ),
                (
                    "events.act_trap",
                    ScriptAction::Trap {
                        dice: "2d6".into(),
                        damage_type: DamageType::Piercing,
                        save: Some(Ability::Dexterity),
                        dc: 13,
                        half_on_success: true,
                    },
                ),
                (
                    "events.act_give",
                    ScriptAction::GiveItem {
                        item_id: "potion_of_healing".into(),
                        quantity: 1,
                    },
                ),
                (
                    "events.act_check",
                    ScriptAction::SkillCheck {
                        skill: Skill::Perception,
                        dc: 12,
                        on_success: vec![],
                        on_failure: vec![],
                    },
                ),
                (
                    "events.act_open",
                    ScriptAction::SetObjectOpen {
                        room,
                        object: g
                            .ui
                            .sel_objects
                            .first()
                            .copied()
                            .unwrap_or(dnd_world::ObjectId(0)),
                        open: None,
                    },
                ),
                (
                    "events.act_reveal_object",
                    ScriptAction::RevealObject {
                        room,
                        object: g
                            .ui
                            .sel_objects
                            .first()
                            .copied()
                            .unwrap_or(dnd_world::ObjectId(0)),
                    },
                ),
                (
                    "events.act_reveal_creature",
                    ScriptAction::RevealCreature {
                        entity: g.ui.selected.unwrap_or(dnd_world::EntityId(0)),
                    },
                ),
                (
                    "events.act_route",
                    ScriptAction::SetEdgeLocked {
                        edge: view
                            .graph
                            .edges
                            .first()
                            .map(|e| e.id)
                            .unwrap_or(dnd_world::EdgeId(0)),
                        locked: false,
                    },
                ),
                (
                    "events.act_spawn",
                    ScriptAction::SpawnMonster {
                        template: g.ui.monster.clone(),
                        room,
                        cell: (1, 1),
                        hidden: false,
                    },
                ),
                (
                    "events.act_dialogue",
                    ScriptAction::Dialogue {
                        speaker: "NPC".into(),
                        text: String::new(),
                        choices: vec![],
                    },
                ),
                (
                    "events.act_light",
                    ScriptAction::SetAmbient {
                        room,
                        ambient: AmbientLight::Darkness,
                    },
                ),
                ("events.act_xp", ScriptAction::AwardXp { xp: 100 }),
            ];
            for (key, a) in opts {
                if ui.button(g.i18n.t(key)).clicked() {
                    actions.push(a);
                }
            }
        });
}

fn action_fields(
    ui: &mut Ui,
    g: &mut GameUi,
    view: &ClientView,
    a: &mut ScriptAction,
    room: RoomId,
    id: &str,
    depth: u8,
) {
    match a {
        ScriptAction::Message { text } => {
            ui.add(
                egui::TextEdit::multiline(text)
                    .desired_rows(2)
                    .hint_text(g.i18n.t("events.message_hint")),
            );
        }
        ScriptAction::Trap {
            dice,
            damage_type,
            save,
            dc,
            half_on_success,
        } => {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::TextEdit::singleline(dice).desired_width(60.0));
                egui::ComboBox::from_id_salt(format!("{id}-dt"))
                    .selected_text(format!("{damage_type:?}"))
                    .show_ui(ui, |ui| {
                        for t in DamageType::ALL {
                            ui.selectable_value(damage_type, t, format!("{t:?}"));
                        }
                    });
                egui::ComboBox::from_id_salt(format!("{id}-sv"))
                    .selected_text(
                        save.map(|s| format!("{s:?} save"))
                            .unwrap_or_else(|| "no save".into()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(save, None, "no save");
                        for ab in Ability::ALL {
                            ui.selectable_value(save, Some(ab), format!("{ab:?}"));
                        }
                    });
                ui.add(egui::DragValue::new(dc).range(5..=30).prefix("DC "));
                ui.checkbox(half_on_success, g.i18n.t("events.half"));
            });
            if dnd_rules::DiceExpr::parse(dice).is_err() {
                ui.label(RichText::new(g.i18n.t("events.bad_dice")).color(theme::BLOOD));
            }
        }
        ScriptAction::GiveItem { item_id, quantity } => {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(format!("{id}-it"))
                    .selected_text(
                        dnd_rules::items::catalog_item(item_id)
                            .map(|i| i.name)
                            .unwrap_or_else(|| item_id.clone()),
                    )
                    .show_ui(ui, |ui| {
                        for it in dnd_rules::items::catalog() {
                            ui.selectable_value(item_id, it.id.clone(), &it.name);
                        }
                    });
                ui.add(egui::DragValue::new(quantity).range(1..=99));
            });
        }
        ScriptAction::SkillCheck {
            skill,
            dc,
            on_success,
            on_failure,
        } => {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(format!("{id}-sk"))
                    .selected_text(crate::character_ui::skill_name(*skill))
                    .show_ui(ui, |ui| {
                        for s in Skill::ALL {
                            ui.selectable_value(skill, s, crate::character_ui::skill_name(s));
                        }
                    });
                ui.add(egui::DragValue::new(dc).range(5..=30).prefix("DC "));
            });
            ui.label(RichText::new(g.i18n.t("events.on_success")).color(theme::HEALTH));
            ui.indent(format!("{id}-s"), |ui| {
                action_list(ui, g, view, on_success, room, &format!("{id}s"), depth + 1)
            });
            ui.label(RichText::new(g.i18n.t("events.on_failure")).color(theme::BLOOD));
            ui.indent(format!("{id}-f"), |ui| {
                action_list(ui, g, view, on_failure, room, &format!("{id}f"), depth + 1)
            });
        }
        ScriptAction::SetObjectOpen { object, open, .. } => {
            ui.horizontal(|ui| {
                ui.label(format!("#{}", object.0));
                if ui.button(g.i18n.t("events.use_selected_object")).clicked()
                    && let Some(o) = g.ui.sel_objects.first()
                {
                    *object = *o;
                }
                ui.selectable_value(open, None, g.i18n.t("events.toggle"));
                ui.selectable_value(open, Some(true), g.i18n.t("props.open"));
                ui.selectable_value(open, Some(false), g.i18n.t("events.close"));
            });
        }
        ScriptAction::RevealObject { object, .. } => {
            ui.horizontal(|ui| {
                ui.label(format!("#{}", object.0));
                if ui.button(g.i18n.t("events.use_selected_object")).clicked()
                    && let Some(o) = g.ui.sel_objects.first()
                {
                    *object = *o;
                }
            });
        }
        ScriptAction::RevealCreature { entity } => {
            egui::ComboBox::from_id_salt(format!("{id}-rc"))
                .selected_text(
                    view.creatures
                        .iter()
                        .find(|c| c.id == *entity)
                        .map(|c| c.name.clone())
                        .unwrap_or_default(),
                )
                .show_ui(ui, |ui| {
                    for c in view.creatures.iter().filter(|c| c.hidden) {
                        ui.selectable_value(entity, c.id, &c.name);
                    }
                });
        }
        ScriptAction::SetEdgeLocked { edge, locked } => {
            ui.horizontal(|ui| {
                let name = |e: &dnd_world::graph::RoomEdge| {
                    format!(
                        "{} ↔ {}",
                        view.graph
                            .nodes
                            .get(&e.from)
                            .map(|n| n.name.as_str())
                            .unwrap_or("?"),
                        view.graph
                            .nodes
                            .get(&e.to)
                            .map(|n| n.name.as_str())
                            .unwrap_or("?")
                    )
                };
                egui::ComboBox::from_id_salt(format!("{id}-ed"))
                    .selected_text(
                        view.graph
                            .edges
                            .iter()
                            .find(|e| e.id == *edge)
                            .map(name)
                            .unwrap_or_default(),
                    )
                    .show_ui(ui, |ui| {
                        for e in &view.graph.edges {
                            ui.selectable_value(edge, e.id, name(e));
                        }
                    });
                ui.checkbox(locked, g.i18n.t("world.locked"));
            });
        }
        ScriptAction::SpawnMonster {
            template,
            cell,
            hidden,
            ..
        } => {
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt(format!("{id}-sp"))
                    .selected_text(template.clone())
                    .height(300.0)
                    .show_ui(ui, |ui| {
                        for t in monster::srd_bestiary_ref() {
                            ui.selectable_value(template, t.id.clone(), &t.name);
                        }
                        if let Some(dm) = &view.dm {
                            for t in &dm.custom_monsters {
                                ui.selectable_value(
                                    template,
                                    t.id.clone(),
                                    format!("★ {}", t.name),
                                );
                            }
                        }
                    });
                ui.label(format!("@ ({}, {})", cell.0, cell.1));
                if ui.button(g.i18n.t("events.pick_cell")).clicked() {
                    g.ui.script_pick = Some(ScriptPick::SpawnCell);
                }
                ui.checkbox(hidden, g.i18n.t("props.hidden"));
            });
        }
        ScriptAction::Dialogue {
            speaker,
            text,
            choices,
        } => {
            ui.horizontal(|ui| {
                ui.label(g.i18n.t("events.speaker"));
                ui.text_edit_singleline(speaker);
            });
            ui.add(egui::TextEdit::multiline(text).desired_rows(2));
            let mut remove = None;
            for (i, c) in choices.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}.", i + 1));
                    ui.text_edit_singleline(&mut c.text);
                    if ui.small_button("✖").clicked() {
                        remove = Some(i);
                    }
                });
                ui.indent(format!("{id}-c{i}"), |ui| {
                    action_list(
                        ui,
                        g,
                        view,
                        &mut c.actions,
                        room,
                        &format!("{id}c{i}"),
                        depth + 1,
                    )
                });
            }
            if let Some(i) = remove {
                choices.remove(i);
            }
            if choices.len() < 6 && ui.button(g.i18n.t("events.add_choice")).clicked() {
                choices.push(DialogueChoice {
                    text: String::new(),
                    actions: vec![],
                });
            }
        }
        ScriptAction::SetAmbient { ambient, .. } => {
            ui.horizontal(|ui| {
                for (amb, key) in [
                    (AmbientLight::Bright, "light.bright"),
                    (AmbientLight::Dim, "light.dim"),
                    (AmbientLight::Darkness, "light.dark"),
                ] {
                    ui.selectable_value(ambient, amb, g.i18n.t(key));
                }
            });
        }
        ScriptAction::AwardXp { xp } => {
            ui.add(egui::DragValue::new(xp).range(0..=100000).suffix(" XP"));
        }
    }
}

/// Map click while the event editor waits for a location/object.
pub fn script_pick_click(
    ui_state: &mut crate::state::UiState,
    map: &dnd_world::map::GridMap,
    cell: (i32, i32),
) -> bool {
    let Some(pick) = ui_state.script_pick else {
        return false;
    };
    let Some(draft) = ui_state.script_draft.as_mut() else {
        ui_state.script_pick = None;
        return false;
    };
    match (pick, &mut draft.trigger) {
        (ScriptPick::AreaStart, Trigger::EnterArea { a, b, .. }) => {
            *a = cell;
            *b = cell;
            ui_state.script_pick = Some(ScriptPick::AreaEnd);
            return true;
        }
        (ScriptPick::AreaEnd, Trigger::EnterArea { b, .. }) => *b = cell,
        (ScriptPick::Object, Trigger::Interact { object, .. }) => {
            if let Some(o) = map.objects.iter().rev().find(|o| o.covers(cell)) {
                *object = o.id;
            }
        }
        (ScriptPick::SpawnCell, _) => {
            fn set_cell(actions: &mut [ScriptAction], cell: (i32, i32)) -> bool {
                for a in actions.iter_mut().rev() {
                    match a {
                        ScriptAction::SpawnMonster { cell: c, .. } => {
                            *c = cell;
                            return true;
                        }
                        ScriptAction::SkillCheck {
                            on_success,
                            on_failure,
                            ..
                        } => {
                            if set_cell(on_success, cell) || set_cell(on_failure, cell) {
                                return true;
                            }
                        }
                        ScriptAction::Dialogue { choices, .. } => {
                            for c in choices.iter_mut() {
                                if set_cell(&mut c.actions, cell) {
                                    return true;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                false
            }
            set_cell(&mut draft.actions, cell);
        }
        _ => {}
    }
    ui_state.script_pick = None;
    true
}

// ------------------------------------------------------------------ session tools

pub fn session_tools(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    theme::heading(ui, g.i18n.t("rest.title"));
    ui.horizontal(|ui| {
        let in_combat = view.combat.is_some();
        if ui
            .add_enabled(!in_combat, egui::Button::new(g.i18n.t("rest.short")))
            .on_hover_text(g.i18n.t("rest.short_help"))
            .clicked()
        {
            g.net.cmd(Command::Dm(DmCommand::Rest { long: false }));
        }
        if ui
            .add_enabled(!in_combat, egui::Button::new(g.i18n.t("rest.long")))
            .on_hover_text(g.i18n.t("rest.long_help"))
            .clicked()
        {
            g.net.cmd(Command::Dm(DmCommand::Rest { long: true }));
        }
    });
    let party: Vec<_> = view
        .creatures
        .iter()
        .filter(|c| c.faction == dnd_session::model::Faction::Party && !c.is_monster)
        .map(|c| c.id)
        .collect();
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(&mut g.ui.xp_award)
                .range(0..=100000)
                .suffix(" XP"),
        );
        if ui
            .button(g.i18n.t("xp.award_party"))
            .on_hover_text(g.i18n.t("xp.award_help"))
            .clicked()
        {
            let xp = g.ui.xp_award;
            g.net.cmd(Command::Dm(DmCommand::AwardXp {
                entities: party.clone(),
                xp,
            }));
        }
    });
    if let Some(sel) = g.ui.selected
        && party.contains(&sel)
    {
        ui.horizontal(|ui| {
            if ui
                .button(g.i18n.t("xp.milestone"))
                .on_hover_text(g.i18n.t("xp.milestone_help"))
                .clicked()
            {
                g.net
                    .cmd(Command::Dm(DmCommand::GrantLevel { entity: sel }));
            }
            egui::ComboBox::from_id_salt("dm-give")
                .selected_text(g.i18n.t("inv.give"))
                .show_ui(ui, |ui| {
                    for it in dnd_rules::items::catalog() {
                        if ui.button(&it.name).clicked() {
                            g.net.cmd(Command::Dm(DmCommand::GiveItemTo {
                                entity: sel,
                                item: it.id.clone(),
                                quantity: 1,
                            }));
                        }
                    }
                });
        });
    }
    if !view.effects.is_empty() {
        theme::heading(ui, g.i18n.t("effects.title"));
        for e in &view.effects {
            ui.horizontal(|ui| {
                let src = view
                    .creatures
                    .iter()
                    .find(|c| c.id == e.source)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();
                ui.label(format!(
                    "{}{} — {src}",
                    if e.concentration { "◎ " } else { "" },
                    e.label
                ));
                if ui
                    .small_button("✖")
                    .on_hover_text(g.i18n.t("effects.end"))
                    .clicked()
                {
                    g.net
                        .cmd(Command::Dm(DmCommand::EndEffect { effect: e.id }));
                }
            });
        }
    }
    room_audio(ui, g, view);
}

/// DM Hit Point panel for one creature: bar, amount field, Damage / Heal / Set HP, damage mode,
/// maximum and temporary HP, restore, set to 0 (confirmed), undo. Everything goes through
/// host-validated DM commands (`ManualHp`, `UndoHp`).
pub fn hp_panel(ui: &mut Ui, g: &mut GameUi, view: &ClientView, id: dnd_world::EntityId) {
    use dnd_session::command::HpOp;
    let Some(c) = view.creatures.iter().find(|c| c.id == id) else {
        return;
    };
    let dnd_session::view::HpView::Exact { current, max, temp } = c.hp else {
        return;
    };
    theme::heading(ui, &format!("{} · {}", g.i18n.t("hp.title"), c.name));
    let frac = if max > 0 {
        current as f32 / max as f32
    } else {
        0.0
    };
    theme::bar(
        ui,
        g.i18n.t("sheet.hp"),
        current,
        max,
        if frac <= 0.5 {
            theme::BURGUNDY
        } else {
            theme::HEALTH
        },
    );
    if temp > 0 {
        ui.label(
            RichText::new(format!("{}: {temp}", g.i18n.t("sheet.temp_hp")))
                .color(theme::MANA)
                .strong(),
        );
    }
    let send = |g: &mut GameUi, op: HpOp| {
        g.net
            .cmd(Command::Dm(DmCommand::ManualHp { entity: id, op }));
    };
    // Amount field: Enter applies damage; the field clears after every action so nothing is applied twice.
    let amount: Option<i32> = g.ui.hp_input.trim().parse().ok().filter(|v: &i32| *v > 0);
    let mut fire: Option<u8> = None;
    ui.horizontal(|ui| {
        let r = ui.add(
            egui::TextEdit::singleline(&mut g.ui.hp_input)
                .desired_width(70.0)
                .hint_text("0")
                .font(egui::FontId::new(18.0, egui::FontFamily::Proportional)),
        );
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && amount.is_some() {
            fire = Some(0);
        }
        ui.add_enabled_ui(amount.is_some(), |ui| {
            if ui
                .button(
                    RichText::new(format!("− {}", g.i18n.t("hp.damage")))
                        .color(egui::Color32::from_rgb(170, 40, 40))
                        .strong(),
                )
                .clicked()
            {
                fire = Some(0);
            }
            if ui
                .button(
                    RichText::new(format!("+ {}", g.i18n.t("hp.heal")))
                        .color(theme::HEALTH)
                        .strong(),
                )
                .clicked()
            {
                fire = Some(1);
            }
        });
        if ui
            .add_enabled(
                g.ui.hp_input.trim().parse::<i32>().is_ok(),
                egui::Button::new(g.i18n.t("hp.set")),
            )
            .clicked()
        {
            fire = Some(2);
        }
    });
    // Damage mode: rules (temp HP first, resistances) vs direct override.
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(g.i18n.t("hp.mode")).small());
        ui.selectable_value(&mut g.ui.hp_rules, true, g.i18n.t("hp.mode_rules"))
            .on_hover_text(g.i18n.t("hp.mode_rules_help"));
        ui.selectable_value(&mut g.ui.hp_rules, false, g.i18n.t("hp.mode_direct"))
            .on_hover_text(g.i18n.t("hp.mode_direct_help"));
        if g.ui.hp_rules {
            egui::ComboBox::from_id_salt("hp-type")
                .width(100.0)
                .selected_text(
                    g.ui.hp_type
                        .map(|t| format!("{t:?}"))
                        .unwrap_or_else(|| g.i18n.t("hp.untyped").to_string()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut g.ui.hp_type, None, g.i18n.t("hp.untyped"));
                    for t in dnd_rules::damage::DamageType::ALL {
                        ui.selectable_value(&mut g.ui.hp_type, Some(t), format!("{t:?}"));
                    }
                });
        }
    });
    let typed: Option<i32> = g.ui.hp_input.trim().parse().ok();
    match (fire, amount, typed) {
        (Some(0), Some(a), _) => send(
            g,
            HpOp::Damage {
                amount: a,
                rules: g.ui.hp_rules,
                damage_type: if g.ui.hp_rules { g.ui.hp_type } else { None },
            },
        ),
        (Some(1), Some(a), _) => send(g, HpOp::Heal { amount: a }),
        (Some(2), _, Some(v)) => send(g, HpOp::SetCurrent(v)),
        _ => {}
    }
    if fire.is_some() {
        g.ui.hp_input.clear();
    }
    // Maximum and temporary HP, restore, zero, undo.
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                typed.is_some_and(|v| v >= 1),
                egui::Button::new(g.i18n.t("hp.set_max")),
            )
            .on_hover_text(g.i18n.t("hp.uses_field"))
            .clicked()
        {
            send(g, HpOp::SetMax(typed.unwrap_or(1)));
            g.ui.hp_input.clear();
        }
        if ui
            .add_enabled(amount.is_some(), egui::Button::new(g.i18n.t("hp.add_temp")))
            .on_hover_text(g.i18n.t("hp.uses_field"))
            .clicked()
        {
            send(g, HpOp::SetTemp(temp + amount.unwrap_or(0)));
            g.ui.hp_input.clear();
        }
        if ui
            .add_enabled(
                typed.is_some_and(|v| v >= 0),
                egui::Button::new(g.i18n.t("hp.set_temp")),
            )
            .on_hover_text(g.i18n.t("hp.uses_field"))
            .clicked()
        {
            send(g, HpOp::SetTemp(typed.unwrap_or(0)));
            g.ui.hp_input.clear();
        }
        if temp > 0 && ui.button(g.i18n.t("hp.clear_temp")).clicked() {
            send(g, HpOp::SetTemp(0));
        }
    });
    ui.horizontal_wrapped(|ui| {
        if current < max && ui.button(g.i18n.t("hp.restore")).clicked() {
            send(g, HpOp::Restore);
        }
        if current > 0 {
            if g.ui.hp_confirm_zero == Some(id) {
                if ui
                    .button(
                        RichText::new(g.i18n.t("hp.zero_confirm"))
                            .color(egui::Color32::from_rgb(170, 40, 40))
                            .strong(),
                    )
                    .clicked()
                {
                    send(g, HpOp::Zero);
                    g.ui.hp_confirm_zero = None;
                }
                if ui.small_button(g.i18n.t("cast.cancel")).clicked() {
                    g.ui.hp_confirm_zero = None;
                }
            } else if ui.button(g.i18n.t("hp.zero")).clicked() {
                g.ui.hp_confirm_zero = Some(id);
            }
        }
        if let Some(name) = view.dm.as_ref().and_then(|d| d.hp_undo.clone())
            && ui
                .button(format!("{} ({name})", g.i18n.t("hp.undo")))
                .clicked()
        {
            g.net.cmd(Command::Dm(DmCommand::UndoHp));
        }
    });
}
