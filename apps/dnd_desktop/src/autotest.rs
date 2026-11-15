//! Developer automation for headless smoke tests (no effect unless `DND55_AUTO` is set).
//!
//! * `DND55_AUTO=host` — host the demo campaign, run a short scripted DM sequence.
//! * `DND55_AUTO=join` — join `DND55_AUTO_ADDR` with code `DND55_SESSION_CODE` as `DND55_AUTO_NAME`,
//!   bringing a ready-made level-1 fighter with a generated portrait.
//! * `DND55_AUTO_SCENARIO=calm` — like the default host script but without starting combat.
//! * `DND55_AUTO_CLASS=wizard` — the auto-joining player brings a level-1 Wizard instead.
//! * `DND55_AUTO_OPEN=bestiary,graph,events,invite,settings,spellbook,prepare,inventory,select,draw,cast`
//!   — open those windows/tools shortly before the screenshot (GUI smoke tests).
//! * `DND55_SCREENSHOT=<path>` + `DND55_SCREENSHOT_AFTER=<secs>` — save a screenshot, then exit.

use crate::hosting;
use crate::state::{GameView, HostRuntime, Net};
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use dnd_rules::character::CharacterSheet;
use dnd_session::command::{Command, DmCommand};

#[derive(Resource, Default)]
pub struct AutoState {
    pub done: Vec<&'static str>,
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

pub fn auto_start(mut commands: Commands, mut net: ResMut<Net>) {
    match env("DND55_AUTO").as_deref() {
        Some("host") => {
            let port = env("DND55_AUTO_PORT")
                .and_then(|p| p.parse().ok())
                .unwrap_or(7878);
            let store = hosting::open_store().expect("store");
            let id = store.create_campaign("The Sunken Crypt").expect("campaign");
            let host =
                hosting::host_session(dnd_session::demo::demo_campaign(), id, port, &mut net)
                    .expect("host");
            commands.insert_resource::<HostRuntime>(host);
        }
        Some("join") => {
            let name = env("DND55_AUTO_NAME").unwrap_or_else(|| "Brunhild".into());
            let sheet = match env("DND55_AUTO_CLASS").as_deref() {
                Some("wizard") => CharacterSheet::example_wizard(&name),
                _ => CharacterSheet::example_fighter(&name),
            };
            net.pending_character = Some((sheet, Some(portrait_png())));
            hosting::join(
                &mut net,
                &env("DND55_AUTO_ADDR").unwrap_or_else(|| "127.0.0.1:7878".into()),
                &env("DND55_SESSION_CODE").unwrap_or_default(),
                &env("DND55_AUTO_NAME").unwrap_or_else(|| "Brunhild".into()),
            )
            .expect("valid auto-join address");
        }
        _ => {}
    }
}

/// A generated portrait: a stylised helmeted face on a dark red field (original art).
fn portrait_png() -> Vec<u8> {
    let img = image::RgbaImage::from_fn(128, 128, |x, y| {
        let (dx, dy) = (x as f32 - 64.0, y as f32 - 70.0);
        let d = (dx * dx + dy * dy).sqrt();
        if d < 34.0 && y > 40 {
            image::Rgba([224, 186, 150, 255]) // face
        } else if d < 46.0 && y < 64 {
            image::Rgba([150, 150, 160, 255]) // helmet
        } else if (y as i32 - 70).abs() < 4 && dx.abs() < 20.0 && dx.abs() > 6.0 {
            image::Rgba([40, 30, 30, 255]) // eyes
        } else {
            image::Rgba([110 + (y / 4) as u8, 20, 24, 255])
        }
    });
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .expect("encode");
    out.into_inner()
}

pub fn auto_script(
    time: Res<Time>,
    mut st: ResMut<AutoState>,
    mut net: ResMut<Net>,
    gv: Res<GameView>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut ui: ResMut<crate::state::UiState>,
) {
    let t = time.elapsed_secs();
    let mut once = |name: &'static str, at: f32| -> bool {
        if t >= at && !st.done.contains(&name) {
            st.done.push(name);
            true
        } else {
            false
        }
    };
    let crypt = env("DND55_AUTO_SCENARIO").as_deref() == Some("crypt");
    if env("DND55_AUTO").as_deref() == Some("host") && crypt {
        if once("travel", 8.0)
            && let Some(to) = gv
                .view
                .as_ref()
                .and_then(|v| v.graph.nodes.values().find(|n| n.name == "Sunken Crypt"))
                .map(|n| n.id)
        {
            net.cmd(Command::Dm(DmCommand::TransitionParty {
                to,
                override_rules: false,
            }));
        }
    } else if env("DND55_AUTO").as_deref() == Some("host") {
        let room = gv.view.as_ref().and_then(|v| v.room.as_ref()).map(|r| r.id);
        if let Some(room) = room {
            if once("spawn", 6.0) {
                net.cmd(Command::Dm(DmCommand::SpawnMonster {
                    template: "goblin_warrior".into(),
                    room,
                    cell: (9, 6),
                    hidden: false,
                }));
            }
            if once("combat", 8.0) && env("DND55_AUTO_SCENARIO").as_deref() != Some("calm") {
                net.cmd(Command::Dm(DmCommand::StartCombat { surprised: vec![] }));
            }
            if once("hurt", 10.0)
                && let Some(g) = gv
                    .view
                    .as_ref()
                    .and_then(|v| v.creatures.iter().find(|c| c.name.starts_with("Goblin")))
            {
                net.cmd(Command::Dm(DmCommand::AdjustHp {
                    entity: g.id,
                    delta: -4,
                }));
            }
        }
    }
    if env("DND55_AUTO").as_deref() == Some("join")
        && crypt
        && once("walk", 11.0)
        && let Some((id, _)) = gv
            .view
            .as_ref()
            .and_then(|v| v.my_characters.first().cloned())
    {
        net.cmd(Command::Move {
            entity: id,
            to: (7, 7),
        });
    }
    if env("DND55_AUTO").as_deref() == Some("join")
        && !crypt
        && once("move", 9.0)
        && let Some((id, _)) = gv
            .view
            .as_ref()
            .and_then(|v| v.my_characters.first().cloned())
    {
        net.cmd(Command::RollDice {
            expr: "1d20+5".into(),
            mode: Default::default(),
            private: false,
            label: "Perception".into(),
        });
        net.cmd(Command::Move {
            entity: id,
            to: (6, 7),
        });
    }
    let after: f32 = env("DND55_SCREENSHOT_AFTER")
        .and_then(|v| v.parse().ok())
        .unwrap_or(10.5);
    if let Some(list) = env("DND55_AUTO_OPEN")
        && once(
            "open",
            env("DND55_AUTO_OPEN_AT")
                .and_then(|v| v.parse().ok())
                .unwrap_or(after - 1.5),
        )
    {
        open_windows(&list, &mut ui, &gv);
    }
    if let Some(path) = env("DND55_SCREENSHOT") {
        if once("shot", after) {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
        if once("exit", after + 2.0) {
            // GUI tests assert on the adventure log the host sent us.
            if let Some(v) = &gv.view {
                for line in &v.log {
                    println!("GAMELOG: {line}");
                }
                println!(
                    "GAMELOG-CREATURES: {}",
                    v.creatures
                        .iter()
                        .map(|c| c.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            exit.write(AppExit::Success);
        }
    }
}

fn open_windows(list: &str, ui: &mut crate::state::UiState, gv: &GameView) {
    use crate::state::{DmTab, Tool};
    let view = gv.view.as_ref();
    for w in list.split(',').map(str::trim) {
        match w {
            "bestiary" => {
                ui.show_bestiary = true;
                ui.bestiary_preview = Some("goblin_warrior".into());
            }
            "graph" => ui.show_graph = true,
            "events" => {
                ui.dm_tab = DmTab::Events;
                let room = view
                    .and_then(|v| v.room.as_ref())
                    .map(|r| r.id)
                    .unwrap_or(dnd_world::RoomId(0));
                ui.script_draft = Some(dnd_session::scripts::ScriptedEvent {
                    id: 0,
                    name: "Pressure plate".into(),
                    enabled: true,
                    once: true,
                    fired: false,
                    trigger: dnd_session::scripts::Trigger::EnterArea {
                        room,
                        a: (5, 5),
                        b: (6, 6),
                    },
                    actions: vec![dnd_session::scripts::ScriptAction::Trap {
                        dice: "2d6".into(),
                        damage_type: dnd_rules::damage::DamageType::Piercing,
                        save: Some(dnd_rules::Ability::Dexterity),
                        dc: 13,
                        half_on_success: true,
                    }],
                });
            }
            "invite" => ui.show_invite = true,
            "settings" => ui.show_settings = true,
            "spellbook" => ui.show_spellbook = true,
            "prepare" => {
                ui.show_spellbook = true;
                ui.spellbook_prepare = true;
            }
            "inventory" => ui.show_inventory = true,
            "select" => {
                ui.tool = Tool::Select;
                ui.dm_tab = DmTab::Editor;
                if let Some(o) = gv.map.as_ref().and_then(|m| m.objects.first()) {
                    ui.sel_objects = vec![o.id];
                }
            }
            "draw" => ui.tool = Tool::Draw,
            "cast" => {
                if let Some((id, sheet)) = view.and_then(|v| v.my_characters.first().cloned())
                    && let Some(sp) = dnd_rules::spells::spell("burning_hands")
                {
                    ui.casting = crate::ui_play::cast_intent(sp, id, Some(1), sheet.level);
                }
            }
            _ => {}
        }
    }
}
