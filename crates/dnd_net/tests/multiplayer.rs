//! Multiplayer integration tests over real TCP sockets on localhost.

use dnd_net::host::{self, HostConfig, SaveTrigger};
use dnd_net::{ClientMsg, Hello, HostRequest, NetClient, NetEvent, PROTOCOL_VERSION, ServerMsg};
use dnd_rules::character::CharacterSheet;
use dnd_session::command::{Command, DmCommand};
use dnd_session::demo::demo_campaign;
use dnd_session::event::Event;
use dnd_session::model::Principal;
use dnd_session::view::ClientView;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Setup {
    _rt: tokio::runtime::Runtime,
    handle: host::HostHandle,
    saves: Arc<Mutex<Vec<String>>>,
}

fn start_host() -> Setup {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let saves = Arc::new(Mutex::new(Vec::new()));
    let saves2 = saves.clone();
    let hook: host::SaveHook = Box::new(move |s, trigger| {
        let label = match trigger {
            SaveTrigger::Manual(l) => l,
            SaveTrigger::Auto => "auto".into(),
        };
        saves2
            .lock()
            .unwrap()
            .push(format!("{label}:{}", s.state.campaign_name));
        Ok("saved".into())
    });
    let mut cfg = HostConfig::new("127.0.0.1:0".parse().unwrap());
    cfg.autosave_every = None;
    let handle = rt
        .block_on(host::start(demo_campaign(), cfg, Some(hook)))
        .unwrap();
    Setup {
        _rt: rt,
        handle,
        saves,
    }
}

fn hello(s: &Setup, name: &str) -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        app_version: "test".into(),
        session_code: s.handle.session_code.clone(),
        name: name.into(),
        resume_token: None,
        dm_secret: None,
    }
}

fn connect(s: &Setup, h: Hello) -> NetClient {
    NetClient::connect(s.handle.local_addr.to_string(), h)
}

/// Waits for the first message matching `pred`, failing after 5 s.
fn wait<T>(c: &NetClient, mut pred: impl FnMut(&ServerMsg) -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match c.recv_timeout(left).ok_or(()) {
            Ok(NetEvent::Message(m)) => {
                if let Some(v) = pred(&m) {
                    return v;
                }
            }
            Ok(NetEvent::Connected) => {}
            Ok(NetEvent::Disconnected(r)) => panic!("disconnected: {r}"),
            Err(_) => panic!("timed out waiting for message"),
        }
    }
}

fn wait_view(c: &NetClient, mut pred: impl FnMut(&ClientView) -> bool) -> ClientView {
    wait(c, |m| match m {
        ServerMsg::View(v) if pred(v) => Some((**v).clone()),
        _ => None,
    })
}

fn wait_result(c: &NetClient, seq: u64) -> Option<String> {
    wait(c, |m| match m {
        ServerMsg::CmdResult { seq: s, error } if *s == seq => Some(error.clone()),
        _ => None,
    })
}

fn rejected(c: &NetClient) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match c
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .ok_or(())
        {
            Ok(NetEvent::Message(ServerMsg::Rejected { reason })) => return reason,
            Ok(_) => {}
            Err(_) => panic!("expected rejection"),
        }
    }
}

