//! Player-facing panels: the combat action bar, spellbook and spell targeting, class features,
//! hit reactions, level up, inventory, loot and NPC dialogue.
//!
//! Every button sends a `Command`; the host validates it and the result comes back in the view.
//! The client only pre-filters what makes sense to show (e.g. features your class has).

use crate::i18n::I18n;
use crate::state::{CastIntent, TargetMode, Tool};
use crate::theme;
use crate::ui_game::GameUi;
use bevy_egui::egui::{self, RichText, Ui};
use dnd_rules::character::{CharacterSheet, FightingStyle};
use dnd_rules::classes;
use dnd_rules::content::ClassId;
use dnd_rules::items::ItemKind;
use dnd_rules::spells::{self, AutoLevel, Spell, SpellRange};
use dnd_rules::{Ability, Skill};
use dnd_session::command::{Command, Feature, LevelUpChoices};
use dnd_session::model::{HitReaction, Resources};
use dnd_session::view::{ClientView, CreatureView};
use dnd_world::EntityId;
use dnd_world::path::grid_distance_ft;

/// The character the local player is acting with (first character, or the DM's selection).
pub fn acting_character(g: &GameUi, view: &ClientView) -> Option<(EntityId, CharacterSheet)> {
    if let Some(sel) = g.ui.selected
        && let Some((id, s)) = view.my_characters.iter().find(|(id, _)| *id == sel)
    {
        return Some((*id, s.clone()));
    }
    view.my_characters.first().cloned()
}

fn creature<'a>(view: &'a ClientView, id: EntityId) -> Option<&'a CreatureView> {
    view.creatures.iter().find(|c| c.id == id)
}

// ------------------------------------------------------------------ spell targeting

/// Builds the targeting intent for a spell. `None` = cast immediately (self/no target).
pub fn cast_intent(
    spell: &Spell,
    caster: EntityId,
    slot: Option<u8>,
    caster_level: u8,
) -> Option<CastIntent> {
    let a = &spell.auto;
    let slot_level = slot.unwrap_or(spell.level).max(spell.level);
    let self_only = spell.range() == SpellRange::SelfOnly;
    let max = spell
        .max_targets(slot_level)
        .max(spell.projectiles(slot_level, caster_level))
        .max(1) as usize;
    let mut intent = CastIntent {
        caster,
        spell: spell.id.clone(),
        label: spell.name.clone(),
        slot,
        targets: vec![],
        max_targets: max,
        repeat: spell.projectiles(slot_level, caster_level) > 1,
        mode: TargetMode::Creatures,
        area: a.area,
        self_origin: self_only,
        range_ft: spell.range_ft(),
        monster_action: None,
    };
    if a.smite {
        return None;
    }
    if let Some(area) = a.area {
        if self_only && area.shape == spells::AreaShape::Emanation {
            return None;
        }
        intent.mode = TargetMode::Point;
        return Some(intent);
    }
    if a.light.is_some() {
        if self_only {
            return None;
        }
        intent.mode = TargetMode::PointOrCreature;
        intent.max_targets = 1;
        return Some(intent);
    }
    if self_only {
        return None;
    }
    Some(intent)
}

fn send_cast(g: &mut GameUi, intent: &CastIntent, point: Option<(i32, i32)>) {
    match intent.monster_action {
        Some(action) => {
            g.net.cmd(Command::MonsterAction {
                monster: intent.caster,
                action,
                targets: intent.targets.clone(),
                point,
            });
        }
        None => {
            g.net.cmd(Command::CastSpell {
                caster: intent.caster,
                spell: intent.spell.clone(),
                slot: intent.slot,
                targets: intent.targets.clone(),
                point,
            });
        }
    }
}

/// Called by the map click handler while a spell is pending. Returns true if the click was consumed.
pub fn targeting_click(
    g_ui: &mut crate::state::UiState,
    net: &mut crate::state::Net,
    cell: (i32, i32),
    clicked: Option<EntityId>,
) -> bool {
    if let Some((who, feature)) = g_ui.feature_target {
        if let Some(t) = clicked {
            let amount = (feature == Feature::LayOnHands).then_some(g_ui.loh_amount);
            net.cmd(Command::UseFeature {
                entity: who,
                feature,
                target: Some(t),
                amount,
            });
            g_ui.feature_target = None;
        }
        return true;
    }
    let Some(mut intent) = g_ui.casting.clone() else {
        return false;
    };
    let send = |net: &mut crate::state::Net, intent: &CastIntent, point| match intent.monster_action
    {
        Some(action) => {
            net.cmd(Command::MonsterAction {
                monster: intent.caster,
                action,
                targets: intent.targets.clone(),
                point,
            });
        }
        None => {
            net.cmd(Command::CastSpell {
                caster: intent.caster,
                spell: intent.spell.clone(),
                slot: intent.slot,
                targets: intent.targets.clone(),
                point,
            });
        }
    };
    match intent.mode {
        TargetMode::Point => {
            send(net, &intent, Some(cell));
            g_ui.casting = None;
        }
        TargetMode::PointOrCreature => {
            if let Some(t) = clicked {
                intent.targets = vec![t];
                send(net, &intent, None);
            } else {
                send(net, &intent, Some(cell));
            }
            g_ui.casting = None;
        }
        TargetMode::Creatures => {
            if let Some(t) = clicked {
                if !intent.repeat && intent.targets.contains(&t) {
                    intent.targets.retain(|x| *x != t);
                } else {
                    intent.targets.push(t);
                }
                if intent.targets.len() >= intent.max_targets {
                    send(net, &intent, None);
                    g_ui.casting = None;
                } else {
                    g_ui.casting = Some(intent);
                }
            }
        }
    }
    true
}

