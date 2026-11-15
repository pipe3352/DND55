//! Casting spells (SRD 5.2.1 "Spells" chapter rules) and the shared machinery for effects,
//! Concentration, saving throws and areas of effect.

use crate::effects::{Effect, EffectKind, LightAnchor};
use crate::event::{Event, FloatKind, Vfx};
use crate::model::*;
use crate::session::{CommandError, Session};
use dnd_rules::Ability;
use dnd_rules::attack::{AttackModifiers, AttackProfile, DamagePart, resolve_attack};
use dnd_rules::conditions::{AttackSituation, Condition, attack_roll_sources};
use dnd_rules::content::ClassId;
use dnd_rules::d20::{D20Roll, ModeSources};
use dnd_rules::damage::DamageType;
use dnd_rules::dice::DiceExpr;
use dnd_rules::equipment::WeaponKind;
use dnd_rules::spells::{AreaShape, CastingKind, Spell, SpellAttackKind};
use dnd_world::EntityId;
use dnd_world::map::Cell;
use dnd_world::path::grid_distance_ft;
use dnd_world::vision::line_of_sight;

type Res<T> = Result<T, CommandError>;

fn rules<T>(s: impl Into<String>) -> Res<T> {
    Err(CommandError::Rules(s.into()))
}

pub(crate) fn vfx_for_damage(t: DamageType) -> Vfx {
    match t {
        DamageType::Fire => Vfx::Fire,
        DamageType::Cold => Vfx::Cold,
        DamageType::Lightning => Vfx::Lightning,
        DamageType::Poison => Vfx::Poison,
        DamageType::Acid => Vfx::Acid,
        DamageType::Radiant => Vfx::Radiant,
        DamageType::Necrotic => Vfx::Necrotic,
        DamageType::Force => Vfx::Force,
        DamageType::Thunder => Vfx::Thunder,
        DamageType::Psychic => Vfx::Psychic,
        DamageType::Slashing => Vfx::Slash,
        DamageType::Piercing => Vfx::Pierce,
        DamageType::Bludgeoning => Vfx::Bludgeon,
    }
}

/// Cells covered by an area of effect. `origin` is the caster's cell; `point` the chosen point.
/// Cones and lines start at the caster and point toward `point`.
pub fn area_cells(
    shape: AreaShape,
    size_ft: u32,
    origin: Cell,
    point: Cell,
    from_point: bool,
) -> Vec<Cell> {
    let n = (size_ft / 5) as i32;
    let mut out = Vec::new();
    match shape {
        AreaShape::Sphere | AreaShape::Cylinder => {
            for y in point.1 - n..=point.1 + n {
                for x in point.0 - n..=point.0 + n {
                    if grid_distance_ft(point, (x, y)) <= size_ft as i32 {
                        out.push((x, y));
                    }
                }
            }
        }
        AreaShape::Emanation => {
            for y in origin.1 - n..=origin.1 + n {
                for x in origin.0 - n..=origin.0 + n {
                    if (x, y) != origin && grid_distance_ft(origin, (x, y)) <= size_ft as i32 {
                        out.push((x, y));
                    }
                }
            }
        }
        AreaShape::Cube => {
            if from_point {
                // A Cube placed at a point: the point is one corner-ish cell; cover n×n cells.
                for y in point.1..point.1 + n {
                    for x in point.0..point.0 + n {
                        out.push((x, y));
                    }
                }
            } else {
                // "Cube originating from you": the cube's face touches the caster, extending toward the point.
                let (dx, dy) = (point.0 - origin.0, point.1 - origin.1);
                let (sx, sy) = if dx.abs() >= dy.abs() {
                    (dx.signum().max(-1).min(1), 0)
                } else {
                    (0, dy.signum())
                };
                let (sx, sy) = if sx == 0 && sy == 0 { (1, 0) } else { (sx, sy) };
                let half = (n - 1) / 2;
                for i in 1..=n {
                    for j in -half..=(n - 1 - half) {
                        let c = if sx != 0 {
                            (origin.0 + sx * i, origin.1 + j)
                        } else {
                            (origin.0 + j, origin.1 + sy * i)
                        };
                        out.push(c);
                    }
                }
            }
        }
        AreaShape::Cone => {
            // Width at any distance equals that distance: half-angle atan(1/2) ≈ 26.57°.
            let (dx, dy) = ((point.0 - origin.0) as f32, (point.1 - origin.1) as f32);
            let len = (dx * dx + dy * dy).sqrt().max(0.001);
            let (ux, uy) = (dx / len, dy / len);
            for y in origin.1 - n..=origin.1 + n {
                for x in origin.0 - n..=origin.0 + n {
                    if (x, y) == origin {
                        continue;
                    }
                    let (vx, vy) = ((x - origin.0) as f32, (y - origin.1) as f32);
                    let along = vx * ux + vy * uy;
                    let across = (vx * uy - vy * ux).abs();
                    if along > 0.0 && along <= n as f32 + 0.01 && across <= along * 0.5 + 0.5 {
                        out.push((x, y));
                    }
                }
            }
        }
        AreaShape::Line => {
            let (dx, dy) = ((point.0 - origin.0) as f32, (point.1 - origin.1) as f32);
            let len = (dx * dx + dy * dy).sqrt().max(0.001);
            let (ux, uy) = (dx / len, dy / len);
            for y in origin.1 - n..=origin.1 + n {
                for x in origin.0 - n..=origin.0 + n {
                    if (x, y) == origin {
                        continue;
                    }
                    let (vx, vy) = ((x - origin.0) as f32, (y - origin.1) as f32);
                    let along = vx * ux + vy * uy;
                    let across = (vx * uy - vy * ux).abs();
                    if along > 0.0 && along <= n as f32 + 0.01 && across <= 0.5 {
                        out.push((x, y));
                    }
                }
            }
        }
    }
    out
}

impl Session {
    // ------------------------------------------------------------------ effects

    pub(crate) fn add_effect(
        &mut self,
        source: EntityId,
        origin: &str,
        kind: EffectKind,
        rounds: Option<u32>,
        concentration: bool,
    ) -> u64 {
        let id = self.next_id();
        if let EffectKind::Condition {
            target, condition, ..
        } = &kind
        {
            self.creature_mut(*target).conditions.add(*condition);
        }
        self.state.effects.push(Effect {
            id,
            source,
            origin: origin.into(),
            kind,
            rounds_left: rounds,
            concentration,
        });
        id
    }

