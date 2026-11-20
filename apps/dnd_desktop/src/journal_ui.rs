//! Journal window (DM journal + quests; players see shared entries and revealed quests), the
//! object interaction menu for players, and the DM room-notes editor.

use crate::theme;
use crate::ui_game::GameUi;
use bevy_egui::egui::{self, RichText, Ui};
use dnd_session::command::{Command, DmCommand, ObjectAction};
use dnd_session::model::{JournalEntry, JournalSection, Objective, Quest, QuestKind, QuestStatus};
use dnd_session::view::ClientView;

fn section_key(s: JournalSection) -> &'static str {
    match s {
        JournalSection::Overview => "journal.overview",
        JournalSection::Chapters => "journal.chapters",
        JournalSection::Locations => "journal.locations",
        JournalSection::Npcs => "journal.npcs",
        JournalSection::Encounters => "journal.encounters",
        JournalSection::SessionNotes => "journal.session_notes",
        JournalSection::Secrets => "journal.secrets",
        JournalSection::Events => "journal.events",
    }
}
fn status_key(s: QuestStatus) -> &'static str {
    match s {
        QuestStatus::Undiscovered => "quest.undiscovered",
        QuestStatus::Active => "quest.active",
        QuestStatus::Completed => "quest.completed",
        QuestStatus::Failed => "quest.failed",
    }
}

pub fn journal_window(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    if !g.ui.show_journal {
        return;
    }
    let dm = g.net.is_dm();
    let mut open = true;
    theme::win(theme::display(g.i18n.t("journal.title"), 24.0))
        .id(egui::Id::new("journal"))
        .open(&mut open)
        .default_size(egui::vec2(760.0, 560.0))
        .resizable(true)
        .show(ctx, |ui| {
            theme::window_bg(ui);
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut g.ui.journal_tab,
                    0,
                    theme::gothic(g.i18n.t("journal.tab_journal"), 18.0),
                );
                ui.selectable_value(
                    &mut g.ui.journal_tab,
                    1,
                    theme::gothic(g.i18n.t("journal.tab_quests"), 18.0),
                );
                ui.add_space(12.0);
                ui.add(
                    egui::TextEdit::singleline(&mut g.ui.journal_search)
                        .hint_text(g.i18n.t("editor.search"))
                        .desired_width(180.0),
                );
            });
            ui.separator();
            if g.ui.journal_tab == 0 {
                journal_tab(ui, g, view, dm);
            } else {
                quests_tab(ui, g, view, dm);
            }
        });
    if !open {
        g.ui.show_journal = false;
    }
}

fn journal_tab(ui: &mut Ui, g: &mut GameUi, view: &ClientView, dm: bool) {
    let q = g.ui.journal_search.to_lowercase();
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        egui::ScrollArea::vertical()
            .id_salt("j-list")
            .show(ui, |ui| {
                for sec in JournalSection::ALL {
                    let entries: Vec<&JournalEntry> = view
                        .journal
                        .iter()
                        .filter(|e| {
                            e.section == sec
                                && (q.is_empty()
                                    || e.title.to_lowercase().contains(&q)
                                    || e.body.to_lowercase().contains(&q))
                        })
                        .collect();
                    if entries.is_empty() && !dm {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        ui.label(theme::bold(g.i18n.t(section_key(sec))));
                        if dm
                            && ui
                                .small_button("+")
                                .on_hover_text(g.i18n.t("journal.new"))
                                .clicked()
                        {
                            g.ui.journal_edit = Some(JournalEntry {
                                section: sec,
                                title: g.i18n.t("journal.untitled").into(),
                                ..Default::default()
                            });
                        }
                    });
                    for e in entries {
                        let sel = g.ui.journal_edit.as_ref().is_some_and(|x| x.id == e.id);
                        let label = if e.shared && dm {
                            format!("{} · {}", e.title, g.i18n.t("journal.shared"))
                        } else {
                            e.title.clone()
                        };
                        if ui.selectable_label(sel, label).clicked() {
                            g.ui.journal_edit = Some(e.clone());
                        }
                    }
                    ui.add_space(4.0);
                }
            });
        let ui = &mut cols[1];
        let Some(mut e) = g.ui.journal_edit.clone() else {
            ui.label(theme::italic(g.i18n.t("journal.pick")));
            return;
        };
        if !dm {
            ui.label(theme::display(&e.title, 22.0).color(theme::WOOD));
            egui::ScrollArea::vertical()
                .id_salt("j-read")
                .show(ui, |ui| ui.label(&e.body));
            return;
        }
        ui.text_edit_singleline(&mut e.title);
        egui::ComboBox::from_id_salt("j-sec")
            .selected_text(g.i18n.t(section_key(e.section)))
            .show_ui(ui, |ui| {
                for sec in JournalSection::ALL {
                    ui.selectable_value(&mut e.section, sec, g.i18n.t(section_key(sec)));
                }
            });
        ui.checkbox(&mut e.shared, g.i18n.t("journal.share"));
        egui::ScrollArea::vertical()
            .id_salt("j-body")
            .max_height(300.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut e.body)
                        .desired_rows(14)
                        .desired_width(f32::INFINITY),
                );
            });
        ui.horizontal(|ui| {
            if ui.button(g.i18n.t("journal.save")).clicked() {
                g.net.cmd(Command::Dm(DmCommand::SaveJournalEntry {
                    entry: e.clone(),
                }));
                if e.id == 0 {
                    g.ui.journal_edit = None;
                    return;
                }
            }
            if e.id != 0 && ui.button(g.i18n.t("journal.delete")).clicked() {
                g.net
                    .cmd(Command::Dm(DmCommand::DeleteJournalEntry { id: e.id }));
                g.ui.journal_edit = None;
                return;
            }
            g.ui.journal_edit = Some(e);
        });
    });
}

