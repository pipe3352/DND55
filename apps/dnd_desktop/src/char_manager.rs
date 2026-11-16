//! Character Manager (main menu): a resizable, maximizable window with a responsive 3-panel
//! layout — list (search/create/delete) | portrait & summary | details/inventory/spells/abilities.
//! Narrow windows stack the panels in one scroll area. Size and maximized state are remembered in
//! `GameSettings` (`char_window`, `char_window_max`).

use crate::character_ui::{CharacterDraft, creator_ui};
use crate::hosting::open_store;
use crate::icons::Icon;
use crate::menu::MenuParams;
use crate::terrain_art::token_art::Side;
use crate::theme;
use bevy_egui::egui::{self, RichText, vec2};
use dnd_rules::ability::{Ability, Skill};
use dnd_rules::character::CharacterSheet;
use dnd_rules::content;

pub const MIN_SIZE: egui::Vec2 = vec2(640.0, 420.0);
/// Below this inner width the three panels stack vertically.
pub const STACK_BELOW: f32 = 900.0;

/// Window rectangle for the current viewport: maximized fills it (with a margin); otherwise the
/// remembered size, clamped so the window always fits and never goes below the minimum.
pub fn window_rect(viewport: egui::Rect, remembered: (f32, f32), maximized: bool) -> egui::Rect {
    let area = viewport.shrink(12.0);
    if maximized {
        return area;
    }
    let size = vec2(remembered.0, remembered.1)
        .max(MIN_SIZE)
        .min(area.size().max(MIN_SIZE));
    egui::Rect::from_center_size(area.center(), size)
}

pub fn window(ctx: &egui::Context, p: &mut MenuParams) {
    let max = p.settings.char_window_max;
    let rect = window_rect(ctx.viewport_rect(), p.settings.char_window, max);
    let mut open = true;
    let mut w = theme::win(theme::display(p.i18n.t("menu.characters"), 24.0))
        .id(egui::Id::new(("char-manager", max)))
        .open(&mut open)
        .collapsible(false)
        .min_size(MIN_SIZE)
        // Never larger than the screen: wide content scrolls instead of growing the window.
        .max_size(ctx.viewport_rect().shrink(12.0).size().max(MIN_SIZE))
        .constrain(true);
    w = if max {
        w.fixed_rect(rect).resizable(false)
    } else {
        w.default_rect(rect).resizable(true)
    };
    let shown = w.show(ctx, |ui| {
        theme::window_bg(ui);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (icon, key) = if max {
                    (Icon::Fit, "chars.restore")
                } else {
                    (Icon::Fit, "chars.maximize")
                };
                if theme::icon_text_button(ui, icon, p.i18n.t(key), true).clicked() {
                    p.settings.char_window_max = !max;
                }
            });
        });
        ui.separator();
        body(ui, p);
    });
    // Remember the size the user dragged to (once the drag is over, so settings aren't written
    // every frame).
    if let Some(r) = shown
        && !max
        && !ctx.input(|i| i.pointer.any_down())
    {
        let s = r.response.rect.size();
        let (ow, oh) = p.settings.char_window;
        if (s.x - ow).abs() > 2.0 || (s.y - oh).abs() > 2.0 {
            p.settings.char_window = (s.x.round(), s.y.round());
        }
    }
    if !open {
        p.ui.menu = crate::state::MenuPage::Main;
    }
}