    pub(crate) fn remove_effect(&mut self, id: u64) {
        let Some(pos) = self.state.effects.iter().position(|e| e.id == id) else {
            return;
        };
        let e = self.state.effects.remove(pos);
        if let EffectKind::Condition {
            target, condition, ..
        } = e.kind
        {
            // Keep the condition if another effect still imposes it.
            let still = self.state.effects.iter().any(|o| matches!(&o.kind, EffectKind::Condition { target: t, condition: c, .. } if *t == target && *c == condition));
            if !still && let Some(c) = self.state.creatures.get_mut(&target) {
                c.conditions.remove(condition);
                let name = c.name.clone();
                self.emit(
                    Audience::All,
                    Event::Floating {
                        entity: target,
                        text: format!("-{condition:?}"),
                        kind: FloatKind::Condition,
                    },
                );
                self.log(Audience::All, format!("{name} is no longer {condition:?}."));
            }
        }
    }

    /// Effects created by `source` tick at the start of its turn.
    pub(crate) fn tick_effects(&mut self, source: EntityId) {
        let mut expired = Vec::new();
        for e in self.state.effects.iter_mut().filter(|e| e.source == source) {
            if let Some(r) = e.rounds_left.as_mut() {
                *r = r.saturating_sub(1);
                if *r == 0 {
                    expired.push(e.id);
                }
            }
            if matches!(
                e.kind,
                EffectKind::Condition {
                    ends_at_source_turn: true,
                    ..
                }
            ) {
                expired.push(e.id);
            }
        }
        for id in expired {
            if let Some(e) = self.state.effects.iter().find(|e| e.id == id) {
                let label = e.label();
                self.log(Audience::All, format!("{label} ends."));
            }
            self.remove_effect(id);
            if !self
                .state
                .effects
                .iter()
                .any(|e| e.source == source && e.concentration)
            {
                if let Some(c) = self.state.creatures.get_mut(&source) {
                    c.concentration = None;
                }
            }
        }
    }

    pub(crate) fn end_concentration(&mut self, entity: EntityId, why: &str) {
        let Some(spell) = self
            .state
            .creatures
            .get(&entity)
            .and_then(|c| c.concentration.clone())
        else {
            return;
        };
        let ids: Vec<u64> = self
            .state
            .effects
            .iter()
            .filter(|e| e.source == entity && e.concentration)
            .map(|e| e.id)
            .collect();
        for id in ids {
            self.remove_effect(id);
        }
        let name = self.state.creatures[&entity].name.clone();
        self.creature_mut(entity).concentration = None;
        let spell_name = dnd_rules::spells::spell(&spell)
            .map(|s| s.name.clone())
            .unwrap_or(spell);
        self.log(
            Audience::All,
            format!("{name} loses Concentration on {spell_name} ({why})."),
        );
    }

    pub(crate) fn has_bless(&self, id: EntityId) -> bool {
        self.state
            .effects
            .iter()
            .any(|e| matches!(&e.kind, EffectKind::Bless { targets } if targets.contains(&id)))
    }

    fn has_effect(&self, id: EntityId, f: impl Fn(&EffectKind) -> bool) -> bool {
        self.state
            .effects
            .iter()
            .any(|e| e.targets().contains(&id) && f(&e.kind))
    }

    /// AC including Mage Armor and Shield.
    pub fn effective_ac(&self, c: &Creature) -> i32 {
        let mut ac = c.armor_class;
        let unarmored = c.sheet().is_none_or(|s| s.body_armor.is_none());
        if unarmored && self.has_effect(c.id, |k| matches!(k, EffectKind::MageArmor { .. })) {
            let shield = if c.sheet().is_some_and(|s| s.shield) {
                2
            } else {
                0
            };
            ac = ac.max(13 + c.mods[Ability::Dexterity.index()] + shield);
        }
        if self.has_effect(c.id, |k| matches!(k, EffectKind::Shield { .. })) {
            ac += 5;
        }
        ac
    }

    /// Damage defenses including Rage (Resistance to Bludgeoning, Piercing, Slashing) and Petrified.
    pub fn defenses_of(&self, c: &Creature) -> dnd_rules::Defenses {
        let mut d = c.defenses.clone();
        if c.status.raging {
            d.resistances.extend([
                DamageType::Bludgeoning,
                DamageType::Piercing,
                DamageType::Slashing,
            ]);
        }
        if c.conditions.has(Condition::Petrified) {
            d.resist_all = true;
        }
        d
    }

    /// Rolls a saving throw for a creature. Returns (success, total).
    pub(crate) fn saving_throw(
        &mut self,
        target: EntityId,
        ability: Ability,
        dc: i32,
        label: &str,
    ) -> (bool, i32) {
        let c = self.state.creatures[&target].clone();
        if c.conditions.auto_fails_save(ability) {
            self.log(
                Audience::All,
                format!(
                    "{} automatically fails the {ability:?} save ({label}).",
                    c.name
                ),
            );
            return (false, 0);
        }
        let mut sources = ModeSources::default();
        if ability == Ability::Dexterity {
            if c.effects.dodging
                && !c.conditions.has(Condition::Incapacitated)
                && c.effective_speed() > 0
            {
                sources.adv("Dodge");
            }
            if c.conditions.has(Condition::Restrained) {
                sources.dis("Restrained");
            }
            if c.has_feature("Danger Sense") && !c.conditions.has(Condition::Incapacitated) {
                sources.adv("Danger Sense");
            }
        }
        if ability == Ability::Strength && c.status.raging {
            sources.adv("Rage");
        }
        let mut bonus = c.save_bonus(ability);
        let mut detail = String::new();
        // Aura of Protection (Paladin 6): +Cha mod (min 1) for the paladin and allies within 10 ft.
        if let Some(p) = self
            .state
            .creatures
            .values()
            .filter(|p| {
                p.room == c.room
                    && p.is_alive()
                    && p.faction == c.faction
                    && p.has_feature("Aura of Protection")
                    && grid_distance_ft(p.pos, c.pos) <= 10
                    && !p.conditions.has(Condition::Incapacitated)
            })
            .max_by_key(|p| p.mods[Ability::Charisma.index()])
        {
            let b = p.mods[Ability::Charisma.index()].max(1);
            bonus += b;
            detail.push_str(&format!(" +{b} aura"));
        }
        let mut roll = D20Roll::roll(
            self.rng.as_mut(),
            sources.mode(),
            bonus,
            c.conditions.exhaustion,
        );
        if self.has_bless(target) {
            let b = DiceExpr::dice(1, 4).roll(self.rng.as_mut(), false).total;
            roll.total += b;
            detail.push_str(&format!(" +{b} Bless"));
        }
        // Bardic Inspiration: rolled only after a failure, possibly turning it into a success.
        if roll.total < dc
            && let (true, Some(die)) = (c.status.inspiration_armed, c.status.inspiration)
        {
            let b = DiceExpr::dice(1, die).roll(self.rng.as_mut(), false).total;
            roll.total += b;
            detail.push_str(&format!(" +{b} Bardic Inspiration"));
            let cm = self.creature_mut(target);
            cm.status.inspiration = None;
            cm.status.inspiration_armed = false;
        }
        let ok = roll.total >= dc;
        self.log(
            Audience::All,
            format!(
                "{} {ability:?} save vs DC {dc} ({label}): {}{detail} → {}",
                c.name,
                roll.breakdown(),
                if ok { "success" } else { "failure" }
            ),
        );
        (ok, roll.total)
    }

