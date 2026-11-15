//! # dnd_net — multiplayer transport
//!
//! **Hosting model (vertical slice):** the DM's application runs the authoritative [`host`] in-process
//! (a "listen server"); the DM's own UI connects to it over loopback exactly like a player, authenticated
//! by a random DM secret. Players connect over TCP with the host address and a session code.
//!
//! Wire format: 4-byte big-endian length prefix + JSON message ([`protocol`]). Frames are size-limited
//! and every message from a client is validated by the session before it can affect the game.
//!
//! NAT traversal / relay hosting is a planned module (see `docs/DESIGN.md` §6); until then players use
//! LAN, port forwarding, or a private overlay network (Tailscale, ZeroTier, ...).

pub mod client;
pub mod host;
pub mod invite;
pub mod protocol;
pub mod upnp;

pub use client::{NetClient, NetEvent};
pub use host::{HostConfig, HostHandle, RelayStatus, SaveHook};
pub use invite::{Invite, Target};
pub use protocol::{ClientMsg, Hello, HostRequest, PROTOCOL_VERSION, ServerMsg};

/// Random token (128-bit, hex) from the OS entropy source.
pub fn random_token() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("OS entropy available");
    hex::encode(b)
}

/// Human-friendly session code: 6 characters without ambiguous glyphs (0/O, 1/I/L).
pub fn new_session_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let mut b = [0u8; 6];
    getrandom::fill(&mut b).expect("OS entropy available");
    b.iter()
        .map(|v| ALPHABET[(*v as usize) % ALPHABET.len()] as char)
        .collect()
}