#[test]
fn join_play_and_hidden_information_over_tcp() {
    let s = start_host();
    let dm = connect(
        &s,
        Hello {
            dm_secret: Some(s.handle.dm_secret.clone()),
            name: "DM".into(),
            ..hello(&s, "DM")
        },
    );
    assert_eq!(
        wait(&dm, |m| match m {
            ServerMsg::Welcome { you, .. } => Some(*you),
            _ => None,
        }),
        Principal::Dm
    );

    let alice = connect(&s, hello(&s, "Alice"));
    let me = wait(&alice, |m| match m {
        ServerMsg::Welcome { you, .. } => Some(*you),
        _ => None,
    });
    assert!(matches!(me, Principal::Player(_)));
    wait_view(&dm, |v| v.players.iter().any(|p| p.name == "Alice"));

    // Player creates a character with a portrait.
    let png = { image_png() };
    alice.send(ClientMsg::Cmd {
        seq: 1,
        command: Command::SubmitCharacter {
            sheet: Box::new(CharacterSheet::example_fighter("Alice")),
            portrait_png: Some(png),
        },
    });
    assert_eq!(wait_result(&alice, 1), None);
    let v = wait_view(&alice, |v| !v.my_characters.is_empty());
    let my_id = v.my_characters[0].0;
    let portrait = v
        .creatures
        .iter()
        .find(|c| c.id == my_id)
        .unwrap()
        .portrait
        .clone()
        .expect("portrait hash");
    alice.send(ClientMsg::GetAsset {
        hash: portrait.clone(),
    });
    let bytes = wait(&alice, |m| match m {
        ServerMsg::Asset { hash, bytes } if *hash == portrait => Some(bytes.clone()),
        _ => None,
    });
    assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));

    // Player can't issue DM commands over the wire either.
    alice.send(ClientMsg::Cmd {
        seq: 2,
        command: Command::Dm(DmCommand::StartSession),
    });
    assert_eq!(wait_result(&alice, 2), Some("not authorized".into()));
    alice.send(ClientMsg::Host {
        seq: 3,
        request: HostRequest::Save { label: "x".into() },
    });
    assert_eq!(wait_result(&alice, 3), Some("not authorized".into()));

    // DM spawns a hidden goblin: the player's stream must never mention it.
    let room = v.room.as_ref().unwrap().id;
    dm.send(ClientMsg::Cmd {
        seq: 10,
        command: Command::Dm(DmCommand::SpawnMonster {
            template: "goblin_warrior".into(),
            room,
            cell: (10, 3),
            hidden: true,
        }),
    });
    assert_eq!(wait_result(&dm, 10), None);
    let dmv = wait_view(&dm, |v| {
        v.creatures.iter().any(|c| c.name.starts_with("Goblin"))
    });
    let goblin = dmv
        .creatures
        .iter()
        .find(|c| c.name.starts_with("Goblin"))
        .unwrap()
        .id;

    // Movement syncs to the DM.
    alice.send(ClientMsg::Cmd {
        seq: 4,
        command: Command::Move {
            entity: my_id,
            to: (5, 3),
        },
    });
    assert_eq!(wait_result(&alice, 4), None);
    wait(&dm, |m| match m {
        ServerMsg::Events(e)
            if e.iter()
                .any(|e| matches!(e, Event::Moved { entity, .. } if *entity == my_id)) =>
        {
            Some(())
        }
        _ => None,
    });
    let v = wait_view(&alice, |v| {
        v.creatures.iter().any(|c| c.id == my_id && c.pos == (5, 3))
    });
    assert!(!serde_json::to_string(&v).unwrap().contains("Goblin"));

    // Dice from both sides.
    alice.send(ClientMsg::Cmd {
        seq: 5,
        command: Command::RollDice {
            expr: "1d20+5".into(),
            mode: Default::default(),
            private: false,
            label: "Perception".into(),
        },
    });
    wait(&dm, |m| match m {
        ServerMsg::Events(e)
            if e.iter()
                .any(|e| matches!(e, Event::DiceRolled { label, .. } if label == "Perception")) =>
        {
            Some(())
        }
        _ => None,
    });

    // Reveal and fight.
    dm.send(ClientMsg::Cmd {
        seq: 11,
        command: Command::Dm(DmCommand::SetCreatureHidden {
            entity: goblin,
            hidden: false,
        }),
    });
    wait_view(&alice, |v| v.creatures.iter().any(|c| c.id == goblin));
    dm.send(ClientMsg::Cmd {
        seq: 12,
        command: Command::Dm(DmCommand::StartCombat { surprised: vec![] }),
    });
    assert_eq!(wait_result(&dm, 12), None);
    wait_view(&alice, |v| v.combat.is_some());

    // DM saves through the host hook.
    dm.send(ClientMsg::Host {
        seq: 13,
        request: HostRequest::Save {
            label: "Session 1".into(),
        },
    });
    assert_eq!(wait_result(&dm, 13), None);
    assert_eq!(
        s.saves.lock().unwrap().as_slice(),
        ["Session 1:The Sunken Crypt"]
    );
    s.handle.shutdown();
}

