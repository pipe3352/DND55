//! Weapons and armor from the SRD 5.2.1 "Equipment" chapter (Weapons table, Armor table, Mastery Properties).

use crate::damage::DamageType;
use crate::dice::DiceExpr;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaponCategory {
    Simple,
    Martial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaponKind {
    Melee,
    Ranged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WeaponProperty {
    Ammunition,
    Finesse,
    Heavy,
    Light,
    Loading,
    Reach,
    Thrown,
    TwoHanded,
    Versatile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mastery {
    Cleave,
    Graze,
    Nick,
    Push,
    Sap,
    Slow,
    Topple,
    Vex,
}

/// Static table row (not deserializable; reference weapons by `id` in saved data).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Weapon {
    pub id: &'static str,
    pub name: &'static str,
    pub category: WeaponCategory,
    pub kind: WeaponKind,
    /// `None` for flat damage (the Blowgun deals exactly 1).
    pub dice_count: u32,
    pub dice_sides: u32,
    pub damage_type: DamageType,
    pub properties: &'static [WeaponProperty],
    /// Normal/long range in feet for Thrown or Ammunition weapons.
    pub range: Option<(u32, u32)>,
    pub versatile_sides: Option<u32>,
    pub mastery: Mastery,
}

impl Weapon {
    pub fn has(&self, p: WeaponProperty) -> bool {
        self.properties.contains(&p)
    }
    pub fn damage_dice(&self) -> DiceExpr {
        if self.dice_sides == 0 {
            DiceExpr::default().with_modifier(self.dice_count as i32)
        } else {
            DiceExpr::dice(self.dice_count, self.dice_sides)
        }
    }
    /// Melee reach in feet ("A Reach weapon adds 5 feet to your reach").
    pub fn reach_ft(&self) -> u32 {
        if self.has(WeaponProperty::Reach) { 10 } else { 5 }
    }
}

use DamageType::{Bludgeoning as B, Piercing as P, Slashing as S};
use Mastery::*;
use WeaponCategory::{Martial, Simple};
use WeaponKind::{Melee, Ranged};
use WeaponProperty::*;

macro_rules! w {
    ($id:literal, $name:literal, $cat:expr, $kind:expr, $n:literal d $s:literal, $dt:expr, [$($p:expr),*], $range:expr, $vers:expr, $m:expr) => {
        Weapon { id: $id, name: $name, category: $cat, kind: $kind, dice_count: $n, dice_sides: $s, damage_type: $dt,
                 properties: &[$($p),*], range: $range, versatile_sides: $vers, mastery: $m }
    };
}

/// The complete SRD 5.2.1 Weapons table.
pub static WEAPONS: &[Weapon] = &[
    // Simple Melee
    w!("club", "Club", Simple, Melee, 1 d 4, B, [Light], None, None, Slow),
    w!("dagger", "Dagger", Simple, Melee, 1 d 4, P, [Finesse, Light, Thrown], Some((20, 60)), None, Nick),
    w!("greatclub", "Greatclub", Simple, Melee, 1 d 8, B, [TwoHanded], None, None, Push),
    w!("handaxe", "Handaxe", Simple, Melee, 1 d 6, S, [Light, Thrown], Some((20, 60)), None, Vex),
    w!("javelin", "Javelin", Simple, Melee, 1 d 6, P, [Thrown], Some((30, 120)), None, Slow),
    w!("light_hammer", "Light Hammer", Simple, Melee, 1 d 4, B, [Light, Thrown], Some((20, 60)), None, Nick),
    w!("mace", "Mace", Simple, Melee, 1 d 6, B, [], None, None, Sap),
    w!("quarterstaff", "Quarterstaff", Simple, Melee, 1 d 6, B, [Versatile], None, Some(8), Topple),
    w!("sickle", "Sickle", Simple, Melee, 1 d 4, S, [Light], None, None, Nick),
    w!("spear", "Spear", Simple, Melee, 1 d 6, P, [Thrown, Versatile], Some((20, 60)), Some(8), Sap),
    // Simple Ranged
    w!("dart", "Dart", Simple, Ranged, 1 d 4, P, [Finesse, Thrown], Some((20, 60)), None, Vex),
    w!("light_crossbow", "Light Crossbow", Simple, Ranged, 1 d 8, P, [Ammunition, Loading, TwoHanded], Some((80, 320)), None, Slow),
    w!("shortbow", "Shortbow", Simple, Ranged, 1 d 6, P, [Ammunition, TwoHanded], Some((80, 320)), None, Vex),
    w!("sling", "Sling", Simple, Ranged, 1 d 4, B, [Ammunition], Some((30, 120)), None, Slow),
    // Martial Melee
    w!("battleaxe", "Battleaxe", Martial, Melee, 1 d 8, S, [Versatile], None, Some(10), Topple),
    w!("flail", "Flail", Martial, Melee, 1 d 8, B, [], None, None, Sap),
    w!("glaive", "Glaive", Martial, Melee, 1 d 10, S, [Heavy, Reach, TwoHanded], None, None, Graze),
    w!("greataxe", "Greataxe", Martial, Melee, 1 d 12, S, [Heavy, TwoHanded], None, None, Cleave),
    w!("greatsword", "Greatsword", Martial, Melee, 2 d 6, S, [Heavy, TwoHanded], None, None, Graze),
    w!("halberd", "Halberd", Martial, Melee, 1 d 10, S, [Heavy, Reach, TwoHanded], None, None, Cleave),
    w!("lance", "Lance", Martial, Melee, 1 d 10, P, [Heavy, Reach, TwoHanded], None, None, Topple),
    w!("longsword", "Longsword", Martial, Melee, 1 d 8, S, [Versatile], None, Some(10), Sap),
    w!("maul", "Maul", Martial, Melee, 2 d 6, B, [Heavy, TwoHanded], None, None, Topple),
    w!("morningstar", "Morningstar", Martial, Melee, 1 d 8, P, [], None, None, Sap),
    w!("pike", "Pike", Martial, Melee, 1 d 10, P, [Heavy, Reach, TwoHanded], None, None, Push),
    w!("rapier", "Rapier", Martial, Melee, 1 d 8, P, [Finesse], None, None, Vex),
    w!("scimitar", "Scimitar", Martial, Melee, 1 d 6, S, [Finesse, Light], None, None, Nick),
    w!("shortsword", "Shortsword", Martial, Melee, 1 d 6, P, [Finesse, Light], None, None, Vex),
    w!("trident", "Trident", Martial, Melee, 1 d 8, P, [Thrown, Versatile], Some((20, 60)), Some(10), Topple),
    w!("warhammer", "Warhammer", Martial, Melee, 1 d 8, B, [Versatile], None, Some(10), Push),
    w!("war_pick", "War Pick", Martial, Melee, 1 d 8, P, [Versatile], None, Some(10), Sap),
    w!("whip", "Whip", Martial, Melee, 1 d 4, S, [Finesse, Reach], None, None, Slow),
    // Martial Ranged
    w!("blowgun", "Blowgun", Martial, Ranged, 1 d 0, P, [Ammunition, Loading], Some((25, 100)), None, Vex),
    w!("hand_crossbow", "Hand Crossbow", Martial, Ranged, 1 d 6, P, [Ammunition, Light, Loading], Some((30, 120)), None, Vex),
    w!("heavy_crossbow", "Heavy Crossbow", Martial, Ranged, 1 d 10, P, [Ammunition, Heavy, Loading, TwoHanded], Some((100, 400)), None, Push),
    w!("longbow", "Longbow", Martial, Ranged, 1 d 8, P, [Ammunition, Heavy, TwoHanded], Some((150, 600)), None, Slow),
    w!("musket", "Musket", Martial, Ranged, 1 d 12, P, [Ammunition, Loading, TwoHanded], Some((40, 120)), None, Slow),
];

pub fn weapon(id: &str) -> Option<&'static Weapon> {
    WEAPONS.iter().find(|w| w.id == id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArmorCategory {
    Light,
    Medium,
    Heavy,
    Shield,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Armor {
    pub id: &'static str,
    pub name: &'static str,
    pub category: ArmorCategory,
    pub base_ac: i32,
    /// `None` = no Dex; `Some(None)` = full Dex; `Some(Some(2))` = Dex max 2.
    pub dex: Option<Option<i32>>,
    pub strength_req: Option<u8>,
    pub stealth_disadvantage: bool,
}

macro_rules! a {
    ($id:literal, $name:literal, $cat:expr, $ac:literal, $dex:expr, $str:expr, $st:literal) => {
        Armor { id: $id, name: $name, category: $cat, base_ac: $ac, dex: $dex, strength_req: $str, stealth_disadvantage: $st }
    };
}

/// The SRD 5.2.1 Armor table.
pub static ARMOR: &[Armor] = &[
    a!("padded", "Padded Armor", ArmorCategory::Light, 11, Some(None), None, true),
    a!("leather", "Leather Armor", ArmorCategory::Light, 11, Some(None), None, false),
    a!("studded_leather", "Studded Leather Armor", ArmorCategory::Light, 12, Some(None), None, false),
    a!("hide", "Hide Armor", ArmorCategory::Medium, 12, Some(Some(2)), None, false),
    a!("chain_shirt", "Chain Shirt", ArmorCategory::Medium, 13, Some(Some(2)), None, false),
    a!("scale_mail", "Scale Mail", ArmorCategory::Medium, 14, Some(Some(2)), None, true),
    a!("breastplate", "Breastplate", ArmorCategory::Medium, 14, Some(Some(2)), None, false),
    a!("half_plate", "Half Plate Armor", ArmorCategory::Medium, 15, Some(Some(2)), None, true),
    a!("ring_mail", "Ring Mail", ArmorCategory::Heavy, 14, None, None, true),
    a!("chain_mail", "Chain Mail", ArmorCategory::Heavy, 16, None, Some(13), true),
    a!("splint", "Splint Armor", ArmorCategory::Heavy, 17, None, Some(15), true),
    a!("plate", "Plate Armor", ArmorCategory::Heavy, 18, None, Some(15), true),
    a!("shield", "Shield", ArmorCategory::Shield, 2, None, None, false),
];

pub fn armor(id: &str) -> Option<&'static Armor> {
    ARMOR.iter().find(|a| a.id == id)
}

/// Armor Class from worn body armor and shield. Without armor: 10 + Dex modifier.
pub fn armor_class(body: Option<&Armor>, shield: bool, dex_mod: i32) -> i32 {
    let base = match body {
        None => 10 + dex_mod,
        Some(a) => match a.dex {
            None => a.base_ac,
            Some(None) => a.base_ac + dex_mod,
            Some(Some(max)) => a.base_ac + dex_mod.min(max),
        },
    };
    base + if shield { 2 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_table_spot_checks() {
        let ls = weapon("longsword").unwrap();
        assert_eq!((ls.dice_count, ls.dice_sides, ls.versatile_sides, ls.mastery), (1, 8, Some(10), Mastery::Sap));
        let gs = weapon("greatsword").unwrap();
        assert_eq!((gs.dice_count, gs.dice_sides, gs.mastery), (2, 6, Mastery::Graze));
        assert_eq!(weapon("dagger").unwrap().mastery, Mastery::Nick);
        assert_eq!(weapon("longbow").unwrap().range, Some((150, 600)));
        assert_eq!(weapon("glaive").unwrap().reach_ft(), 10);
        assert_eq!(WEAPONS.len(), 37); // 10 simple melee, 4 simple ranged, 18 martial melee, 5 martial ranged
        assert_eq!(weapon("blowgun").unwrap().damage_dice().to_string(), "1");
    }

    #[test]
    fn srd_armor_examples() {
        // SRD: "if you wear Leather Armor and have a Dexterity modifier of +3, your AC is 14 ... 16 in Chain Mail."
        assert_eq!(armor_class(armor("leather"), false, 3), 14);
        assert_eq!(armor_class(armor("chain_mail"), false, 3), 16);
        assert_eq!(armor_class(armor("chain_shirt"), true, 3), 17);
        assert_eq!(armor_class(None, false, 2), 12);
    }
}
