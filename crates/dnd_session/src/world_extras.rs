//! Merchants and trades, the campaign clock, prefabs and map templates, timed conditions.

use crate::command::DmCommand;
use crate::model::*;
use crate::session::{CommandError, Res, Session};
use dnd_rules::items::{self, Currency};
use dnd_world::map::{LayerKind, MapObject};
use dnd_world::{EntityId, ObjectId, RoomId};

fn rules<T>(s: impl Into<String>) -> Res<T> {
    Err(CommandError::Rules(s.into()))
}
fn invalid<T>(s: impl Into<String>) -> Res<T> {
    Err(CommandError::Invalid(s.into()))
}

/// Pays `cp` from a purse; change comes back in gp/sp/cp. False if there isn't enough.
pub fn pay(purse: &mut Currency, cp: u64) -> bool {
    let total = purse.total_cp();
    if total < cp {
        return false;
    }
    let r = total - cp;
    *purse = Currency {
        pp: 0,
        ep: 0,
        gp: (r / 100) as u32,
        sp: (r % 100 / 10) as u32,
        cp: (r % 10) as u32,
    };
    true
}
pub fn receive(purse: &mut Currency, cp: u64) {
    purse.gp += (cp / 100) as u32;
    purse.sp += (cp % 100 / 10) as u32;
    purse.cp += (cp % 10) as u32;
}
pub fn price_label(cp: u64) -> String {
    let (gp, sp, c) = (cp / 100, cp % 100 / 10, cp % 10);
    let mut v = vec![];
    if gp > 0 {
        v.push(format!("{gp} gp"));
    }
    if sp > 0 {
        v.push(format!("{sp} sp"));
    }
    if c > 0 || v.is_empty() {
        v.push(format!("{c} cp"));
    }
    v.join(" ")
}

