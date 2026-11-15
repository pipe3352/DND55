//! Character creator (2024 rules, validated live) and the in-game character sheet.

use crate::i18n::I18n;
use crate::state::{Net, UiState};
use crate::theme;
use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use dnd_rules::ability::{Ability, Skill};
use dnd_rules::character::{
    BackgroundBonus, CharacterSheet, POINT_BUY_BUDGET, STANDARD_ARRAY, ScoreMethod, Severity,
    point_cost,
};
use dnd_rules::content::{self, BACKGROUNDS, CLASSES, SPECIES};
use dnd_rules::d20::RollMode;
use dnd_rules::equipment::{ARMOR, ArmorCategory, WEAPONS};
use dnd_session::command::Command;
use dnd_session::view::{CreatureView, HpView};

#[derive(Resource)]
pub struct CharacterDraft {
    pub sheet: CharacterSheet,
    pub portrait: Option<Vec<u8>>,
    pub portrait_tex: Option<egui::TextureHandle>,
    pub library_id: Option<i64>,
    pub step: usize,
    pub message: String,
}

impl Default for CharacterDraft {
    fn default() -> Self {
        let mut sheet = CharacterSheet::example_fighter("");
        sheet.biography.clear();
        CharacterDraft {
            sheet,
            portrait: None,
            portrait_tex: None,
            library_id: None,
            step: 0,
            message: String::new(),
        }
    }
}

fn ability_label(i18n: &I18n, a: Ability) -> &str {
    i18n.t(match a {
        Ability::Strength => "ability.str",
        Ability::Dexterity => "ability.dex",
        Ability::Constitution => "ability.con",
        Ability::Intelligence => "ability.int",
        Ability::Wisdom => "ability.wis",
        Ability::Charisma => "ability.cha",
    })
}

pub fn skill_name(s: Skill) -> String {
    let dbg = format!("{s:?}");
    let mut out = String::new();
    for (i, ch) in dbg.chars().enumerate() {
        if i > 0 && ch.is_uppercase() {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

fn load_portrait_tex(ctx: &egui::Context, png: &[u8]) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(png)
        .ok()?
        .thumbnail(192, 192)
        .to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(ctx.load_texture(
        "portrait-preview",
        egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()),
        egui::TextureOptions::LINEAR,
    ))
}

/// The creator. Returns `true` when the user pressed "Save to library".
pub fn creator_ui(ui: &mut egui::Ui, d: &mut CharacterDraft, i18n: &I18n) -> bool {
    let steps = [
        "creator.step.class",
        "creator.step.origin",
        "creator.step.abilities",
        "creator.step.skills",
        "creator.step.equipment",
        "creator.step.details",
    ];
    let mut saved = false;
    ui.horizontal(|ui| {
        for (i, s) in steps.iter().enumerate() {
            let txt = format!("{}. {}", i + 1, i18n.t(s));
            if ui.selectable_label(d.step == i, txt).clicked() {
                d.step = i;
            }
        }
    });
    ui.separator();
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        egui::ScrollArea::vertical()
            .id_salt("creator-left")
            .max_height(460.0)
            .show(ui, |ui| match d.step {
                0 => step_class(ui, d, i18n),
                1 => step_origin(ui, d, i18n),
                2 => step_abilities(ui, d, i18n),
                3 => step_skills(ui, d, i18n),
                4 => step_equipment(ui, d, i18n),
                _ => step_details(ui, d, i18n),
            });
        ui.horizontal(|ui| {
            if d.step > 0 && ui.button(i18n.t("creator.back")).clicked() {
                d.step -= 1;
            }
            if d.step < steps.len() - 1 && ui.button(i18n.t("creator.next")).clicked() {
                d.step += 1;
            }
        });
        let ui = &mut cols[1];
        summary(ui, d, i18n);
        let errors = d
            .sheet
            .validate()
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count();
        ui.add_space(8.0);
        if ui
            .add_enabled(
                errors == 0,
                egui::Button::new(RichText::new(i18n.t("creator.save")).strong()),
            )
            .clicked()
        {
            saved = true;
        }
        if !d.message.is_empty() {
            ui.label(&d.message);
        }
    });
    saved
}