/// Floating panel while choosing targets.
pub fn targeting_window(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    if let Some((_, f)) = g.ui.feature_target {
        egui::Window::new(g.i18n.t("cast.choose"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 70.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("{f:?}: {}", g.i18n.t("cast.click_creature")));
                if f == Feature::LayOnHands {
                    ui.add(
                        egui::Slider::new(&mut g.ui.loh_amount, 1..=100)
                            .text(g.i18n.t("feature.hp_to_restore")),
                    );
                }
                if ui.button(g.i18n.t("cast.cancel")).clicked() {
                    g.ui.feature_target = None;
                }
            });
        return;
    }
    let Some(intent) = g.ui.casting.clone() else {
        return;
    };
    egui::Window::new(g.i18n.t("cast.choose"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 70.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(RichText::new(&intent.label).strong().color(theme::GOLD));
            let hint = match intent.mode {
                TargetMode::Point if intent.self_origin => g.i18n.t("cast.click_direction"),
                TargetMode::Point => g.i18n.t("cast.click_point"),
                TargetMode::PointOrCreature => g.i18n.t("cast.click_point_or_creature"),
                TargetMode::Creatures => g.i18n.t("cast.click_creature"),
            };
            ui.label(hint);
            if intent.mode == TargetMode::Creatures {
                let names: Vec<String> = intent
                    .targets
                    .iter()
                    .filter_map(|t| creature(view, *t).map(|c| c.name.clone()))
                    .collect();
                ui.label(format!(
                    "{} {}/{}: {}",
                    g.i18n.t("cast.targets"),
                    intent.targets.len(),
                    intent.max_targets,
                    names.join(", ")
                ));
            }
            ui.horizontal(|ui| {
                if intent.mode == TargetMode::Creatures
                    && ui
                        .button(RichText::new(g.i18n.t("cast.cast_now")).strong())
                        .clicked()
                {
                    send_cast(g, &intent, None);
                    g.ui.casting = None;
                }
                if ui.button(g.i18n.t("cast.cancel")).clicked() {
                    g.ui.casting = None;
                }
            });
        });
}

// ------------------------------------------------------------------ resources

fn slot_line(sheet: &CharacterSheet, res: &Resources) -> Vec<(u8, u8, u8)> {
    // (spell level, remaining, total)
    if sheet.class == ClassId::Warlock {
        let (n, lvl) = classes::pact_slots(sheet.level);
        if n == 0 {
            return vec![];
        }
        return vec![(lvl, n.saturating_sub(res.pact_used), n)];
    }
    sheet
        .spell_slots()
        .iter()
        .enumerate()
        .filter(|(_, n)| **n > 0)
        .map(|(i, n)| (i as u8 + 1, n.saturating_sub(res.slots_used[i]), *n))
        .collect()
}

/// Feature uses left, for the resource strip.
fn resource_labels(sheet: &CharacterSheet, c: &CreatureView, i18n: &I18n) -> Vec<String> {
    let Some(res) = &c.resources else {
        return vec![];
    };
    let lvl = sheet.level;
    let mut v = vec![];
    if sheet.has_class_feature("Rage") {
        let (uses, dmg) = classes::rage(lvl);
        v.push(format!(
            "{} {}/{} (+{dmg})",
            i18n.t("feature.rage"),
            uses.saturating_sub(res.rage_used),
            uses
        ));
    }
    if sheet.has_class_feature("Second Wind") {
        let n = classes::second_wind_uses(lvl);
        v.push(format!(
            "{} {}/{}",
            i18n.t("feature.second_wind"),
            n.saturating_sub(res.second_wind_used),
            n
        ));
    }
    if sheet.has_class_feature("Action Surge") {
        let n = classes::action_surge_uses(lvl);
        v.push(format!(
            "{} {}/{}",
            i18n.t("feature.action_surge"),
            n.saturating_sub(res.action_surge_used),
            n
        ));
    }
    if sheet.has_class_feature("Monk’s Focus") {
        let n = classes::focus_points(lvl);
        v.push(format!(
            "{} {}/{}",
            i18n.t("feature.focus"),
            n.saturating_sub(res.focus_used),
            n
        ));
    }
    if sheet.has_class_feature("Lay On Hands") {
        v.push(format!(
            "{} {}",
            i18n.t("feature.lay_on_hands_pool"),
            5 * lvl as i32 - res.lay_on_hands_used
        ));
    }
    if sheet.has_class_feature("Bardic Inspiration") {
        let n = sheet.modifier(Ability::Charisma).max(1) as u8;
        v.push(format!(
            "{} {}/{} (d{})",
            i18n.t("feature.bardic"),
            n.saturating_sub(res.bardic_used),
            n,
            classes::bardic_inspiration_die(lvl)
        ));
    }
    if sheet.has_class_feature("Channel Divinity") {
        let n = classes::channel_divinity_uses(sheet.class, lvl);
        v.push(format!(
            "{} {}/{}",
            i18n.t("feature.channel_divinity"),
            n.saturating_sub(res.channel_divinity_used),
            n
        ));
    }
    let hd = lvl.saturating_sub(res.hit_dice_used);
    v.push(format!(
        "{} {hd}/{lvl} d{}",
        i18n.t("feature.hit_dice"),
        sheet.class_def().hit_die
    ));
    v
}

