//! Original procedural map tokens: an emblem on a coloured field (`face`) inside a frame whose
//! *shape* tells the side — friendly: smooth gold ring; hostile: spiked iron ring; neutral:
//! bronze octagon — so sides read without relying on red/green alone.

use super::{Canvas, INK, Rgba, alpha, c3, mix};
use bevy::image::Image;
use std::f32::consts::{PI, TAU};

/// Token texture size in pixels.
pub const SIZE: u32 = 128;
const C: f32 = SIZE as f32 / 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Friendly,
    Hostile,
    Neutral,
}

type Pt = (f32, f32);
const IV: Rgba = [0.93, 0.89, 0.79, 1.0];

/// Emblems for monsters, keyed by lowercase creature type.
#[cfg_attr(not(test), allow(dead_code))]
pub const MONSTER_EMBLEMS: [&str; 14] = [
    "undead",
    "beast",
    "dragon",
    "fiend",
    "humanoid",
    "aberration",
    "construct",
    "elemental",
    "fey",
    "giant",
    "monstrosity",
    "ooze",
    "plant",
    "celestial",
];

/// Emblem scale: emblems are authored in ±36 px and drawn slightly larger to read at map size.
const K: f32 = 1.12;
fn p(x: f32, y: f32) -> Pt {
    (C + x * K, C + y * K)
}
/// Ivory stroke with an ink outline.
fn ln(c: &mut Canvas, a: Pt, b: Pt, w: f32) {
    c.line(a, b, w + 4.0, INK);
    c.line(a, b, w, IV);
}
fn pg(c: &mut Canvas, pts: &[Pt], col: Rgba) {
    c.poly(pts, col, 2.5);
}
fn arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, n: usize) -> Vec<Pt> {
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            p(cx + a.cos() * r, cy + a.sin() * r)
        })
        .collect()
}
fn ell(cx: f32, cy: f32, rx: f32, ry: f32, ang: f32) -> Vec<Pt> {
    let (sa, ca) = ang.sin_cos();
    (0..24)
        .map(|i| {
            let t = i as f32 / 24.0 * TAU;
            let (x, y) = (t.cos() * rx, t.sin() * ry);
            p(cx + x * ca - y * sa, cy + x * sa + y * ca)
        })
        .collect()
}
fn polyline(c: &mut Canvas, pts: &[Pt], w: f32) {
    for s in pts.windows(2) {
        c.line(s[0], s[1], w + 4.0, INK);
    }
    for s in pts.windows(2) {
        c.line(s[0], s[1], w, IV);
    }
}
fn dot(c: &mut Canvas, x: f32, y: f32, r: f32, col: Rgba) {
    let (x, y) = p(x, y);
    c.disc(x, y, r * K + 2.0, INK);
    c.disc(x, y, r * K, col);
}

fn field(side: Side) -> (Rgba, Rgba) {
    match side {
        Side::Friendly => (c3(34, 58, 104), c3(70, 104, 160)),
        Side::Hostile => (c3(88, 22, 24), c3(140, 46, 40)),
        Side::Neutral => (c3(70, 72, 56), c3(118, 116, 92)),
    }
}

/// Emblem on a coloured field (round, radius 50 px). Unknown emblems get "monstrosity" fangs.
pub fn face(emblem: &str, side: Side) -> Image {
    let mut c = Canvas::new(SIZE, SIZE);
    let (dark, light) = field(side);
    for k in 0..12 {
        let t = k as f32 / 11.0;
        c.disc(
            C - t * 8.0,
            C - t * 10.0,
            50.0 * (1.0 - t * 0.55),
            mix(dark, light, t * 0.8),
        );
    }
    emblem_art(&mut c, emblem);
    c.into_image()
}

