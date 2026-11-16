//! Original procedural top-down art for the expanded object library (trees, plants, rocks,
//! furniture, dungeon architecture/atmosphere/gameplay objects). Every id gets its own drawing;
//! `tests::every_new_object_has_distinct_art` guards against generic look-alikes.

use super::{Canvas, INK, Rgba, alpha, c3, hash, mix, shade};
use std::f32::consts::{PI, TAU};

/// Object ids drawn here (the original objects are drawn in `terrain_art::object`).
#[cfg_attr(not(test), allow(dead_code))]
pub const IDS: &[&str] = &[
    "pine_tree",
    "birch_tree",
    "dead_tree",
    "ancient_tree",
    "small_tree",
    "fallen_tree",
    "stump",
    "flower_bush",
    "fern",
    "tall_grass",
    "mushrooms",
    "glowing_mushrooms",
    "vines",
    "roots",
    "wildflowers",
    "reeds",
    "small_stones",
    "large_rock",
    "rock_cluster",
    "mossy_rock",
    "fallen_branches",
    "log",
    "leaf_pile",
    "puddle",
    "small_table",
    "round_table",
    "tavern_table",
    "dining_table",
    "bench",
    "stool",
    "throne",
    "reinforced_chest",
    "cabinet",
    "wardrobe",
    "shelf",
    "sack",
    "bedroll",
    "carpet",
    "rug",
    "candle",
    "candelabra",
    "fireplace",
    "cooking_pot",
    "wall_banner",
    "painting",
    "mirror",
    "desk",
    "scroll",
    "books",
    "bottles",
    "plates",
    "lantern",
    "broken_pillar",
    "stone_arch",
    "iron_gate",
    "prison_bars",
    "heavy_door",
    "secret_door",
    "stone_stairs",
    "broken_wall",
    "rubble_pile",
    "skeleton",
    "skull_pile",
    "cobweb",
    "chains",
    "hanging_cage",
    "tombstone",
    "coffin",
    "sarcophagus",
    "bloodstain",
    "statue",
    "spike_trap",
    "pit_trap",
    "switch",
    "locked_chest",
    "magic_altar",
    "ritual_symbol",
    "magic_crystal",
    "portal",
    "puzzle_mechanism",
    "pressure_plate",
];

type Pt = (f32, f32);

fn ell_pts(cx: f32, cy: f32, rx: f32, ry: f32, ang: f32, n: usize) -> Vec<Pt> {
    let (sa, ca) = ang.sin_cos();
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32 * TAU;
            let (x, y) = (t.cos() * rx, t.sin() * ry);
            (cx + x * ca - y * sa, cy + x * sa + y * ca)
        })
        .collect()
}
fn ell(c: &mut Canvas, cx: f32, cy: f32, rx: f32, ry: f32, col: Rgba, ow: f32) {
    c.poly(&ell_pts(cx, cy, rx, ry, 0.0, 28), col, ow);
}
fn leaf(c: &mut Canvas, cx: f32, cy: f32, len: f32, ang: f32, col: Rgba, ow: f32) {
    c.poly(&ell_pts(cx, cy, len, len * 0.42, ang, 12), col, ow);
}
fn rect(c: &mut Canvas, x0: f32, y0: f32, x1: f32, y1: f32, col: Rgba, ow: f32) {
    c.poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], col, ow);
}
pub(super) fn star(cx: f32, cy: f32, ro: f32, ri: f32, n: usize, rot: f32) -> Vec<Pt> {
    (0..n * 2)
        .map(|i| {
            let a = rot + i as f32 / (n * 2) as f32 * TAU;
            let r = if i % 2 == 0 { ro } else { ri };
            (cx + a.cos() * r, cy + a.sin() * r)
        })
        .collect()
}
/// Wooden board with grain running along its long side.
fn wood(c: &mut Canvas, x0: f32, y0: f32, x1: f32, y1: f32, col: Rgba, ow: f32, s: f32) {
    rect(c, x0, y0, x1, y1, col, ow);
    let horiz = x1 - x0 >= y1 - y0;
    let (len, across) = if horiz {
        (x1 - x0, y1 - y0)
    } else {
        (y1 - y0, x1 - x0)
    };
    let n = (across / (9.0 * s)).floor().max(1.0) as usize;
    for i in 1..=n {
        let t = i as f32 / (n + 1) as f32;
        let (a, b) = if horiz {
            (
                (x0 + 2.0 * s, y0 + across * t),
                (x0 + len - 2.0 * s, y0 + across * t),
            )
        } else {
            (
                (x0 + across * t, y0 + 2.0 * s),
                (x0 + across * t, y0 + len - 2.0 * s),
            )
        };
        c.line(a, b, 1.2 * s, shade(col, 0.7));
    }
    c.line(
        (x0 + ow, y0 + ow),
        if horiz {
            (x1 - ow, y0 + ow)
        } else {
            (x0 + ow, y1 - ow)
        },
        1.4 * s,
        alpha(c3(255, 230, 190), 0.35),
    );
}
fn rock(c: &mut Canvas, cx: f32, cy: f32, r: f32, col: Rgba, seed: u32, ow: f32) {
    let n = 9;
    let pts: Vec<Pt> = (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * TAU + hash(i, seed, 3) * 0.3;
            let rr = r * (0.78 + hash(i, seed, 4) * 0.27);
            (cx + a.cos() * rr, cy + a.sin() * rr * 0.9)
        })
        .collect();
    c.poly(&pts, col, ow);
    let hi: Vec<Pt> = pts
        .iter()
        .map(|(x, y)| {
            (
                cx + (x - cx) * 0.55 - r * 0.15,
                cy + (y - cy) * 0.5 - r * 0.2,
            )
        })
        .collect();
    c.poly(&hi, alpha(shade(col, 1.25), 0.7), 0.0);
    c.line(
        (cx - r * 0.1, cy + r * 0.05),
        (cx + r * 0.35, cy + r * 0.35),
        ow * 0.5,
        alpha(INK, 0.6),
    );
}
#[allow(clippy::too_many_arguments)]
fn canopy(
    c: &mut Canvas,
    cx: f32,
    cy: f32,
    r0: f32,
    lobes: u32,
    seed: u32,
    cols: [Rgba; 3],
    ow: f32,
) {
    for pass in 0..3 {
        for i in 0..lobes {
            let a = i as f32 / lobes as f32 * TAU + hash(i, seed, 5) * 0.4;
            let d = r0 * 0.55;
            let (x, y) = (cx + a.cos() * d, cy + a.sin() * d);
            let rr = r0 * (0.46 + hash(i, seed + 1, 5) * 0.14);
            match pass {
                0 => c.disc(x, y, rr + ow, INK),
                1 => c.disc(x, y, rr, mix(cols[0], cols[1], hash(i, seed + 2, 5))),
                _ => c.disc(x - rr * 0.25, y - rr * 0.3, rr * 0.5, alpha(cols[2], 0.5)),
            }
        }
        match pass {
            0 => c.disc(cx, cy, r0 * 0.6 + ow, INK),
            1 => c.disc(cx, cy, r0 * 0.6, cols[1]),
            _ => c.disc(
                cx - r0 * 0.15,
                cy - r0 * 0.2,
                r0 * 0.33,
                alpha(cols[2], 0.6),
            ),
        }
    }
}
fn flame(c: &mut Canvas, x: f32, y: f32, r: f32) {
    c.blob(x, y, r * 3.2, r * 3.2, alpha(c3(255, 170, 70), 0.45));
    c.poly(
        &[(x, y - r * 1.6), (x + r, y), (x, y + r * 0.8), (x - r, y)],
        c3(240, 120, 40),
        0.0,
    );
    c.disc(x, y, r * 0.65, c3(255, 210, 110));
    c.disc(x, y - r * 0.1, r * 0.3, c3(255, 250, 225));
}
fn skull(c: &mut Canvas, x: f32, y: f32, r: f32, ow: f32) {
    let bone = c3(226, 218, 196);
    rect(
        c,
        x - r * 0.45,
        y + r * 0.4,
        x + r * 0.45,
        y + r * 0.95,
        shade(bone, 0.9),
        ow * 0.7,
    );
    c.disc(x, y, r + ow * 0.7, INK);
    c.disc(x, y, r, bone);
    c.disc(x - r * 0.38, y + r * 0.1, r * 0.26, INK);
    c.disc(x + r * 0.38, y + r * 0.1, r * 0.26, INK);
    c.disc(
        x - r * 0.2,
        y - r * 0.35,
        r * 0.3,
        alpha(c3(255, 252, 240), 0.5),
    );
}
fn bars(c: &mut Canvas, x0: f32, x1: f32, y: f32, n: usize, r: f32, col: Rgba, ow: f32) {
    for i in 0..n {
        let x = x0 + (x1 - x0) * (i as f32 + 0.5) / n as f32;
        c.disc(x, y, r + ow * 0.6, INK);
        c.disc(x, y, r, col);
        c.disc(
            x - r * 0.3,
            y - r * 0.3,
            r * 0.35,
            alpha(c3(220, 220, 230), 0.6),
        );
    }
}
fn chest(c: &mut Canvas, fw: f32, fh: f32, wood_col: Rgba, band: Rgba, ow: f32, s: f32) {
    let (x0, y0, x1, y1) = (fw * 0.14, fh * 0.24, fw * 0.86, fh * 0.8);
    wood(c, x0, y0, x1, y1, wood_col, ow, s);
    rect(
        c,
        x0,
        y0,
        x1,
        y0 + (y1 - y0) * 0.35,
        shade(wood_col, 1.15),
        ow,
    );
    for t in [0.2, 0.8] {
        let x = x0 + (x1 - x0) * t;
        rect(c, x - 3.0 * s, y0, x + 3.0 * s, y1, band, ow * 0.6);
        for k in 0..3 {
            c.disc(
                x,
                y0 + (y1 - y0) * (0.2 + k as f32 * 0.3),
                1.1 * s,
                c3(200, 200, 205),
            );
        }
    }
}
fn stones_heap(c: &mut Canvas, cx: f32, cy: f32, r: f32, n: u32, seed: u32, base: Rgba, ow: f32) {
    for i in 0..n {
        let a = hash(i, seed, 1) * TAU;
        let d = r * hash(i, seed, 2).sqrt() * 0.75;
        let rr = r * (0.16 + hash(i, seed, 3) * 0.16);
        let col = mix(shade(base, 0.8), shade(base, 1.2), hash(i, seed, 4));
        rock(
            c,
            cx + a.cos() * d,
            cy + a.sin() * d,
            rr,
            col,
            seed + i,
            ow * 0.7,
        );
    }
}

