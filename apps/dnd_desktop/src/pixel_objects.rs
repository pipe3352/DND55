//! Hand-built pixel-art map objects, 32 px per cell, 3/4 view (top face + front face), light from
//! the top-left, 1 px ink outline and a ground shadow — the same rules as `sprites.rs`.
//! Flat floor objects (carpets, traps, circles) are drawn top-down without outline.
//! [`image`] falls back to a pixelized version of the procedural art for any id not drawn here.

use crate::pixel::*;
use bevy::image::Image;

fn h(x: i32, y: i32, s: u32) -> u32 {
    let mut v = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ s.wrapping_mul(2_246_822_519);
    v = (v ^ (v >> 13)).wrapping_mul(1_274_126_177);
    v ^ (v >> 16)
}
/// Deterministic speckle (grain, moss, gravel).
fn speck(p: &mut Pix, x: i32, y: i32, w: i32, hh: i32, col: C, seed: u32, per: u32) {
    for yy in y..y + hh {
        for xx in x..x + w {
            if h(xx, yy, seed).is_multiple_of(per) && p.get(xx, yy)[3] > 0 {
                p.set(xx, yy, col);
            }
        }
    }
}
/// Plank surface: shaded rectangle with board seams and grain.
fn planks(p: &mut Pix, x: i32, y: i32, w: i32, hh: i32, r: R, horizontal: bool, seed: u32) {
    p.srect(x, y, w, hh, r);
    if horizontal {
        let mut yy = y + 4;
        while yy < y + hh - 1 {
            p.rect(x + 1, yy, w - 2, 1, r.d);
            yy += 4;
        }
    } else {
        let mut xx = x + 5;
        while xx < x + w - 1 {
            p.rect(xx, y + 1, 1, hh - 2, r.d);
            xx += 5;
        }
    }
    speck(p, x + 1, y + 1, w - 2, hh - 2, r.d, seed, 11);
    speck(p, x + 1, y + 1, w - 2, hh - 2, r.l, seed + 7, 17);
}
/// Front (vertical) face below a top: darker, with a highlight line under the edge.
fn front(p: &mut Pix, x: i32, y: i32, w: i32, hh: i32, r: R) {
    p.rect(x, y, w, hh, r.d);
    p.rect(x, y, w, 1, r.m);
    p.rect(
        x,
        y + hh - 1,
        w,
        1,
        [r.d[0] / 2 + 10, r.d[1] / 2 + 8, r.d[2] / 2 + 10, 255],
    );
}
fn leg(p: &mut Pix, x: i32, y: i32, hh: i32, r: R) {
    p.rect(x, y, 2, hh, r.d);
    p.set(x, y, r.m);
}
fn band(p: &mut Pix, x: i32, y: i32, w: i32, hh: i32) {
    p.rect(x, y, w, hh, IRON.m);
    p.rect(x, y, w, 1, IRON.l);
    if w > hh {
        for xx in (x + 1..x + w - 1).step_by(4) {
            p.set(xx, y + hh / 2, STEEL.l);
        }
    } else {
        for yy in (y + 1..y + hh - 1).step_by(4) {
            p.set(x + w / 2, yy, STEEL.l);
        }
    }
}
fn flame(p: &mut Pix, x: i32, y: i32, big: bool) {
    let s = if big { 2 } else { 1 };
    p.rect(x - s, y - s, 2 * s + 1, 2 * s + 1, c(236, 110, 40));
    p.rect(x - s + 1, y - 2 * s, 2 * s - 1, s + 1, c(250, 170, 60));
    p.rect(x, y - 3 * s, 1, s + 1, c(255, 210, 90));
    p.set(x, y, c(255, 244, 200));
    if big {
        p.set(x - 1, y - 1, c(255, 230, 130));
    }
}
fn glow(p: &mut Pix, cx: f32, cy: f32, r: f32, col: C) {
    for y in (cy - r) as i32..=(cy + r) as i32 {
        for x in (cx - r) as i32..=(cx + r) as i32 {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt() / r;
            if d < 1.0 && p.get(x, y)[3] == 0 && (h(x, y, 9) % 4 < ((1.0 - d) * 6.0) as u32) {
                p.set(x, y, [col[0], col[1], col[2], 70]);
            }
        }
    }
}
fn candle(p: &mut Pix, x: i32, base: i32, hh: i32) {
    p.rect(x - 1, base - hh, 3, hh, c(240, 230, 200));
    p.rect(x + 1, base - hh, 1, hh, c(200, 184, 150));
    p.set(x - 1, base - hh + 2, c(250, 246, 230));
    p.set(x, base - hh - 1, INK);
    flame(p, x, base - hh - 3, false);
}
fn bottle(p: &mut Pix, x: i32, base: i32, col: R, tall: bool, label: bool) {
    let bh = if tall { 9 } else { 6 };
    p.srect(x, base - bh, 5, bh, col);
    p.rect(x + 1, base - bh - 3, 3, 3, col.m);
    p.rect(x + 1, base - bh - 4, 3, 1, c(150, 110, 70));
    p.rect(x + 1, base - bh + 1, 1, bh - 3, col.l);
    if label {
        p.rect(x + 1, base - bh / 2 - 1, 3, 2, c(226, 214, 180));
    }
}
fn mug(p: &mut Pix, x: i32, y: i32) {
    p.srect(x, y, 5, 6, ramp(170, 170, 176));
    p.rect(x + 1, y, 3, 1, c(250, 246, 236));
    p.rect(x + 5, y + 2, 1, 3, ramp(170, 170, 176).d);
}
fn plate(p: &mut Pix, x: i32, y: i32, food: C) {
    p.ell(x as f32, y as f32, 4.0, 2.5, c(226, 220, 206));
    p.ell(x as f32, y as f32 - 0.5, 2.2, 1.4, food);
}
fn rock(p: &mut Pix, cx: f32, cy: f32, rx: f32, ry: f32, r: R, seed: u32) {
    p.sell(cx, cy, rx, ry, r);
    p.sell(
        cx - rx * 0.2,
        cy - ry * 0.35,
        rx * 0.65,
        ry * 0.45,
        ramp(r.l[0], r.l[1], r.l[2]),
    );
    speck(
        p,
        (cx - rx) as i32,
        (cy - ry) as i32,
        (rx * 2.0) as i32 + 1,
        (ry * 2.0) as i32 + 1,
        r.d,
        seed,
        9,
    );
    p.line(
        (cx - rx * 0.1) as i32,
        (cy) as i32,
        (cx + rx * 0.4) as i32,
        (cy + ry * 0.5) as i32,
        r.d,
    );
}
/// Tree canopy made of leafy clumps with dithered highlights.
fn canopy(p: &mut Pix, cx: f32, cy: f32, rx: f32, ry: f32, r: R, seed: u32) {
    let n = 7 + (rx as u32) / 3;
    p.sell(cx, cy, rx, ry, ramp(r.d[0], r.d[1], r.d[2]));
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU
            + (h(i as i32, 0, seed) % 100) as f32 / 160.0;
        let (x, y) = (cx + a.cos() * rx * 0.62, cy + a.sin() * ry * 0.62);
        p.sell(x, y, rx * 0.42, ry * 0.42, r);
    }
    p.sell(cx - rx * 0.15, cy - ry * 0.2, rx * 0.55, ry * 0.5, r);
    speck(
        p,
        (cx - rx) as i32,
        (cy - ry) as i32,
        (rx * 1.2) as i32,
        (ry * 1.1) as i32,
        r.l,
        seed,
        5,
    );
    speck(
        p,
        (cx - rx) as i32,
        (cy) as i32,
        (rx * 2.0) as i32,
        (ry) as i32 + 1,
        r.d,
        seed + 3,
        6,
    );
}
fn trunk(p: &mut Pix, x: i32, top: i32, bottom: i32, w: i32, r: R) {
    p.srect(x, top, w, bottom - top, r);
    p.rect(x - 1, bottom - 2, w + 2, 2, r.m);
    speck(p, x, top, w, bottom - top, r.d, 4, 4);
}

fn finish(mut p: Pix, shadow: bool) -> Pix {
    p.outline();
    if shadow {
        let (w, hh) = (p.w as f32, p.h as f32);
        p.ground_shadow(w / 2.0, hh - 2.0, w * 0.42, 2.2);
    }
    p
}

/// Ids drawn here (the rest use the pixelized procedural art).
pub fn has(id: &str) -> bool {
    draw(id, 1, 1).is_some()
}

/// Pixel-art map object at native resolution (32 px per cell), or the pixelized fallback.
pub fn image(id: &str, w: u8, h: u8, fallback: [u8; 3]) -> Image {
    match draw(id, w, h) {
        Some(p) => p.into_image(),
        None => pixelize(&crate::terrain_art::object(id, w, h, fallback), false),
    }
}