fn body(ui: &mut egui::Ui, p: &mut MenuParams) {
    if ui.available_width() >= STACK_BELOW {
        egui::Panel::left("cm-list")
            .resizable(true)
            .default_size(250.0)
            .min_size(200.0)
            .max_size(380.0)
            .show(ui, |ui| list_panel(ui, p));
        if !p.draft.cm_editing {
            let w = ui.available_width();
            egui::Panel::right("cm-details")
                .resizable(true)
                .default_size((w * 0.42).max(280.0))
                .min_size(260.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("cm-details-scroll")
                        .show(ui, |ui| details_panel(ui, p));
                });
        }
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .max_width(ui.available_width())
            .id_salt("cm-center-scroll")
            .show(ui, |ui| center(ui, p));
    } else {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .max_width(ui.available_width())
            .id_salt("cm-stacked")
            .show(ui, |ui| {
                egui::CollapsingHeader::new(theme::gothic(p.i18n.t("chars.library"), 18.0))
                    .default_open(true)
                    .show(ui, |ui| list_panel(ui, p));
                ui.separator();
                center(ui, p);
                if !p.draft.cm_editing {
                    ui.separator();
                    details_panel(ui, p);
                }
            });
    }
}

fn select(draft: &mut CharacterDraft, c: &dnd_persist::StoredCharacter) {
    draft.sheet = c.sheet.clone();
    draft.portrait_tex = None;
    draft.portrait = c.portrait_png.clone();
    draft.library_id = Some(c.id);
    draft.cm_editing = false;
    draft.cm_confirm_delete = None;
    draft.message.clear();
}

fn list_panel(ui: &mut egui::Ui, p: &mut MenuParams) {
    ui.add(
        egui::TextEdit::singleline(&mut p.draft.cm_search)
            .hint_text(p.i18n.t("editor.search"))
            .desired_width(f32::INFINITY),
    );
    ui.horizontal_wrapped(|ui| {
        if theme::icon_text_button(ui, Icon::Plus, p.i18n.t("chars.new"), true).clicked() {
            let search = std::mem::take(&mut p.draft.cm_search);
            *p.draft = CharacterDraft::default();
            p.draft.cm_search = search;
            p.draft.cm_editing = true;
        }
        let sel = p.draft.library_id;
        if theme::icon_text_button(ui, Icon::Trash, p.i18n.t("chars.delete"), sel.is_some())
            .clicked()
        {
            p.draft.cm_confirm_delete = sel;
        }
    });
    if let Some(id) = p.draft.cm_confirm_delete {
        let name = p
            .cache
            .characters
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        ui.label(
            RichText::new(format!("{} «{name}»?", p.i18n.t("chars.delete_confirm")))
                .color(theme::BLOOD),
        );
        ui.horizontal(|ui| {
            if ui.button(p.i18n.t("chars.delete")).clicked() {
                match open_store().and_then(|s| s.delete_character(id).map_err(|e| e.to_string())) {
                    Ok(()) => {
                        let search = std::mem::take(&mut p.draft.cm_search);
                        *p.draft = CharacterDraft::default();
                        p.draft.cm_search = search;
                        p.cache.refresh();
                    }
                    Err(e) => p.draft.message = e,
                }
            }
            if ui.button(p.i18n.t("cast.cancel")).clicked() {
                p.draft.cm_confirm_delete = None;
            }
        });
    }
    ui.separator();
    let q = p.draft.cm_search.to_lowercase();
    let mut pick = None;
    egui::ScrollArea::vertical()
        .id_salt("cm-list-scroll")
        .show(ui, |ui| {
            let list: Vec<_> = p
                .cache
                .characters
                .iter()
                .filter(|c| {
                    q.is_empty()
                        || c.name.to_lowercase().contains(&q)
                        || c.sheet.class_def().name.to_lowercase().contains(&q)
                })
                .collect();
            if list.is_empty() {
                ui.label(theme::italic(p.i18n.t("chars.none")));
            }
            for c in list {
                let selected = p.draft.library_id == Some(c.id);
                let tex =
                    crate::tokens::token_egui(ui.ctx(), &c.sheet.token_icon(), Side::Friendly);
                let resp = ui
                    .horizontal(|ui| {
                        ui.image((tex.id(), vec2(30.0, 30.0)));
                        ui.vertical(|ui| {
                            ui.add(egui::Label::new(theme::bold(&c.name)).selectable(false));
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!(
                                        "{} {} · {}",
                                        p.i18n.t("sheet.level"),
                                        c.sheet.level,
                                        c.sheet.class_def().name
                                    ))
                                    .small(),
                                )
                                .selectable(false),
                            );
                        });
                    })
                    .response;
                let resp = resp.interact(egui::Sense::click());
                if selected || resp.hovered() {
                    ui.painter().rect_stroke(
                        resp.rect.expand(2.0),
                        3.0,
                        egui::Stroke::new(if selected { 2.0 } else { 1.0 }, theme::BURGUNDY),
                        egui::StrokeKind::Outside,
                    );
                }
                if resp.clicked() {
                    pick = Some(c.clone());
                }
            }
        });
    if let Some(c) = pick {
        select(&mut p.draft, &c);
    }
}

