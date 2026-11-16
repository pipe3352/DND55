//! Combat and spell visual effects: typed particle bursts, slash arcs, shock rings, zig-zag
//! lightning, readable damage numbers with a dark under-print, and a short camera shake on
//! critical hits. Visuals only follow host events; they never decide outcomes. Effects stay brief
//! and small so the tactical map remains readable; Reduced Motion removes shake and most motion.

use crate::settings::GameSettings;
use crate::state::CELL;
use crate::terrain_art::Canvas;
use bevy::prelude::*;
use dnd_session::event::Vfx;

#[derive(Resource)]
pub struct FxAssets {
    pub font: Handle<Font>,
    dot: Handle<Image>,
    ring: Handle<Image>,
    shard: Handle<Image>,
    slash: Handle<Image>,
}

#[derive(Component)]
pub struct FxParticle {
    vel: Vec2,
    gravity: f32,
    drag: f32,
    life: f32,
    max: f32,
    grow: f32,
    base_alpha: f32,
}

/// Camera shake request (seconds left, magnitude in world units).
#[derive(Resource, Default)]
pub struct CameraShake {
    pub t: f32,
    pub mag: f32,
}

pub fn setup_fx(
    mut commands: Commands,
    mut fonts: ResMut<Assets<Font>>,
    mut images: ResMut<Assets<Image>>,
) {
    let font = fonts.add(Font::from_bytes(
        include_bytes!("../assets/fonts/CrimsonText-SemiBold.ttf").to_vec(),
    ));
    let mut dot = Canvas::new(32, 32);
    dot.blob(16.0, 16.0, 16.0, 16.0, [1.0, 1.0, 1.0, 0.7]);
    dot.disc(16.0, 16.0, 4.0, [1.0, 1.0, 1.0, 1.0]);
    let mut ring = Canvas::new(64, 64);
    ring.fill(|x, y| {
        let d = ((x as f32 + 0.5 - 32.0).powi(2) + (y as f32 + 0.5 - 32.0).powi(2)).sqrt();
        let a = (1.0 - ((d - 27.0).abs() / 4.0)).clamp(0.0, 1.0);
        [1.0, 1.0, 1.0, a]
    });
    let mut shard = Canvas::new(16, 32);
    shard.poly(
        &[(8.0, 0.0), (14.0, 16.0), (8.0, 32.0), (2.0, 16.0)],
        [1.0, 1.0, 1.0, 1.0],
        0.0,
    );
    let mut slash = Canvas::new(64, 64);
    for i in 0..60 {
        let t = i as f32 / 60.0;
        let a = -1.2 + t * 2.4;
        let w = (1.0 - (t - 0.5).abs() * 2.0) * 4.0 + 0.5;
        slash.disc(
            32.0 + a.cos() * 26.0,
            32.0 + a.sin() * 26.0,
            w,
            [1.0, 1.0, 1.0, 0.9],
        );
    }
    commands.insert_resource(FxAssets {
        font,
        dot: images.add(dot.into_image()),
        ring: images.add(ring.into_image()),
        shard: images.add(shard.into_image()),
        slash: images.add(slash.into_image()),
    });
}

fn particle(
    commands: &mut Commands,
    image: Handle<Image>,
    pos: Vec2,
    vel: Vec2,
    size: f32,
    color: Color,
    life: f32,
    gravity: f32,
    grow: f32,
    rot: f32,
) {
    commands.spawn((
        FxParticle {
            vel,
            gravity,
            drag: 1.8,
            life,
            max: life,
            grow,
            base_alpha: color.alpha(),
        },
        Sprite {
            image,
            custom_size: Some(Vec2::splat(size)),
            color,
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y, 9.5).with_rotation(Quat::from_rotation_z(rot)),
    ));
}