/// Compact sheet add-on: resources, slots, effects and buttons for the extra windows.
pub fn sheet_extras(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    let Some((id, sheet)) = acting_character(g, view) else {
        return;
    };
    let Some(me) = creature(view, id).cloned() else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        if let Some(ac) = me.effective_ac {
            ui.label(RichText::new(format!("{} {ac}", g.i18n.t("sheet.ac"))).strong());
        }
        if me.raging {
            ui.label(
                RichText::new(g.i18n.t("feature.raging"))
                    .color(theme::BLOOD)
                    .strong(),
            );
        }
        if let Some(conc) = &me.concentration {
            ui.label(RichText::new(format!("◎ {conc}")).color(theme::GOLD));
            if ui
                .small_button(g.i18n.t("spell.end_concentration"))
                .clicked()
            {
                g.net.cmd(Command::EndConcentration { entity: id });
            }
        }
        if let Some(st) = &me.status
            && let Some(d) = st.inspiration
        {
            let label = format!(
                "{} d{d}{}",
                g.i18n.t("feature.inspired"),
                if st.inspiration_armed { " ✔" } else { "" }
            );
            if ui
                .small_button(label)
                .on_hover_text(g.i18n.t("feature.inspiration_help"))
                .clicked()
            {
                g.net.cmd(Command::UseFeature {
                    entity: id,
                    feature: Feature::UseInspiration,
                    target: None,
                    amount: None,
                });
            }
        }
    });
    if let Some(res) = &me.resources {
        let slots = slot_line(&sheet, res);
        if !slots.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(g.i18n.t("spell.slots"));
                for (lvl, left, total) in slots {
                    ui.label(
                        RichText::new(format!(
                            "{lvl}: {}{}",
                            "●".repeat(left as usize),
                            "○".repeat((total - left) as usize)
                        ))
                        .color(theme::GOLD),
                    );
                }
            });
        }
        let labels = resource_labels(&sheet, &me, &g.i18n);
        ui.label(RichText::new(labels.join(" · ")).small());
    }
    let mine: Vec<_> = view
        .effects
        .iter()
        .filter(|e| e.targets.contains(&id) || e.source == id)
        .collect();
    if !mine.is_empty() {
        ui.label(
            RichText::new(
                mine.iter()
                    .map(|e| e.label.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .small()
            .italics(),
        );
    }
    ui.horizontal_wrapped(|ui| {
        if classes::spellcasting(sheet.class).is_some()
            && ui
                .button(format!("📖 {}", g.i18n.t("spell.book")))
                .clicked()
        {
            g.ui.show_spellbook = !g.ui.show_spellbook;
        }
        if ui.button(format!("🎒 {}", g.i18n.t("inv.title"))).clicked() {
            g.ui.show_inventory = !g.ui.show_inventory;
        }
        let can_level = sheet.level < 20
            && (classes::level_for_xp(sheet.experience_points) > sheet.level
                || me.status.as_ref().is_some_and(|s| s.milestone_levels > 0));
        if can_level
            && ui
                .button(
                    RichText::new(format!("⬆ {}", g.i18n.t("level.up")))
                        .color(theme::GOLD)
                        .strong(),
                )
                .clicked()
        {
            g.ui.show_levelup = true;
            g.ui.levelup = LevelUpChoices::default();
        }
        if view.combat.is_none()
            && ui
                .button(g.i18n.t("rest.spend_hit_die"))
                .on_hover_text(g.i18n.t("rest.spend_hit_die_help"))
                .clicked()
        {
            g.net.cmd(Command::SpendHitDie { entity: id });
        }
    });
    if sheet.level < 20 {
        let next = classes::XP_THRESHOLDS[sheet.level as usize];
        ui.label(RichText::new(format!("XP {} / {next}", sheet.experience_points)).small());
    }
}

// ------------------------------------------------------------------ class features

struct FeatureButton {
    feature: Feature,
    key: &'static str,
    needs_target: bool,
}

fn feature_buttons(sheet: &CharacterSheet, me: &CreatureView) -> Vec<FeatureButton> {
    let mut v = vec![];
    let mut add = |name: &str, feature, key, needs_target| {
        if sheet.has_class_feature(name) {
            v.push(FeatureButton {
                feature,
                key,
                needs_target,
            });
        }
    };
    if me.raging {
        add("Rage", Feature::EndRage, "feature.end_rage", false);
    } else {
        add("Rage", Feature::Rage, "feature.rage", false);
    }
    add(
        "Reckless Attack",
        Feature::RecklessAttack,
        "feature.reckless",
        false,
    );
    add(
        "Second Wind",
        Feature::SecondWind,
        "feature.second_wind",
        false,
    );
    add(
        "Action Surge",
        Feature::ActionSurge,
        "feature.action_surge",
        false,
    );
    add(
        "Cunning Action",
        Feature::CunningDash,
        "feature.cunning_dash",
        false,
    );
    add(
        "Cunning Action",
        Feature::CunningDisengage,
        "feature.cunning_disengage",
        false,
    );
    add(
        "Steady Aim",
        Feature::SteadyAim,
        "feature.steady_aim",
        false,
    );
    add(
        "Martial Arts",
        Feature::MartialArtsStrike,
        "feature.martial_arts",
        false,
    );
    add(
        "Monk’s Focus",
        Feature::FlurryOfBlows,
        "feature.flurry",
        false,
    );
    add(
        "Monk’s Focus",
        Feature::PatientDefense { focus: true },
        "feature.patient_defense",
        false,
    );
    add(
        "Monk’s Focus",
        Feature::StepOfTheWind { focus: true },
        "feature.step_wind",
        false,
    );
    add(
        "Stunning Strike",
        Feature::StunningStrike,
        "feature.stunning_strike",
        false,
    );
    add(
        "Lay On Hands",
        Feature::LayOnHands,
        "feature.lay_on_hands",
        true,
    );
    add(
        "Bardic Inspiration",
        Feature::BardicInspiration,
        "feature.bardic",
        true,
    );
    add(
        "Channel Divinity",
        Feature::DivineSparkHeal,
        "feature.spark_heal",
        true,
    );
    add(
        "Channel Divinity",
        Feature::DivineSparkDamage,
        "feature.spark_damage",
        true,
    );
    add(
        "Innate Sorcery",
        Feature::InnateSorcery,
        "feature.innate_sorcery",
        false,
    );
    v
}

fn features_row(ui: &mut Ui, g: &mut GameUi, sheet: &CharacterSheet, me: &CreatureView) {
    let buttons = feature_buttons(sheet, me);
    if buttons.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for b in buttons {
            if ui.button(g.i18n.t(b.key)).clicked() {
                if b.needs_target {
                    g.ui.feature_target = Some((me.id, b.feature));
                    g.ui.casting = None;
                    g.ui.targeting = None;
                } else {
                    g.net.cmd(Command::UseFeature {
                        entity: me.id,
                        feature: b.feature,
                        target: None,
                        amount: None,
                    });
                }
            }
        }
    });
}

// ------------------------------------------------------------------ combat action bar

