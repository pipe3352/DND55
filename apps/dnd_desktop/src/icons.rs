//! Original illustrated icon set, painted procedurally with egui shapes.
//!
//! Style (inspired by hand-illustrated medieval emblems, not copied from any source): thick, slightly
//! irregular dark-ink outlines; muted earthy fills from the theme palette; a painted highlight;
//! strong silhouettes that stay readable from 16 px to 64 px. Every icon is drawn in a unit square,
//! so it scales without blurring and ships no image files.

use crate::theme::*;
use bevy_egui::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    // menu & HUD
    Book,
    D20,
    Quill,
    Banner,
    Chest,
    Helm,
    Gear,
    Door,
    Sword,
    Shield,
    Potion,
    Skull,
    Torch,
    Heart,
    Star,
    Flame,
    Snowflake,
    Bolt,
    Leaf,
    Eye,
    Crown,
    Scroll,
    Map,
    Bag,
    Campfire,
    Hourglass,
    // editor
    Pointer,
    Brush,
    Eraser,
    Square,
    Bucket,
    Tree,
    Flag,
    Trash,
    Undo,
    Redo,
    Seal,
    Play,
    Link,
    Lock,
    Search,
    Plus,
    Grid,
    Layers,
    Fit,
    Target,
    // classes
    Axe,
    Lute,
    Sun,
    Fist,
    Bow,
    Dagger,
    Hat,
    // abilities
    Hammer,
    Feather,
}

struct Pen<'a> {
    p: &'a Painter,
    r: Rect,
    ow: f32,
    seed: u32,
}

const OUT: Color32 = INK;

impl Pen<'_> {
    /// Unit coordinates → screen, with a tiny deterministic wobble for a hand-drawn feel.
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        let h = ((x * 97.0 + y * 57.0 + self.seed as f32).sin() * 43758.547).fract();
        let w = (h - 0.5) * 0.018;
        pos2(
            self.r.left() + (x + w) * self.r.width(),
            self.r.top() + (y - w) * self.r.height(),
        )
    }
    fn pts(&self, v: &[(f32, f32)]) -> Vec<Pos2> {
        v.iter().map(|(x, y)| self.pt(*x, *y)).collect()
    }
    fn s(&self, u: f32) -> f32 {
        u * self.r.width()
    }
    /// Convex filled shape with an ink outline.
    fn poly(&self, v: &[(f32, f32)], fill: Color32) {
        self.p.add(Shape::convex_polygon(
            self.pts(v),
            fill,
            Stroke::new(self.ow, OUT),
        ));
    }
    /// Convex fill without outline (for building concave silhouettes).
    fn fill(&self, v: &[(f32, f32)], fill: Color32) {
        self.p
            .add(Shape::convex_polygon(self.pts(v), fill, Stroke::NONE));
    }
    fn outline(&self, v: &[(f32, f32)]) {
        self.p
            .add(Shape::closed_line(self.pts(v), Stroke::new(self.ow, OUT)));
    }
    fn line(&self, v: &[(f32, f32)], w: f32, c: Color32) {
        self.p
            .add(Shape::line(self.pts(v), Stroke::new(self.s(w), c)));
    }
    fn ink(&self, v: &[(f32, f32)]) {
        self.p
            .add(Shape::line(self.pts(v), Stroke::new(self.ow, OUT)));
    }
    fn circle(&self, x: f32, y: f32, r: f32, fill: Color32) {
        self.p
            .circle(self.pt(x, y), self.s(r), fill, Stroke::new(self.ow, OUT));
    }
    fn dot(&self, x: f32, y: f32, r: f32, c: Color32) {
        self.p.circle_filled(self.pt(x, y), self.s(r), c);
    }
    fn ring(&self, x: f32, y: f32, r: f32, w: f32, c: Color32) {
        self.p
            .circle_stroke(self.pt(x, y), self.s(r), Stroke::new(self.s(w), c));
    }
    fn arc(&self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32, w: f32, c: Color32) {
        let pts: Vec<Pos2> = (0..=16)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / 16.0;
                self.pt(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        self.p.add(Shape::line(pts, Stroke::new(self.s(w), c)));
    }
    fn hi(&self, v: &[(f32, f32)]) {
        self.line(v, 0.045, Color32::from_white_alpha(120));
    }
}

const STEEL: Color32 = Color32::from_rgb(0xA9, 0xAD, 0xA8);
const STEEL_D: Color32 = Color32::from_rgb(0x6E, 0x72, 0x70);
const WOODC: Color32 = Color32::from_rgb(0x7A, 0x52, 0x34);
const FIRE: Color32 = Color32::from_rgb(0xE0, 0x7A, 0x2A);
const FIRE_Y: Color32 = Color32::from_rgb(0xF3, 0xC4, 0x5A);
const ICE: Color32 = Color32::from_rgb(0x9C, 0xC8, 0xD8);
const GREEN: Color32 = Color32::from_rgb(0x6C, 0x8C, 0x48);
const RED: Color32 = Color32::from_rgb(0x9C, 0x34, 0x2C);
const BONE: Color32 = Color32::from_rgb(0xE6, 0xDC, 0xC4);
const PURPLE: Color32 = Color32::from_rgb(0x6A, 0x4A, 0x86);