fn step_class(ui: &mut egui::Ui, d: &mut CharacterDraft, i18n: &I18n) {
    theme::heading(ui, i18n.t("creator.step.class"));
    for c in CLASSES {
        let selected = d.sheet.class == c.id;
        let text = format!(
            "{}  —  d{} HP, saves {:?}/{:?}",
            c.name, c.hit_die, c.saving_throws[0], c.saving_throws[1]
        );
        if ui.selectable_label(selected, text).clicked() && !selected {
            d.sheet.class = c.id;
            d.sheet.class_skills.clear();
            d.sheet.weapon_masteries.clear();
        }
    }
    ui.label(
        RichText::new(i18n.t("creator.class_note"))
            .italics()
            .small(),
    );
}

fn step_origin(ui: &mut egui::Ui, d: &mut CharacterDraft, i18n: &I18n) {
    theme::heading(ui, i18n.t("creator.species"));
    egui::ComboBox::from_id_salt("species")
        .selected_text(content::species(d.sheet.species).name)
        .show_ui(ui, |ui| {
            for s in SPECIES {
                if ui
                    .selectable_value(&mut d.sheet.species, s.id, s.name)
                    .clicked()
                {
                    d.sheet.size = s.sizes[0];
                    if s.id != content::SpeciesId::Human {
                        d.sheet.species_skill = None;
                    }
                }
            }
        });
    let sp = content::species(d.sheet.species);
    ui.label(format!(
        "{} {} ft · {} {}",
        i18n.t("sheet.speed"),
        sp.speed_ft,
        i18n.t("sheet.darkvision"),
        if sp.darkvision_ft > 0 {
            format!("{} ft", sp.darkvision_ft)
        } else {
            "—".into()
        }
    ));
    if sp.sizes.len() > 1 {
        ui.horizontal(|ui| {
            ui.label(i18n.t("creator.size"));
            for s in sp.sizes {
                ui.selectable_value(&mut d.sheet.size, *s, format!("{s:?}"));
            }
        });
    }
    if d.sheet.species == content::SpeciesId::Human {
        ui.horizontal(|ui| {
            ui.label(i18n.t("creator.skillful"));
            egui::ComboBox::from_id_salt("human-skill")
                .selected_text(
                    d.sheet
                        .species_skill
                        .map(skill_name)
                        .unwrap_or_else(|| "—".into()),
                )
                .show_ui(ui, |ui| {
                    for s in Skill::ALL {
                        ui.selectable_value(&mut d.sheet.species_skill, Some(s), skill_name(s));
                    }
                });
        });
    }
    ui.add_space(6.0);
    theme::heading(ui, i18n.t("creator.background"));
    egui::ComboBox::from_id_salt("bg")
        .selected_text(content::background(d.sheet.background).name)
        .show_ui(ui, |ui| {
            for b in BACKGROUNDS {
                if ui
                    .selectable_value(&mut d.sheet.background, b.id, b.name)
                    .clicked()
                {
                    d.sheet.background_bonus = BackgroundBonus::PlusTwoPlusOne {
                        plus_two: b.abilities[0],
                        plus_one: b.abilities[1],
                    };
                }
            }
        });
    let bg = content::background(d.sheet.background);
    ui.label(format!(
        "{}: {:?} · {}: {} & {}",
        i18n.t("creator.feat"),
        bg.feat,
        i18n.t("creator.skills"),
        skill_name(bg.skills[0]),
        skill_name(bg.skills[1])
    ));
    ui.label(i18n.t("creator.bg_bonus"));
    let mut three = matches!(d.sheet.background_bonus, BackgroundBonus::ThreePlusOnes);
    ui.horizontal(|ui| {
        if ui.radio(!three, "+2 / +1").clicked() {
            three = false;
            d.sheet.background_bonus = BackgroundBonus::PlusTwoPlusOne {
                plus_two: bg.abilities[0],
                plus_one: bg.abilities[1],
            };
        }
        if ui.radio(three, "+1 / +1 / +1").clicked() {
            d.sheet.background_bonus = BackgroundBonus::ThreePlusOnes;
        }
    });
    if let BackgroundBonus::PlusTwoPlusOne { plus_two, plus_one } = &mut d.sheet.background_bonus {
        ui.horizontal(|ui| {
            ui.label("+2");
            for a in bg.abilities {
                ui.selectable_value(plus_two, a, ability_label(i18n, a));
            }
        });
        ui.horizontal(|ui| {
            ui.label("+1");
            for a in bg.abilities {
                ui.selectable_value(plus_one, a, ability_label(i18n, a));
            }
        });
    }
}

