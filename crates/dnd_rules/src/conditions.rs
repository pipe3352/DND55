//! The 15 conditions of the 2024 rules and their mechanical effects on attacks, saves and speed.
//! (SRD 5.2.1 "Rules Glossary": Blinded, Charmed, Deafened, Exhaustion, Frightened, Grappled,
//! Incapacitated, Invisible, Paralyzed, Petrified, Poisoned, Prone, Restrained, Stunned, Unconscious)

use crate::ability::Ability;
use crate::d20::ModeSources;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Condition {
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
}

/// The conditions a creature currently has. Exhaustion is tracked as a level (0–6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionSet {
    pub conditions: BTreeSet<Condition>,
    pub exhaustion: u8,
}

impl ConditionSet {
    pub fn add(&mut self, c: Condition) {
        self.conditions.insert(c);
    }
    pub fn remove(&mut self, c: Condition) {
        self.conditions.remove(&c);
    }

    /// Direct or implied presence (Paralyzed, Petrified, Stunned and Unconscious include Incapacitated;
    /// Unconscious includes Prone).
    pub fn has(&self, c: Condition) -> bool {
        use Condition::*;
        if self.conditions.contains(&c) {
            return true;
        }
        match c {
            Incapacitated => [Paralyzed, Petrified, Stunned, Unconscious]
                .iter()
                .any(|x| self.conditions.contains(x)),
            Prone => self.conditions.contains(&Unconscious),
            _ => false,
        }
    }

    /// Exhaustion level 6 kills.
    pub fn add_exhaustion(&mut self) -> bool {
        self.exhaustion = (self.exhaustion + 1).min(6);
        self.exhaustion >= 6
    }

    /// Finishing a Long Rest removes 1 Exhaustion level.
    pub fn long_rest(&mut self) {
        self.exhaustion = self.exhaustion.saturating_sub(1);
    }

    /// Speed after conditions. Grappled/Restrained/Paralyzed/Petrified/Unconscious: Speed 0.
    /// Exhaustion: reduced by 5 ft per level.
    pub fn effective_speed(&self, base_ft: i32) -> i32 {
        use Condition::*;
        if [Grappled, Restrained, Paralyzed, Petrified, Unconscious]
            .iter()
            .any(|c| self.has(*c))
        {
            return 0;
        }
        (base_ft - 5 * self.exhaustion as i32).max(0)
    }

    /// Paralyzed, Petrified, Stunned, Unconscious: automatically fail Strength and Dexterity saving throws.
    pub fn auto_fails_save(&self, a: Ability) -> bool {
        use Condition::*;
        matches!(a, Ability::Strength | Ability::Dexterity)
            && [Paralyzed, Petrified, Stunned, Unconscious]
                .iter()
                .any(|c| self.has(*c))
    }

    /// Incapacitated: can't take any action, Bonus Action, or Reaction.
    pub fn can_act(&self) -> bool {
        !self.has(Condition::Incapacitated)
    }
}

/// Situational facts about an attack that conditions depend on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttackSituation {
    pub attacker_within_5ft: bool,
    /// Whether the attacker can see the target (false when the target is Invisible and the attacker lacks a way to see it, etc).
    pub attacker_can_see_target: bool,
    pub target_can_see_attacker: bool,
    /// For Grappled: the target is the creature grappling the attacker.
    pub target_is_attackers_grappler: bool,
    /// For Frightened: the source of fear is within the attacker's line of sight.
    pub attacker_fear_source_in_sight: bool,
    /// Target took the Dodge action and it is still in effect.
    pub target_dodging: bool,
}

/// Collects advantage/disadvantage sources for an attack roll from both creatures' conditions.
pub fn attack_roll_sources(
    attacker: &ConditionSet,
    target: &ConditionSet,
    s: AttackSituation,
    sources: &mut ModeSources,
) {
    use Condition::*;
    // Attacker's own conditions.
    if attacker.has(Blinded) {
        sources.dis("attacker Blinded");
    }
    if attacker.has(Poisoned) {
        sources.dis("attacker Poisoned");
    }
    if attacker.has(Prone) {
        sources.dis("attacker Prone");
    }
    if attacker.has(Restrained) {
        sources.dis("attacker Restrained");
    }
    if attacker.has(Frightened) && s.attacker_fear_source_in_sight {
        sources.dis("attacker Frightened");
    }
    if attacker.has(Grappled) && !s.target_is_attackers_grappler {
        sources.dis("attacker Grappled (target isn't the grappler)");
    }
    // "Unseen Attackers and Targets": attacking a target you can't see -> Disadvantage;
    // a target that can't see you -> Advantage. Invisible/Blinded/Darkness feed these two facts.
    if !s.target_can_see_attacker {
        sources.adv("attacker unseen by target");
    }
    if !s.attacker_can_see_target {
        sources.dis("target unseen by attacker");
    }

    // Target's conditions.
    if target.has(Blinded) {
        sources.adv("target Blinded");
    }
    if target.has(Paralyzed)
        || target.has(Petrified)
        || target.has(Stunned)
        || target.has(Unconscious)
        || target.has(Restrained)
    {
        sources.adv("target Paralyzed/Petrified/Stunned/Unconscious/Restrained");
    }
    if target.has(Prone) {
        if s.attacker_within_5ft {
            sources.adv("target Prone (within 5 ft)");
        } else {
            sources.dis("target Prone (beyond 5 ft)");
        }
    }
    if s.target_dodging && s.target_can_see_attacker && !target.has(Incapacitated) {
        sources.dis("target Dodging");
    }
}