/// Paints `icon` inside `rect` (square works best).
pub fn paint(painter: &Painter, rect: Rect, icon: Icon) {
    let side = rect.width().min(rect.height());
    let r = Rect::from_center_size(rect.center(), vec2(side, side));
    let k = Pen {
        p: painter,
        r,
        ow: (side * 0.065).clamp(1.2, 4.0),
        seed: icon as u32 * 13,
    };
    use Icon::*;
    match icon {
        Book => {
            k.poly(
                &[(0.08, 0.22), (0.5, 0.3), (0.5, 0.88), (0.08, 0.78)],
                PARCHMENT,
            );
            k.poly(
                &[(0.5, 0.3), (0.92, 0.22), (0.92, 0.78), (0.5, 0.88)],
                PARCHMENT_DARK,
            );
            k.poly(
                &[
                    (0.04, 0.8),
                    (0.5, 0.92),
                    (0.96, 0.8),
                    (0.96, 0.86),
                    (0.5, 0.97),
                    (0.04, 0.86),
                ],
                BURGUNDY,
            );
            for i in 0..4 {
                let y = 0.4 + i as f32 * 0.1;
                k.line(&[(0.15, y - 0.02), (0.42, y + 0.02)], 0.025, INK_SOFT);
                k.line(&[(0.58, y + 0.02), (0.85, y - 0.02)], 0.025, INK_SOFT);
            }
            k.line(&[(0.66, 0.2), (0.66, 0.5)], 0.06, BURGUNDY);
        }
        D20 => {
            let hex = [
                (0.5, 0.04),
                (0.93, 0.28),
                (0.93, 0.72),
                (0.5, 0.96),
                (0.07, 0.72),
                (0.07, 0.28),
            ];
            k.poly(&hex, BURGUNDY);
            k.fill(&[(0.5, 0.2), (0.8, 0.66), (0.2, 0.66)], RED);
            k.ink(&[(0.5, 0.2), (0.8, 0.66), (0.2, 0.66), (0.5, 0.2)]);
            for (a, b) in [
                ((0.5, 0.04), (0.5, 0.2)),
                ((0.93, 0.28), (0.8, 0.66)),
                ((0.07, 0.28), (0.2, 0.66)),
                ((0.5, 0.96), (0.8, 0.66)),
                ((0.5, 0.96), (0.2, 0.66)),
                ((0.07, 0.28), (0.5, 0.2)),
                ((0.93, 0.28), (0.5, 0.2)),
            ] {
                k.ink(&[a, b]);
            }
            k.p.text(
                k.pt(0.5, 0.5),
                egui::Align2::CENTER_CENTER,
                "20",
                egui::FontId::new(k.s(0.2), bold_family()),
                IVORY,
            );
        }
        Quill => {
            k.poly(
                &[(0.82, 0.06), (0.92, 0.14), (0.4, 0.66), (0.3, 0.6)],
                IVORY,
            );
            k.poly(&[(0.82, 0.06), (0.6, 0.18), (0.3, 0.6)], BONE);
            k.line(&[(0.86, 0.1), (0.24, 0.76)], 0.035, INK_SOFT);
            k.poly(
                &[(0.28, 0.66), (0.36, 0.72), (0.14, 0.92), (0.18, 0.78)],
                INK,
            );
            k.poly(
                &[(0.06, 0.88), (0.5, 0.82), (0.52, 0.94), (0.08, 0.98)],
                PARCHMENT,
            );
        }
        Banner => {
            k.line(&[(0.08, 0.1), (0.92, 0.1)], 0.07, WOODC);
            k.fill(
                &[(0.18, 0.12), (0.82, 0.12), (0.82, 0.88), (0.5, 0.72)],
                BURGUNDY,
            );
            k.fill(&[(0.18, 0.12), (0.5, 0.72), (0.18, 0.88)], BURGUNDY);
            k.outline(&[
                (0.18, 0.12),
                (0.82, 0.12),
                (0.82, 0.88),
                (0.5, 0.72),
                (0.18, 0.88),
            ]);
            k.poly(&[(0.5, 0.26), (0.62, 0.4), (0.5, 0.54), (0.38, 0.4)], GOLD);
            k.line(
                &[(0.24, 0.18), (0.24, 0.76)],
                0.03,
                GOLD.gamma_multiply(0.7),
            );
        }
        Chest => {
            k.poly(&[(0.1, 0.42), (0.9, 0.42), (0.9, 0.9), (0.1, 0.9)], WOODC);
            k.poly(
                &[
                    (0.1, 0.42),
                    (0.14, 0.22),
                    (0.5, 0.14),
                    (0.86, 0.22),
                    (0.9, 0.42),
                ],
                WOOD_LIGHT,
            );
            k.line(&[(0.3, 0.17), (0.3, 0.9)], 0.07, IRON);
            k.line(&[(0.7, 0.17), (0.7, 0.9)], 0.07, IRON);
            k.poly(
                &[(0.42, 0.4), (0.58, 0.4), (0.58, 0.58), (0.42, 0.58)],
                GOLD,
            );
            k.dot(0.5, 0.5, 0.03, INK);
            k.hi(&[(0.18, 0.3), (0.45, 0.22)]);
        }
        Helm => {
            k.fill(
                &[
                    (0.2, 0.5),
                    (0.24, 0.22),
                    (0.5, 0.08),
                    (0.76, 0.22),
                    (0.8, 0.5),
                ],
                STEEL,
            );
            k.fill(&[(0.2, 0.48), (0.8, 0.48), (0.78, 0.9), (0.22, 0.9)], STEEL);
            k.outline(&[
                (0.2, 0.5),
                (0.24, 0.22),
                (0.5, 0.08),
                (0.76, 0.22),
                (0.8, 0.5),
                (0.78, 0.9),
                (0.22, 0.9),
            ]);
            k.poly(
                &[(0.28, 0.46), (0.72, 0.46), (0.72, 0.54), (0.28, 0.54)],
                INK,
            );
            k.line(&[(0.5, 0.56), (0.5, 0.86)], 0.06, INK);
            k.line(&[(0.5, 0.08), (0.5, 0.42)], 0.05, STEEL_D);
            k.hi(&[(0.3, 0.22), (0.42, 0.14)]);
            k.poly(
                &[(0.5, 0.08), (0.6, 0.0), (0.66, 0.06), (0.52, 0.14)],
                BURGUNDY,
            );
        }
        Gear => {
            let c = (0.5, 0.5);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (s, co) = a.sin_cos();
                let (px, py) = (-s, co);
                let pts = [
                    (c.0 + co * 0.3 + px * 0.08, c.1 + s * 0.3 + py * 0.08),
                    (c.0 + co * 0.46 + px * 0.06, c.1 + s * 0.46 + py * 0.06),
                    (c.0 + co * 0.46 - px * 0.06, c.1 + s * 0.46 - py * 0.06),
                    (c.0 + co * 0.3 - px * 0.08, c.1 + s * 0.3 - py * 0.08),
                ];
                k.poly(&pts, BRONZE);
            }
            k.circle(0.5, 0.5, 0.34, BRONZE);
            k.circle(0.5, 0.5, 0.13, PARCHMENT_DARK);
            k.arc(
                0.5,
                0.5,
                0.25,
                3.6,
                4.6,
                0.05,
                Color32::from_white_alpha(110),
            );
        }
        Door => {
            k.fill(
                &[(0.18, 0.95), (0.18, 0.3), (0.82, 0.3), (0.82, 0.95)],
                WOODC,
            );
            k.fill(
                &[
                    (0.18, 0.32),
                    (0.3, 0.12),
                    (0.5, 0.05),
                    (0.7, 0.12),
                    (0.82, 0.32),
                ],
                WOODC,
            );
            k.outline(&[
                (0.18, 0.95),
                (0.18, 0.32),
                (0.3, 0.12),
                (0.5, 0.05),
                (0.7, 0.12),
                (0.82, 0.32),
                (0.82, 0.95),
            ]);
            for x in [0.34, 0.5, 0.66] {
                k.line(&[(x, 0.12), (x, 0.93)], 0.025, WOOD);
            }
            k.line(&[(0.2, 0.4), (0.8, 0.4)], 0.06, IRON);
            k.line(&[(0.2, 0.8), (0.8, 0.8)], 0.06, IRON);
            k.circle(0.7, 0.6, 0.05, GOLD);
        }
        Sword => {
            k.poly(
                &[
                    (0.5, 0.02),
                    (0.58, 0.14),
                    (0.57, 0.64),
                    (0.43, 0.64),
                    (0.42, 0.14),
                ],
                STEEL,
            );
            k.line(&[(0.5, 0.08), (0.5, 0.6)], 0.025, STEEL_D);
            k.poly(&[(0.2, 0.64), (0.8, 0.64), (0.8, 0.72), (0.2, 0.72)], GOLD);
            k.poly(
                &[(0.44, 0.72), (0.56, 0.72), (0.56, 0.9), (0.44, 0.9)],
                WOODC,
            );
            k.circle(0.5, 0.94, 0.06, GOLD);
            k.hi(&[(0.46, 0.16), (0.46, 0.56)]);
        }
        Shield => {
            k.fill(
                &[(0.12, 0.12), (0.88, 0.12), (0.88, 0.5), (0.12, 0.5)],
                SLATE,
            );
            k.fill(&[(0.12, 0.48), (0.88, 0.48), (0.5, 0.96)], SLATE);
            k.outline(&[
                (0.12, 0.12),
                (0.5, 0.06),
                (0.88, 0.12),
                (0.88, 0.5),
                (0.5, 0.96),
                (0.12, 0.5),
            ]);
            k.line(&[(0.5, 0.1), (0.5, 0.9)], 0.07, GOLD);
            k.line(&[(0.16, 0.36), (0.84, 0.36)], 0.07, GOLD);
            k.hi(&[(0.2, 0.18), (0.4, 0.14)]);
        }
        Potion => {
            k.circle(0.5, 0.64, 0.3, RED);
            k.fill(
                &[(0.24, 0.6), (0.76, 0.6), (0.72, 0.8), (0.28, 0.8)],
                RED.lerp_to_gamma(Color32::BLACK, 0.2),
            );
            k.poly(
                &[(0.4, 0.14), (0.6, 0.14), (0.6, 0.38), (0.4, 0.38)],
                ICE.gamma_multiply(0.9),
            );
            k.poly(
                &[(0.36, 0.06), (0.64, 0.06), (0.64, 0.14), (0.36, 0.14)],
                WOODC,
            );
            k.ring(0.5, 0.64, 0.3, 0.0, OUT);
            k.dot(0.38, 0.56, 0.06, Color32::from_white_alpha(170));
        }
        Skull => {
            k.fill(
                &[
                    (0.18, 0.5),
                    (0.2, 0.22),
                    (0.5, 0.08),
                    (0.8, 0.22),
                    (0.82, 0.5),
                    (0.7, 0.68),
                    (0.3, 0.68),
                ],
                BONE,
            );
            k.fill(
                &[(0.32, 0.64), (0.68, 0.64), (0.66, 0.9), (0.34, 0.9)],
                BONE,
            );
            k.outline(&[
                (0.18, 0.5),
                (0.2, 0.22),
                (0.5, 0.08),
                (0.8, 0.22),
                (0.82, 0.5),
                (0.68, 0.66),
                (0.66, 0.9),
                (0.34, 0.9),
                (0.32, 0.66),
            ]);
            k.poly(
                &[(0.26, 0.42), (0.44, 0.4), (0.42, 0.56), (0.28, 0.54)],
                INK,
            );
            k.poly(
                &[(0.56, 0.4), (0.74, 0.42), (0.72, 0.54), (0.58, 0.56)],
                INK,
            );
            k.poly(&[(0.5, 0.58), (0.55, 0.68), (0.45, 0.68)], INK);
            for x in [0.41, 0.5, 0.59] {
                k.line(&[(x, 0.76), (x, 0.88)], 0.025, INK);
            }
        }
        Torch => {
            k.poly(
                &[(0.44, 0.48), (0.56, 0.48), (0.54, 0.96), (0.46, 0.96)],
                WOODC,
            );
            k.poly(
                &[(0.36, 0.44), (0.64, 0.44), (0.6, 0.54), (0.4, 0.54)],
                IRON,
            );
            flame(&k, 0.5, 0.42, 0.34);
        }
        Heart => {
            k.fill(
                &[
                    (0.5, 0.3),
                    (0.88, 0.32),
                    (0.86, 0.5),
                    (0.5, 0.9),
                    (0.14, 0.5),
                    (0.12, 0.32),
                ],
                RED,
            );
            k.dot(0.31, 0.32, 0.2, RED);
            k.dot(0.69, 0.32, 0.2, RED);
            k.arc(0.31, 0.32, 0.2, 2.6, 5.5, 0.065, OUT);
            k.arc(0.69, 0.32, 0.2, 3.9, 6.9, 0.065, OUT);
            k.ink(&[(0.12, 0.38), (0.5, 0.9), (0.88, 0.38)]);
            k.dot(0.27, 0.27, 0.06, Color32::from_white_alpha(150));
        }
        Star => {
            let mut v = vec![];
            for i in 0..10 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let rr = if i % 2 == 0 { 0.46 } else { 0.2 };
                v.push((0.5 + rr * a.cos(), 0.52 + rr * a.sin()));
            }
            for i in 0..5 {
                let j = i * 2;
                k.fill(&[(0.5, 0.52), v[j], v[(j + 1) % 10]], GOLD_BRIGHT);
                k.fill(&[(0.5, 0.52), v[(j + 9) % 10], v[j]], GOLD);
            }
            k.outline(&v);
        }
        Flame => flame(&k, 0.5, 0.55, 0.46),
        Snowflake => {
            for i in 0..3 {
                let a = i as f32 * std::f32::consts::PI / 3.0;
                let (s, c) = a.sin_cos();
                k.line(
                    &[
                        (0.5 - c * 0.42, 0.5 - s * 0.42),
                        (0.5 + c * 0.42, 0.5 + s * 0.42),
                    ],
                    0.11,
                    OUT,
                );
                k.line(
                    &[
                        (0.5 - c * 0.4, 0.5 - s * 0.4),
                        (0.5 + c * 0.4, 0.5 + s * 0.4),
                    ],
                    0.06,
                    ICE,
                );
            }
            k.circle(0.5, 0.5, 0.08, IVORY);
        }
        Bolt => {
            k.fill(
                &[(0.58, 0.02), (0.22, 0.56), (0.46, 0.56), (0.54, 0.4)],
                FIRE_Y,
            );
            k.fill(
                &[(0.46, 0.56), (0.36, 0.98), (0.8, 0.4), (0.54, 0.4)],
                FIRE_Y,
            );
            k.outline(&[
                (0.58, 0.02),
                (0.22, 0.56),
                (0.46, 0.56),
                (0.36, 0.98),
                (0.8, 0.4),
                (0.54, 0.4),
            ]);
        }
        Leaf => {
            k.fill(
                &[
                    (0.12, 0.88),
                    (0.2, 0.4),
                    (0.52, 0.12),
                    (0.9, 0.08),
                    (0.84, 0.48),
                    (0.56, 0.8),
                ],
                GREEN,
            );
            k.outline(&[
                (0.12, 0.88),
                (0.2, 0.4),
                (0.52, 0.12),
                (0.9, 0.08),
                (0.84, 0.48),
                (0.56, 0.8),
            ]);
            k.line(&[(0.12, 0.88), (0.76, 0.22)], 0.04, FOREST);
            for (a, b) in [
                ((0.35, 0.64), (0.3, 0.42)),
                ((0.5, 0.49), (0.46, 0.28)),
                ((0.4, 0.6), (0.62, 0.62)),
                ((0.56, 0.42), (0.76, 0.44)),
            ] {
                k.line(&[a, b], 0.025, FOREST);
            }
        }
        Eye => {
            k.fill(
                &[
                    (0.04, 0.5),
                    (0.3, 0.26),
                    (0.7, 0.26),
                    (0.96, 0.5),
                    (0.7, 0.74),
                    (0.3, 0.74),
                ],
                IVORY,
            );
            k.outline(&[
                (0.04, 0.5),
                (0.3, 0.26),
                (0.7, 0.26),
                (0.96, 0.5),
                (0.7, 0.74),
                (0.3, 0.74),
            ]);
            k.circle(0.5, 0.5, 0.19, PURPLE);
            k.circle(0.5, 0.5, 0.08, INK);
            k.dot(0.45, 0.44, 0.04, Color32::from_white_alpha(200));
        }
        Crown => {
            k.fill(
                &[
                    (0.1, 0.78),
                    (0.1, 0.3),
                    (0.3, 0.52),
                    (0.5, 0.2),
                    (0.7, 0.52),
                    (0.9, 0.3),
                    (0.9, 0.78),
                ],
                GOLD,
            );
            k.fill(&[(0.1, 0.5), (0.9, 0.5), (0.9, 0.78), (0.1, 0.78)], GOLD);
            k.outline(&[
                (0.1, 0.8),
                (0.1, 0.3),
                (0.3, 0.52),
                (0.5, 0.2),
                (0.7, 0.52),
                (0.9, 0.3),
                (0.9, 0.8),
            ]);
            k.line(&[(0.12, 0.68), (0.88, 0.68)], 0.04, BRONZE);
            for (x, c) in [(0.3, RED), (0.5, MANA), (0.7, RED)] {
                k.circle(x, 0.68, 0.05, c);
            }
            for (x, y) in [(0.1, 0.27), (0.5, 0.17), (0.9, 0.27)] {
                k.circle(x, y, 0.05, GOLD_BRIGHT);
            }
        }
        Scroll => {
            k.poly(
                &[(0.18, 0.16), (0.82, 0.16), (0.82, 0.84), (0.18, 0.84)],
                PARCHMENT,
            );
            k.poly(
                &[(0.1, 0.1), (0.9, 0.1), (0.9, 0.22), (0.1, 0.22)],
                PARCHMENT_DARK,
            );
            k.poly(
                &[(0.1, 0.78), (0.9, 0.78), (0.9, 0.9), (0.1, 0.9)],
                PARCHMENT_DARK,
            );
            for i in 0..4 {
                let y = 0.32 + i as f32 * 0.11;
                k.line(
                    &[(0.26, y), (0.74 - (i % 2) as f32 * 0.12, y)],
                    0.03,
                    INK_SOFT,
                );
            }
            k.circle(0.7, 0.72, 0.08, BURGUNDY);
        }
        Map => {
            k.poly(
                &[(0.06, 0.2), (0.36, 0.12), (0.36, 0.84), (0.06, 0.92)],
                PARCHMENT,
            );
            k.poly(
                &[(0.36, 0.12), (0.64, 0.2), (0.64, 0.92), (0.36, 0.84)],
                PARCHMENT_DARK,
            );
            k.poly(
                &[(0.64, 0.2), (0.94, 0.12), (0.94, 0.84), (0.64, 0.92)],
                PARCHMENT,
            );
            k.line(
                &[
                    (0.14, 0.7),
                    (0.3, 0.5),
                    (0.48, 0.6),
                    (0.62, 0.38),
                    (0.8, 0.44),
                ],
                0.03,
                BURGUNDY,
            );
            k.ink(&[(0.78, 0.36), (0.86, 0.48)]);
            k.ink(&[(0.86, 0.36), (0.78, 0.48)]);
        }
        Bag => {
            k.fill(
                &[
                    (0.2, 0.42),
                    (0.36, 0.3),
                    (0.64, 0.3),
                    (0.8, 0.42),
                    (0.86, 0.72),
                    (0.72, 0.92),
                    (0.28, 0.92),
                    (0.14, 0.72),
                ],
                WOODC,
            );
            k.outline(&[
                (0.2, 0.42),
                (0.36, 0.3),
                (0.64, 0.3),
                (0.8, 0.42),
                (0.86, 0.72),
                (0.72, 0.92),
                (0.28, 0.92),
                (0.14, 0.72),
            ]);
            k.poly(
                &[(0.38, 0.14), (0.62, 0.14), (0.66, 0.3), (0.34, 0.3)],
                WOODC,
            );
            k.line(&[(0.32, 0.31), (0.68, 0.31)], 0.06, GOLD);
            k.poly(
                &[(0.42, 0.52), (0.58, 0.52), (0.58, 0.66), (0.42, 0.66)],
                BRONZE,
            );
        }
        Campfire => {
            k.poly(
                &[(0.1, 0.84), (0.86, 0.66), (0.9, 0.76), (0.14, 0.94)],
                WOODC,
            );
            k.poly(
                &[(0.14, 0.66), (0.9, 0.84), (0.86, 0.94), (0.1, 0.76)],
                WOOD_LIGHT,
            );
            flame(&k, 0.5, 0.66, 0.42);
        }
        Hourglass => {
            k.poly(
                &[(0.16, 0.04), (0.84, 0.04), (0.84, 0.12), (0.16, 0.12)],
                WOODC,
            );
            k.poly(
                &[(0.16, 0.88), (0.84, 0.88), (0.84, 0.96), (0.16, 0.96)],
                WOODC,
            );
            k.poly(
                &[(0.24, 0.12), (0.76, 0.12), (0.52, 0.5), (0.48, 0.5)],
                ICE.gamma_multiply(0.7),
            );
            k.poly(
                &[(0.48, 0.5), (0.52, 0.5), (0.76, 0.88), (0.24, 0.88)],
                ICE.gamma_multiply(0.7),
            );
            k.fill(&[(0.34, 0.88), (0.5, 0.66), (0.66, 0.88)], GOLD);
            k.fill(&[(0.36, 0.28), (0.64, 0.28), (0.5, 0.48)], GOLD);
        }
        Pointer => {
            k.fill(&[(0.22, 0.06), (0.8, 0.6), (0.22, 0.84)], IVORY);
            k.fill(
                &[(0.42, 0.6), (0.54, 0.6), (0.68, 0.94), (0.56, 0.98)],
                IVORY,
            );
            k.outline(&[
                (0.22, 0.06),
                (0.8, 0.6),
                (0.52, 0.62),
                (0.68, 0.94),
                (0.56, 0.98),
                (0.42, 0.66),
                (0.22, 0.84),
            ]);
        }
        Brush => {
            k.poly(&[(0.7, 0.06), (0.94, 0.3), (0.5, 0.74), (0.26, 0.5)], WOODC);
            k.poly(&[(0.26, 0.5), (0.5, 0.74), (0.44, 0.8), (0.2, 0.56)], STEEL);
            k.poly(
                &[(0.2, 0.56), (0.44, 0.8), (0.12, 0.94), (0.06, 0.88)],
                FOREST,
            );
        }
        Eraser => {
            k.poly(
                &[(0.5, 0.12), (0.92, 0.5), (0.6, 0.82), (0.18, 0.44)],
                RED.lerp_to_gamma(IVORY, 0.4),
            );
            k.poly(
                &[(0.18, 0.44), (0.6, 0.82), (0.48, 0.94), (0.06, 0.56)],
                IVORY,
            );
            k.line(&[(0.1, 0.96), (0.9, 0.96)], 0.03, INK_SOFT);
        }
        Square => {
            k.poly(
                &[(0.12, 0.12), (0.88, 0.12), (0.88, 0.88), (0.12, 0.88)],
                PARCHMENT_DARK,
            );
            for (x, y) in [(0.12, 0.12), (0.88, 0.12), (0.88, 0.88), (0.12, 0.88)] {
                k.circle(x, y, 0.07, GOLD);
            }
        }
        Bucket => {
            k.poly(
                &[(0.16, 0.34), (0.7, 0.34), (0.64, 0.9), (0.22, 0.9)],
                STEEL,
            );
            k.arc(0.43, 0.34, 0.27, 3.14, 6.28, 0.05, IRON);
            k.poly(&[(0.7, 0.36), (0.9, 0.6), (0.86, 0.86), (0.74, 0.62)], MANA);
            k.line(&[(0.2, 0.5), (0.68, 0.5)], 0.04, STEEL_D);
        }
        Tree => {
            k.poly(
                &[(0.44, 0.62), (0.56, 0.62), (0.58, 0.96), (0.42, 0.96)],
                WOODC,
            );
            k.circle(0.32, 0.5, 0.2, FOREST);
            k.circle(0.68, 0.5, 0.2, FOREST);
            k.circle(0.5, 0.3, 0.26, GREEN);
            k.dot(0.42, 0.22, 0.07, Color32::from_white_alpha(70));
        }
        Flag => {
            k.line(&[(0.22, 0.06), (0.22, 0.96)], 0.07, WOODC);
            k.poly(&[(0.24, 0.1), (0.86, 0.22), (0.24, 0.5)], FOREST);
            k.circle(0.22, 0.06, 0.05, GOLD);
        }
        Trash => {
            k.poly(
                &[(0.2, 0.26), (0.8, 0.26), (0.72, 0.94), (0.28, 0.94)],
                STEEL_D,
            );
            k.poly(
                &[(0.12, 0.16), (0.88, 0.16), (0.88, 0.26), (0.12, 0.26)],
                STEEL,
            );
            k.poly(&[(0.4, 0.06), (0.6, 0.06), (0.6, 0.16), (0.4, 0.16)], STEEL);
            for x in [0.38, 0.5, 0.62] {
                k.line(&[(x, 0.36), (x, 0.84)], 0.035, INK);
            }
        }
        Undo | Redo => {
            let flip = |x: f32| if icon == Redo { 1.0 - x } else { x };
            k.arc(
                flip(0.5),
                0.56,
                0.3,
                if icon == Redo { 3.6 } else { -0.5 },
                if icon == Redo { 6.9 } else { 2.6 },
                0.12,
                OUT,
            );
            k.arc(
                flip(0.5),
                0.56,
                0.3,
                if icon == Redo { 3.6 } else { -0.5 },
                if icon == Redo { 6.9 } else { 2.6 },
                0.06,
                GOLD,
            );
            k.poly(
                &[(flip(0.04), 0.4), (flip(0.36), 0.2), (flip(0.32), 0.56)],
                GOLD,
            );
        }
        Seal => {
            k.circle(0.5, 0.52, 0.4, BURGUNDY);
            k.ring(
                0.5,
                0.52,
                0.28,
                0.03,
                RED.lerp_to_gamma(Color32::BLACK, 0.4),
            );
            k.poly(&[(0.5, 0.3), (0.64, 0.52), (0.5, 0.74), (0.36, 0.52)], RED);
            k.dot(0.36, 0.36, 0.06, Color32::from_white_alpha(90));
        }
        Play => k.poly(
            &[(0.22, 0.1), (0.88, 0.5), (0.22, 0.9)],
            FOREST.lerp_to_gamma(Color32::WHITE, 0.2),
        ),
        Link => {
            k.ring(0.36, 0.5, 0.2, 0.13, OUT);
            k.ring(0.64, 0.5, 0.2, 0.13, OUT);
            k.ring(0.36, 0.5, 0.2, 0.07, GOLD);
            k.ring(0.64, 0.5, 0.2, 0.07, BRONZE);
        }
        Lock => {
            k.arc(0.5, 0.42, 0.22, 3.14, 6.28, 0.14, OUT);
            k.arc(0.5, 0.42, 0.22, 3.14, 6.28, 0.07, STEEL);
            k.poly(
                &[(0.18, 0.42), (0.82, 0.42), (0.82, 0.92), (0.18, 0.92)],
                GOLD,
            );
            k.circle(0.5, 0.62, 0.06, INK);
            k.line(&[(0.5, 0.64), (0.5, 0.8)], 0.06, INK);
        }
        Search => {
            k.line(&[(0.58, 0.58), (0.92, 0.92)], 0.16, OUT);
            k.line(&[(0.6, 0.6), (0.9, 0.9)], 0.08, WOODC);
            k.circle(0.4, 0.4, 0.3, ICE.gamma_multiply(0.6));
            k.ring(0.4, 0.4, 0.3, 0.07, BRONZE);
            k.arc(
                0.4,
                0.4,
                0.2,
                3.5,
                4.6,
                0.05,
                Color32::from_white_alpha(150),
            );
        }
        Plus => {
            k.poly(&[(0.4, 0.1), (0.6, 0.1), (0.6, 0.9), (0.4, 0.9)], GOLD);
            k.poly(&[(0.1, 0.4), (0.9, 0.4), (0.9, 0.6), (0.1, 0.6)], GOLD);
        }
        Grid => {
            k.poly(
                &[(0.1, 0.1), (0.9, 0.1), (0.9, 0.9), (0.1, 0.9)],
                PARCHMENT_DARK,
            );
            for t in [0.37, 0.63] {
                k.line(&[(t, 0.1), (t, 0.9)], 0.04, INK);
                k.line(&[(0.1, t), (0.9, t)], 0.04, INK);
            }
        }
        Layers => {
            for (i, c) in [(2, SLATE), (1, FOREST), (0, GOLD)] {
                let y = 0.2 + i as f32 * 0.2;
                k.poly(
                    &[
                        (0.5, y),
                        (0.92, y + 0.18),
                        (0.5, y + 0.36),
                        (0.08, y + 0.18),
                    ],
                    c,
                );
            }
        }
        Fit => {
            for (x, y, dx, dy) in [
                (0.1, 0.1, 1.0, 1.0),
                (0.9, 0.1, -1.0, 1.0),
                (0.9, 0.9, -1.0, -1.0),
                (0.1, 0.9, 1.0, -1.0),
            ] {
                k.poly(&[(x, y), (x + dx * 0.32, y), (x, y + dy * 0.32)], GOLD);
            }
            k.poly(
                &[(0.34, 0.34), (0.66, 0.34), (0.66, 0.66), (0.34, 0.66)],
                PARCHMENT_DARK,
            );
        }
        Target => {
            k.circle(0.5, 0.5, 0.42, IVORY);
            k.circle(0.5, 0.5, 0.28, BURGUNDY);
            k.circle(0.5, 0.5, 0.12, GOLD);
        }
        Axe => {
            k.poly(
                &[(0.46, 0.06), (0.56, 0.06), (0.6, 0.96), (0.5, 0.96)],
                WOODC,
            );
            k.fill(
                &[
                    (0.52, 0.12),
                    (0.84, 0.04),
                    (0.94, 0.3),
                    (0.84, 0.52),
                    (0.52, 0.42),
                ],
                STEEL,
            );
            k.outline(&[
                (0.52, 0.12),
                (0.84, 0.04),
                (0.94, 0.3),
                (0.84, 0.52),
                (0.52, 0.42),
            ]);
            k.poly(
                &[(0.48, 0.16), (0.26, 0.2), (0.22, 0.34), (0.48, 0.36)],
                STEEL_D,
            );
            k.hi(&[(0.86, 0.12), (0.9, 0.36)]);
        }
        Lute => {
            k.circle(0.4, 0.62, 0.3, WOODC);
            k.circle(0.4, 0.62, 0.08, INK);
            k.poly(
                &[(0.56, 0.4), (0.86, 0.1), (0.92, 0.16), (0.62, 0.46)],
                WOOD_LIGHT,
            );
            k.poly(&[(0.82, 0.06), (0.96, 0.0), (0.98, 0.12), (0.9, 0.2)], WOOD);
            k.line(&[(0.2, 0.8), (0.88, 0.14)], 0.015, IVORY);
            k.line(&[(0.24, 0.84), (0.9, 0.18)], 0.015, IVORY);
        }
        Sun => {
            for i in 0..12 {
                let a = i as f32 * std::f32::consts::TAU / 12.0;
                let (s, c) = a.sin_cos();
                let n = (-s, c);
                k.poly(
                    &[
                        (0.5 + c * 0.24 + n.0 * 0.07, 0.5 + s * 0.24 + n.1 * 0.07),
                        (0.5 + c * 0.47, 0.5 + s * 0.47),
                        (0.5 + c * 0.24 - n.0 * 0.07, 0.5 + s * 0.24 - n.1 * 0.07),
                    ],
                    if i % 2 == 0 { GOLD_BRIGHT } else { GOLD },
                );
            }
            k.circle(0.5, 0.5, 0.26, GOLD_BRIGHT);
            k.dot(0.42, 0.42, 0.07, Color32::from_white_alpha(120));
        }
        Fist => {
            k.poly(
                &[
                    (0.2, 0.36),
                    (0.8, 0.36),
                    (0.82, 0.7),
                    (0.64, 0.84),
                    (0.3, 0.84),
                    (0.18, 0.66),
                ],
                BONE.lerp_to_gamma(WOODC, 0.35),
            );
            for i in 0..4 {
                let x = 0.22 + i as f32 * 0.15;
                k.circle(x + 0.07, 0.36, 0.08, BONE.lerp_to_gamma(WOODC, 0.25));
            }
            k.poly(
                &[(0.3, 0.84), (0.64, 0.84), (0.62, 0.98), (0.32, 0.98)],
                BURGUNDY,
            );
            k.ink(&[(0.2, 0.56), (0.5, 0.52)]);
        }
        Bow => {
            k.arc(0.86, 0.5, 0.5, 2.1, 4.18, 0.12, OUT);
            k.arc(0.86, 0.5, 0.5, 2.1, 4.18, 0.06, WOODC);
            k.line(&[(0.62, 0.07), (0.62, 0.93)], 0.02, IVORY);
            k.line(&[(0.08, 0.5), (0.9, 0.5)], 0.035, WOODC);
            k.poly(&[(0.92, 0.5), (0.8, 0.43), (0.8, 0.57)], STEEL);
            k.poly(&[(0.08, 0.5), (0.18, 0.42), (0.2, 0.5)], RED);
            k.poly(&[(0.08, 0.5), (0.2, 0.5), (0.18, 0.58)], RED);
        }
        Dagger => {
            k.poly(
                &[(0.84, 0.06), (0.92, 0.14), (0.42, 0.62), (0.36, 0.56)],
                STEEL,
            );
            k.poly(&[(0.26, 0.5), (0.5, 0.74), (0.44, 0.8), (0.2, 0.56)], GOLD);
            k.poly(
                &[(0.32, 0.66), (0.36, 0.7), (0.16, 0.9), (0.1, 0.86)],
                WOOD_LIGHT,
            );
            k.circle(0.1, 0.92, 0.05, GOLD);
        }
        Hat => {
            k.poly(
                &[(0.04, 0.82), (0.96, 0.82), (0.86, 0.94), (0.14, 0.94)],
                SLATE,
            );
            k.poly(
                &[(0.28, 0.82), (0.72, 0.82), (0.56, 0.28), (0.44, 0.24)],
                SLATE,
            );
            k.poly(
                &[(0.44, 0.24), (0.56, 0.28), (0.82, 0.08), (0.7, 0.04)],
                SLATE,
            );
            k.poly(&[(0.3, 0.72), (0.7, 0.72), (0.72, 0.8), (0.28, 0.8)], GOLD);
            for (x, y) in [(0.46, 0.52), (0.56, 0.42)] {
                k.dot(x, y, 0.03, GOLD_BRIGHT);
            }
        }
        Hammer => {
            k.poly(&[(0.46, 0.3), (0.56, 0.3), (0.6, 0.96), (0.5, 0.96)], WOODC);
            k.poly(
                &[(0.16, 0.08), (0.86, 0.08), (0.86, 0.36), (0.16, 0.36)],
                STEEL,
            );
            k.line(
                &[(0.2, 0.14), (0.82, 0.14)],
                0.03,
                Color32::from_white_alpha(110),
            );
        }
        Feather => {
            k.fill(
                &[
                    (0.86, 0.04),
                    (0.92, 0.26),
                    (0.6, 0.6),
                    (0.26, 0.78),
                    (0.38, 0.46),
                ],
                IVORY,
            );
            k.outline(&[
                (0.86, 0.04),
                (0.92, 0.26),
                (0.6, 0.6),
                (0.26, 0.78),
                (0.38, 0.46),
            ]);
            k.line(&[(0.86, 0.06), (0.14, 0.94)], 0.035, INK_SOFT);
            for i in 0..4 {
                let t = 0.25 + i as f32 * 0.14;
                let (x, y) = (0.86 - t * 0.72, 0.06 + t * 0.88);
                k.line(&[(x, y), (x + 0.08, y + 0.08)], 0.02, INK_SOFT);
            }
        }
    }
}

