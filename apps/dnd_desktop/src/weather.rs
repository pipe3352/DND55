//! Atmosphere particles: per-room weather chosen by the DM (rain, snow, fog, falling leaves, dust,
//! embers, fireflies, magical motes, ash) plus automatic embers rising from lava.
//!
//! Purely cosmetic: visibility and rules never depend on it. Particle counts scale with the
//! graphics quality preset; Reduced Motion turns moving particles off.

use crate::settings::GameSettings;
use crate::state::{CELL, GameView, cell_to_world};
use crate::terrain_art::Canvas;
use bevy::prelude::*;
use dnd_session::model::WeatherKind;
use std::collections::HashMap;

#[derive(Component)]
pub struct Particle {
    vel: Vec2,
    life: f32,
    max: f32,
    spin: f32,
    phase: f32,
    kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Weather(WeatherKind),
    LavaEmber,
}

#[derive(Resource, Default)]
pub struct WeatherSprites {
    images: HashMap<&'static str, Handle<Image>>,
    seed: u32,
}

fn sprite(name: &'static str) -> Image {
    let mut c = Canvas::new(32, 32);
    match name {
        "streak" => {
            for i in 0..28 {
                c.disc(
                    16.0,
                    2.0 + i as f32,
                    1.1,
                    [0.8, 0.85, 0.95, i as f32 / 40.0],
                );
            }
        }
        "flake" => {
            c.blob(16.0, 16.0, 10.0, 10.0, [1.0, 1.0, 1.0, 0.9]);
            c.disc(16.0, 16.0, 3.5, [1.0, 1.0, 1.0, 1.0]);
        }
        "leaf" => {
            c.poly(
                &[(16.0, 3.0), (25.0, 14.0), (16.0, 29.0), (7.0, 14.0)],
                [0.72, 0.42, 0.16, 1.0],
                1.2,
            );
            c.line((16.0, 5.0), (16.0, 27.0), 1.0, [0.4, 0.22, 0.08, 1.0]);
        }
        "fog" => c.blob(16.0, 16.0, 16.0, 16.0, [0.82, 0.84, 0.88, 0.9]),
        _ => {
            // soft glowing dot
            c.blob(16.0, 16.0, 16.0, 16.0, [1.0, 1.0, 1.0, 0.55]);
            c.disc(16.0, 16.0, 3.0, [1.0, 1.0, 1.0, 1.0]);
        }
    }
    c.into_image()
}