    // ------------------------------------------------------------------ slots

    /// Lowest available slot of at least `min_level` (Pact Magic slots for Warlocks).
    pub(crate) fn lowest_free_slot(&self, c: &Creature, min_level: u8) -> Option<u8> {
        let sheet = c.sheet()?;
        if sheet.class == ClassId::Warlock {
            let (count, level) = dnd_rules::classes::pact_slots(sheet.level);
            return (level >= min_level && c.resources.pact_used < count).then_some(level);
        }
        let slots = sheet.spell_slots();
        (min_level.max(1)..=9)
            .find(|l| slots[*l as usize - 1] > c.resources.slots_used[*l as usize - 1])
    }

    pub(crate) fn slot_available(&self, c: &Creature, level: u8) -> bool {
        let Some(sheet) = c.sheet() else { return false };
        if sheet.class == ClassId::Warlock {
            let (count, pact) = dnd_rules::classes::pact_slots(sheet.level);
            return level == pact && c.resources.pact_used < count;
        }
        (1..=9).contains(&level)
            && sheet.spell_slots()[level as usize - 1] > c.resources.slots_used[level as usize - 1]
    }

    pub(crate) fn spend_slot(&mut self, id: EntityId, level: u8) {
        let warlock = self.state.creatures[&id].is_class(ClassId::Warlock);
        let c = self.creature_mut(id);
        if warlock {
            c.resources.pact_used += 1;
        } else if (1..=9).contains(&level) {
            c.resources.slots_used[level as usize - 1] += 1;
        }
    }

    // ------------------------------------------------------------------ casting