fn flame(k: &Pen, cx: f32, base: f32, h: f32) {
    let o = [
        (cx - h * 0.45, base),
        (cx - h * 0.5, base - h * 0.45),
        (cx - h * 0.1, base - h * 0.75),
        (cx, base - h),
        (cx + h * 0.2, base - h * 0.6),
        (cx + h * 0.5, base - h * 0.5),
        (cx + h * 0.42, base),
    ];
    k.fill(&[o[0], o[1], o[2], o[4], o[5], o[6]], FIRE);
    k.fill(&[o[2], o[3], o[4]], FIRE);
    k.outline(&o);
    k.fill(
        &[
            (cx - h * 0.22, base),
            (cx - h * 0.2, base - h * 0.35),
            (cx, base - h * 0.62),
            (cx + h * 0.2, base - h * 0.32),
            (cx + h * 0.2, base),
        ],
        FIRE_Y,
    );
}

/// Class emblem for character creation and sheets.
pub fn class_icon(class: dnd_rules::content::ClassId) -> Icon {
    use dnd_rules::content::ClassId::*;
    match class {
        Barbarian => Icon::Axe,
        Bard => Icon::Lute,
        Cleric => Icon::Sun,
        Druid => Icon::Leaf,
        Fighter => Icon::Sword,
        Monk => Icon::Fist,
        Paladin => Icon::Shield,
        Ranger => Icon::Bow,
        Rogue => Icon::Dagger,
        Sorcerer => Icon::Flame,
        Warlock => Icon::Eye,
        Wizard => Icon::Hat,
    }
}

