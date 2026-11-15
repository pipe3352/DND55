//! Creature tokens: portrait or tinted disc, name label, health bar, smooth movement.

use crate::state::{CELL, GameView, cell_to_world};
use crate::textures::TextureCache;
use bevy::prelude::*;
use dnd_session::model::Faction;
use dnd_session::view::{HealthStatus, HpView};
use dnd_world::EntityId;
use std::collections::{HashMap, VecDeque};

#[derive(Component)]
pub struct Token {
    pub id: EntityId,
}

#[derive(Component)]
pub struct TokenLabel;

#[derive(Component)]
pub struct HpFill;

#[derive(Component)]
pub struct HpBack;

#[derive(Resource, Default)]
pub struct TokenAnims {
    pub paths: HashMap<EntityId, VecDeque<Vec2>>,
    pub pos: HashMap<EntityId, Vec2>,
    /// Damage/heal tint: (color, seconds left).
    pub flash: HashMap<EntityId, (Color, f32)>,
}

pub fn faction_color(f: Faction) -> Color {
    match f {
        Faction::Party => Color::srgb(0.30, 0.48, 0.85),
        Faction::Hostile => Color::srgb(0.78, 0.22, 0.18),
        Faction::Neutral => Color::srgb(0.6, 0.6, 0.55),
    }
}

pub fn sync_tokens(
    mut commands: Commands,
    mut gv: ResMut<GameView>,
    mut tokens: Query<(Entity, &Token, &mut Sprite, &Children)>,
    mut labels: Query<&mut Text2d, With<TokenLabel>>,
    mut fills: Query<(&mut Sprite, &mut Transform), (With<HpFill>, Without<Token>)>,
    mut anims: ResMut<TokenAnims>,
    mut tex: ResMut<TextureCache>,
    mut images: ResMut<Assets<Image>>,
) {
    if !gv.view_dirty {
        return;
    }
    gv.view_dirty = false;
    let Some(view) = &gv.view else { return };
    let present: HashMap<EntityId, usize> = view
        .creatures
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id, i))
        .collect();

    // Update or despawn existing tokens.
    let mut seen = std::collections::HashSet::new();
    for (e, tok, mut sprite, children) in &mut tokens {
        let Some(&i) = present.get(&tok.id) else {
            commands.entity(e).despawn();
            anims.pos.remove(&tok.id);
            anims.paths.remove(&tok.id);
            continue;
        };
        seen.insert(tok.id);
        let c = &view.creatures[i];
        let (img, tint) = token_look(c.portrait.as_deref(), c.faction, &mut tex, &mut images);
        sprite.image = img;
        let dead = matches!(c.hp, HpView::Status(HealthStatus::Dead))
            || matches!(c.hp, HpView::Exact { current: 0, .. }) && c.is_monster;
        sprite.color = if dead {
            Color::srgba(0.4, 0.4, 0.4, 0.6)
        } else if c.hidden {
            tint.with_alpha(0.5)
        } else {
            tint
        };
        for child in children.iter() {
            if let Ok(mut t) = labels.get_mut(child) {
                t.0 = label_text(c);
            }
            if let Ok((mut s, mut tr)) = fills.get_mut(child) {
                let (frac, color) = hp_fraction(&c.hp);
                s.custom_size = Some(Vec2::new(CELL * 0.8 * frac, 5.0));
                s.color = color;
                tr.translation.x = -CELL * 0.4 * (1.0 - frac);
            }
        }
        if !anims.paths.get(&c.id).is_some_and(|p| !p.is_empty()) {
            anims.pos.insert(c.id, cell_to_world(c.pos));
        }
    }

    // Spawn new tokens.
    for c in &view.creatures {
        if seen.contains(&c.id) {
            continue;
        }
        let (img, tint) = token_look(c.portrait.as_deref(), c.faction, &mut tex, &mut images);
        let p = cell_to_world(c.pos);
        anims.pos.insert(c.id, p);
        let (frac, color) = hp_fraction(&c.hp);
        commands
            .spawn((
                Token { id: c.id },
                Sprite {
                    image: img,
                    custom_size: Some(Vec2::splat(CELL * 0.86)),
                    color: tint,
                    ..default()
                },
                Transform::from_xyz(p.x, p.y, 2.0),
            ))
            .with_children(|parent| {
                parent.spawn((
                    TokenLabel,
                    Text2d::new(label_text(c)),
                    TextFont {
                        font_size: bevy::text::FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.94, 0.82)),
                    Transform::from_xyz(0.0, -CELL * 0.58, 2.5),
                ));
                parent.spawn((
                    HpBack,
                    Sprite {
                        color: Color::srgba(0.1, 0.05, 0.03, 0.85),
                        custom_size: Some(Vec2::new(CELL * 0.84, 7.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, CELL * 0.5, 2.4),
                ));
                parent.spawn((
                    HpFill,
                    Sprite {
                        color,
                        custom_size: Some(Vec2::new(CELL * 0.8 * frac, 5.0)),
                        ..default()
                    },
                    Transform::from_xyz(-CELL * 0.4 * (1.0 - frac), CELL * 0.5, 2.45),
                ));
            });
    }
}