fn emblem_art(c: &mut Canvas, emblem: &str) {
    match emblem {
        "warrior" => {
            for s in [-1.0f32, 1.0] {
                ln(c, p(-24.0 * s, 24.0), p(22.0 * s, -26.0), 6.0);
                ln(c, p(-12.0 * s, 4.0), p(-2.0 * s, 14.0), 4.0);
                dot(c, -27.0 * s, 27.0, 4.0, IV);
            }
        }
        "knight" => {
            let mut dome = arc(0.0, -2.0, 22.0, PI, TAU, 16);
            dome.push(p(22.0, 26.0));
            dome.push(p(-22.0, 26.0));
            pg(c, &dome, IV);
            c.line(p(-16.0, 4.0), p(16.0, 4.0), 5.0, INK);
            c.line(p(0.0, 8.0), p(0.0, 22.0), 3.0, INK);
            for i in 0..4 {
                c.disc(
                    C - 10.0 + i as f32 * 2.0,
                    C + 12.0 + i as f32 * 3.0,
                    1.2,
                    INK,
                );
            }
            polyline(c, &arc(10.0, -26.0, 14.0, PI * 1.05, PI * 1.9, 10), 5.0);
        }
        "mage" => {
            pg(
                c,
                &[
                    p(-28.0, 18.0),
                    p(-6.0, -12.0),
                    p(6.0, -34.0),
                    p(10.0, -22.0),
                    p(4.0, -10.0),
                    p(24.0, 18.0),
                ],
                IV,
            );
            pg(c, &ell(-2.0, 20.0, 32.0, 7.0, 0.0), IV);
            c.poly(
                &super::object_star(p(-2.0, 2.0).0, p(-2.0, 2.0).1, 7.0 * K, 3.0 * K, 5),
                c3(240, 200, 80),
                1.5,
            );
        }
        "archer" => {
            polyline(c, &arc(-18.0, 0.0, 30.0, -PI / 2.6, PI / 2.6, 14), 5.0);
            let (top, bot) = (
                p(-18.0 + 30.0 * (PI / 2.6).cos(), -30.0 * (PI / 2.6).sin()),
                p(-18.0 + 30.0 * (PI / 2.6).cos(), 30.0 * (PI / 2.6).sin()),
            );
            c.line(top, bot, 1.6, IV);
            ln(c, p(-20.0, 0.0), p(26.0, 0.0), 3.0);
            pg(c, &[p(32.0, 0.0), p(22.0, -6.0), p(22.0, 6.0)], IV);
            pg(
                c,
                &[p(-20.0, 0.0), p(-28.0, -7.0), p(-24.0, 0.0), p(-28.0, 7.0)],
                c3(200, 70, 60),
            );
        }
        "rogue" => {
            pg(
                c,
                &[
                    p(0.0, -32.0),
                    p(24.0, -4.0),
                    p(26.0, 26.0),
                    p(-26.0, 26.0),
                    p(-24.0, -4.0),
                ],
                IV,
            );
            pg(c, &ell(0.0, 4.0, 14.0, 16.0, 0.0), c3(20, 16, 18));
            c.disc(p(-6.0, 2.0).0, p(-6.0, 2.0).1, 2.4 * K, c3(250, 220, 120));
            c.disc(p(6.0, 2.0).0, p(6.0, 2.0).1, 2.4 * K, c3(250, 220, 120));
            ln(c, p(16.0, 30.0), p(32.0, 10.0), 3.5);
        }
        "cleric" => {
            for i in 0..12 {
                let a = i as f32 / 12.0 * TAU;
                let r = if i % 2 == 0 { 34.0 } else { 26.0 };
                pg(
                    c,
                    &[
                        p(a.cos() * r, a.sin() * r),
                        p((a + 0.16).cos() * 14.0, (a + 0.16).sin() * 14.0),
                        p((a - 0.16).cos() * 14.0, (a - 0.16).sin() * 14.0),
                    ],
                    c3(244, 214, 120),
                );
            }
            dot(c, 0.0, 0.0, 15.0, IV);
            c.line(p(0.0, -9.0), p(0.0, 9.0), 4.0, c3(190, 150, 60));
            c.line(p(-7.0, -3.0), p(7.0, -3.0), 4.0, c3(190, 150, 60));
        }
        "paladin" => {
            pg(
                c,
                &[
                    p(-24.0, -26.0),
                    p(24.0, -26.0),
                    p(24.0, 0.0),
                    p(0.0, 32.0),
                    p(-24.0, 0.0),
                ],
                IV,
            );
            c.line(p(0.0, -20.0), p(0.0, 22.0), 7.0, c3(170, 40, 40));
            c.line(p(-17.0, -8.0), p(17.0, -8.0), 7.0, c3(170, 40, 40));
        }
        "druid" => {
            pg(c, &ell(0.0, -2.0, 30.0, 15.0, -PI / 4.0), c3(150, 200, 110));
            ln(c, p(-26.0, 26.0), p(18.0, -18.0), 2.5);
            for t in [-12.0f32, -2.0, 8.0] {
                c.line(p(t, -t), p(t - 9.0, -t - 7.0), 1.6, c3(40, 70, 30));
                c.line(p(t, -t), p(t + 7.0, -t + 9.0), 1.6, c3(40, 70, 30));
            }
        }
        "barbarian" => {
            ln(c, p(0.0, -32.0), p(0.0, 32.0), 5.0);
            for s in [-1.0f32, 1.0] {
                let mut blade = arc(
                    0.0,
                    -14.0,
                    26.0,
                    if s > 0.0 { -0.75 } else { PI - 0.75 },
                    if s > 0.0 { 0.75 } else { PI + 0.75 },
                    10,
                );
                blade.push(p(4.0 * s, -4.0));
                blade.push(p(4.0 * s, -24.0));
                pg(c, &blade, IV);
            }
        }
        "bard" => {
            ln(c, p(6.0, -6.0), p(26.0, -30.0), 7.0);
            pg(
                c,
                &[
                    p(22.0, -30.0),
                    p(30.0, -38.0),
                    p(34.0, -30.0),
                    p(28.0, -24.0),
                ],
                IV,
            );
            pg(c, &ell(-8.0, 10.0, 20.0, 24.0, -0.7), IV);
            dot(c, -6.0, 6.0, 5.0, c3(40, 26, 20));
            for k in [-1.5f32, 0.0, 1.5] {
                c.line(
                    p(-14.0 + k, 18.0 + k),
                    p(24.0 + k, -28.0 + k),
                    0.8,
                    c3(120, 90, 60),
                );
            }
        }
        "monk" => {
            for i in 0..5 {
                let a = -PI / 2.0 + (i as f32 - 2.0) * 0.55;
                pg(
                    c,
                    &ell(a.cos() * 16.0, 8.0 + a.sin() * 16.0, 16.0, 7.0, a),
                    if i % 2 == 0 { IV } else { c3(240, 200, 170) },
                );
            }
            pg(c, &ell(0.0, 22.0, 28.0, 6.0, 0.0), c3(200, 220, 180));
            dot(c, 0.0, 6.0, 5.0, c3(250, 210, 90));
        }
        "ranger" => {
            pg(c, &ell(0.0, 12.0, 16.0, 13.0, 0.0), IV);
            for (x, y) in [(-20.0, -6.0), (-8.0, -18.0), (8.0, -18.0), (20.0, -6.0)] {
                pg(c, &ell(x, y, 6.5, 8.5, 0.0), IV);
            }
        }
        "sorcerer" => {
            pg(
                c,
                &[
                    p(0.0, -36.0),
                    p(10.0, -16.0),
                    p(22.0, -18.0),
                    p(20.0, 8.0),
                    p(14.0, 26.0),
                    p(-14.0, 26.0),
                    p(-22.0, 6.0),
                    p(-14.0, -10.0),
                    p(-6.0, -4.0),
                ],
                c3(250, 180, 80),
            );
            pg(
                c,
                &[
                    p(0.0, -8.0),
                    p(9.0, 8.0),
                    p(6.0, 22.0),
                    p(-6.0, 22.0),
                    p(-9.0, 8.0),
                ],
                c3(255, 236, 170),
            );
        }
        "warlock" => {
            pg(
                c,
                &[
                    p(-32.0, 0.0),
                    p(-14.0, -16.0),
                    p(14.0, -16.0),
                    p(32.0, 0.0),
                    p(14.0, 16.0),
                    p(-14.0, 16.0),
                ],
                IV,
            );
            dot(c, 0.0, 0.0, 12.0, c3(120, 40, 160));
            pg(c, &ell(0.0, 0.0, 3.0, 11.0, 0.0), c3(10, 6, 10));
            for i in 0..3 {
                let x = -12.0 + i as f32 * 12.0;
                polyline(c, &[p(x, 18.0), p(x - 4.0, 26.0), p(x + 2.0, 34.0)], 2.5);
            }
        }
        // ---- monsters by creature type
        "undead" => {
            pg(
                c,
                &[p(-12.0, 14.0), p(12.0, 14.0), p(10.0, 28.0), p(-10.0, 28.0)],
                IV,
            );
            dot(c, 0.0, -4.0, 24.0, IV);
            c.disc(p(-9.0, -2.0).0, p(-9.0, -2.0).1, 6.5 * K, INK);
            c.disc(p(9.0, -2.0).0, p(9.0, -2.0).1, 6.5 * K, INK);
            pg(c, &[p(0.0, 6.0), p(3.0, 12.0), p(-3.0, 12.0)], INK);
            for i in 0..4 {
                c.line(
                    p(-7.0 + i as f32 * 4.6, 16.0),
                    p(-7.0 + i as f32 * 4.6, 26.0),
                    1.4,
                    INK,
                );
            }
        }
        "beast" => {
            for i in 0..3 {
                let o = (i as f32 - 1.0) * 14.0;
                polyline(
                    c,
                    &[p(-20.0 + o, -26.0), p(-4.0 + o, 0.0), p(4.0 + o, 28.0)],
                    4.0 - (i as f32 - 1.0).abs(),
                );
            }
        }
        "dragon" => {
            pg(
                c,
                &[
                    p(-30.0, -24.0),
                    p(-4.0, -8.0),
                    p(30.0, -30.0),
                    p(24.0, 6.0),
                    p(12.0, 2.0),
                    p(14.0, 18.0),
                    p(0.0, 10.0),
                    p(-6.0, 26.0),
                    p(-14.0, 8.0),
                    p(-28.0, 12.0),
                ],
                IV,
            );
            for (a, b) in [
                ((-4.0, -8.0), (24.0, 6.0)),
                ((-4.0, -8.0), (14.0, 18.0)),
                ((-4.0, -8.0), (-6.0, 26.0)),
            ] {
                c.line(p(a.0, a.1), p(b.0, b.1), 1.4, INK);
            }
        }
        "fiend" => {
            for s in [-1.0f32, 1.0] {
                pg(
                    c,
                    &[
                        p(8.0 * s, -2.0),
                        p(24.0 * s, -12.0),
                        p(30.0 * s, -34.0),
                        p(34.0 * s, -10.0),
                        p(18.0 * s, 8.0),
                    ],
                    IV,
                );
            }
            pg(
                c,
                &[
                    p(-16.0, -2.0),
                    p(16.0, -2.0),
                    p(10.0, 22.0),
                    p(0.0, 30.0),
                    p(-10.0, 22.0),
                ],
                c3(200, 70, 50),
            );
            c.disc(p(-6.0, 6.0).0, p(-6.0, 6.0).1, 2.6 * K, c3(255, 230, 120));
            c.disc(p(6.0, 6.0).0, p(6.0, 6.0).1, 2.6 * K, c3(255, 230, 120));
        }
        "humanoid" => {
            ln(c, p(0.0, -34.0), p(0.0, 18.0), 6.0);
            ln(c, p(-14.0, 14.0), p(14.0, 14.0), 5.0);
            ln(c, p(0.0, 18.0), p(0.0, 28.0), 4.0);
            dot(c, 0.0, 32.0, 4.0, IV);
        }
        "aberration" => {
            for i in 0..4 {
                let x = -18.0 + i as f32 * 12.0;
                let pts: Vec<Pt> = (0..8)
                    .map(|k| p(x + (k as f32 * 0.9).sin() * 5.0, 4.0 + k as f32 * 4.0))
                    .collect();
                polyline(c, &pts, 4.0);
            }
            pg(c, &ell(0.0, -10.0, 22.0, 16.0, 0.0), c3(170, 210, 150));
            dot(c, 0.0, -10.0, 8.0, c3(240, 200, 60));
            c.disc(p(0.0, -10.0).0, p(0.0, -10.0).1, 3.0 * K, INK);
        }
        "construct" => {
            pg(
                c,
                &super::object_star(p(0.0, 0.0).0, p(0.0, 0.0).1, 32.0 * K, 24.0 * K, 10),
                IV,
            );
            dot(c, 0.0, 0.0, 14.0, c3(150, 150, 160));
            c.disc(p(0.0, 0.0).0, p(0.0, 0.0).1, 6.0 * K, INK);
        }
        "elemental" => {
            pg(
                c,
                &[p(0.0, -34.0), p(24.0, 0.0), p(0.0, 34.0), p(-24.0, 0.0)],
                c3(140, 210, 240),
            );
            polyline(c, &arc(0.0, 0.0, 12.0, 0.0, PI * 1.6, 10), 3.0);
            polyline(c, &arc(0.0, 0.0, 5.0, PI, PI * 2.4, 8), 2.0);
        }
        "fey" => {
            for s in [-1.0f32, 1.0] {
                pg(
                    c,
                    &ell(16.0 * s, -10.0, 16.0, 11.0, 0.5 * s),
                    c3(200, 170, 240),
                );
                pg(
                    c,
                    &ell(12.0 * s, 14.0, 11.0, 8.0, -0.5 * s),
                    c3(240, 190, 220),
                );
            }
            ln(c, p(0.0, -18.0), p(0.0, 24.0), 4.0);
        }
        "giant" => {
            pg(
                c,
                &[
                    p(-6.0, 32.0),
                    p(6.0, 32.0),
                    p(18.0, -22.0),
                    p(10.0, -34.0),
                    p(-6.0, -30.0),
                    p(-10.0, -16.0),
                ],
                c3(180, 130, 80),
            );
            for (x, y) in [(6.0, -24.0), (0.0, -12.0), (8.0, -6.0)] {
                dot(c, x, y, 2.0, c3(120, 84, 50));
            }
        }
        "ooze" => {
            pg(
                c,
                &[
                    p(-28.0, 10.0),
                    p(-22.0, -14.0),
                    p(0.0, -24.0),
                    p(22.0, -14.0),
                    p(28.0, 10.0),
                    p(18.0, 16.0),
                    p(14.0, 30.0),
                    p(8.0, 16.0),
                    p(-6.0, 16.0),
                    p(-10.0, 26.0),
                    p(-14.0, 16.0),
                ],
                c3(150, 220, 110),
            );
            dot(c, -8.0, -6.0, 4.0, c3(230, 255, 210));
            dot(c, 10.0, 0.0, 2.5, c3(230, 255, 210));
        }
        "plant" => {
            ln(c, p(0.0, 32.0), p(0.0, -6.0), 4.0);
            pg(c, &ell(-14.0, -10.0, 16.0, 8.0, 0.5), c3(150, 210, 110));
            pg(c, &ell(14.0, -10.0, 16.0, 8.0, -0.5), c3(150, 210, 110));
            pg(c, &ell(0.0, -24.0, 7.0, 12.0, 0.0), c3(170, 230, 130));
        }
        "celestial" => {
            c.poly(&ell(0.0, -26.0, 18.0, 6.0, 0.0), alpha(IV, 0.0), 0.0);
            polyline(c, &arc(0.0, -26.0, 16.0, 0.0, TAU, 20), 3.0);
            for s in [-1.0f32, 1.0] {
                pg(
                    c,
                    &[
                        p(4.0 * s, -6.0),
                        p(34.0 * s, -18.0),
                        p(30.0 * s, 0.0),
                        p(26.0 * s, 14.0),
                        p(6.0 * s, 16.0),
                    ],
                    IV,
                );
            }
            dot(c, 0.0, 4.0, 7.0, c3(250, 220, 120));
        }
        // "monstrosity" and anything unknown: fanged jaws.
        _ => {
            pg(
                c,
                &[p(-28.0, -14.0), p(28.0, -14.0), p(20.0, 8.0), p(-20.0, 8.0)],
                c3(120, 30, 40),
            );
            for i in 0..5 {
                let x = -20.0 + i as f32 * 10.0;
                pg(
                    c,
                    &[
                        p(x - 4.0, -14.0),
                        p(x + 4.0, -14.0),
                        p(x, -2.0 + (i % 2) as f32 * 6.0),
                    ],
                    IV,
                );
                pg(c, &[p(x - 3.0, 8.0), p(x + 3.0, 8.0), p(x, -2.0)], IV);
            }
            dot(c, -14.0, -24.0, 4.0, c3(250, 220, 60));
            dot(c, 14.0, -24.0, 4.0, c3(250, 220, 60));
        }
    }
}

