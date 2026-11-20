//! Shop window (players buy/sell; the DM edits merchants and approves trades) and the campaign
//! clock in the top bar.

use crate::theme;
use crate::ui_game::GameUi;
use bevy_egui::egui::{self, RichText, Ui};
use dnd_session::command::{Command, DmCommand};
use dnd_session::model::{Merchant, ShopItem};
use dnd_session::view::ClientView;
use dnd_session::world_extras::price_label;

pub fn shop_window(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    if !g.ui.show_shop {
        return;
    }
    let dm = g.net.is_dm();
    let mut open = true;
    theme::win(theme::display(g.i18n.t("shop.title"), 24.0))
        .id(egui::Id::new("shop"))
        .open(&mut open)
        .default_size(egui::vec2(720.0, 520.0))
        .resizable(true)
        .show(ctx, |ui| {
            theme::window_bg(ui);
            ui.horizontal_wrapped(|ui| {
                for m in &view.merchants {
                    let label = if dm && !m.open {
                        format!("{} ({})", m.name, g.i18n.t("shop.closed"))
                    } else {
                        m.name.clone()
                    };
                    if ui
                        .selectable_label(g.ui.shop_sel == Some(m.id), theme::gothic(label, 18.0))
                        .clicked()
                    {
                        g.ui.shop_sel = Some(m.id);
                        g.ui.shop_edit = None;
                    }
                }
                if dm && ui.button(g.i18n.t("shop.new")).clicked() {
                    g.ui.shop_edit = Some(Merchant {
                        name: g.i18n.t("shop.new_name").into(),
                        ..Default::default()
                    });
                    g.ui.shop_sel = None;
                }
            });
            ui.separator();
            if dm && !view.trades.is_empty() {
                pending_trades(ui, g, view);
                ui.separator();
            }
            if dm && g.ui.shop_edit.is_some() {
                merchant_editor(ui, g);
                return;
            }
            let Some(m) =
                g.ui.shop_sel
                    .and_then(|id| view.merchants.iter().find(|m| m.id == id))
                    .cloned()
            else {
                ui.label(theme::italic(g.i18n.t(if view.merchants.is_empty() {
                    "shop.none"
                } else {
                    "shop.pick"
                })));
                return;
            };
            if dm && ui.button(g.i18n.t("shop.edit")).clicked() {
                g.ui.shop_edit = Some(m.clone());
                return;
            }
            storefront(ui, g, view, &m, dm);
        });
    if !open {
        g.ui.show_shop = false;
    }
}

fn my_character(
    g: &GameUi,
    view: &ClientView,
) -> Option<(dnd_world::EntityId, dnd_rules::character::CharacterSheet)> {
    if let Some(sel) = g.ui.selected
        && let Some(c) = view.my_characters.iter().find(|(id, _)| *id == sel)
    {
        return Some(c.clone());
    }
    view.my_characters.first().cloned()
}

