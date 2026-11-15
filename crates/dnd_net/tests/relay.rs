//! Players joining through the relay (no direct connection to the host's port).

use dnd_net::host::{self, HostConfig};
use dnd_net::{
    ClientMsg, Hello, Invite, NetClient, NetEvent, PROTOCOL_VERSION, RelayStatus, ServerMsg, Target,
};
use dnd_session::command::Command;
use dnd_session::model::Principal;
use std::time::{Duration, Instant};

fn wait<T>(c: &NetClient, mut pred: impl FnMut(&ServerMsg) -> Option<T>) -> Result<T, String> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match c.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Some(NetEvent::Message(m)) => {
                if let Some(v) = pred(&m) {
                    return Ok(v);
                }
            }
            Some(NetEvent::Connected) => {}
            Some(NetEvent::Disconnected(r)) => return Err(r),
            None => return Err("timeout".into()),
        }
    }
}

#[test]
fn join_and_rejoin_through_the_relay() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    // 1. A relay server.
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    rt.spawn(dnd_relay::run(
        dnd_relay::RelayConfig::new("127.0.0.1:0".parse().unwrap()),
        Some(ready_tx),
    ));
    let relay_addr = rt.block_on(ready_rx).unwrap().to_string();

    // 2. The DM hosts; the game registers on the relay.
    let mut cfg = HostConfig::new("127.0.0.1:0".parse().unwrap());
    cfg.autosave_every = None;
    let handle = rt
        .block_on(host::start(dnd_session::demo::demo_campaign(), cfg, None))
        .unwrap();
    {
        let _g = rt.enter();
        handle.link_relay(relay_addr.clone());
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let invite_code = loop {
        if let RelayStatus::Online { invite, .. } = handle.relay.lock().unwrap().clone() {
            break invite;
        }
        assert!(
            Instant::now() < deadline,
            "relay registration timed out: {:?}",
            handle.relay.lock().unwrap()
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let invite = Invite::relay(&relay_addr, &invite_code, &handle.session_code);
    let parsed = Invite::parse(&invite.to_string()).unwrap();

    // 3. A player joins knowing only the invite string.
    let hello = |name: &str, token: Option<String>| Hello {
        protocol: PROTOCOL_VERSION,
        app_version: "t".into(),
        session_code: parsed.session_code.clone(),
        name: name.into(),
        resume_token: token,
        dm_secret: None,
    };
    let alice = NetClient::connect_target(parsed.target.clone(), hello("Alice", None));
    let (me, token) = wait(&alice, |m| match m {
        ServerMsg::Welcome {
            you, resume_token, ..
        } => Some((*you, resume_token.clone())),
        _ => None,
    })
    .unwrap();
    assert!(matches!(me, Principal::Player(_)));
    alice.send(ClientMsg::Cmd {
        seq: 1,
        command: Command::SubmitCharacter {
            sheet: Box::new(dnd_rules::character::CharacterSheet::example_fighter(
                "Alice",
            )),
            portrait_png: None,
        },
    });
    let r = wait(&alice, |m| match m {
        ServerMsg::CmdResult { seq: 1, error } => Some(error.clone()),
        _ => None,
    })
    .unwrap();
    assert_eq!(r, None, "commands work end-to-end through the relay");
    wait(&alice, |m| match m {
        ServerMsg::View(v) if !v.my_characters.is_empty() => Some(()),
        _ => None,
    })
    .unwrap();

    // 4. Wrong invite code: clear error from the relay.
    let bad = NetClient::connect_target(
        Target::Relay {
            relay: relay_addr.clone(),
            invite: "NOPE2345".into(),
        },
        hello("Eve", None),
    );
    let err = wait(&bad, |_| None::<()>).unwrap_err();
    assert!(err.contains("no game with that invite code"), "{err}");

    // 5. Reconnect through the relay with the resume token.
    drop(alice);
    std::thread::sleep(Duration::from_millis(200));
    let again = NetClient::connect_target(parsed.target.clone(), hello("Alice", Some(token)));
    let me2 = wait(&again, |m| match m {
        ServerMsg::Welcome { you, .. } => Some(*you),
        _ => None,
    })
    .unwrap();
    assert_eq!(me, me2);
    handle.shutdown();
}
