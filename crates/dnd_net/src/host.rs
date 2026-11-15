//! The authoritative host. One task owns the [`Session`]; connection tasks only parse frames and
//! forward messages, so all game logic is serialized and race-free.

use crate::protocol::{self, ClientMsg, Hello, HostRequest, PROTOCOL_VERSION, ServerMsg};
use dnd_session::Session;
use dnd_session::model::{PlayerId, Principal};
use dnd_world::RoomId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::Instant;

/// Why the host is asking the application to persist the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveTrigger {
    Manual(String),
    Auto,
}

/// Called on the game task to persist the session (manual saves and autosaves). Returns a message for the DM.
pub type SaveHook = Box<dyn FnMut(&Session, SaveTrigger) -> Result<String, String> + Send>;

#[derive(Debug, Clone)]
pub struct HostConfig {
    pub bind: SocketAddr,
    pub session_code: String,
    pub dm_secret: String,
    pub autosave_every: Option<Duration>,
    /// Max messages per second per connection (burst = 2x).
    pub rate_limit: u32,
}

impl HostConfig {
    pub fn new(bind: SocketAddr) -> Self {
        HostConfig {
            bind,
            session_code: crate::new_session_code(),
            dm_secret: crate::random_token(),
            autosave_every: Some(Duration::from_secs(300)),
            rate_limit: 30,
        }
    }
}

pub struct HostHandle {
    pub local_addr: SocketAddr,
    pub session_code: String,
    pub dm_secret: String,
    shutdown: watch::Sender<bool>,
    spawner: Spawner,
    /// Relay link status (if [`HostHandle::link_relay`] was called).
    pub relay: Arc<std::sync::Mutex<RelayStatus>>,
}

/// Spawns connection handlers for streams from the listener or the relay.
#[derive(Clone)]
struct Spawner {
    in_tx: mpsc::Sender<Inbound>,
    next: Arc<AtomicU64>,
    rate: u32,
    shutdown: watch::Receiver<bool>,
}

impl Spawner {
    fn spawn(&self, stream: tokio::net::TcpStream) {
        let conn = self.next.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(connection(conn, stream, self.in_tx.clone(), self.rate, self.shutdown.clone()));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayStatus {
    Off,
    Connecting,
    /// Registered: players join with this relay invite code.
    Online { relay: String, invite: String },
    Failed(String),
}

impl HostHandle {
    pub fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }

    /// Registers this game on a relay server so players can join without port forwarding.
    /// Keeps retrying (with backoff) if the relay connection drops. Must be called inside the host's
    /// Tokio runtime.
    pub fn link_relay(&self, relay_addr: String) {
        *self.relay.lock().expect("relay status") = RelayStatus::Connecting;
        let status = self.relay.clone();
        let spawner = self.spawner.clone();
        let mut shutdown = self.spawner.shutdown.clone();
        tokio::spawn(async move {
            let mut backoff = 1u64;
            loop {
                let result = tokio::select! {
                    _ = shutdown.changed() => return,
                    r = relay_session(&relay_addr, &spawner, &status) => r,
                };
                let reason = match result {
                    Ok(()) => "relay connection closed".to_string(),
                    Err(e) => e.to_string(),
                };
                *status.lock().expect("relay status") = RelayStatus::Failed(format!("{reason} — retrying in {backoff}s"));
                tokio::select! {
                    _ = shutdown.changed() => return,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(backoff)) => {}
                }
                backoff = (backoff * 2).min(30);
                *status.lock().expect("relay status") = RelayStatus::Connecting;
            }
        });
    }
}