fn step_abilities(ui: &mut egui::Ui, d: &mut CharacterDraft, i18n: &I18n) {
    theme::heading(ui, i18n.t("creator.step.abilities"));
    ui.horizontal(|ui| {
        if ui
            .radio(
                d.sheet.score_method == ScoreMethod::StandardArray,
                i18n.t("creator.standard_array"),
            )
            .clicked()
        {
            d.sheet.score_method = ScoreMethod::StandardArray;
            d.sheet.base_scores.0 = STANDARD_ARRAY;
        }
        if ui
            .radio(
                d.sheet.score_method == ScoreMethod::PointBuy,
                i18n.t("creator.point_buy"),
            )
            .clicked()
        {
            d.sheet.score_method = ScoreMethod::PointBuy;
            d.sheet.base_scores.0 = [8; 6];
        }
    });
    ui.label(
        RichText::new(i18n.t("creator.rolled_note"))
            .small()
            .italics(),
    );
    egui::Grid::new("abilities")
        .num_columns(4)
        .striped(true)
        .show(ui, |ui| {
            let finals = d.sheet.scores();
            for a in Ability::ALL {
                ui.label(ability_label(i18n, a));
                let v = &mut d.sheet.base_scores.0[a.index()];
                match d.sheet.score_method {
                    ScoreMethod::StandardArray => {
                        egui::ComboBox::from_id_salt(("sa", a.index()))
                            .width(50.0)
                            .selected_text(v.to_string())
                            .show_ui(ui, |ui| {
                                for s in STANDARD_ARRAY {
                                    ui.selectable_value(v, s, s.to_string());
                                }
                            });
                    }
                    _ => {
                        ui.add(egui::DragValue::new(v).range(8..=15));
                    }
                }
                let f = finals.get(a);
                ui.label(format!("▶ {f}"));
                ui.label(RichText::new(format!("{:+}", dnd_rules::ability_modifier(f))).strong());
                ui.end_row();
            }
        });
    if d.sheet.score_method == ScoreMethod::PointBuy {
        let spent: u32 = d
            .sheet
            .base_scores
            .0
            .iter()
            .filter_map(|s| point_cost(*s))
            .sum();
        ui.label(format!(
            "{}: {spent} / {POINT_BUY_BUDGET}",
            i18n.t("creator.points")
        ));
    }
}

fn step_skills(ui: &mut egui::Ui, d: &mut CharacterDraft, i18n: &I18n) {
    let cd = content::class(d.sheet.class);
    theme::heading(
        ui,
        &format!(
            "{} ({} {})",
            i18n.t("creator.skills"),
            cd.skill_choices,
            i18n.t("creator.to_choose")
        ),
    );
    let bg = content::background(d.sheet.background);
    let options: Vec<Skill> = if cd.skill_list.is_empty() {
        Skill::ALL.to_vec()
    } else {
        cd.skill_list.to_vec()
    };
    for s in options {
        let from_bg = bg.skills.contains(&s);
        let mut on = d.sheet.class_skills.contains(&s);
        let full = d.sheet.class_skills.len() >= cd.skill_choices as usize;
        let label = if from_bg {
            format!("{} ({})", skill_name(s), i18n.t("creator.from_background"))
        } else {
            skill_name(s)
        };
        if ui
            .add_enabled(
                !from_bg && (on || !full),
                egui::Checkbox::new(&mut on, label),
            )
            .changed()
        {
            if on {
                d.sheet.class_skills.push(s);
            } else {
                d.sheet.class_skills.retain(|x| *x != s);
            }
        }
    }
}

