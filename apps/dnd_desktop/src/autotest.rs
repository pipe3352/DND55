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
    let showcase = env("DND55_AUTO_SCENARIO").as_deref() == Some("showcase");
    if env("DND55_AUTO").as_deref() == Some("host") && showcase {
        if once("sc-create", 5.0) {
            net.cmd(Command::Dm(DmCommand::CreateRoom {
                name: "Whispering Vale".into(),
                width: 26,
                height: 15,
                ambient: dnd_world::map::AmbientLight::Dim,
            }));
        }
        let vale = gv
            .view
            .as_ref()
            .and_then(|v| v.graph.nodes.values().find(|n| n.name == "Whispering Vale"))
            .map(|n| n.id);
        if let Some(room) = vale {
            if once("sc-paint", 6.0) {
                for c in showcase_commands(room) {
                    net.cmd(c);
                }
            }
            if once("sc-rooms", 6.3) {
                for name in ["Old Mill", "Bandit Camp", "Ruined Shrine"] {
                    net.cmd(Command::Dm(DmCommand::CreateRoom {
                        name: name.into(),
                        width: 16,
                        height: 12,
                        ambient: dnd_world::map::AmbientLight::Dim,
                    }));
                }
            }
            if once("sc-links", 7.0)
                && let Some(v) = gv.view.as_ref()
            {
                let id = |n: &str| v.graph.nodes.values().find(|x| x.name == n).map(|x| x.id);
                if let (Some(guard), Some(crypt), Some(mill), Some(camp), Some(shrine)) = (
                    id("Guard Room"),
                    id("Sunken Crypt"),
                    id("Old Mill"),
                    id("Bandit Camp"),
                    id("Ruined Shrine"),
                ) {
                    for (a, b, one_way) in [
                        (guard, room, false),
                        (room, mill, false),
                        (mill, camp, false),
                        (camp, shrine, true),
                        (crypt, shrine, false),
                    ] {
                        net.cmd(Command::Dm(DmCommand::ConnectRooms {
                            from: a,
                            to: b,
                            one_way,
                        }));
                    }
                    for (r, region) in [
                        (guard, "Keep"),
                        (crypt, "Keep"),
                        (room, "Wilds"),
                        (mill, "Wilds"),
                        (camp, "Wilds"),
                    ] {
                        net.cmd(Command::Dm(DmCommand::UpdateRoomMeta {
                            room: r,
                            region: region.into(),
                            chapter: String::new(),
                            secret: false,
                        }));
                    }
                    net.cmd(Command::Dm(DmCommand::UpdateRoomMeta {
                        room: shrine,
                        region: String::new(),
                        chapter: String::new(),
                        secret: true,
                    }));
                }
            }
            if once("sc-edges", 7.4)
                && let Some(v) = gv.view.as_ref()
            {
                let id = |n: &str| v.graph.nodes.values().find(|x| x.name == n).map(|x| x.id);
                for e in &v.graph.edges {
                    let locked = Some(e.from) == id("Old Mill") && Some(e.to) == id("Bandit Camp");
                    let secret =
                        Some(e.from) == id("Sunken Crypt") && Some(e.to) == id("Ruined Shrine");
                    if locked || secret {
                        net.cmd(Command::Dm(DmCommand::EditEdge {
                            edge: e.id,
                            one_way: e.one_way,
                            locked,
                            secret,
                            label: if locked {
                                "Iron gate".into()
                            } else {
                                String::new()
                            },
                        }));
                    }
                }
                for (r, pos) in crate::graph_editor::auto_layout(&v.graph, Some(room)) {
                    net.cmd(Command::Dm(DmCommand::SetRoomPos { room: r, pos }));
                }
            }
            if once("sc-travel", 7.5) {
                net.cmd(Command::Dm(DmCommand::TransitionParty {
                    to: room,
                    override_rules: true,
                }));
            }
            if once("sc-view", 8.5) {
                net.cmd(Command::Dm(DmCommand::ViewRoom { room }));
            }
        }
    } else if env("DND55_AUTO").as_deref() == Some("host") && crypt {
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
        && (crypt || showcase)
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
        && !showcase
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
            "creator" => ui.show_creator = true,
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

/// A room exercising every terrain type, water, lava and object families (visual regression scene).
fn showcase_commands(room: dnd_world::RoomId) -> Vec<Command> {
    use dnd_world::map::LayerKind::{Terrain, Walls};
    let fill = |layer, a: (i32, i32), b: (i32, i32), asset: &str| {
        Command::Dm(DmCommand::FillRect {
            room,
            layer,
            a,
            b,
            asset: Some(asset.into()),
        })
    };
    let obj = |asset: &str, cell: (i32, i32)| {
        Command::Dm(DmCommand::PlaceObject {
            room,
            asset: asset.into(),
            cell,
            hidden: false,
        })
    };
    let mut v = vec![
        fill(Terrain, (0, 0), (25, 14), "grass"),
        fill(Terrain, (0, 6), (25, 7), "road"),
        fill(Terrain, (8, 0), (9, 14), "dirt"),
        fill(Terrain, (1, 1), (6, 4), "water_shallow"),
        fill(Terrain, (2, 2), (5, 3), "water_deep"),
        fill(Terrain, (1, 9), (5, 13), "mud"),
        fill(Terrain, (11, 9), (16, 13), "stone_floor"),
        fill(Walls, (11, 9), (16, 9), "stone_wall"),
        fill(Walls, (11, 9), (11, 13), "stone_wall"),
        fill(Terrain, (18, 9), (24, 13), "cave_floor"),
        fill(Terrain, (20, 10), (23, 12), "lava"),
        fill(Terrain, (11, 0), (15, 4), "sand"),
        fill(Terrain, (16, 0), (20, 4), "snow"),
        fill(Terrain, (21, 0), (25, 4), "ice"),
        fill(Terrain, (12, 11), (15, 12), "wood_floor"),
        fill(Terrain, (6, 9), (7, 13), "gravel"),
        fill(Terrain, (2, 8), (4, 8), "undergrowth"),
        Command::Dm(DmCommand::SetRoomWeather {
            room,
            weather: dnd_session::model::Weather {
                kind: dnd_session::model::WeatherKind::Leaves,
                intensity: 0.8,
            },
        }),
    ];
    for (a, c) in [
        ("tree", (0, 5)),
        ("big_tree", (6, 0)),
        ("bush", (7, 5)),
        ("bush", (10, 8)),
        ("boulder", (17, 6)),
        ("crystal", (19, 9)),
        ("torch", (12, 10)),
        ("brazier", (24, 9)),
        ("table", (13, 11)),
        ("chest", (15, 12)),
        ("barrel", (14, 10)),
        ("bones", (21, 13)),
        ("stalagmite", (18, 13)),
        ("pillar", (16, 10)),
    ] {
        v.push(obj(a, c));
    }
    v
}

/// With `DND55_AUTO` set: prints `CAMERA-ZOOM: <target>` whenever the map zoom target changes and
/// `MAP-POINTER: <bool>` when the pointer enters/leaves the map canvas (used by the GUI wheel test).
pub fn log_camera(
    fit: Res<crate::input::CameraFit>,
    ptr: Res<crate::pointer::MapPointer>,
    mut last: Local<(Option<f32>, Option<bool>)>,
) {
    if std::env::var("DND55_AUTO").is_err() {
        return;
    }
    if fit.target_scale != last.0 {
        last.0 = fit.target_scale;
        if let Some(s) = fit.target_scale {
            println!("CAMERA-ZOOM: {s:.4}");
        }
    }
    if Some(ptr.over_map) != last.1 {
        last.1 = Some(ptr.over_map);
        println!("MAP-POINTER: {}", ptr.over_map);
    }
}