impl WeatherSprites {
    fn get(&mut self, name: &'static str, images: &mut Assets<Image>) -> Handle<Image> {
        self.images
            .entry(name)
            .or_insert_with(|| images.add(sprite(name)))
            .clone()
    }
    fn rand(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Target particle count for a weather type at intensity 1 and High quality.
fn base_count(k: WeatherKind) -> f32 {
    match k {
        WeatherKind::None => 0.0,
        WeatherKind::Rain => 260.0,
        WeatherKind::Snow => 180.0,
        WeatherKind::Fog => 22.0,
        WeatherKind::Leaves => 40.0,
        WeatherKind::Dust => 90.0,
        WeatherKind::Embers => 90.0,
        WeatherKind::Fireflies => 45.0,
        WeatherKind::Magic => 70.0,
        WeatherKind::Ash => 120.0,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_weather(
    mut commands: Commands,
    time: Res<Time>,
    gv: Res<GameView>,
    settings: Res<GameSettings>,
    cam: Single<(&Transform, &Projection), With<Camera2d>>,
    window: Single<&Window>,
    mut sprites: ResMut<WeatherSprites>,
    mut images: ResMut<Assets<Image>>,
    mut q: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite), Without<Camera2d>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let weather = gv
        .view
        .as_ref()
        .and_then(|v| v.room.as_ref())
        .map(|r| r.weather)
        .unwrap_or_default();
    let reduced = settings.reduced_motion;
    let mult = settings.quality.particles();
    let (ct, proj) = *cam;
    let scale = if let Projection::Orthographic(o) = proj {
        o.scale
    } else {
        1.0
    };
    let half = Vec2::new(window.width(), window.height()) * scale / 2.0;
    let center = ct.translation.truncate();
    let (min, max) = (
        center - half - Vec2::splat(CELL),
        center + half + Vec2::splat(CELL),
    );

    // Move, fade and recycle.
    let mut counts: HashMap<Kind, usize> = HashMap::new();
    for (e, mut p, mut tr, mut s) in &mut q {
        let wrong_kind = match p.kind {
            Kind::Weather(k) => k != weather.kind || reduced,
            Kind::LavaEmber => reduced || mult < 0.5,
        };
        p.life -= dt;
        let pos = tr.translation.truncate();
        if wrong_kind
            || p.life <= 0.0
            || pos.x < min.x - 200.0
            || pos.x > max.x + 200.0
            || pos.y < min.y - 200.0
            || pos.y > max.y + 200.0
        {
            commands.entity(e).despawn();
            continue;
        }
        *counts.entry(p.kind).or_default() += 1;
        let sway = match p.kind {
            Kind::Weather(WeatherKind::Snow) | Kind::Weather(WeatherKind::Ash) => {
                Vec2::new((t * 1.3 + p.phase).sin() * 14.0, 0.0)
            }
            Kind::Weather(WeatherKind::Leaves) => Vec2::new((t * 0.9 + p.phase).sin() * 30.0, 0.0),
            Kind::Weather(WeatherKind::Fireflies) => {
                Vec2::new((t * 0.7 + p.phase).sin(), (t * 0.9 + p.phase * 1.3).cos()) * 22.0
            }
            Kind::Weather(WeatherKind::Magic) => Vec2::new((t * 2.0 + p.phase).cos(), 0.0) * 18.0,
            _ => Vec2::ZERO,
        };
        tr.translation += ((p.vel + sway) * dt).extend(0.0);
        tr.rotate_z(p.spin * dt);
        let life = (p.life / p.max).clamp(0.0, 1.0);
        let fade = (life * 4.0).min(1.0) * ((1.0 - life) * 6.0).min(1.0);
        let a = match p.kind {
            Kind::Weather(WeatherKind::Fireflies) => fade * (0.5 + 0.5 * (t * 3.0 + p.phase).sin()),
            Kind::Weather(WeatherKind::Embers) | Kind::LavaEmber => {
                fade * (0.65 + 0.35 * (t * 11.0 + p.phase).sin())
            }
            Kind::Weather(WeatherKind::Fog) => fade * 0.16,
            Kind::Weather(WeatherKind::Rain) => fade * 0.55,
            _ => fade * 0.85,
        };
        s.color.set_alpha(a);
    }

    // Spawn weather particles across the visible area.
    if !reduced && weather.kind != WeatherKind::None {
        let want = (base_count(weather.kind) * weather.intensity.clamp(0.05, 1.0) * mult) as usize;
        let have = counts
            .get(&Kind::Weather(weather.kind))
            .copied()
            .unwrap_or(0);
        for _ in 0..(want.saturating_sub(have)).min(40) {
            let (rx, ry) = (sprites.rand(), sprites.rand());
            let pos = Vec2::new(min.x + rx * (max.x - min.x), min.y + ry * (max.y - min.y));
            let (img, size, vel, life, color, spin) = match weather.kind {
                WeatherKind::Rain => (
                    "streak",
                    Vec2::new(10.0, 30.0),
                    Vec2::new(-90.0, -720.0),
                    1.6,
                    Color::srgb(0.75, 0.8, 0.9),
                    0.0,
                ),
                WeatherKind::Snow => (
                    "flake",
                    Vec2::splat(6.0 + 6.0 * sprites.rand()),
                    Vec2::new(-12.0, -55.0 - 30.0 * sprites.rand()),
                    9.0,
                    Color::WHITE,
                    0.0,
                ),
                WeatherKind::Fog => (
                    "fog",
                    Vec2::splat(CELL * (5.0 + 4.0 * sprites.rand())),
                    Vec2::new(10.0 + 8.0 * sprites.rand(), 2.0),
                    24.0,
                    Color::srgb(0.75, 0.78, 0.84),
                    0.0,
                ),
                WeatherKind::Leaves => (
                    "leaf",
                    Vec2::splat(12.0 + 6.0 * sprites.rand()),
                    Vec2::new(30.0, -40.0),
                    10.0,
                    Color::srgb(1.0, 0.85 + 0.15 * sprites.rand(), 0.7),
                    1.6,
                ),
                WeatherKind::Dust => (
                    "dot",
                    Vec2::splat(5.0),
                    Vec2::new(6.0, 4.0),
                    8.0,
                    Color::srgb(1.0, 0.92, 0.75),
                    0.0,
                ),
                WeatherKind::Embers => (
                    "dot",
                    Vec2::splat(7.0),
                    Vec2::new(8.0, 60.0),
                    4.5,
                    Color::srgb(1.0, 0.55, 0.2),
                    0.0,
                ),
                WeatherKind::Fireflies => (
                    "dot",
                    Vec2::splat(12.0),
                    Vec2::ZERO,
                    12.0,
                    Color::srgb(0.85, 1.0, 0.45),
                    0.0,
                ),
                WeatherKind::Magic => (
                    "dot",
                    Vec2::splat(9.0),
                    Vec2::new(0.0, 28.0),
                    6.0,
                    if sprites.rand() > 0.5 {
                        Color::srgb(0.7, 0.55, 1.0)
                    } else {
                        Color::srgb(0.5, 0.95, 1.0)
                    },
                    0.0,
                ),
                WeatherKind::Ash => (
                    "flake",
                    Vec2::splat(5.0),
                    Vec2::new(10.0, -25.0),
                    10.0,
                    Color::srgb(0.55, 0.53, 0.5),
                    0.0,
                ),
                WeatherKind::None => continue,
            };
            let phase = sprites.rand() * 6.28;
            let life = life * (0.6 + 0.8 * sprites.rand());
            let angle = if weather.kind == WeatherKind::Rain {
                (vel.x / vel.y).atan()
            } else {
                0.0
            };
            commands.spawn((
                Particle {
                    vel,
                    life,
                    max: life,
                    spin,
                    phase,
                    kind: Kind::Weather(weather.kind),
                },
                Sprite {
                    image: sprites.get(img, &mut images),
                    custom_size: Some(if weather.kind == WeatherKind::Fog {
                        size
                    } else {
                        size * scale.max(1.0)
                    }),
                    color: color.with_alpha(0.0),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, 4.2).with_rotation(Quat::from_rotation_z(angle)),
            ));
        }
    }

    // Embers rising from lava cells.
    if !reduced
        && mult >= 0.5
        && let Some(map) = &gv.map
        && let Some(layer) = map
            .layers
            .iter()
            .find(|l| l.kind == dnd_world::map::LayerKind::Terrain)
    {
        let lava: Vec<(i32, i32)> = layer
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, v)| {
                **v != 0
                    && map
                        .palette
                        .get(**v as usize - 1)
                        .is_some_and(|p| p.asset_id == "lava")
            })
            .map(|(i, _)| ((i as i32) % map.width, (i as i32) / map.width))
            .collect();
        let want = ((lava.len() as f32 * 0.8 * mult) as usize).min(160);
        let have = counts.get(&Kind::LavaEmber).copied().unwrap_or(0);
        for _ in 0..want.saturating_sub(have).min(8) {
            let c = lava[(sprites.rand() * lava.len() as f32) as usize % lava.len()];
            let p = cell_to_world(c)
                + Vec2::new((sprites.rand() - 0.5) * CELL, (sprites.rand() - 0.5) * CELL);
            let life = 1.2 + sprites.rand() * 1.6;
            let phase = sprites.rand() * 6.28;
            commands.spawn((
                Particle {
                    vel: Vec2::new((sprites.rand() - 0.5) * 20.0, 30.0 + sprites.rand() * 40.0),
                    life,
                    max: life,
                    spin: 0.0,
                    phase,
                    kind: Kind::LavaEmber,
                },
                Sprite {
                    image: sprites.get("dot", &mut images),
                    custom_size: Some(Vec2::splat(4.0 + sprites.rand() * 4.0)),
                    color: Color::srgba(1.0, 0.6, 0.2, 0.0),
                    ..default()
                },
                Transform::from_xyz(p.x, p.y, 4.1),
            ));
        }
    }
}