fn step_equipment(ui: &mut egui::Ui, d: &mut CharacterDraft, i18n: &I18n) {
    let cd = content::class(d.sheet.class);
    theme::heading(ui, i18n.t("creator.armor"));
    let current = d
        .sheet
        .body_armor
        .as_deref()
        .and_then(dnd_rules::equipment::armor)
        .map(|a| a.name)
        .unwrap_or("—");
    egui::ComboBox::from_id_salt("armor")
        .selected_text(current)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut d.sheet.body_armor, None, "—");
            for a in ARMOR
                .iter()
                .filter(|a| a.category != ArmorCategory::Shield && cd.trained_in_armor(a.category))
            {
                ui.selectable_value(
                    &mut d.sheet.body_armor,
                    Some(a.id.to_string()),
                    format!("{} (AC {})", a.name, a.base_ac),
                );
            }
        });
    ui.add_enabled(
        cd.trained_in_armor(ArmorCategory::Shield),
        egui::Checkbox::new(&mut d.sheet.shield, i18n.t("creator.shield")),
    );
    theme::heading(ui, i18n.t("creator.weapons"));
    ui.label(format!(
        "{} {}",
        i18n.t("creator.masteries"),
        cd.weapon_mastery_count
    ));
    egui::Grid::new("weapons").num_columns(3).show(ui, |ui| {
        for w in WEAPONS.iter().filter(|w| cd.proficient_with(w)) {
            let mut carried = d.sheet.weapons.iter().any(|x| x == w.id);
            if ui
                .checkbox(
                    &mut carried,
                    format!(
                        "{} ({}d{} {:?})",
                        w.name, w.dice_count, w.dice_sides, w.damage_type
                    ),
                )
                .changed()
            {
                if carried {
                    d.sheet.weapons.push(w.id.into());
                } else {
                    d.sheet.weapons.retain(|x| x != w.id);
                    d.sheet.weapon_masteries.retain(|x| x != w.id);
                }
            }
            let mut mastered = d.sheet.weapon_masteries.iter().any(|x| x == w.id);
            let can = cd.weapon_mastery_count > 0
                && (mastered || d.sheet.weapon_masteries.len() < cd.weapon_mastery_count as usize)
                && !(d.sheet.class == content::ClassId::Barbarian
                    && w.kind != dnd_rules::equipment::WeaponKind::Melee);
            if ui
                .add_enabled(
                    can,
                    egui::Checkbox::new(&mut mastered, format!("{:?}", w.mastery)),
                )
                .changed()
            {
                if mastered {
                    d.sheet.weapon_masteries.push(w.id.into());
                } else {
                    d.sheet.weapon_masteries.retain(|x| x != w.id);
                }
            }
            ui.end_row();
        }
    });
}

fn step_details(ui: &mut egui::Ui, d: &mut CharacterDraft, i18n: &I18n) {
    theme::heading(ui, i18n.t("creator.step.details"));
    ui.horizontal(|ui| {
        ui.label(i18n.t("creator.name"));
        ui.text_edit_singleline(&mut d.sheet.name);
    });
    ui.horizontal(|ui| {
        if ui.button(i18n.t("creator.portrait")).clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG", &["png"])
                .pick_file()
        {
            match std::fs::read(&path) {
                Ok(bytes) if bytes.len() <= dnd_session::assets::MAX_UPLOAD_BYTES => {
                    d.portrait_tex = load_portrait_tex(ui.ctx(), &bytes);
                    if d.portrait_tex.is_some() {
                        d.portrait = Some(bytes);
                        d.message.clear();
                    } else {
                        d.message = i18n.t("creator.portrait_invalid").into();
                    }
                }
                _ => d.message = i18n.t("creator.portrait_invalid").into(),
            }
        }
        if d.portrait.is_some() && ui.button(i18n.t("creator.remove_portrait")).clicked() {
            d.portrait = None;
            d.portrait_tex = None;
        }
    });
    if let Some(t) = &d.portrait_tex {
        ui.image((t.id(), egui::vec2(96.0, 96.0)));
    }
    ui.label(i18n.t("creator.biography"));
    ui.add(egui::TextEdit::multiline(&mut d.sheet.biography).desired_rows(5));
}

