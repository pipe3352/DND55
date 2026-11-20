//! Native pixel-art terrain tiles (32×32, seamless: every feature wraps across the tile edges).
//! Light from the top-left, 3–5 tones per material, the same rules as the object sprites.

use crate::pixel::*;
use bevy::image::Image;

const N: i32 = 32;

fn h(x: i32, y: i32, s: u32) -> u32 {
    let mut v = (x.rem_euclid(N) as u32).wrapping_mul(374_761_393)
        ^ (y.rem_euclid(N) as u32).wrapping_mul(668_265_263)
        ^ s.wrapping_mul(2_246_822_519);
    v = (v ^ (v >> 13)).wrapping_mul(1_274_126_177);
    v ^ (v >> 16)
}
fn hf(x: i32, y: i32, s: u32) -> f32 {
    (h(x, y, s) & 0xffff) as f32 / 65535.0
}
/// Tileable value noise (lattice `cell` must divide 32), 0..1.
fn noise(x: i32, y: i32, cell: i32, s: u32) -> f32 {
    let (gx, gy) = (x.div_euclid(cell), y.div_euclid(cell));
    let (fx, fy) = (
        (x.rem_euclid(cell)) as f32 / cell as f32,
        (y.rem_euclid(cell)) as f32 / cell as f32,
    );
    let g = |a: i32, b: i32| hf((a * cell).rem_euclid(N), (b * cell).rem_euclid(N), s);
    let sm = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v) = (sm(fx), sm(fy));
    let top = g(gx, gy) * (1.0 - u) + g(gx + 1, gy) * u;
    let bot = g(gx, gy + 1) * (1.0 - u) + g(gx + 1, gy + 1) * u;
    top * (1.0 - v) + bot * v
}
fn fbm(x: i32, y: i32, s: u32) -> f32 {
    noise(x, y, 16, s) * 0.5 + noise(x, y, 8, s + 1) * 0.3 + noise(x, y, 4, s + 2) * 0.2
}
/// Picks one of the ramp tones by value (with 2×2 ordered dither between bands).
fn tone(tones: &[C], v: f32, x: i32, y: i32) -> C {
    let d = [[0.0, 0.5], [0.75, 0.25]][(y & 1) as usize][(x & 1) as usize] - 0.375;
    let i = ((v + d * 0.18).clamp(0.0, 0.999) * tones.len() as f32) as usize;
    tones[i]
}
fn wset(p: &mut Pix, x: i32, y: i32, col: C) {
    p.set(x.rem_euclid(N), y.rem_euclid(N), col);
}
fn wline(p: &mut Pix, x0: i32, y0: i32, x1: i32, y1: i32, col: C) {
    let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for i in 0..=n {
        wset(p, x0 + (x1 - x0) * i / n, y0 + (y1 - y0) * i / n, col);
    }
}
/// Flagstones of size `s`, rows offset by `off`; each stone gets its own shade, bevel and mortar.
fn flags(p: &mut Pix, s: i32, off: i32, tones: &[C], mortar: C, seed: u32) {
    for y in 0..N {
        for x in 0..N {
            let row = y.div_euclid(s);
            let sx = (x + row * off).rem_euclid(N);
            let (cx, cy) = (sx.div_euclid(s), row);
            let (lx, ly) = (sx.rem_euclid(s), y.rem_euclid(s));
            let base = hf(cx * 7, cy * 13, seed) * 0.5 + fbm(x, y, seed + 9) * 0.5;
            let mut col = tone(tones, base, x, y);
            if lx == 0 || ly == 0 {
                col = mortar;
            } else if lx == 1 || ly == 1 {
                col = tones
                    [(tones.len() - 1).min(tones.iter().position(|t| *t == col).unwrap_or(0) + 1)];
            } else if lx == s - 1 || ly == s - 1 {
                col = tones[tones
                    .iter()
                    .position(|t| *t == col)
                    .unwrap_or(1)
                    .saturating_sub(1)];
            }
            p.set(x, y, col);
        }
    }
}
fn cracks(p: &mut Pix, n: u32, col: C, seed: u32) {
    for i in 0..n {
        let (mut x, mut y) = (
            (h(i as i32, 1, seed) % 32) as i32,
            (h(i as i32, 2, seed) % 32) as i32,
        );
        for k in 0..6 {
            wset(p, x, y, col);
            x += [1, 1, 0, -1][(h(k, i as i32, seed) % 4) as usize];
            y += [0, 1, 1, 1][(h(k + 3, i as i32, seed) % 4) as usize];
        }
    }
}