pub fn action_bar(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    let Some(c) = &view.combat else { return };
    let (Some(turn), Some(active)) = (&c.turn, c.active) else {
        return;
    };
    let Some(me) = creature(view, active).cloned() else {
        return;
    };
    let sheet = view
        .my_characters
        .iter()
        .find(|(id, _)| *id == active)
        .map(|(_, s)| s.clone());
    egui::Window::new(g.i18n.t("combat.your_turn"))
        .id("action-bar".into())
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -190.0))
        .collapsible(true)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(RichText::new(&me.name).strong());
            let tick = |used: bool| if used { "✖" } else { "✔" };
            ui.label(format!(
                "{}: {} ft · {}: {} · {}: {} · {}: {}",
                g.i18n.t("combat.movement"),
                turn.movement_left_ft,
                g.i18n.t("combat.action"),
                tick(turn.action_used && turn.extra_actions == 0),
                g.i18n.t("combat.bonus_action"),
                tick(turn.bonus_action_used),
                g.i18n.t("combat.attacks_left"),
                turn.attacks_left + turn.bonus_attacks
            ));
            let can_attack = !turn.action_used
                || turn.attacks_left > 0
                || turn.extra_actions > 0
                || turn.bonus_attacks > 0;
            ui.horizontal_wrapped(|ui| {
                for (i, a) in me.attacks.iter().enumerate() {
                    if ui
                        .add_enabled(can_attack, egui::Button::new(format!("⚔ {}", a.name)))
                        .on_hover_text(a.summary())
                        .clicked()
                    {
                        g.ui.targeting = Some(i);
                        g.ui.casting = None;
                        g.ui.selected = Some(me.id);
                        g.ui.tool = Tool::Play;
                    }
                }
                if g.net.is_dm() {
                    for (i, sa) in me.save_actions.iter().enumerate() {
                        let charging = me
                            .resources
                            .as_ref()
                            .is_some_and(|r| r.recharging.contains(&i));
                        let label = format!("☄ {}{}", sa.name, if charging { " ⏳" } else { "" });
                        if ui
                            .add_enabled(!charging && !turn.action_used, egui::Button::new(label))
                            .on_hover_text(&sa.text)
                            .clicked()
                        {
                            g.ui.casting = Some(monster_intent(&me, i, sa));
                        }
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                let free = !turn.action_used || turn.extra_actions > 0;
                if ui
                    .add_enabled(free, egui::Button::new(g.i18n.t("action.dash")))
                    .clicked()
                {
                    g.net.cmd(Command::Dash { entity: me.id });
                }
                if ui
                    .add_enabled(free, egui::Button::new(g.i18n.t("action.dodge")))
                    .clicked()
                {
                    g.net.cmd(Command::Dodge { entity: me.id });
                }
                if ui
                    .add_enabled(free, egui::Button::new(g.i18n.t("action.disengage")))
                    .clicked()
                {
                    g.net.cmd(Command::Disengage { entity: me.id });
                }
                if sheet
                    .as_ref()
                    .is_some_and(|s| classes::spellcasting(s.class).is_some())
                    && ui
                        .button(format!("📖 {}", g.i18n.t("spell.book")))
                        .clicked()
                {
                    g.ui.selected = Some(me.id);
                    g.ui.show_spellbook = true;
                }
                if ui
                    .button(RichText::new(g.i18n.t("action.end_turn")).strong())
                    .clicked()
                {
                    g.net.cmd(Command::EndTurn);
                    g.ui.targeting = None;
                    g.ui.casting = None;
                }
            });
            if let Some(s) = &sheet {
                features_row(ui, g, s, &me);
            }
            if g.ui.targeting.is_some() {
                ui.label(RichText::new(g.i18n.t("combat.choose_target")).color(theme::BLOOD));
            }
        });
}

fn monster_intent(me: &CreatureView, i: usize, sa: &dnd_rules::monster::SaveAction) -> CastIntent {
    CastIntent {
        caster: me.id,
        spell: String::new(),
        label: sa.name.clone(),
        slot: None,
        targets: vec![],
        max_targets: 8,
        repeat: false,
        mode: if sa.area.is_some() {
            TargetMode::Point
        } else {
            TargetMode::Creatures
        },
        area: sa.area,
        self_origin: sa.area.is_some_and(|a| {
            matches!(
                a.shape,
                spells::AreaShape::Cone | spells::AreaShape::Line | spells::AreaShape::Emanation
            )
        }),
        range_ft: u32::MAX,
        monster_action: Some(i),
    }
}

/// Out-of-turn prompts: opportunity attacks and on-hit reactions (Shield, Uncanny Dodge, Deflect Attacks).
pub fn reaction_prompts(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    let Some(c) = &view.combat else { return };
    if let Some(p) = &c.your_hit_reaction {
        let attacker = creature(view, p.attacker)
            .map(|a| a.name.clone())
            .unwrap_or_default();
        let target = creature(view, p.target)
            .map(|a| a.name.clone())
            .unwrap_or_default();
        egui::Window::new(g.i18n.t("reaction.hit_title"))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -40.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!(
                    "{attacker} → {target}: {} {}",
                    g.i18n.t("reaction.hit_roll"),
                    p.result.d20.total
                ));
                ui.horizontal(|ui| {
                    for o in &p.options {
                        let (key, hint) = match o {
                            HitReaction::Shield => ("reaction.shield", "reaction.shield_help"),
                            HitReaction::UncannyDodge => {
                                ("reaction.uncanny_dodge", "reaction.uncanny_dodge_help")
                            }
                            HitReaction::DeflectAttacks => {
                                ("reaction.deflect", "reaction.deflect_help")
                            }
                        };
                        if ui
                            .button(RichText::new(g.i18n.t(key)).strong())
                            .on_hover_text(g.i18n.t(hint))
                            .clicked()
                        {
                            g.net.cmd(Command::HitReaction { choice: Some(*o) });
                        }
                    }
                    if ui.button(g.i18n.t("reaction.decline")).clicked() {
                        g.net.cmd(Command::HitReaction { choice: None });
                    }
                });
            });
    }
}

// ------------------------------------------------------------------ spellbook

fn spell_badge(s: &Spell, i18n: &I18n) -> RichText {
    match s.auto.level {
        AutoLevel::Full => RichText::new(i18n.t("spell.auto_full"))
            .small()
            .color(theme::HEALTH),
        AutoLevel::Partial => RichText::new(i18n.t("spell.auto_partial"))
            .small()
            .color(theme::GOLD),
        AutoLevel::Manual => RichText::new(i18n.t("spell.auto_manual"))
            .small()
            .color(theme::WOOD_LIGHT),
    }
}