async fn relay_session(relay_addr: &str, spawner: &Spawner, status: &Arc<std::sync::Mutex<RelayStatus>>) -> std::io::Result<()> {
    use dnd_relay::{RELAY_PROTOCOL, RelayHello, RelayMsg};
    let mut control = tokio::time::timeout(Duration::from_secs(8), tokio::net::TcpStream::connect(relay_addr))
        .await
        .map_err(|_| std::io::Error::other(format!("timed out connecting to relay {relay_addr}")))??;
    let (invite, key) = match dnd_relay::handshake(&mut control, &RelayHello::Host { protocol: RELAY_PROTOCOL }).await? {
        RelayMsg::Registered { invite, host_key } => (invite, host_key),
        RelayMsg::Error { reason } => return Err(std::io::Error::other(reason)),
        other => return Err(std::io::Error::other(format!("unexpected relay reply {other:?}"))),
    };
    *status.lock().expect("relay status") = RelayStatus::Online { relay: relay_addr.to_string(), invite: invite.clone() };
    loop {
        match dnd_relay::read_msg(&mut control).await? {
            RelayMsg::Open { stream } => {
                let (relay_addr, invite, key, spawner) = (relay_addr.to_string(), invite.clone(), key.clone(), spawner.clone());
                tokio::spawn(async move {
                    let Ok(mut s) = tokio::net::TcpStream::connect(&relay_addr).await else { return };
                    let _ = s.set_nodelay(true);
                    let hello = RelayHello::Accept { protocol: RELAY_PROTOCOL, invite, stream, host_key: key };
                    if let Ok(RelayMsg::Paired) = dnd_relay::handshake(&mut s, &hello).await {
                        spawner.spawn(s);
                    }
                });
            }
            RelayMsg::Ping | RelayMsg::Paired | RelayMsg::Registered { .. } => {}
            RelayMsg::Error { reason } => return Err(std::io::Error::other(reason)),
        }
    }
}

type ConnId = u64;

enum Inbound {
    Hello {
        conn: ConnId,
        hello: Hello,
        tx: mpsc::Sender<ServerMsg>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Msg {
        conn: ConnId,
        msg: ClientMsg,
    },
    Gone {
        conn: ConnId,
    },
}

struct Conn {
    principal: Principal,
    tx: mpsc::Sender<ServerMsg>,
    known_map: Option<(RoomId, u64)>,
}

/// Starts the host on the current Tokio runtime.
pub async fn start(
    session: Session,
    config: HostConfig,
    save_hook: Option<SaveHook>,
) -> std::io::Result<HostHandle> {
    let listener = TcpListener::bind(config.bind).await?;
    let local_addr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (in_tx, in_rx) = mpsc::channel::<Inbound>(1024);

    tokio::spawn(game_loop(
        session,
        config.clone(),
        save_hook,
        in_rx,
        shutdown_rx.clone(),
    ));
    let spawner = Spawner { in_tx, next: Arc::new(AtomicU64::new(1)), rate: config.rate_limit, shutdown: shutdown_rx.clone() };
    tokio::spawn(accept_loop(listener, spawner.clone(), shutdown_rx));

    Ok(HostHandle {
        local_addr,
        session_code: config.session_code,
        dm_secret: config.dm_secret,
        shutdown: shutdown_tx,
        spawner,
        relay: Arc::new(std::sync::Mutex::new(RelayStatus::Off)),
    })
}

async fn accept_loop(listener: TcpListener, spawner: Spawner, mut shutdown: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            _ = shutdown.changed() => return,
            accepted = listener.accept() => {
                let Ok((stream, _addr)) = accepted else { continue };
                let _ = stream.set_nodelay(true);
                spawner.spawn(stream);
            }
        }
    }
}