/// Ids drawn natively here.
#[cfg_attr(not(test), allow(dead_code))]
pub const IDS: &[&str] = &[
    "forest_floor",
    "moss",
    "cobblestone",
    "marble",
    "obsidian",
    "dungeon_floor",
    "ruined_floor",
    "swamp",
    "ash",
    "corrupted_ground",
    "magic_ground",
];

pub fn tile(id: &str, variant: u32) -> Option<Pix> {
    let mut p = Pix::new(N, N);
    let s = id
        .bytes()
        .fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32))
        .wrapping_add(variant * 977);
    match id {
        "forest_floor" => {
            let t = [c(50, 44, 30), c(66, 58, 36), c(82, 70, 42), c(94, 84, 50)];
            for y in 0..N {
                for x in 0..N {
                    p.set(x, y, tone(&t, fbm(x, y, s), x, y));
                }
            }
            for i in 0..14 {
                let (x, y) = ((h(i, 0, s) % 32) as i32, (h(i, 1, s) % 32) as i32);
                let col = [
                    c(178, 96, 40),
                    c(150, 70, 34),
                    c(196, 150, 60),
                    c(100, 120, 50),
                ][(i % 4) as usize];
                wset(&mut p, x, y, col);
                wset(&mut p, x + 1, y, col);
                wset(
                    &mut p,
                    x,
                    y + 1,
                    [col[0] / 2 + 20, col[1] / 2 + 10, col[2] / 2, 255],
                );
            }
            for i in 0..3 {
                let (x, y) = ((h(i, 5, s) % 32) as i32, (h(i, 6, s) % 32) as i32);
                wline(&mut p, x, y, x + 5, y + 2, c(110, 80, 50));
            }
            for y in 0..N {
                for x in 0..N {
                    if noise(x, y, 8, s + 4) > 0.72 {
                        p.set(
                            x,
                            y,
                            tone(&[c(56, 86, 40), c(74, 108, 48)], hf(x, y, s), x, y),
                        );
                    }
                }
            }
        }
        "moss" => {
            let t = [
                c(44, 70, 34),
                c(58, 90, 40),
                c(76, 112, 48),
                c(98, 134, 58),
                c(126, 158, 74),
            ];
            for y in 0..N {
                for x in 0..N {
                    p.set(
                        x,
                        y,
                        tone(&t, fbm(x, y, s) * 0.8 + noise(x, y, 4, s + 3) * 0.3, x, y),
                    );
                }
            }
            for i in 0..18 {
                wset(
                    &mut p,
                    (h(i, 0, s) % 32) as i32,
                    (h(i, 1, s) % 32) as i32,
                    c(170, 196, 104),
                );
            }
        }
        "cobblestone" => {
            let t = [
                c(92, 88, 84),
                c(112, 108, 102),
                c(132, 128, 120),
                c(150, 146, 138),
            ];
            for y in 0..N {
                for x in 0..N {
                    p.set(x, y, c(52, 48, 46));
                }
            }
            for row in 0..5 {
                for k in 0..5 {
                    let cx = k * 7 + (row % 2) * 3 + (h(k, row, s) % 2) as i32;
                    let cy = row * 7 - 1 + (h(row, k, s) % 2) as i32;
                    let (rx, ry) = (3.2 + hf(k, row, s + 1) * 0.6, 2.6 + hf(row, k, s + 2) * 0.5);
                    let shade = hf(k, row, s + 3);
                    for dy in -4..=4 {
                        for dx in -4..=4 {
                            let (fx, fy) = (dx as f32 / rx, dy as f32 / ry);
                            let d = fx * fx + fy * fy;
                            if d <= 1.0 {
                                let lit = (-fx - fy) * 0.35 + shade * 0.5 + 0.25;
                                wset(&mut p, cx + dx, cy + dy, tone(&t, lit, cx + dx, cy + dy));
                            }
                        }
                    }
                }
            }
        }
        "marble" => {
            let t = [
                c(200, 198, 196),
                c(216, 214, 210),
                c(230, 228, 224),
                c(242, 240, 236),
            ];
            flags(&mut p, 16, 0, &t, c(160, 156, 152), s);
            for i in 0..3 {
                let mut x = (h(i, 0, s) % 32) as i32;
                for y in 0..N {
                    x += (noise(x, y, 8, s + i as u32) * 3.0) as i32 - 1;
                    wset(&mut p, x, y, c(150, 150, 160));
                    if y % 3 == 0 {
                        wset(&mut p, x + 1, y, c(176, 176, 186));
                    }
                }
            }
        }
        "obsidian" => {
            let t = [c(18, 14, 24), c(30, 24, 40), c(44, 36, 58), c(62, 52, 82)];
            flags(&mut p, 16, 8, &t, c(8, 6, 12), s);
            for k in 0..2 {
                let (x, y) = (2 + k * 16 + (h(k, 0, s) % 6) as i32, 3 + k * 16);
                wline(&mut p, x, y + 6, x + 6, y, c(140, 120, 190));
                wline(&mut p, x + 1, y + 6, x + 6, y + 1, c(90, 76, 130));
            }
        }
        "dungeon_floor" | "ruined_floor" => {
            let t = [c(62, 60, 60), c(76, 74, 72), c(90, 88, 84), c(104, 100, 96)];
            flags(&mut p, 8, 4, &t, c(34, 32, 32), s);
            cracks(&mut p, 2, c(40, 38, 38), s);
            for y in 0..N {
                for x in 0..N {
                    if p.get(x, y) == c(34, 32, 32) && noise(x, y, 8, s + 5) > 0.66 {
                        p.set(x, y, c(56, 80, 44));
                    }
                }
            }
            if id == "ruined_floor" {
                // Missing flagstones show dirt; loose debris.
                for y in 0..N {
                    for x in 0..N {
                        if noise(x, y, 16, s + 7) > 0.62 {
                            p.set(
                                x,
                                y,
                                tone(
                                    &[c(78, 60, 42), c(96, 74, 50), c(110, 88, 60)],
                                    hf(x, y, s),
                                    x,
                                    y,
                                ),
                            );
                        }
                    }
                }
                for i in 0..6 {
                    let (x, y) = ((h(i, 0, s + 2) % 32) as i32, (h(i, 1, s + 2) % 32) as i32);
                    wset(&mut p, x, y, c(120, 116, 110));
                    wset(&mut p, x + 1, y, c(90, 86, 82));
                }
            }
        }
        "swamp" => {
            let mud = [c(48, 52, 30), c(60, 64, 34), c(74, 76, 40)];
            let water = [c(34, 58, 52), c(44, 72, 62), c(60, 92, 76)];
            for y in 0..N {
                for x in 0..N {
                    let wv = noise(x, y, 16, s) * 0.7 + noise(x, y, 8, s + 1) * 0.3;
                    p.set(
                        x,
                        y,
                        if wv > 0.52 {
                            tone(&water, hf(x, y, s + 2) * 0.6 + (wv - 0.52) * 2.0, x, y)
                        } else {
                            tone(&mud, fbm(x, y, s + 3), x, y)
                        },
                    );
                }
            }
            for i in 0..8 {
                let (x, y) = ((h(i, 0, s) % 32) as i32, (h(i, 1, s) % 32) as i32);
                wline(&mut p, x, y, x, y - 3, c(110, 130, 60));
            }
            for i in 0..4 {
                wset(
                    &mut p,
                    (h(i, 4, s) % 32) as i32,
                    (h(i, 5, s) % 32) as i32,
                    c(170, 200, 150),
                );
            }
        }
        "ash" => {
            let t = [
                c(70, 68, 68),
                c(88, 86, 84),
                c(106, 104, 100),
                c(124, 120, 116),
            ];
            for y in 0..N {
                for x in 0..N {
                    p.set(x, y, tone(&t, fbm(x, y, s), x, y));
                }
            }
            for i in 0..20 {
                wset(
                    &mut p,
                    (h(i, 0, s) % 32) as i32,
                    (h(i, 1, s) % 32) as i32,
                    c(44, 40, 40),
                );
            }
            for i in 0..3 {
                wset(
                    &mut p,
                    (h(i, 2, s) % 32) as i32,
                    (h(i, 3, s) % 32) as i32,
                    c(250, 140, 60),
                );
            }
        }
        "corrupted_ground" => {
            let t = [c(30, 18, 34), c(44, 26, 50), c(60, 34, 66), c(76, 44, 82)];
            for y in 0..N {
                for x in 0..N {
                    p.set(x, y, tone(&t, fbm(x, y, s), x, y));
                }
            }
            for i in 0..3 {
                let (mut x, mut y) = ((h(i, 0, s) % 32) as i32, (h(i, 1, s) % 32) as i32);
                for k in 0..14 {
                    wset(&mut p, x, y, c(200, 70, 210));
                    if k % 3 == 0 {
                        wset(&mut p, x + 1, y, c(120, 40, 140));
                    }
                    x += [1, 1, 0, -1, 1][(h(k, i, s) % 5) as usize];
                    y += [0, 1, 1, 1, -1][(h(k + 9, i, s) % 5) as usize];
                }
            }
        }
        "magic_ground" => {
            let t = [c(26, 34, 64), c(34, 44, 80), c(44, 56, 96), c(56, 70, 114)];
            flags(&mut p, 16, 0, &t, c(16, 20, 40), s);
            let glow = c(120, 220, 250);
            for (ox, oy) in [(8, 8), (24, 24)] {
                for i in 0..16 {
                    let a = i as f32 / 16.0 * std::f32::consts::TAU;
                    wset(
                        &mut p,
                        ox + (a.cos() * 4.0).round() as i32,
                        oy + (a.sin() * 4.0).round() as i32,
                        glow,
                    );
                }
                wline(&mut p, ox - 2, oy, ox + 2, oy, c(80, 160, 210));
                wline(&mut p, ox, oy - 2, ox, oy + 2, c(80, 160, 210));
            }
        }
        _ => return None,
    }
    Some(p)
}