fn storefront(ui: &mut Ui, g: &mut GameUi, view: &ClientView, m: &Merchant, dm: bool) {
    let me = my_character(g, view);
    if let Some((_, sheet)) = &me {
        ui.label(theme::bold(format!(
            "{} · {}: {}",
            sheet.name,
            g.i18n.t("shop.purse"),
            price_label(sheet.currency.total_cp())
        )));
    } else if !dm {
        ui.label(theme::italic(g.i18n.t("shop.no_character")));
    }
    if !m.auto_approve {
        ui.label(
            RichText::new(g.i18n.t("shop.needs_approval"))
                .small()
                .italics(),
        );
    }
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        theme::heading(ui, g.i18n.t("shop.buy"));
        egui::ScrollArea::vertical()
            .id_salt("shop-stock")
            .show(ui, |ui| {
                for (i, e) in m.stock.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let stock = e.stock.map(|s| format!(" ×{s}")).unwrap_or_default();
                        ui.label(theme::bold(&e.item.name))
                            .on_hover_text(&e.item.description);
                        ui.label(
                            RichText::new(format!("{}{stock}", price_label(e.price_cp as u64)))
                                .color(theme::INK_SOFT),
                        );
                        if let Some((id, sheet)) = &me {
                            let afford = sheet.currency.total_cp() >= e.price_cp as u64
                                && e.stock != Some(0);
                            if ui
                                .add_enabled(afford, egui::Button::new(g.i18n.t("shop.buy_one")))
                                .clicked()
                            {
                                g.net.cmd(Command::Buy {
                                    entity: *id,
                                    merchant: m.id,
                                    item: i,
                                    quantity: 1,
                                });
                            }
                        }
                    });
                }
            });
        let ui = &mut cols[1];
        theme::heading(ui, g.i18n.t("shop.sell"));
        if !m.buys {
            ui.label(theme::italic(g.i18n.t("shop.doesnt_buy")));
            return;
        }
        let Some((id, sheet)) = &me else { return };
        egui::ScrollArea::vertical()
            .id_salt("shop-sell")
            .show(ui, |ui| {
                for it in sheet.inventory.iter().filter(|i| i.value_cp > 0) {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} ×{}", it.name, it.quantity));
                        let offer = it.value_cp as u64 * m.buy_rate as u64 / 100;
                        ui.label(RichText::new(price_label(offer)).color(theme::INK_SOFT));
                        if offer > 0 && ui.button(g.i18n.t("shop.sell_one")).clicked() {
                            g.net.cmd(Command::Sell {
                                entity: *id,
                                merchant: m.id,
                                item_id: it.id.clone(),
                                quantity: 1,
                            });
                        }
                    });
                }
            });
    });
    // The player's own requests waiting for the DM.
    let mine: Vec<_> = view.trades.iter().filter(|t| t.merchant == m.id).collect();
    if !dm && !mine.is_empty() {
        ui.separator();
        for t in mine {
            ui.label(theme::italic(format!(
                "{}: {}× {} ({})",
                g.i18n.t("shop.waiting"),
                t.quantity,
                t.item_id,
                price_label(t.total_cp)
            )));
        }
    }
}

fn pending_trades(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    theme::heading(ui, g.i18n.t("shop.pending"));
    for t in &view.trades {
        let who = view
            .creatures
            .iter()
            .find(|c| c.id == t.entity)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        let shop = view
            .merchants
            .iter()
            .find(|m| m.id == t.merchant)
            .map(|m| m.name.clone())
            .unwrap_or_default();
        ui.horizontal(|ui| {
            let verb = g.i18n.t(if t.sell { "shop.sells" } else { "shop.buys" });
            ui.label(format!(
                "{who} {verb} {}× {} — {} ({shop})",
                t.quantity,
                t.item_id,
                price_label(t.total_cp)
            ));
            if ui.button(g.i18n.t("shop.approve")).clicked() {
                g.net.cmd(Command::Dm(DmCommand::ResolveTrade {
                    id: t.id,
                    approve: true,
                }));
            }
            if ui.button(g.i18n.t("shop.refuse")).clicked() {
                g.net.cmd(Command::Dm(DmCommand::ResolveTrade {
                    id: t.id,
                    approve: false,
                }));
            }
        });
    }
}