fn center(ui: &mut egui::Ui, p: &mut MenuParams) {
    if p.draft.cm_editing {
        ui.horizontal(|ui| {
            theme::heading(
                ui,
                p.i18n.t(if p.draft.library_id.is_some() {
                    "chars.editing"
                } else {
                    "chars.creating"
                }),
            );
            if ui.button(p.i18n.t("chars.close_editor")).clicked() {
                p.draft.cm_editing = false;
            }
        });
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
                    p.draft.cm_editing = false;
                    p.cache.refresh();
                }
                Err(e) => p.draft.message = e,
            }
        }
        return;
    }
    if p.draft.library_id.is_none() {
        ui.add_space(30.0);
        ui.vertical_centered(|ui| {
            ui.label(theme::italic(p.i18n.t("chars.pick_hint")));
        });
        return;
    }
    if !p.draft.message.is_empty() {
        ui.label(theme::italic(&p.draft.message).color(theme::FOREST));
    }
    let s = p.draft.sheet.clone();
    ui.vertical_centered(|ui| {
        if p.draft.portrait_tex.is_none()
            && let Some(png) = &p.draft.portrait
        {
            p.draft.portrait_tex = crate::character_ui::load_portrait_tex(ui.ctx(), png);
        }
        let side = (ui.available_width() * 0.5).clamp(96.0, 220.0);
        match &p.draft.portrait_tex {
            Some(t) => ui.image((t.id(), vec2(side, side))),
            None => ui.image((
                crate::tokens::token_egui(ui.ctx(), &s.token_icon(), Side::Friendly).id(),
                vec2(side, side),
            )),
        };
        ui.label(theme::display(&s.name, 32.0).color(theme::WOOD));
        ui.label(theme::italic(format!(
            "{} {} · {} {}",
            content::species(s.species).name,
            s.class_def().name,
            p.i18n.t("sheet.level"),
            s.level
        )));
        if let Some(sub) = s.subclass_name() {
            ui.label(RichText::new(sub).small());
        }
    });
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        theme::stat_chip(
            ui,
            Icon::Heart,
            &s.max_hp().to_string(),
            p.i18n.t("sheet.hp"),
        );
        theme::stat_chip(
            ui,
            Icon::Shield,
            &s.armor_class().to_string(),
            p.i18n.t("sheet.ac"),
        );
        theme::stat_chip(
            ui,
            Icon::Feather,
            &s.speed_ft().to_string(),
            p.i18n.t("sheet.speed"),
        );
        theme::stat_chip(
            ui,
            Icon::Star,
            &format!("{:+}", s.proficiency_bonus()),
            p.i18n.t("sheet.proficiency"),
        );
        theme::stat_chip(
            ui,
            Icon::Hourglass,
            &format!("{:+}", s.initiative_bonus()),
            p.i18n.t("sheet.initiative"),
        );
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for a in Ability::ALL {
            let abbrev: String = format!("{a:?}")
                .chars()
                .take(3)
                .collect::<String>()
                .to_uppercase();
            theme::stat_medallion(
                ui,
                crate::icons::ability_icon(a),
                &abbrev,
                s.scores().get(a),
                s.modifier(a),
            );
        }
    });
    ui.add_space(6.0);
    if theme::icon_text_button(ui, Icon::Quill, p.i18n.t("chars.edit"), true).clicked() {
        p.draft.cm_editing = true;
        p.draft.step = 0;
    }
}

