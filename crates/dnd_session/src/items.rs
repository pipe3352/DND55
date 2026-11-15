//! Inventory, equipment changes, potions, containers and loot.

use crate::event::{Event, Vfx};
use crate::model::*;
use crate::session::{CommandError, Session};
use dnd_rules::items::{self, ItemKind};
use dnd_world::path::grid_distance_ft;
use dnd_world::{EntityId, ObjectId};

type Res<T> = Result<T, CommandError>;

fn rules<T>(s: impl Into<String>) -> Res<T> {
    Err(CommandError::Rules(s.into()))
}

impl Session {
    fn pc_inventory(&self, id: EntityId) -> Res<Vec<items::ItemStack>> {
        Ok(self
            .creature(id)?
            .sheet()
            .ok_or_else(|| CommandError::Rules("only characters carry an inventory".into()))?
            .inventory
            .clone())
    }

    pub(crate) fn cmd_use_item(
        &mut self,
        who: Principal,
        entity: EntityId,
        item: &str,
        target: Option<EntityId>,
    ) -> Res<()> {
        let c = self.creature(entity)?.clone();
        if !Self::controls(who, &c) {
            return Err(CommandError::NotAuthorized);
        }
        if !c.conditions.can_act() {
            return rules("Incapacitated");
        }
        let inv = self.pc_inventory(entity)?;
        let stack = inv
            .iter()
            .find(|i| i.id == item)
            .cloned()
            .ok_or_else(|| CommandError::NotFound(item.into()))?;
        let target = target.unwrap_or(entity);
        let t = self.creature(target)?.clone();
        match &stack.kind {
            ItemKind::HealingPotion(_) => {
                if grid_distance_ft(c.pos, t.pos) > 5 {
                    return rules("administer a potion to a creature within 5 feet");
                }
                // "Drinking a potion or administering it to another creature requires a Bonus Action."
                if let Some(cb) = self.state.combat.as_ref().filter(|cb| cb.room == c.room) {
                    if cb.active() != entity {
                        return rules("it isn't this creature's turn");
                    }
                    if cb.turn.bonus_action_used {
                        return rules("bonus action already used");
                    }
                }
                if let Some(cb) = self.state.combat.as_mut() {
                    if cb.turn.entity == entity {
                        cb.turn.bonus_action_used = true;
                    }
                }
                let dice = stack.healing_dice().unwrap_or_default();
                let r = dice.roll(self.rng.as_mut(), false);
                self.take_item(entity, item, 1);
                self.log(
                    Audience::All,
                    format!(
                        "{} uses a {} on {}: {}",
                        c.name,
                        stack.name,
                        t.name,
                        r.breakdown()
                    ),
                );
                self.heal(target, r.total);
                self.emit(
                    Audience::All,
                    Event::SpellVisual {
                        caster: entity,
                        name: stack.name.clone(),
                        vfx: Vfx::Healing,
                        from: c.pos,
                        targets: vec![t.pos],
                        area: vec![],
                    },
                );
            }
            ItemKind::LightSource { bright_ft, dim_ft } => {
                let cm = self.creature_mut(entity);
                cm.carried_light = if cm.carried_light.is_some() {
                    None
                } else {
                    Some((*bright_ft, *dim_ft))
                };
                let lit = cm.carried_light.is_some();
                self.log(
                    Audience::All,
                    format!(
                        "{} {} a {}.",
                        c.name,
                        if lit { "lights" } else { "puts out" },
                        stack.name
                    ),
                );
            }
            _ => {
                return rules(format!(
                    "{} can't be used that way; equip it or give it away",
                    stack.name
                ));
            }
        }
        Ok(())
    }

    fn take_item(&mut self, entity: EntityId, item: &str, qty: u32) -> Option<items::ItemStack> {
        let sheet = self.creature_mut(entity).sheet_mut()?;
        items::take_from(&mut sheet.inventory, item, qty)
    }

    pub(crate) fn give_item_to(&mut self, entity: EntityId, stack: items::ItemStack) {
        let name = self.state.creatures[&entity].name.clone();
        let label = format!("{}× {}", stack.quantity, stack.name);
        if let Some(sheet) = self.creature_mut(entity).sheet_mut() {
            items::add_to(&mut sheet.inventory, stack);
            self.log(Audience::All, format!("{name} receives {label}."));
        }
    }