fn spell_tooltip(ui: &mut Ui, s: &Spell) {
    ui.set_max_width(420.0);
    ui.label(RichText::new(&s.name).strong());
    ui.label(format!(
        "{} · {} · {} · {} · {}{}",
        if s.level == 0 {
            "Cantrip".to_string()
        } else {
            format!("Level {}", s.level)
        },
        s.school,
        s.casting_time,
        s.range,
        s.duration,
        if s.concentration { " (C)" } else { "" }
    ));
    ui.label(RichText::new(&s.components).small());
    ui.label(&s.description);
}

pub fn spellbook_window(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    if !g.ui.show_spellbook {
        return;
    }
    let Some((id, sheet)) = acting_character(g, view) else {
        return;
    };
    let Some(me) = creature(view, id).cloned() else {
        return;
    };
    let mut open = true;
    egui::Window::new(format!("📖 {} — {}", g.i18n.t("spell.book"), sheet.name))
        .open(&mut open)
        .default_width(520.0)
        .default_height(520.0)
        .show(ctx, |ui| {
            let Some((ability, _)) = classes::spellcasting(sheet.class) else {
                ui.label(g.i18n.t("spell.not_caster"));
                return;
            };
            ui.label(format!(
                "{}: {:?} · {} {} · {} {:+}",
                g.i18n.t("spell.ability"),
                ability,
                g.i18n.t("spell.save_dc"),
                sheet.spell_save_dc().unwrap_or(10),
                g.i18n.t("spell.attack"),
                sheet.spell_attack_bonus().unwrap_or(0)
            ));
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut g.ui.spellbook_prepare,
                    false,
                    g.i18n.t("spell.cast_tab"),
                );
                ui.selectable_value(
                    &mut g.ui.spellbook_prepare,
                    true,
                    g.i18n.t("spell.prepare_tab"),
                );
            });
            ui.separator();
            if g.ui.spellbook_prepare {
                prepare_tab(ui, g, id, &sheet, view);
            } else {
                cast_tab(ui, g, id, &sheet, &me, view);
            }
        });
    if !open {
        g.ui.show_spellbook = false;
    }
}

fn cast_tab(
    ui: &mut Ui,
    g: &mut GameUi,
    id: EntityId,
    sheet: &CharacterSheet,
    me: &CreatureView,
    view: &ClientView,
) {
    let res = me.resources.clone().unwrap_or_default();
    let slots = slot_line(sheet, &res);
    let in_combat = view.combat.is_some();
    let mut known: Vec<&Spell> = sheet
        .cantrips
        .iter()
        .chain(sheet.all_prepared().iter())
        .filter_map(|s| spells::spell(s))
        .collect();
    known.sort_by_key(|s| (s.level, s.name.clone()));
    known.dedup_by_key(|s| s.id.clone());
    if known.is_empty() {
        ui.label(g.i18n.t("spell.none_prepared"));
        return;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut last_level = 255;
        for s in known {
            if s.level != last_level {
                last_level = s.level;
                theme::heading(
                    ui,
                    &if s.level == 0 {
                        g.i18n.t("spell.cantrips").to_string()
                    } else {
                        format!("{} {}", g.i18n.t("spell.level"), s.level)
                    },
                );
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(&s.name).strong())
                    .on_hover_ui(|ui| spell_tooltip(ui, s));
                ui.label(spell_badge(s, &g.i18n));
                if s.concentration {
                    ui.label(RichText::new("◎").color(theme::GOLD))
                        .on_hover_text(g.i18n.t("spell.concentration"));
                }
                let long = s.casting_kind() == spells::CastingKind::Long;
                let reaction = s.casting_kind() == spells::CastingKind::Reaction;
                // Slot choice: every slot level ≥ the spell's level with uses left.
                let options: Vec<u8> = slots
                    .iter()
                    .filter(|(l, left, _)| *l >= s.level && *left > 0)
                    .map(|(l, _, _)| *l)
                    .collect();
                let castable = s.level == 0 || !options.is_empty();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if reaction {
                        ui.label(RichText::new(g.i18n.t("spell.reaction_auto")).small());
                        return;
                    }
                    if long && in_combat {
                        ui.label(RichText::new(g.i18n.t("spell.not_in_combat")).small());
                        return;
                    }
                    let mut chosen: Option<u8> = None;
                    if s.level == 0 {
                        if ui
                            .add_enabled(castable, egui::Button::new(g.i18n.t("spell.cast")))
                            .clicked()
                        {
                            chosen = Some(0);
                        }
                    } else {
                        for l in options.iter().rev() {
                            let label = if *l == s.level {
                                format!("{} ({l})", g.i18n.t("spell.cast"))
                            } else {
                                format!("↑{l}")
                            };
                            if ui
                                .button(label)
                                .on_hover_text(g.i18n.t("spell.upcast_help"))
                                .clicked()
                            {
                                chosen = Some(*l);
                            }
                        }
                        if options.is_empty() {
                            ui.label(
                                RichText::new(g.i18n.t("spell.no_slots"))
                                    .small()
                                    .color(theme::BLOOD),
                            );
                        }
                    }
                    if let Some(l) = chosen {
                        let slot = (l > 0).then_some(l);
                        g.ui.targeting = None;
                        g.ui.feature_target = None;
                        match cast_intent(s, id, slot, sheet.level) {
                            Some(intent) => {
                                g.ui.casting = Some(intent);
                                g.ui.tool = Tool::Play;
                            }
                            None => {
                                g.net.cmd(Command::CastSpell {
                                    caster: id,
                                    spell: s.id.clone(),
                                    slot,
                                    targets: vec![],
                                    point: None,
                                });
                            }
                        }
                    }
                });
            });
        }
    });
}