fn details_panel(ui: &mut egui::Ui, p: &mut MenuParams) {
    if p.draft.library_id.is_none() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for (i, key) in [
            "chars.tab_details",
            "sheet.inventory",
            "chars.tab_spells",
            "sheet.abilities",
        ]
        .iter()
        .enumerate()
        {
            if ui
                .selectable_label(
                    p.draft.cm_tab == i as u8,
                    theme::gothic(p.i18n.t(key), 17.0),
                )
                .clicked()
            {
                p.draft.cm_tab = i as u8;
            }
        }
    });
    ui.separator();
    let s = p.draft.sheet.clone();
    match p.draft.cm_tab {
        0 => details(ui, &s, p),
        1 => inventory(ui, &s, p),
        2 => spells(ui, &s, p),
        _ => abilities(ui, &s, p),
    }
    ui.add_space(8.0);
    if theme::icon_text_button(ui, Icon::Quill, p.i18n.t("chars.edit"), true).clicked() {
        p.draft.cm_editing = true;
        p.draft.step = 0;
    }
}

fn details(ui: &mut egui::Ui, s: &CharacterSheet, p: &MenuParams) {
    let xp_next = dnd_rules::classes::XP_THRESHOLDS[(s.level as usize).min(19)] as i32;
    theme::bar(
        ui,
        p.i18n.t("sheet.xp"),
        s.experience_points as i32,
        xp_next,
        theme::GOLD,
    );
    egui::Grid::new("cm-details-grid")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label(theme::bold(p.i18n.t("creator.background")));
            ui.label(s.background_def().name);
            ui.end_row();
            ui.label(theme::bold(p.i18n.t("creator.species")));
            ui.label(content::species(s.species).name);
            ui.end_row();
            ui.label(theme::bold(p.i18n.t("sheet.passive_perception")));
            ui.label(s.passive_perception().to_string());
            ui.end_row();
            if s.darkvision_ft() > 0 {
                ui.label(theme::bold(p.i18n.t("sheet.darkvision")));
                ui.label(format!("{} ft", s.darkvision_ft()));
                ui.end_row();
            }
        });
    if !s.biography.trim().is_empty() {
        ui.add_space(6.0);
        ui.label(theme::bold(p.i18n.t("creator.biography")));
        ui.label(theme::italic(&s.biography));
    }
}

fn inventory(ui: &mut egui::Ui, s: &CharacterSheet, p: &MenuParams) {
    if let Some(a) = &s.body_armor {
        ui.label(format!(
            "{} — {}",
            p.i18n.t("creator.armor"),
            dnd_rules::equipment::armor(a).map(|x| x.name).unwrap_or(a)
        ));
    }
    if s.shield {
        ui.label(p.i18n.t("creator.shield"));
    }
    for w in &s.weapons {
        ui.label(format!(
            "• {}",
            dnd_rules::equipment::weapon(w).map(|x| x.name).unwrap_or(w)
        ));
    }
    for it in &s.inventory {
        ui.label(format!("• {} ×{}", it.name, it.quantity));
    }
    let c = &s.currency;
    ui.label(theme::bold(format!(
        "{} pp · {} gp · {} ep · {} sp · {} cp",
        c.pp, c.gp, c.ep, c.sp, c.cp
    )));
}

