//! # dnd_relay — rendezvous + byte relay for DND55 sessions
//!
//! Players behind home routers usually cannot accept incoming connections. The relay is a small
//! public server both sides connect *out* to:
//!
//! ```text
//!   DM's DND55 ──control──▶ relay ◀──join── player's DND55
//!        │  ◀── "open stream 7" ──┘
//!        └──accept(stream 7)──▶ relay ══ splices bytes ══▶ player
//! ```
//!
//! 1. The host connects and sends [`RelayHello::Host`]; the relay answers with an 8-character
//!    invite code and a secret host key, and keeps the control connection open.
//! 2. A player connects with [`RelayHello::Join`]`{ invite }`; the relay tells the host
//!    [`RelayMsg::Open`]`{ stream }`.
//! 3. The host opens a new connection with [`RelayHello::Accept`]`{ invite, stream, host_key }`;
//!    the relay replies [`RelayMsg::Paired`] to both and then copies bytes between the two sockets.
//!
//! After pairing, the normal DND55 protocol runs end to end (session code, DM authority, hidden
//! information filtering all stay on the DM's machine). The relay never sees the DM secret, but it
//! can read the game traffic it forwards (no end-to-end encryption yet): run your own relay or use
//! one you trust.

use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub const RELAY_PROTOCOL: u32 = 1;
pub const DEFAULT_PORT: u16 = 7979;
const MAX_HANDSHAKE: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayHello {
    Host {
        protocol: u32,
    },
    Join {
        protocol: u32,
        invite: String,
    },
    Accept {
        protocol: u32,
        invite: String,
        stream: u64,
        host_key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayMsg {
    Registered { invite: String, host_key: String },
    Open { stream: u64 },
    Paired,
    Error { reason: String },
    Ping,
}

#[derive(Debug, Clone)]
pub struct RelayConfig {
    pub bind: SocketAddr,
    pub max_hosts: usize,
    pub max_streams_per_host: usize,
    /// How long a joining player waits for the host to accept.
    pub pair_timeout: Duration,
}

impl RelayConfig {
    pub fn new(bind: SocketAddr) -> Self {
        RelayConfig {
            bind,
            max_hosts: 200,
            max_streams_per_host: 16,
            pair_timeout: Duration::from_secs(10),
        }
    }
}

struct HostEntry {
    key: String,
    control: mpsc::Sender<RelayMsg>,
    streams: usize,
}

#[derive(Default)]
struct State {
    hosts: HashMap<String, HostEntry>,
    pending: HashMap<u64, oneshot::Sender<TcpStream>>,
    next_stream: u64,
}

type Shared = Arc<Mutex<State>>;

fn framed(s: TcpStream) -> Framed<TcpStream, LengthDelimitedCodec> {
    Framed::new(
        s,
        LengthDelimitedCodec::builder()
            .max_frame_length(MAX_HANDSHAKE)
            .new_codec(),
    )
}

async fn send(
    w: &mut Framed<TcpStream, LengthDelimitedCodec>,
    m: &RelayMsg,
) -> std::io::Result<()> {
    w.send(Bytes::from(
        serde_json::to_vec(m).map_err(std::io::Error::other)?,
    ))
    .await
}

/// Sends the first relay message on a raw stream (used by clients and hosts).
pub async fn handshake(stream: &mut TcpStream, hello: &RelayHello) -> std::io::Result<RelayMsg> {
    use tokio::io::AsyncWriteExt;
    let body = serde_json::to_vec(hello).map_err(std::io::Error::other)?;
    stream.write_all(&(body.len() as u32).to_be_bytes()).await?;
    stream.write_all(&body).await?;
    read_msg(stream).await
}

/// Reads one length-prefixed relay message from a raw stream without over-reading.
pub async fn read_msg(stream: &mut TcpStream) -> std::io::Result<RelayMsg> {
    use tokio::io::AsyncReadExt;
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).await?;
    let n = u32::from_be_bytes(len) as usize;
    if n > MAX_HANDSHAKE {
        return Err(std::io::Error::other("relay message too large"));
    }
    let mut buf = vec![0u8; n];
    stream.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf).map_err(std::io::Error::other)
}

fn random_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).expect("OS entropy");
    hex::encode(b)
}

fn invite_code() -> String {
    const A: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("OS entropy");
    b.iter().map(|v| A[*v as usize % A.len()] as char).collect()
}

/// Runs a relay until the listener fails. Returns the bound address via `ready`.
pub async fn run(
    cfg: RelayConfig,
    ready: Option<oneshot::Sender<SocketAddr>>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(cfg.bind).await?;
    if let Some(r) = ready {
        let _ = r.send(listener.local_addr()?);
    }
    let state: Shared = Arc::new(Mutex::new(State::default()));
    let cfg = Arc::new(cfg);
    loop {
        let (stream, _) = listener.accept().await?;
        let _ = stream.set_nodelay(true);
        let (state, cfg) = (state.clone(), cfg.clone());
        tokio::spawn(async move {
            let _ = handle(stream, state, cfg).await;
        });
    }
}

