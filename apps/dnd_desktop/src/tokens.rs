//! Creature tokens: portrait or tinted disc, name label, health bar, smooth movement.

use crate::state::{CELL, GameView, cell_to_world};
use crate::terrain_art::token_art::{self, Side};
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

/// Small gold arrow showing a north/south facing.
#[derive(Component)]
pub struct FacingPip;

fn pip_image() -> crate::pixel::Pix {
    use crate::pixel::*;
    let mut p = Pix::new(9, 6);
    p.poly(&[(4, 0), (8, 5), (0, 5)], GOLD.l);
    p.outline();
    p
}

/// Faction frame drawn over the token face or portrait.
#[derive(Component)]
pub struct TokenFrame;

/// Selection ring (gold, pulsing) and active-turn ring (ivory, rotating) under the token.
#[derive(Component)]
pub struct SelRing;
#[derive(Component)]
pub struct TurnRing;

/// Cached token textures: faces by (emblem, side), frames by side, rings.
#[derive(Resource, Default)]
pub struct TokenArt {
    faces: HashMap<(String, Side), Handle<Image>>,
    frames: HashMap<(Side, bool), Handle<Image>>,
    ring: Option<Handle<Image>>,
    pip: Option<Handle<Image>>,
}

impl TokenArt {
    fn face(&mut self, emblem: &str, side: Side, images: &mut Assets<Image>) -> Handle<Image> {
        self.faces
            .entry((emblem.to_string(), side))
            .or_insert_with(|| images.add(crate::sprites::creature(emblem).into_image()))
            .clone()
    }
    /// Side marker: a ground ring under sprites, the round frame over portraits.
    fn frame(&mut self, side: Side, portrait: bool, images: &mut Assets<Image>) -> Handle<Image> {
        self.frames
            .entry((side, portrait))
            .or_insert_with(|| {
                images.add(if portrait {
                    token_art::frame(side)
                } else {
                    base_marker(side).into_image()
                })
            })
            .clone()
    }
    fn ring(&mut self, images: &mut Assets<Image>) -> Handle<Image> {
        self.ring
            .get_or_insert_with(|| images.add(token_art::indicator_ring()))
            .clone()
    }
}

pub fn side(f: Faction) -> Side {
    match f {
        Faction::Party => Side::Friendly,
        Faction::Hostile => Side::Hostile,
        Faction::Neutral => Side::Neutral,
    }
}

/// Sprite key for a creature: `class:<icon>` for characters, `mon:<type>:<name>` for monsters
/// (trailing numbers such as "Goblin 2" are ignored, so every goblin shares its look).
pub fn emblem(c: &dnd_session::view::CreatureView) -> String {
    match &c.token {
        Some(icon) => format!("class:{icon}"),
        None => {
            let name = c
                .name
                .trim_end_matches(|ch: char| ch.is_ascii_digit() || ch == ' ')
                .to_lowercase();
            format!("mon:{}:{name}", c.creature_type.to_lowercase())
        }
    }
}

/// Ground marker under a sprite token. Its *shape* tells the side: friendly = smooth gold ring,
/// hostile = red ring with four spikes, neutral = broken bronze ring.
pub fn base_marker(side: Side) -> crate::pixel::Pix {
    use crate::pixel::*;
    let mut p = Pix::new(32, 32);
    let (col, fill): (C, C) = match side {
        Side::Friendly => (c(240, 196, 80), [70, 120, 220, 110]),
        Side::Hostile => (c(214, 50, 44), [150, 30, 30, 110]),
        Side::Neutral => (c(170, 130, 80), [120, 110, 90, 90]),
    };
    for i in 0..64 {
        let a = i as f32 / 64.0 * std::f32::consts::TAU;
        let (x, y) = (
            (16.0 + a.cos() * 12.5) as i32,
            (28.5 + a.sin() * 2.6) as i32,
        );
        if side != Side::Neutral || i % 8 < 5 {
            p.set(x, y, col);
        }
    }
    for y in 27..31 {
        for x in 5..27 {
            let (dx, dy) = (
                (x as f32 + 0.5 - 16.0) / 11.5,
                (y as f32 + 0.5 - 28.5) / 2.0,
            );
            if dx * dx + dy * dy < 1.0 && p.get(x, y)[3] == 0 {
                p.set(x, y, fill);
            }
        }
    }
    if side == Side::Hostile {
        for (x, y, dx) in [(2, 28, -1), (29, 28, 1)] {
            p.set(x, y, col);
            p.set(x + dx, y, col);
        }
        p.rect(15, 31, 2, 1, col);
        p.rect(15, 25, 2, 1, col);
    }
    p
}

/// Token diameter: scales with the grid and creature size but never covers neighbouring cells.
fn token_px(size: dnd_rules::content::Size) -> f32 {
    use dnd_rules::content::Size::*;
    CELL * match size {
        Tiny => 0.7,
        Small => 0.9,
        Medium => 1.0,
        Large => 1.8,
        Huge => 2.6,
        Gargantuan => 3.4,
    }
}