    pub(crate) fn cmd_cast_spell(
        &mut self,
        who: Principal,
        caster: EntityId,
        spell_id: &str,
        slot: Option<u8>,
        targets: Vec<EntityId>,
        point: Option<Cell>,
    ) -> Res<()> {
        let spell = dnd_rules::spells::spell(spell_id)
            .ok_or_else(|| CommandError::NotFound(format!("spell {spell_id}")))?;
        let c = self.creature(caster)?.clone();
        if !Self::controls(who, &c) {
            return Err(CommandError::NotAuthorized);
        }
        if !c.is_alive() || !c.conditions.can_act() {
            return rules(format!("{} can't cast spells right now", c.name));
        }
        if c.status.raging {
            return rules("you can't cast spells while raging");
        }
        // Monsters cast as the DM adjudicates (no slots); characters must know the spell.
        let is_pc = c.sheet().is_some();
        if is_pc && !c.sheet().is_some_and(|s| s.knows_spell(spell_id)) {
            return rules(format!("{} doesn't have {} prepared", c.name, spell.name));
        }
        // Slot level.
        let slot_level = if spell.is_cantrip() {
            0
        } else {
            let s = slot.unwrap_or(spell.level);
            if s < spell.level || s > 9 {
                return rules(format!(
                    "{} needs a level {}+ spell slot",
                    spell.name, spell.level
                ));
            }
            s
        };
        let free_smite = spell.id == "divine_smite"
            && c.has_feature("Paladin’s Smite")
            && !c.resources.free_smite_used
            && slot.is_none();
        if is_pc && slot_level > 0 && !free_smite && !self.slot_available(&c, slot_level) {
            return rules(format!("no level {slot_level} spell slot available"));
        }
        // Action economy.
        let kind = spell.casting_kind();
        let in_combat = self
            .state
            .combat
            .as_ref()
            .is_some_and(|cb| cb.room == c.room);
        if kind == CastingKind::Reaction {
            return rules(format!(
                "{} is cast as a Reaction when its trigger happens",
                spell.name
            ));
        }
        if in_combat {
            let cb = self.state.combat.as_ref().expect("checked");
            if cb.paused || cb.pending.is_some() || cb.pending_hit.is_some() {
                return rules("waiting for something else to resolve");
            }
            if cb.active() != caster {
                return rules("it isn't this creature's turn");
            }
            match kind {
                CastingKind::Long => {
                    return rules(format!(
                        "{} takes {} to cast: not in combat",
                        spell.name, spell.casting_time
                    ));
                }
                // The Magic action can't be the extra action from Action Surge.
                CastingKind::Action if cb.turn.action_used => {
                    return rules(
                        "action already used (Action Surge's extra action can't be the Magic action)",
                    );
                }
                CastingKind::BonusAction if cb.turn.bonus_action_used => {
                    return rules("bonus action already used");
                }
                _ => {}
            }
            if slot_level > 0 && !free_smite && cb.turn.spell_slot_used {
                return rules("you can expend only one spell slot to cast a spell per turn");
            }
        }
        // Divine Smite: immediately after hitting with a melee weapon or Unarmed Strike.
        let smite_target = if spell.auto.smite {
            let hit = self
                .state
                .combat
                .as_ref()
                .and_then(|cb| cb.turn.last_melee_hit);
            match hit {
                Some(h) => Some(h),
                None => {
                    return rules(
                        "cast Divine Smite right after hitting a target with a melee weapon or Unarmed Strike",
                    );
                }
            }
        } else {
            None
        };
        // Targets & range.
        let lights = self.light_levels(c.room);
        let map_ok = self.room(c.room)?.map.clone();
        let range = spell.range_ft();
        let caster_level = c.level().max(1);
        let a = &spell.auto;
        let point_needed = a.area.is_some()
            && !matches!(spell.range(), dnd_rules::spells::SpellRange::SelfOnly)
            || a.light.is_some() && targets.is_empty() && point.is_some();
        if let Some(p) = point {
            if range != u32::MAX
                && grid_distance_ft(c.pos, p) > range as i32
                && !matches!(spell.range(), dnd_rules::spells::SpellRange::SelfOnly)
            {
                return rules(format!(
                    "that point is beyond {}'s range ({} ft)",
                    spell.name, range
                ));
            }
            if !line_of_sight(&map_ok, c.pos, p) {
                return rules("no line of sight to that point");
            }
        } else if point_needed && a.area.is_some() {
            return rules(format!("choose a point for {}", spell.name));
        }
        let max_targets = spell
            .max_targets(slot_level)
            .max(spell.projectiles(slot_level, caster_level)) as usize;
        if targets.len() > max_targets.max(1) {
            return rules(format!("{} can target at most {max_targets}", spell.name));
        }
        for t in &targets {
            let tc = self.creature(*t)?;
            if tc.room != c.room {
                return rules("target isn't in this room");
            }
            let d = grid_distance_ft(c.pos, tc.pos);
            if range != u32::MAX && d > range.max(5) as i32 && *t != caster {
                return rules(format!(
                    "{} is {d} ft away; {} has a range of {}",
                    tc.name, spell.name, spell.range
                ));
            }
            if *t != caster && !line_of_sight(&map_ok, c.pos, tc.pos) {
                return rules(format!("no line of sight to {}", tc.name));
            }
            if a.humanoid_only && !tc.creature_type.contains("Humanoid") {
                return rules(format!("{} only affects Humanoids", spell.name));
            }
        }
        let needs_target = a.area.is_none()
            && a.light.is_none()
            && !spell.auto.smite
            && spell.range() != dnd_rules::spells::SpellRange::SelfOnly
            && a.buff.is_none()
            && (a.attack.is_some()
                || a.save.is_some()
                || a.healing.is_some()
                || a.auto_hit
                || a.condition.is_some());
        if needs_target && targets.is_empty() {
            return rules(format!("choose a target for {}", spell.name));
        }
        let _ = &lights;

        // ---- validated: pay costs
        if in_combat {
            let cb = self.state.combat.as_mut().expect("checked");
            match kind {
                CastingKind::Action => cb.turn.action_used = true,
                CastingKind::BonusAction => cb.turn.bonus_action_used = true,
                _ => {}
            }
            if slot_level > 0 && !free_smite {
                cb.turn.spell_slot_used = true;
            }
        }
        if is_pc && slot_level > 0 {
            if free_smite {
                self.creature_mut(caster).resources.free_smite_used = true;
            } else {
                self.spend_slot(caster, slot_level);
            }
        }
        if spell.concentration {
            self.end_concentration(caster, &format!("cast {}", spell.name));
            self.creature_mut(caster).concentration = Some(spell.id.clone());
        }
        {
            let cm = self.creature_mut(caster);
            cm.hidden = false;
        }
        let slot_txt = if slot_level > 0 {
            format!(" (level {slot_level} slot)")
        } else {
            String::new()
        };
        self.log(
            Audience::All,
            format!("{} casts {}{slot_txt}.", c.name, spell.name),
        );
        self.resolve_spell(
            &c,
            spell,
            slot_level,
            caster_level,
            targets,
            point,
            smite_target,
        );
        Ok(())
    }

    fn spell_dc(&self, c: &Creature, spell: &Spell) -> i32 {
        let base = c
            .sheet()
            .and_then(|s| s.spell_save_dc())
            .unwrap_or(8 + c.mods.iter().copied().max().unwrap_or(0) + 2);
        let innate = spell.classes.iter().any(|k| k == "Sorcerer")
            && self.has_effect(c.id, |k| matches!(k, EffectKind::InnateSorcery { .. }));
        base + if innate { 1 } else { 0 }
    }

    fn spell_attack_bonus(&self, c: &Creature) -> i32 {
        c.sheet()
            .and_then(|s| s.spell_attack_bonus())
            .unwrap_or(c.mods.iter().copied().max().unwrap_or(0) + 2)
    }

