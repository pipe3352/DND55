//! Turns validated host events into animations and sounds. Visuals never decide outcomes.

use crate::audio::{Sfx, SfxBank};
use crate::state::{AudioSettings, CELL, GameView, Net, cell_to_world};
use crate::tokens::TokenAnims;
use bevy::prelude::*;
use dnd_session::event::{Event, FloatKind, Vfx};

#[derive(Component)]
pub struct FloatText {
    pub vel: Vec2,
    pub life: f32,
    pub max: f32,
}

#[derive(Component)]
pub struct Projectile {
    pub from: Vec2,
    pub to: Vec2,
    pub t: f32,
    pub dur: f32,
}

#[derive(Component)]
pub struct Flash {
    pub life: f32,
    pub max: f32,
    pub grow: f32,
}

/// Screen fade when the party changes rooms; also the "last dice roll" for the dice tray.
#[derive(Resource, Default)]
pub struct Presentation {
    pub fade: f32,
    pub room_title: Option<(String, f32)>,
    pub last_roll: Option<(String, String, i32, f32)>,
    pub your_turn_banner: f32,
    pub chat: Vec<(String, String)>,
}

fn vfx_color(v: Vfx) -> Color {
    match v {
        Vfx::Slash | Vfx::Pierce | Vfx::Bludgeon | Vfx::Arrow => Color::srgb(0.95, 0.92, 0.85),
        Vfx::Fire => Color::srgb(1.0, 0.45, 0.1),
        Vfx::Cold => Color::srgb(0.6, 0.85, 1.0),
        Vfx::Lightning => Color::srgb(0.9, 0.9, 0.3),
        Vfx::Poison => Color::srgb(0.4, 0.85, 0.2),
        Vfx::Acid => Color::srgb(0.6, 0.9, 0.1),
        Vfx::Radiant => Color::srgb(1.0, 0.95, 0.6),
        Vfx::Necrotic => Color::srgb(0.45, 0.15, 0.55),
        Vfx::Force => Color::srgb(0.65, 0.5, 1.0),
        Vfx::Thunder => Color::srgb(0.55, 0.6, 0.9),
        Vfx::Psychic => Color::srgb(0.95, 0.4, 0.8),
        Vfx::Healing => Color::srgb(0.3, 1.0, 0.45),
    }
}