fn quests_tab(ui: &mut Ui, g: &mut GameUi, view: &ClientView, dm: bool) {
    let q = g.ui.journal_search.to_lowercase();
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        if dm && ui.button(g.i18n.t("quest.new")).clicked() {
            g.ui.quest_edit = Some(Quest {
                title: g.i18n.t("journal.untitled").into(),
                ..Default::default()
            });
        }
        egui::ScrollArea::vertical()
            .id_salt("q-list")
            .show(ui, |ui| {
                for qu in view
                    .quests
                    .iter()
                    .filter(|x| q.is_empty() || x.title.to_lowercase().contains(&q))
                {
                    let sel = g.ui.quest_edit.as_ref().is_some_and(|x| x.id == qu.id);
                    let kind = if qu.kind == QuestKind::Main {
                        g.i18n.t("quest.main")
                    } else {
                        g.i18n.t("quest.side")
                    };
                    let txt = format!(
                        "{} · {} · {}",
                        qu.title,
                        kind,
                        g.i18n.t(status_key(qu.status))
                    );
                    if ui.selectable_label(sel, txt).clicked() {
                        g.ui.quest_edit = Some(qu.clone());
                    }
                }
            });
        let ui = &mut cols[1];
        let Some(mut qu) = g.ui.quest_edit.clone() else {
            ui.label(theme::italic(g.i18n.t("journal.pick")));
            return;
        };
        if !dm {
            ui.label(theme::display(&qu.title, 22.0).color(theme::WOOD));
            ui.label(theme::italic(g.i18n.t(status_key(qu.status))));
            ui.label(&qu.description);
            for o in &qu.objectives {
                let mark = if o.done { "[x]" } else { "[ ]" };
                let opt = if o.optional {
                    format!(" ({})", g.i18n.t("quest.optional"))
                } else {
                    String::new()
                };
                ui.label(format!("{mark} {}{opt}", o.text));
            }
            if !qu.rewards.is_empty() {
                ui.label(format!("{}: {}", g.i18n.t("quest.rewards"), qu.rewards));
            }
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("q-edit")
            .show(ui, |ui| {
                ui.text_edit_singleline(&mut qu.title);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut qu.kind, QuestKind::Main, g.i18n.t("quest.main"));
                    ui.selectable_value(&mut qu.kind, QuestKind::Side, g.i18n.t("quest.side"));
                });
                ui.horizontal_wrapped(|ui| {
                    for st in [
                        QuestStatus::Undiscovered,
                        QuestStatus::Active,
                        QuestStatus::Completed,
                        QuestStatus::Failed,
                    ] {
                        ui.selectable_value(&mut qu.status, st, g.i18n.t(status_key(st)));
                    }
                });
                ui.label(g.i18n.t("quest.description"));
                ui.add(
                    egui::TextEdit::multiline(&mut qu.description)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                ui.label(theme::bold(g.i18n.t("quest.objectives")));
                let mut remove = None;
                for (i, o) in qu.objectives.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut o.done, "");
                        ui.text_edit_singleline(&mut o.text);
                        ui.checkbox(&mut o.hidden, g.i18n.t("quest.hidden"));
                        ui.checkbox(&mut o.optional, g.i18n.t("quest.optional"));
                        if ui.small_button("x").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    qu.objectives.remove(i);
                }
                if ui.small_button(g.i18n.t("quest.add_objective")).clicked() {
                    qu.objectives.push(Objective::default());
                }
                ui.horizontal(|ui| {
                    ui.label(g.i18n.t("quest.rewards"));
                    ui.text_edit_singleline(&mut qu.rewards);
                });
                ui.horizontal(|ui| {
                    ui.label(g.i18n.t("quest.npcs"));
                    ui.text_edit_singleline(&mut qu.npcs);
                });
                ui.label(RichText::new(g.i18n.t("quest.dm_notes")).color(theme::BURGUNDY));
                ui.add(
                    egui::TextEdit::multiline(&mut qu.dm_notes)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    if ui.button(g.i18n.t("journal.save")).clicked() {
                        g.net
                            .cmd(Command::Dm(DmCommand::SaveQuest { quest: qu.clone() }));
                        if qu.id == 0 {
                            g.ui.quest_edit = None;
                            return;
                        }
                    }
                    if qu.id != 0 && ui.button(g.i18n.t("journal.delete")).clicked() {
                        g.net.cmd(Command::Dm(DmCommand::DeleteQuest { id: qu.id }));
                        g.ui.quest_edit = None;
                        return;
                    }
                    g.ui.quest_edit = Some(qu.clone());
                });
            });
    });
}