/// Cheap deterministic pseudo-random sequence (no RNG resource needed for visuals).
struct Seq(u32);
impl Seq {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// A burst of particles typed by damage/effect kind at `pos`.
pub fn burst(
    commands: &mut Commands,
    fx: &FxAssets,
    settings: &GameSettings,
    pos: Vec2,
    vfx: Vfx,
    strength: f32,
) {
    let mult = settings.quality.particles() * if settings.reduced_motion { 0.3 } else { 1.0 };
    let n = ((14.0 * strength * mult) as usize).clamp(2, 60);
    let mut r = Seq((pos.x.to_bits() ^ pos.y.to_bits().rotate_left(7)) | 1);
    let speed = if settings.reduced_motion { 0.3 } else { 1.0 };
    for _ in 0..n {
        let a = r.next() * std::f32::consts::TAU;
        let dir = Vec2::new(a.cos(), a.sin());
        let (img, vel, size, color, life, gravity, rot) = match vfx {
            Vfx::Fire => (
                fx.dot.clone(),
                dir * 60.0 + Vec2::Y * 70.0,
                10.0,
                Color::srgb(1.0, 0.45 + r.next() * 0.35, 0.1),
                0.7,
                40.0,
                0.0,
            ),
            Vfx::Cold => (
                fx.shard.clone(),
                dir * 140.0,
                9.0,
                Color::srgb(0.75, 0.92, 1.0),
                0.55,
                0.0,
                a - std::f32::consts::FRAC_PI_2,
            ),
            Vfx::Lightning => (
                fx.dot.clone(),
                dir * 180.0,
                6.0,
                Color::srgb(1.0, 1.0, 0.6),
                0.3,
                0.0,
                0.0,
            ),
            Vfx::Radiant => (
                fx.shard.clone(),
                dir * 170.0,
                8.0,
                Color::srgb(1.0, 0.95, 0.65),
                0.5,
                0.0,
                a - std::f32::consts::FRAC_PI_2,
            ),
            Vfx::Necrotic => (
                fx.dot.clone(),
                dir * 25.0 + Vec2::Y * 35.0,
                16.0,
                Color::srgba(0.35, 0.1, 0.45, 0.8),
                1.0,
                10.0,
                0.0,
            ),
            Vfx::Poison => (
                fx.dot.clone(),
                dir * 30.0 + Vec2::Y * 25.0,
                12.0,
                Color::srgba(0.45, 0.85, 0.25, 0.8),
                0.9,
                15.0,
                0.0,
            ),
            Vfx::Acid => (
                fx.dot.clone(),
                dir * 70.0,
                9.0,
                Color::srgb(0.7, 0.95, 0.15),
                0.6,
                -120.0,
                0.0,
            ),
            Vfx::Force | Vfx::Arcane => (
                fx.dot.clone(),
                dir * 110.0,
                8.0,
                Color::srgb(0.65, 0.55, 1.0),
                0.5,
                0.0,
                0.0,
            ),
            Vfx::Thunder => (
                fx.dot.clone(),
                dir * 150.0,
                7.0,
                Color::srgb(0.6, 0.65, 0.95),
                0.4,
                0.0,
                0.0,
            ),
            Vfx::Psychic => (
                fx.dot.clone(),
                dir * 50.0,
                10.0,
                Color::srgb(0.95, 0.45, 0.85),
                0.8,
                0.0,
                0.0,
            ),
            Vfx::Healing => (
                fx.dot.clone(),
                dir * 18.0 + Vec2::Y * 55.0,
                9.0,
                Color::srgb(0.5, 1.0, 0.55),
                1.0,
                0.0,
                0.0,
            ),
            Vfx::Buff => (
                fx.dot.clone(),
                Vec2::new(dir.x * 30.0, 50.0),
                8.0,
                Color::srgb(1.0, 0.85, 0.35),
                0.9,
                0.0,
                0.0,
            ),
            Vfx::Light => (
                fx.dot.clone(),
                dir * 40.0,
                12.0,
                Color::srgb(1.0, 0.97, 0.8),
                0.9,
                0.0,
                0.0,
            ),
            Vfx::Slash | Vfx::Pierce | Vfx::Bludgeon | Vfx::Arrow => (
                fx.dot.clone(),
                dir * 120.0,
                5.0,
                Color::srgb(1.0, 0.9, 0.75),
                0.25,
                -200.0,
                0.0,
            ),
        };
        let jitter = 0.6 + r.next() * 0.8;
        let start = pos + dir * CELL * 0.1 * r.next();
        particle(
            commands,
            img,
            start,
            vel * jitter * speed,
            size * (0.7 + r.next() * 0.6),
            color,
            life * jitter,
            gravity,
            0.0,
            rot,
        );
    }
    // Signature shapes.
    match vfx {
        Vfx::Force | Vfx::Thunder | Vfx::Buff | Vfx::Arcane => {
            ring(commands, fx, pos, vfx_ring_color(vfx), 0.5)
        }
        Vfx::Light | Vfx::Healing => ring(commands, fx, pos, vfx_ring_color(vfx), 0.7),
        _ => {}
    }
}

fn vfx_ring_color(v: Vfx) -> Color {
    match v {
        Vfx::Thunder => Color::srgb(0.7, 0.75, 1.0),
        Vfx::Buff => Color::srgb(1.0, 0.85, 0.35),
        Vfx::Healing => Color::srgb(0.5, 1.0, 0.55),
        Vfx::Light => Color::srgb(1.0, 0.97, 0.8),
        _ => Color::srgb(0.7, 0.6, 1.0),
    }
}

/// Expanding shock ring.
pub fn ring(commands: &mut Commands, fx: &FxAssets, pos: Vec2, color: Color, life: f32) {
    commands.spawn((
        FxParticle {
            vel: Vec2::ZERO,
            gravity: 0.0,
            drag: 0.0,
            life,
            max: life,
            grow: 2.6,
            base_alpha: 0.9,
        },
        Sprite {
            image: fx.ring.clone(),
            custom_size: Some(Vec2::splat(CELL * 0.8)),
            color: color.with_alpha(0.9),
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y, 9.4),
    ));
}

/// Weapon slash arc across the target, oriented along the attack.
pub fn slash(commands: &mut Commands, fx: &FxAssets, at: Vec2, dir: Vec2, crit: bool) {
    let angle = dir.y.atan2(dir.x);
    commands.spawn((
        FxParticle {
            vel: Vec2::ZERO,
            gravity: 0.0,
            drag: 0.0,
            life: 0.28,
            max: 0.28,
            grow: 0.35,
            base_alpha: 1.0,
        },
        Sprite {
            image: fx.slash.clone(),
            custom_size: Some(Vec2::splat(CELL * if crit { 1.4 } else { 1.05 })),
            color: if crit {
                Color::srgb(1.0, 0.85, 0.4)
            } else {
                Color::srgb(1.0, 0.97, 0.9)
            },
            ..default()
        },
        Transform::from_xyz(at.x, at.y, 9.6).with_rotation(Quat::from_rotation_z(angle + 0.6)),
    ));
}

/// Jagged lightning bolt made of short bright segments.
pub fn lightning(commands: &mut Commands, fx: &FxAssets, from: Vec2, to: Vec2) {
    let mut r = Seq((from.x.to_bits() ^ to.y.to_bits()) | 1);
    let steps = ((to - from).length() / (CELL * 0.45)).ceil().max(3.0) as usize;
    let normal = (to - from).perp().normalize_or_zero();
    let mut prev = from;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let off = if i == steps {
            0.0
        } else {
            (r.next() - 0.5) * CELL * 0.5
        };
        let p = from.lerp(to, t) + normal * off;
        let d = p - prev;
        for (w, c, life) in [
            (7.0, Color::srgba(0.6, 0.7, 1.0, 0.6), 0.32),
            (3.0, Color::srgb(1.0, 1.0, 0.85), 0.25),
        ] {
            commands.spawn((
                FxParticle {
                    vel: Vec2::ZERO,
                    gravity: 0.0,
                    drag: 0.0,
                    life,
                    max: life,
                    grow: 0.0,
                    base_alpha: c.alpha(),
                },
                Sprite {
                    image: fx.dot.clone(),
                    custom_size: Some(Vec2::new(d.length() + w, w)),
                    color: c,
                    ..default()
                },
                Transform::from_xyz((prev.x + p.x) / 2.0, (prev.y + p.y) / 2.0, 9.6)
                    .with_rotation(Quat::from_rotation_z(d.y.atan2(d.x))),
            ));
        }
        prev = p;
    }
}