async fn handle(
    mut stream: TcpStream,
    state: Shared,
    cfg: Arc<RelayConfig>,
) -> std::io::Result<()> {
    use tokio::io::AsyncReadExt;
    // First frame: the hello (read manually so nothing beyond it is buffered).
    let mut len = [0u8; 4];
    tokio::time::timeout(Duration::from_secs(10), stream.read_exact(&mut len))
        .await
        .map_err(|_| std::io::Error::other("timeout"))??;
    let n = u32::from_be_bytes(len) as usize;
    if n > MAX_HANDSHAKE {
        return Ok(());
    }
    let mut buf = vec![0u8; n];
    stream.read_exact(&mut buf).await?;
    let Ok(hello) = serde_json::from_slice::<RelayHello>(&buf) else {
        return Ok(());
    };
    match hello {
        RelayHello::Host { protocol } => {
            let mut w = framed(stream);
            if protocol != RELAY_PROTOCOL {
                return send(
                    &mut w,
                    &RelayMsg::Error {
                        reason: format!("relay protocol {RELAY_PROTOCOL} required"),
                    },
                )
                .await;
            }
            let (tx, mut rx) = mpsc::channel::<RelayMsg>(64);
            let (invite, key) = {
                let mut st = state.lock().await;
                if st.hosts.len() >= cfg.max_hosts {
                    drop(st);
                    return send(
                        &mut w,
                        &RelayMsg::Error {
                            reason: "relay is full, try again later".into(),
                        },
                    )
                    .await;
                }
                let mut invite = invite_code();
                while st.hosts.contains_key(&invite) {
                    invite = invite_code();
                }
                let key = random_hex(16);
                st.hosts.insert(
                    invite.clone(),
                    HostEntry {
                        key: key.clone(),
                        control: tx,
                        streams: 0,
                    },
                );
                (invite, key)
            };
            send(
                &mut w,
                &RelayMsg::Registered {
                    invite: invite.clone(),
                    host_key: key,
                },
            )
            .await?;
            let mut ping = tokio::time::interval(Duration::from_secs(20));
            loop {
                tokio::select! {
                    m = rx.recv() => match m {
                        Some(m) => if send(&mut w, &m).await.is_err() { break },
                        None => break,
                    },
                    _ = ping.tick() => if send(&mut w, &RelayMsg::Ping).await.is_err() { break },
                    incoming = w.next() => if incoming.is_none() || matches!(incoming, Some(Err(_))) { break },
                }
            }
            state.lock().await.hosts.remove(&invite);
            Ok(())
        }
        RelayHello::Join { protocol, invite } => {
            let invite = invite.trim().to_uppercase();
            let reply_err = |reason: &str| RelayMsg::Error {
                reason: reason.into(),
            };
            if protocol != RELAY_PROTOCOL {
                let mut w = framed(stream);
                return send(&mut w, &reply_err("relay protocol mismatch: update DND55")).await;
            }
            let (tx, rx) = oneshot::channel();
            let stream_id = {
                let mut st = state.lock().await;
                st.next_stream += 1;
                let id = st.next_stream;
                let control = match st.hosts.get_mut(&invite) {
                    Some(h) if h.streams < cfg.max_streams_per_host => {
                        h.streams += 1;
                        h.control.clone()
                    }
                    Some(_) => {
                        drop(st);
                        let mut w = framed(stream);
                        return send(&mut w, &reply_err("this table is full")).await;
                    }
                    None => {
                        drop(st);
                        let mut w = framed(stream);
                        return send(
                            &mut w,
                            &reply_err("no game with that invite code is running on this relay"),
                        )
                        .await;
                    }
                };
                st.pending.insert(id, tx);
                let _ = control.try_send(RelayMsg::Open { stream: id });
                id
            };
            let host_side = tokio::time::timeout(cfg.pair_timeout, rx).await;
            {
                let mut st = state.lock().await;
                st.pending.remove(&stream_id);
            }
            let Ok(Ok(mut host_stream)) = host_side else {
                if let Some(h) = state.lock().await.hosts.get_mut(&invite) {
                    h.streams = h.streams.saturating_sub(1);
                }
                let mut w = framed(stream);
                return send(&mut w, &reply_err("the host did not answer")).await;
            };
            write_raw(&mut stream, &RelayMsg::Paired).await?;
            write_raw(&mut host_stream, &RelayMsg::Paired).await?;
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut host_stream).await;
            if let Some(h) = state.lock().await.hosts.get_mut(&invite) {
                h.streams = h.streams.saturating_sub(1);
            }
            Ok(())
        }
        RelayHello::Accept {
            invite,
            stream: id,
            host_key,
            ..
        } => {
            let mut st = state.lock().await;
            let valid = st
                .hosts
                .get(&invite.trim().to_uppercase())
                .is_some_and(|h| h.key == host_key);
            if valid && let Some(tx) = st.pending.remove(&id) {
                let _ = tx.send(stream);
            }
            Ok(())
        }
    }
}

async fn write_raw(stream: &mut TcpStream, m: &RelayMsg) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let body = serde_json::to_vec(m).map_err(std::io::Error::other)?;
    stream.write_all(&(body.len() as u32).to_be_bytes()).await?;
    stream.write_all(&body).await
}