/// Draws `id` if it is one of the expanded objects. Returns false for other ids.
pub fn draw(c: &mut Canvas, id: &str, fw: f32, fh: f32) -> bool {
    let (cx, cy) = (fw / 2.0, fh / 2.0);
    let s = fw.min(fh) / 64.0;
    let m = fw.min(fh);
    let ow = 2.0 * s.max(1.0);
    let bark = c3(96, 68, 44);
    let wood_c = c3(128, 88, 52);
    let stone = c3(128, 124, 116);
    let iron = c3(70, 72, 78);
    let gold = c3(212, 170, 70);
    let bone = c3(226, 218, 196);
    match id {
        // ---------------- trees
        "pine_tree" => {
            for (k, (r, col)) in [
                (0.46, c3(26, 58, 40)),
                (0.34, c3(38, 80, 52)),
                (0.22, c3(58, 104, 66)),
                (0.1, c3(84, 130, 82)),
            ]
            .iter()
            .enumerate()
            {
                c.poly(
                    &star(cx, cy, m * r, m * r * 0.62, 8, k as f32 * 0.2),
                    *col,
                    ow,
                );
            }
            c.disc(cx, cy, 2.5 * s, bark);
        }
        "birch_tree" => {
            canopy(
                c,
                cx,
                cy,
                m * 0.36,
                7,
                11,
                [c3(96, 140, 56), c3(132, 170, 74), c3(196, 220, 120)],
                ow,
            );
            for i in 0..10 {
                c.disc(
                    cx + (hash(i, 9, 1) - 0.5) * m * 0.6,
                    cy + (hash(i, 9, 2) - 0.5) * m * 0.6,
                    1.6 * s,
                    c3(220, 200, 90),
                );
            }
            rect(
                c,
                cx - 3.0 * s,
                cy + m * 0.3,
                cx + 3.0 * s,
                cy + m * 0.46,
                c3(236, 232, 222),
                ow * 0.6,
            );
            c.line(
                (cx - 3.0 * s, cy + m * 0.36),
                (cx + 1.0 * s, cy + m * 0.36),
                1.2 * s,
                INK,
            );
            c.line(
                (cx - 1.0 * s, cy + m * 0.42),
                (cx + 3.0 * s, cy + m * 0.42),
                1.2 * s,
                INK,
            );
        }
        "dead_tree" => {
            let grey = c3(108, 96, 84);
            for i in 0..6u32 {
                let a = i as f32 / 6.0 * TAU + hash(i, 3, 3) * 0.5;
                let l = m * (0.3 + hash(i, 3, 4) * 0.14);
                let end = (cx + a.cos() * l, cy + a.sin() * l);
                c.line((cx, cy), end, 5.0 * s + ow, INK);
                c.line((cx, cy), end, 5.0 * s, grey);
                for side in [-0.6f32, 0.6] {
                    let mid = (cx + a.cos() * l * 0.6, cy + a.sin() * l * 0.6);
                    let b = (
                        mid.0 + (a + side).cos() * l * 0.35,
                        mid.1 + (a + side).sin() * l * 0.35,
                    );
                    c.line(mid, b, 2.4 * s, shade(grey, 0.85));
                }
            }
            c.disc(cx, cy, 7.0 * s + ow, INK);
            c.disc(cx, cy, 7.0 * s, shade(grey, 1.1));
            c.disc(cx, cy, 3.0 * s, shade(grey, 0.7));
        }
        "ancient_tree" => {
            for i in 0..9u32 {
                let a = i as f32 / 9.0 * TAU + 0.2;
                c.line(
                    (cx, cy),
                    (cx + a.cos() * m * 0.49, cy + a.sin() * m * 0.49),
                    6.0 * s,
                    bark,
                );
            }
            canopy(
                c,
                cx,
                cy,
                m * 0.42,
                13,
                21,
                [c3(22, 54, 44), c3(36, 82, 60), c3(90, 140, 90)],
                ow,
            );
            for i in 0..8 {
                c.disc(
                    cx + (hash(i, 4, 1) - 0.5) * m * 0.55,
                    cy + (hash(i, 4, 2) - 0.5) * m * 0.55,
                    2.0 * s,
                    alpha(c3(200, 240, 160), 0.6),
                );
            }
        }
        "small_tree" => {
            c.disc(cx, cy, m * 0.3, alpha(INK, 0.15));
            canopy(
                c,
                cx,
                cy,
                m * 0.24,
                5,
                31,
                [c3(70, 120, 48), c3(98, 150, 60), c3(160, 200, 100)],
                ow,
            );
        }
        "fallen_tree" => {
            let (y0, y1) = (fh * 0.3, fh * 0.7);
            rect(c, fw * 0.08, y0, fw * 0.86, y1, bark, ow);
            for i in 0..14 {
                let x = fw * 0.1 + i as f32 * fw * 0.055;
                c.line(
                    (x, y0 + 2.0 * s),
                    (x + 6.0 * s, y1 - 2.0 * s),
                    1.3 * s,
                    shade(bark, 0.7),
                );
            }
            ell(
                c,
                fw * 0.08,
                cy,
                6.0 * s,
                (y1 - y0) / 2.0,
                c3(190, 150, 100),
                ow,
            );
            for r in [0.6, 0.35] {
                ell(
                    c,
                    fw * 0.08,
                    cy,
                    6.0 * s * r,
                    (y1 - y0) / 2.0 * r,
                    c3(160, 120, 76),
                    0.0,
                );
            }
            c.line((fw * 0.4, y0), (fw * 0.45, y0 - fh * 0.22), 3.0 * s, bark);
            c.line((fw * 0.6, y1), (fw * 0.66, y1 + fh * 0.2), 3.0 * s, bark);
            canopy(
                c,
                fw * 0.9,
                cy,
                fh * 0.3,
                5,
                7,
                [c3(60, 96, 44), c3(90, 128, 56), c3(150, 180, 90)],
                ow,
            );
        }
        "stump" => {
            for i in 0..5u32 {
                let a = i as f32 / 5.0 * TAU + 0.4;
                c.line(
                    (cx, cy),
                    (cx + a.cos() * m * 0.4, cy + a.sin() * m * 0.4),
                    4.0 * s,
                    bark,
                );
            }
            ell(c, cx, cy, m * 0.3, m * 0.3, bark, ow);
            ell(c, cx, cy, m * 0.24, m * 0.24, c3(196, 156, 104), 0.0);
            for r in [0.18, 0.12, 0.06] {
                c.poly(
                    &ell_pts(cx, cy, m * r, m * r, 0.0, 24),
                    alpha(c3(150, 110, 70), 0.0),
                    0.0,
                );
                for i in 0..24 {
                    let a = i as f32 / 24.0 * TAU;
                    c.disc(
                        cx + a.cos() * m * r,
                        cy + a.sin() * m * r,
                        0.7 * s,
                        c3(150, 110, 70),
                    );
                }
            }
            c.line((cx, cy), (cx + m * 0.2, cy - m * 0.08), 1.2 * s, INK);
        }
        // ---------------- plants
        "flower_bush" => {
            canopy(
                c,
                cx,
                cy,
                m * 0.36,
                6,
                41,
                [c3(46, 84, 40), c3(70, 112, 52), c3(120, 160, 80)],
                ow,
            );
            for i in 0..14 {
                let col = [c3(236, 120, 170), c3(250, 240, 240), c3(240, 210, 80)][i % 3];
                let (x, y) = (
                    cx + (hash(i as u32, 5, 1) - 0.5) * m * 0.62,
                    cy + (hash(i as u32, 5, 2) - 0.5) * m * 0.62,
                );
                c.disc(x, y, 2.4 * s, col);
                c.disc(x, y, 0.9 * s, c3(250, 210, 60));
            }
        }
        "fern" => {
            for i in 0..7u32 {
                let a = i as f32 / 7.0 * TAU + hash(i, 6, 1) * 0.3;
                let l = m * 0.42;
                c.line(
                    (cx, cy),
                    (cx + a.cos() * l, cy + a.sin() * l),
                    1.6 * s,
                    c3(50, 90, 40),
                );
                for k in 1..7 {
                    let t = k as f32 / 7.0;
                    let (x, y) = (cx + a.cos() * l * t, cy + a.sin() * l * t);
                    let ll = (1.0 - t * 0.7) * 5.0 * s;
                    for side in [-1.0f32, 1.0] {
                        let b = a + side * 1.1;
                        leaf(
                            c,
                            x + b.cos() * ll,
                            y + b.sin() * ll,
                            ll,
                            b,
                            mix(c3(70, 130, 60), c3(120, 170, 80), t),
                            0.0,
                        );
                    }
                }
            }
        }
        "tall_grass" => {
            for i in 0..60u32 {
                let (x, y) = (
                    fw * (0.08 + hash(i, 7, 1) * 0.84),
                    fh * (0.2 + hash(i, 7, 2) * 0.72),
                );
                let lean = (hash(i, 7, 3) - 0.5) * 10.0 * s;
                let col = mix(c3(90, 130, 50), c3(190, 200, 110), hash(i, 7, 4));
                c.line(
                    (x, y),
                    (x + lean, y - (10.0 + hash(i, 7, 5) * 10.0) * s),
                    1.4 * s,
                    col,
                );
            }
        }
        "mushrooms" => {
            for (i, (dx, dy, r)) in [
                (-0.16, 0.08, 0.17),
                (0.16, -0.12, 0.13),
                (0.14, 0.2, 0.1),
                (-0.12, -0.2, 0.08),
            ]
            .iter()
            .enumerate()
            {
                let (x, y, r) = (cx + dx * m, cy + dy * m, r * m);
                c.disc(x + r * 0.2, y + r * 0.3, r, alpha(INK, 0.3));
                c.disc(x, y, r + ow * 0.6, INK);
                c.disc(x, y, r, c3(186, 50, 40));
                for k in 0..4u32 {
                    let a = k as f32 * 1.7 + i as f32;
                    c.disc(
                        x + a.cos() * r * 0.55,
                        y + a.sin() * r * 0.55,
                        r * 0.16,
                        c3(246, 236, 220),
                    );
                }
            }
        }
        "glowing_mushrooms" => {
            c.blob(cx, cy, m * 0.48, m * 0.48, alpha(c3(80, 220, 230), 0.4));
            for (dx, dy, r) in [
                (-0.14, 0.1, 0.12),
                (0.12, -0.1, 0.1),
                (0.16, 0.16, 0.08),
                (-0.1, -0.18, 0.07),
                (0.0, 0.0, 0.06),
            ] {
                let (x, y, r) = (cx + dx * m, cy + dy * m, r * m);
                c.disc(x, y, r + ow * 0.5, c3(20, 40, 60));
                c.disc(x, y, r, c3(70, 190, 220));
                c.disc(x - r * 0.2, y - r * 0.2, r * 0.5, c3(200, 255, 250));
            }
        }
        "vines" => {
            for k in 0..3 {
                let y0 = fh * (0.2 + k as f32 * 0.3);
                let mut prev = (0.0, y0);
                for i in 1..=16 {
                    let x = fw * i as f32 / 16.0;
                    let p = (x, y0 + (x / fw * TAU * 1.5 + k as f32).sin() * 6.0 * s);
                    c.line(prev, p, 2.0 * s, c3(60, 90, 40));
                    if i % 2 == 0 {
                        let a: f32 = if i % 4 == 0 { -0.9 } else { 2.2 };
                        leaf(
                            c,
                            p.0 + a.cos() * 4.0 * s,
                            p.1 + a.sin() * 4.0 * s,
                            4.0 * s,
                            a,
                            c3(80, 140, 60),
                            ow * 0.4,
                        );
                    }
                    prev = p;
                }
            }
        }
        "roots" => {
            for i in 0..5u32 {
                let mut p = (fw * (0.1 + i as f32 * 0.2), 0.0);
                let mut w = 6.0 * s;
                for k in 0..6u32 {
                    let a = PI / 2.0 + (hash(i, k, 8) - 0.5) * 1.4;
                    let q = (p.0 + a.cos() * fh * 0.16, p.1 + a.sin() * fh * 0.16);
                    c.line(p, q, w + ow, INK);
                    c.line(p, q, w, c3(116, 86, 58));
                    p = q;
                    w *= 0.78;
                }
            }
        }
        "wildflowers" => {
            for i in 0..12u32 {
                let (x, y) = (
                    fw * (0.12 + hash(i, 12, 1) * 0.76),
                    fh * (0.12 + hash(i, 12, 2) * 0.76),
                );
                c.line((x, y), (x + 2.0 * s, y + 6.0 * s), 1.2 * s, c3(80, 130, 60));
                let col = [
                    c3(230, 90, 120),
                    c3(120, 120, 230),
                    c3(250, 220, 90),
                    c3(250, 250, 250),
                ][(i % 4) as usize];
                for p in 0..5 {
                    let a = p as f32 / 5.0 * TAU;
                    c.disc(x + a.cos() * 2.2 * s, y + a.sin() * 2.2 * s, 1.6 * s, col);
                }
                c.disc(x, y, 1.1 * s, c3(240, 190, 60));
            }
        }
        "reeds" => {
            c.blob(
                cx,
                cy + m * 0.1,
                m * 0.45,
                m * 0.3,
                alpha(c3(70, 110, 140), 0.5),
            );
            for i in 0..14u32 {
                let x = fw * (0.15 + hash(i, 13, 1) * 0.7);
                let y = fh * (0.8 - hash(i, 13, 2) * 0.1);
                let top = (
                    x + (hash(i, 13, 3) - 0.5) * 8.0 * s,
                    fh * (0.15 + hash(i, 13, 4) * 0.25),
                );
                c.line((x, y), top, 1.6 * s, c3(120, 140, 64));
                if i % 2 == 0 {
                    ell(
                        c,
                        top.0,
                        top.1 + 5.0 * s,
                        1.8 * s,
                        5.0 * s,
                        c3(110, 70, 40),
                        ow * 0.4,
                    );
                }
            }
        }
        // ---------------- rocks & terrain decorations
        "small_stones" => stones_heap(c, cx, cy, m * 0.5, 7, 3, c3(146, 140, 130), ow),
        "large_rock" => rock(c, cx, cy, m * 0.42, c3(120, 114, 106), 5, ow),
        "rock_cluster" => {
            for (i, (dx, dy, r)) in [
                (-0.18, -0.12, 0.24),
                (0.2, -0.16, 0.18),
                (0.12, 0.2, 0.22),
                (-0.22, 0.24, 0.13),
                (0.34, 0.3, 0.1),
            ]
            .iter()
            .enumerate()
            {
                rock(
                    c,
                    cx + dx * m,
                    cy + dy * m,
                    r * m,
                    mix(c3(104, 98, 92), c3(140, 134, 124), i as f32 / 4.0),
                    40 + i as u32,
                    ow,
                );
            }
        }
        "mossy_rock" => {
            rock(c, cx, cy, m * 0.4, c3(110, 108, 100), 9, ow);
            for i in 0..9u32 {
                c.disc(
                    cx + (hash(i, 14, 1) - 0.6) * m * 0.4,
                    cy + (hash(i, 14, 2) - 0.7) * m * 0.3,
                    (3.0 + hash(i, 14, 3) * 4.0) * s,
                    alpha(c3(84, 130, 54), 0.85),
                );
            }
        }
        "fallen_branches" => {
            for i in 0..4u32 {
                let a = hash(i, 15, 1) * PI;
                let l = m * 0.38;
                let (p, q) = (
                    (
                        cx - a.cos() * l,
                        cy - a.sin() * l * 0.8 + (i as f32 - 1.5) * 6.0 * s,
                    ),
                    (
                        cx + a.cos() * l,
                        cy + a.sin() * l * 0.8 + (i as f32 - 1.5) * 6.0 * s,
                    ),
                );
                c.line(p, q, 3.0 * s + ow, INK);
                c.line(p, q, 3.0 * s, c3(120, 88, 56));
                let mid = ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0);
                c.line(
                    mid,
                    (mid.0 + 7.0 * s, mid.1 - 6.0 * s),
                    1.5 * s,
                    c3(120, 88, 56),
                );
            }
        }
        "log" => {
            let (y0, y1) = (fh * 0.28, fh * 0.72);
            rect(c, fw * 0.1, y0, fw * 0.9, y1, c3(124, 90, 56), ow);
            c.line(
                (fw * 0.12, y0 + 3.0 * s),
                (fw * 0.88, y0 + 3.0 * s),
                2.0 * s,
                alpha(c3(200, 160, 110), 0.5),
            );
            for x in [0.1, 0.9] {
                ell(
                    c,
                    fw * x,
                    cy,
                    5.0 * s,
                    (y1 - y0) / 2.0,
                    c3(200, 160, 110),
                    ow,
                );
                ell(
                    c,
                    fw * x,
                    cy,
                    2.5 * s,
                    (y1 - y0) / 4.0,
                    c3(160, 120, 76),
                    0.0,
                );
            }
            c.blob(
                fw * 0.55,
                y0 + 4.0 * s,
                fw * 0.12,
                5.0 * s,
                alpha(c3(90, 140, 60), 0.9),
            );
        }
        "leaf_pile" => {
            for i in 0..70u32 {
                let a = hash(i, 16, 1) * TAU;
                let d = m * 0.4 * hash(i, 16, 2).sqrt();
                let col = [
                    c3(200, 110, 40),
                    c3(170, 60, 30),
                    c3(220, 170, 60),
                    c3(130, 80, 40),
                ][(i % 4) as usize];
                leaf(
                    c,
                    cx + a.cos() * d,
                    cy + a.sin() * d * 0.8,
                    3.6 * s,
                    hash(i, 16, 3) * TAU,
                    col,
                    ow * 0.25,
                );
            }
        }
        "puddle" => {
            let pts: Vec<Pt> = (0..14u32)
                .map(|i| {
                    let a = i as f32 / 14.0 * TAU;
                    let r = m * (0.3 + hash(i, 17, 1) * 0.12);
                    (cx + a.cos() * r, cy + a.sin() * r * 0.75)
                })
                .collect();
            c.poly(&pts, alpha(c3(60, 90, 70), 0.6), 0.0);
            c.poly(
                &ell_pts(cx, cy, m * 0.28, m * 0.2, 0.0, 20),
                alpha(c3(90, 140, 180), 0.75),
                0.0,
            );
            c.line(
                (cx - m * 0.15, cy - m * 0.06),
                (cx + m * 0.05, cy - m * 0.1),
                1.5 * s,
                alpha(c3(230, 245, 255), 0.8),
            );
            c.line(
                (cx - m * 0.05, cy + m * 0.06),
                (cx + m * 0.12, cy + m * 0.04),
                1.2 * s,
                alpha(c3(230, 245, 255), 0.6),
            );
        }
        // ---------------- tables & seating
        "small_table" => {
            wood(c, fw * 0.2, fh * 0.2, fw * 0.8, fh * 0.8, wood_c, ow, s);
            c.disc(cx + 6.0 * s, cy - 4.0 * s, 4.0 * s, c3(150, 110, 70));
            c.disc(cx + 6.0 * s, cy - 4.0 * s, 2.6 * s, c3(110, 60, 40));
        }
        "round_table" => {
            ell(c, cx, cy, m * 0.42, m * 0.42, wood_c, ow);
            for i in 0..6 {
                let y = cy - m * 0.35 + i as f32 * m * 0.14;
                c.line(
                    (cx - m * 0.34, y),
                    (cx + m * 0.34, y),
                    1.0 * s,
                    shade(wood_c, 0.75),
                );
            }
            ell(c, cx, cy, m * 0.12, m * 0.12, c3(236, 230, 214), ow * 0.6);
            c.disc(cx, cy, m * 0.06, c3(190, 120, 50));
        }
        "tavern_table" => {
            wood(
                c,
                fw * 0.06,
                fh * 0.16,
                fw * 0.94,
                fh * 0.84,
                c3(116, 80, 46),
                ow,
                s,
            );
            for (x, y) in [(0.25, 0.4), (0.55, 0.62), (0.78, 0.36)] {
                c.disc(fw * x, fh * y, 6.0 * s + ow * 0.6, INK);
                c.disc(fw * x, fh * y, 6.0 * s, c3(150, 110, 60));
                c.disc(fw * x, fh * y, 4.5 * s, c3(220, 160, 60));
                c.disc(fw * x - s, fh * y - s, 2.6 * s, c3(250, 244, 226));
            }
            c.disc(fw * 0.4, fh * 0.5, 2.0 * s, alpha(c3(90, 50, 20), 0.6));
        }
        "dining_table" => {
            wood(
                c,
                fw * 0.03,
                fh * 0.14,
                fw * 0.97,
                fh * 0.86,
                c3(110, 74, 44),
                ow,
                s,
            );
            rect(
                c,
                fw * 0.05,
                fh * 0.36,
                fw * 0.95,
                fh * 0.64,
                c3(236, 228, 210),
                0.0,
            );
            c.line(
                (fw * 0.05, fh * 0.4),
                (fw * 0.95, fh * 0.4),
                1.2 * s,
                c3(170, 40, 40),
            );
            c.line(
                (fw * 0.05, fh * 0.6),
                (fw * 0.95, fh * 0.6),
                1.2 * s,
                c3(170, 40, 40),
            );
            for i in 0..5 {
                let x = fw * (0.12 + i as f32 * 0.19);
                for y in [0.25, 0.75] {
                    c.disc(x, fh * y, 5.0 * s, c3(230, 226, 216));
                    c.disc(x, fh * y, 3.0 * s, c3(200, 190, 170));
                }
                c.disc(x + 9.0 * s, fh * 0.5, 2.8 * s, gold);
            }
        }
        "bench" => {
            wood(
                c,
                fw * 0.04,
                fh * 0.36,
                fw * 0.96,
                fh * 0.64,
                c3(128, 90, 54),
                ow,
                s,
            );
            for x in [0.08, 0.92] {
                rect(
                    c,
                    fw * x - 3.0 * s,
                    fh * 0.3,
                    fw * x + 3.0 * s,
                    fh * 0.7,
                    shade(wood_c, 0.7),
                    ow * 0.6,
                );
            }
        }
        "stool" => {
            for k in 0..3 {
                let a = k as f32 / 3.0 * TAU + 0.5;
                c.disc(
                    cx + a.cos() * m * 0.3,
                    cy + a.sin() * m * 0.3,
                    3.0 * s,
                    shade(wood_c, 0.6),
                );
            }
            ell(c, cx, cy, m * 0.25, m * 0.25, c3(140, 100, 60), ow);
            ell(c, cx, cy, m * 0.15, m * 0.15, c3(160, 120, 74), 0.0);
        }
        "throne" => {
            c.poly(
                &[
                    (fw * 0.16, fh * 0.1),
                    (fw * 0.3, fh * 0.02),
                    (cx, fh * 0.1),
                    (fw * 0.7, fh * 0.02),
                    (fw * 0.84, fh * 0.1),
                    (fw * 0.84, fh * 0.34),
                    (fw * 0.16, fh * 0.34),
                ],
                gold,
                ow,
            );
            rect(c, fw * 0.14, fh * 0.3, fw * 0.86, fh * 0.9, gold, ow);
            rect(
                c,
                fw * 0.26,
                fh * 0.36,
                fw * 0.74,
                fh * 0.82,
                c3(140, 30, 40),
                ow * 0.6,
            );
            c.disc(cx, fh * 0.2, 4.0 * s, c3(60, 120, 200));
            for x in [0.2, 0.8] {
                c.disc(fw * x, fh * 0.86, 3.0 * s, c3(250, 220, 120));
            }
        }
        // ---------------- storage
        "reinforced_chest" => chest(c, fw, fh, c3(110, 80, 44), iron, ow, s),
        "locked_chest" => {
            chest(c, fw, fh, c3(130, 96, 40), c3(180, 140, 50), ow, s);
            c.line(
                (fw * 0.12, fh * 0.22),
                (fw * 0.88, fh * 0.82),
                3.0 * s,
                iron,
            );
            c.line(
                (fw * 0.88, fh * 0.22),
                (fw * 0.12, fh * 0.82),
                3.0 * s,
                iron,
            );
            rect(
                c,
                cx - 6.0 * s,
                cy - 2.0 * s,
                cx + 6.0 * s,
                cy + 9.0 * s,
                gold,
                ow * 0.7,
            );
            c.disc(cx, cy + 3.0 * s, 1.8 * s, INK);
            c.poly(
                &ell_pts(cx, cy - 2.0 * s, 4.0 * s, 4.5 * s, 0.0, 16),
                alpha(gold, 0.0),
                0.0,
            );
            c.line(
                (cx - 4.0 * s, cy - 2.0 * s),
                (cx - 4.0 * s, cy - 7.0 * s),
                1.8 * s,
                c3(170, 170, 176),
            );
            c.line(
                (cx + 4.0 * s, cy - 2.0 * s),
                (cx + 4.0 * s, cy - 7.0 * s),
                1.8 * s,
                c3(170, 170, 176),
            );
            c.line(
                (cx - 4.0 * s, cy - 7.0 * s),
                (cx + 4.0 * s, cy - 7.0 * s),
                1.8 * s,
                c3(170, 170, 176),
            );
        }
        "cabinet" => {
            wood(
                c,
                fw * 0.12,
                fh * 0.12,
                fw * 0.88,
                fh * 0.88,
                c3(112, 76, 46),
                ow,
                s,
            );
            for x0 in [0.17, 0.52] {
                rect(
                    c,
                    fw * x0,
                    fh * 0.18,
                    fw * (x0 + 0.31),
                    fh * 0.82,
                    c3(132, 92, 56),
                    ow * 0.6,
                );
            }
            c.disc(fw * 0.45, cy, 1.8 * s, gold);
            c.disc(fw * 0.55, cy, 1.8 * s, gold);
        }
        "wardrobe" => {
            wood(
                c,
                fw * 0.05,
                fh * 0.1,
                fw * 0.95,
                fh * 0.9,
                c3(100, 66, 40),
                ow,
                s,
            );
            for x0 in [0.09, 0.51] {
                rect(
                    c,
                    fw * x0,
                    fh * 0.18,
                    fw * (x0 + 0.4),
                    fh * 0.82,
                    c3(120, 82, 50),
                    ow * 0.6,
                );
                let mid = fw * (x0 + 0.2);
                c.poly(
                    &ell_pts(mid, fh * 0.5, fw * 0.12, fh * 0.22, 0.0, 20),
                    alpha(c3(150, 106, 64), 0.8),
                    ow * 0.4,
                );
            }
            c.disc(fw * 0.47, cy, 2.0 * s, gold);
            c.disc(fw * 0.53, cy, 2.0 * s, gold);
        }
        "shelf" => {
            wood(
                c,
                fw * 0.03,
                fh * 0.3,
                fw * 0.97,
                fh * 0.7,
                c3(126, 88, 52),
                ow,
                s,
            );
            for (i, col) in [
                c3(150, 90, 50),
                c3(70, 120, 90),
                c3(200, 190, 160),
                c3(120, 60, 120),
                c3(170, 120, 60),
            ]
            .iter()
            .enumerate()
            {
                let x = fw * (0.12 + i as f32 * 0.19);
                c.disc(x, cy, 6.0 * s + ow * 0.6, INK);
                c.disc(x, cy, 6.0 * s, *col);
                c.disc(x, cy, 2.4 * s, shade(*col, 0.6));
            }
        }
        "sack" => {
            ell(
                c,
                cx,
                cy + m * 0.04,
                m * 0.32,
                m * 0.3,
                c3(170, 146, 100),
                ow,
            );
            for i in 0..6 {
                let a = i as f32 * 0.5 - 1.2;
                c.line(
                    (cx, cy - m * 0.22),
                    (cx + a.sin() * m * 0.3, cy + m * 0.2),
                    1.0 * s,
                    shade(c3(170, 146, 100), 0.8),
                );
            }
            c.disc(cx, cy - m * 0.24, 5.0 * s + ow * 0.6, INK);
            c.disc(cx, cy - m * 0.24, 5.0 * s, c3(150, 126, 84));
            c.line(
                (cx - 5.0 * s, cy - m * 0.24),
                (cx + 5.0 * s, cy - m * 0.24),
                1.6 * s,
                c3(100, 60, 30),
            );
            for i in 0..8u32 {
                c.disc(
                    cx + m * 0.3 + hash(i, 18, 1) * 6.0 * s,
                    cy + m * 0.3 + hash(i, 18, 2) * 5.0 * s,
                    1.1 * s,
                    c3(230, 200, 120),
                );
            }
        }
        // ---------------- interior decorations
        "bedroll" => {
            rect(
                c,
                fw * 0.22,
                fh * 0.2,
                fw * 0.78,
                fh * 0.9,
                c3(80, 110, 70),
                ow,
            );
            for k in 0..3 {
                let y = fh * (0.45 + k as f32 * 0.14);
                c.line((fw * 0.22, y), (fw * 0.78, y), 2.0 * s, c3(190, 170, 110));
            }
            ell(c, cx, fh * 0.2, fw * 0.3, 7.0 * s, c3(100, 130, 86), ow);
            c.line(
                (fw * 0.25, fh * 0.2),
                (fw * 0.75, fh * 0.2),
                1.2 * s,
                shade(c3(100, 130, 86), 0.7),
            );
        }
        "carpet" => {
            rect(
                c,
                fw * 0.08,
                fh * 0.06,
                fw * 0.92,
                fh * 0.94,
                c3(150, 40, 44),
                0.0,
            );
            rect(
                c,
                fw * 0.14,
                fh * 0.1,
                fw * 0.86,
                fh * 0.9,
                c3(196, 160, 70),
                0.0,
            );
            rect(
                c,
                fw * 0.18,
                fh * 0.13,
                fw * 0.82,
                fh * 0.87,
                c3(130, 30, 36),
                0.0,
            );
            for k in 0..4 {
                let y = fh * (0.22 + k as f32 * 0.19);
                c.poly(
                    &[
                        (cx, y - 8.0 * s),
                        (cx + 14.0 * s, y),
                        (cx, y + 8.0 * s),
                        (cx - 14.0 * s, y),
                    ],
                    c3(214, 180, 90),
                    0.0,
                );
                c.disc(cx, y, 3.0 * s, c3(40, 60, 110));
            }
            for i in 0..12 {
                let x = fw * (0.1 + i as f32 * 0.073);
                c.line((x, fh * 0.02), (x, fh * 0.06), 1.0 * s, c3(220, 200, 150));
                c.line((x, fh * 0.94), (x, fh * 0.98), 1.0 * s, c3(220, 200, 150));
            }
        }
        "rug" => {
            for (k, col) in [
                c3(60, 80, 140),
                c3(200, 170, 90),
                c3(50, 66, 120),
                c3(170, 60, 60),
                c3(220, 200, 150),
            ]
            .iter()
            .enumerate()
            {
                let r = 0.46 - k as f32 * 0.08;
                ell(c, cx, cy, m * r, m * r * 0.8, *col, 0.0);
            }
        }
        "candle" => {
            c.disc(cx, cy, m * 0.18, c3(150, 150, 156));
            c.disc(cx, cy, m * 0.15, c3(190, 190, 196));
            c.disc(cx, cy, m * 0.09 + ow * 0.5, INK);
            c.disc(cx, cy, m * 0.09, c3(240, 232, 210));
            c.blob(
                cx + m * 0.08,
                cy + m * 0.04,
                3.0 * s,
                4.0 * s,
                c3(240, 232, 210),
            );
            flame(c, cx, cy - 1.0 * s, 3.0 * s);
        }
        "candelabra" => {
            for a in [-0.5f32, 0.5] {
                c.line(
                    (cx, cy + m * 0.1),
                    (cx + a * m * 0.6, cy - m * 0.05),
                    3.0 * s,
                    gold,
                );
            }
            c.disc(cx, cy + m * 0.18, m * 0.14, shade(gold, 0.8));
            for dx in [-0.3, 0.0, 0.3] {
                c.disc(cx + dx * m, cy - m * 0.05, 4.0 * s + ow * 0.5, INK);
                c.disc(cx + dx * m, cy - m * 0.05, 4.0 * s, c3(240, 232, 210));
                flame(c, cx + dx * m, cy - m * 0.08, 2.4 * s);
            }
        }
        "fireplace" => {
            rect(c, fw * 0.02, fh * 0.04, fw * 0.98, fh * 0.6, stone, ow);
            for i in 0..8 {
                let x = fw * (0.02 + i as f32 * 0.12);
                rect(
                    c,
                    x,
                    fh * 0.04,
                    x + fw * 0.12,
                    fh * 0.32,
                    mix(stone, c3(160, 154, 144), hash(i, 19, 1)),
                    ow * 0.4,
                );
            }
            c.poly(
                &ell_pts(cx, fh * 0.58, fw * 0.32, fh * 0.36, 0.0, 28),
                c3(30, 24, 22),
                ow,
            );
            rect(
                c,
                fw * 0.02,
                fh * 0.6,
                fw * 0.98,
                fh * 0.96,
                alpha(c3(0, 0, 0), 0.0),
                0.0,
            );
            c.line(
                (cx - fw * 0.18, fh * 0.62),
                (cx + fw * 0.15, fh * 0.5),
                4.0 * s,
                bark,
            );
            c.line(
                (cx - fw * 0.12, fh * 0.48),
                (cx + fw * 0.2, fh * 0.6),
                4.0 * s,
                bark,
            );
            flame(c, cx, fh * 0.5, 8.0 * s);
            flame(c, cx - fw * 0.08, fh * 0.56, 5.0 * s);
            flame(c, cx + fw * 0.09, fh * 0.55, 5.0 * s);
        }
        "cooking_pot" => {
            for k in 0..3 {
                let a = k as f32 / 3.0 * TAU - PI / 2.0;
                c.line(
                    (cx, cy),
                    (cx + a.cos() * m * 0.46, cy + a.sin() * m * 0.46),
                    2.4 * s,
                    bark,
                );
            }
            c.disc(cx, cy, m * 0.33 + ow, INK);
            c.disc(cx, cy, m * 0.33, c3(56, 56, 60));
            c.disc(cx, cy, m * 0.25, c3(130, 84, 40));
            for i in 0..5u32 {
                c.disc(
                    cx + (hash(i, 20, 1) - 0.5) * m * 0.35,
                    cy + (hash(i, 20, 2) - 0.5) * m * 0.35,
                    2.0 * s,
                    c3(190, 140, 70),
                );
            }
            c.line(
                (cx - m * 0.33, cy),
                (cx - m * 0.4, cy - m * 0.04),
                2.0 * s,
                iron,
            );
            c.line(
                (cx + m * 0.33, cy),
                (cx + m * 0.4, cy - m * 0.04),
                2.0 * s,
                iron,
            );
            c.blob(
                cx + m * 0.1,
                cy - m * 0.2,
                m * 0.15,
                m * 0.1,
                alpha(c3(240, 240, 240), 0.35),
            );
        }
        "wall_banner" => {
            c.line(
                (fw * 0.18, fh * 0.12),
                (fw * 0.82, fh * 0.12),
                3.0 * s + ow,
                INK,
            );
            c.line(
                (fw * 0.18, fh * 0.12),
                (fw * 0.82, fh * 0.12),
                3.0 * s,
                gold,
            );
            c.poly(
                &[
                    (fw * 0.26, fh * 0.14),
                    (fw * 0.74, fh * 0.14),
                    (fw * 0.74, fh * 0.9),
                    (cx, fh * 0.74),
                    (fw * 0.26, fh * 0.9),
                ],
                c3(150, 36, 40),
                ow,
            );
            c.poly(
                &star(cx, fh * 0.42, m * 0.12, m * 0.05, 5, -PI / 2.0),
                gold,
                0.0,
            );
            c.line(
                (fw * 0.3, fh * 0.2),
                (fw * 0.3, fh * 0.82),
                1.0 * s,
                alpha(gold, 0.7),
            );
            c.line(
                (fw * 0.7, fh * 0.2),
                (fw * 0.7, fh * 0.82),
                1.0 * s,
                alpha(gold, 0.7),
            );
        }
        "painting" => {
            rect(c, fw * 0.12, fh * 0.2, fw * 0.88, fh * 0.8, gold, ow);
            rect(
                c,
                fw * 0.2,
                fh * 0.28,
                fw * 0.8,
                fh * 0.72,
                c3(140, 180, 210),
                ow * 0.4,
            );
            c.poly(
                &[
                    (fw * 0.2, fh * 0.6),
                    (fw * 0.4, fh * 0.44),
                    (fw * 0.58, fh * 0.58),
                    (fw * 0.8, fh * 0.48),
                    (fw * 0.8, fh * 0.72),
                    (fw * 0.2, fh * 0.72),
                ],
                c3(80, 120, 60),
                0.0,
            );
            c.disc(fw * 0.68, fh * 0.36, 3.0 * s, c3(250, 230, 150));
            for (x, y) in [(0.12, 0.2), (0.88, 0.2), (0.12, 0.8), (0.88, 0.8)] {
                c.disc(fw * x, fh * y, 2.5 * s, c3(240, 210, 120));
            }
        }
        "mirror" => {
            ell(c, cx, cy, m * 0.3, m * 0.42, c3(170, 170, 180), ow);
            for i in 0..16 {
                let t = i as f32 / 16.0;
                let r = 1.0 - t * 0.9;
                c.poly(
                    &ell_pts(cx, cy, m * 0.24 * r, m * 0.36 * r, 0.0, 24),
                    alpha(mix(c3(150, 190, 210), c3(225, 240, 250), t), 0.5),
                    0.0,
                );
            }
            c.line(
                (cx - m * 0.12, cy - m * 0.2),
                (cx + m * 0.02, cy - m * 0.3),
                2.0 * s,
                alpha(c3(255, 255, 255), 0.9),
            );
            c.line(
                (cx - m * 0.14, cy - m * 0.08),
                (cx + m * 0.06, cy - m * 0.22),
                1.2 * s,
                alpha(c3(255, 255, 255), 0.7),
            );
        }
        "desk" => {
            wood(
                c,
                fw * 0.04,
                fh * 0.14,
                fw * 0.96,
                fh * 0.86,
                c3(118, 80, 48),
                ow,
                s,
            );
            rect(
                c,
                fw * 0.18,
                fh * 0.3,
                fw * 0.42,
                fh * 0.74,
                c3(230, 216, 180),
                ow * 0.4,
            );
            for k in 0..5 {
                let y = fh * (0.38 + k as f32 * 0.07);
                c.line((fw * 0.21, y), (fw * 0.39, y), 0.9 * s, alpha(INK, 0.7));
            }
            c.disc(fw * 0.62, fh * 0.42, 4.0 * s + ow * 0.5, INK);
            c.disc(fw * 0.62, fh * 0.42, 4.0 * s, c3(30, 30, 50));
            c.line(
                (fw * 0.64, fh * 0.4),
                (fw * 0.78, fh * 0.2),
                1.6 * s,
                c3(240, 240, 240),
            );
            rect(
                c,
                fw * 0.7,
                fh * 0.52,
                fw * 0.9,
                fh * 0.76,
                c3(120, 50, 40),
                ow * 0.5,
            );
        }
        "scroll" => {
            rect(
                c,
                fw * 0.2,
                fh * 0.3,
                fw * 0.8,
                fh * 0.7,
                c3(226, 210, 170),
                ow,
            );
            for x in [0.2, 0.8] {
                ell(c, fw * x, cy, 5.0 * s, fh * 0.22, c3(200, 180, 140), ow);
                c.disc(fw * x, fh * 0.26, 2.0 * s, bark);
                c.disc(fw * x, fh * 0.74, 2.0 * s, bark);
            }
            for k in 0..4 {
                let y = fh * (0.38 + k as f32 * 0.07);
                c.line((fw * 0.3, y), (fw * 0.68, y), 0.9 * s, alpha(INK, 0.6));
            }
            c.disc(fw * 0.62, fh * 0.64, 4.0 * s, c3(170, 30, 30));
        }
        "books" => {
            for (k, col) in [c3(120, 50, 40), c3(50, 80, 120), c3(60, 100, 60)]
                .iter()
                .enumerate()
            {
                let o = k as f32 * 5.0 * s;
                let (x0, y0) = (fw * 0.22 + o * 0.6, fh * 0.26 + o);
                rect(c, x0, y0, x0 + fw * 0.5, y0 + fh * 0.3, *col, ow);
                c.line(
                    (x0 + fw * 0.46, y0 + 3.0 * s),
                    (x0 + fw * 0.46, y0 + fh * 0.27),
                    2.0 * s,
                    c3(236, 226, 200),
                );
                c.line(
                    (x0 + 3.0 * s, y0 + fh * 0.15),
                    (x0 + fw * 0.4, y0 + fh * 0.15),
                    1.0 * s,
                    gold,
                );
            }
        }
        "bottles" => {
            for (k, (dx, dy, col)) in [
                (-0.18, 0.06, c3(60, 120, 70)),
                (0.0, -0.12, c3(120, 70, 40)),
                (0.18, 0.08, c3(60, 90, 160)),
                (0.02, 0.22, c3(140, 40, 60)),
            ]
            .iter()
            .enumerate()
            {
                let (x, y) = (cx + dx * m, cy + dy * m);
                let r = (7.0 - k as f32 * 0.6) * s;
                c.disc(x, y, r + ow * 0.6, INK);
                c.disc(x, y, r, *col);
                c.disc(x, y, r * 0.45, shade(*col, 0.7));
                c.disc(x, y, r * 0.3, c3(170, 130, 90));
                c.disc(
                    x - r * 0.4,
                    y - r * 0.4,
                    r * 0.25,
                    alpha(c3(255, 255, 255), 0.7),
                );
            }
        }
        "plates" => {
            for (dx, dy) in [(-0.17, -0.1), (0.18, 0.12)] {
                let (x, y) = (cx + dx * m, cy + dy * m);
                c.disc(x, y, m * 0.2 + ow * 0.5, INK);
                c.disc(x, y, m * 0.2, c3(232, 228, 218));
                c.disc(x, y, m * 0.14, c3(214, 208, 196));
            }
            ell(
                c,
                cx - 0.17 * m,
                cy - 0.1 * m,
                m * 0.1,
                m * 0.06,
                c3(196, 140, 70),
                ow * 0.4,
            );
            c.poly(
                &[
                    (cx + 0.1 * m, cy + 0.06 * m),
                    (cx + 0.28 * m, cy + 0.1 * m),
                    (cx + 0.14 * m, cy + 0.22 * m),
                ],
                c3(240, 200, 80),
                ow * 0.4,
            );
            c.disc(cx + 0.22 * m, cy - 0.22 * m, 4.0 * s, c3(170, 40, 50));
        }
        "lantern" => {
            c.blob(cx, cy, m * 0.42, m * 0.42, alpha(c3(255, 200, 110), 0.45));
            rect(
                c,
                cx - m * 0.18,
                cy - m * 0.18,
                cx + m * 0.18,
                cy + m * 0.18,
                iron,
                ow,
            );
            rect(
                c,
                cx - m * 0.13,
                cy - m * 0.13,
                cx + m * 0.13,
                cy + m * 0.13,
                c3(255, 214, 120),
                0.0,
            );
            c.line((cx - m * 0.18, cy), (cx + m * 0.18, cy), 1.6 * s, iron);
            c.line((cx, cy - m * 0.18), (cx, cy + m * 0.18), 1.6 * s, iron);
            c.disc(cx, cy, 2.5 * s, c3(255, 250, 220));
            c.poly(
                &ell_pts(cx, cy - m * 0.26, m * 0.08, m * 0.07, 0.0, 14),
                alpha(iron, 0.0),
                ow * 0.8,
            );
        }
        // ---------------- dungeon architecture
        "broken_pillar" => {
            c.poly(
                &ell_pts(cx, cy, m * 0.36, m * 0.36, PI / 8.0, 8),
                shade(stone, 0.85),
                ow,
            );
            let jag: Vec<Pt> = (0..12u32)
                .map(|i| {
                    let a = i as f32 / 12.0 * TAU;
                    let r = m * if i % 2 == 0 {
                        0.27
                    } else {
                        0.2 + hash(i, 22, 1) * 0.05
                    };
                    (cx + a.cos() * r, cy + a.sin() * r)
                })
                .collect();
            c.poly(&jag, c3(160, 154, 144), ow);
            c.disc(cx - 3.0 * s, cy - 3.0 * s, m * 0.08, c3(186, 180, 170));
            rock(c, cx + m * 0.38, cy + m * 0.34, m * 0.1, stone, 3, ow * 0.7);
            rock(c, cx - m * 0.36, cy + m * 0.4, m * 0.07, stone, 4, ow * 0.7);
        }
        "stone_arch" => {
            for x in [0.1, 0.9] {
                rect(
                    c,
                    fw * x - fh * 0.3,
                    fh * 0.15,
                    fw * x + fh * 0.3,
                    fh * 0.85,
                    stone,
                    ow,
                );
                rect(
                    c,
                    fw * x - fh * 0.22,
                    fh * 0.23,
                    fw * x + fh * 0.22,
                    fh * 0.77,
                    c3(150, 144, 134),
                    0.0,
                );
            }
            rect(
                c,
                fw * 0.1 + fh * 0.3,
                fh * 0.3,
                fw * 0.9 - fh * 0.3,
                fh * 0.7,
                alpha(c3(140, 134, 124), 0.9),
                ow,
            );
            let n = 7;
            for i in 1..n {
                let x = fw * 0.1 + fh * 0.3 + (fw * 0.8 - fh * 0.6) * i as f32 / n as f32;
                c.line((x, fh * 0.3), (x, fh * 0.7), 1.2 * s, INK);
            }
            c.poly(
                &[
                    (cx - 5.0 * s, fh * 0.3),
                    (cx + 5.0 * s, fh * 0.3),
                    (cx + 3.0 * s, fh * 0.7),
                    (cx - 3.0 * s, fh * 0.7),
                ],
                c3(170, 164, 154),
                ow * 0.5,
            );
        }
        "iron_gate" => {
            rect(
                c,
                fw * 0.04,
                fh * 0.38,
                fw * 0.96,
                fh * 0.62,
                alpha(iron, 0.0),
                0.0,
            );
            for k in 0..3 {
                let y = fh * (0.4 + k as f32 * 0.1);
                c.line((fw * 0.04, y), (fw * 0.96, y), 2.5 * s + ow, INK);
                c.line((fw * 0.04, y), (fw * 0.96, y), 2.5 * s, iron);
            }
            for i in 0..7 {
                let x = fw * (0.08 + i as f32 * 0.14);
                c.line((x, fh * 0.32), (x, fh * 0.68), 2.5 * s + ow, INK);
                c.line((x, fh * 0.32), (x, fh * 0.68), 2.5 * s, c3(96, 98, 106));
                c.poly(
                    &[
                        (x, fh * 0.24),
                        (x + 3.0 * s, fh * 0.32),
                        (x - 3.0 * s, fh * 0.32),
                    ],
                    gold,
                    ow * 0.4,
                );
            }
        }
        "prison_bars" => {
            rect(
                c,
                fw * 0.02,
                fh * 0.44,
                fw * 0.98,
                fh * 0.56,
                c3(84, 66, 52),
                ow,
            );
            bars(c, fw * 0.02, fw * 0.98, cy, 6, 3.6 * s, c3(110, 84, 64), ow);
            for i in 0..6u32 {
                c.disc(
                    fw * (0.1 + hash(i, 23, 1) * 0.8),
                    cy + (hash(i, 23, 2) - 0.5) * 6.0 * s,
                    1.2 * s,
                    c3(160, 80, 40),
                );
            }
        }
        "heavy_door" => {
            wood(
                c,
                fw * 0.08,
                fh * 0.3,
                fw * 0.92,
                fh * 0.7,
                c3(96, 66, 40),
                ow,
                s,
            );
            for x in [0.22, 0.5, 0.78] {
                rect(
                    c,
                    fw * x - 2.5 * s,
                    fh * 0.3,
                    fw * x + 2.5 * s,
                    fh * 0.7,
                    iron,
                    ow * 0.5,
                );
                for y in [0.36, 0.5, 0.64] {
                    c.disc(fw * x, fh * y, 1.3 * s, c3(190, 190, 196));
                }
            }
            c.poly(
                &ell_pts(fw * 0.66, cy + 3.0 * s, 5.0 * s, 5.0 * s, 0.0, 16),
                alpha(gold, 0.0),
                2.0 * s,
            );
            c.disc(fw * 0.66, cy - 2.0 * s, 2.0 * s, gold);
            rect(c, fw * 0.02, fh * 0.26, fw * 0.08, fh * 0.74, stone, ow);
            rect(c, fw * 0.92, fh * 0.26, fw * 0.98, fh * 0.74, stone, ow);
        }
        "secret_door" => {
            rect(c, 0.0, fh * 0.2, fw, fh * 0.8, c3(110, 106, 100), 0.0);
            for row in 0..3 {
                let y0 = fh * (0.2 + row as f32 * 0.2);
                let off = if row % 2 == 0 { 0.0 } else { fw * 0.16 };
                for i in 0..4 {
                    let x0 = off + i as f32 * fw * 0.33 - fw * 0.1;
                    rect(
                        c,
                        x0,
                        y0,
                        x0 + fw * 0.31,
                        y0 + fh * 0.18,
                        mix(c3(100, 96, 90), c3(132, 128, 120), hash(i, row, 24)),
                        ow * 0.4,
                    );
                }
            }
            c.line(
                (fw * 0.3, fh * 0.2),
                (fw * 0.3, fh * 0.8),
                0.8 * s,
                alpha(c3(20, 16, 14), 0.9),
            );
            c.line(
                (fw * 0.7, fh * 0.2),
                (fw * 0.7, fh * 0.8),
                0.8 * s,
                alpha(c3(20, 16, 14), 0.9),
            );
            c.poly(
                &star(cx, cy, 4.0 * s, 1.6 * s, 4, 0.0),
                alpha(c3(150, 200, 220), 0.6),
                0.0,
            );
        }
        "stone_stairs" => {
            let n = 7;
            for i in 0..n {
                let t = i as f32 / n as f32;
                let (y0, y1) = (
                    fh * (0.04 + t * 0.92),
                    fh * (0.04 + (t + 1.0 / n as f32) * 0.92),
                );
                let col = mix(c3(176, 170, 160), c3(60, 56, 52), t);
                rect(c, fw * 0.12, y0, fw * 0.88, y1, col, ow * 0.6);
                c.line(
                    (fw * 0.14, y0 + 1.5 * s),
                    (fw * 0.86, y0 + 1.5 * s),
                    1.4 * s,
                    alpha(c3(230, 226, 216), 0.6 * (1.0 - t)),
                );
            }
            rect(c, fw * 0.02, fh * 0.02, fw * 0.12, fh * 0.98, stone, ow);
            rect(c, fw * 0.88, fh * 0.02, fw * 0.98, fh * 0.98, stone, ow);
        }
        "broken_wall" => {
            for row in 0..4 {
                let y0 = fh * (0.1 + row as f32 * 0.2);
                let width = 1.0 - row as f32 * 0.22;
                let off = if row % 2 == 0 { 0.0 } else { fw * 0.12 };
                let mut x = off;
                while x < fw * width {
                    rect(
                        c,
                        x,
                        y0,
                        (x + fw * 0.24).min(fw * width),
                        y0 + fh * 0.18,
                        mix(c3(110, 102, 96), c3(146, 140, 130), hash(x as u32, row, 25)),
                        ow * 0.6,
                    );
                    x += fw * 0.25;
                }
            }
            stones_heap(
                c,
                fw * 0.75,
                fh * 0.75,
                m * 0.3,
                6,
                7,
                c3(130, 122, 112),
                ow,
            );
        }
        "rubble_pile" => {
            c.blob(cx, cy, m * 0.48, m * 0.4, alpha(c3(150, 140, 126), 0.6));
            stones_heap(c, cx, cy, m * 0.52, 12, 11, c3(128, 120, 110), ow);
        }
        // ---------------- dungeon atmosphere
        "skeleton" => {
            let (x, top) = (cx, fh * 0.2);
            c.line((x, top + 8.0 * s), (x, fh * 0.64), 2.0 * s, bone);
            for k in 0..4 {
                let y = fh * (0.34 + k as f32 * 0.06);
                let w = (10.0 - k as f32 * 1.4) * s;
                c.line((x - w, y + 2.0 * s), (x, y), 1.6 * s, bone);
                c.line((x + w, y + 2.0 * s), (x, y), 1.6 * s, bone);
            }
            c.line((x, fh * 0.32), (x - 16.0 * s, fh * 0.5), 1.8 * s, bone);
            c.line((x, fh * 0.32), (x + 15.0 * s, fh * 0.46), 1.8 * s, bone);
            ell(c, x, fh * 0.66, 7.0 * s, 4.0 * s, bone, ow * 0.5);
            c.line(
                (x - 4.0 * s, fh * 0.68),
                (x - 9.0 * s, fh * 0.92),
                2.0 * s,
                bone,
            );
            c.line(
                (x + 4.0 * s, fh * 0.68),
                (x + 12.0 * s, fh * 0.9),
                2.0 * s,
                bone,
            );
            skull(c, x, top, 6.5 * s, ow);
        }
        "skull_pile" => {
            for (i, (dx, dy)) in [
                (-0.2, 0.18),
                (0.0, 0.2),
                (0.2, 0.18),
                (-0.1, 0.0),
                (0.1, 0.0),
                (0.0, -0.18),
            ]
            .iter()
            .enumerate()
            {
                let r = (8.0 - (i / 3) as f32) * s;
                skull(c, cx + dx * m, cy + dy * m, r, ow);
            }
        }
        "cobweb" => {
            let o = (fw * 0.02, fh * 0.02);
            let web = alpha(c3(236, 236, 242), 0.8);
            for i in 0..7 {
                let a = i as f32 / 6.0 * PI / 2.0;
                c.line(
                    o,
                    (o.0 + a.cos() * m * 1.0, o.1 + a.sin() * m * 1.0),
                    1.0 * s,
                    web,
                );
            }
            for k in 1..6 {
                let r = k as f32 * m * 0.17;
                let pts: Vec<Pt> = (0..7)
                    .map(|i| {
                        let a = i as f32 / 6.0 * PI / 2.0;
                        (o.0 + a.cos() * r, o.1 + a.sin() * r)
                    })
                    .collect();
                for w in pts.windows(2) {
                    let mid = (
                        (w[0].0 + w[1].0) / 2.0 - 2.0 * s,
                        (w[0].1 + w[1].1) / 2.0 - 2.0 * s,
                    );
                    c.line(w[0], mid, 0.9 * s, web);
                    c.line(mid, w[1], 0.9 * s, web);
                }
            }
            c.disc(o.0 + m * 0.4, o.1 + m * 0.36, 3.0 * s, INK);
            c.disc(o.0 + m * 0.4, o.1 + m * 0.31, 2.0 * s, INK);
        }
        "chains" => {
            for k in 0..9 {
                let t = k as f32 / 8.0;
                let (x, y) = (fw * (0.12 + t * 0.76), fh * (0.12 + t * 0.6));
                let ang = PI / 4.0 + if k % 2 == 0 { 0.0 } else { PI / 2.0 };
                c.poly(
                    &ell_pts(x, y, 6.0 * s, 3.4 * s, ang, 16),
                    alpha(iron, 0.0),
                    2.4 * s,
                );
                c.poly(
                    &ell_pts(x, y, 5.0 * s, 2.4 * s, ang, 16),
                    alpha(iron, 0.0),
                    0.0,
                );
            }
            c.poly(
                &ell_pts(fw * 0.88, fh * 0.78, 8.0 * s, 8.0 * s, 0.0, 20),
                alpha(iron, 0.0),
                3.0 * s,
            );
            c.disc(fw * 0.12, fh * 0.12, 4.0 * s, c3(110, 112, 120));
        }
        "hanging_cage" => {
            c.disc(cx + 4.0 * s, cy + 6.0 * s, m * 0.38, alpha(INK, 0.3));
            ell(c, cx, cy, m * 0.36, m * 0.36, c3(40, 34, 30), ow);
            skull(c, cx + 3.0 * s, cy + 4.0 * s, 5.0 * s, ow * 0.6);
            for i in 0..12 {
                let a = i as f32 / 12.0 * TAU;
                c.line(
                    (cx, cy),
                    (cx + a.cos() * m * 0.36, cy + a.sin() * m * 0.36),
                    1.6 * s,
                    iron,
                );
            }
            c.poly(
                &ell_pts(cx, cy, m * 0.36, m * 0.36, 0.0, 28),
                alpha(iron, 0.0),
                2.6 * s,
            );
            c.poly(
                &ell_pts(cx, cy, m * 0.2, m * 0.2, 0.0, 20),
                alpha(iron, 0.0),
                1.8 * s,
            );
            c.disc(cx, cy, 4.0 * s, c3(110, 112, 120));
        }
        "tombstone" => {
            for i in 0..14u32 {
                let x = fw * (0.2 + hash(i, 26, 1) * 0.6);
                c.line(
                    (x, fh * 0.86),
                    (x + 2.0 * s, fh * 0.76),
                    1.4 * s,
                    c3(90, 130, 60),
                );
            }
            let mut pts = ell_pts(cx, fh * 0.34, fw * 0.28, fh * 0.2, 0.0, 24);
            pts.retain(|p| p.1 <= fh * 0.34);
            pts.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            pts.insert(0, (cx + fw * 0.28, fh * 0.82));
            pts.push((cx - fw * 0.28, fh * 0.82));
            c.poly(&pts, c3(140, 138, 132), ow);
            c.line((cx, fh * 0.28), (cx, fh * 0.62), 3.0 * s, c3(90, 88, 84));
            c.line(
                (cx - 8.0 * s, fh * 0.38),
                (cx + 8.0 * s, fh * 0.38),
                3.0 * s,
                c3(90, 88, 84),
            );
            c.blob(
                cx - fw * 0.15,
                fh * 0.7,
                6.0 * s,
                4.0 * s,
                alpha(c3(90, 130, 60), 0.9),
            );
        }
        "coffin" => {
            let pts = [
                (cx, fh * 0.04),
                (fw * 0.84, fh * 0.2),
                (fw * 0.7, fh * 0.96),
                (fw * 0.3, fh * 0.96),
                (fw * 0.16, fh * 0.2),
            ];
            c.poly(&pts, c3(90, 60, 40), ow);
            let inner: Vec<Pt> = pts
                .iter()
                .map(|(x, y)| (cx + (x - cx) * 0.8, fh * 0.5 + (y - fh * 0.5) * 0.9))
                .collect();
            c.poly(&inner, c3(110, 76, 50), ow * 0.4);
            c.line((cx, fh * 0.2), (cx, fh * 0.6), 3.0 * s, gold);
            c.line(
                (cx - 9.0 * s, fh * 0.32),
                (cx + 9.0 * s, fh * 0.32),
                3.0 * s,
                gold,
            );
        }
        "sarcophagus" => {
            rect(
                c,
                fw * 0.1,
                fh * 0.04,
                fw * 0.9,
                fh * 0.96,
                c3(150, 140, 120),
                ow,
            );
            rect(
                c,
                fw * 0.16,
                fh * 0.08,
                fw * 0.84,
                fh * 0.92,
                c3(176, 166, 146),
                ow * 0.4,
            );
            ell(
                c,
                cx,
                fh * 0.18,
                8.0 * s,
                9.0 * s,
                c3(196, 186, 166),
                ow * 0.6,
            );
            c.poly(
                &[
                    (cx - 14.0 * s, fh * 0.3),
                    (cx + 14.0 * s, fh * 0.3),
                    (cx + 9.0 * s, fh * 0.86),
                    (cx - 9.0 * s, fh * 0.86),
                ],
                c3(190, 180, 160),
                ow * 0.6,
            );
            c.line(
                (cx - 10.0 * s, fh * 0.36),
                (cx + 6.0 * s, fh * 0.46),
                3.0 * s,
                c3(160, 150, 130),
            );
            c.line(
                (cx + 10.0 * s, fh * 0.36),
                (cx - 6.0 * s, fh * 0.46),
                3.0 * s,
                c3(160, 150, 130),
            );
            c.line((cx, fh * 0.4), (cx, fh * 0.8), 2.0 * s, gold);
            rect(c, fw * 0.1, fh * 0.04, fw * 0.9, fh * 0.08, gold, 0.0);
            rect(c, fw * 0.1, fh * 0.92, fw * 0.9, fh * 0.96, gold, 0.0);
        }
        "bloodstain" => {
            let red = c3(120, 18, 22);
            ell(c, cx, cy, m * 0.24, m * 0.18, alpha(red, 0.85), 0.0);
            for i in 0..12u32 {
                let a = hash(i, 27, 1) * TAU;
                let d = m * (0.22 + hash(i, 27, 2) * 0.22);
                c.disc(
                    cx + a.cos() * d,
                    cy + a.sin() * d,
                    (1.2 + hash(i, 27, 3) * 3.0) * s,
                    alpha(red, 0.85),
                );
            }
            c.blob(
                cx - 3.0 * s,
                cy - 3.0 * s,
                m * 0.1,
                m * 0.06,
                alpha(c3(200, 60, 60), 0.4),
            );
        }
        "statue" => {
            rect(
                c,
                fw * 0.1,
                fh * 0.1,
                fw * 0.9,
                fh * 0.9,
                c3(130, 126, 118),
                ow,
            );
            rect(
                c,
                fw * 0.16,
                fh * 0.16,
                fw * 0.84,
                fh * 0.84,
                c3(150, 146, 138),
                ow * 0.4,
            );
            ell(
                c,
                cx,
                cy + 2.0 * s,
                m * 0.3,
                m * 0.14,
                c3(196, 192, 182),
                ow,
            );
            c.line(
                (cx + m * 0.24, cy - m * 0.3),
                (cx + m * 0.2, cy + m * 0.32),
                3.0 * s,
                c3(180, 176, 168),
            );
            c.line(
                (cx + m * 0.12, cy - m * 0.12),
                (cx + m * 0.28, cy - m * 0.12),
                2.4 * s,
                c3(180, 176, 168),
            );
            c.disc(cx, cy, m * 0.12 + ow * 0.6, INK);
            c.disc(cx, cy, m * 0.12, c3(214, 210, 200));
            c.disc(cx - 2.0 * s, cy - 2.0 * s, m * 0.05, c3(236, 234, 226));
        }
        // ---------------- dungeon gameplay
        "spike_trap" => {
            rect(
                c,
                fw * 0.08,
                fh * 0.08,
                fw * 0.92,
                fh * 0.92,
                c3(90, 90, 96),
                ow,
            );
            for i in 0..4 {
                for j in 0..4 {
                    let (x, y) = (fw * (0.2 + i as f32 * 0.2), fh * (0.2 + j as f32 * 0.2));
                    c.disc(x, y, 4.0 * s, c3(40, 40, 44));
                    c.poly(
                        &star(x, y, 4.5 * s, 1.5 * s, 4, PI / 4.0),
                        c3(196, 198, 206),
                        ow * 0.3,
                    );
                    c.disc(x, y, 1.0 * s, c3(250, 250, 255));
                }
            }
        }
        "pressure_plate" => {
            rect(
                c,
                fw * 0.16,
                fh * 0.16,
                fw * 0.84,
                fh * 0.84,
                c3(84, 80, 76),
                ow,
            );
            rect(
                c,
                fw * 0.22,
                fh * 0.22,
                fw * 0.78,
                fh * 0.78,
                c3(128, 124, 116),
                ow * 0.5,
            );
            c.line(
                (fw * 0.24, fh * 0.24),
                (fw * 0.76, fh * 0.24),
                1.4 * s,
                alpha(c3(220, 214, 200), 0.6),
            );
            c.line(
                (fw * 0.24, fh * 0.76),
                (fw * 0.76, fh * 0.76),
                1.4 * s,
                alpha(INK, 0.6),
            );
            c.poly(
                &[
                    (cx, cy - 7.0 * s),
                    (cx + 7.0 * s, cy),
                    (cx, cy + 7.0 * s),
                    (cx - 7.0 * s, cy),
                ],
                alpha(c3(90, 86, 80), 0.9),
                ow * 0.4,
            );
            c.disc(cx, cy, 1.8 * s, c3(150, 146, 136));
        }
        "pit_trap" => {
            for i in 0..18 {
                let t = i as f32 / 18.0;
                c.poly(
                    &ell_pts(
                        cx,
                        cy,
                        m * (0.44 - t * 0.38),
                        m * (0.44 - t * 0.38),
                        0.0,
                        24,
                    ),
                    mix(c3(70, 60, 54), c3(6, 4, 4), t),
                    0.0,
                );
            }
            for (a, b) in [((0.1, 0.2), (0.4, 0.3)), ((0.7, 0.75), (0.95, 0.6))] {
                c.line(
                    (fw * a.0, fh * a.1),
                    (fw * b.0, fh * b.1),
                    5.0 * s + ow,
                    INK,
                );
                c.line(
                    (fw * a.0, fh * a.1),
                    (fw * b.0, fh * b.1),
                    5.0 * s,
                    c3(130, 94, 58),
                );
            }
        }
        "switch" => {
            rect(
                c,
                fw * 0.22,
                fh * 0.22,
                fw * 0.78,
                fh * 0.78,
                c3(140, 130, 110),
                ow,
            );
            rect(
                c,
                fw * 0.28,
                fh * 0.28,
                fw * 0.72,
                fh * 0.72,
                c3(110, 100, 86),
                ow * 0.4,
            );
            c.disc(cx, cy, m * 0.13 + ow, INK);
            c.disc(cx, cy, m * 0.13, c3(170, 40, 40));
            c.disc(cx - 2.0 * s, cy - 2.0 * s, m * 0.05, c3(240, 140, 130));
            for (x, y) in [(0.28, 0.28), (0.72, 0.28), (0.28, 0.72), (0.72, 0.72)] {
                c.disc(fw * x, fh * y, 1.6 * s, gold);
            }
        }
        "magic_altar" => {
            c.blob(cx, cy, fw * 0.5, fh * 0.6, alpha(c3(160, 110, 255), 0.4));
            rect(
                c,
                fw * 0.06,
                fh * 0.16,
                fw * 0.94,
                fh * 0.84,
                c3(80, 70, 96),
                ow,
            );
            rect(
                c,
                fw * 0.1,
                fh * 0.22,
                fw * 0.9,
                fh * 0.78,
                c3(110, 100, 130),
                ow * 0.4,
            );
            for i in 0..8 {
                let x = fw * (0.16 + i as f32 * 0.1);
                c.poly(
                    &star(x, fh * 0.3, 2.4 * s, 1.0 * s, 3, i as f32),
                    c3(200, 170, 255),
                    0.0,
                );
                c.poly(
                    &star(x, fh * 0.7, 2.4 * s, 1.0 * s, 4, i as f32),
                    c3(200, 170, 255),
                    0.0,
                );
            }
            c.poly(
                &[
                    (cx, fh * 0.26),
                    (cx + 6.0 * s, cy),
                    (cx, fh * 0.74),
                    (cx - 6.0 * s, cy),
                ],
                c3(180, 140, 255),
                ow * 0.6,
            );
            c.disc(cx - 1.5 * s, cy - 4.0 * s, 1.8 * s, c3(250, 240, 255));
        }
        "ritual_symbol" => {
            let red = alpha(c3(160, 24, 32), 0.85);
            let r = m * 0.42;
            c.poly(&ell_pts(cx, cy, r, r, 0.0, 40), alpha(red, 0.0), 2.5 * s);
            for i in 0..40 {
                let a = i as f32 / 40.0 * TAU;
                c.disc(cx + a.cos() * r, cy + a.sin() * r, 1.4 * s, red);
            }
            let p: Vec<Pt> = (0..5)
                .map(|i| {
                    let a = -PI / 2.0 + i as f32 / 5.0 * TAU;
                    (cx + a.cos() * r * 0.92, cy + a.sin() * r * 0.92)
                })
                .collect();
            for i in 0..5 {
                c.line(p[i], p[(i + 2) % 5], 2.0 * s, red);
            }
            for (i, q) in p.iter().enumerate() {
                c.poly(&star(q.0, q.1, 3.0 * s, 1.0 * s, 3 + i % 2, 0.0), red, 0.0);
            }
            c.disc(cx, cy, 2.5 * s, red);
        }
        "magic_crystal" => {
            c.blob(cx, cy, m * 0.48, m * 0.48, alpha(c3(120, 230, 255), 0.45));
            for (dx, h, w, ang) in [
                (-0.16, 0.3, 0.08, -0.35),
                (0.16, 0.28, 0.07, 0.4),
                (0.0, 0.42, 0.1, 0.0),
            ] {
                let (bx, by) = (cx + dx * m, cy + m * 0.26);
                let (sa, ca) = (ang as f32).sin_cos();
                let pt = |x: f32, y: f32| (bx + x * ca - y * sa, by + x * sa + y * ca);
                let (h, w) = (h * m, w * m);
                c.poly(
                    &[
                        pt(-w, 0.0),
                        pt(-w, -h * 0.7),
                        pt(0.0, -h),
                        pt(w, -h * 0.7),
                        pt(w, 0.0),
                    ],
                    c3(120, 220, 240),
                    ow,
                );
                c.poly(
                    &[pt(0.0, 0.0), pt(0.0, -h), pt(w, -h * 0.7), pt(w, 0.0)],
                    c3(180, 245, 255),
                    0.0,
                );
            }
            for i in 0..5u32 {
                c.disc(
                    cx + (hash(i, 28, 1) - 0.5) * m * 0.8,
                    cy + (hash(i, 28, 2) - 0.5) * m * 0.8,
                    1.3 * s,
                    c3(255, 255, 255),
                );
            }
        }
        "portal" => {
            for i in 0..10u32 {
                let a = i as f32 / 10.0 * TAU;
                rock(
                    c,
                    cx + a.cos() * m * 0.43,
                    cy + a.sin() * m * 0.43,
                    m * 0.06,
                    c3(90, 86, 96),
                    i,
                    ow * 0.6,
                );
                c.disc(
                    cx + a.cos() * m * 0.43,
                    cy + a.sin() * m * 0.43,
                    1.6 * s,
                    c3(200, 160, 255),
                );
            }
            for i in 0..24 {
                let t = i as f32 / 24.0;
                c.poly(
                    &ell_pts(
                        cx,
                        cy,
                        m * (0.36 - t * 0.33),
                        m * (0.36 - t * 0.33),
                        0.0,
                        28,
                    ),
                    mix(c3(60, 20, 110), c3(220, 200, 255), t),
                    0.0,
                );
            }
            for arm in 0..4 {
                let mut prev = (cx, cy);
                for k in 1..20 {
                    let t = k as f32 / 20.0;
                    let a = arm as f32 * PI / 2.0 + t * 4.0;
                    let p = (cx + a.cos() * m * 0.34 * t, cy + a.sin() * m * 0.34 * t);
                    c.line(
                        prev,
                        p,
                        2.0 * s * (1.0 - t * 0.5),
                        alpha(c3(250, 240, 255), 0.7),
                    );
                    prev = p;
                }
            }
        }
        "puzzle_mechanism" => {
            c.poly(
                &star(cx, cy, m * 0.46, m * 0.39, 16, 0.0),
                c3(150, 130, 90),
                ow,
            );
            ell(c, cx, cy, m * 0.33, m * 0.33, c3(120, 114, 104), ow * 0.6);
            ell(c, cx, cy, m * 0.2, m * 0.2, c3(150, 144, 132), ow * 0.6);
            for i in 0..6 {
                let a = i as f32 / 6.0 * TAU;
                let (x, y) = (cx + a.cos() * m * 0.27, cy + a.sin() * m * 0.27);
                c.poly(
                    &star(x, y, 3.0 * s, 1.2 * s, 3 + i % 3, a),
                    c3(230, 200, 110),
                    0.0,
                );
            }
            c.line((cx, cy), (cx + m * 0.17, cy - m * 0.06), 2.4 * s, gold);
            c.disc(cx, cy, 3.0 * s, gold);
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(id: &str) -> Vec<f32> {
        let mut c = Canvas::new(64, 64);
        assert!(draw(&mut c, id, 64.0, 64.0), "{id} has no art");
        c.px.iter()
            .flat_map(|p| [p[0] * p[3], p[1] * p[3], p[2] * p[3], p[3]])
            .collect()
    }

    /// `DND55_ART_SHEET=/path/sheet.rgba cargo test dump_contact_sheet -- --ignored` writes every
    /// object as raw RGBA (10 per row, 64 px cells) for a visual check.
    #[test]
    #[ignore]
    fn dump_contact_sheet() {
        let Ok(path) = std::env::var("DND55_ART_SHEET") else {
            return;
        };
        let cols = 10usize;
        let rows = IDS.len().div_ceil(cols);
        let (w, h) = (cols * 64, rows * 64);
        let mut out = vec![0u8; w * h * 4];
        for (k, id) in IDS.iter().enumerate() {
            let img = render(id);
            let (ox, oy) = (k % cols * 64, k / cols * 64);
            for y in 0..64 {
                for x in 0..64 {
                    let p = &img[(y * 64 + x) * 4..][..4];
                    let i = ((oy + y) * w + ox + x) * 4;
                    let bg = if (x / 8 + y / 8) % 2 == 0 { 0.36 } else { 0.42 };
                    for ch in 0..3 {
                        out[i + ch] = ((p[ch] + bg * (1.0 - p[3])).clamp(0.0, 1.0) * 255.0) as u8;
                    }
                    out[i + 3] = 255;
                }
            }
        }
        std::fs::write(&path, out).unwrap();
        std::fs::write(format!("{path}.size"), format!("{w} {h}")).unwrap();
    }

    /// No two new objects may share the same (or a near-identical) drawing.
    #[test]
    fn every_new_object_has_distinct_art() {
        let imgs: Vec<(&str, Vec<f32>)> = IDS.iter().map(|id| (*id, render(id))).collect();
        for (id, img) in &imgs {
            let cover = img.chunks(4).filter(|p| p[3] > 0.05).count();
            assert!(cover > 60, "{id} draws almost nothing ({cover} px)");
        }
        for i in 0..imgs.len() {
            for j in i + 1..imgs.len() {
                let d: f32 = imgs[i]
                    .1
                    .iter()
                    .zip(&imgs[j].1)
                    .map(|(a, b)| (a - b).abs())
                    .sum::<f32>()
                    / (64.0 * 64.0);
                assert!(
                    d > 0.08,
                    "{} and {} look the same (difference {d})",
                    imgs[i].0,
                    imgs[j].0
                );
            }
        }
    }
}