/// Floating combat text with a dark under-print for readability on any terrain.
pub fn float_text(
    commands: &mut Commands,
    fx: &FxAssets,
    pos: Vec2,
    text: String,
    size: f32,
    color: Color,
    life: f32,
    drift: Vec2,
) {
    for (offset, c, z) in [
        (
            Vec2::new(2.0, -2.0),
            Color::srgba(0.05, 0.03, 0.02, 0.85),
            10.0,
        ),
        (Vec2::ZERO, color, 10.1),
    ] {
        commands.spawn((
            crate::fx::FloatText {
                vel: drift,
                life,
                max: life,
            },
            Text2d::new(text.clone()),
            TextFont {
                font: fx.font.clone().into(),
                font_size: bevy::text::FontSize::Px(size),
                ..default()
            },
            TextColor(c),
            Transform::from_xyz(pos.x + offset.x, pos.y + offset.y, z),
        ));
    }
}

pub fn animate_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut FxParticle, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs().min(0.05);
    for (e, mut p, mut t, mut s) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        p.vel.y -= p.gravity * dt;
        let damp = (1.0 - p.drag * dt).max(0.0);
        p.vel *= damp;
        t.translation += (p.vel * dt).extend(0.0);
        let k = p.life / p.max;
        t.scale = Vec3::splat(1.0 + p.grow * (1.0 - k));
        s.color.set_alpha(p.base_alpha * k.powf(0.7));
    }
}

/// Applies (and decays) the camera shake.
pub fn apply_shake(
    time: Res<Time>,
    settings: Res<GameSettings>,
    mut shake: ResMut<CameraShake>,
    mut cam: Query<&mut Transform, With<Camera2d>>,
    mut offset: Local<Vec2>,
) {
    let Ok(mut t) = cam.single_mut() else { return };
    // Remove last frame's offset so panning/zooming stays exact.
    t.translation -= offset.extend(0.0);
    *offset = Vec2::ZERO;
    if shake.t <= 0.0 || settings.reduced_motion {
        shake.t = 0.0;
        return;
    }
    shake.t -= time.delta_secs();
    let e = time.elapsed_secs() * 60.0;
    let k = shake.t.max(0.0) * 4.0;
    *offset = Vec2::new((e * 1.7).sin(), (e * 2.3).cos()) * shake.mag * k.min(1.0);
    t.translation += offset.extend(0.0);
}