/// Ability score emblem.
pub fn ability_icon(a: dnd_rules::Ability) -> Icon {
    use dnd_rules::Ability::*;
    match a {
        Strength => Icon::Hammer,
        Dexterity => Icon::Feather,
        Constitution => Icon::Heart,
        Intelligence => Icon::Book,
        Wisdom => Icon::Eye,
        Charisma => Icon::Crown,
    }
}

/// An icon inside an illustrated emblem frame (shield-shaped, iron rim, colored field).
pub fn emblem(painter: &Painter, rect: Rect, icon: Icon, field: Color32) {
    let k = Pen {
        p: painter,
        r: rect,
        ow: (rect.width() * 0.04).max(1.2),
        seed: 3,
    };
    let outline = [
        (0.08, 0.06),
        (0.92, 0.06),
        (0.92, 0.5),
        (0.5, 0.97),
        (0.08, 0.5),
    ];
    k.fill(
        &[(0.08, 0.06), (0.92, 0.06), (0.92, 0.5), (0.08, 0.5)],
        IRON,
    );
    k.fill(&[(0.08, 0.48), (0.92, 0.48), (0.5, 0.97)], IRON);
    k.outline(&outline);
    let f = [
        (0.16, 0.13),
        (0.84, 0.13),
        (0.84, 0.48),
        (0.5, 0.86),
        (0.16, 0.48),
    ];
    k.fill(&[f[0], f[1], f[2], f[4]], field);
    k.fill(&[f[4], f[2], f[3]], field);
    k.p.add(Shape::closed_line(k.pts(&f), Stroke::new(k.s(0.025), GOLD)));
    for (x, y) in [(0.14, 0.1), (0.86, 0.1)] {
        k.dot(x, y, 0.03, GOLD.gamma_multiply(0.8));
    }
    let inner = Rect::from_center_size(k.pt(0.5, 0.43), rect.size() * 0.5);
    paint(painter, inner, icon);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_paints_without_panicking() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx, 1.0);
        ctx.begin_pass(egui::RawInput::default());
        ctx.end_pass().textures_delta.clear(); // fonts become active on the next pass
        ctx.begin_pass(egui::RawInput::default());
        let painter = ctx.layer_painter(egui::LayerId::background());
        for (i, icon) in ALL.iter().enumerate() {
            let r = Rect::from_min_size(
                pos2((i % 10) as f32 * 40.0, (i / 10) as f32 * 40.0),
                vec2(36.0, 36.0),
            );
            paint(&painter, r, *icon);
            emblem(&painter, r, *icon, BURGUNDY);
        }
        ctx.end_pass().textures_delta.clear();
    }
}