pub fn process_events(
    mut commands: Commands,
    mut gv: ResMut<GameView>,
    mut anims: ResMut<TokenAnims>,
    mut pres: ResMut<Presentation>,
    net: Res<Net>,
    sfx: Res<SfxBank>,
    audio: Res<AudioSettings>,
) {
    if gv.events.is_empty() {
        return;
    }
    let events = std::mem::take(&mut gv.events);
    let pos_of = |anims: &TokenAnims, gv: &GameView, id| {
        anims.pos.get(&id).copied().or_else(|| {
            gv.view
                .as_ref()?
                .creatures
                .iter()
                .find(|c| c.id == id)
                .map(|c| cell_to_world(c.pos))
        })
    };
    for ev in events {
        match ev {
            Event::Moved { entity, path } => {
                let q = anims.paths.entry(entity).or_default();
                q.extend(path.into_iter().skip(1).map(cell_to_world));
                sfx.play(&mut commands, Sfx::Step, &audio);
            }
            Event::Floating { entity, text, kind } => {
                let Some(p) = pos_of(&anims, &gv, entity) else {
                    continue;
                };
                let (color, size) = match kind {
                    FloatKind::Damage => (Color::srgb(1.0, 0.25, 0.2), 22.0),
                    FloatKind::CriticalDamage => (Color::srgb(1.0, 0.1, 0.05), 30.0),
                    FloatKind::Healing => (Color::srgb(0.35, 1.0, 0.4), 22.0),
                    FloatKind::TempHp => (Color::srgb(0.5, 0.8, 1.0), 20.0),
                    FloatKind::Miss => (Color::srgb(0.85, 0.85, 0.85), 18.0),
                    FloatKind::Resisted => (Color::srgb(0.95, 0.6, 0.3), 20.0),
                    FloatKind::Immune => (Color::srgb(0.7, 0.7, 0.95), 18.0),
                    FloatKind::Condition => (Color::srgb(0.95, 0.85, 0.4), 16.0),
                    FloatKind::Info => (Color::srgb(0.9, 0.9, 0.75), 15.0),
                };
                commands.spawn((
                    FloatText {
                        vel: Vec2::new(0.0, 46.0),
                        life: 1.5,
                        max: 1.5,
                    },
                    Text2d::new(text),
                    TextFont {
                        font_size: bevy::text::FontSize::Px(size),
                        ..default()
                    },
                    TextColor(color),
                    Transform::from_xyz(p.x, p.y + CELL * 0.4, 10.0),
                ));
                match kind {
                    FloatKind::Damage | FloatKind::CriticalDamage | FloatKind::Resisted => {
                        anims
                            .flash
                            .insert(entity, (Color::srgb(1.0, 0.2, 0.15), 0.35));
                        sfx.play(
                            &mut commands,
                            if kind == FloatKind::CriticalDamage {
                                Sfx::Crit
                            } else {
                                Sfx::Hit
                            },
                            &audio,
                        );
                    }
                    FloatKind::Healing => {
                        anims
                            .flash
                            .insert(entity, (Color::srgb(0.3, 1.0, 0.4), 0.35));
                        commands.spawn((
                            Flash {
                                life: 0.6,
                                max: 0.6,
                                grow: 1.6,
                            },
                            Sprite {
                                color: vfx_color(Vfx::Healing).with_alpha(0.5),
                                custom_size: Some(Vec2::splat(CELL)),
                                ..default()
                            },
                            Transform::from_xyz(p.x, p.y, 9.0),
                        ));
                        sfx.play(&mut commands, Sfx::Heal, &audio);
                    }
                    FloatKind::Miss => sfx.play(&mut commands, Sfx::Miss, &audio),
                    _ => {}
                }
            }
            Event::AttackVisual {
                attacker,
                target,
                vfx,
                ranged,
                ..
            } => {
                let (Some(a), Some(t)) =
                    (pos_of(&anims, &gv, attacker), pos_of(&anims, &gv, target))
                else {
                    continue;
                };
                let color = vfx_color(vfx);
                if ranged {
                    let dir = t - a;
                    commands.spawn((
                        Projectile {
                            from: a,
                            to: t,
                            t: 0.0,
                            dur: 0.22,
                        },
                        Sprite {
                            color,
                            custom_size: Some(Vec2::new(CELL * 0.5, 4.0)),
                            ..default()
                        },
                        Transform::from_xyz(a.x, a.y, 9.0)
                            .with_rotation(Quat::from_rotation_z(dir.y.atan2(dir.x))),
                    ));
                    sfx.play(&mut commands, Sfx::Bow, &audio);
                } else {
                    let dir = t - a;
                    commands.spawn((
                        Flash {
                            life: 0.22,
                            max: 0.22,
                            grow: 0.4,
                        },
                        Sprite {
                            color: color.with_alpha(0.9),
                            custom_size: Some(Vec2::new(CELL * 1.0, 6.0)),
                            ..default()
                        },
                        Transform::from_xyz(t.x, t.y, 9.0)
                            .with_rotation(Quat::from_rotation_z(dir.y.atan2(dir.x) + 0.9)),
                    ));
                    sfx.play(&mut commands, Sfx::Swing, &audio);
                }
            }
            Event::DiceRolled {
                actor,
                label,
                detail,
                total,
            } => {
                pres.last_roll = Some((
                    format!(
                        "{actor}{}",
                        if label.is_empty() {
                            String::new()
                        } else {
                            format!(" — {label}")
                        }
                    ),
                    detail,
                    total,
                    0.9,
                ));
                sfx.play(&mut commands, Sfx::Dice, &audio);
            }
            Event::RoomChanged { name, .. } => {
                pres.fade = 1.0;
                pres.room_title = Some((name, 3.0));
                anims.paths.clear();
                sfx.play(&mut commands, Sfx::Door, &audio);
            }
            Event::CombatStarted => sfx.play(&mut commands, Sfx::Horn, &audio),
            Event::TurnStarted { entity, .. } => {
                let mine = gv
                    .view
                    .as_ref()
                    .is_some_and(|v| v.creatures.iter().any(|c| c.id == entity && c.yours));
                if mine && !net.is_dm() {
                    pres.your_turn_banner = 2.0;
                    sfx.play(&mut commands, Sfx::Turn, &audio);
                }
            }
            Event::ReactionPrompt { .. } => sfx.play(&mut commands, Sfx::Turn, &audio),
            Event::Chat { from, text } => {
                pres.chat.push((from, text));
                if pres.chat.len() > 200 {
                    pres.chat.remove(0);
                }
            }
            Event::Notice { text } => gv.toast(text),
            Event::CombatEnded | Event::Log { .. } => {}
        }
    }
}

pub fn animate_fx(
    mut commands: Commands,
    time: Res<Time>,
    mut floats: Query<(Entity, &mut FloatText, &mut Transform, &mut TextColor)>,
    mut proj: Query<(Entity, &mut Projectile, &mut Transform), Without<FloatText>>,
    mut flashes: Query<
        (Entity, &mut Flash, &mut Transform, &mut Sprite),
        (Without<FloatText>, Without<Projectile>),
    >,
    mut pres: ResMut<Presentation>,
    mut gv: ResMut<GameView>,
) {
    let dt = time.delta_secs();
    for (e, mut f, mut t, mut c) in &mut floats {
        f.life -= dt;
        t.translation += (f.vel * dt).extend(0.0);
        f.vel *= 0.97;
        let a = (f.life / f.max).clamp(0.0, 1.0);
        c.0.set_alpha(a.min(1.0) * 1.2);
        if f.life <= 0.0 {
            commands.entity(e).despawn();
        }
    }
    for (e, mut p, mut t) in &mut proj {
        p.t += dt;
        let k = (p.t / p.dur).min(1.0);
        let pos = p.from.lerp(p.to, k);
        t.translation.x = pos.x;
        t.translation.y = pos.y;
        if k >= 1.0 {
            commands.entity(e).despawn();
        }
    }
    for (e, mut f, mut t, mut s) in &mut flashes {
        f.life -= dt;
        let k = (f.life / f.max).clamp(0.0, 1.0);
        t.scale = Vec3::splat(1.0 + f.grow * (1.0 - k));
        s.color.set_alpha(k * 0.9);
        if f.life <= 0.0 {
            commands.entity(e).despawn();
        }
    }
    pres.fade = (pres.fade - dt * 0.8).max(0.0);
    pres.your_turn_banner = (pres.your_turn_banner - dt).max(0.0);
    if let Some((_, t)) = &mut pres.room_title {
        *t -= dt;
        if *t <= 0.0 {
            pres.room_title = None;
        }
    }
    if let Some((_, _, _, t)) = &mut pres.last_roll {
        *t = (*t - dt).max(0.0);
    }
    for t in gv.toasts.iter_mut() {
        t.1 -= dt;
    }
    gv.toasts.retain(|t| t.1 > 0.0);
}