fn prepare_tab(
    ui: &mut Ui,
    g: &mut GameUi,
    id: EntityId,
    sheet: &CharacterSheet,
    view: &ClientView,
) {
    let class_name = sheet.class_def().name;
    let max_c = classes::cantrips_known(sheet.class, sheet.level) as usize;
    let max_p = classes::prepared_spells(sheet.class, sheet.level) as usize;
    let max_level = classes::max_spell_level(sheet.class, sheet.level);
    let draft =
        g.ui.prep_draft
            .get_or_insert_with(|| (sheet.cantrips.clone(), sheet.prepared_spells.clone()))
            .clone();
    let (mut cantrips, mut prepared) = draft;
    let always = classes::subclass_always_prepared(sheet.class, sheet.level);
    ui.label(format!(
        "{} {}/{max_c} · {} {}/{max_p}",
        g.i18n.t("spell.cantrips"),
        cantrips.len(),
        g.i18n.t("spell.prepared"),
        prepared.len()
    ));
    ui.add(egui::TextEdit::singleline(&mut g.ui.spell_search).hint_text(g.i18n.t("editor.search")));
    let q = g.ui.spell_search.to_lowercase();
    egui::ScrollArea::vertical()
        .max_height(360.0)
        .show(ui, |ui| {
            let mut list: Vec<&Spell> = spells::class_list(class_name)
                .filter(|s| s.level <= max_level)
                .filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q))
                .collect();
            list.sort_by_key(|s| (s.level, s.name.clone()));
            let mut last = 255;
            for s in list {
                if s.level != last {
                    last = s.level;
                    ui.label(
                        RichText::new(if s.level == 0 {
                            g.i18n.t("spell.cantrips").to_string()
                        } else {
                            format!("{} {}", g.i18n.t("spell.level"), s.level)
                        })
                        .strong()
                        .color(theme::WOOD),
                    );
                }
                ui.horizontal(|ui| {
                    let is_always = always.contains(&s.id.as_str());
                    let list = if s.level == 0 {
                        &mut cantrips
                    } else {
                        &mut prepared
                    };
                    let mut on = list.contains(&s.id) || is_always;
                    let full = if s.level == 0 {
                        list.len() >= max_c
                    } else {
                        list.len() >= max_p
                    };
                    let enabled = !is_always && (on || !full);
                    if ui
                        .add_enabled(enabled, egui::Checkbox::new(&mut on, &s.name))
                        .on_hover_ui(|ui| spell_tooltip(ui, s))
                        .changed()
                    {
                        if on {
                            list.push(s.id.clone());
                        } else {
                            list.retain(|x| *x != s.id);
                        }
                    }
                    if is_always {
                        ui.label(RichText::new(g.i18n.t("spell.always_prepared")).small());
                    }
                    ui.label(spell_badge(s, &g.i18n));
                });
            }
        });
    g.ui.prep_draft = Some((cantrips.clone(), prepared.clone()));
    ui.horizontal(|ui| {
        if ui
            .button(RichText::new(g.i18n.t("spell.save_prepared")).strong())
            .clicked()
        {
            g.net.cmd(Command::PrepareSpells {
                entity: id,
                cantrips,
                prepared,
            });
            g.ui.prep_draft = None;
        }
        if ui.button(g.i18n.t("spell.reset")).clicked() {
            g.ui.prep_draft = None;
        }
    });
    if view.combat.is_some() {
        ui.label(
            RichText::new(g.i18n.t("spell.prepare_outside_combat"))
                .small()
                .color(theme::BLOOD),
        );
    }
    ui.label(RichText::new(g.i18n.t("spell.prepare_help")).small());
}

// ------------------------------------------------------------------ level up