/// 32 px tile image (native pixel tile, or the pixelized procedural material).
pub fn tile_image(id: &str, variant: u32, fallback: [u8; 3]) -> Image {
    match tile(id, variant) {
        Some(p) => p.into_image(),
        None => pixelize(&crate::terrain_art::tile(id, variant, fallback), false),
    }
}

/// 64 px base image for the edge-transition generator (native tiles are scaled up 2×).
pub fn transition_base(id: &str, fallback: [u8; 3]) -> Image {
    match tile(id, 0) {
        Some(p) => {
            let mut big = Pix::new(N * 2, N * 2);
            for y in 0..N * 2 {
                for x in 0..N * 2 {
                    big.set(x, y, p.get(x / 2, y / 2));
                }
            }
            crate::textures::new_image((N * 2) as u32, (N * 2) as u32, big.rgba())
        }
        None => crate::terrain_art::tile(id, 0, fallback),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_tiles_are_seamless_opaque_and_distinct() {
        let imgs: Vec<Pix> = IDS.iter().map(|id| tile(id, 0).unwrap()).collect();
        for (id, p) in IDS.iter().zip(&imgs) {
            assert!(p.px.iter().all(|c| c[3] == 255), "{id} has holes");
            // Seamless: the outermost rows/columns continue each other (no hard seam band).
            let diff = |a: C, b: C| {
                (0..3)
                    .map(|i| (a[i] as i32 - b[i] as i32).abs())
                    .sum::<i32>()
            };
            let edge: i32 = (0..N)
                .map(|i| diff(p.get(0, i), p.get(N - 1, i)) + diff(p.get(i, 0), p.get(i, N - 1)))
                .sum();
            let inner: i32 = (0..N)
                .map(|i| diff(p.get(15, i), p.get(16, i)) + diff(p.get(i, 15), p.get(i, 16)))
                .sum();
            assert!(
                edge <= inner * 3 + 400,
                "{id}: seam {edge} vs interior {inner}"
            );
        }
        for i in 0..imgs.len() {
            for j in i + 1..imgs.len() {
                let same = imgs[i]
                    .px
                    .iter()
                    .zip(&imgs[j].px)
                    .filter(|(a, b)| a == b)
                    .count();
                assert!(same < 500, "{} and {} look alike", IDS[i], IDS[j]);
            }
        }
    }
}