fn summary(ui: &mut egui::Ui, d: &CharacterDraft, i18n: &I18n) {
    let s = &d.sheet;
    theme::heading(
        ui,
        if s.name.trim().is_empty() {
            i18n.t("creator.unnamed")
        } else {
            s.name.as_str()
        },
    );
    ui.label(format!(
        "{} · {} · {} · {} 1",
        content::species(s.species).name,
        content::background(s.background).name,
        s.class_def().name,
        i18n.t("sheet.level")
    ));
    egui::Grid::new("summary").num_columns(2).show(ui, |ui| {
        ui.label(i18n.t("sheet.hp"));
        ui.label(s.max_hp().to_string());
        ui.end_row();
        ui.label(i18n.t("sheet.ac"));
        ui.label(s.armor_class().to_string());
        ui.end_row();
        ui.label(i18n.t("sheet.initiative"));
        ui.label(format!("{:+}", s.initiative_bonus()));
        ui.end_row();
        ui.label(i18n.t("sheet.speed"));
        ui.label(format!("{} ft", s.speed_ft()));
        ui.end_row();
        ui.label(i18n.t("sheet.proficiency"));
        ui.label(format!("{:+}", s.proficiency_bonus()));
        ui.end_row();
    });
    ui.label(RichText::new(i18n.t("sheet.attacks")).strong());
    for a in s.attacks() {
        ui.label(a.summary());
    }
    let issues = s.validate();
    if !issues.is_empty() {
        ui.add_space(4.0);
        for i in issues {
            let color = if i.severity == Severity::Error {
                theme::BLOOD
            } else {
                egui::Color32::from_rgb(150, 100, 20)
            };
            ui.label(
                RichText::new(format!("• {}", i.detail))
                    .color(color)
                    .small(),
            );
        }
    }
}