async fn connection(
    conn: ConnId,
    stream: tokio::net::TcpStream,
    in_tx: mpsc::Sender<Inbound>,
    rate: u32,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut wire = protocol::framed(stream);
    // The first message must be Hello, within 10 seconds.
    let hello = match tokio::time::timeout(
        Duration::from_secs(10),
        protocol::recv::<ClientMsg>(&mut wire),
    )
    .await
    {
        Ok(Ok(ClientMsg::Hello(h))) => h,
        _ => {
            let _ = protocol::send(
                &mut wire,
                &ServerMsg::Rejected {
                    reason: "expected Hello".into(),
                },
            )
            .await;
            return;
        }
    };
    let (tx, mut rx) = mpsc::channel::<ServerMsg>(512);
    let (reply_tx, reply_rx) = oneshot::channel();
    if in_tx
        .send(Inbound::Hello {
            conn,
            hello,
            tx,
            reply: reply_tx,
        })
        .await
        .is_err()
    {
        return;
    }
    match reply_rx.await {
        Ok(Ok(())) => {}
        Ok(Err(reason)) => {
            let _ = protocol::send(&mut wire, &ServerMsg::Rejected { reason }).await;
            return;
        }
        Err(_) => return,
    }

    // Token bucket rate limiter.
    let capacity = (rate * 2) as f64;
    let mut tokens = capacity;
    let mut last = Instant::now();
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            out = rx.recv() => match out {
                Some(msg) => if protocol::send(&mut wire, &msg).await.is_err() { break },
                None => break, // host dropped us (kicked or slow)
            },
            incoming = protocol::recv::<ClientMsg>(&mut wire) => {
                let Ok(msg) = incoming else { break };
                let now = Instant::now();
                tokens = (tokens + now.duration_since(last).as_secs_f64() * rate as f64).min(capacity);
                last = now;
                if tokens < 1.0 {
                    let _ = protocol::send(&mut wire, &ServerMsg::Rejected { reason: "rate limit exceeded".into() }).await;
                    break;
                }
                tokens -= 1.0;
                if matches!(msg, ClientMsg::Hello(_)) {
                    break; // protocol violation
                }
                if in_tx.send(Inbound::Msg { conn, msg }).await.is_err() {
                    break;
                }
            }
        }
    }
    let _ = in_tx.send(Inbound::Gone { conn }).await;
}

struct Game {
    session: Session,
    config: HostConfig,
    save_hook: Option<SaveHook>,
    conns: HashMap<ConnId, Conn>,
    tokens: HashMap<String, PlayerId>,
    dirty_since_autosave: bool,
}

async fn game_loop(
    session: Session,
    config: HostConfig,
    save_hook: Option<SaveHook>,
    mut in_rx: mpsc::Receiver<Inbound>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut g = Game {
        session,
        config: config.clone(),
        save_hook,
        conns: HashMap::new(),
        tokens: HashMap::new(),
        dirty_since_autosave: false,
    };
    let period = config
        .autosave_every
        .unwrap_or(Duration::from_secs(3600 * 24 * 365));
    let mut autosave = tokio::time::interval_at(Instant::now() + period, period);
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                g.autosave();
                return;
            }
            _ = autosave.tick(), if config.autosave_every.is_some() => g.autosave(),
            msg = in_rx.recv() => match msg {
                None => return,
                Some(Inbound::Hello { conn, hello, tx, reply }) => {
                    match g.admit(&hello) {
                        Ok((principal, token)) => {
                            let _ = reply.send(Ok(()));
                            let _ = tx.try_send(ServerMsg::Welcome { you: principal, resume_token: token, campaign: g.session.state.campaign_name.clone() });
                            g.conns.insert(conn, Conn { principal, tx, known_map: None });
                            g.broadcast(&[]);
                        }
                        Err(reason) => { let _ = reply.send(Err(reason)); }
                    }
                }
                Some(Inbound::Msg { conn, msg }) => g.handle(conn, msg),
                Some(Inbound::Gone { conn }) => {
                    if g.drop_conn(conn) {
                        g.broadcast(&[]);
                    }
                }
            }
        }
    }
}

impl Game {
    fn admit(&mut self, h: &Hello) -> Result<(Principal, String), String> {
        if h.protocol != PROTOCOL_VERSION {
            return Err(format!(
                "version mismatch: host protocol {PROTOCOL_VERSION}, client protocol {}",
                h.protocol
            ));
        }
        if !h
            .session_code
            .trim()
            .eq_ignore_ascii_case(&self.config.session_code)
        {
            return Err("wrong session code".into());
        }
        if let Some(secret) = &h.dm_secret {
            return if *secret == self.config.dm_secret {
                Ok((Principal::Dm, secret.clone()))
            } else {
                Err("invalid DM credentials".into())
            };
        }
        if let Some(tok) = &h.resume_token
            && let Some(p) = self.tokens.get(tok).copied()
        {
            // Reclaim the seat: drop any stale connection for this player.
            self.conns
                .retain(|_, c| c.principal != Principal::Player(p));
            self.session.set_connected(p, true);
            return Ok((Principal::Player(p), tok.clone()));
        }
        let name = h.name.trim();
        if name.is_empty() || name.chars().count() > 32 || name.chars().any(|c| c.is_control()) {
            return Err("name must be 1–32 printable characters".into());
        }
        if self.session.find_player_by_name(name).is_some() {
            return Err("that name is already taken in this session".into());
        }
        let p = self.session.add_player(name);
        let token = crate::random_token();
        self.tokens.insert(token.clone(), p);
        Ok((Principal::Player(p), token))
    }