pub fn levelup_window(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    if !g.ui.show_levelup {
        return;
    }
    let Some((id, sheet)) = acting_character(g, view) else {
        return;
    };
    let new_level = sheet.level + 1;
    let mut open = true;
    egui::Window::new(format!("⬆ {} {} → {}", sheet.name, sheet.level, new_level))
        .open(&mut open)
        .default_width(460.0)
        .show(ctx, |ui| {
            let subclass = sheet.subclass.clone().or_else(|| {
                (new_level >= 3).then(|| classes::srd_subclass(sheet.class).to_string())
            });
            let feats: Vec<_> = classes::features(sheet.class, new_level, subclass.as_deref())
                .into_iter()
                .filter(|f| f.level == new_level)
                .collect();
            theme::heading(ui, g.i18n.t("level.new_features"));
            if feats.is_empty() {
                ui.label("—");
            }
            for f in &feats {
                ui.label(RichText::new(&f.name).strong())
                    .on_hover_text(&f.text);
            }
            if new_level == 3 && sheet.subclass.is_none() {
                ui.label(format!(
                    "{}: {}",
                    g.i18n.t("level.subclass"),
                    classes::srd_subclass(sheet.class)
                ));
            }
            let hp_gain =
                (sheet.class_def().hit_die as i32 / 2 + 1 + sheet.modifier(Ability::Constitution))
                    .max(1);
            ui.label(format!("{}: +{hp_gain}", g.i18n.t("level.hp_gain")));
            let ch = &mut g.ui.levelup;
            // Ability Score Improvement.
            if classes::is_asi_level(sheet.class, new_level) {
                theme::heading(ui, g.i18n.t("level.asi"));
                let scores = sheet.scores();
                ui.horizontal_wrapped(|ui| {
                    for a in Ability::ALL {
                        let cur: u8 = ch
                            .asi
                            .iter()
                            .filter(|(x, _)| *x == a)
                            .map(|(_, n)| *n)
                            .sum();
                        let total: u8 = ch.asi.iter().map(|(_, n)| *n).sum();
                        let score = scores.get(a);
                        ui.vertical(|ui| {
                            ui.label(format!(
                                "{:?} {}{}",
                                a,
                                score,
                                if cur > 0 {
                                    format!(" +{cur}")
                                } else {
                                    String::new()
                                }
                            ));
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        total < 2 && score + cur < 20,
                                        egui::Button::new("+").small(),
                                    )
                                    .clicked()
                                {
                                    ch.asi.push((a, 1));
                                }
                                if ui
                                    .add_enabled(cur > 0, egui::Button::new("−").small())
                                    .clicked()
                                    && let Some(p) = ch.asi.iter().position(|(x, _)| *x == a)
                                {
                                    ch.asi.remove(p);
                                }
                            });
                        });
                    }
                });
                // Merge +1/+1 on the same ability into +2.
                let mut merged: Vec<(Ability, u8)> = vec![];
                for (a, n) in &ch.asi {
                    match merged.iter_mut().find(|(x, _)| x == a) {
                        Some(m) => m.1 += n,
                        None => merged.push((*a, *n)),
                    }
                }
                ch.asi = merged
                    .into_iter()
                    .flat_map(|(a, n)| std::iter::repeat_n((a, 1), n as usize))
                    .collect();
            }
            if feats.iter().any(|f| f.name.starts_with("Fighting Style")) {
                theme::heading(ui, g.i18n.t("level.fighting_style"));
                ui.horizontal_wrapped(|ui| {
                    for (fs, name) in [
                        (FightingStyle::Archery, "Archery"),
                        (FightingStyle::Defense, "Defense"),
                        (FightingStyle::GreatWeaponFighting, "Great Weapon Fighting"),
                        (FightingStyle::TwoWeaponFighting, "Two-Weapon Fighting"),
                    ] {
                        ui.selectable_value(&mut ch.fighting_style, Some(fs), name);
                    }
                });
            }
            if feats.iter().any(|f| f.name.starts_with("Expertise")) {
                theme::heading(ui, g.i18n.t("level.expertise"));
                ui.horizontal_wrapped(|ui| {
                    for s in Skill::ALL {
                        if !sheet.skill_proficient(s) || sheet.expertise.contains(&s) {
                            continue;
                        }
                        let mut on = ch.expertise.contains(&s);
                        if ui
                            .add_enabled(
                                on || ch.expertise.len() < 2,
                                egui::Checkbox::new(&mut on, crate::character_ui::skill_name(s)),
                            )
                            .changed()
                        {
                            if on {
                                ch.expertise.push(s);
                            } else {
                                ch.expertise.retain(|x| *x != s);
                            }
                        }
                    }
                });
            }
            // New spells and masteries.
            let extra_c = classes::cantrips_known(sheet.class, new_level)
                .saturating_sub(classes::cantrips_known(sheet.class, sheet.level))
                as usize;
            let extra_p = classes::prepared_spells(sheet.class, new_level)
                .saturating_sub(classes::prepared_spells(sheet.class, sheet.level))
                as usize;
            let extra_m = classes::weapon_mastery_count(sheet.class, new_level)
                .saturating_sub(classes::weapon_mastery_count(sheet.class, sheet.level))
                as usize;
            if extra_c + extra_p > 0 {
                theme::heading(
                    ui,
                    &format!(
                        "{} (+{extra_c} {}, +{extra_p} {})",
                        g.i18n.t("level.new_spells"),
                        g.i18n.t("spell.cantrips"),
                        g.i18n.t("spell.prepared")
                    ),
                );
                let max_level = classes::max_spell_level(sheet.class, new_level);
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for s in spells::class_list(sheet.class_def().name)
                            .filter(|s| s.level <= max_level && !sheet.knows_spell(&s.id))
                        {
                            let (list, cap) = if s.level == 0 {
                                (&mut ch.new_cantrips, extra_c)
                            } else {
                                (&mut ch.new_prepared, extra_p)
                            };
                            if cap == 0 {
                                continue;
                            }
                            let mut on = list.contains(&s.id);
                            if ui
                                .add_enabled(
                                    on || list.len() < cap,
                                    egui::Checkbox::new(
                                        &mut on,
                                        format!("{} ({})", s.name, s.level),
                                    ),
                                )
                                .on_hover_ui(|ui| spell_tooltip(ui, s))
                                .changed()
                            {
                                if on {
                                    list.push(s.id.clone());
                                } else {
                                    list.retain(|x| *x != s.id);
                                }
                            }
                        }
                    });
            }
            if extra_m > 0 {
                theme::heading(ui, &format!("{} (+{extra_m})", g.i18n.t("level.masteries")));
                ui.horizontal_wrapped(|ui| {
                    for w in dnd_rules::equipment::WEAPONS
                        .iter()
                        .filter(|w| !sheet.weapon_masteries.iter().any(|m| m == w.id))
                    {
                        let mut on = ch.new_masteries.iter().any(|m| m == w.id);
                        if ui
                            .add_enabled(
                                on || ch.new_masteries.len() < extra_m,
                                egui::Checkbox::new(&mut on, w.name),
                            )
                            .changed()
                        {
                            if on {
                                ch.new_masteries.push(w.id.to_string());
                            } else {
                                ch.new_masteries.retain(|x| x != w.id);
                            }
                        }
                    }
                });
            }
            ui.separator();
            if ui
                .button(RichText::new(g.i18n.t("level.confirm")).strong())
                .clicked()
            {
                let choices = Box::new(g.ui.levelup.clone());
                g.net.cmd(Command::LevelUp {
                    entity: id,
                    choices,
                });
                g.ui.show_levelup = false;
            }
            ui.label(RichText::new(g.i18n.t("level.help")).small());
        });
    if !open {
        g.ui.show_levelup = false;
    }
}

// ------------------------------------------------------------------ inventory & loot