/// DM-only room notes in the Room Details inspector (saved when the field loses focus).
pub fn room_notes(ui: &mut Ui, g: &mut GameUi, view: &ClientView, room: dnd_world::RoomId) {
    let saved = view
        .graph
        .nodes
        .get(&room)
        .map(|n| n.dm_notes.clone())
        .unwrap_or_default();
    egui::CollapsingHeader::new(theme::gothic(g.i18n.t("room.notes"), 18.0).color(theme::WOOD))
        .id_salt("room-notes")
        .default_open(!saved.is_empty())
        .show(ui, |ui| {
            let mut text = match &g.ui.room_notes_edit {
                Some((r, t)) if *r == room => t.clone(),
                _ => saved.clone(),
            };
            let resp = ui.add(
                egui::TextEdit::multiline(&mut text)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY)
                    .hint_text(g.i18n.t("room.notes_hint")),
            );
            if resp.changed() {
                g.ui.room_notes_edit = Some((room, text.clone()));
            }
            if resp.lost_focus()
                && let Some((r, t)) = g.ui.room_notes_edit.take()
                && t != saved
            {
                g.net
                    .cmd(Command::Dm(DmCommand::SetRoomNotes { room: r, notes: t }));
            }
            ui.label(RichText::new(g.i18n.t("room.notes_help")).small().italics());
        });
}

/// Player action menu for an object (Inspect, Open/Close, key, pick, force, break).
pub fn object_menu(ctx: &egui::Context, g: &mut GameUi) {
    let Some((object, entity, pos)) = g.ui.object_menu else {
        return;
    };
    let Some(o) = g.gv.map.as_ref().and_then(|m| m.object(object)).cloned() else {
        g.ui.object_menu = None;
        return;
    };
    let name = if o.label.is_empty() {
        g.catalog
            .0
            .get(&o.asset_id)
            .map(|a| a.name.clone())
            .unwrap_or_default()
    } else {
        o.label.clone()
    };
    let mut close = false;
    let area = egui::Area::new("object-menu".into())
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(pos.0, pos.1))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(theme::PARCHMENT)
                .stroke(egui::Stroke::new(1.5, theme::BRONZE))
                .show(ui, |ui| {
                    ui.label(theme::gothic(&name, 19.0).color(theme::WOOD));
                    if o.destroyed {
                        ui.label(theme::italic(g.i18n.t("object.broken")));
                    } else if o.is_locked() {
                        ui.label(theme::italic(g.i18n.t("object.locked")));
                    }
                    let mut act = |ui: &mut Ui, key: &str, a: ObjectAction| {
                        if ui.button(g.i18n.t(key)).clicked() {
                            g.net.cmd(Command::ObjectAction {
                                entity,
                                object,
                                action: a,
                            });
                            close = true;
                        }
                    };
                    act(ui, "object.inspect", ObjectAction::Inspect);
                    if o.is_locked() {
                        act(ui, "object.use_key", ObjectAction::UseKey);
                        act(ui, "object.pick", ObjectAction::PickLock);
                        act(ui, "object.force", ObjectAction::ForceLock);
                    }
                    if o.break_dc.is_some() && !o.destroyed {
                        act(ui, "object.break", ObjectAction::Break);
                    }
                    if o.interactable
                        && !o.is_locked()
                        && ui
                            .button(g.i18n.t(if o.open {
                                "object.close"
                            } else {
                                "object.open"
                            }))
                            .clicked()
                    {
                        g.net.cmd(Command::Interact { entity, object });
                        close = true;
                    }
                    if ui.small_button(g.i18n.t("cast.cancel")).clicked() {
                        close = true;
                    }
                });
        });
    // Close on Esc, on a choice, or on a click outside (ignoring the click that opened it).
    let frame = ctx.cumulative_frame_nr();
    let id = egui::Id::new(("object-menu-opened", object.0));
    let opened = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || frame));
    let outside = ctx.input(|i| i.pointer.any_pressed())
        && !ctx
            .pointer_latest_pos()
            .is_some_and(|p| area.response.rect.contains(p));
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) || outside && frame > opened + 1 {
        g.ui.object_menu = None;
        ctx.data_mut(|d| d.remove::<u64>(id));
    }
}