fn merchant_editor(ui: &mut Ui, g: &mut GameUi) {
    let Some(mut m) = g.ui.shop_edit.clone() else {
        return;
    };
    egui::ScrollArea::vertical()
        .id_salt("shop-edit")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(g.i18n.t("shop.name"));
                ui.text_edit_singleline(&mut m.name);
            });
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut m.open, g.i18n.t("shop.open"));
                ui.checkbox(&mut m.auto_approve, g.i18n.t("shop.auto"));
                ui.checkbox(&mut m.buys, g.i18n.t("shop.buys_items"));
                ui.add(
                    egui::DragValue::new(&mut m.buy_rate)
                        .range(0..=100)
                        .suffix("%"),
                );
            });
            ui.horizontal(|ui| {
                let mut limited = m.funds_cp.is_some();
                ui.checkbox(&mut limited, g.i18n.t("shop.funds"));
                if limited {
                    let mut gp = m.funds_cp.unwrap_or(10_000) / 100;
                    ui.add(
                        egui::DragValue::new(&mut gp)
                            .range(0..=1_000_000)
                            .suffix(" gp"),
                    );
                    m.funds_cp = Some(gp * 100);
                } else {
                    m.funds_cp = None;
                }
            });
            theme::heading(ui, g.i18n.t("shop.stock"));
            let mut remove = None;
            for (i, e) in m.stock.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(&e.item.name);
                    let mut gp = e.price_cp as f32 / 100.0;
                    if ui
                        .add(
                            egui::DragValue::new(&mut gp)
                                .range(0.0..=100_000.0)
                                .speed(0.1)
                                .suffix(" gp"),
                        )
                        .changed()
                    {
                        e.price_cp = (gp * 100.0).round() as u32;
                    }
                    let mut limited = e.stock.is_some();
                    ui.checkbox(&mut limited, g.i18n.t("shop.limited"));
                    if limited {
                        let mut n = e.stock.unwrap_or(5);
                        ui.add(egui::DragValue::new(&mut n).range(0..=999));
                        e.stock = Some(n);
                    } else {
                        e.stock = None;
                    }
                    if ui.small_button("x").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                m.stock.remove(i);
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut g.ui.shop_search)
                        .hint_text(g.i18n.t("shop.add_item"))
                        .desired_width(200.0),
                );
            });
            let q = g.ui.shop_search.to_lowercase();
            if q.len() >= 2 {
                for it in dnd_rules::items::catalog()
                    .into_iter()
                    .filter(|i| i.name.to_lowercase().contains(&q))
                    .take(12)
                {
                    if ui
                        .small_button(format!(
                            "+ {} ({})",
                            it.name,
                            price_label(it.value_cp as u64)
                        ))
                        .clicked()
                    {
                        let price = it.value_cp.max(1);
                        m.stock.push(ShopItem {
                            item: dnd_rules::items::ItemStack { quantity: 1, ..it },
                            price_cp: price,
                            stock: None,
                        });
                    }
                }
            }
            ui.horizontal(|ui| {
                if ui.button(g.i18n.t("journal.save")).clicked() {
                    g.net.cmd(Command::Dm(DmCommand::SaveMerchant {
                        merchant: m.clone(),
                    }));
                    g.ui.shop_sel = (m.id != 0).then_some(m.id);
                    g.ui.shop_edit = None;
                    return;
                }
                if m.id != 0 && ui.button(g.i18n.t("journal.delete")).clicked() {
                    g.net
                        .cmd(Command::Dm(DmCommand::DeleteMerchant { id: m.id }));
                    g.ui.shop_edit = None;
                    g.ui.shop_sel = None;
                    return;
                }
                if ui.button(g.i18n.t("cast.cancel")).clicked() {
                    g.ui.shop_edit = None;
                    return;
                }
                g.ui.shop_edit = Some(m.clone());
            });
        });
}

/// Clock in the top bar; the DM's dropdown advances time and runs rests.
pub fn clock(ui: &mut Ui, g: &mut GameUi, view: &ClientView) {
    use dnd_session::model::DayPhase;
    let t = view.time;
    let icon = match t.phase() {
        DayPhase::Night => crate::icons::Icon::Star,
        _ => crate::icons::Icon::Sun,
    };
    let r = crate::chrome::bar_button(ui, Some(icon), &t.label(), false);
    if g.net.is_dm() {
        egui::Popup::from_toggle_button_response(&r).show(|ui| {
            theme::dark_scope(ui);
            ui.label(theme::bold(g.i18n.t("time.advance")));
            ui.horizontal(|ui| {
                for (label, m) in [
                    ("+10 min", 10u32),
                    ("+1 h", 60),
                    ("+8 h", 480),
                    ("+1 day", 1440),
                ] {
                    if ui.button(label).clicked() {
                        g.net
                            .cmd(Command::Dm(DmCommand::AdvanceTime { minutes: m }));
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .button(g.i18n.t("time.short_rest"))
                    .on_hover_text(g.i18n.t("time.short_rest_help"))
                    .clicked()
                {
                    g.net.cmd(Command::Dm(DmCommand::Rest { long: false }));
                }
                if ui
                    .button(g.i18n.t("time.long_rest"))
                    .on_hover_text(g.i18n.t("time.long_rest_help"))
                    .clicked()
                {
                    g.net.cmd(Command::Dm(DmCommand::Rest { long: true }));
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                let (mut d, mut h) = (t.day, t.minute / 60);
                let a = ui.add(
                    egui::DragValue::new(&mut d)
                        .range(1..=100_000)
                        .prefix(g.i18n.t("time.day")),
                );
                let b = ui.add(egui::DragValue::new(&mut h).range(0..=23).suffix(":00"));
                if (a.changed() || b.changed()) && !(a.dragged() || b.dragged()) {
                    g.net.cmd(Command::Dm(DmCommand::SetTime {
                        day: d,
                        minute: h * 60,
                    }));
                }
            });
        });
    }
}