#[allow(clippy::too_many_lines)]
pub fn draw(id: &str, w: u8, hc: u8) -> Option<Pix> {
    let (pw, ph) = (32 * w.max(1) as i32, 32 * hc.max(1) as i32);
    let mut p = Pix::new(pw, ph);
    let (cx, cy) = (pw as f32 / 2.0, ph as f32 / 2.0);
    let wood = WOOD;
    let dark = WOOD_D;
    let red = ramp(170, 44, 44);
    let shadow = match id {
        // ------------------------------------------------ tables & seating
        "table" | "small_table" | "tavern_table" | "dining_table" | "desk" => {
            let (x0, x1) = (2, pw - 2);
            let depth = if id == "small_table" { 12 } else { 14 };
            planks(&mut p, x0, 5, x1 - x0, depth, wood, true, 1);
            front(&mut p, x0, 5 + depth, x1 - x0, 4, wood);
            for x in [x0 + 1, x1 - 3] {
                leg(&mut p, x, 5 + depth + 4, ph - depth - 12, wood);
            }
            if pw > 64 {
                leg(&mut p, pw / 2 - 1, 5 + depth + 4, ph - depth - 12, wood);
            }
            match id {
                "tavern_table" => {
                    mug(&mut p, 10, 6);
                    mug(&mut p, 44, 9);
                    plate(&mut p, 30, 12, c(200, 120, 70));
                }
                "dining_table" => {
                    p.rect(x0 + 2, 10, x1 - x0 - 4, 5, c(236, 230, 214));
                    p.rect(x0 + 2, 14, x1 - x0 - 4, 1, c(190, 60, 60));
                    for k in 0..5 {
                        plate(
                            &mut p,
                            12 + k * 18,
                            8,
                            [c(200, 120, 70), c(230, 200, 90), c(170, 60, 60)][k as usize % 3],
                        );
                    }
                    candle(&mut p, pw / 2, 12, 5);
                }
                "desk" => {
                    p.rect(8, 7, 12, 9, c(232, 220, 186));
                    for yy in [9, 11, 13] {
                        p.rect(10, yy, 8, 1, c(150, 130, 110));
                    }
                    p.srect(30, 8, 4, 4, ramp(40, 40, 60));
                    p.line(33, 9, 38, 4, WHITE);
                    p.srect(44, 7, 10, 7, ramp(120, 50, 40));
                    for x in [14, 42] {
                        p.srect(x, 20, 10, 3, dark);
                        p.set(x + 5, 21, GOLD.l);
                    }
                }
                _ => {}
            }
            true
        }
        "round_table" => {
            p.ell(cx, 30.0, 27.0, 18.0, wood.d);
            p.sell(cx, 26.0, 27.0, 18.0, wood);
            for k in [8.0f32, 16.0] {
                for i in 0..48 {
                    let a = i as f32 / 48.0 * std::f32::consts::TAU;
                    p.set(
                        (cx + a.cos() * k * 1.5) as i32,
                        (26.0 + a.sin() * k) as i32,
                        wood.d,
                    );
                }
            }
            speck(&mut p, 6, 9, 52, 34, wood.l, 3, 13);
            p.srect(28, 44, 8, 14, dark);
            p.rect(20, 56, 24, 3, dark.m);
            mug(&mut p, 18, 17);
            plate(&mut p, 38, 26, c(230, 200, 90));
            candle(&mut p, 30, 24, 4);
            true
        }
        "chair" => {
            planks(&mut p, 8, 2, 16, 8, wood, false, 2);
            front(&mut p, 8, 10, 16, 2, wood);
            planks(&mut p, 7, 14, 18, 7, wood, true, 3);
            front(&mut p, 7, 21, 18, 3, wood);
            for x in [7, 23] {
                leg(&mut p, x, 24, 6, wood);
            }
            true
        }
        "bench" => {
            planks(&mut p, 2, 12, pw - 4, 7, wood, true, 4);
            front(&mut p, 2, 19, pw - 4, 3, wood);
            for x in [4, pw - 6] {
                leg(&mut p, x, 22, 7, wood);
            }
            true
        }
        "stool" => {
            for x in [10, 15, 20] {
                leg(&mut p, x, 17, 12, wood);
            }
            p.ell(16.0, 16.0, 9.0, 4.0, red.d);
            p.sell(16.0, 14.0, 9.0, 4.5, red);
            p.rect(12, 12, 4, 1, red.l);
            true
        }
        "throne" => {
            p.srect(6, 1, 20, 18, GOLD);
            p.srect(9, 4, 14, 14, red);
            p.rect(15, 0, 2, 2, GOLD.l);
            p.set(16, 7, c(80, 140, 230));
            p.srect(4, 16, 24, 7, GOLD);
            p.srect(7, 17, 18, 5, red);
            front(&mut p, 4, 23, 24, 5, GOLD);
            for x in [5, 25] {
                p.srect(x - 1, 12, 4, 10, GOLD);
            }
            true
        }
        // ------------------------------------------------ storage
        "barrel" => {
            p.ell(16.0, 26.0, 11.0, 4.0, wood.d);
            p.rect(5, 8, 22, 18, wood.m);
            for x in [5, 26] {
                p.rect(x, 9, 1, 16, wood.d);
            }
            for x in [9, 14, 19, 23] {
                p.rect(x, 9, 1, 17, wood.d);
            }
            p.rect(7, 9, 2, 16, wood.l);
            p.ell(16.0, 8.0, 11.0, 4.0, wood.l);
            p.ell(16.0, 8.0, 9.0, 2.8, wood.m);
            p.rect(10, 8, 12, 1, wood.d);
            band(&mut p, 5, 12, 22, 2);
            band(&mut p, 5, 21, 22, 2);
            true
        }
        "crate" => {
            planks(&mut p, 4, 4, 24, 8, wood, true, 5);
            front(&mut p, 4, 12, 24, 16, wood);
            planks(
                &mut p,
                4,
                12,
                24,
                16,
                ramp(wood.m[0] - 14, wood.m[1] - 10, wood.m[2] - 8),
                true,
                6,
            );
            p.srect(4, 12, 24, 2, wood);
            p.srect(4, 26, 24, 2, wood);
            p.line(6, 25, 25, 14, wood.l);
            p.line(7, 25, 26, 14, wood.l);
            for (x, y) in [(5, 13), (26, 13), (5, 26), (26, 26)] {
                p.set(x, y, IRON.l);
            }
            true
        }
        "chest" | "reinforced_chest" | "locked_chest" => {
            let r = if id == "locked_chest" {
                ramp(150, 100, 40)
            } else {
                wood
            };
            p.sell(16.0, 10.0, 12.0, 6.0, r);
            p.rect(4, 10, 24, 4, r.m);
            planks(&mut p, 4, 14, 24, 14, r, true, 7);
            front(&mut p, 4, 14, 24, 2, r);
            let bands: &[i32] = if id == "chest" {
                &[7, 23]
            } else {
                &[6, 12, 18, 24]
            };
            for &x in bands {
                band(&mut p, x, 5, 2, 23);
            }
            band(&mut p, 4, 13, 24, 2);
            p.srect(14, 15, 5, 6, GOLD);
            p.set(16, 18, INK);
            if id == "locked_chest" {
                p.line(4, 17, 28, 25, IRON.l);
                p.line(4, 25, 28, 17, IRON.l);
                p.srect(13, 18, 7, 7, IRON);
                p.rect(15, 15, 3, 3, CLEAR);
                p.set(16, 21, INK);
            }
            true
        }
        "bookshelf" | "shelf" | "cabinet" | "wardrobe" => {
            let tall = id != "shelf";
            let top = if tall { 0 } else { 8 };
            p.srect(2, top, pw - 4, ph - top - 2, dark);
            p.rect(1, top, pw - 2, 2, dark.l);
            match id {
                "bookshelf" | "shelf" => {
                    let rows: &[i32] = if id == "bookshelf" { &[3, 13] } else { &[11] };
                    for (ri, &ry) in rows.iter().enumerate() {
                        p.rect(4, ry, pw - 8, 9, c(40, 26, 22));
                        p.rect(4, ry + 9, pw - 8, 1, dark.l);
                        let mut x = 5;
                        let mut k = ri as u32 * 7;
                        while x < pw - 7 {
                            let col = [
                                ramp(150, 40, 40),
                                ramp(50, 80, 140),
                                ramp(60, 110, 60),
                                ramp(150, 110, 50),
                                ramp(110, 60, 120),
                            ][(h(x, ry, k) % 5) as usize];
                            if id == "shelf" {
                                if k.is_multiple_of(3) {
                                    bottle(&mut p, x, ry + 9, col, false, true);
                                } else {
                                    p.srect(x, ry + 3, 5, 6, ramp(200, 190, 170));
                                    p.rect(x + 1, ry + 2, 3, 1, col.m);
                                }
                                x += 7;
                            } else {
                                let bh = 6 + (h(x, ry, 3) % 3) as i32;
                                p.srect(x, ry + 9 - bh, 3, bh, col);
                                p.set(x + 1, ry + 9 - bh + 2, GOLD.l);
                                x += 3 + (h(x, ry, 5) % 2) as i32;
                            }
                            k += 1;
                        }
                    }
                    p.rect(2, ph - 4, pw - 4, 2, dark.d);
                }
                _ => {
                    let doors = 2;
                    let dw = (pw - 8) / doors;
                    for d in 0..doors {
                        let x = 4 + d * dw;
                        p.srect(x, 4, dw - 1, ph - 10, wood);
                        p.rect(x + 2, 6, dw - 5, ph - 14, wood.d);
                        p.rect(x + 3, 7, dw - 7, ph - 16, wood.m);
                        speck(&mut p, x + 3, 7, dw - 7, ph - 16, wood.l, 8, 9);
                        p.set(if d == 0 { x + dw - 3 } else { x + 1 }, ph / 2, GOLD.l);
                    }
                    p.rect(3, ph - 4, pw - 6, 2, dark.d);
                }
            }
            true
        }
        "sack" => {
            p.sell(16.0, 21.0, 10.0, 8.0, ramp(186, 156, 104));
            p.sell(16.0, 12.0, 5.0, 4.0, ramp(186, 156, 104));
            p.rect(13, 10, 6, 2, c(110, 70, 40));
            p.rect(14, 6, 4, 4, ramp(186, 156, 104).l);
            for (x, y) in [(10, 18), (20, 22), (14, 25), (22, 17)] {
                p.set(x, y, ramp(186, 156, 104).d);
            }
            p.rect(24, 27, 3, 1, c(230, 200, 120));
            true
        }
        // ------------------------------------------------ beds & floor textiles
        "bed" | "bedroll" => {
            if id == "bed" {
                p.srect(2, 1, 28, 8, dark);
                p.rect(4, 3, 24, 1, dark.l);
                p.srect(3, 8, 26, ph - 12, wood);
                p.srect(6, 10, 20, 9, ramp(236, 232, 222));
                p.rect(8, 15, 16, 1, c(200, 194, 180));
                p.srect(4, 20, 24, ph - 26, red);
                p.rect(4, 20, 24, 3, red.l);
                for y in (26..ph - 8).step_by(6) {
                    p.rect(6, y, 20, 1, red.d);
                }
                p.srect(2, ph - 6, 28, 4, dark);
            } else {
                p.srect(5, 12, 22, 15, ramp(80, 110, 70));
                p.sell(16.0, 10.0, 11.0, 4.0, ramp(100, 130, 86));
                for y in [16, 21] {
                    p.rect(5, y, 22, 1, c(200, 180, 120));
                }
            }
            true
        }
        "carpet" | "rug" => {
            if id == "carpet" {
                p.rect(2, 2, pw - 4, ph - 4, c(140, 34, 40));
                p.rect(5, 5, pw - 10, ph - 10, c(206, 160, 70));
                p.rect(7, 7, pw - 14, ph - 14, c(120, 26, 34));
                for k in 0..4 {
                    let y = 16 + k * 20;
                    p.poly(
                        &[
                            (cx as i32, y - 6),
                            (cx as i32 + 10, y),
                            (cx as i32, y + 6),
                            (cx as i32 - 10, y),
                        ],
                        c(214, 176, 90),
                    );
                    p.rect(cx as i32 - 1, y - 1, 3, 3, c(40, 60, 120));
                }
                for x in (4..pw - 4).step_by(3) {
                    p.set(x, 0, c(220, 200, 150));
                    p.set(x, ph - 1, c(220, 200, 150));
                }
            } else {
                for (k, col) in [
                    c(56, 76, 136),
                    c(206, 170, 90),
                    c(46, 60, 116),
                    c(170, 56, 56),
                    c(220, 200, 150),
                ]
                .iter()
                .enumerate()
                {
                    let r = 29.0 - k as f32 * 5.0;
                    p.ell(cx, cy, r, r * 0.8, *col);
                }
            }
            false
        }
        // ------------------------------------------------ light & small items
        "candle" => {
            p.ell(16.0, 25.0, 6.0, 2.0, GOLD.d);
            p.ell(16.0, 24.0, 6.0, 2.0, GOLD.m);
            candle(&mut p, 16, 23, 9);
            p.outline();
            glow(&mut p, 16.0, 11.0, 7.0, c(255, 200, 100));
            p.ground_shadow(16.0, 27.0, 7.0, 1.5);
            return Some(p);
        }
        "candelabra" => {
            p.rect(15, 14, 2, 13, GOLD.m);
            p.ell(16.0, 27.0, 6.0, 2.0, GOLD.d);
            p.rect(7, 14, 18, 2, GOLD.m);
            p.rect(7, 12, 2, 3, GOLD.m);
            p.rect(23, 12, 2, 3, GOLD.m);
            for x in [8, 16, 24] {
                candle(&mut p, x, 12, 5);
            }
            p.outline();
            glow(&mut p, 16.0, 5.0, 9.0, c(255, 200, 100));
            p.ground_shadow(16.0, 28.5, 8.0, 1.6);
            return Some(p);
        }
        "lantern" => {
            p.line(16, 2, 13, 6, IRON.d);
            p.line(16, 2, 19, 6, IRON.d);
            p.srect(11, 6, 10, 3, IRON);
            p.rect(11, 9, 10, 12, c(250, 210, 110));
            p.rect(13, 11, 6, 8, c(255, 240, 180));
            for x in [11, 15, 20] {
                p.rect(x, 9, 1, 12, IRON.m);
            }
            p.srect(10, 21, 12, 3, IRON);
            p.outline();
            glow(&mut p, 16.0, 15.0, 12.0, c(255, 210, 120));
            p.ground_shadow(16.0, 27.0, 7.0, 1.5);
            return Some(p);
        }
        "torch" => {
            p.srect(13, 4, 6, 4, IRON);
            p.rect(15, 8, 2, 16, wood.m);
            p.rect(15, 8, 1, 16, wood.l);
            band(&mut p, 14, 14, 4, 2);
            flame(&mut p, 16, 2, true);
            p.outline();
            glow(&mut p, 16.0, 2.0, 11.0, c(255, 170, 70));
            return Some(p);
        }
        "brazier" => {
            for (a, b) in [
                ((9, 28), (13, 18)),
                ((23, 28), (19, 18)),
                ((16, 29), (16, 18)),
            ] {
                p.line(a.0, a.1, b.0, b.1, IRON.d);
            }
            p.ell(16.0, 17.0, 10.0, 3.0, IRON.d);
            p.ell(16.0, 15.0, 10.0, 3.5, IRON.m);
            p.ell(16.0, 14.5, 8.0, 2.2, c(90, 30, 20));
            for (x, y) in [(12, 14), (16, 13), (20, 14)] {
                p.set(x, y, c(255, 150, 60));
            }
            flame(&mut p, 13, 11, false);
            flame(&mut p, 16, 9, true);
            flame(&mut p, 19, 11, false);
            p.outline();
            glow(&mut p, 16.0, 9.0, 13.0, c(255, 160, 70));
            p.ground_shadow(16.0, 29.5, 9.0, 1.6);
            return Some(p);
        }
        "fireplace" => {
            p.srect(2, 2, pw - 4, ph - 4, STONE);
            for row in 0..6 {
                let y = 3 + row * 5;
                let off = (row % 2) * 4;
                for x in (2 + off..pw - 2).step_by(8) {
                    p.rect(x, y, 1, 5, STONE.d);
                }
                p.rect(2, y + 4, pw - 4, 1, STONE.d);
            }
            p.rect(1, 2, pw - 2, 3, STONE.l);
            p.ell(cx, 30.0, 18.0, 20.0, c(30, 20, 18));
            p.rect(14, 24, 36, 6, c(30, 20, 18));
            p.line(20, 27, 42, 24, dark.m);
            p.line(22, 24, 44, 27, dark.m);
            flame(&mut p, 26, 22, true);
            flame(&mut p, 32, 20, true);
            flame(&mut p, 38, 22, true);
            p.outline();
            glow(&mut p, 32.0, 20.0, 14.0, c(255, 150, 60));
            return Some(p);
        }
        "cooking_pot" => {
            for (a, b) in [((5, 30), (14, 4)), ((27, 30), (18, 4))] {
                p.line(a.0, a.1, b.0, b.1, dark.m);
            }
            p.line(16, 4, 16, 10, IRON.d);
            p.sell(16.0, 19.0, 10.0, 8.0, ramp(60, 60, 66));
            p.ell(16.0, 13.0, 9.0, 2.5, ramp(60, 60, 66).l);
            p.ell(16.0, 13.0, 7.5, 1.8, c(150, 96, 44));
            p.set(13, 13, c(210, 160, 80));
            p.set(18, 12, c(110, 150, 70));
            p.outline();
            p.ground_shadow(16.0, 29.5, 10.0, 1.6);
            for (x, y) in [(14, 8), (17, 6), (15, 3)] {
                p.set(x, y, [230, 230, 236, 150]);
            }
            return Some(p);
        }
        "bottles" => {
            for (i, col) in [
                ramp(70, 46, 34),
                ramp(60, 110, 60),
                ramp(60, 90, 170),
                ramp(140, 40, 50),
            ]
            .iter()
            .enumerate()
            {
                bottle(
                    &mut p,
                    3 + i as i32 * 7,
                    27 - (i % 2) as i32 * 2,
                    *col,
                    i % 2 == 0,
                    i != 2,
                );
            }
            true
        }
        "plates" => {
            p.srect(4, 12, 24, 12, wood);
            p.rect(4, 24, 24, 2, wood.d);
            p.sell(11.0, 16.0, 4.0, 3.0, ramp(210, 150, 80));
            p.srect(18, 14, 7, 5, ramp(240, 200, 90));
            p.set(20, 16, ramp(240, 200, 90).d);
            p.sell(12.0, 20.0, 3.0, 2.0, ramp(180, 70, 70));
            p.rect(21, 20, 4, 2, c(230, 210, 190));
            true
        }
        "scroll" => {
            p.srect(7, 12, 18, 10, ramp(232, 216, 176));
            for y in [14, 16, 18] {
                p.rect(9, y, 14, 1, c(150, 130, 110));
            }
            for x in [5, 25] {
                p.srect(x, 10, 3, 14, ramp(200, 176, 130));
            }
            p.rect(18, 20, 3, 3, c(170, 30, 30));
            true
        }
        "books" => {
            for (i, col) in [ramp(120, 40, 36), ramp(50, 80, 130), ramp(60, 100, 60)]
                .iter()
                .enumerate()
            {
                let y = 22 - i as i32 * 5;
                let x = 6 + i as i32 * 2;
                p.srect(x, y, 18, 5, *col);
                p.rect(x + 1, y + 1, 15, 3, c(236, 226, 200));
                p.rect(x, y + 1, 1, 3, col.d);
                p.set(x + 8, y, GOLD.l);
            }
            true
        }
        "wall_banner" => {
            p.rect(5, 3, 22, 2, GOLD.m);
            p.poly(&[(8, 5), (24, 5), (24, 26), (16, 21), (8, 26)], red.m);
            p.rect(8, 5, 2, 18, red.l);
            p.rect(22, 5, 2, 18, red.d);
            p.rect(15, 9, 2, 8, GOLD.l);
            p.rect(12, 11, 8, 2, GOLD.l);
            true
        }
        "painting" => {
            p.srect(4, 6, 24, 20, GOLD);
            p.rect(7, 9, 18, 14, c(40, 52, 74));
            p.poly(
                &[(7, 22), (12, 15), (16, 19), (20, 13), (24, 22)],
                c(70, 90, 70),
            );
            p.rect(19, 10, 2, 2, c(240, 230, 170));
            true
        }
        "mirror" => {
            p.sell(16.0, 15.0, 9.0, 12.0, ramp(190, 194, 206));
            p.ell(16.0, 15.0, 7.0, 10.0, c(150, 186, 210));
            p.line(12, 9, 15, 6, WHITE);
            p.line(12, 13, 17, 8, c(220, 236, 246));
            true
        }
        // ------------------------------------------------ nature
        "tree" | "big_tree" | "small_tree" | "birch_tree" | "ancient_tree" => {
            let (r, bark) = match id {
                "birch_tree" => (ramp(130, 176, 70), ramp(226, 224, 214)),
                "ancient_tree" => (ramp(40, 96, 66), ramp(96, 70, 50)),
                "small_tree" => (ramp(100, 156, 64), WOOD),
                _ => (LEAF, WOOD),
            };
            let tw = (pw / 8).max(4);
            trunk(
                &mut p,
                pw / 2 - tw / 2,
                (ph as f32 * 0.5) as i32,
                ph - 3,
                tw,
                bark,
            );
            if id == "birch_tree" {
                for y in (ph / 2 + 2..ph - 4).step_by(3) {
                    p.rect(pw / 2 - tw / 2 + (y % 2), y, 2, 1, c(40, 36, 40));
                }
            }
            if id == "ancient_tree" {
                for d in [-1, 1] {
                    p.line(pw / 2, ph - 4, pw / 2 + d * 30, ph - 2, bark.m);
                    p.line(pw / 2, ph - 6, pw / 2 + d * 20, ph - 1, bark.d);
                }
            }
            let s = if id == "small_tree" { 0.34 } else { 0.46 };
            canopy(
                &mut p,
                cx,
                ph as f32 * 0.38,
                pw as f32 * s,
                ph as f32 * s * 0.85,
                r,
                h(pw, ph, 2),
            );
            true
        }
        "pine_tree" => {
            trunk(&mut p, 14, 22, 30, 4, WOOD);
            let r = ramp(40, 96, 66);
            for (k, (y, half)) in [(4, 4), (9, 7), (14, 10), (19, 13)].iter().enumerate() {
                p.poly(&[(16, y - 5), (16 + half, y + 4), (16 - half, y + 4)], r.m);
                p.rect(16 - half + 1, y + 3, *half, 1, r.d);
                p.line(16, y - 5, 16 - half + 1, y + 3, r.l);
                speck(&mut p, 16 - half, y - 4, half * 2, 8, r.d, k as u32, 5);
            }
            true
        }
        "dead_tree" => {
            let r = ramp(110, 96, 84);
            trunk(&mut p, 14, 10, 30, 4, r);
            for (a, b, c2) in [
                ((15, 14), (6, 6), (4, 3)),
                ((17, 12), (26, 4), (28, 6)),
                ((16, 10), (14, 2), (17, 1)),
                ((17, 18), (24, 14), (27, 15)),
            ] {
                p.line(a.0, a.1, b.0, b.1, r.m);
                p.line(b.0, b.1, c2.0, c2.1, r.d);
            }
            true
        }
        "bush" | "flower_bush" => {
            canopy(&mut p, 16.0, 20.0, 12.0, 9.0, ramp(66, 120, 56), 7);
            if id == "flower_bush" {
                for (i, (x, y)) in [
                    (9, 16),
                    (14, 13),
                    (20, 15),
                    (24, 20),
                    (11, 22),
                    (18, 23),
                    (16, 18),
                ]
                .iter()
                .enumerate()
                {
                    p.set(*x, *y, [c(240, 120, 170), WHITE, c(250, 210, 80)][i % 3]);
                    p.set(
                        x + 1,
                        *y,
                        [c(200, 80, 130), c(220, 220, 220), c(220, 170, 50)][i % 3],
                    );
                }
            }
            true
        }
        "fern" | "tall_grass" | "reeds" | "wildflowers" => {
            let r = if id == "reeds" {
                ramp(130, 140, 70)
            } else {
                ramp(80, 140, 60)
            };
            for i in 0..16 {
                let x = 4 + (h(i, 1, 3) % 24) as i32;
                let top = 6 + (h(i, 2, 3) % 14) as i32;
                let lean = (h(i, 3, 3) % 5) as i32 - 2;
                p.line(
                    x,
                    28,
                    x + lean,
                    top,
                    if i % 3 == 0 {
                        r.l
                    } else if i % 3 == 1 {
                        r.m
                    } else {
                        r.d
                    },
                );
                if id == "reeds" && i % 3 == 0 {
                    p.rect(x + lean - 1, top, 2, 4, c(120, 76, 44));
                }
                if id == "wildflowers" && i % 2 == 0 {
                    p.set(
                        x + lean,
                        top,
                        [c(240, 110, 150), c(120, 120, 240), c(250, 220, 80), WHITE]
                            [(i % 4) as usize],
                    );
                }
            }
            if id == "reeds" {
                p.ell(16.0, 28.0, 12.0, 2.5, c(70, 110, 140));
            }
            id != "reeds"
        }
        "mushrooms" | "glowing_mushrooms" => {
            let glowing = id == "glowing_mushrooms";
            for (x, y, s) in [(10, 24, 5), (20, 21, 6), (16, 27, 3), (25, 27, 3)] {
                p.rect(x - 1, y, 3, 30 - y, c(230, 222, 200));
                let cap = if glowing {
                    ramp(70, 200, 220)
                } else {
                    ramp(190, 50, 40)
                };
                p.sell(x as f32, y as f32, s as f32, s as f32 * 0.6, cap);
                p.rect(x - s + 1, y, 2 * s - 1, 1, cap.d);
                p.set(x - 1, y - 2, WHITE);
                p.set(x + 2, y - 1, WHITE);
            }
            p.outline();
            if glowing {
                glow(&mut p, 16.0, 22.0, 13.0, c(90, 230, 240));
            }
            p.ground_shadow(16.0, 29.5, 11.0, 1.4);
            return Some(p);
        }
        "boulder" | "large_rock" | "mossy_rock" | "stalagmite" | "rock_cluster"
        | "small_stones" => {
            let r = STONE;
            match id {
                "stalagmite" => {
                    p.poly(&[(16, 2), (24, 29), (8, 29)], r.m);
                    p.poly(&[(16, 2), (13, 29), (8, 29)], r.l);
                    p.poly(&[(16, 2), (24, 29), (20, 29)], r.d);
                    speck(&mut p, 8, 4, 16, 25, r.d, 2, 7);
                }
                "rock_cluster" => {
                    for (i, (x, y, rx)) in [
                        (22.0, 40.0, 14.0),
                        (44.0, 44.0, 11.0),
                        (30.0, 52.0, 9.0),
                        (46.0, 24.0, 8.0),
                        (12.0, 54.0, 6.0),
                    ]
                    .iter()
                    .enumerate()
                    {
                        rock(&mut p, *x, *y, *rx, rx * 0.8, r, i as u32);
                    }
                }
                "small_stones" => {
                    for (i, (x, y, rx)) in [
                        (9.0, 20.0, 3.5),
                        (17.0, 24.0, 4.0),
                        (24.0, 18.0, 3.0),
                        (13.0, 27.0, 2.5),
                        (25.0, 26.0, 2.5),
                    ]
                    .iter()
                    .enumerate()
                    {
                        rock(&mut p, *x, *y, *rx, rx * 0.8, r, i as u32);
                    }
                }
                _ => {
                    let rx = if id == "boulder" { 13.0 } else { 11.0 };
                    rock(
                        &mut p,
                        cx,
                        cy + 3.0,
                        rx * pw as f32 / 32.0,
                        rx * 0.8 * ph as f32 / 32.0,
                        r,
                        1,
                    );
                    if id == "mossy_rock" {
                        speck(&mut p, 5, 6, 22, 10, LEAF.m, 5, 2);
                        speck(&mut p, 5, 6, 22, 8, LEAF.l, 6, 4);
                    }
                }
            }
            true
        }
        "crystal" | "magic_crystal" => {
            let r = if id == "crystal" {
                ramp(170, 110, 230)
            } else {
                ramp(110, 220, 240)
            };
            for (x, top, w2) in [(11, 10, 3), (20, 12, 3), (16, 3, 4)] {
                p.poly(
                    &[
                        (x - w2, 28),
                        (x - w2, top + 4),
                        (x, top),
                        (x + w2, top + 4),
                        (x + w2, 28),
                    ],
                    r.m,
                );
                p.rect(x - w2 + 1, top + 4, w2 - 1, 24 - top, r.l);
                p.rect(x + 1, top + 4, w2 - 1, 24 - top, r.d);
            }
            p.outline();
            glow(&mut p, 16.0, 16.0, 14.0, r.l);
            p.ground_shadow(16.0, 29.5, 10.0, 1.5);
            return Some(p);
        }
        "log" | "fallen_tree" | "stump" => {
            if id == "stump" {
                p.srect(8, 14, 16, 13, WOOD);
                speck(&mut p, 8, 14, 16, 13, WOOD.d, 3, 3);
                p.ell(16.0, 14.0, 8.0, 3.5, c(210, 170, 110));
                p.ell(16.0, 14.0, 5.0, 2.0, c(180, 136, 84));
                p.set(16, 14, c(150, 106, 64));
                for d in [-1, 1] {
                    p.line(16 + d * 7, 26, 16 + d * 12, 29, WOOD.m);
                }
            } else {
                p.srect(4, 10, pw - 10, 14, WOOD);
                speck(&mut p, 4, 10, pw - 10, 14, WOOD.d, 5, 3);
                p.rect(4, 11, pw - 10, 2, WOOD.l);
                p.ell(pw as f32 - 6.0, 17.0, 4.0, 7.0, c(210, 170, 110));
                p.ell(pw as f32 - 6.0, 17.0, 2.0, 4.0, c(170, 126, 76));
                if id == "fallen_tree" {
                    canopy(&mut p, 10.0, 16.0, 10.0, 12.0, LEAF, 9);
                }
                speck(&mut p, 12, 10, 20, 3, LEAF.m, 7, 3);
            }
            true
        }
        "fallen_branches" | "roots" | "vines" => {
            let r = if id == "vines" { LEAF } else { WOOD };
            for i in 0..5 {
                let (x0, y0) = (2 + (h(i, 0, 1) % 8) as i32, 6 + i * 5);
                let (x1, y1) = (
                    22 + (h(i, 1, 1) % 8) as i32,
                    4 + i * 5 + (h(i, 2, 1) % 6) as i32,
                );
                p.line(x0, y0, x1, y1, r.m);
                p.line(x0, y0 + 1, x1, y1 + 1, r.d);
                if id == "vines" {
                    for t in (x0..x1).step_by(4) {
                        p.set(t, y0 + (t - x0) * (y1 - y0) / (x1 - x0).max(1) - 1, r.l);
                    }
                }
            }
            id == "fallen_branches"
        }
        "leaf_pile" => {
            for i in 0..140 {
                let (x, y) = (5 + (h(i, 0, 4) % 22) as i32, 12 + (h(i, 1, 4) % 15) as i32);
                if ((x as f32 - 16.0) / 11.0).powi(2) + ((y as f32 - 20.0) / 8.0).powi(2) < 1.0 {
                    p.set(
                        x,
                        y,
                        [
                            c(206, 110, 44),
                            c(170, 60, 30),
                            c(226, 170, 60),
                            c(130, 80, 40),
                        ][(i % 4) as usize],
                    );
                }
            }
            true
        }
        "puddle" => {
            p.ell(16.0, 18.0, 13.0, 8.0, [60, 90, 120, 200]);
            p.ell(15.0, 17.0, 10.0, 6.0, [90, 130, 170, 210]);
            p.rect(10, 15, 5, 1, [220, 236, 250, 220]);
            p.rect(17, 19, 3, 1, [220, 236, 250, 200]);
            false
        }
        "bones" | "skeleton" | "skull_pile" => {
            let skull = |p: &mut Pix, x: i32, y: i32| {
                p.sell(x as f32, y as f32, 4.0, 3.5, BONE);
                p.rect(x - 2, y, 2, 2, INK);
                p.rect(x + 1, y, 2, 2, INK);
                p.rect(x - 1, y + 3, 3, 2, BONE.m);
            };
            match id {
                "skull_pile" => {
                    for (x, y) in [(9, 24), (16, 25), (23, 24), (12, 18), (20, 18), (16, 12)] {
                        skull(&mut p, x, y);
                    }
                }
                "skeleton" => {
                    skull(&mut p, 8, 12);
                    p.line(11, 15, 24, 18, BONE.m);
                    for k in 0..4 {
                        p.line(13 + k * 3, 13, 14 + k * 3, 21, BONE.l);
                    }
                    p.line(24, 18, 29, 12, BONE.m);
                    p.line(24, 18, 29, 25, BONE.m);
                    p.line(12, 16, 6, 24, BONE.m);
                }
                _ => {
                    p.line(6, 22, 24, 16, BONE.l);
                    p.line(8, 14, 22, 25, BONE.m);
                    for (x, y) in [(6, 22), (24, 16), (8, 14), (22, 25)] {
                        p.rect(x - 1, y - 1, 3, 3, BONE.l);
                    }
                    skull(&mut p, 17, 11);
                }
            }
            true
        }
        // ------------------------------------------------ dungeon architecture
        "pillar" | "broken_pillar" => {
            let top = if id == "pillar" { 1 } else { 12 };
            p.srect(5, ph - 6, 22, 4, STONE);
            p.rect(8, top + 3, 16, ph - top - 9, STONE.m);
            p.rect(8, top + 3, 4, ph - top - 9, STONE.l);
            p.rect(19, top + 3, 5, ph - top - 9, STONE.d);
            for x in [13, 17] {
                p.rect(x, top + 4, 1, ph - top - 11, STONE.d);
            }
            if id == "pillar" {
                p.srect(5, top, 22, 4, STONE);
            } else {
                p.poly(
                    &[
                        (8, top + 3),
                        (12, top - 2),
                        (15, top + 2),
                        (19, top - 4),
                        (24, top + 3),
                    ],
                    STONE.m,
                );
                rock(&mut p, 26.0, 27.0, 3.0, 2.5, STONE, 3);
                rock(&mut p, 5.0, 28.0, 2.5, 2.0, STONE, 4);
            }
            true
        }
        "statue" => {
            let mut s = crate::sprites::creature("class:paladin");
            for px in &mut s.px {
                if px[3] > 0 && *px != INK {
                    let g = (px[0] as u16 + px[1] as u16 + px[2] as u16) / 3;
                    let g = (g as f32 * 0.7 + 50.0) as u8;
                    *px = [g, g, (g as u16 + 6).min(255) as u8, px[3]];
                }
            }
            p.srect(4, 24, 24, 7, STONE);
            p.blit(&s, 0, -4);
            true
        }
        "altar" | "magic_altar" => {
            let r = if id == "altar" {
                STONE
            } else {
                ramp(90, 80, 110)
            };
            p.srect(4, 6, pw - 8, 10, r);
            front(&mut p, 4, 16, pw - 8, 12, r);
            for x in (8..pw - 8).step_by(10) {
                p.rect(x, 18, 6, 8, r.m);
            }
            if id == "altar" {
                p.rect(10, 6, pw - 20, 4, c(236, 230, 214));
                p.rect(pw / 2 - 2, 6, 4, 22, c(170, 40, 40));
                candle(&mut p, 10, 9, 5);
                candle(&mut p, pw - 10, 9, 5);
            } else {
                for x in (10..pw - 10).step_by(6) {
                    p.set(x, 21, c(200, 160, 255));
                    p.set(x + 1, 22, c(200, 160, 255));
                }
                p.poly(
                    &[(pw / 2, 0), (pw / 2 + 4, 6), (pw / 2, 12), (pw / 2 - 4, 6)],
                    c(190, 140, 255),
                );
                p.set(pw / 2 - 1, 4, WHITE);
            }
            p.outline();
            if id == "magic_altar" {
                glow(&mut p, pw as f32 / 2.0, 8.0, 18.0, c(180, 130, 255));
            }
            p.ground_shadow(pw as f32 / 2.0, 30.0, pw as f32 * 0.42, 1.8);
            return Some(p);
        }
        "tombstone" => {
            p.poly(
                &[
                    (7, 28),
                    (7, 10),
                    (10, 5),
                    (16, 3),
                    (22, 5),
                    (25, 10),
                    (25, 28),
                ],
                STONE.m,
            );
            p.rect(7, 8, 3, 20, STONE.l);
            p.rect(22, 8, 3, 20, STONE.d);
            p.rect(15, 8, 2, 12, STONE.d);
            p.rect(12, 11, 8, 2, STONE.d);
            speck(&mut p, 8, 6, 16, 20, STONE.d, 4, 9);
            p.rect(5, 27, 22, 3, ramp(80, 120, 60).m);
            speck(&mut p, 5, 26, 22, 3, LEAF.l, 3, 3);
            true
        }
        "coffin" | "sarcophagus" => {
            let r = if id == "coffin" {
                dark
            } else {
                ramp(160, 150, 130)
            };
            p.poly(
                &[(16, 2), (27, 12), (23, ph - 3), (9, ph - 3), (5, 12)],
                r.m,
            );
            p.poly(
                &[(16, 5), (24, 13), (21, ph - 6), (11, ph - 6), (8, 13)],
                r.l,
            );
            if id == "coffin" {
                p.rect(15, 12, 2, 24, GOLD.m);
                p.rect(11, 17, 10, 2, GOLD.m);
            } else {
                p.sell(16.0, 14.0, 4.0, 4.5, r);
                p.srect(11, 19, 10, 30, r);
                p.line(11, 24, 20, 28, r.d);
                p.line(20, 24, 11, 28, r.d);
                p.rect(15, 28, 2, 18, GOLD.m);
            }
            true
        }
        "chains" => {
            for k in 0..8 {
                let (x, y) = (6 + k * 3, 3 + k * 3);
                if k % 2 == 0 {
                    p.rect(x, y, 4, 2, IRON.l);
                    p.set(x + 1, y + 1, CLEAR);
                } else {
                    p.rect(x, y, 2, 4, IRON.m);
                }
            }
            p.ell(28.0, 28.0, 3.0, 3.0, IRON.m);
            p.ell(28.0, 28.0, 1.5, 1.5, CLEAR);
            true
        }
        "hanging_cage" => {
            p.line(16, 0, 16, 4, IRON.d);
            p.ell(16.0, 8.0, 9.0, 3.0, IRON.m);
            for x in [8, 11, 14, 17, 20, 23] {
                p.rect(x, 8, 1, 18, IRON.m);
            }
            p.ell(16.0, 26.0, 9.0, 3.0, IRON.d);
            p.rect(7, 16, 18, 1, IRON.l);
            p.sell(14.0, 22.0, 3.0, 2.5, BONE);
            p.set(13, 22, INK);
            true
        }
        "iron_gate" | "prison_bars" => {
            let r = if id == "iron_gate" {
                IRON
            } else {
                ramp(110, 84, 64)
            };
            p.rect(1, 4, pw - 2, 3, r.m);
            p.rect(1, 22, pw - 2, 2, r.m);
            for x in (3..pw - 2).step_by(4) {
                p.rect(x, 2, 2, 26, r.m);
                p.set(x, 3, r.l);
                if id == "iron_gate" {
                    p.rect(x, 0, 2, 2, GOLD.m);
                }
            }
            if id == "prison_bars" {
                speck(&mut p, 0, 0, 32, 30, c(170, 90, 40), 5, 9);
            }
            true
        }
        "door" | "heavy_door" | "secret_door" => {
            if id == "secret_door" {
                p.srect(0, 2, pw, 27, STONE);
                for row in 0..5 {
                    let y = 2 + row * 6;
                    p.rect(0, y + 5, pw, 1, STONE.d);
                    for x in ((row % 2) * 4..pw).step_by(8) {
                        p.rect(x, y, 1, 6, STONE.d);
                    }
                }
                p.rect(9, 6, 1, 22, c(30, 26, 30));
                p.rect(22, 6, 1, 22, c(30, 26, 30));
                p.set(16, 15, c(150, 200, 230));
            } else {
                p.srect(2, 1, 28, 29, STONE);
                planks(
                    &mut p,
                    5,
                    4,
                    22,
                    26,
                    if id == "heavy_door" { dark } else { wood },
                    false,
                    3,
                );
                for y in [8, 22] {
                    band(&mut p, 5, y, 22, 2);
                }
                p.rect(
                    21,
                    15,
                    3,
                    3,
                    if id == "heavy_door" { IRON.l } else { GOLD.m },
                );
            }
            true
        }
        "stone_stairs" => {
            let n = 8;
            for i in 0..n {
                let y = 2 + i * (ph - 4) / n;
                let t = i as f32 / n as f32;
                let g = (176.0 - t * 120.0) as u8;
                p.rect(4, y, 24, (ph - 4) / n, c(g, g - 4, g - 10));
                p.rect(
                    4,
                    y,
                    24,
                    1,
                    c(
                        g.saturating_add(30),
                        g.saturating_add(26),
                        g.saturating_add(20),
                    ),
                );
            }
            p.srect(1, 1, 3, ph - 2, STONE);
            p.srect(28, 1, 3, ph - 2, STONE);
            false
        }
        "broken_wall" | "rubble_pile" => {
            if id == "broken_wall" {
                for row in 0..4 {
                    let y = 6 + row * 6;
                    let width = 32 - (3 - row) * 6;
                    for x in ((row % 2) * 4..width).step_by(8) {
                        p.srect(x, y, 8, 6, STONE);
                    }
                }
            }
            for i in 0..8 {
                rock(
                    &mut p,
                    6.0 + (h(i, 0, 6) % 22) as f32,
                    18.0 + (h(i, 1, 6) % 10) as f32,
                    2.5 + (h(i, 2, 6) % 3) as f32,
                    2.0 + (h(i, 3, 6) % 2) as f32,
                    STONE,
                    i as u32,
                );
            }
            true
        }
        "stone_arch" => {
            for x in [2, pw - 14] {
                p.srect(x, 4, 12, 26, STONE);
                for y in (8..28).step_by(6) {
                    p.rect(x + 1, y, 10, 1, STONE.d);
                }
            }
            p.srect(2, 0, pw - 4, 7, STONE);
            for x in (6..pw - 6).step_by(8) {
                p.rect(x, 1, 1, 5, STONE.d);
            }
            p.srect(pw / 2 - 4, 0, 8, 9, ramp(150, 146, 140));
            true
        }
        // ------------------------------------------------ traps & magic (floor, top-down)
        "pressure_plate" | "spike_trap" | "pit_trap" | "switch" | "lever" | "puzzle_mechanism" => {
            match id {
                "pressure_plate" => {
                    p.srect(6, 6, 20, 20, STONE);
                    p.rect(9, 9, 14, 14, STONE.d);
                    p.srect(10, 10, 12, 12, STONE);
                    p.set(16, 16, STONE.d);
                    return Some(p);
                }
                "spike_trap" => {
                    p.srect(3, 3, 26, 26, IRON);
                    for y in (7..28).step_by(6) {
                        for x in (7..28).step_by(6) {
                            p.rect(x - 1, y - 1, 3, 3, c(30, 30, 36));
                            p.set(x, y - 1, STEEL.l);
                            p.set(x, y, WHITE);
                        }
                    }
                    return Some(p);
                }
                "pit_trap" => {
                    for k in 0..6 {
                        let r = 13.0 - k as f32 * 2.0;
                        let g = 70 - k * 12;
                        p.ell(
                            16.0,
                            16.0,
                            r,
                            r,
                            c(g as u8, (g - 6).max(0) as u8, (g - 8).max(0) as u8),
                        );
                    }
                    p.line(3, 6, 12, 10, dark.m);
                    p.line(20, 24, 29, 20, dark.m);
                    return Some(p);
                }
                "lever" => {
                    p.srect(9, 20, 14, 8, STONE);
                    p.line(16, 22, 22, 6, dark.m);
                    p.line(17, 22, 23, 6, dark.l);
                    p.srect(21, 3, 5, 5, red);
                }
                "switch" => {
                    p.srect(8, 8, 16, 16, STONE);
                    p.sell(16.0, 16.0, 4.0, 4.0, red);
                    for (x, y) in [(10, 10), (21, 10), (10, 21), (21, 21)] {
                        p.set(x, y, GOLD.l);
                    }
                }
                _ => {
                    p.sell(16.0, 16.0, 13.0, 13.0, ramp(150, 130, 90));
                    p.sell(16.0, 16.0, 9.0, 9.0, STONE);
                    for i in 0..6 {
                        let a = i as f32 / 6.0 * std::f32::consts::TAU;
                        p.rect(
                            (16.0 + a.cos() * 11.0) as i32,
                            (16.0 + a.sin() * 11.0) as i32,
                            2,
                            2,
                            GOLD.l,
                        );
                    }
                    p.line(16, 16, 21, 13, GOLD.m);
                }
            }
            true
        }
        "rune_circle" | "ritual_symbol" | "portal" => {
            let (col, glowc) = match id {
                "ritual_symbol" => (c(170, 30, 40), c(220, 60, 60)),
                "portal" => (c(150, 90, 240), c(190, 140, 255)),
                _ => (c(160, 110, 230), c(190, 150, 255)),
            };
            let r = pw.min(ph) as f32 / 2.0 - 3.0;
            for i in 0..(r * 8.0) as i32 {
                let a = i as f32 / (r * 8.0) * std::f32::consts::TAU;
                p.set((cx + a.cos() * r) as i32, (cy + a.sin() * r) as i32, col);
                p.set(
                    (cx + a.cos() * (r - 3.0)) as i32,
                    (cy + a.sin() * (r - 3.0)) as i32,
                    col,
                );
            }
            if id == "portal" {
                for k in 0..8 {
                    let rr = (r - 4.0) * (1.0 - k as f32 / 8.0);
                    let t = k as u8 * 18;
                    p.ell(cx, cy, rr, rr, c(60 + t, 30 + t, 120 + t / 2));
                }
                for arm in 0..3 {
                    for s in 0..30 {
                        let t = s as f32 / 30.0;
                        let a = arm as f32 * 2.1 + t * 4.0;
                        p.set(
                            (cx + a.cos() * (r - 4.0) * t) as i32,
                            (cy + a.sin() * (r - 4.0) * t) as i32,
                            WHITE,
                        );
                    }
                }
            } else {
                let pts: Vec<(i32, i32)> = (0..5)
                    .map(|i| {
                        let a =
                            -std::f32::consts::FRAC_PI_2 + i as f32 / 5.0 * std::f32::consts::TAU;
                        (
                            (cx + a.cos() * (r - 4.0)) as i32,
                            (cy + a.sin() * (r - 4.0)) as i32,
                        )
                    })
                    .collect();
                for i in 0..5 {
                    p.line(
                        pts[i].0,
                        pts[i].1,
                        pts[(i + 2) % 5].0,
                        pts[(i + 2) % 5].1,
                        col,
                    );
                }
                for (i, pt) in pts.iter().enumerate() {
                    p.rect(
                        pt.0 - 1,
                        pt.1 - 1,
                        3,
                        3,
                        if i % 2 == 0 { glowc } else { col },
                    );
                }
            }
            glow(&mut p, cx, cy, r + 2.0, glowc);
            return Some(p);
        }
        "cobweb" => {
            let web = [236, 236, 244, 200];
            for i in 0..7 {
                let a = i as f32 / 6.0 * std::f32::consts::FRAC_PI_2;
                p.line(0, 0, (a.cos() * 30.0) as i32, (a.sin() * 30.0) as i32, web);
            }
            for k in 1..6 {
                let r = k as f32 * 5.5;
                for i in 0..(r * 2.0) as i32 {
                    let a = i as f32 / (r * 2.0) * std::f32::consts::FRAC_PI_2;
                    p.set((a.cos() * r) as i32, (a.sin() * r) as i32, web);
                }
            }
            p.rect(12, 11, 3, 3, INK);
            return Some(p);
        }
        "bloodstain" => {
            for i in 0..90 {
                let (x, y) = (4 + (h(i, 0, 8) % 24) as i32, 6 + (h(i, 1, 8) % 20) as i32);
                if ((x as f32 - 16.0) / 9.0).powi(2) + ((y as f32 - 16.0) / 6.0).powi(2) < 1.0
                    || i % 9 == 0
                {
                    p.set(x, y, [120, 16, 22, 210]);
                }
            }
            return Some(p);
        }
        // ------------------------------------------------ expansion pack
        "weapon_rack" => {
            planks(&mut p, 2, 20, pw - 4, 5, dark, true, 21);
            front(&mut p, 2, 25, pw - 4, 3, dark);
            p.srect(2, 3, 3, 22, dark);
            p.srect(pw - 5, 3, 3, 22, dark);
            p.rect(4, 6, pw - 8, 2, dark.m);
            for (k, x) in [10, 20, 30, 40, 52].iter().enumerate() {
                match k % 3 {
                    0 => {
                        p.rect(*x, 2, 2, 20, STEEL.l);
                        p.rect(x - 2, 17, 6, 2, GOLD.m);
                    }
                    1 => {
                        p.rect(*x, 2, 1, 20, wood.m);
                        p.srect(x - 3, 2, 4, 5, STEEL);
                    }
                    _ => {
                        p.rect(*x, 0, 1, 22, wood.l);
                        p.poly(&[(*x, -3), (x + 2, 2), (x - 2, 2)], STEEL.l);
                    }
                }
            }
            true
        }
        "armor_stand" => {
            p.rect(15, 20, 2, 8, dark.m);
            p.rect(10, 27, 12, 2, dark.m);
            p.srect(9, 9, 14, 12, STEEL);
            p.rect(11, 11, 10, 1, STEEL.l);
            p.srect(6, 9, 4, 5, STEEL);
            p.srect(22, 9, 4, 5, STEEL);
            p.srect(11, 1, 10, 8, STEEL);
            p.rect(13, 4, 6, 2, c(30, 26, 36));
            p.rect(15, 12, 2, 7, red.m);
            true
        }
        "anvil" => {
            p.srect(12, 19, 8, 9, IRON);
            p.rect(9, 27, 14, 2, IRON.d);
            p.poly(&[(3, 11), (27, 11), (27, 18), (8, 18)], IRON.m);
            p.rect(9, 11, 18, 2, IRON.l);
            p.poly(&[(3, 11), (8, 11), (8, 14)], IRON.l);
            p.srect(20, 6, 3, 5, wood);
            p.srect(18, 4, 7, 3, STEEL);
            true
        }
        "bar_counter" => {
            planks(&mut p, 1, 6, pw - 2, 9, wood, true, 23);
            front(&mut p, 1, 15, pw - 2, 14, dark);
            for x in (4..pw - 4).step_by(12) {
                p.srect(x, 17, 9, 10, dark);
            }
            mug(&mut p, 12, 4);
            mug(&mut p, 40, 5);
            bottle(&mut p, 70, 12, ramp(70, 120, 70), true, true);
            bottle(&mut p, 78, 12, ramp(120, 60, 40), false, true);
            true
        }
        "training_dummy" => {
            p.rect(15, 18, 2, 10, wood.d);
            p.rect(10, 27, 12, 2, wood.d);
            p.sell(16.0, 15.0, 6.5, 6.0, ramp(200, 170, 110));
            p.rect(4, 11, 24, 3, wood.m);
            p.sell(16.0, 6.0, 4.0, 4.0, ramp(200, 170, 110));
            p.line(10, 15, 22, 15, c(140, 110, 70));
            for r in [5.0f32, 3.0] {
                for i in 0..24 {
                    let a = i as f32 / 24.0 * std::f32::consts::TAU;
                    p.set(
                        (16.0 + a.cos() * r) as i32,
                        (15.0 + a.sin() * r) as i32,
                        red.m,
                    );
                }
            }
            true
        }
        "well" => {
            p.ell(cx, 42.0, 22.0, 12.0, STONE.d);
            p.sell(cx, 38.0, 22.0, 12.0, STONE);
            for i in 0..24 {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                p.set(
                    (cx + a.cos() * 20.0) as i32,
                    (38.0 + a.sin() * 10.5) as i32,
                    STONE.d,
                );
            }
            p.ell(cx, 37.0, 15.0, 7.0, c(20, 28, 40));
            p.ell(cx, 38.0, 10.0, 4.0, c(40, 70, 100));
            for x in [8, pw - 10] {
                p.srect(x, 8, 3, 30, wood);
            }
            p.poly(&[(4, 10), (cx as i32, 1), (pw - 4, 10)], red.m);
            p.rect(8, 16, pw - 16, 2, wood.d);
            p.line(cx as i32, 17, cx as i32, 30, c(200, 180, 140));
            p.srect(cx as i32 - 3, 30, 6, 5, wood);
            true
        }
        "signpost" => {
            p.rect(15, 6, 2, 22, wood.m);
            p.rect(15, 6, 1, 22, wood.l);
            p.poly(&[(4, 6), (24, 6), (28, 9), (24, 12), (4, 12)], wood.l);
            p.poly(&[(8, 14), (27, 14), (27, 19), (8, 19), (4, 16)], wood.m);
            for (x, y) in [(8, 9), (12, 9), (16, 9), (12, 16), (16, 16), (20, 16)] {
                p.set(x, y, dark.d);
            }
            true
        }
        "autumn_tree" => {
            trunk(&mut p, 14, 16, 29, 4, WOOD);
            canopy(&mut p, 16.0, 12.0, 14.0, 11.0, ramp(206, 108, 44), 31);
            speck(&mut p, 3, 2, 26, 20, c(240, 190, 80), 32, 7);
            for i in 0..5 {
                p.set(
                    4 + (h(i, 0, 3) % 24) as i32,
                    28 + (h(i, 1, 3) % 3) as i32,
                    c(200, 90, 40),
                );
            }
            true
        }
        "willow_tree" => {
            trunk(&mut p, 29, 26, ph - 3, 6, WOOD);
            let r = ramp(110, 150, 80);
            canopy(&mut p, cx, 22.0, 28.0, 18.0, r, 41);
            for i in 0..22 {
                let x = 6 + i * 2 + (h(i, 0, 4) % 2) as i32;
                let len = 12 + (h(i, 1, 4) % 18) as i32;
                p.line(
                    x,
                    26,
                    x + (h(i, 2, 4) % 3) as i32 - 1,
                    26 + len,
                    if i % 2 == 0 { r.m } else { r.d },
                );
            }
            true
        }
        "berry_bush" => {
            canopy(&mut p, 16.0, 20.0, 12.0, 9.0, ramp(60, 104, 52), 51);
            for (i, (x, y)) in [
                (10, 16),
                (14, 21),
                (19, 14),
                (22, 20),
                (12, 24),
                (17, 18),
                (24, 24),
            ]
            .iter()
            .enumerate()
            {
                let col = if i % 2 == 0 {
                    c(150, 40, 120)
                } else {
                    c(60, 60, 160)
                };
                p.rect(*x, *y, 2, 2, col);
                p.set(*x, *y, c(240, 200, 240));
            }
            true
        }
        "giant_mushroom" => {
            p.srect(12, 14, 8, 15, ramp(230, 220, 200));
            p.rect(10, 27, 12, 2, ramp(230, 220, 200).d);
            p.sell(16.0, 10.0, 14.0, 8.0, ramp(170, 70, 140));
            p.rect(2, 12, 28, 3, ramp(170, 70, 140).d);
            for (x, y) in [(8, 6), (15, 4), (22, 7), (11, 9), (19, 9)] {
                p.rect(x, y, 2, 2, c(250, 220, 250));
            }
            p.outline();
            glow(&mut p, 16.0, 10.0, 15.0, c(220, 130, 230));
            p.ground_shadow(16.0, 30.0, 9.0, 1.5);
            return Some(p);
        }
        "lily_pads" => {
            p.ell(16.0, 16.0, 14.0, 12.0, [60, 100, 130, 150]);
            for (x, y, r) in [(10.0, 12.0, 5.0f32), (21.0, 18.0, 6.0), (13.0, 22.0, 4.0)] {
                p.sell(x, y, r, r * 0.8, ramp(80, 140, 70));
                p.line(
                    x as i32,
                    y as i32,
                    (x + r) as i32,
                    y as i32,
                    ramp(80, 140, 70).d,
                );
            }
            p.rect(20, 16, 3, 2, c(240, 180, 210));
            p.set(21, 15, WHITE);
            return Some(p);
        }
        "crystal_cluster" => {
            let r = ramp(120, 200, 230);
            for (x, top, w2) in [
                (16, 22, 5),
                (30, 8, 7),
                (44, 18, 6),
                (24, 30, 4),
                (40, 34, 4),
            ] {
                let base = 58;
                p.poly(
                    &[
                        (x - w2, base),
                        (x - w2, top + 6),
                        (x, top),
                        (x + w2, top + 6),
                        (x + w2, base),
                    ],
                    r.m,
                );
                p.rect(x - w2 + 1, top + 6, w2 - 1, base - top - 6, r.l);
                p.rect(x + 1, top + 6, w2 - 1, base - top - 6, r.d);
            }
            rock(&mut p, 32.0, 58.0, 22.0, 5.0, STONE, 7);
            p.outline();
            glow(&mut p, 32.0, 30.0, 30.0, r.l);
            p.ground_shadow(32.0, 61.0, 26.0, 2.0);
            return Some(p);
        }
        "rune_pillar" => {
            let r = ramp(90, 80, 120);
            p.srect(5, ph - 6, 22, 4, r);
            p.srect(8, 4, 16, ph - 10, r);
            p.srect(5, 1, 22, 4, r);
            for y in [9, 15, 21] {
                p.rect(13, y, 6, 1, c(190, 150, 255));
                p.set(15, y - 1, c(230, 210, 255));
                p.set(16, y + 1, c(190, 150, 255));
            }
            p.outline();
            glow(&mut p, 16.0, 15.0, 14.0, c(170, 130, 255));
            p.ground_shadow(16.0, 30.0, 11.0, 1.6);
            return Some(p);
        }
        "sewer_grate" => {
            p.srect(5, 5, 22, 22, IRON);
            p.rect(7, 7, 18, 18, c(16, 16, 20));
            for x in (8..25).step_by(4) {
                p.rect(x, 7, 2, 18, IRON.m);
                p.set(x, 7, IRON.l);
            }
            p.rect(7, 15, 18, 2, IRON.m);
            return Some(p);
        }
        "gargoyle_statue" => {
            let mut g = crate::sprites::creature("mon:monstrosity:gargoyle");
            for px in &mut g.px {
                if px[3] > 0 && *px != INK {
                    let v = (px[0] as u16 + px[1] as u16 + px[2] as u16) / 3;
                    let v = (v as f32 * 0.6 + 60.0) as u8;
                    *px = [v, v, v.saturating_add(4), px[3]];
                }
            }
            p.srect(4, 24, 24, 7, STONE);
            p.blit(&g, 0, -4);
            true
        }
        "treasure_pile" => {
            p.sell(16.0, 22.0, 12.0, 6.0, GOLD);
            p.sell(14.0, 18.0, 8.0, 5.0, GOLD);
            for (x, y) in [(8, 20), (12, 16), (18, 15), (22, 21), (15, 23), (20, 18)] {
                p.rect(x, y, 2, 1, c(255, 240, 170));
            }
            p.rect(20, 11, 4, 5, c(190, 40, 50));
            p.set(21, 12, c(255, 170, 180));
            p.srect(6, 13, 5, 6, ramp(80, 120, 200));
            true
        }
        _ => return None,
    };
    Some(finish(p, shadow))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `DND55_OBJ_SHEET=/path.rgba cargo test dump_object_sheet -- --ignored`
    #[test]
    #[ignore]
    fn dump_object_sheet() {
        let Ok(path) = std::env::var("DND55_OBJ_SHEET") else {
            return;
        };
        let cat = dnd_world::catalog::Catalog::builtin();
        let objs: Vec<_> = cat.assets.iter().filter(|a| a.is_object).collect();
        let cols = 12usize;
        let cell = 100usize;
        let rows = objs.len().div_ceil(cols);
        let (w, hh) = (cols * cell, rows * cell);
        let mut out = vec![96u8; w * hh * 4];
        for (k, a) in objs.iter().enumerate() {
            let (sw, sh) = (a.size.0.min(3), a.size.1.min(3));
            let img = image(&a.id, sw, sh, a.color);
            let (iw, ih) = (img.width() as usize, img.height() as usize);
            let data = img.data.unwrap();
            let (ox, oy) = (
                k % cols * cell + (cell - iw) / 2,
                k / cols * cell + (cell - ih) / 2,
            );
            for y in 0..ih {
                for x in 0..iw {
                    let s = (y * iw + x) * 4;
                    let al = data[s + 3] as f32 / 255.0;
                    let o = ((oy + y) * w + ox + x) * 4;
                    for ch in 0..3 {
                        out[o + ch] = (data[s + ch] as f32 * al + 96.0 * (1.0 - al)) as u8;
                    }
                }
            }
        }
        for px in out.chunks_mut(4) {
            px[3] = 255;
        }
        std::fs::write(&path, out).unwrap();
        std::fs::write(format!("{path}.size"), format!("{w} {hh}")).unwrap();
    }

    #[test]
    fn common_furniture_nature_and_dungeon_objects_are_hand_drawn() {
        for id in [
            "table",
            "round_table",
            "chair",
            "bench",
            "barrel",
            "crate",
            "chest",
            "bookshelf",
            "cabinet",
            "bed",
            "wardrobe",
            "desk",
            "shelf",
            "carpet",
            "candle",
            "lantern",
            "fireplace",
            "bottles",
            "plates",
            "scroll",
            "books",
            "tree",
            "bush",
            "boulder",
            "mushrooms",
            "wildflowers",
            "tall_grass",
            "vines",
            "roots",
            "log",
            "crystal",
            "bones",
            "pillar",
            "altar",
            "tombstone",
            "coffin",
            "skeleton",
            "chains",
            "hanging_cage",
            "iron_gate",
            "torch",
            "brazier",
            "spike_trap",
            "lever",
            "portal",
            "rune_circle",
            "statue",
            "door",
        ] {
            assert!(has(id), "{id} has no pixel sprite");
            let img = image(id, 1, 1, [0, 0, 0]);
            assert_eq!(img.width() % 32, 0, "{id}");
        }
    }
}
