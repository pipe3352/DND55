//! Client side. Runs its own Tokio runtime thread so a game engine can poll it without blocking.

use crate::protocol::{self, ClientMsg, Hello, ServerMsg};
use std::sync::mpsc as std_mpsc;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum NetEvent {
    Connected,
    Message(ServerMsg),
    Disconnected(String),
}

/// A connection to a host. Drop it to disconnect.
pub struct NetClient {
    // Mutex makes the client `Sync` so it can live in an ECS resource; only one consumer polls it.
    incoming: std::sync::Mutex<std_mpsc::Receiver<NetEvent>>,
    outgoing: mpsc::UnboundedSender<ClientMsg>,
    _thread: std::thread::JoinHandle<()>,
}

impl NetClient {
    pub fn connect(addr: String, hello: Hello) -> Self {
        let (in_tx, in_rx) = std_mpsc::channel();
        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<ClientMsg>();
        let thread = std::thread::Builder::new()
            .name("dnd-net-client".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio runtime");
                rt.block_on(async move {
                    let stream = match tokio::time::timeout(std::time::Duration::from_secs(8), tokio::net::TcpStream::connect(&addr)).await {
                        Ok(Ok(s)) => s,
                        Ok(Err(e)) => {
                            let _ = in_tx.send(NetEvent::Disconnected(format!("could not connect to {addr}: {e}")));
                            return;
                        }
                        Err(_) => {
                            let _ = in_tx.send(NetEvent::Disconnected(format!("timed out connecting to {addr}")));
                            return;
                        }
                    };
                    let _ = stream.set_nodelay(true);
                    let mut wire = protocol::framed(stream);
                    if protocol::send(&mut wire, &ClientMsg::Hello(hello)).await.is_err() {
                        let _ = in_tx.send(NetEvent::Disconnected("connection lost".into()));
                        return;
                    }
                    let _ = in_tx.send(NetEvent::Connected);
                    loop {
                        tokio::select! {
                            out = out_rx.recv() => match out {
                                Some(m) => if protocol::send(&mut wire, &m).await.is_err() { break },
                                None => return, // client dropped
                            },
                            msg = protocol::recv::<ServerMsg>(&mut wire) => match msg {
                                Ok(m) => {
                                    let rejected = matches!(m, ServerMsg::Rejected { .. });
                                    if in_tx.send(NetEvent::Message(m)).is_err() { return; }
                                    if rejected { return; }
                                }
                                Err(e) => {
                                    let _ = in_tx.send(NetEvent::Disconnected(e.to_string()));
                                    return;
                                }
                            }
                        }
                    }
                    let _ = in_tx.send(NetEvent::Disconnected("connection lost".into()));
                });
            })
            .expect("spawn network thread");
        NetClient {
            incoming: std::sync::Mutex::new(in_rx),
            outgoing: out_tx,
            _thread: thread,
        }
    }

    pub fn send(&self, msg: ClientMsg) -> bool {
        self.outgoing.send(msg).is_ok()
    }

    /// Non-blocking drain of everything received so far.
    pub fn poll(&self) -> Vec<NetEvent> {
        self.incoming
            .lock()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default()
    }

    /// Blocking receive with timeout (tests and tools).
    pub fn recv_timeout(&self, d: std::time::Duration) -> Option<NetEvent> {
        self.incoming.lock().ok()?.recv_timeout(d).ok()
    }
}