#[derive(Resource, Default)]
pub struct TokenAnims {
    pub paths: HashMap<EntityId, VecDeque<Vec2>>,
    pub pos: HashMap<EntityId, Vec2>,
    /// Damage/heal tint: (color, seconds left).
    pub flash: HashMap<EntityId, (Color, f32)>,
}

pub fn sync_tokens(
    mut commands: Commands,
    mut gv: ResMut<GameView>,
    mut tokens: Query<(Entity, &Token, &mut Sprite, &Children)>,
    mut labels: Query<&mut Text2d, With<TokenLabel>>,
    mut fills: Query<
        (&mut Sprite, &mut Transform),
        (With<HpFill>, Without<Token>, Without<TokenFrame>),
    >,
    mut anims: ResMut<TokenAnims>,
    mut tex: ResMut<TextureCache>,
    mut art: ResMut<TokenArt>,
    mut images: ResMut<Assets<Image>>,
    mut pips: Query<
        (&mut Visibility, &mut Transform),
        (
            With<FacingPip>,
            Without<Token>,
            Without<HpFill>,
            Without<TokenFrame>,
        ),
    >,
    mut frames: Query<
        (&mut Sprite, &mut Transform),
        (With<TokenFrame>, Without<Token>, Without<HpFill>),
    >,
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
        let (img, tint) = token_look(c, &mut tex, &mut art, &mut images);
        sprite.image = img;
        // Facing: west mirrors the sprite (a directional "frame"); north/south show a ground arrow.
        sprite.flip_x = c.facing == 2;
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
            if let Ok((mut v, mut pt)) = pips.get_mut(child) {
                let d = token_px(c.size);
                *v = if c.facing % 2 == 1 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                let down = c.facing == 1;
                pt.translation = Vec3::new(
                    0.0,
                    if down { -d * 0.5 + 2.0 } else { d * 0.5 - 6.0 },
                    -0.01,
                );
                pt.rotation = Quat::from_rotation_z(if down { std::f32::consts::PI } else { 0.0 });
            }
            if let Ok((mut f, mut ftr)) = frames.get_mut(child) {
                let portrait = has_portrait(c, &tex);
                f.image = art.frame(side(c.faction), portrait, &mut images);
                ftr.translation.z = if portrait { 0.02 } else { -0.02 };
            }
            if let Ok((mut s, mut tr)) = fills.get_mut(child) {
                let (frac, color) = hp_fraction(&c.hp);
                s.custom_size = Some(Vec2::new(CELL * 0.56 * frac, 4.0));
                s.color = color;
                tr.translation.x = -CELL * 0.28 * (1.0 - frac);
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
        let (img, tint) = token_look(c, &mut tex, &mut art, &mut images);
        let p = cell_to_world(c.pos);
        anims.pos.insert(c.id, p);
        let (frac, color) = hp_fraction(&c.hp);
        let d = token_px(c.size);
        let portrait = has_portrait(c, &tex);
        let frame = art.frame(side(c.faction), portrait, &mut images);
        let ring = art.ring(&mut images);
        let disc = tex.disc(&mut images);
        let pip = art
            .pip
            .get_or_insert_with(|| images.add(pip_image().into_image()))
            .clone();
        commands
            .spawn((
                Token { id: c.id },
                Sprite {
                    image: img,
                    custom_size: Some(Vec2::splat(d)),
                    color: tint,
                    ..default()
                },
                Transform::from_xyz(p.x, p.y, 2.0),
            ))
            .with_children(|parent| {
                // Portrait discs get a soft drop shadow; sprites carry their own ground shadow.
                if portrait {
                    parent.spawn((
                        Sprite {
                            image: disc,
                            custom_size: Some(Vec2::splat(d * 0.86)),
                            color: Color::srgba(0.0, 0.0, 0.0, 0.38),
                            ..default()
                        },
                        Transform::from_xyz(d * 0.05, -d * 0.07, -0.06),
                    ));
                }
                for (marker_sel, color) in [
                    (true, Color::srgb(1.0, 0.82, 0.3)),
                    (false, Color::srgb(0.9, 0.98, 0.85)),
                ] {
                    let mut e = parent.spawn((
                        Sprite {
                            image: ring.clone(),
                            custom_size: Some(Vec2::splat(d * if marker_sel { 1.12 } else { 1.2 })),
                            color,
                            ..default()
                        },
                        Transform::from_xyz(0.0, 0.0, -0.03),
                        Visibility::Hidden,
                    ));
                    if marker_sel {
                        e.insert(SelRing);
                    } else {
                        e.insert(TurnRing);
                    }
                }
                parent.spawn((
                    FacingPip,
                    Sprite {
                        image: pip.clone(),
                        custom_size: Some(Vec2::new(18.0, 12.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, -0.01),
                    Visibility::Hidden,
                ));
                parent.spawn((
                    TokenFrame,
                    Sprite {
                        image: frame,
                        custom_size: Some(Vec2::splat(d)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, if portrait { 0.02 } else { -0.02 }),
                ));
                parent.spawn((
                    TokenLabel,
                    Text2d::new(label_text(c)),
                    TextFont {
                        font_size: bevy::text::FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.94, 0.82)),
                    Transform::from_xyz(0.0, -d * 0.5 - 7.0, 2.5),
                ));
                parent.spawn((
                    HpBack,
                    Sprite {
                        color: Color::srgba(0.1, 0.05, 0.03, 0.85),
                        custom_size: Some(Vec2::new(CELL * 0.6, 6.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, d * 0.5 + 2.0, 2.4),
                ));
                parent.spawn((
                    HpFill,
                    Sprite {
                        color,
                        custom_size: Some(Vec2::new(CELL * 0.56 * frac, 4.0)),
                        ..default()
                    },
                    Transform::from_xyz(-CELL * 0.28 * (1.0 - frac), d * 0.5 + 2.0, 2.45),
                ));
            });
    }
}

/// Selection/turn rings and optional names: cheap per-frame update.
#[allow(clippy::type_complexity)]
pub fn token_indicators(
    time: Res<Time>,
    gv: Res<GameView>,
    ui: Res<crate::state::UiState>,
    settings: Res<crate::settings::GameSettings>,
    tokens: Query<(&Token, &Children)>,
    mut sel: Query<
        (&mut Visibility, &mut Sprite, &mut Transform),
        (With<SelRing>, Without<TurnRing>),
    >,
    mut turn: Query<(&mut Visibility, &mut Transform), (With<TurnRing>, Without<SelRing>)>,
    mut labels: Query<&mut Visibility, (With<TokenLabel>, Without<SelRing>, Without<TurnRing>)>,
) {
    let active = gv
        .view
        .as_ref()
        .and_then(|v| v.combat.as_ref())
        .and_then(|c| c.active);
    let t = if settings.reduced_motion {
        0.0
    } else {
        time.elapsed_secs()
    };
    for (tok, children) in &tokens {
        for child in children.iter() {
            if let Ok((mut v, mut s, mut tr)) = sel.get_mut(child) {
                let on = ui.selected == Some(tok.id);
                *v = if on {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                s.color = s.color.with_alpha(0.75 + 0.25 * (t * 4.0).sin());
                tr.scale = Vec3::splat(1.0 + 0.03 * (t * 4.0).sin());
            }
            if let Ok((mut v, mut tr)) = turn.get_mut(child) {
                let on = active == Some(tok.id);
                *v = if on {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                tr.rotation = Quat::from_rotation_z(t * 0.8);
            }
            if let Ok(mut v) = labels.get_mut(child) {
                *v = if settings.token_names || ui.selected == Some(tok.id) {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

fn label_text(c: &dnd_session::view::CreatureView) -> String {
    let mut s = c.name.clone();
    if !c.conditions.is_empty() {
        let short: Vec<String> = c
            .conditions
            .iter()
            .map(|k| {
                let short: String = format!("{k:?}").chars().take(4).collect();
                match c.condition_rounds.iter().find(|(x, _)| x == k) {
                    Some((_, r)) => format!("{short} {r}"),
                    None => short,
                }
            })
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

fn has_portrait(c: &dnd_session::view::CreatureView, tex: &TextureCache) -> bool {
    c.portrait
        .as_deref()
        .is_some_and(|h| tex.portraits.contains_key(h))
}

/// Portrait (if the player uploaded one) or the pixel-art sprite. Never a plain disc.
fn token_look(
    c: &dnd_session::view::CreatureView,
    tex: &mut TextureCache,
    art: &mut TokenArt,
    images: &mut Assets<Image>,
) -> (Handle<Image>, Color) {
    match c
        .portrait
        .as_deref()
        .and_then(|h| tex.portraits.get(h).cloned())
    {
        Some(h) => (h, Color::WHITE),
        None => (art.face(&emblem(c), side(c.faction), images), Color::WHITE),
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

/// egui texture of a complete token (face + side frame), cached in egui memory. Used by the
/// initiative tracker and the character creator's token preview.
pub fn token_egui(
    ctx: &bevy_egui::egui::Context,
    emblem: &str,
    side: Side,
) -> bevy_egui::egui::TextureHandle {
    use bevy_egui::egui;
    let id = egui::Id::new(("token-tex", emblem, side as u8));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    // Sprite over its side marker, centred on a square canvas, crisp pixels.
    let key = if emblem.contains(':') {
        emblem.to_string()
    } else {
        format!("class:{emblem}")
    };
    let sprite = crate::sprites::creature(&key);
    let n = sprite.w.max(32);
    let mut canvas = crate::pixel::Pix::new(n, n);
    canvas.blit(&base_marker(side), (n - 32) / 2, n - 32);
    canvas.blit(&sprite, (n - sprite.w) / 2, n - sprite.h);
    let img = egui::ColorImage::from_rgba_unmultiplied([n as usize, n as usize], &canvas.rgba());
    let t = ctx.load_texture(
        format!("token-{emblem}-{}", side as u8),
        img,
        egui::TextureOptions::NEAREST,
    );
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}