/// The side frame (transparent centre), drawn over the face or a portrait.
pub fn frame(side: Side) -> Image {
    let mut c = Canvas::new(SIZE, SIZE);
    match side {
        Side::Friendly => {
            ring(&mut c, 54.0, 8.0, c3(212, 170, 70));
            for i in 0..4 {
                let a = PI / 4.0 + i as f32 * PI / 2.0;
                dot(
                    &mut c,
                    a.cos() * 54.0,
                    a.sin() * 54.0,
                    4.5,
                    c3(90, 150, 230),
                );
            }
        }
        Side::Hostile => {
            for i in 0..10 {
                let a = i as f32 / 10.0 * TAU;
                let tip = p(a.cos() * 63.0, a.sin() * 63.0);
                let l = p((a - 0.2).cos() * 46.0, (a - 0.2).sin() * 46.0);
                let r = p((a + 0.2).cos() * 46.0, (a + 0.2).sin() * 46.0);
                c.poly(&[tip, l, r], c3(128, 122, 128), 2.0);
                c.line(tip, p(a.cos() * 50.0, a.sin() * 50.0), 1.5, c3(220, 70, 50));
            }
            ring(&mut c, 50.0, 8.0, c3(92, 88, 94));
            ring(&mut c, 50.0, 2.5, c3(190, 50, 40));
        }
        Side::Neutral => {
            let oct = |r: f32| -> Vec<Pt> {
                (0..8)
                    .map(|i| {
                        let a = PI / 8.0 + i as f32 / 8.0 * TAU;
                        p(a.cos() * r, a.sin() * r)
                    })
                    .collect()
            };
            let (outer, inner) = (oct(60.0), oct(50.0));
            for i in 0..8 {
                let j = (i + 1) % 8;
                c.poly(
                    &[outer[i], outer[j], inner[j], inner[i]],
                    c3(176, 128, 72),
                    0.0,
                );
            }
            for pts in [outer, inner] {
                for i in 0..8 {
                    c.line(pts[i], pts[(i + 1) % 8], 2.0, INK);
                }
            }
        }
    }
    c.into_image()
}