impl Session {
    pub(crate) fn dm_extras(&mut self, d: DmCommand) -> Res<()> {
        use DmCommand::*;
        match d {
            SaveMerchant { mut merchant } => {
                merchant.name = merchant.name.trim().chars().take(60).collect();
                if merchant.name.is_empty() {
                    return invalid("give the merchant a name");
                }
                if merchant.stock.len() > 200 {
                    return invalid("too many items");
                }
                merchant.buy_rate = merchant.buy_rate.min(100);
                if merchant.id == 0 {
                    merchant.id = self.next_id();
                    self.state.merchants.push(merchant);
                } else {
                    let m = self
                        .state
                        .merchants
                        .iter_mut()
                        .find(|m| m.id == merchant.id)
                        .ok_or_else(|| CommandError::NotFound("merchant".into()))?;
                    *m = merchant;
                }
            }
            DeleteMerchant { id } => {
                self.state.merchants.retain(|m| m.id != id);
                self.state.trades.retain(|t| t.merchant != id);
            }
            ResolveTrade { id, approve } => {
                let pos = self
                    .state
                    .trades
                    .iter()
                    .position(|t| t.id == id)
                    .ok_or_else(|| CommandError::NotFound("trade".into()))?;
                let t = self.state.trades.remove(pos);
                if approve {
                    self.execute_trade(&t)?;
                } else {
                    let name = self
                        .state
                        .creatures
                        .get(&t.entity)
                        .map(|c| c.name.clone())
                        .unwrap_or_default();
                    self.log(
                        Audience::All,
                        format!("The merchant refuses {name}'s offer."),
                    );
                }
            }
            AdvanceTime { minutes } => {
                if minutes == 0 || minutes > 60 * 24 * 365 {
                    return invalid("advance between 1 minute and a year");
                }
                self.state.time.advance(minutes);
                self.log(
                    Audience::All,
                    format!("Time passes… {}", self.state.time.label()),
                );
            }
            SetTime { day, minute } => {
                if day == 0 || minute >= 1440 {
                    return invalid("day starts at 1, minute must be < 1440");
                }
                self.state.time = CampaignTime { day, minute };
                self.log(
                    Audience::All,
                    format!("It is now {}.", self.state.time.label()),
                );
            }
            SavePrefab {
                room,
                name,
                objects,
            } => {
                let rs = self.room(room)?;
                let objs: Vec<MapObject> = rs
                    .map
                    .objects
                    .iter()
                    .filter(|o| objects.contains(&o.id))
                    .cloned()
                    .collect();
                if objs.is_empty() {
                    return invalid("select some objects first");
                }
                let (x0, y0) = (
                    objs.iter().map(|o| o.x).min().unwrap_or(0),
                    objs.iter().map(|o| o.y).min().unwrap_or(0),
                );
                let objects = objs
                    .into_iter()
                    .map(|mut o| {
                        o.x -= x0;
                        o.y -= y0;
                        o.id = ObjectId(0);
                        o
                    })
                    .collect();
                let name: String = name.trim().chars().take(60).collect();
                let id = self.next_id();
                self.state.prefabs.push(Prefab {
                    id,
                    name: if name.is_empty() {
                        "Prefab".into()
                    } else {
                        name
                    },
                    objects,
                });
            }
            DeletePrefab { id } => self.state.prefabs.retain(|p| p.id != id),
            PlacePrefab { room, prefab, cell } => {
                let p = self
                    .state
                    .prefabs
                    .iter()
                    .find(|p| p.id == prefab)
                    .cloned()
                    .ok_or_else(|| CommandError::NotFound("prefab".into()))?;
                let map = &self.room(room)?.map;
                let mut placed = vec![];
                for mut o in p.objects {
                    o.x += cell.0;
                    o.y += cell.1;
                    if !o.footprint().all(|c| map.in_bounds(c)) {
                        return rules("the prefab doesn't fit there");
                    }
                    placed.push(o);
                }
                for mut o in placed {
                    o.id = ObjectId(self.next_id());
                    self.state
                        .rooms
                        .get_mut(&room)
                        .expect("checked")
                        .map
                        .objects
                        .push(o);
                }
            }
            ApplyTemplate { room, template } => self.apply_template(room, template)?,
            _ => unreachable!("dispatched by dm()"),
        }
        Ok(())
    }