    fn handle(&mut self, conn: ConnId, msg: ClientMsg) {
        let Some(principal) = self.conns.get(&conn).map(|c| c.principal) else {
            return;
        };
        match msg {
            ClientMsg::Hello(_) => {}
            ClientMsg::Ping { t } => self.send(conn, ServerMsg::Pong { t }),
            ClientMsg::GetAsset { hash } => {
                if let Some(bytes) = self.session.assets.get(&hash) {
                    let bytes = bytes.to_vec();
                    self.send(conn, ServerMsg::Asset { hash, bytes });
                }
            }
            ClientMsg::Cmd { seq, command } => match self.session.apply(principal, command) {
                Ok(events) => {
                    self.dirty_since_autosave = true;
                    let travelled = events
                        .iter()
                        .any(|r| matches!(r.event, dnd_session::event::Event::RoomChanged { .. }));
                    self.send(conn, ServerMsg::CmdResult { seq, error: None });
                    self.broadcast(&events);
                    if travelled {
                        // Autosave on every room transition.
                        self.autosave();
                    }
                }
                Err(e) => self.send(
                    conn,
                    ServerMsg::CmdResult {
                        seq,
                        error: Some(e.to_string()),
                    },
                ),
            },
            ClientMsg::Host { seq, request } => {
                if principal != Principal::Dm {
                    self.send(
                        conn,
                        ServerMsg::CmdResult {
                            seq,
                            error: Some("not authorized".into()),
                        },
                    );
                    return;
                }
                match request {
                    HostRequest::Save { label } => {
                        let r = match self.save_hook.as_mut() {
                            Some(hook) => hook(&self.session, SaveTrigger::Manual(label)),
                            None => Err("saving is not configured".into()),
                        };
                        let error = r.err();
                        if error.is_none() {
                            self.dirty_since_autosave = false;
                        }
                        self.send(conn, ServerMsg::CmdResult { seq, error });
                    }
                }
            }
        }
    }

    fn autosave(&mut self) {
        if !self.dirty_since_autosave {
            return;
        }
        if let Some(hook) = self.save_hook.as_mut()
            && hook(&self.session, SaveTrigger::Auto).is_ok()
        {
            self.dirty_since_autosave = false;
        }
    }

    /// Removes a connection. Returns true if a player thereby became disconnected (views must update).
    fn drop_conn(&mut self, conn: ConnId) -> bool {
        let Some(c) = self.conns.remove(&conn) else { return false };
        if let Principal::Player(p) = c.principal
            && !self.conns.values().any(|o| o.principal == c.principal)
        {
            self.session.set_connected(p, false);
            return true;
        }
        false
    }

    fn send(&mut self, conn: ConnId, msg: ServerMsg) {
        if let Some(c) = self.conns.get(&conn)
            && c.tx.try_send(msg).is_err()
        {
            // Slow or dead consumer: drop it; it can reconnect with its resume token.
            if self.drop_conn(conn) {
                self.broadcast(&[]);
            }
        }
    }

    /// Sends each connection its filtered events and an updated view.
    fn broadcast(&mut self, events: &[dnd_session::event::Routed]) {
        let mut dead = Vec::new();
        for (id, c) in self.conns.iter_mut() {
            let visible = self.session.visible_creature_ids(c.principal);
            let evs = self.session.events_for(c.principal, events, &visible);
            if !evs.is_empty() && c.tx.try_send(ServerMsg::Events(evs)).is_err() {
                dead.push(*id);
                continue;
            }
            let view = self.session.build_view(c.principal, c.known_map);
            if let Some(r) = &view.room {
                c.known_map = Some((r.id, r.map_rev));
            }
            if c.tx.try_send(ServerMsg::View(Box::new(view))).is_err() {
                dead.push(*id);
            }
        }
        let mut changed = false;
        for id in dead {
            changed |= self.drop_conn(id);
        }
        if changed {
            // Tell everyone else that a player dropped (bounded: dead connections are gone now).
            self.broadcast(&[]);
        }
    }
}