/// In-game character sheet. Clicking a skill/save/attack rolls it through the host.
pub fn sheet_ui(
    ui: &mut egui::Ui,
    sheet: &CharacterSheet,
    me: Option<&CreatureView>,
    net: &mut Net,
    uis: &mut UiState,
    i18n: &I18n,
) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&sheet.name)
                .size(20.0)
                .strong()
                .color(theme::WOOD),
        );
        ui.label(format!(
            "{} {} · {}",
            content::species(sheet.species).name,
            sheet.class_def().name,
            i18n.t("sheet.level")
        ));
        ui.label(sheet.level.to_string());
    });
    if let Some(c) = me {
        if let HpView::Exact { current, max, temp } = c.hp {
            theme::bar(ui, i18n.t("sheet.hp"), current, max, theme::BLOOD);
            if temp > 0 {
                ui.label(format!("{}: {temp}", i18n.t("sheet.temp_hp")));
            }
            if current == 0 {
                ui.label(
                    RichText::new(format!(
                        "{}: ✔{} ✖{}",
                        i18n.t("sheet.death_saves"),
                        c.death_saves.0,
                        c.death_saves.1
                    ))
                    .color(theme::BLOOD),
                );
            }
        }
        if !c.conditions.is_empty() {
            ui.label(format!(
                "{}: {:?}",
                i18n.t("sheet.conditions"),
                c.conditions
            ));
        }
    }
    let xp_next = 300; // level 2 threshold (SRD Character Advancement table)
    theme::bar(
        ui,
        i18n.t("sheet.xp"),
        sheet.experience_points as i32,
        xp_next,
        theme::GOLD,
    );
    ui.horizontal(|ui| {
        ui.label(format!("{} {}", i18n.t("sheet.ac"), sheet.armor_class()));
        ui.label(format!(
            "{} {:+}",
            i18n.t("sheet.initiative"),
            sheet.initiative_bonus()
        ));
        ui.label(format!("{} {} ft", i18n.t("sheet.speed"), sheet.speed_ft()));
        ui.label(format!(
            "{} {:+}",
            i18n.t("sheet.proficiency"),
            sheet.proficiency_bonus()
        ));
    });
    let roll = |net: &mut Net, label: String, bonus: i32| {
        net.cmd(Command::RollDice {
            expr: format!("1d20{bonus:+}"),
            mode: uis.dice_mode,
            private: uis.private_roll,
            label,
        });
    };
    egui::CollapsingHeader::new(i18n.t("sheet.abilities"))
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("sheet-ab")
                .num_columns(4)
                .striped(true)
                .show(ui, |ui| {
                    for a in Ability::ALL {
                        let score = sheet.scores().get(a);
                        if ui
                            .button(format!("{} {score}", ability_label(i18n, a)))
                            .on_hover_text(i18n.t("sheet.click_check"))
                            .clicked()
                        {
                            roll(
                                net,
                                format!("{} check", ability_label(i18n, a)),
                                sheet.modifier(a),
                            );
                        }
                        ui.label(format!("{:+}", sheet.modifier(a)));
                        let save = sheet.save_bonus(a);
                        if ui
                            .button(format!("{} {save:+}", i18n.t("sheet.save")))
                            .clicked()
                        {
                            roll(net, format!("{} save", ability_label(i18n, a)), save);
                        }
                        ui.end_row();
                    }
                });
        });
    egui::CollapsingHeader::new(i18n.t("sheet.skills"))
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("sheet-sk")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for s in Skill::ALL {
                        let prof = if sheet.skill_proficient(s) {
                            "•"
                        } else {
                            "○"
                        };
                        let b = sheet.skill_bonus(s);
                        if ui.button(format!("{prof} {}", skill_name(s))).clicked() {
                            roll(net, skill_name(s), b);
                        }
                        ui.label(format!("{b:+}"));
                        ui.end_row();
                    }
                });
            ui.label(format!(
                "{}: {}",
                i18n.t("sheet.passive_perception"),
                sheet.passive_perception()
            ));
        });
    egui::CollapsingHeader::new(i18n.t("sheet.attacks"))
        .default_open(true)
        .show(ui, |ui| {
            let attacks = me
                .map(|c| c.attacks.clone())
                .unwrap_or_else(|| sheet.attacks());
            for (i, a) in attacks.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui
                        .button(a.summary())
                        .on_hover_text(i18n.t("sheet.click_attack"))
                        .clicked()
                    {
                        uis.targeting = Some(i);
                        uis.selected = me.map(|c| c.id);
                    }
                    if let Some(m) = a.mastery {
                        ui.label(RichText::new(format!("{m:?}")).small().color(theme::GOLD));
                    }
                });
            }
        });
    egui::CollapsingHeader::new(i18n.t("sheet.spells")).show(ui, |ui| {
        ui.label(RichText::new(i18n.t("sheet.spells_pending")).italics());
    });
    egui::CollapsingHeader::new(i18n.t("sheet.features")).show(ui, |ui| {
        let bg = sheet.background_def();
        ui.label(format!("{}: {:?}", i18n.t("creator.feat"), bg.feat));
        ui.label(format!(
            "{}: {}",
            i18n.t("sheet.masteries"),
            sheet.weapon_masteries.join(", ")
        ));
        ui.label(format!(
            "{}: {:?} / {:?}",
            i18n.t("sheet.saving_throws"),
            sheet.class_def().saving_throws[0],
            sheet.class_def().saving_throws[1]
        ));
    });
    egui::CollapsingHeader::new(i18n.t("sheet.inventory")).show(ui, |ui| {
        if let Some(a) = &sheet.body_armor {
            ui.label(format!(
                "🛡 {}",
                dnd_rules::equipment::armor(a).map(|x| x.name).unwrap_or(a)
            ));
        }
        if sheet.shield {
            ui.label("🛡 Shield");
        }
        for w in &sheet.weapons {
            ui.label(format!(
                "⚔ {}",
                dnd_rules::equipment::weapon(w).map(|x| x.name).unwrap_or(w)
            ));
        }
    });
    if !sheet.biography.is_empty() {
        egui::CollapsingHeader::new(i18n.t("sheet.biography")).show(ui, |ui| {
            ui.label(&sheet.biography);
        });
    }
    let _ = RollMode::Normal;
}