#[test]
fn rejects_wrong_code_version_name_and_dm_secret() {
    let s = start_host();
    let bad_code = connect(
        &s,
        Hello {
            session_code: "WRONG1".into(),
            ..hello(&s, "Eve")
        },
    );
    assert_eq!(rejected(&bad_code), "wrong session code");
    let bad_version = connect(
        &s,
        Hello {
            protocol: 999,
            ..hello(&s, "Eve")
        },
    );
    assert!(rejected(&bad_version).contains("version mismatch"));
    let fake_dm = connect(
        &s,
        Hello {
            dm_secret: Some("guess".into()),
            ..hello(&s, "Eve")
        },
    );
    assert_eq!(rejected(&fake_dm), "invalid DM credentials");
    let a = connect(&s, hello(&s, "Bob"));
    wait(&a, |m| matches!(m, ServerMsg::Welcome { .. }).then_some(()));
    let dup = connect(&s, hello(&s, "bob"));
    assert!(rejected(&dup).contains("already taken"));
    s.handle.shutdown();
}

#[test]
fn reconnect_with_resume_token_restores_seat() {
    let s = start_host();
    let dm = connect(
        &s,
        Hello {
            dm_secret: Some(s.handle.dm_secret.clone()),
            ..hello(&s, "DM")
        },
    );
    wait(&dm, |m| {
        matches!(m, ServerMsg::Welcome { .. }).then_some(())
    });
    let alice = connect(&s, hello(&s, "Alice"));
    let (me, token) = wait(&alice, |m| match m {
        ServerMsg::Welcome {
            you, resume_token, ..
        } => Some((*you, resume_token.clone())),
        _ => None,
    });
    alice.send(ClientMsg::Cmd {
        seq: 1,
        command: Command::SubmitCharacter {
            sheet: Box::new(CharacterSheet::example_fighter("Alice")),
            portrait_png: None,
        },
    });
    assert_eq!(wait_result(&alice, 1), None);
    drop(alice);
    wait_view(&dm, |v| {
        v.players.iter().any(|p| p.name == "Alice" && !p.connected)
    });

    let again = connect(
        &s,
        Hello {
            resume_token: Some(token),
            name: "whatever".into(),
            ..hello(&s, "Alice")
        },
    );
    let me2 = wait(&again, |m| match m {
        ServerMsg::Welcome { you, .. } => Some(*you),
        _ => None,
    });
    assert_eq!(me, me2);
    let v = wait_view(&again, |v| !v.my_characters.is_empty());
    assert_eq!(v.my_characters[0].1.name, "Alice");
    wait_view(&dm, |v| {
        v.players.iter().any(|p| p.name == "Alice" && p.connected)
    });
    s.handle.shutdown();
}

#[test]
fn garbage_frames_drop_the_connection_without_hurting_the_host() {
    use std::io::Write;
    let s = start_host();
    let mut raw = std::net::TcpStream::connect(s.handle.local_addr).unwrap();
    raw.write_all(&[0, 0, 0, 5, b'h', b'e', b'l', b'l', b'o'])
        .unwrap();
    raw.write_all(&[0xff, 0xff, 0xff, 0xff]).unwrap(); // absurd frame length
    drop(raw);
    // The host still serves real clients.
    let a = connect(&s, hello(&s, "Carol"));
    wait(&a, |m| matches!(m, ServerMsg::Welcome { .. }).then_some(()));
    s.handle.shutdown();
}

fn image_png() -> Vec<u8> {
    // 2x2 opaque PNG generated by the host's own validator path would be circular; hand-build with a tiny encoder.
    // Use the PNG produced by the `image` crate through dnd_session's dependency instead.
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([10, 200, 10, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}
