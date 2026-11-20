//! Class features in play, rests, experience and leveling, spell preparation.

use crate::command::{Feature, LevelUpChoices};
use crate::effects::EffectKind;
use crate::event::{Event, FloatKind, Vfx};
use crate::model::*;
use crate::session::{CommandError, Session};
use dnd_rules::Ability;
use dnd_rules::classes;
use dnd_rules::conditions::Condition;
use dnd_rules::content::ClassId;
use dnd_rules::dice::DiceExpr;
use dnd_rules::equipment::ArmorCategory;
use dnd_world::EntityId;
use dnd_world::path::grid_distance_ft;

type Res<T> = Result<T, CommandError>;

fn rules<T>(s: impl Into<String>) -> Res<T> {
    Err(CommandError::Rules(s.into()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cost {
    Free,
    Action,
    Bonus,
}

impl Session {
    fn feature_turn_check(&self, who: Principal, entity: EntityId, cost: Cost) -> Res<bool> {
        let c = self.creature(entity)?;
        if !Self::controls(who, c) {
            return Err(CommandError::NotAuthorized);
        }
        if !c.is_alive() || !c.conditions.can_act() {
            return rules(format!("{} can't use features right now", c.name));
        }
        let in_combat = self
            .state
            .combat
            .as_ref()
            .is_some_and(|cb| cb.room == c.room);
        if in_combat {
            let cb = self.state.combat.as_ref().expect("checked");
            if cb.pending.is_some() || cb.pending_hit.is_some() {
                return rules("waiting for a reaction decision");
            }
            if cost != Cost::Free && cb.active() != entity {
                return rules("it isn't this creature's turn");
            }
            match cost {
                Cost::Bonus if cb.turn.bonus_action_used => {
                    return rules("bonus action already used this turn");
                }
                Cost::Action if cb.turn.action_used && cb.turn.extra_actions == 0 => {
                    return rules("action already used this turn");
                }
                _ => {}
            }
        }
        Ok(in_combat)
    }

    fn pay(&mut self, in_combat: bool, cost: Cost) {
        if !in_combat {
            return;
        }
        let t = &mut self.state.combat.as_mut().expect("in combat").turn;
        match cost {
            Cost::Bonus => t.bonus_action_used = true,
            Cost::Action => {
                if !t.action_used {
                    t.action_used = true;
                } else {
                    t.extra_actions -= 1;
                }
            }
            Cost::Free => {}
        }
    }

    fn sheet_of(&self, id: EntityId) -> Res<dnd_rules::character::CharacterSheet> {
        self.creature(id)?
            .sheet()
            .cloned()
            .ok_or_else(|| CommandError::Rules("only player characters have class features".into()))
    }

    pub(crate) fn cmd_use_feature(
        &mut self,
        who: Principal,
        entity: EntityId,
        feature: Feature,
        target: Option<EntityId>,
        amount: Option<i32>,
    ) -> Res<()> {
        let sheet = self.sheet_of(entity)?;
        let c = self.state.creatures[&entity].clone();
        let lvl = sheet.level;
        let need = |name: &str| -> Res<()> {
            if sheet.has_class_feature(name) {
                Ok(())
            } else {
                Err(CommandError::Rules(format!(
                    "{} doesn't have {name}",
                    c.name
                )))
            }
        };
        let name = c.name.clone();
        match feature {
            Feature::Rage => {
                need("Rage")?;
                if c.status.raging {
                    // Extending an active Rage costs a Bonus Action.
                    let ic = self.feature_turn_check(who, entity, Cost::Bonus)?;
                    self.pay(ic, Cost::Bonus);
                    self.creature_mut(entity).status.rage_extended = true;
                    self.log(Audience::All, format!("{name} keeps raging."));
                    return Ok(());
                }
                let heavy = sheet
                    .body_armor
                    .as_deref()
                    .and_then(dnd_rules::equipment::armor)
                    .is_some_and(|a| a.category == ArmorCategory::Heavy);
                if heavy {
                    return rules("you can't Rage in Heavy armor");
                }
                let (uses, bonus) = classes::rage(lvl);
                if c.resources.rage_used >= uses {
                    return rules(
                        "no Rage uses left (regain one on a Short Rest, all on a Long Rest)",
                    );
                }
                let ic = self.feature_turn_check(who, entity, Cost::Bonus)?;
                self.pay(ic, Cost::Bonus);
                self.end_concentration(entity, "entered Rage");
                let cm = self.creature_mut(entity);
                cm.resources.rage_used += 1;
                cm.status.raging = true;
                cm.status.rage_extended = true;
                cm.status.rage_rounds = 0;
                if sheet.has_class_feature("Mindless Rage") {
                    cm.conditions.remove(Condition::Charmed);
                    cm.conditions.remove(Condition::Frightened);
                }
                self.log(Audience::All, format!("{name} enters a Rage! (+{bonus} damage, Resistance to Bludgeoning/Piercing/Slashing)"));
                self.emit(
                    Audience::All,
                    Event::Floating {
                        entity,
                        text: "RAGE!".into(),
                        kind: FloatKind::Info,
                    },
                );
                self.emit(
                    Audience::All,
                    Event::SpellVisual {
                        caster: entity,
                        name: "Rage".into(),
                        vfx: Vfx::Fire,
                        from: c.pos,
                        targets: vec![c.pos],
                        area: vec![],
                    },
                );
            }
            Feature::EndRage => {
                if !c.status.raging {
                    return rules("not raging");
                }
                self.creature_mut(entity).status.raging = false;
                self.log(Audience::All, format!("{name}'s Rage ends."));
            }
            Feature::RecklessAttack => {
                need("Reckless Attack")?;
                let ic = self.feature_turn_check(who, entity, Cost::Free)?;
                if ic
                    && self
                        .state
                        .combat
                        .as_ref()
                        .is_some_and(|cb| cb.turn.action_used || cb.turn.attacks_left > 0)
                {
                    return rules(
                        "decide on Reckless Attack before your first attack roll this turn",
                    );
                }
                self.creature_mut(entity).status.reckless = true;
                self.log(Audience::All, format!("{name} attacks recklessly (Advantage on Strength attacks; attacks against them have Advantage)."));
            }
            Feature::SecondWind => {
                need("Second Wind")?;
                if c.resources.second_wind_used >= classes::second_wind_uses(lvl) {
                    return rules("no Second Wind uses left");
                }
                let ic = self.feature_turn_check(who, entity, Cost::Bonus)?;
                self.pay(ic, Cost::Bonus);
                self.creature_mut(entity).resources.second_wind_used += 1;
                let r = DiceExpr::dice(1, 10)
                    .with_modifier(lvl as i32)
                    .roll(self.rng.as_mut(), false);
                self.log(
                    Audience::All,
                    format!("{name} uses Second Wind: {}", r.breakdown()),
                );
                self.heal(entity, r.total);
                self.emit(
                    Audience::All,
                    Event::SpellVisual {
                        caster: entity,
                        name: "Second Wind".into(),
                        vfx: Vfx::Healing,
                        from: c.pos,
                        targets: vec![c.pos],
                        area: vec![],
                    },
                );
            }
            Feature::ActionSurge => {
                need("Action Surge")?;
                if c.resources.action_surge_used >= classes::action_surge_uses(lvl) {
                    return rules("Action Surge already used (recharges on a Short or Long Rest)");
                }
                let ic = self.feature_turn_check(who, entity, Cost::Free)?;
                if !ic {
                    return rules("Action Surge is for your turn in combat");
                }
                if self
                    .state
                    .combat
                    .as_ref()
                    .is_some_and(|cb| cb.turn.surge_used_this_turn)
                {
                    return rules("only once on a turn");
                }
                self.creature_mut(entity).resources.action_surge_used += 1;
                let t = &mut self.state.combat.as_mut().expect("checked").turn;
                t.extra_actions += 1;
                t.surge_used_this_turn = true;
                self.log(
                    Audience::All,
                    format!(
                        "{name} uses Action Surge: one additional action (not the Magic action)."
                    ),
                );
                self.emit(
                    Audience::All,
                    Event::Floating {
                        entity,
                        text: "Action Surge".into(),
                        kind: FloatKind::Info,
                    },
                );
            }
            Feature::CunningDash | Feature::CunningDisengage => {
                need("Cunning Action")?;
                let ic = self.feature_turn_check(who, entity, Cost::Bonus)?;
                if !ic {
                    return rules("Cunning Action is used on your turn in combat");
                }
                self.pay(ic, Cost::Bonus);
                let speed = c.effective_speed();
                let t = &mut self.state.combat.as_mut().expect("checked").turn;
                if feature == Feature::CunningDash {
                    t.movement_left_ft += speed;
                    self.log(Audience::All, format!("{name} Dashes as a Bonus Action."));
                } else {
                    t.disengaged = true;
                    self.log(
                        Audience::All,
                        format!("{name} Disengages as a Bonus Action."),
                    );
                }
            }
            Feature::SteadyAim => {
                need("Steady Aim")?;
                let ic = self.feature_turn_check(who, entity, Cost::Bonus)?;
                if !ic {
                    return rules("Steady Aim is used on your turn in combat");
                }
                if self.state.combat.as_ref().is_some_and(|cb| cb.turn.moved) {
                    return rules("Steady Aim requires that you haven't moved this turn");
                }
                self.pay(ic, Cost::Bonus);
                let t = &mut self.state.combat.as_mut().expect("checked").turn;
                t.steady_aim = true;
                t.movement_left_ft = 0;
                self.log(
                    Audience::All,
                    format!("{name} takes Steady Aim (Advantage on the next attack; Speed 0)."),
                );
            }
            Feature::MartialArtsStrike | Feature::FlurryOfBlows => {
                need("Martial Arts")?;
                let flurry = feature == Feature::FlurryOfBlows;
                if flurry {
                    need("Monk’s Focus")?;
                    if c.resources.focus_used >= classes::focus_points(lvl) {
                        return rules("no Focus Points left");
                    }
                }
                let ic = self.feature_turn_check(who, entity, Cost::Bonus)?;
                if !ic {
                    return rules("bonus-action strikes are for your turn in combat");
                }
                self.pay(ic, Cost::Bonus);
                if flurry {
                    self.creature_mut(entity).resources.focus_used += 1;
                }
                self.state
                    .combat
                    .as_mut()
                    .expect("checked")
                    .turn
                    .bonus_attacks += if flurry { 2 } else { 1 };
                self.log(
                    Audience::All,
                    format!(
                        "{name}: {}",
                        if flurry {
                            "Flurry of Blows (two Unarmed Strikes)"
                        } else {
                            "bonus Unarmed Strike"
                        }
                    ),
                );
            }
            Feature::PatientDefense { focus } | Feature::StepOfTheWind { focus } => {
                need("Monk’s Focus")?;
                if focus && c.resources.focus_used >= classes::focus_points(lvl) {
                    return rules("no Focus Points left");
                }
                let ic = self.feature_turn_check(who, entity, Cost::Bonus)?;
                if !ic {
                    return rules("use this on your turn in combat");
                }
                self.pay(ic, Cost::Bonus);
                if focus {
                    self.creature_mut(entity).resources.focus_used += 1;
                }
                let speed = c.effective_speed();
                let patient = matches!(feature, Feature::PatientDefense { .. });
                let t = &mut self.state.combat.as_mut().expect("checked").turn;
                match (patient, focus) {
                    (true, false) => t.disengaged = true,
                    (true, true) => {
                        t.disengaged = true;
                        self.creature_mut(entity).effects.dodging = true;
                    }
                    (false, false) => t.movement_left_ft += speed,
                    (false, true) => {
                        t.disengaged = true;
                        t.movement_left_ft += speed;
                    }
                }
                self.log(
                    Audience::All,
                    format!(
                        "{name} uses {}{}.",
                        if patient {
                            "Patient Defense"
                        } else {
                            "Step of the Wind"
                        },
                        if focus { " (1 Focus Point)" } else { "" }
                    ),
                );
            }
            Feature::StunningStrike => {
                need("Stunning Strike")?;
                if c.resources.focus_used >= classes::focus_points(lvl) {
                    return rules("no Focus Points left");
                }
                self.feature_turn_check(who, entity, Cost::Free)?;
                self.creature_mut(entity).status.stunning_strike_ready = true;
                self.log(
                    Audience::All,
                    format!("{name} readies a Stunning Strike for the next hit."),
                );
            }
            Feature::LayOnHands => {
                need("Lay On Hands")?;
                let target = target.unwrap_or(entity);
                let t = self.creature(target)?.clone();
                if grid_distance_ft(c.pos, t.pos) > 5 {
                    return rules("Lay On Hands requires touch (5 ft)");
                }
                let pool = 5 * lvl as i32 - c.resources.lay_on_hands_used;
                let want = amount.unwrap_or(t.hp.max - t.hp.current).clamp(0, pool);
                if want <= 0 {
                    return rules("nothing to heal or the pool is empty");
                }
                let ic = self.feature_turn_check(who, entity, Cost::Bonus)?;
                self.pay(ic, Cost::Bonus);
                self.creature_mut(entity).resources.lay_on_hands_used += want;
                self.log(
                    Audience::All,
                    format!(
                        "{name} uses Lay On Hands on {} ({want} HP; {} left in the pool).",
                        t.name,
                        pool - want
                    ),
                );
                self.heal(target, want);
                self.emit(
                    Audience::All,
                    Event::SpellVisual {
                        caster: entity,
                        name: "Lay On Hands".into(),
                        vfx: Vfx::Healing,
                        from: c.pos,
                        targets: vec![t.pos],
                        area: vec![],
                    },
                );
            }
            Feature::BardicInspiration => {
                need("Bardic Inspiration")?;
                let target = target
                    .ok_or_else(|| CommandError::Invalid("choose a creature to inspire".into()))?;
                if target == entity {
                    return rules("you inspire another creature");
                }
                let t = self.creature(target)?.clone();
                if grid_distance_ft(c.pos, t.pos) > 60 {
                    return rules("the creature must be within 60 feet");
                }
                if t.status.inspiration.is_some() {
                    return rules("a creature can have only one Bardic Inspiration die at a time");
                }
                let uses = c.mods[Ability::Charisma.index()].max(1) as u8;
                if c.resources.bardic_used >= uses {
                    return rules("no Bardic Inspiration uses left");
                }
                let ic = self.feature_turn_check(who, entity, Cost::Bonus)?;
                self.pay(ic, Cost::Bonus);
                self.creature_mut(entity).resources.bardic_used += 1;
                let die = classes::bardic_inspiration_die(lvl);
                self.creature_mut(target).status.inspiration = Some(die);
                self.log(
                    Audience::All,
                    format!("{name} inspires {} (d{die}).", t.name),
                );
                self.emit(
                    Audience::All,
                    Event::SpellVisual {
                        caster: entity,
                        name: "Bardic Inspiration".into(),
                        vfx: Vfx::Buff,
                        from: c.pos,
                        targets: vec![t.pos],
                        area: vec![],
                    },
                );
            }
            Feature::UseInspiration => {
                if c.status.inspiration.is_none() {
                    return rules("no Bardic Inspiration die");
                }
                self.feature_turn_check(who, entity, Cost::Free)?;
                let armed = !c.status.inspiration_armed;
                self.creature_mut(entity).status.inspiration_armed = armed;
                self.log(
                    Audience::DmOnly,
                    format!(
                        "{name} {} Bardic Inspiration for the next failed roll.",
                        if armed { "readies" } else { "holds" }
                    ),
                );
            }
            Feature::DivineSparkHeal | Feature::DivineSparkDamage => {
                need("Channel Divinity")?;
                if c.resources.channel_divinity_used
                    >= classes::channel_divinity_uses(ClassId::Cleric, lvl)
                {
                    return rules("no Channel Divinity uses left");
                }
                let target =
                    target.ok_or_else(|| CommandError::Invalid("choose a creature".into()))?;
                let t = self.creature(target)?.clone();
                if target == entity || grid_distance_ft(c.pos, t.pos) > 30 {
                    return rules("Divine Spark targets another creature within 30 feet");
                }
                let ic = self.feature_turn_check(who, entity, Cost::Action)?;
                self.pay(ic, Cost::Action);
                self.creature_mut(entity).resources.channel_divinity_used += 1;
                let dice_n = match lvl {
                    0..=6 => 1,
                    7..=12 => 2,
                    13..=17 => 3,
                    _ => 4,
                };
                let r = DiceExpr::dice(dice_n, 8)
                    .with_modifier(c.mods[Ability::Wisdom.index()])
                    .roll(self.rng.as_mut(), false);
                self.log(
                    Audience::All,
                    format!(
                        "{name} channels Divine Spark at {}: {}",
                        t.name,
                        r.breakdown()
                    ),
                );
                if feature == Feature::DivineSparkHeal {
                    self.heal(target, r.total.max(0));
                    self.emit(
                        Audience::All,
                        Event::SpellVisual {
                            caster: entity,
                            name: "Divine Spark".into(),
                            vfx: Vfx::Healing,
                            from: c.pos,
                            targets: vec![t.pos],
                            area: vec![],
                        },
                    );
                } else {
                    let dc = sheet.spell_save_dc().unwrap_or(10);
                    let (ok, _) =
                        self.saving_throw(target, Ability::Constitution, dc, "Divine Spark");
                    let dmg = if ok {
                        r.total.max(0) / 2
                    } else {
                        r.total.max(0)
                    };
                    let o = self
                        .defenses_of(&t)
                        .apply(dnd_rules::DamageType::Radiant, dmg, 0);
                    self.emit(
                        Audience::All,
                        Event::SpellVisual {
                            caster: entity,
                            name: "Divine Spark".into(),
                            vfx: Vfx::Radiant,
                            from: c.pos,
                            targets: vec![t.pos],
                            area: vec![],
                        },
                    );
                    self.emit(
                        Audience::All,
                        Event::Floating {
                            entity: target,
                            text: format!("-{}", o.final_amount),
                            kind: FloatKind::Damage,
                        },
                    );
                    self.apply_damage_from(target, o.final_amount, false, Some(entity));
                }
            }
            Feature::InnateSorcery => {
                need("Innate Sorcery")?;
                if c.resources.innate_sorcery_used >= 2 {
                    return rules("Innate Sorcery is used up until a Long Rest");
                }
                let ic = self.feature_turn_check(who, entity, Cost::Bonus)?;
                self.pay(ic, Cost::Bonus);
                self.creature_mut(entity).resources.innate_sorcery_used += 1;
                self.add_effect(
                    entity,
                    "Innate Sorcery",
                    EffectKind::InnateSorcery { target: entity },
                    Some(10),
                    false,
                );
                self.log(Audience::All, format!("{name} unleashes Innate Sorcery (+1 spell save DC, Advantage on Sorcerer spell attacks for 1 minute)."));
                self.emit(
                    Audience::All,
                    Event::SpellVisual {
                        caster: entity,
                        name: "Innate Sorcery".into(),
                        vfx: Vfx::Arcane,
                        from: c.pos,
                        targets: vec![c.pos],
                        area: vec![],
                    },
                );
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------ rests

    pub(crate) fn rest(&mut self, long: bool) -> Res<()> {
        if self.state.combat.is_some() {
            return rules("you can't rest during combat");
        }
        // 2024: a Short Rest lasts 1 hour, a Long Rest 8 hours.
        self.state.time.advance(if long { 8 * 60 } else { 60 });
        let party: Vec<EntityId> = self
            .state
            .creatures
            .values()
            .filter(|c| c.faction == Faction::Party && c.is_alive())
            .map(|c| c.id)
            .collect();
        for id in &party {
            let c = self.state.creatures[id].clone();
            let Some(sheet) = c.sheet().cloned() else {
                continue;
            };
            let lvl = sheet.level;
            if long && c.hp.current <= 0 {
                // "To start a Long Rest, you must have at least 1 Hit Point."
                self.log(
                    Audience::All,
                    format!(
                        "{} is at 0 HP and gains no benefit from the Long Rest.",
                        c.name
                    ),
                );
                continue;
            }
            let cm = self.creature_mut(*id);
            let r = &mut cm.resources;
            // Short-rest recoveries (also part of a Long Rest).
            r.pact_used = 0;
            r.action_surge_used = 0;
            r.focus_used = 0;
            r.second_wind_used = r.second_wind_used.saturating_sub(1);
            r.rage_used = r.rage_used.saturating_sub(1);
            r.channel_divinity_used = r.channel_divinity_used.saturating_sub(1);
            if sheet.has_class_feature("Font of Inspiration") {
                r.bardic_used = 0;
            }
            if long {
                r.slots_used = [0; 9];
                r.second_wind_used = 0;
                r.rage_used = 0;
                r.channel_divinity_used = 0;
                r.lay_on_hands_used = 0;
                r.bardic_used = 0;
                r.innate_sorcery_used = 0;
                r.free_smite_used = false;
                r.arcane_recovery_used = false;
                r.hit_dice_used = 0;
                cm.hp.current = cm.hp.max;
                cm.hp.temp = 0;
                cm.hp.death_saves = Default::default();
                cm.conditions.long_rest();
                cm.conditions.remove(Condition::Unconscious);
            } else if sheet.class == ClassId::Wizard && !r.arcane_recovery_used {
                // Arcane Recovery: recover slots totalling up to half the Wizard level (rounded up), none 6+.
                let mut budget = lvl.div_ceil(2);
                let mut recovered = Vec::new();
                for l in (1..=5u8).rev() {
                    while budget >= l && r.slots_used[l as usize - 1] > 0 {
                        r.slots_used[l as usize - 1] -= 1;
                        budget -= l;
                        recovered.push(l);
                    }
                }
                if !recovered.is_empty() {
                    r.arcane_recovery_used = true;
                    let name = cm.name.clone();
                    self.log(
                        Audience::All,
                        format!("{name} uses Arcane Recovery (slots {:?}).", recovered),
                    );
                }
            }
        }
        if long {
            // Effects lasting up to 8 hours end; "until dispelled" effects stay.
            let ids: Vec<u64> = self
                .state
                .effects
                .iter()
                .filter(|e| e.rounds_left.is_some_and(|r| r <= 4800))
                .map(|e| e.id)
                .collect();
            for id in ids {
                self.remove_effect(id);
            }
            for c in self.state.creatures.values_mut() {
                c.concentration = None;
                c.status = CombatStatus {
                    milestone_levels: c.status.milestone_levels,
                    ..Default::default()
                };
            }
        }
        self.log(
            Audience::All,
            if long {
                "The party finishes a Long Rest: HP, spell slots and features restored."
            } else {
                "The party finishes a Short Rest. Spend Hit Point Dice from your character sheet."
            },
        );
        Ok(())
    }

    pub(crate) fn cmd_spend_hit_die(&mut self, who: Principal, entity: EntityId) -> Res<()> {
        let c = self.creature(entity)?.clone();
        if !Self::controls(who, &c) {
            return Err(CommandError::NotAuthorized);
        }
        let sheet = c
            .sheet()
            .ok_or_else(|| CommandError::Rules("characters only".into()))?;
        if self.state.combat.is_some() {
            return rules("Hit Point Dice are spent during a Short Rest, not in combat");
        }
        if c.hp.current == 0 || c.hp.dead {
            return rules("you need at least 1 Hit Point to rest");
        }
        if c.resources.hit_dice_used >= sheet.level {
            return rules("no Hit Point Dice left (regained on a Long Rest)");
        }
        let die = sheet.class_def().hit_die;
        let con = c.mods[Ability::Constitution.index()];
        let r = DiceExpr::dice(1, die)
            .with_modifier(con)
            .roll(self.rng.as_mut(), false);
        self.creature_mut(entity).resources.hit_dice_used += 1;
        self.log(
            Audience::All,
            format!("{} spends a Hit Point Die: {}", c.name, r.breakdown()),
        );
        self.heal(entity, r.total.max(1));
        Ok(())
    }

    // ------------------------------------------------------------------ progression

    pub(crate) fn award_xp(&mut self, entities: Vec<EntityId>, xp: u32) -> Res<()> {
        for id in &entities {
            self.creature(*id)?;
        }
        for id in entities {
            let c = self.creature_mut(id);
            let name = c.name.clone();
            if let Some(s) = c.sheet_mut() {
                s.experience_points = s.experience_points.saturating_add(xp);
                let can = classes::level_for_xp(s.experience_points) > s.level;
                self.log(
                    Audience::All,
                    format!(
                        "{name} gains {xp} XP{}",
                        if can { " — level up available!" } else { "." }
                    ),
                );
            }
        }
        Ok(())
    }

    pub(crate) fn cmd_level_up(
        &mut self,
        who: Principal,
        entity: EntityId,
        choices: LevelUpChoices,
    ) -> Res<()> {
        let c = self.creature(entity)?.clone();
        if !Self::controls(who, &c) {
            return Err(CommandError::NotAuthorized);
        }
        if self.state.combat.is_some() {
            return rules("level up outside combat");
        }
        let mut sheet = c
            .sheet()
            .cloned()
            .ok_or_else(|| CommandError::Rules("characters only".into()))?;
        if sheet.level >= 20 {
            return rules("already level 20");
        }
        let by_xp = classes::level_for_xp(sheet.experience_points) > sheet.level;
        if !by_xp && c.status.milestone_levels == 0 && who != Principal::Dm {
            return rules(format!(
                "{} XP needed for level {}",
                classes::XP_THRESHOLDS[sheet.level as usize],
                sheet.level + 1
            ));
        }
        sheet.level += 1;
        let new_level = sheet.level;
        if new_level >= 3 && sheet.subclass.is_none() {
            sheet.subclass = Some(classes::srd_subclass(sheet.class).to_string());
        }
        if classes::is_asi_level(sheet.class, new_level) {
            let pts: u8 = choices.asi.iter().map(|(_, n)| *n).sum();
            if pts != 2 {
                return rules("Ability Score Improvement: +2 to one ability or +1 to two");
            }
            sheet.asi.extend(choices.asi.iter().copied());
        } else if !choices.asi.is_empty() {
            return rules(format!(
                "level {new_level} doesn't grant an Ability Score Improvement"
            ));
        }
        if choices.fighting_style.is_some() {
            sheet.fighting_style = choices.fighting_style;
        }
        sheet.expertise.extend(choices.expertise.iter().copied());
        sheet.cantrips.extend(choices.new_cantrips.iter().cloned());
        sheet
            .prepared_spells
            .extend(choices.new_prepared.iter().cloned());
        sheet
            .weapon_masteries
            .extend(choices.new_masteries.iter().cloned());
        let errors: Vec<String> = sheet
            .validate()
            .into_iter()
            .filter(|i| i.severity == dnd_rules::character::Severity::Error)
            .map(|i| i.detail)
            .collect();
        if !errors.is_empty() {
            return Err(CommandError::Invalid(errors.join("; ")));
        }
        let cm = self.creature_mut(entity);
        if !by_xp && cm.status.milestone_levels > 0 {
            cm.status.milestone_levels -= 1;
        }
        let old_max = cm.hp.max;
        *cm.sheet_mut().expect("pc") = sheet.clone();
        cm.refresh_from_sheet();
        let gained = cm.hp.max - old_max;
        let name = cm.name.clone();
        let new_feats: Vec<String> =
            classes::features(sheet.class, new_level, sheet.subclass.as_deref())
                .into_iter()
                .filter(|f| f.level == new_level)
                .map(|f| f.name.clone())
                .collect();
        self.log(
            Audience::All,
            format!(
                "{name} reaches level {new_level}! (+{gained} HP) New: {}",
                if new_feats.is_empty() {
                    "—".into()
                } else {
                    new_feats.join(", ")
                }
            ),
        );
        self.emit(
            Audience::All,
            Event::Floating {
                entity,
                text: format!("Level {new_level}!"),
                kind: FloatKind::Info,
            },
        );
        let pos = self.state.creatures[&entity].pos;
        self.emit(
            Audience::All,
            Event::SpellVisual {
                caster: entity,
                name: "Level Up".into(),
                vfx: Vfx::Radiant,
                from: pos,
                targets: vec![pos],
                area: vec![],
            },
        );
        Ok(())
    }

    pub(crate) fn cmd_prepare_spells(
        &mut self,
        who: Principal,
        entity: EntityId,
        cantrips: Vec<String>,
        prepared: Vec<String>,
    ) -> Res<()> {
        let c = self.creature(entity)?.clone();
        if !Self::controls(who, &c) {
            return Err(CommandError::NotAuthorized);
        }
        if self
            .state
            .combat
            .as_ref()
            .is_some_and(|cb| cb.room == c.room)
        {
            return rules("change prepared spells outside combat");
        }
        let mut sheet = c
            .sheet()
            .cloned()
            .ok_or_else(|| CommandError::Rules("characters only".into()))?;
        sheet.cantrips = cantrips;
        sheet.prepared_spells = prepared;
        let errors: Vec<String> = sheet
            .validate()
            .into_iter()
            .filter(|i| {
                i.severity == dnd_rules::character::Severity::Error
                    && i.key.starts_with("char.spells")
            })
            .map(|i| i.detail)
            .collect();
        if !errors.is_empty() {
            return Err(CommandError::Invalid(errors.join("; ")));
        }
        *self.creature_mut(entity).sheet_mut().expect("pc") = sheet;
        self.log(
            Audience::DmOnly,
            format!("{} changes prepared spells.", c.name),
        );
        Ok(())
    }
}