/// Every icon (for tests and the icon gallery).
pub const ALL: &[Icon] = &[
    Icon::Book,
    Icon::D20,
    Icon::Quill,
    Icon::Banner,
    Icon::Chest,
    Icon::Helm,
    Icon::Gear,
    Icon::Door,
    Icon::Sword,
    Icon::Shield,
    Icon::Potion,
    Icon::Skull,
    Icon::Torch,
    Icon::Heart,
    Icon::Star,
    Icon::Flame,
    Icon::Snowflake,
    Icon::Bolt,
    Icon::Leaf,
    Icon::Eye,
    Icon::Crown,
    Icon::Scroll,
    Icon::Map,
    Icon::Bag,
    Icon::Campfire,
    Icon::Hourglass,
    Icon::Pointer,
    Icon::Brush,
    Icon::Eraser,
    Icon::Square,
    Icon::Bucket,
    Icon::Tree,
    Icon::Flag,
    Icon::Trash,
    Icon::Undo,
    Icon::Redo,
    Icon::Seal,
    Icon::Play,
    Icon::Link,
    Icon::Lock,
    Icon::Search,
    Icon::Plus,
    Icon::Grid,
    Icon::Layers,
    Icon::Fit,
    Icon::Target,
    Icon::Axe,
    Icon::Lute,
    Icon::Sun,
    Icon::Fist,
    Icon::Bow,
    Icon::Dagger,
    Icon::Hat,
    Icon::Hammer,
    Icon::Feather,
];