    fn trade_quote(
        &self,
        entity: EntityId,
        merchant: u64,
        sell: bool,
        item: &str,
        qty: u32,
        index: Option<usize>,
    ) -> Res<(TradeRequest, String)> {
        if qty == 0 || qty > 999 {
            return invalid("quantity must be 1–999");
        }
        let m = self
            .state
            .merchants
            .iter()
            .find(|m| m.id == merchant && m.open)
            .ok_or_else(|| CommandError::NotFound("merchant".into()))?;
        let c = self.creature(entity)?;
        let sheet = c
            .sheet()
            .ok_or_else(|| CommandError::Rules("only characters can trade".into()))?;
        let (item_id, name, total) = if sell {
            if !m.buys {
                return rules(format!("{} doesn't buy items", m.name));
            }
            let it = sheet
                .inventory
                .iter()
                .find(|i| i.id == item && i.quantity >= qty)
                .ok_or_else(|| CommandError::Rules("you don't have that many".into()))?;
            let unit = it.value_cp as u64 * m.buy_rate as u64 / 100;
            if unit == 0 {
                return rules("the merchant won't pay anything for that");
            }
            if m.funds_cp.is_some_and(|f| f < unit * qty as u64) {
                return rules(format!("{} can't afford that", m.name));
            }
            (it.id.clone(), it.name.clone(), unit * qty as u64)
        } else {
            let e = index
                .and_then(|i| m.stock.get(i))
                .ok_or_else(|| CommandError::NotFound("item".into()))?;
            if e.stock.is_some_and(|s| s < qty) {
                return rules("not enough in stock");
            }
            let total = e.price_cp as u64 * qty as u64;
            if sheet.currency.total_cp() < total {
                return rules("not enough money");
            }
            (e.item.id.clone(), e.item.name.clone(), total)
        };
        Ok((
            TradeRequest {
                id: 0,
                entity,
                merchant,
                sell,
                item_id,
                quantity: qty,
                total_cp: total,
            },
            name,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn cmd_trade(
        &mut self,
        who: Principal,
        entity: EntityId,
        merchant: u64,
        sell: bool,
        item: &str,
        qty: u32,
        index: Option<usize>,
    ) -> Res<()> {
        let c = self.creature(entity)?;
        if !Self::controls(who, c) {
            return Err(CommandError::NotAuthorized);
        }
        let (mut t, name) = self.trade_quote(entity, merchant, sell, item, qty, index)?;
        let auto = self
            .state
            .merchants
            .iter()
            .find(|m| m.id == merchant)
            .is_some_and(|m| m.auto_approve);
        if auto {
            self.execute_trade(&t)
        } else {
            t.id = self.next_id();
            let who_name = self.creature(entity)?.name.clone();
            self.log(
                Audience::DmOnly,
                format!(
                    "{who_name} wants to {} {}× {name} for {} — approve in the Shop window.",
                    if sell { "sell" } else { "buy" },
                    qty,
                    price_label(t.total_cp)
                ),
            );
            self.state.trades.push(t);
            Ok(())
        }
    }

    /// Re-validates and performs a trade (prices and stock may have changed since the request).
    fn execute_trade(&mut self, t: &TradeRequest) -> Res<()> {
        let index = self
            .state
            .merchants
            .iter()
            .find(|m| m.id == t.merchant)
            .and_then(|m| m.stock.iter().position(|e| e.item.id == t.item_id));
        let (q, name) =
            self.trade_quote(t.entity, t.merchant, t.sell, &t.item_id, t.quantity, index)?;
        let total = q.total_cp;
        let mi = self
            .state
            .merchants
            .iter()
            .position(|m| m.id == t.merchant)
            .expect("quoted");
        let who = self.state.creatures[&t.entity].name.clone();
        if t.sell {
            let sheet = self.creature_mut(t.entity).sheet_mut().expect("quoted");
            let item =
                items::take_from(&mut sheet.inventory, &t.item_id, t.quantity).expect("quoted");
            receive(&mut sheet.currency, total);
            let m = &mut self.state.merchants[mi];
            if let Some(f) = &mut m.funds_cp {
                *f -= total;
            }
            if let Some(e) = m.stock.iter_mut().find(|e| e.item.id == item.id)
                && let Some(s) = &mut e.stock
            {
                *s += t.quantity;
            }
            self.log(
                Audience::All,
                format!(
                    "{who} sells {}× {name} for {}.",
                    t.quantity,
                    price_label(total)
                ),
            );
        } else {
            let i = index.expect("quoted");
            let mut item = self.state.merchants[mi].stock[i].item.clone();
            item.quantity = t.quantity;
            let sheet = self.creature_mut(t.entity).sheet_mut().expect("quoted");
            if !pay(&mut sheet.currency, total) {
                return rules("not enough money");
            }
            items::add_to(&mut sheet.inventory, item);
            let m = &mut self.state.merchants[mi];
            if let Some(s) = &mut m.stock[i].stock {
                *s -= t.quantity;
            }
            if let Some(f) = &mut m.funds_cp {
                *f += total;
            }
            self.log(
                Audience::All,
                format!(
                    "{who} buys {}× {name} for {}.",
                    t.quantity,
                    price_label(total)
                ),
            );
        }
        Ok(())
    }

    /// Timed conditions tick at the end of the creature's turn.
    pub(crate) fn tick_condition_timers(&mut self, entity: EntityId) {
        let Some(c) = self.state.creatures.get_mut(&entity) else {
            return;
        };
        let mut ended = vec![];
        for (cond, r) in c.condition_rounds.iter_mut() {
            *r = r.saturating_sub(1);
            if *r == 0 {
                ended.push(*cond);
            }
        }
        for cond in &ended {
            c.condition_rounds.remove(cond);
            c.conditions.remove(*cond);
        }
        let name = c.name.clone();
        for cond in ended {
            self.log(Audience::All, format!("{name} is no longer {cond:?}."));
        }
    }

    fn apply_template(&mut self, room: RoomId, t: MapTemplate) -> Res<()> {
        let (w, h) = {
            let m = &self.room(room)?.map;
            (m.width, m.height)
        };
        if w < 10 || h < 8 {
            return rules("templates need a map of at least 10×8");
        }
        if self
            .state
            .creatures
            .values()
            .any(|c| c.room == room && c.faction != Faction::Party)
        {
            return rules("remove the monsters and NPCs first (or use the eraser)");
        }
        use MapTemplate::*;
        let (floor, wall, indoor): (&str, Option<&str>, bool) = match t {
            ForestClearing => ("grass", None, false),
            SmallCave => ("cave_floor", Some("stone_wall"), true),
            DungeonChamber => ("dungeon_floor", Some("stone_wall"), true),
            Tavern => ("wood_floor", Some("wood_wall"), true),
            VillageSquare => ("cobblestone", None, false),
            CastleRoom => ("marble", Some("stone_wall"), true),
            Crypt => ("dungeon_floor", Some("stone_wall"), true),
            RuinedTemple => ("ruined_floor", Some("stone_wall"), false),
        };
        let cat = self.catalog.clone();
        let (fx, fy) = (
            |f: f32| ((w as f32 * f) as i32).clamp(1, w - 2),
            |f: f32| ((h as f32 * f) as i32).clamp(1, h - 2),
        );
        // Objects as (asset, x fraction, y fraction).
        let objs: &[(&str, f32, f32)] = match t {
            ForestClearing => &[
                ("tree", 0.1, 0.15),
                ("pine_tree", 0.85, 0.2),
                ("big_tree", 0.15, 0.75),
                ("autumn_tree", 0.8, 0.8),
                ("bush", 0.3, 0.1),
                ("berry_bush", 0.7, 0.12),
                ("stump", 0.55, 0.65),
                ("log", 0.42, 0.42),
                ("mushrooms", 0.25, 0.55),
                ("wildflowers", 0.62, 0.35),
                ("boulder", 0.9, 0.5),
            ],
            SmallCave => &[
                ("stalagmite", 0.25, 0.3),
                ("stalagmite", 0.7, 0.65),
                ("boulder", 0.8, 0.25),
                ("crystal", 0.2, 0.7),
                ("small_stones", 0.5, 0.5),
                ("glowing_mushrooms", 0.6, 0.3),
                ("puddle", 0.4, 0.7),
                ("bones", 0.3, 0.5),
            ],
            DungeonChamber => &[
                ("pillar", 0.25, 0.25),
                ("pillar", 0.75, 0.25),
                ("pillar", 0.25, 0.75),
                ("pillar", 0.75, 0.75),
                ("torch", 0.1, 0.1),
                ("torch", 0.9, 0.1),
                ("chest", 0.5, 0.2),
                ("rubble_pile", 0.6, 0.65),
                ("chains", 0.15, 0.5),
                ("door", 0.5, 0.95),
            ],
            Tavern => &[
                ("bar_counter", 0.15, 0.15),
                ("barrel", 0.75, 0.1),
                ("barrel", 0.82, 0.1),
                ("tavern_table", 0.25, 0.5),
                ("tavern_table", 0.6, 0.5),
                ("round_table", 0.4, 0.7),
                ("bench", 0.25, 0.65),
                ("chair", 0.7, 0.62),
                ("fireplace", 0.85, 0.45),
                ("candle", 0.3, 0.48),
                ("door", 0.5, 0.95),
            ],
            VillageSquare => &[
                ("well", 0.45, 0.4),
                ("signpost", 0.2, 0.3),
                ("crate", 0.75, 0.2),
                ("barrel", 0.8, 0.25),
                ("tree", 0.1, 0.8),
                ("tree", 0.9, 0.8),
                ("bench", 0.3, 0.75),
                ("lantern", 0.6, 0.7),
            ],
            CastleRoom => &[
                ("throne", 0.5, 0.15),
                ("carpet", 0.45, 0.3),
                ("pillar", 0.2, 0.3),
                ("pillar", 0.8, 0.3),
                ("wall_banner", 0.3, 0.05),
                ("wall_banner", 0.7, 0.05),
                ("armor_stand", 0.12, 0.7),
                ("armor_stand", 0.88, 0.7),
                ("candelabra", 0.35, 0.15),
                ("candelabra", 0.65, 0.15),
                ("door", 0.5, 0.95),
            ],
            Crypt => &[
                ("sarcophagus", 0.5, 0.35),
                ("coffin", 0.2, 0.3),
                ("coffin", 0.8, 0.3),
                ("tombstone", 0.2, 0.7),
                ("tombstone", 0.8, 0.7),
                ("skull_pile", 0.35, 0.75),
                ("cobweb", 0.05, 0.05),
                ("brazier", 0.35, 0.15),
                ("brazier", 0.65, 0.15),
                ("bones", 0.6, 0.6),
            ],
            RuinedTemple => &[
                ("broken_pillar", 0.2, 0.25),
                ("pillar", 0.8, 0.25),
                ("broken_pillar", 0.2, 0.7),
                ("statue", 0.8, 0.7),
                ("magic_altar", 0.45, 0.2),
                ("rubble_pile", 0.6, 0.55),
                ("vines", 0.1, 0.45),
                ("ritual_symbol", 0.42, 0.5),
                ("moss", 0.0, 0.0),
            ],
        };
        let rs = self.state.rooms.get_mut(&room).expect("checked");
        let m = &mut rs.map;
        for l in &mut m.layers {
            l.locked = false;
            l.tiles.iter_mut().for_each(|t| *t = 0);
        }
        m.objects.clear();
        m.indoor = indoor;
        let fa = cat.get(floor).cloned();
        let wa = wall.and_then(|w| cat.get(w).cloned());
        for y in 0..h {
            for x in 0..w {
                let _ = m.set_tile(LayerKind::Terrain, (x, y), fa.as_ref());
                if let Some(wa) = &wa {
                    let edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
                    let gap = y == h - 1 && (x - w / 2).abs() <= 0
                        || t == MapTemplate::RuinedTemple && (x + y) % 5 == 0;
                    if edge && !gap {
                        let _ = m.set_tile(LayerKind::Walls, (x, y), Some(wa));
                    }
                }
            }
        }
        let mut next = self.state.next_entity;
        for (asset, px, py) in objs {
            let Some(def) = cat.get(asset).filter(|a| a.is_object) else {
                continue;
            };
            let mut o = MapObject::from_asset(ObjectId(0), def, fx(*px), fy(*py));
            if *asset == "door" {
                o.y = h - 1;
            }
            if !o.footprint().all(|c| m.in_bounds(c)) {
                o.x = o.x.min(w - o.w as i32);
                o.y = o.y.min(h - o.h as i32);
            }
            o.id = ObjectId(next);
            next += 1;
            m.objects.push(o);
        }
        self.state.next_entity = next;
        let rs = self.state.rooms.get_mut(&room).expect("checked");
        rs.entry = (w / 2, h - 2);
        rs.containers.clear();
        rs.extra_lights.clear();
        for c in self.state.creatures.values_mut().filter(|c| c.room == room) {
            c.pos = (w / 2, h - 2);
        }
        Ok(())
    }
}
