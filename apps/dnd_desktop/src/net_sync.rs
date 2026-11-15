//! Pulls messages from the network thread into Bevy resources.

use crate::state::{AppState, GameView, Net};
use crate::textures::{TextureCache, portrait_token};
use bevy::prelude::*;
use dnd_net::{ClientMsg, NetEvent, ServerMsg};
use dnd_session::command::Command;

pub fn poll_network(
    mut net: ResMut<Net>,
    mut gv: ResMut<GameView>,
    mut tex: ResMut<TextureCache>,
    mut images: ResMut<Assets<Image>>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();
    // Automatic reconnection with backoff (1, 2, 4 … 30 s).
    if net.client.is_none() {
        if let Some(at) = net.retry_at
            && now >= at
        {
            net.retries += 1;
            crate::hosting::reconnect(&mut net);
        }
        return;
    }
    if now - net.last_ping > 5.0 {
        net.last_ping = now;
        if let Some(c) = &net.client {
            c.send(ClientMsg::Ping {
                t: (now * 1000.0) as u64,
            });
        }
    }
    let Some(client) = &net.client else { return };
    let msgs = client.poll();
    let mut lost = None;
    for m in msgs {
        match m {
            NetEvent::Connected => net.status = "Connected, waiting for the host…".into(),
            NetEvent::Disconnected(reason) => lost = Some(reason),
            NetEvent::Message(msg) => match msg {
                ServerMsg::Welcome {
                    you,
                    resume_token,
                    campaign,
                } => {
                    net.you = Some(you);
                    net.resume_token = Some(resume_token);
                    if net.retries > 0 {
                        gv.toast("Reconnected.");
                    }
                    net.retries = 0;
                    net.retry_at = None;
                    net.status = format!("Joined {campaign}");
                    if let Some((sheet, png)) = net.pending_character.take() {
                        net.cmd(Command::SubmitCharacter {
                            sheet: Box::new(sheet),
                            portrait_png: png,
                        });
                    }
                    if *state.get() != AppState::InGame {
                        next.set(AppState::InGame);
                    }
                }
                ServerMsg::Rejected { reason } => {
                    net.rejected = true;
                    net.status = format!("Rejected: {reason}");
                    gv.toast(format!("Rejected: {reason}"));
                }
                ServerMsg::View(v) => {
                    if let Some(r) = &v.room
                        && let Some(map) = &r.map
                    {
                        gv.map = Some(map.clone());
                        gv.map_key = Some((r.id, r.map_rev));
                        gv.map_dirty = true;
                    }
                    // Ask for portraits we haven't seen.
                    for c in &v.creatures {
                        if let Some(h) = &c.portrait
                            && !tex.portraits.contains_key(h)
                            && tex.requested.insert(h.clone())
                            && let Some(cl) = &net.client
                        {
                            cl.send(ClientMsg::GetAsset { hash: h.clone() });
                        }
                    }
                    gv.view = Some(*v);
                    gv.view_dirty = true;
                }
                ServerMsg::Events(evs) => gv.events.extend(evs),
                ServerMsg::CmdResult { error: Some(e), .. } => gv.toast(e),
                ServerMsg::CmdResult { error: None, .. } => {}
                ServerMsg::Asset { hash, bytes } => {
                    if let Some(img) = portrait_token(&bytes) {
                        let h = images.add(img);
                        tex.portraits.insert(hash, h);
                        gv.view_dirty = true;
                    }
                }
                ServerMsg::Pong { t } => {
                    net.rtt_ms = Some(((now * 1000.0) as u64).saturating_sub(t));
                }
            },
        }
    }
    if let Some(reason) = lost {
        let reason = explain_network(&reason);
        net.client = None;
        net.rtt_ms = None;
        // Only retry sessions we had actually joined, and never after an explicit rejection.
        if net.you.is_some() && !net.rejected && net.retries < crate::state::MAX_RETRIES {
            let wait = (1u64 << net.retries.min(5)).min(30) as f64;
            net.retry_at = Some(now + wait);
            net.status = format!("Connection lost ({reason}). Retrying in {wait:.0}s…");
        } else {
            net.retry_at = None;
            net.status = format!("Disconnected: {reason}");
        }
        gv.toast(net.status.clone());
    }
}

/// Turns low-level network errors into advice a player can act on.
pub fn explain_network(reason: &str) -> String {
    let r = reason.to_lowercase();
    let hint = if r.contains("refused") {
        "nothing is listening at that address — check the IP and port, make sure the DM has started hosting, and that the DM's firewall allows DND55"
    } else if r.contains("timed out") || r.contains("timeout") {
        "no answer — the address may be wrong, or a router/firewall is blocking it (internet play needs port forwarding, UPnP or the relay)"
    } else if r.contains("no game with that invite") {
        "the relay doesn't know that invite — the DM's game may have closed or restarted; ask for a fresh invite"
    } else if r.contains("dns") || r.contains("resolve") || r.contains("lookup") {
        "that host name could not be found — check the spelling"
    } else if r.contains("reset")
        || r.contains("closed")
        || r.contains("eof")
        || r.contains("broken pipe")
    {
        "the connection was closed by the other side or the network"
    } else {
        ""
    };
    if hint.is_empty() {
        reason.to_string()
    } else {
        format!("{reason} — {hint}")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn network_errors_get_advice() {
        assert!(super::explain_network("Connection refused (os error 111)").contains("firewall"));
        assert!(super::explain_network("weird").eq("weird"));
    }
}