pub fn inventory_window(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    if !g.ui.show_inventory {
        return;
    }
    let Some((id, sheet)) = acting_character(g, view) else {
        return;
    };
    let me_pos = creature(view, id).map(|c| c.pos);
    let mut open = true;
    egui::Window::new(format!("🎒 {}", sheet.name))
        .open(&mut open)
        .default_width(440.0)
        .show(ctx, |ui| {
            let cur = sheet.currency;
            ui.label(format!(
                "{} {} pp · {} gp · {} ep · {} sp · {} cp",
                g.i18n.t("inv.coins"),
                cur.pp,
                cur.gp,
                cur.ep,
                cur.sp,
                cur.cp
            ));
            theme::heading(ui, g.i18n.t("inv.equipped"));
            ui.horizontal_wrapped(|ui| {
                if let Some(a) = &sheet.body_armor {
                    if ui
                        .button(format!("🛡 {} ✖", a.replace('_', " ")))
                        .on_hover_text(g.i18n.t("inv.unequip"))
                        .clicked()
                    {
                        g.net.cmd(Command::EquipItem {
                            entity: id,
                            item: a.clone(),
                            equip: false,
                        });
                    }
                }
                if sheet.shield
                    && ui
                        .button(format!("🛡 {} ✖", g.i18n.t("inv.shield")))
                        .clicked()
                {
                    g.net.cmd(Command::EquipItem {
                        entity: id,
                        item: "shield".into(),
                        equip: false,
                    });
                }
                for w in &sheet.weapons {
                    if ui
                        .button(format!("⚔ {} ✖", w.replace('_', " ")))
                        .on_hover_text(g.i18n.t("inv.unequip"))
                        .clicked()
                    {
                        g.net.cmd(Command::EquipItem {
                            entity: id,
                            item: w.clone(),
                            equip: false,
                        });
                    }
                }
            });
            theme::heading(ui, g.i18n.t("inv.backpack"));
            if sheet.inventory.is_empty() {
                ui.label(RichText::new(g.i18n.t("inv.empty")).italics());
            }
            // Give target: party members within 5 ft.
            let near: Vec<(EntityId, String)> = view
                .creatures
                .iter()
                .filter(|c| {
                    c.id != id
                        && c.faction == dnd_session::model::Faction::Party
                        && me_pos.is_some_and(|p| grid_distance_ft(p, c.pos) <= 5)
                })
                .map(|c| (c.id, c.name.clone()))
                .collect();
            for it in &sheet.inventory {
                ui.horizontal(|ui| {
                    ui.label(format!("{}× {}", it.quantity, it.name))
                        .on_hover_text(&it.description);
                    match &it.kind {
                        ItemKind::HealingPotion(_) => {
                            if ui.button(g.i18n.t("inv.drink")).clicked() {
                                g.net.cmd(Command::UseItem {
                                    entity: id,
                                    item: it.id.clone(),
                                    target: None,
                                });
                            }
                            for (n_id, n) in &near {
                                if ui
                                    .small_button(format!("→ {n}"))
                                    .on_hover_text(g.i18n.t("inv.administer"))
                                    .clicked()
                                {
                                    g.net.cmd(Command::UseItem {
                                        entity: id,
                                        item: it.id.clone(),
                                        target: Some(*n_id),
                                    });
                                }
                            }
                        }
                        ItemKind::LightSource { .. } => {
                            if ui.button(g.i18n.t("inv.light_toggle")).clicked() {
                                g.net.cmd(Command::UseItem {
                                    entity: id,
                                    item: it.id.clone(),
                                    target: None,
                                });
                            }
                        }
                        ItemKind::Weapon(_) | ItemKind::Armor(_) => {
                            if ui.button(g.i18n.t("inv.equip")).clicked() {
                                g.net.cmd(Command::EquipItem {
                                    entity: id,
                                    item: it.id.clone(),
                                    equip: true,
                                });
                            }
                        }
                        _ => {}
                    }
                    if !near.is_empty() {
                        egui::ComboBox::from_id_salt(("give", &it.id))
                            .selected_text(g.i18n.t("inv.give"))
                            .width(70.0)
                            .show_ui(ui, |ui| {
                                for (n_id, n) in &near {
                                    if ui.button(n).clicked() {
                                        g.net.cmd(Command::GiveItem {
                                            from: id,
                                            to: *n_id,
                                            item: it.id.clone(),
                                            quantity: 1,
                                        });
                                    }
                                }
                            });
                    }
                });
            }
            ui.label(RichText::new(g.i18n.t("inv.help")).small());
        });
    if !open {
        g.ui.show_inventory = false;
    }
}

/// Shows the contents of open containers next to your character.
pub fn loot_window(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    if g.net.is_dm() {
        return;
    }
    let Some((id, _)) = acting_character(g, view) else {
        return;
    };
    let Some(me) = creature(view, id) else { return };
    let Some(room) = &view.room else { return };
    let Some(map) = &g.gv.map else { return };
    for (obj, items) in &room.containers {
        let Some(o) = map.object(*obj) else { continue };
        if !o.open || items.is_empty() || !o.footprint().any(|f| grid_distance_ft(f, me.pos) <= 5) {
            continue;
        }
        let title = if o.label.is_empty() {
            g.i18n.t("loot.container").to_string()
        } else {
            o.label.clone()
        };
        egui::Window::new(format!("💰 {title}"))
            .id(egui::Id::new(("loot", obj.0)))
            .default_pos(egui::pos2(420.0, 160.0))
            .show(ctx, |ui| {
                for it in items {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}× {}", it.quantity, it.name))
                            .on_hover_text(&it.description);
                        if ui.button(g.i18n.t("loot.take")).clicked() {
                            g.net.cmd(Command::TakeFromContainer {
                                entity: id,
                                object: *obj,
                                item: it.id.clone(),
                                quantity: 1,
                            });
                        }
                        if it.quantity > 1 && ui.button(g.i18n.t("loot.take_all")).clicked() {
                            g.net.cmd(Command::TakeFromContainer {
                                entity: id,
                                object: *obj,
                                item: it.id.clone(),
                                quantity: it.quantity,
                            });
                        }
                    });
                }
            });
    }
}

/// NPC dialogue with choices (from scripted events) and narrated messages.
pub fn dialogue_window(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    if let Some((speaker, text, choices)) = &view.dialogue {
        egui::Window::new(if speaker.is_empty() {
            g.i18n.t("dialogue.title").to_string()
        } else {
            speaker.clone()
        })
        .id("dialogue".into())
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 80.0))
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .show(ctx, |ui| {
            ui.label(RichText::new(text).size(16.0));
            ui.add_space(6.0);
            for (i, c) in choices.iter().enumerate() {
                if ui.button(format!("{}. {c}", i + 1)).clicked() {
                    g.net.cmd(Command::DialogueChoice { choice: i });
                }
            }
            if g.net.is_dm() {
                ui.label(RichText::new(g.i18n.t("dialogue.dm_note")).small());
            }
        });
        return;
    }
    if let Some((n, speaker, text)) = g.pres.message.clone()
        && n > g.ui.dismissed_message
    {
        egui::Window::new(if speaker.is_empty() {
            g.i18n.t("dialogue.narration").to_string()
        } else {
            speaker
        })
        .id("narration".into())
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 110.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_max_width(480.0);
            ui.label(RichText::new(text).italics().size(15.0));
            if ui.button(g.i18n.t("dialogue.ok")).clicked() {
                g.ui.dismissed_message = n;
            }
        });
    }
}
