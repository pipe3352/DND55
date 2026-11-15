//! # dnd_session — the authoritative game session
//!
//! A pure state machine (no IO): the host feeds it [`Command`]s from principals (DM or players)
//! and routes the resulting [`event::Event`]s and per-client [`view::ClientView`]s over the network.
//! Because it is IO-free, every multiplayer rule — authority, validation, hidden information — is
//! unit-testable without sockets.

pub mod assets;
pub mod command;
pub mod demo;
pub mod effects;
mod features;
mod items;
pub mod scripts;
mod spellcasting;
pub mod event;
pub mod hexbytes;
pub mod model;
pub mod session;
pub mod view;

pub use command::{Command, DmCommand};
pub use model::{GameState, PlayerId, Principal};
pub use session::{CommandError, Session};
pub use view::ClientView;