/// Paralyzed and Unconscious: any attack roll that hits is a Critical Hit if the attacker is within 5 feet.
pub fn hit_becomes_critical(target: &ConditionSet, attacker_within_5ft: bool) -> bool {
    attacker_within_5ft && (target.has(Condition::Paralyzed) || target.has(Condition::Unconscious))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::d20::RollMode;

    fn set(cs: &[Condition]) -> ConditionSet {
        ConditionSet {
            conditions: cs.iter().copied().collect(),
            exhaustion: 0,
        }
    }
    fn sit() -> AttackSituation {
        AttackSituation {
            attacker_within_5ft: true,
            attacker_can_see_target: true,
            target_can_see_attacker: true,
            ..Default::default()
        }
    }
    fn mode(a: &ConditionSet, t: &ConditionSet, s: AttackSituation) -> RollMode {
        let mut m = ModeSources::default();
        attack_roll_sources(a, t, s, &mut m);
        m.mode()
    }

    #[test]
    fn unconscious_implies_incapacitated_and_prone() {
        let c = set(&[Condition::Unconscious]);
        assert!(c.has(Condition::Incapacitated) && c.has(Condition::Prone));
        assert!(!c.can_act());
        assert!(c.auto_fails_save(Ability::Dexterity));
        assert!(!c.auto_fails_save(Ability::Wisdom));
    }

    #[test]
    fn prone_depends_on_distance() {
        let none = set(&[]);
        let prone = set(&[Condition::Prone]);
        assert_eq!(mode(&none, &prone, sit()), RollMode::Advantage);
        let far = AttackSituation {
            attacker_within_5ft: false,
            ..sit()
        };
        assert_eq!(mode(&none, &prone, far), RollMode::Disadvantage);
    }

    #[test]
    fn poisoned_attacker_vs_restrained_target_cancels() {
        assert_eq!(
            mode(
                &set(&[Condition::Poisoned]),
                &set(&[Condition::Restrained]),
                sit()
            ),
            RollMode::Normal
        );
    }

    #[test]
    fn invisible_rules() {
        let none = set(&[]);
        let inv = set(&[Condition::Invisible]);
        let cant_see = AttackSituation {
            attacker_can_see_target: false,
            ..sit()
        };
        assert_eq!(mode(&none, &inv, cant_see), RollMode::Disadvantage);
        // If the attacker can somehow see the invisible target, no benefit.
        assert_eq!(mode(&none, &inv, sit()), RollMode::Normal);
        let unseen_attacker = AttackSituation {
            target_can_see_attacker: false,
            ..sit()
        };
        assert_eq!(mode(&inv, &none, unseen_attacker), RollMode::Advantage);
    }

    #[test]
    fn grappled_only_penalizes_attacks_on_others() {
        let g = set(&[Condition::Grappled]);
        let none = set(&[]);
        assert_eq!(mode(&g, &none, sit()), RollMode::Disadvantage);
        let vs_grappler = AttackSituation {
            target_is_attackers_grappler: true,
            ..sit()
        };
        assert_eq!(mode(&g, &none, vs_grappler), RollMode::Normal);
    }

    #[test]
    fn auto_crit_within_5ft_only() {
        let p = set(&[Condition::Paralyzed]);
        assert!(hit_becomes_critical(&p, true));
        assert!(!hit_becomes_critical(&p, false));
        assert!(!hit_becomes_critical(&set(&[Condition::Stunned]), true));
    }

    #[test]
    fn exhaustion_speed_and_death() {
        let mut c = set(&[]);
        for _ in 0..3 {
            assert!(!c.add_exhaustion());
        }
        assert_eq!(c.effective_speed(30), 15);
        c.long_rest();
        assert_eq!(c.exhaustion, 2);
        for _ in 0..3 {
            c.add_exhaustion();
        }
        assert!(c.add_exhaustion(), "level 6 is death");
        assert_eq!(set(&[Condition::Grappled]).effective_speed(30), 0);
    }

    #[test]
    fn dodge_requires_seeing_the_attacker() {
        let none = set(&[]);
        let s = AttackSituation {
            target_dodging: true,
            ..sit()
        };
        assert_eq!(mode(&none, &none, s), RollMode::Disadvantage);
        // Dodge doesn't apply against an attacker the target can't see — and that unseen attacker has Advantage.
        let s = AttackSituation {
            target_dodging: true,
            target_can_see_attacker: false,
            ..sit()
        };
        assert_eq!(mode(&none, &none, s), RollMode::Advantage);
    }
}