fn label_text(c: &dnd_session::view::CreatureView) -> String {
    let mut s = c.name.clone();
    if !c.conditions.is_empty() {
        let short: Vec<String> = c
            .conditions
            .iter()
            .map(|k| format!("{k:?}").chars().take(4).collect())
            .collect();
        s.push_str(&format!(" [{}]", short.join(",")));
    }
    s
}

fn hp_fraction(hp: &HpView) -> (f32, Color) {
    match hp {
        HpView::Exact { current, max, .. } => {
            let f = if *max > 0 {
                (*current as f32 / *max as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (
                f,
                if f > 0.5 {
                    Color::srgb(0.3, 0.7, 0.25)
                } else if f > 0.0 {
                    Color::srgb(0.85, 0.55, 0.1)
                } else {
                    Color::srgb(0.4, 0.0, 0.0)
                },
            )
        }
        // Players don't get exact enemy HP: show a coarse status bar.
        HpView::Status(HealthStatus::Healthy) => (1.0, Color::srgb(0.3, 0.7, 0.25)),
        HpView::Status(HealthStatus::Bloodied) => (0.5, Color::srgb(0.85, 0.55, 0.1)),
        HpView::Status(_) => (0.0, Color::srgb(0.4, 0.0, 0.0)),
    }
}

fn token_look(
    portrait: Option<&str>,
    f: Faction,
    tex: &mut TextureCache,
    images: &mut Assets<Image>,
) -> (Handle<Image>, Color) {
    match portrait.and_then(|h| tex.portraits.get(h).cloned()) {
        Some(h) => (h, Color::WHITE),
        None => (tex.disc(images), faction_color(f)),
    }
}

/// Moves tokens along queued paths and applies hit flashes.
pub fn animate_tokens(
    time: Res<Time>,
    mut anims: ResMut<TokenAnims>,
    mut q: Query<(&Token, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    let speed = CELL * 7.0;
    let ids: Vec<EntityId> = anims.paths.keys().copied().collect();
    for id in ids {
        let Some(mut cur) = anims.pos.get(&id).copied() else {
            continue;
        };
        let mut budget = speed * dt;
        let path = anims.paths.get_mut(&id).expect("key exists");
        while budget > 0.0 {
            let Some(&next) = path.front() else { break };
            let d = next - cur;
            let len = d.length();
            if len <= budget {
                cur = next;
                budget -= len;
                path.pop_front();
            } else {
                cur += d / len * budget;
                budget = 0.0;
            }
        }
        if path.is_empty() {
            anims.paths.remove(&id);
        }
        anims.pos.insert(id, cur);
    }
    let mut done = vec![];
    for (id, (_, t)) in anims.flash.iter_mut() {
        *t -= dt;
        if *t <= 0.0 {
            done.push(*id);
        }
    }
    for (tok, mut tr, mut sprite) in &mut q {
        if let Some(p) = anims.pos.get(&tok.id) {
            tr.translation.x = p.x;
            tr.translation.y = p.y;
        }
        if let Some((c, t)) = anims.flash.get(&tok.id) {
            let k = (t / 0.35).clamp(0.0, 1.0);
            let base = sprite.color.to_srgba();
            let fc = c.to_srgba();
            sprite.color = Color::srgba(
                base.red + (fc.red - base.red) * k,
                base.green + (fc.green - base.green) * k,
                base.blue + (fc.blue - base.blue) * k,
                base.alpha,
            );
            // Small shake on hit.
            tr.translation.x += (t * 80.0).sin() * 3.0 * k;
        }
    }
    for id in done {
        anims.flash.remove(&id);
    }
}