    pub(crate) fn cmd_equip(
        &mut self,
        who: Principal,
        entity: EntityId,
        item: &str,
        equip: bool,
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
            return rules("change equipment outside combat (donning armor takes minutes)");
        }
        let mut sheet = c
            .sheet()
            .cloned()
            .ok_or_else(|| CommandError::Rules("characters only".into()))?;
        if equip {
            let stack = items::take_from(&mut sheet.inventory, item, 1)
                .ok_or_else(|| CommandError::NotFound(item.into()))?;
            match stack.kind {
                ItemKind::Weapon(w) => sheet.weapons.push(w),
                ItemKind::Armor(a) if a == "shield" => {
                    if sheet.shield {
                        return rules("already holding a shield");
                    }
                    sheet.shield = true;
                }
                ItemKind::Armor(a) => {
                    if let Some(old) = sheet.body_armor.replace(a) {
                        if let Some(it) = items::catalog_item(&old) {
                            items::add_to(&mut sheet.inventory, it);
                        }
                    }
                }
                _ => return rules("only weapons, armor and shields can be equipped"),
            }
        } else {
            // Unequip by id: weapon, body armor or "shield".
            if let Some(pos) = sheet.weapons.iter().position(|w| w == item) {
                sheet.weapons.remove(pos);
            } else if sheet.body_armor.as_deref() == Some(item) {
                sheet.body_armor = None;
            } else if item == "shield" && sheet.shield {
                sheet.shield = false;
            } else {
                return Err(CommandError::NotFound(item.into()));
            }
            if let Some(it) = items::catalog_item(item) {
                items::add_to(&mut sheet.inventory, it);
            }
        }
        let cm = self.creature_mut(entity);
        *cm.sheet_mut().expect("pc") = sheet;
        cm.refresh_from_sheet();
        self.log(
            Audience::All,
            format!(
                "{} {} {}.",
                c.name,
                if equip { "equips" } else { "stows" },
                item.replace('_', " ")
            ),
        );
        Ok(())
    }

    pub(crate) fn cmd_give(
        &mut self,
        who: Principal,
        from: EntityId,
        to: EntityId,
        item: &str,
        qty: u32,
    ) -> Res<()> {
        let a = self.creature(from)?.clone();
        let b = self.creature(to)?.clone();
        if !Self::controls(who, &a) {
            return Err(CommandError::NotAuthorized);
        }
        if b.sheet().is_none() || a.room != b.room || grid_distance_ft(a.pos, b.pos) > 5 {
            return rules("hand items to a character within 5 feet");
        }
        if qty == 0 {
            return rules("quantity must be at least 1");
        }
        let mut inv = self.pc_inventory(from)?;
        let stack = items::take_from(&mut inv, item, qty)
            .ok_or_else(|| CommandError::Rules("not enough of that item".into()))?;
        self.take_item(from, item, qty);
        self.give_item_to(to, stack);
        Ok(())
    }

    pub(crate) fn cmd_take_from_container(
        &mut self,
        who: Principal,
        entity: EntityId,
        object: ObjectId,
        item: &str,
        qty: u32,
    ) -> Res<()> {
        let c = self.creature(entity)?.clone();
        if !Self::controls(who, &c) {
            return Err(CommandError::NotAuthorized);
        }
        c.sheet()
            .ok_or_else(|| CommandError::Rules("characters only".into()))?;
        let room = self.room(c.room)?;
        let o = room
            .map
            .object(object)
            .filter(|o| !o.hidden)
            .ok_or_else(|| CommandError::NotFound("container".into()))?;
        if !o.open {
            return rules("open it first");
        }
        if !o.footprint().any(|f| grid_distance_ft(f, c.pos) <= 5) {
            return rules("move next to it first");
        }
        let mut contents = room.containers.get(&object).cloned().unwrap_or_default();
        let stack = items::take_from(&mut contents, item, qty.max(1))
            .ok_or_else(|| CommandError::Rules("that isn't in there".into()))?;
        self.state
            .rooms
            .get_mut(&c.room)
            .expect("checked")
            .containers
            .insert(object, contents);
        self.give_item_to(entity, stack);
        Ok(())
    }
}