fn spells(ui: &mut egui::Ui, s: &CharacterSheet, p: &MenuParams) {
    let Some(dc) = s.spell_save_dc() else {
        ui.label(theme::italic(p.i18n.t("chars.no_spells")));
        return;
    };
    ui.label(format!(
        "{} {dc} · {} {:+}",
        p.i18n.t("sheet.spell_dc"),
        p.i18n.t("sheet.spell_attack"),
        s.spell_attack_bonus().unwrap_or(0)
    ));
    let slots = s.spell_slots();
    let txt: Vec<String> = slots
        .iter()
        .enumerate()
        .filter(|(_, n)| **n > 0)
        .map(|(l, n)| format!("{}: {n}", l + 1))
        .collect();
    if !txt.is_empty() {
        ui.label(format!("{} — {}", p.i18n.t("sheet.slots"), txt.join(" · ")));
    }
    let mut list: Vec<(u8, String)> = s
        .cantrips
        .iter()
        .chain(s.all_prepared().iter())
        .filter_map(|id| dnd_rules::spells::spell(id).map(|sp| (sp.level, sp.name.clone())))
        .collect();
    list.sort();
    list.dedup();
    for (lvl, name) in list {
        let tag = if lvl == 0 {
            p.i18n.t("chars.cantrip").to_string()
        } else {
            format!("{lvl}")
        };
        ui.label(format!("[{tag}] {name}"));
    }
}

fn abilities(ui: &mut egui::Ui, s: &CharacterSheet, p: &MenuParams) {
    ui.label(theme::bold(p.i18n.t("sheet.saving_throws")));
    ui.horizontal_wrapped(|ui| {
        for a in Ability::ALL {
            ui.label(format!("{a:?} {:+}", s.save_bonus(a)));
        }
    });
    ui.add_space(4.0);
    ui.label(theme::bold(p.i18n.t("sheet.skills")));
    egui::Grid::new("cm-skills").num_columns(2).show(ui, |ui| {
        for sk in Skill::ALL {
            let prof = s.skill_proficient(sk);
            let t = format!("{sk:?}");
            ui.label(if prof {
                theme::bold(t)
            } else {
                RichText::new(t)
            });
            ui.label(format!("{:+}", s.skill_bonus(sk)));
            ui.end_row();
        }
    });
    if !s.weapon_masteries.is_empty() {
        ui.label(format!(
            "{}: {}",
            p.i18n.t("creator.masteries"),
            s.weapon_masteries.join(", ")
        ));
    }
    if let Some(fs) = &s.fighting_style {
        ui.label(format!("{fs:?}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_respects_minimum_viewport_and_maximize() {
        let vp = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1600.0, 900.0));
        let r = window_rect(vp, (1100.0, 720.0), false);
        assert_eq!(r.size(), vec2(1100.0, 720.0));
        assert!(vp.contains_rect(r));
        // Remembered size larger than the screen is clamped; smaller than the minimum is raised.
        assert!(vp.contains_rect(window_rect(vp, (4000.0, 3000.0), false)));
        assert_eq!(window_rect(vp, (100.0, 100.0), false).size(), MIN_SIZE);
        // Maximized fills the viewport minus the margin.
        assert_eq!(window_rect(vp, (800.0, 600.0), true), vp.shrink(12.0));
    }
}

/// `DND55_SEED_DB=<data dir>/campaigns.db cargo test seed_demo_characters -- --ignored` adds two
/// example characters (for manual and screenshot checks of the Character Manager).
#[cfg(test)]
#[test]
#[ignore]
fn seed_demo_characters() {
    let Ok(path) = std::env::var("DND55_SEED_DB") else {
        return;
    };
    let store = dnd_persist::Store::open(std::path::Path::new(&path)).unwrap();
    let mut a = CharacterSheet::example_fighter("Aldric the Bold");
    a.token_icon = Some("knight".into());
    a.biography = "A knight of the fallen order, sworn to guard the old roads.".into();
    store.save_character(None, &a, None).unwrap();
    store
        .save_character(None, &CharacterSheet::example_wizard("Mirela"), None)
        .unwrap();
}