/// Plain white ring (tinted by the sprite colour): selection and active-turn indicators.
pub fn indicator_ring() -> Image {
    let mut c = Canvas::new(SIZE, SIZE);
    for k in 0..6 {
        let r = 58.0 + k as f32;
        for i in 0..96 {
            let a = i as f32 / 96.0 * TAU;
            c.disc(
                C + a.cos() * r,
                C + a.sin() * r,
                1.4,
                alpha(IV, 1.0 - k as f32 * 0.16),
            );
        }
    }
    c.into_image()
}

fn ring(c: &mut Canvas, r: f32, w: f32, col: Rgba) {
    let n = 160;
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        c.disc(C + a.cos() * r, C + a.sin() * r, w / 2.0 + 1.5, INK);
    }
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        let shade = 0.8 + 0.35 * (a - PI * 1.25).cos().max(0.0);
        c.disc(
            C + a.cos() * r,
            C + a.sin() * r,
            w / 2.0,
            [col[0] * shade, col[1] * shade, col[2] * shade, 1.0],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(img: &Image) -> Vec<f32> {
        img.data
            .as_ref()
            .unwrap()
            .iter()
            .map(|v| *v as f32 / 255.0)
            .collect()
    }

    /// `DND55_TOKEN_SHEET=/path.rgba cargo test dump_token_sheet -- --ignored`: raw RGBA sheet.
    #[test]
    #[ignore]
    fn dump_token_sheet() {
        let Ok(path) = std::env::var("DND55_TOKEN_SHEET") else {
            return;
        };
        let mut all: Vec<&str> = dnd_rules::character::TOKEN_ICONS.to_vec();
        all.extend(MONSTER_EMBLEMS);
        let n = SIZE as usize;
        let cols = 7;
        let rows = all.len().div_ceil(cols);
        let (w, h) = (cols * n, rows * n);
        let mut out = vec![60u8; w * h * 4];
        for (k, e) in all.iter().enumerate() {
            let side = [Side::Friendly, Side::Hostile, Side::Neutral][k % 3];
            let (f, fr) = (px(&face(e, side)), px(&frame(side)));
            for y in 0..n {
                for x in 0..n {
                    let i = (y * n + x) * 4;
                    let o = ((k / cols * n + y) * w + k % cols * n + x) * 4;
                    for ch in 0..3 {
                        let base = f[i + ch] * f[i + 3] + 0.3 * (1.0 - f[i + 3]);
                        let v = fr[i + ch] * fr[i + 3] + base * (1.0 - fr[i + 3]);
                        out[o + ch] = (v.clamp(0.0, 1.0) * 255.0) as u8;
                    }
                    out[o + 3] = 255;
                }
            }
        }
        std::fs::write(&path, out).unwrap();
        std::fs::write(format!("{path}.size"), format!("{w} {h}")).unwrap();
    }

    #[test]
    fn every_token_emblem_is_distinct_and_frames_differ_by_shape() {
        let mut all: Vec<&str> = dnd_rules::character::TOKEN_ICONS.to_vec();
        all.extend(MONSTER_EMBLEMS);
        let faces: Vec<(&str, Vec<f32>)> = all
            .iter()
            .map(|e| (*e, px(&face(e, Side::Friendly))))
            .collect();
        for i in 0..faces.len() {
            for j in i + 1..faces.len() {
                let d: f32 = faces[i]
                    .1
                    .iter()
                    .zip(&faces[j].1)
                    .map(|(a, b)| (a - b).abs())
                    .sum::<f32>()
                    / (SIZE * SIZE) as f32;
                assert!(
                    d > 0.07,
                    "{} and {} tokens look alike ({d})",
                    faces[i].0,
                    faces[j].0
                );
            }
        }
        // Frame alpha masks (shape only, colour ignored) must differ between sides.
        let mask = |s: Side| -> Vec<bool> { px(&frame(s)).chunks(4).map(|p| p[3] > 0.5).collect() };
        let (f, h, n) = (
            mask(Side::Friendly),
            mask(Side::Hostile),
            mask(Side::Neutral),
        );
        let diff = |a: &[bool], b: &[bool]| a.iter().zip(b).filter(|(x, y)| x != y).count();
        assert!(diff(&f, &h) > 600 && diff(&f, &n) > 600 && diff(&h, &n) > 600);
    }
}