    fn spell_mod(&self, c: &Creature) -> i32 {
        c.sheet()
            .and_then(|s| s.spellcasting_ability().map(|a| s.modifier(a)))
            .unwrap_or(0)
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_spell(
        &mut self,
        c: &Creature,
        spell: &Spell,
        slot: u8,
        caster_level: u8,
        targets: Vec<EntityId>,
        point: Option<Cell>,
        smite: Option<(EntityId, bool)>,
    ) {
        let a = &spell.auto;
        let caster = c.id;
        let rounds = spell.duration_rounds();
        let vfx = a
            .damage
            .as_ref()
            .map(|d| vfx_for_damage(d.damage_type))
            .unwrap_or(if a.healing.is_some() {
                Vfx::Healing
            } else if a.light.is_some() {
                Vfx::Light
            } else {
                Vfx::Arcane
            });

        // Divine Smite: extra Radiant damage to the creature just hit (doubled on a Critical Hit).
        if let Some((target, crit)) = smite {
            let mut dice = spell
                .damage_dice(slot.max(1), caster_level)
                .unwrap_or_else(|| DiceExpr::dice(2, 8));
            let t = self.state.creatures[&target].clone();
            if a.extra_vs
                .iter()
                .any(|k| t.creature_type.contains(k.as_str()))
            {
                dice.terms[0].count += 1;
            }
            let roll = dice.roll(self.rng.as_mut(), crit);
            let o = self
                .defenses_of(&t)
                .apply(DamageType::Radiant, roll.total, 0);
            self.log(
                Audience::All,
                format!(
                    "Divine Smite on {}: {} → {} Radiant",
                    t.name,
                    roll.breakdown(),
                    o.final_amount
                ),
            );
            self.emit(
                Audience::All,
                Event::SpellVisual {
                    caster,
                    name: spell.name.clone(),
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
            self.apply_damage_from(target, o.final_amount, false, Some(caster));
            return;
        }

        // Light-producing spells: attach to a target creature (its gear) or to a point.
        if let Some(light) = a.light {
            let anchor = match (targets.first(), point) {
                (Some(t), _) => LightAnchor::Creature(*t),
                (None, Some(p)) => LightAnchor::Cell(c.room, p),
                (None, None) => LightAnchor::Creature(caster),
            };
            if spell.id == "light" {
                // "The spell ends if you cast it again."
                let old: Vec<u64> = self
                    .state
                    .effects
                    .iter()
                    .filter(|e| e.source == caster && e.origin == "light")
                    .map(|e| e.id)
                    .collect();
                for id in old {
                    self.remove_effect(id);
                }
            }
            let at = match &anchor {
                LightAnchor::Creature(id) => self.state.creatures[id].pos,
                LightAnchor::Cell(_, p) => *p,
            };
            self.add_effect(
                caster,
                &spell.id,
                EffectKind::Light {
                    anchor,
                    bright_ft: light.bright_ft,
                    dim_ft: light.dim_ft,
                },
                rounds,
                spell.concentration,
            );
            self.emit(
                Audience::All,
                Event::SpellVisual {
                    caster,
                    name: spell.name.clone(),
                    vfx: Vfx::Light,
                    from: c.pos,
                    targets: vec![at],
                    area: vec![],
                },
            );
            return;
        }

        match a.buff.as_deref() {
            Some("bless") => {
                let t: Vec<EntityId> = targets
                    .iter()
                    .copied()
                    .take(spell.max_targets(slot) as usize)
                    .collect();
                let cells: Vec<Cell> = t.iter().map(|id| self.state.creatures[id].pos).collect();
                self.add_effect(
                    caster,
                    &spell.id,
                    EffectKind::Bless { targets: t },
                    rounds,
                    spell.concentration,
                );
                self.emit(
                    Audience::All,
                    Event::SpellVisual {
                        caster,
                        name: spell.name.clone(),
                        vfx: Vfx::Buff,
                        from: c.pos,
                        targets: cells,
                        area: vec![],
                    },
                );
                return;
            }
            Some("mage_armor") => {
                let t = targets.first().copied().unwrap_or(caster);
                self.add_effect(
                    caster,
                    &spell.id,
                    EffectKind::MageArmor { target: t },
                    rounds,
                    false,
                );
                let pos = self.state.creatures[&t].pos;
                self.emit(
                    Audience::All,
                    Event::SpellVisual {
                        caster,
                        name: spell.name.clone(),
                        vfx: Vfx::Force,
                        from: c.pos,
                        targets: vec![pos],
                        area: vec![],
                    },
                );
                return;
            }
            _ => {}
        }

        // Healing.
        if a.healing.is_some() {
            let dice = spell
                .healing_dice(slot.max(spell.level), caster_level)
                .unwrap_or_default();
            let add = if a.healing.as_ref().is_some_and(|h| h.add_mod) {
                self.spell_mod(c)
            } else {
                0
            };
            // Disciple of Life (Life Domain 3): +2 + slot level when a spell slot restores HP.
            let disciple = if slot > 0 && c.has_feature("Disciple of Life") {
                2 + slot as i32
            } else {
                0
            };
            let mut cells = Vec::new();
            for t in targets.iter().take(spell.max_targets(slot) as usize) {
                let roll = dice.roll(self.rng.as_mut(), false);
                let amount = (roll.total + add + disciple).max(0);
                let name = self.state.creatures[t].name.clone();
                self.log(
                    Audience::All,
                    format!(
                        "{name} regains {amount} HP ({} {add:+}{}).",
                        roll.breakdown(),
                        if disciple > 0 {
                            format!(" +{disciple} Disciple of Life")
                        } else {
                            String::new()
                        }
                    ),
                );
                cells.push(self.state.creatures[t].pos);
                self.heal(*t, amount);
            }
            self.emit(
                Audience::All,
                Event::SpellVisual {
                    caster,
                    name: spell.name.clone(),
                    vfx: Vfx::Healing,
                    from: c.pos,
                    targets: cells,
                    area: vec![],
                },
            );
            return;
        }

        // Magic Missile: darts hit automatically.
        if a.auto_hit {
            let darts = spell.projectiles(slot.max(1), caster_level) as usize;
            let dice = DiceExpr::parse(
                &a.damage
                    .as_ref()
                    .map(|d| d.dice.clone())
                    .unwrap_or_default(),
            )
            .unwrap_or_default();
            let mut cells = Vec::new();
            for i in 0..darts {
                let Some(t) = targets.get(i % targets.len().max(1)).copied() else {
                    break;
                };
                let tc = self.state.creatures[&t].clone();
                let roll = dice.roll(self.rng.as_mut(), false);
                let o = self
                    .defenses_of(&tc)
                    .apply(DamageType::Force, roll.total, 0);
                cells.push(tc.pos);
                self.emit(
                    Audience::All,
                    Event::Floating {
                        entity: t,
                        text: format!("-{}", o.final_amount),
                        kind: FloatKind::Damage,
                    },
                );
                self.log(
                    Audience::All,
                    format!(
                        "Dart {} hits {}: {} → {}",
                        i + 1,
                        tc.name,
                        roll.breakdown(),
                        o.final_amount
                    ),
                );
                self.apply_damage_from(t, o.final_amount, false, Some(caster));
            }
            self.emit(
                Audience::All,
                Event::SpellVisual {
                    caster,
                    name: spell.name.clone(),
                    vfx: Vfx::Force,
                    from: c.pos,
                    targets: cells,
                    area: vec![],
                },
            );
            return;
        }

        // Spell attacks (one roll per projectile).
        if let Some(kind) = a.attack {
            let dmg = a
                .damage
                .as_ref()
                .map(|d| d.damage_type)
                .unwrap_or(DamageType::Force);
            let dice = spell
                .damage_dice(slot.max(spell.level), caster_level)
                .unwrap_or_default();
            let n = spell.projectiles(slot.max(spell.level), caster_level) as usize;
            let lights = self.light_levels(c.room);
            let mut cells = Vec::new();
            for i in 0..n {
                let Some(t) = targets.get(i % targets.len().max(1)).copied() else {
                    break;
                };
                let tc = self.state.creatures[&t].clone();
                if !tc.is_alive() {
                    continue;
                }
                cells.push(tc.pos);
                let dist = grid_distance_ft(c.pos, tc.pos);
                let mut sources = ModeSources::default();
                let situation = AttackSituation {
                    attacker_within_5ft: dist <= 5,
                    attacker_can_see_target: self.can_see(c, &tc, &lights),
                    target_can_see_attacker: self.can_see(&tc, c, &lights),
                    target_is_attackers_grappler: false,
                    attacker_fear_source_in_sight: true,
                    target_dodging: tc.effects.dodging,
                };
                attack_roll_sources(&c.conditions, &tc.conditions, situation, &mut sources);
                if kind == SpellAttackKind::Ranged && self.threatened_in_melee(c, &lights) {
                    sources.dis("ranged spell attack with an enemy within 5 ft");
                }
                if tc.status.exposed.is_some() {
                    sources.adv("target exposed");
                }
                if spell.classes.iter().any(|k| k == "Sorcerer")
                    && self.has_effect(caster, |k| matches!(k, EffectKind::InnateSorcery { .. }))
                {
                    sources.adv("Innate Sorcery");
                }
                let profile = AttackProfile {
                    name: spell.name.clone(),
                    kind: if kind == SpellAttackKind::Melee {
                        WeaponKind::Melee
                    } else {
                        WeaponKind::Ranged
                    },
                    to_hit: self.spell_attack_bonus(c),
                    damage: vec![DamagePart {
                        dice: dice.clone(),
                        damage_type: dmg,
                    }],
                    reach_ft: 5,
                    ..Default::default()
                };
                let mut mods = AttackModifiers::new();
                mods.sources = sources;
                mods.exhaustion_level = c.conditions.exhaustion;
                if self.has_bless(caster) {
                    mods.to_hit_dice
                        .push(("Bless".into(), DiceExpr::dice(1, 4)));
                }
                let (ac, defenses) = (self.effective_ac(&tc), self.defenses_of(&tc));
                let result = resolve_attack(self.rng.as_mut(), &profile, ac, &defenses, &mods);
                if tc.status.exposed.is_some() {
                    self.creature_mut(t).status.exposed = None;
                }
                self.log(Audience::All, result.log_line(&c.name, &tc.name));
                let round = self.state.combat.as_ref().map(|cb| cb.round).unwrap_or(0);
                if result.hit {
                    if a.grants_advantage_next_attack {
                        self.creature_mut(t).status.exposed = Some((caster, round));
                    }
                    if let Some(ft) = a.speed_reduction_ft {
                        self.creature_mut(t).status.speed_penalty = Some((caster, ft as i32));
                    }
                    if a.no_reactions {
                        self.creature_mut(t).status.no_reactions = true;
                    }
                    if a.no_healing {
                        self.creature_mut(t).status.no_healing_from = Some((caster, round));
                    }
                    let options = self.hit_reaction_options(c, &tc, &result);
                    if options.contains(&HitReaction::Shield) {
                        // Shield against a spell attack.
                        self.state.combat.as_mut().expect("in combat").pending_hit =
                            Some(PendingHit {
                                attacker: caster,
                                target: t,
                                result,
                                ranged: true,
                                melee_weapon_hit: false,
                                options: vec![HitReaction::Shield],
                                vfx: vfx_for_damage(dmg),
                            });
                        self.emit(
                            Audience::All,
                            Event::SpellVisual {
                                caster,
                                name: spell.name.clone(),
                                vfx: vfx_for_damage(dmg),
                                from: c.pos,
                                targets: cells.clone(),
                                area: vec![],
                            },
                        );
                        self.emit(
                            Audience::All,
                            Event::ReactionPrompt {
                                reactor: t,
                                mover: caster,
                            },
                        );
                        return;
                    }
                    self.emit(
                        Audience::All,
                        Event::Floating {
                            entity: t,
                            text: if result.critical {
                                format!("-{} CRIT!", result.total_damage)
                            } else {
                                format!("-{}", result.total_damage)
                            },
                            kind: if result.critical {
                                FloatKind::CriticalDamage
                            } else {
                                FloatKind::Damage
                            },
                        },
                    );
                    self.apply_damage_from(t, result.total_damage, result.critical, Some(caster));
                } else if spell.is_cantrip() && c.has_feature("Potent Cantrip") {
                    // Potent Cantrip (Evoker 3): half damage on a miss, no other effect.
                    let half = dice.roll(self.rng.as_mut(), false).total / 2;
                    let o = self.defenses_of(&tc).apply(dmg, half, 0);
                    self.log(
                        Audience::All,
                        format!(
                            "Potent Cantrip: {} takes {} anyway.",
                            tc.name, o.final_amount
                        ),
                    );
                    self.apply_damage_from(t, o.final_amount, false, Some(caster));
                } else {
                    self.emit(
                        Audience::All,
                        Event::Floating {
                            entity: t,
                            text: "Miss".into(),
                            kind: FloatKind::Miss,
                        },
                    );
                }
                // Ice Knife: the shard explodes, hit or miss.
                if let Some(sec) = &a.secondary {
                    let burst =
                        area_cells(AreaShape::Sphere, sec.area.size_ft, c.pos, tc.pos, true);
                    let sdice = DiceExpr::parse(&sec.damage.dice).unwrap_or_default();
                    let extra = slot.saturating_sub(spell.level) as u32;
                    let mut sdice = sdice;
                    if let Some(t0) = sdice.terms.first_mut() {
                        t0.count += extra;
                    }
                    self.area_save_damage(
                        c,
                        spell,
                        sec.save,
                        &sdice,
                        sec.damage.damage_type,
                        false,
                        &burst,
                        None,
                        false,
                    );
                }
            }
            self.emit(
                Audience::All,
                Event::SpellVisual {
                    caster,
                    name: spell.name.clone(),
                    vfx: vfx_for_damage(dmg),
                    from: c.pos,
                    targets: cells,
                    area: vec![],
                },
            );
            return;
        }

        // Saving-throw spells: single/multiple targets or an area.
        if let Some(save) = a.save {
            let dmg_type = a.damage.as_ref().map(|d| d.damage_type);
            let dice = spell.damage_dice(slot.max(spell.level), caster_level);
            let area: Vec<Cell> = match a.area {
                Some(ar) => {
                    let self_origin =
                        matches!(spell.range(), dnd_rules::spells::SpellRange::SelfOnly);
                    let p = point.unwrap_or(c.pos);
                    area_cells(ar.shape, ar.size_ft, c.pos, p, !self_origin)
                }
                None => vec![],
            };
            let affected_targets = if area.is_empty() {
                Some(targets.clone())
            } else {
                None
            };
            let cells = if area.is_empty() {
                targets
                    .iter()
                    .map(|t| self.state.creatures[t].pos)
                    .collect()
            } else {
                vec![point.unwrap_or(c.pos)]
            };
            self.emit(
                Audience::All,
                Event::SpellVisual {
                    caster,
                    name: spell.name.clone(),
                    vfx: dmg_type.map(vfx_for_damage).unwrap_or(Vfx::Arcane),
                    from: c.pos,
                    targets: cells,
                    area: area.clone(),
                },
            );
            let dice = dice.unwrap_or_default();
            self.area_save_damage(
                c,
                spell,
                save,
                &dice,
                dmg_type.unwrap_or(DamageType::Force),
                a.half_on_success,
                &area,
                affected_targets,
                true,
            );
            return;
        }

        // Anything else: the DM adjudicates the text; the slot/action/concentration were handled.
        let level_note = if spell.auto.level == dnd_rules::spells::AutoLevel::Manual {
            "effect adjudicated by the DM"
        } else {
            "partially automated"
        };
        self.log(
            Audience::All,
            format!(
                "{}: {level_note} — \"{}\"",
                spell.name,
                spell.description.chars().take(220).collect::<String>()
            ),
        );
        self.emit(
            Audience::All,
            Event::SpellVisual {
                caster,
                name: spell.name.clone(),
                vfx,
                from: c.pos,
                targets: point.into_iter().collect(),
                area: vec![],
            },
        );
    }

    /// Damage/conditions for every creature in `area` (or the listed targets) that fails a save.
    /// Damage is rolled once for all targets ("Damage against Multiple Targets").
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn area_save_damage(
        &mut self,
        c: &Creature,
        spell: &Spell,
        save: Ability,
        dice: &DiceExpr,
        dtype: DamageType,
        half: bool,
        area: &[Cell],
        targets: Option<Vec<EntityId>>,
        apply_spell_extras: bool,
    ) {
        let dc = self.spell_dc(c, spell);
        let map = self.state.rooms[&c.room].map.clone();
        let origin = c.pos;
        let affected: Vec<EntityId> = match targets {
            Some(t) => t,
            None => self
                .state
                .creatures
                .values()
                .filter(|t| t.room == c.room && t.is_alive() && area.contains(&t.pos))
                // Total Cover: the effect needs a clear line from its origin.
                .filter(|t| {
                    area.iter().any(|_| {
                        line_of_sight(
                            &map,
                            if matches!(spell.range(), dnd_rules::spells::SpellRange::SelfOnly) {
                                origin
                            } else {
                                area[area.len() / 2]
                            },
                            t.pos,
                        )
                    })
                })
                .map(|t| t.id)
                .collect(),
        };
        let roll = (!dice.terms.is_empty() || dice.modifier != 0)
            .then(|| dice.roll(self.rng.as_mut(), false));
        if let Some(r) = &roll {
            self.log(
                Audience::All,
                format!("{} damage: {}", spell.name, r.breakdown()),
            );
        }
        if c.status.raging {
            self.creature_mut(c.id).status.rage_extended = true;
        }
        for t in affected {
            let tc = self.state.creatures[&t].clone();
            let (ok, _) = self.saving_throw(t, save, dc, &spell.name);
            if let Some(r) = &roll {
                let base = if ok {
                    if half {
                        r.total / 2
                    } else if spell.is_cantrip() && c.has_feature("Potent Cantrip") {
                        r.total / 2
                    } else {
                        0
                    }
                } else {
                    r.total
                };
                if base > 0 {
                    let o = self.defenses_of(&tc).apply(dtype, base, 0);
                    self.emit(
                        Audience::All,
                        Event::Floating {
                            entity: t,
                            text: if o.immune {
                                "Immune".into()
                            } else {
                                format!("-{}", o.final_amount)
                            },
                            kind: if o.resisted {
                                FloatKind::Resisted
                            } else {
                                FloatKind::Damage
                            },
                        },
                    );
                    self.apply_damage_from(t, o.final_amount, false, Some(c.id));
                } else if ok {
                    self.emit(
                        Audience::All,
                        Event::Floating {
                            entity: t,
                            text: "Saved".into(),
                            kind: FloatKind::Miss,
                        },
                    );
                }
            }
            if !ok && apply_spell_extras {
                if let Some(cond) = spell.auto.condition
                    && self.state.creatures[&t].is_alive()
                    && !self
                        .state
                        .creatures
                        .get(&t)
                        .is_some_and(|x| x.creature_type.is_empty())
                {
                    let rounds = spell.duration_rounds();
                    self.add_effect(
                        c.id,
                        &spell.id,
                        EffectKind::Condition {
                            target: t,
                            condition: cond,
                            save: Some((save, dc)),
                            repeat_save: spell.auto.repeat_save,
                            ends_at_source_turn: false,
                        },
                        rounds,
                        spell.concentration,
                    );
                    self.emit(
                        Audience::All,
                        Event::Floating {
                            entity: t,
                            text: format!("{cond:?}"),
                            kind: FloatKind::Condition,
                        },
                    );
                }
                if let Some(ft) = spell.auto.push_ft {
                    self.push_away(c.id, t, ft as i32);
                }
            }
        }
    }

    /// A monster's save-based action (breath weapon etc.). Recharge abilities must be recharged.
    pub(crate) fn cmd_monster_action(
        &mut self,
        who: Principal,
        monster: EntityId,
        action: usize,
        targets: Vec<EntityId>,
        point: Option<Cell>,
    ) -> Res<()> {
        Self::require_dm(who)?;
        let m = self.creature(monster)?.clone();
        let act = m
            .save_actions
            .get(action)
            .cloned()
            .ok_or_else(|| CommandError::NotFound("action".into()))?;
        if m.resources.recharging.contains(&action) {
            return rules(format!("{} hasn't recharged yet", act.name));
        }
        if let Some(cb) = self.state.combat.as_ref().filter(|cb| cb.room == m.room) {
            if cb.active() != monster {
                return rules("it isn't this creature's turn");
            }
            if cb.turn.action_used {
                return rules("action already used this turn");
            }
        }
        let area: Vec<Cell> = match (act.area, point) {
            (Some(ar), Some(p)) => area_cells(
                ar.shape,
                ar.size_ft,
                m.pos,
                p,
                !matches!(
                    ar.shape,
                    AreaShape::Cone | AreaShape::Line | AreaShape::Emanation
                ),
            ),
            (Some(_), None) => return rules(format!("choose a point/direction for {}", act.name)),
            (None, _) if targets.is_empty() => return rules("choose targets"),
            _ => vec![],
        };
        if let Some(cb) = self.state.combat.as_mut().filter(|cb| cb.room == m.room) {
            cb.turn.action_used = true;
        }
        if act.recharge.is_some() {
            self.creature_mut(monster).resources.recharging.push(action);
        }
        let dtype = act
            .damage
            .first()
            .map(|d| d.damage_type)
            .unwrap_or(DamageType::Force);
        let mut dice = DiceExpr::default();
        for d in &act.damage {
            for t in &d.dice.terms {
                dice.terms.push(*t);
            }
            dice.modifier += d.dice.modifier;
        }
        self.log(
            Audience::All,
            format!(
                "{} uses {} (DC {} {:?}).",
                m.name, act.name, act.dc, act.ability
            ),
        );
        let cells = if area.is_empty() {
            targets
                .iter()
                .filter_map(|t| self.state.creatures.get(t).map(|c| c.pos))
                .collect()
        } else {
            vec![point.unwrap_or(m.pos)]
        };
        self.emit(
            Audience::All,
            Event::SpellVisual {
                caster: monster,
                name: act.name.clone(),
                vfx: vfx_for_damage(dtype),
                from: m.pos,
                targets: cells,
                area: area.clone(),
            },
        );
        // Reuse the save machinery with a synthetic "spell" carrying the DC.
        let pseudo = Spell {
            id: format!("monster_{}", act.name.to_lowercase()),
            name: act.name.clone(),
            level: 1,
            school: String::new(),
            classes: vec![],
            casting_time: "Action".into(),
            range: "Self".into(),
            components: String::new(),
            duration: "Instantaneous".into(),
            ritual: false,
            concentration: false,
            verbal: false,
            somatic: false,
            material: None,
            description: act.text.clone(),
            auto: dnd_rules::spells::SpellAuto {
                condition: act.conditions.first().copied(),
                ..Default::default()
            },
        };
        let mc = m.clone();
        let targets_opt = if area.is_empty() { Some(targets) } else { None };
        self.area_save_damage_dc(
            &mc,
            &pseudo,
            act.ability,
            act.dc,
            &dice,
            dtype,
            act.half_on_success,
            &area,
            targets_opt,
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn area_save_damage_dc(
        &mut self,
        c: &Creature,
        spell: &Spell,
        save: Ability,
        dc: i32,
        dice: &DiceExpr,
        dtype: DamageType,
        half: bool,
        area: &[Cell],
        targets: Option<Vec<EntityId>>,
    ) {
        let affected: Vec<EntityId> = match targets {
            Some(t) => t,
            None => self
                .state
                .creatures
                .values()
                .filter(|t| {
                    t.room == c.room && t.is_alive() && t.id != c.id && area.contains(&t.pos)
                })
                .map(|t| t.id)
                .collect(),
        };
        let roll = dice.roll(self.rng.as_mut(), false);
        self.log(
            Audience::All,
            format!("{} damage: {}", spell.name, roll.breakdown()),
        );
        for t in affected {
            let tc = self.state.creatures[&t].clone();
            let (ok, _) = self.saving_throw(t, save, dc, &spell.name);
            let base = if ok {
                if half { roll.total / 2 } else { 0 }
            } else {
                roll.total
            };
            if base > 0 {
                let o = self.defenses_of(&tc).apply(dtype, base, 0);
                self.emit(
                    Audience::All,
                    Event::Floating {
                        entity: t,
                        text: format!("-{}", o.final_amount),
                        kind: FloatKind::Damage,
                    },
                );
                self.apply_damage_from(t, o.final_amount, false, Some(c.id));
            }
            if !ok && let Some(cond) = spell.auto.condition {
                self.creature_mut(t).conditions.add(cond);
                self.emit(
                    Audience::All,
                    Event::Floating {
                        entity: t,
                        text: format!("{cond:?}"),
                        kind: FloatKind::Condition,
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_cone_line_cube_shapes() {
        // 20-ft-radius Sphere on a grid: 9×9 square of cells (distance counted in squares).
        assert_eq!(
            area_cells(AreaShape::Sphere, 20, (0, 0), (10, 10), true).len(),
            81
        );
        // 15-ft Cone pointing east from (0,0): includes (1,0), (3,0), (3,1); excludes (1,1)... (width = distance).
        let cone = area_cells(AreaShape::Cone, 15, (0, 0), (5, 0), false);
        assert!(cone.contains(&(1, 0)) && cone.contains(&(3, 0)) && cone.contains(&(3, 1)));
        assert!(!cone.contains(&(0, 1)) && !cone.contains(&(-1, 0)) && !cone.contains(&(4, 0)));
        // 100-ft Line east: 20 cells in a row.
        let line = area_cells(AreaShape::Line, 100, (0, 0), (5, 0), false);
        assert_eq!(line.len(), 20);
        assert!(line.iter().all(|c| c.1 == 0));
        // 15-ft Cube originating from the caster toward the east: 3×3 cells starting next to the caster.
        let cube = area_cells(AreaShape::Cube, 15, (0, 0), (5, 0), false);
        assert_eq!(cube.len(), 9);
        assert!(cube.contains(&(1, 0)) && cube.contains(&(3, 1)) && !cube.contains(&(0, 0)));
        // Emanation excludes the origin.
        assert!(!area_cells(AreaShape::Emanation, 10, (0, 0), (0, 0), false).contains(&(0, 0)));
    }
}
