//! Starting an in-process authoritative host (DM) or connecting to one (player).

use crate::state::{HostRuntime, Net};
use dnd_net::host::{self, HostConfig, SaveTrigger};
use dnd_net::{Hello, NetClient, PROTOCOL_VERSION};
use dnd_persist::{SaveKind, Store};
use dnd_session::Session;
use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("DND55_DATA_DIR") {
        return PathBuf::from(p);
    }
    #[cfg(windows)]
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("dnd55");
    }
    if let Ok(x) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(x).join("dnd55");
    }
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".local/share/dnd55"))
        .unwrap_or_else(|_| PathBuf::from("dnd55-data"))
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
    net.client = Some(NetClient::connect(addr.clone(), hello.clone()));
    net.address = addr;
    net.hello = Some(hello);
    net.status = "Connecting…".into();
    Ok(HostRuntime {
        runtime,
        handle,
        campaign_id,
    })
}

pub fn join(net: &mut Net, address: &str, code: &str, name: &str) {
    let hello = Hello {
        protocol: PROTOCOL_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        session_code: code.trim().to_uppercase(),
        name: name.trim().into(),
        resume_token: None,
        dm_secret: None,
    };
    net.client = Some(NetClient::connect(
        address.trim().to_string(),
        hello.clone(),
    ));
    net.address = address.trim().into();
    net.hello = Some(hello);
    net.status = "Connecting…".into();
}

/// Reconnects with the stored resume token after a dropped connection.
pub fn reconnect(net: &mut Net) {
    if let Some(mut hello) = net.hello.clone() {
        hello.resume_token = net.resume_token.clone().or(hello.resume_token);
        net.client = Some(NetClient::connect(net.address.clone(), hello));
        net.status = "Reconnecting…".into();
    }
}
