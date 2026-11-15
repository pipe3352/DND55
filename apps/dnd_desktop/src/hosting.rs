//! Starting an in-process authoritative host (DM) or connecting to one (player).

use crate::state::{HostRuntime, Net};
use dnd_net::host::{self, HostConfig, SaveTrigger};
use dnd_net::{Hello, Invite, NetClient, PROTOCOL_VERSION, RelayStatus, Target};
use dnd_persist::{SaveKind, Store};
use dnd_session::Session;
use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    crate::platform::data_dir()
}

pub fn open_store() -> Result<Store, String> {
    Store::open(&data_dir().join("campaigns.db")).map_err(|e| e.to_string())
}

/// Starts hosting `session` on `port` and connects the DM's own client to it.
pub fn host_session(
    session: Session,
    campaign_id: i64,
    port: u16,
    net: &mut Net,
) -> Result<HostRuntime, String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let mut store = open_store()?;
    let hook: host::SaveHook = Box::new(move |s: &Session, trigger| {
        let (kind, label) = match trigger {
            SaveTrigger::Manual(l) => (
                SaveKind::Manual,
                if l.trim().is_empty() {
                    "Manual save".to_string()
                } else {
                    l
                },
            ),
            SaveTrigger::Auto => (SaveKind::Auto, "Autosave".to_string()),
        };
        store
            .save(campaign_id, kind, &label, &s.state, &s.assets)
            .map(|id| format!("saved #{id}"))
            .map_err(|e| e.to_string())
    });
    let mut cfg = HostConfig::new(([0, 0, 0, 0], port).into());
    cfg.autosave_every = Some(std::time::Duration::from_secs(180));
    // Developer override for scripted multi-client tests.
    if let Ok(code) = std::env::var("DND55_SESSION_CODE")
        && !code.is_empty()
    {
        cfg.session_code = code;
    }
    let handle = runtime
        .block_on(host::start(session, cfg, Some(hook)))
        .map_err(|e| format!("could not listen on port {port}: {e}"))?;
    let hello = Hello {
        protocol: PROTOCOL_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        session_code: handle.session_code.clone(),
        name: "DM".into(),
        resume_token: None,
        dm_secret: Some(handle.dm_secret.clone()),
    };
    let addr = format!("127.0.0.1:{}", handle.local_addr.port());
    connect(net, Target::Direct(addr), hello);
    Ok(HostRuntime {
        runtime,
        handle,
        campaign_id,
        upnp: Default::default(),
    })
}

fn connect(net: &mut Net, target: Target, hello: Hello) {
    net.client = Some(NetClient::connect_target(target.clone(), hello.clone()));
    net.address = match &target {
        Target::Direct(a) => a.clone(),
        Target::Relay { relay, .. } => relay.clone(),
    };
    net.target = Some(target);
    net.hello = Some(hello);
    net.status = "Connecting…".into();
    net.rejected = false;
    net.retry_at = None;
}

/// Registers the hosted game on a relay server (internet play without port forwarding).
pub fn go_online(host: &HostRuntime, relay: &str) {
    let relay = relay.trim().to_string();
    let _guard = host.runtime.enter();
    host.handle.link_relay(relay);
}

/// Tries to open the host port on the router (UPnP) in the background.
pub fn start_upnp(host: &HostRuntime) {
    let slot = host.upnp.clone();
    let port = host.handle.local_addr.port();
    *slot.lock().expect("upnp") = None;
    std::thread::Builder::new()
        .name("upnp".into())
        .spawn(move || {
            let r = dnd_net::upnp::map_port(port);
            *slot.lock().expect("upnp") = Some(r);
        })
        .ok();
}

/// Invite strings the DM can share.
pub fn invites(host: &HostRuntime) -> Vec<(&'static str, String)> {
    let code = &host.handle.session_code;
    let port = host.handle.local_addr.port();
    let mut v = vec![];
    if let RelayStatus::Online { relay, invite } = host.handle.relay.lock().expect("relay").clone()
    {
        v.push((
            "invite.online",
            Invite::relay(&relay, &invite, code).to_string(),
        ));
    }
    if let Some(Ok(ext)) = host.upnp.lock().expect("upnp").clone() {
        v.push((
            "invite.internet_direct",
            Invite::direct(&ext.to_string(), code).to_string(),
        ));
    }
    if let Some(ip) = dnd_net::invite::lan_address() {
        v.push((
            "invite.lan",
            Invite::direct(&format!("{ip}:{port}"), code).to_string(),
        ));
    }
    v.push((
        "invite.same_pc",
        Invite::direct(&format!("127.0.0.1:{port}"), code).to_string(),
    ));
    v
}

/// Joins with either an invite string (`dnd55://…`) or a plain `host:port` plus a session code.
pub fn join(net: &mut Net, address_or_invite: &str, code: &str, name: &str) -> Result<(), String> {
    let (target, code) = match Invite::parse(address_or_invite) {
        Some(inv) => (inv.target, inv.session_code),
        None => {
            let a = address_or_invite.trim();
            if a.starts_with("dnd55://") {
                return Err(
                    "That invite link is incomplete or damaged — ask the DM to copy it again."
                        .into(),
                );
            }
            if !a.contains(':') {
                return Err("Enter the DM's address as host:port (for example 192.168.1.20:7878) or paste an invite link.".into());
            }
            if code.trim().is_empty() {
                return Err("Enter the session code shown in the DM's top bar.".into());
            }
            (Target::Direct(a.to_string()), code.trim().to_uppercase())
        }
    };
    let hello = Hello {
        protocol: PROTOCOL_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        session_code: code,
        name: name.trim().into(),
        resume_token: None,
        dm_secret: None,
    };
    net.resume_token = None;
    net.retries = 0;
    connect(net, target, hello);
    Ok(())
}

/// Reconnects with the stored resume token after a dropped connection.
pub fn reconnect(net: &mut Net) {
    if let (Some(mut hello), Some(target)) = (net.hello.clone(), net.target.clone()) {
        hello.resume_token = net.resume_token.clone().or(hello.resume_token);
        let retries = net.retries;
        connect(net, target, hello);
        net.retries = retries;
        net.status = "Reconnecting…".into();
    }
}
