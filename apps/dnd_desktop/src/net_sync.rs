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
) {
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
                ServerMsg::Pong { .. } => {}
            },
        }
    }
    if let Some(reason) = lost {
        net.status = format!("Disconnected: {reason}");
        gv.toast(format!("Disconnected: {reason}"));
        net.client = None;
    }
}
