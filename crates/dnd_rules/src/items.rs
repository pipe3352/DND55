//! Inventory items, currency and the small SRD item catalog used for loot.

use crate::dice::DiceExpr;
use crate::equipment;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    /// References `equipment::WEAPONS` by id.
    Weapon(String),
    /// References `equipment::ARMOR` by id (including "shield").
    Armor(String),
    /// Drinkable healing potion: dice from the SRD "Potions of Healing" table.
    HealingPotion(String),
    /// Light source while carried (e.g. Torch: Bright 20 ft, Dim +20 ft).
    LightSource { bright_ft: u32, dim_ft: u32 },
    Gear,
    Treasure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
    pub quantity: u32,
    /// Value in copper pieces for one item (0 = unknown / priceless).
    pub value_cp: u32,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Currency {
    pub cp: u32,
    pub sp: u32,
    pub ep: u32,
    pub gp: u32,
    pub pp: u32,
}

impl Currency {
    /// Total value in copper pieces (1 pp = 10 gp = 20 ep = 100 sp = 1000 cp).
    pub fn total_cp(&self) -> u64 {
        self.cp as u64 + self.sp as u64 * 10 + self.ep as u64 * 50 + self.gp as u64 * 100 + self.pp as u64 * 1000
    }
}

impl ItemStack {
    pub fn healing_dice(&self) -> Option<DiceExpr> {
        match &self.kind {
            ItemKind::HealingPotion(d) => DiceExpr::parse(d).ok(),
            _ => None,
        }
    }
}

fn potion(id: &str, name: &str, dice: &str, gp: u32) -> ItemStack {
    ItemStack {
        id: id.into(),
        name: name.into(),
        kind: ItemKind::HealingPotion(dice.into()),
        quantity: 1,
        value_cp: gp * 100,
        description: format!("Bonus Action: drink it or administer it to a creature within 5 feet; it regains {dice} Hit Points."),
    }
}

/// Items the DM can place in containers or give as rewards.
pub fn catalog() -> Vec<ItemStack> {
    let mut v = vec![
        potion("potion_of_healing", "Potion of Healing", "2d4+2", 50),
        potion("potion_of_healing_greater", "Potion of Healing (greater)", "4d4+4", 0),
        potion("potion_of_healing_superior", "Potion of Healing (superior)", "8d4+8", 0),
        potion("potion_of_healing_supreme", "Potion of Healing (supreme)", "10d4+20", 0),
        ItemStack {
            id: "torch".into(),
            name: "Torch".into(),
            kind: ItemKind::LightSource { bright_ft: 20, dim_ft: 20 },
            quantity: 1,
            value_cp: 1,
            description: "Burns for 1 hour, casting Bright Light in a 20-foot radius and Dim Light for an additional 20 feet.".into(),
        },
        ItemStack { id: "rope".into(), name: "Rope (50 feet)".into(), kind: ItemKind::Gear, quantity: 1, value_cp: 100, description: String::new() },
        ItemStack { id: "rations".into(), name: "Rations (1 day)".into(), kind: ItemKind::Gear, quantity: 1, value_cp: 50, description: String::new() },
        ItemStack { id: "healers_kit".into(), name: "Healer's Kit".into(), kind: ItemKind::Gear, quantity: 1, value_cp: 500, description: "Ten uses; Utilize action: stabilize an Unconscious creature at 0 HP without a Medicine check.".into() },
        ItemStack { id: "gem".into(), name: "Gemstone".into(), kind: ItemKind::Treasure, quantity: 1, value_cp: 5000, description: String::new() },
        ItemStack { id: "golden_idol".into(), name: "Golden Idol".into(), kind: ItemKind::Treasure, quantity: 1, value_cp: 25_000, description: String::new() },
    ];
    for w in equipment::WEAPONS {
        v.push(ItemStack { id: w.id.into(), name: w.name.into(), kind: ItemKind::Weapon(w.id.into()), quantity: 1, value_cp: 0, description: String::new() });
    }
    for a in equipment::ARMOR {
        v.push(ItemStack { id: a.id.into(), name: a.name.into(), kind: ItemKind::Armor(a.id.into()), quantity: 1, value_cp: 0, description: String::new() });
    }
    v
}

pub fn catalog_item(id: &str) -> Option<ItemStack> {
    catalog().into_iter().find(|i| i.id == id)
}

/// Adds items to an inventory, merging identical stacks.
pub fn add_to(inv: &mut Vec<ItemStack>, item: ItemStack) {
    match inv.iter_mut().find(|i| i.id == item.id && i.kind == item.kind && i.name == item.name) {
        Some(s) => s.quantity += item.quantity,
        None => inv.push(item),
    }
}

/// Removes `qty` of an item; returns the removed stack or `None` if not enough.
pub fn take_from(inv: &mut Vec<ItemStack>, id: &str, qty: u32) -> Option<ItemStack> {
    let idx = inv.iter().position(|i| i.id == id && i.quantity >= qty)?;
    let mut out = inv[idx].clone();
    out.quantity = qty;
    inv[idx].quantity -= qty;
    if inv[idx].quantity == 0 {
        inv.remove(idx);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potions_match_srd_table() {
        assert_eq!(catalog_item("potion_of_healing").unwrap().healing_dice().unwrap().to_string(), "2d4+2");
        assert_eq!(catalog_item("potion_of_healing_supreme").unwrap().healing_dice().unwrap().to_string(), "10d4+20");
    }

    #[test]
    fn stacks_merge_and_split() {
        let mut inv = vec![];
        add_to(&mut inv, catalog_item("potion_of_healing").unwrap());
        add_to(&mut inv, catalog_item("potion_of_healing").unwrap());
        assert_eq!(inv[0].quantity, 2);
        assert!(take_from(&mut inv, "potion_of_healing", 3).is_none());
        assert_eq!(take_from(&mut inv, "potion_of_healing", 2).unwrap().quantity, 2);
        assert!(inv.is_empty());
        assert_eq!(Currency { gp: 1, sp: 2, ..Default::default() }.total_cp(), 120);
    }
}
