//! Developer automation for headless smoke tests (no effect unless `DND55_AUTO` is set).
//!
//! * `DND55_AUTO=host` — host the demo campaign, run a short scripted DM sequence.
//! * `DND55_AUTO=join` — join `DND55_AUTO_ADDR` with code `DND55_SESSION_CODE` as `DND55_AUTO_NAME`,
//!   bringing a ready-made level-1 fighter with a generated portrait.
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
            let sheet = CharacterSheet::example_fighter(
                &env("DND55_AUTO_NAME").unwrap_or_else(|| "Brunhild".into()),
            );
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
            if once("combat", 8.0) {
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
    if let Some(path) = env("DND55_SCREENSHOT") {
        let after: f32 = env("DND55_SCREENSHOT_AFTER")
            .and_then(|v| v.parse().ok())
            .unwrap_or(10.5);
        if once("shot", after) {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
        if once("exit", after + 2.0) {
            exit.write(AppExit::Success);
        }
    }
}
