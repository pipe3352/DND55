//! Procedural environment art: painterly, original, generated at runtime (no image files).
//!
//! Art rules shared by every material, so maps read as one illustrated world:
//! * 64 px per grid cell, top-down, light from the upper left (bevel highlights top/left,
//!   shading bottom/right), muted earthy palette.
//! * The base noise of each material is tileable and identical across its variants, so
//!   neighbouring cells join without seams; the four variants differ only in interior details
//!   (stones, flowers, pebbles, plank joints) to break repetition.
//! * Objects get a dark-ink outline (as in the UI icons) and a soft contact shadow.
//! * Transitions between terrains use organic noise masks (see [`edge_mask`]).

use crate::theme::fbm;
use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

pub const T: u32 = 64;
pub const VARIANTS: u32 = 4;

type Rgba = [f32; 4];

pub struct Canvas {
    pub w: u32,
    pub h: u32,
    px: Vec<Rgba>,
}

fn hash(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x.wrapping_mul(374_761_393)
        ^ y.wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

pub fn cell_hash(x: i32, y: i32, seed: u32) -> f32 {
    hash(x as u32, y as u32, seed)
}

fn c3(r: u8, g: u8, b: u8) -> Rgba {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}
fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}
fn shade(c: Rgba, k: f32) -> Rgba {
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}
fn alpha(c: Rgba, a: f32) -> Rgba {
    [c[0], c[1], c[2], c[3] * a]
}
fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[path = "object_art.rs"]
mod object_art;
#[path = "token_art.rs"]
pub mod token_art;

fn object_star(cx: f32, cy: f32, ro: f32, ri: f32, n: usize) -> Vec<(f32, f32)> {
    object_art::star(cx, cy, ro, ri, n, -std::f32::consts::FRAC_PI_2)
}

const INK: Rgba = [0.141, 0.114, 0.102, 1.0];

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Canvas {
            w,
            h,
            px: vec![[0.0; 4]; (w * h) as usize],
        }
    }
    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h)
            .then(|| (y as u32 * self.w + x as u32) as usize)
    }
    pub fn fill(&mut self, f: impl Fn(u32, u32) -> Rgba) {
        for y in 0..self.h {
            for x in 0..self.w {
                self.px[(y * self.w + x) as usize] = f(x, y);
            }
        }
    }
    /// Alpha-over blend.
    pub fn blend(&mut self, x: i32, y: i32, c: Rgba) {
        if let Some(i) = self.idx(x, y) {
            let d = self.px[i];
            let a = c[3];
            let oa = a + d[3] * (1.0 - a);
            if oa <= 0.0 {
                return;
            }
            let mut o = [0.0; 4];
            for k in 0..3 {
                o[k] = (c[k] * a + d[k] * d[3] * (1.0 - a)) / oa;
            }
            o[3] = oa;
            self.px[i] = o;
        }
    }
    /// Wrapping blend (for features that cross tile edges on tileable materials).
    fn blend_wrap(&mut self, x: i32, y: i32, c: Rgba) {
        let (w, h) = (self.w as i32, self.h as i32);
        self.blend(x.rem_euclid(w), y.rem_euclid(h), c);
    }
    /// Antialiased disc.
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        for y in (cy - r - 1.0).floor() as i32..=(cy + r + 1.0).ceil() as i32 {
            for x in (cx - r - 1.0).floor() as i32..=(cx + r + 1.0).ceil() as i32 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let cov = (r + 0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    self.blend(x, y, alpha(c, cov));
                }
            }
        }
    }
    fn disc_wrap(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        for y in (cy - r - 1.0).floor() as i32..=(cy + r + 1.0).ceil() as i32 {
            for x in (cx - r - 1.0).floor() as i32..=(cx + r + 1.0).ceil() as i32 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let cov = (r + 0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    self.blend_wrap(x, y, alpha(c, cov));
                }
            }
        }
    }
    /// Soft radial blob (shadows, glows).
    pub fn blob(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgba) {
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
                let d = (((x as f32 + 0.5 - cx) / rx).powi(2)
                    + ((y as f32 + 0.5 - cy) / ry).powi(2))
                .sqrt();
                if d < 1.0 {
                    self.blend(x, y, alpha(c, (1.0 - d).powf(1.5)));
                }
            }
        }
    }
    /// Filled polygon (any shape, even-odd), 2×2 supersampled edges, optional ink outline.
    pub fn poly(&mut self, pts: &[(f32, f32)], c: Rgba, outline: f32) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (x, y) in pts {
            x0 = x0.min(*x);
            y0 = y0.min(*y);
            x1 = x1.max(*x);
            y1 = y1.max(*y);
        }
        let inside = |px: f32, py: f32| {
            let mut c = false;
            let n = pts.len();
            for i in 0..n {
                let (ax, ay) = pts[i];
                let (bx, by) = pts[(i + n - 1) % n];
                if (ay > py) != (by > py) && px < (bx - ax) * (py - ay) / (by - ay) + ax {
                    c = !c;
                }
            }
            c
        };
        for y in y0.floor() as i32..=y1.ceil() as i32 {
            for x in x0.floor() as i32..=x1.ceil() as i32 {
                let mut cov = 0.0;
                for (sx, sy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    if inside(x as f32 + sx, y as f32 + sy) {
                        cov += 0.25;
                    }
                }
                if cov > 0.0 {
                    self.blend(x, y, alpha(c, cov));
                }
            }
        }
        if outline > 0.0 {
            for i in 0..pts.len() {
                self.line(pts[i], pts[(i + 1) % pts.len()], outline, INK);
            }
        }
    }
    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), w: f32, c: Rgba) {
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(0.01);
        let steps = (len * 2.0).ceil() as i32;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            self.disc(x, y, w / 2.0, alpha(c, 0.55));
        }
    }
    fn line_wrap(&mut self, a: (f32, f32), b: (f32, f32), w: f32, c: Rgba) {
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(0.01);
        let steps = (len * 2.0).ceil() as i32;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            self.disc_wrap(
                a.0 + (b.0 - a.0) * t,
                a.1 + (b.1 - a.1) * t,
                w / 2.0,
                alpha(c, 0.55),
            );
        }
    }
    pub fn map(&mut self, f: impl Fn(u32, u32, Rgba) -> Rgba) {
        for y in 0..self.h {
            for x in 0..self.w {
                let i = (y * self.w + x) as usize;
                self.px[i] = f(x, y, self.px[i]);
            }
        }
    }
    pub fn into_image(self) -> Image {
        let mut data = Vec::with_capacity((self.w * self.h * 4) as usize);
        for p in &self.px {
            for v in p {
                data.push((v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
        }
        let mut img = Image::new_fill(
            Extent3d {
                width: self.w,
                height: self.h,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        img.data = Some(data);
        img.sampler = ImageSampler::linear();
        img
    }
}

/// Base noise field shared by all variants of a material (tileable, seamless across cells).
fn base(x: u32, y: u32, seed: u32) -> f32 {
    fbm(x, y, T, 2, 4, seed)
}

/// Tileable Voronoi over a `n`×`n` jittered grid: (nearest distance, second distance, cell id).
fn voronoi(x: f32, y: f32, n: u32, seed: u32) -> (f32, f32, u32) {
    let cs = T as f32 / n as f32;
    let (gx, gy) = ((x / cs).floor() as i32, (y / cs).floor() as i32);
    let (mut d1, mut d2, mut id) = (f32::MAX, f32::MAX, 0);
    for oy in -1..=1 {
        for ox in -1..=1 {
            let (cx, cy) = (gx + ox, gy + oy);
            let (wx, wy) = (
                cx.rem_euclid(n as i32) as u32,
                cy.rem_euclid(n as i32) as u32,
            );
            let px = (cx as f32 + 0.15 + 0.7 * hash(wx, wy, seed)) * cs;
            let py = (cy as f32 + 0.15 + 0.7 * hash(wx, wy, seed + 1)) * cs;
            let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if d < d1 {
                d2 = d1;
                d1 = d;
                id = wy * n + wx;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    (d1, d2, id)
}

/// Paints a terrain tile material. `variant` changes only interior details.
pub fn tile(id: &str, variant: u32, fallback: [u8; 3]) -> Image {
    let mut c = Canvas::new(T, T);
    let v = variant;
    let seed = id
        .bytes()
        .fold(17u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let rs = seed.wrapping_add(v * 7919); // per-variant randomness
    let r = |i: u32, k: u32| hash(i, k, rs);
    match id {
        "grass" | "undergrowth" => {
            let dense = id == "undergrowth";
            let (a, b) = if dense {
                (c3(46, 70, 34), c3(70, 96, 44))
            } else {
                (c3(78, 104, 48), c3(108, 134, 62))
            };
            c.fill(|x, y| {
                let n = base(x, y, seed);
                let m = fbm(x, y, T, 4, 3, seed + 3);
                shade(mix(a, b, n), 0.9 + m * 0.2)
            });
            // Grass blades (wrapping so they cross cell edges seamlessly).
            let blades = if dense { 260 } else { 170 };
            for i in 0..blades {
                let (x, y) = (hash(i, 1, seed) * 64.0, hash(i, 2, seed) * 64.0);
                let ang = -std::f32::consts::FRAC_PI_2 + (hash(i, 3, seed) - 0.5) * 1.0;
                let len = 3.0 + hash(i, 4, seed) * 4.0;
                let light = hash(i, 5, seed) > 0.5;
                let col = if light {
                    mix(b, c3(170, 180, 90), 0.4)
                } else {
                    shade(a, 0.7)
                };
                c.line_wrap(
                    (x, y),
                    (x + ang.cos() * len, y + ang.sin() * len),
                    1.0,
                    alpha(col, 0.8),
                );
            }
            if dense {
                for i in 0..10 {
                    let (x, y) = (8.0 + r(i, 1) * 48.0, 8.0 + r(i, 2) * 48.0);
                    c.disc(x, y, 3.0 + r(i, 3) * 3.0, alpha(c3(36, 58, 28), 0.85));
                    c.disc(x - 1.0, y - 1.0, 2.0, alpha(c3(84, 116, 52), 0.8));
                }
            } else if v >= 2 {
                // Wildflowers on two of the four variants.
                let colors = [c3(236, 220, 140), c3(240, 236, 226), c3(170, 130, 200)];
                for i in 0..(3 + v) {
                    let (x, y) = (8.0 + r(i, 1) * 48.0, 8.0 + r(i, 2) * 48.0);
                    let col = colors[(r(i, 3) * 3.0) as usize % 3];
                    for k in 0..5 {
                        let a = k as f32 * 1.2566;
                        c.disc(x + a.cos() * 1.6, y + a.sin() * 1.6, 1.1, col);
                    }
                    c.disc(x, y, 0.9, c3(220, 170, 60));
                }
            }
        }
        "dirt" | "mud" => {
            let mud = id == "mud";
            let (a, b) = if mud {
                (c3(58, 42, 30), c3(84, 62, 42))
            } else {
                (c3(104, 78, 52), c3(138, 108, 72))
            };
            c.fill(|x, y| {
                shade(
                    mix(a, b, base(x, y, seed)),
                    0.92 + fbm(x, y, T, 8, 2, seed + 5) * 0.14,
                )
            });
            if mud {
                for i in 0..(2 + v) {
                    let (x, y) = (12.0 + r(i, 1) * 40.0, 12.0 + r(i, 2) * 40.0);
                    c.blob(
                        x,
                        y,
                        7.0 + r(i, 3) * 8.0,
                        4.0 + r(i, 4) * 4.0,
                        alpha(c3(40, 30, 24), 0.7),
                    );
                    c.blob(x - 2.0, y - 1.5, 4.0, 1.5, alpha(c3(150, 140, 120), 0.35));
                }
            } else {
                for i in 0..(4 + v * 2) {
                    let (x, y) = (6.0 + r(i, 1) * 52.0, 6.0 + r(i, 2) * 52.0);
                    let rr = 1.2 + r(i, 3) * 2.2;
                    c.disc(x + 0.8, y + 0.8, rr, alpha(c3(60, 44, 30), 0.6));
                    c.disc(x, y, rr, mix(c3(130, 120, 106), c3(160, 150, 130), r(i, 4)));
                    c.disc(
                        x - rr * 0.35,
                        y - rr * 0.35,
                        rr * 0.4,
                        alpha(c3(220, 210, 190), 0.5),
                    );
                }
                if v == 3 {
                    c.line((14.0, 30.0), (26.0, 34.0), 1.0, alpha(c3(70, 50, 34), 0.8));
                    c.line((26.0, 34.0), (34.0, 28.0), 1.0, alpha(c3(70, 50, 34), 0.8));
                }
            }
        }
        "sand" => {
            c.fill(|x, y| {
                let n = base(x, y, seed);
                let ripple =
                    ((y as f32 * 0.3 + x as f32 * 0.1 + fbm(x, y, T, 2, 3, seed + 1) * 14.0).sin()
                        * 0.5
                        + 0.5)
                        .powf(4.0);
                shade(
                    mix(c3(204, 180, 130), c3(228, 208, 162), n),
                    0.96 + ripple * 0.06,
                )
            });
            for i in 0..(2 + v) {
                c.disc(
                    8.0 + r(i, 1) * 48.0,
                    8.0 + r(i, 2) * 48.0,
                    1.0 + r(i, 3),
                    c3(170, 150, 120),
                );
            }
        }
        "snow" => {
            c.fill(|x, y| {
                let n = base(x, y, seed);
                mix(
                    c3(214, 222, 232),
                    c3(244, 246, 250),
                    n * 0.8 + fbm(x, y, T, 4, 2, seed + 2) * 0.2,
                )
            });
            for i in 0..14 {
                c.disc(
                    hash(i, 1, seed) * 64.0,
                    hash(i, 2, seed) * 64.0,
                    0.6,
                    c3(255, 255, 255),
                );
            }
            if v == 1 {
                c.blob(30.0, 34.0, 10.0, 5.0, alpha(c3(170, 186, 210), 0.35));
            }
        }
        "ice" => {
            c.fill(|x, y| {
                let n = base(x, y, seed);
                let streak = (((x as f32 + y as f32) * 0.05 + fbm(x, y, T, 2, 2, seed + 4) * 6.0)
                    .sin()
                    * 0.5
                    + 0.5)
                    .powf(10.0);
                mix(
                    mix(c3(150, 196, 214), c3(196, 228, 238), n),
                    c3(240, 250, 255),
                    streak * 0.35,
                )
            });
            let mut p = (r(0, 1) * 20.0 + 10.0, r(0, 2) * 20.0 + 10.0);
            for i in 0..(4 + v) {
                let q = (p.0 + (r(i, 3) - 0.3) * 18.0, p.1 + (r(i, 4) - 0.3) * 18.0);
                c.line(p, q, 0.8, alpha(c3(250, 255, 255), 0.9));
                p = q;
            }
        }
        "stone_floor" | "stone_wall" | "pressure_plate" => {
            let wall = id == "stone_wall";
            // Flagstones aligned to the cell: mortar on every border, layout varies per variant.
            let layouts: [&[(f32, f32, f32, f32)]; 4] = [
                &[
                    (0.0, 0.0, 32.0, 32.0),
                    (32.0, 0.0, 32.0, 32.0),
                    (0.0, 32.0, 32.0, 32.0),
                    (32.0, 32.0, 32.0, 32.0),
                ],
                &[
                    (0.0, 0.0, 40.0, 26.0),
                    (40.0, 0.0, 24.0, 26.0),
                    (0.0, 26.0, 24.0, 38.0),
                    (24.0, 26.0, 40.0, 38.0),
                ],
                &[
                    (0.0, 0.0, 64.0, 30.0),
                    (0.0, 30.0, 30.0, 34.0),
                    (30.0, 30.0, 34.0, 34.0),
                ],
                &[
                    (0.0, 0.0, 22.0, 64.0),
                    (22.0, 0.0, 42.0, 34.0),
                    (22.0, 34.0, 42.0, 30.0),
                ],
            ];
            let wall_layout: [&[(f32, f32, f32, f32)]; 2] = [
                &[
                    (0.0, 0.0, 40.0, 21.0),
                    (40.0, 0.0, 24.0, 21.0),
                    (0.0, 21.0, 20.0, 22.0),
                    (20.0, 21.0, 44.0, 22.0),
                    (0.0, 43.0, 34.0, 21.0),
                    (34.0, 43.0, 30.0, 21.0),
                ],
                &[
                    (0.0, 0.0, 26.0, 21.0),
                    (26.0, 0.0, 38.0, 21.0),
                    (0.0, 21.0, 44.0, 22.0),
                    (44.0, 21.0, 20.0, 22.0),
                    (0.0, 43.0, 22.0, 21.0),
                    (22.0, 43.0, 42.0, 21.0),
                ],
            ];
            let rects = if wall {
                wall_layout[(v % 2) as usize]
            } else {
                layouts[v as usize % 4]
            };
            let (lo, hi, mortar) = if wall {
                (c3(92, 90, 88), c3(120, 116, 110), c3(34, 30, 28))
            } else {
                (c3(118, 112, 102), c3(150, 142, 128), c3(66, 58, 50))
            };
            c.fill(|_, _| mortar);
            for (i, (rx, ry, rw, rh)) in rects.iter().enumerate() {
                let tone = r(i as u32, 9);
                let col = mix(lo, hi, tone);
                let (x0, y0, x1, y1) = (*rx + 1.5, *ry + 1.5, rx + rw - 1.5, ry + rh - 1.5);
                for y in y0 as u32..y1.ceil() as u32 {
                    for x in x0 as u32..x1.ceil() as u32 {
                        let n = base(x, y, seed);
                        let mut k = 0.86 + n * 0.26;
                        let (dx0, dy0, dx1, dy1) =
                            (x as f32 - x0, y as f32 - y0, x1 - x as f32, y1 - y as f32);
                        if dx0 < 2.0 || dy0 < 2.0 {
                            k *= 1.18; // lit upper-left bevel
                        }
                        if dx1 < 2.5 || dy1 < 2.5 {
                            k *= 0.74; // shaded lower-right bevel
                        }
                        c.blend(x as i32, y as i32, shade(col, k));
                    }
                }
                if hash(i as u32, v, seed + 77) > 0.7 {
                    let (cx, cy) = (rx + rw * 0.3, ry + rh * 0.5);
                    c.line(
                        (cx, cy),
                        (cx + rw * 0.25, cy + rh * 0.15),
                        0.8,
                        alpha(mortar, 0.9),
                    );
                }
            }
            if wall {
                // A wall is a raised mass: darken it slightly and add a top-lit cap line.
                c.map(|_, y, p| shade(p, if y < 3 { 1.25 } else { 0.92 }));
            }
            if id == "pressure_plate" {
                for (a, b) in [
                    ((14.0, 14.0), (50.0, 14.0)),
                    ((50.0, 14.0), (50.0, 50.0)),
                    ((50.0, 50.0), (14.0, 50.0)),
                    ((14.0, 50.0), (14.0, 14.0)),
                ] {
                    c.line(a, b, 1.6, alpha(c3(30, 26, 22), 0.9));
                }
            }
        }
        "road" | "gravel" | "rubble" | "cave_floor" => {
            let (n, mortar_w, cols, mortar_c, var) = match id {
                "road" => (
                    4,
                    1.6,
                    (c3(118, 108, 96), c3(150, 138, 118)),
                    c3(92, 72, 50),
                    0.25,
                ),
                "gravel" => (
                    9,
                    0.9,
                    (c3(110, 104, 98), c3(158, 150, 138)),
                    c3(80, 68, 56),
                    0.4,
                ),
                "rubble" => (
                    5,
                    2.4,
                    (c3(96, 92, 88), c3(130, 124, 116)),
                    c3(72, 60, 50),
                    0.35,
                ),
                _ => (
                    3,
                    0.7,
                    (c3(74, 68, 64), c3(92, 86, 80)),
                    c3(40, 36, 34),
                    0.15,
                ),
            };
            stones_simple(&mut c, n, seed, mortar_w, cols, mortar_c, var);
            if id == "cave_floor" {
                for i in 0..(2 + v) {
                    let (x, y) = (8.0 + r(i, 1) * 48.0, 8.0 + r(i, 2) * 48.0);
                    c.disc(x, y, 1.5 + r(i, 3) * 1.5, c3(110, 104, 98));
                    c.disc(x - 0.5, y - 0.5, 0.8, alpha(c3(190, 184, 176), 0.6));
                }
            }
        }
        "wood_floor" | "wood_wall" => {
            let wall = id == "wood_wall";
            let (lo, hi) = if wall {
                (c3(70, 46, 30), c3(96, 64, 40))
            } else {
                (c3(112, 76, 46), c3(146, 102, 62))
            };
            c.fill(|x, y| {
                let plank = x / 16;
                let tone = hash(plank, 1, seed);
                let warp = fbm(x, y / 4, T, 4, 2, seed + plank) * 5.0;
                let grain = ((x as f32 * 1.3 + warp).sin() * 0.5 + 0.5).powf(2.5);
                let mut col = shade(mix(lo, hi, tone), 0.9 + grain * 0.18);
                if x % 16 == 0 {
                    col = shade(col, 0.45);
                } else if x % 16 == 1 {
                    col = shade(col, 1.15);
                }
                col
            });
            // Staggered plank ends (variant), nails.
            for p in 0..4u32 {
                let end = ((hash(p, v, seed + 3) * 48.0) as i32 + 8) as f32;
                c.line(
                    (p as f32 * 16.0 + 1.0, end),
                    (p as f32 * 16.0 + 15.0, end),
                    1.0,
                    alpha(c3(40, 26, 18), 0.9),
                );
                for dy in [end - 3.0, end + 3.0] {
                    c.disc(p as f32 * 16.0 + 4.0, dy, 0.8, c3(60, 56, 52));
                    c.disc(p as f32 * 16.0 + 12.0, dy, 0.8, c3(60, 56, 52));
                }
            }
            if wall {
                for y in [10.0, 54.0] {
                    c.line((0.0, y), (64.0, y), 4.0, c3(52, 50, 48));
                }
            }
        }
        "water_shallow" | "water_deep" => {
            // River bed under the animated water surface (visible at the soft shoreline).
            let deep = id == "water_deep";
            c.fill(|x, y| {
                let n = base(x, y, seed);
                if deep {
                    mix(c3(30, 46, 52), c3(42, 60, 64), n)
                } else {
                    mix(c3(118, 106, 80), c3(146, 132, 100), n)
                }
            });
        }
        "lava" => {
            c.fill(|x, y| {
                let n = base(x, y, seed);
                mix(c3(40, 22, 18), c3(70, 34, 22), n)
            });
        }
        _ => {
            let [rr, gg, bb] = fallback;
            let col = c3(rr, gg, bb);
            c.fill(|x, y| shade(col, 0.88 + base(x, y, seed) * 0.24));
        }
    }
    c.into_image()
}

/// Simple tileable cobbles/stones with a bevel from the Voronoi distance field.
fn stones_simple(
    c: &mut Canvas,
    n: u32,
    seed: u32,
    mortar: f32,
    cols: (Rgba, Rgba),
    mortar_c: Rgba,
    var: f32,
) {
    c.fill(|x, y| {
        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
        let (d1, d2, id) = voronoi(fx, fy, n, seed);
        let edge = d2 - d1;
        let tone = hash(id, 7, seed);
        let mut col = mix(cols.0, cols.1, tone);
        col = shade(col, 1.0 - var / 2.0 + var * hash(id, 3, seed));
        col = shade(col, 0.9 + fbm(x, y, T, 4, 3, seed + 9) * 0.2);
        // Dome shading: brighter towards the stone centre, lit from the upper left.
        let (d1l, _, idl) = voronoi(fx - 2.0, fy - 2.0, n, seed);
        let light = if idl == id {
            (d1l < d1) as i32 as f32
        } else {
            0.0
        };
        col = shade(col, 0.92 + light * 0.16 + (edge / 10.0).min(0.08));
        let m = smooth(mortar * 0.5, mortar + 1.2, edge);
        mix(mortar_c, col, m)
    });
}

// ------------------------------------------------------------------ transitions

/// Directions for edge masks: 0 N, 1 E, 2 S, 3 W (edges), 4 NE, 5 SE, 6 SW, 7 NW (outer corners).
pub const DIRS: [(i32, i32); 8] = [
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, 0),
    (1, -1),
    (1, 1),
    (-1, 1),
    (-1, -1),
];

/// Organic alpha mask: opaque at the given edge/corner, fading inward with a noisy front.
pub fn edge_mask(dir: usize, seed: u32) -> Vec<f32> {
    let mut m = vec![0.0; (T * T) as usize];
    for y in 0..T {
        for x in 0..T {
            let (u, v) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = match dir {
                0 => v,
                1 => 64.0 - u,
                2 => 64.0 - v,
                3 => u,
                4 => ((64.0 - u).powi(2) + v.powi(2)).sqrt(),
                5 => ((64.0 - u).powi(2) + (64.0 - v).powi(2)).sqrt(),
                6 => (u.powi(2) + (64.0 - v).powi(2)).sqrt(),
                _ => (u.powi(2) + v.powi(2)).sqrt(),
            } / 64.0;
            // The front wanders with low-frequency noise; small speckles break it up further.
            let front = 0.24 + (fbm(x, y, T, 2, 3, seed + dir as u32) - 0.5) * 0.28;
            let speck = (hash(x, y, seed) - 0.5) * 0.05;
            m[(y * T + x) as usize] = 1.0 - smooth(front - 0.07, front + 0.07, d + speck);
        }
    }
    m
}

/// `material` texture cut by an edge mask (overlay drawn on the neighbouring cell).
pub fn transition(material: &Image, dir: usize) -> Image {
    let mask = edge_mask(dir, 5);
    let data = material.data.as_ref().expect("cpu image");
    let mut c = Canvas::new(T, T);
    c.fill(|x, y| {
        let i = ((y * T + x) * 4) as usize;
        let a = mask[(y * T + x) as usize];
        [
            data[i] as f32 / 255.0,
            data[i + 1] as f32 / 255.0,
            data[i + 2] as f32 / 255.0,
            a,
        ]
    });
    c.into_image()
}

/// Ambient-occlusion shade cast onto a floor cell by an adjacent wall in `dir`.
pub fn ambient_occlusion(dir: usize) -> Image {
    let mut c = Canvas::new(T, T);
    c.fill(|x, y| {
        let (u, v) = (x as f32 + 0.5, y as f32 + 0.5);
        let d = match dir {
            0 => v,
            1 => 64.0 - u,
            2 => 64.0 - v,
            _ => u,
        } / 64.0;
        [
            0.05,
            0.03,
            0.02,
            (1.0 - smooth(0.0, 0.42, d)).powf(1.6) * 0.55,
        ]
    });
    c.into_image()
}

/// Soft elliptical contact shadow (stretched per object).
pub fn shadow() -> Image {
    let mut c = Canvas::new(T, T);
    c.blob(32.0, 32.0, 31.0, 31.0, [0.03, 0.02, 0.02, 0.8]);
    c.into_image()
}

/// Smooth light glow: bright core, long soft falloff (lights, lava, spells).
pub fn glow() -> Image {
    let s = 128u32;
    let mut c = Canvas::new(s, s);
    c.fill(|x, y| {
        let d = (((x as f32 + 0.5 - 64.0).powi(2) + (y as f32 + 0.5 - 64.0).powi(2)).sqrt() / 64.0)
            .min(1.0);
        let a = (1.0 - d).powf(2.2) * 0.85 + (1.0 - smooth(0.0, 0.25, d)) * 0.15;
        [1.0, 1.0, 1.0, a]
    });
    c.into_image()
}

// ------------------------------------------------------------------ decorations

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Decor {
    Tuft,
    Flowers,
    Pebbles,
    Leaves,
    Bones,
    Twig,
}

pub fn decor_for(terrain: &str, h: f32) -> Option<Decor> {
    match terrain {
        "grass" if h < 0.16 => Some(Decor::Tuft),
        "grass" if h < 0.22 => Some(Decor::Flowers),
        "grass" if h < 0.26 => Some(Decor::Leaves),
        "undergrowth" if h < 0.3 => Some(Decor::Tuft),
        "dirt" | "road" if h < 0.14 => Some(Decor::Pebbles),
        "dirt" if h < 0.18 => Some(Decor::Twig),
        "cave_floor" if h < 0.12 => Some(Decor::Pebbles),
        "cave_floor" if h < 0.15 => Some(Decor::Bones),
        "sand" if h < 0.08 => Some(Decor::Pebbles),
        "snow" if h < 0.06 => Some(Decor::Twig),
        "mud" if h < 0.1 => Some(Decor::Leaves),
        _ => None,
    }
}

pub fn decor(kind: Decor) -> Image {
    let mut c = Canvas::new(32, 32);
    match kind {
        Decor::Tuft => {
            for i in 0..14 {
                let a = -std::f32::consts::FRAC_PI_2 + (hash(i, 1, 3) - 0.5) * 1.6;
                let len = 7.0 + hash(i, 2, 3) * 8.0;
                let col = mix(c3(60, 90, 40), c3(140, 160, 70), hash(i, 3, 3));
                c.line(
                    (16.0 + (hash(i, 4, 3) - 0.5) * 6.0, 26.0),
                    (16.0 + a.cos() * len, 26.0 + a.sin() * len),
                    1.4,
                    col,
                );
            }
        }
        Decor::Flowers => {
            for i in 0..4 {
                let (x, y) = (8.0 + hash(i, 1, 5) * 16.0, 8.0 + hash(i, 2, 5) * 16.0);
                c.line((x, y + 6.0), (x, y), 1.0, c3(60, 100, 40));
                for k in 0..5 {
                    let a = k as f32 * 1.2566;
                    c.disc(
                        x + a.cos() * 2.2,
                        y + a.sin() * 2.2,
                        1.5,
                        [0.93, 0.88, 0.95, 1.0],
                    );
                }
                c.disc(x, y, 1.2, c3(230, 180, 60));
            }
        }
        Decor::Pebbles => {
            for i in 0..5 {
                let (x, y, r) = (
                    6.0 + hash(i, 1, 9) * 20.0,
                    6.0 + hash(i, 2, 9) * 20.0,
                    1.5 + hash(i, 3, 9) * 2.5,
                );
                c.disc(x + 1.0, y + 1.0, r, [0.1, 0.08, 0.06, 0.5]);
                c.disc(
                    x,
                    y,
                    r,
                    mix(c3(120, 114, 106), c3(170, 162, 150), hash(i, 4, 9)),
                );
                c.disc(x - r * 0.3, y - r * 0.3, r * 0.4, [1.0, 1.0, 1.0, 0.3]);
            }
        }
        Decor::Leaves => {
            for i in 0..5 {
                let (x, y) = (6.0 + hash(i, 1, 13) * 20.0, 6.0 + hash(i, 2, 13) * 20.0);
                let a = hash(i, 3, 13) * std::f32::consts::TAU;
                let col = mix(c3(150, 90, 40), c3(190, 140, 50), hash(i, 4, 13));
                let (dx, dy) = (a.cos() * 3.5, a.sin() * 3.5);
                c.poly(
                    &[
                        (x - dx, y - dy),
                        (x - dy * 0.5, y + dx * 0.5),
                        (x + dx, y + dy),
                        (x + dy * 0.5, y - dx * 0.5),
                    ],
                    col,
                    0.0,
                );
            }
        }
        Decor::Bones => {
            c.line((8.0, 20.0), (22.0, 14.0), 2.2, c3(220, 212, 190));
            for (x, y) in [(8.0, 20.0), (22.0, 14.0)] {
                c.disc(x, y - 1.0, 1.8, c3(225, 218, 196));
                c.disc(x, y + 1.0, 1.8, c3(225, 218, 196));
            }
            c.disc(20.0, 23.0, 3.5, c3(214, 206, 186));
        }
        Decor::Twig => {
            c.line((6.0, 22.0), (26.0, 12.0), 1.6, c3(80, 56, 36));
            c.line((16.0, 17.0), (19.0, 24.0), 1.0, c3(80, 56, 36));
        }
    }
    c.into_image()
}

// ------------------------------------------------------------------ objects

/// Illustrated top-down object art (`w`×`h` cells), ink-outlined, lit from the upper left.
pub fn object(id: &str, w: u8, h: u8, fallback: [u8; 3]) -> Image {
    let (pw, ph) = (T * w as u32, T * h as u32);
    let mut c = Canvas::new(pw, ph);
    let (fw, fh) = (pw as f32, ph as f32);
    let (cx, cy) = (fw / 2.0, fh / 2.0);
    let s = fw.min(fh) / 64.0; // scale relative to one cell
    let ow = 2.0 * s.max(1.0);
    let ring = |c: &mut Canvas, cx: f32, cy: f32, r: f32, col: Rgba| {
        c.disc(cx, cy, r + ow * 0.6, INK);
        c.disc(cx, cy, r, col);
    };
    if object_art::draw(&mut c, id, fw, fh) {
        return c.into_image();
    }
    match id {
        "tree" | "big_tree" | "bush" => {
            let bush = id == "bush";
            let (dark, mid, light) = if bush {
                (c3(40, 70, 34), c3(64, 100, 46), c3(110, 146, 70))
            } else {
                (c3(30, 58, 30), c3(52, 88, 42), c3(98, 136, 62))
            };
            let r0 = fw.min(fh) * if bush { 0.36 } else { 0.42 };
            let lobes = if bush { 6 } else { 9 };
            // Outline pass, then body, then highlights: a clustered canopy.
            for pass in 0..3 {
                for i in 0..lobes {
                    let a = i as f32 / lobes as f32 * std::f32::consts::TAU + hash(i, 1, 5) * 0.4;
                    let d = r0 * 0.55;
                    let (x, y) = (cx + a.cos() * d, cy + a.sin() * d);
                    let rr = r0 * (0.48 + hash(i, 2, 5) * 0.12);
                    match pass {
                        0 => c.disc(x, y, rr + ow, INK),
                        1 => c.disc(x, y, rr, mix(dark, mid, hash(i, 3, 5))),
                        _ => c.disc(x - rr * 0.25, y - rr * 0.3, rr * 0.55, alpha(light, 0.5)),
                    }
                }
                match pass {
                    0 => c.disc(cx, cy, r0 * 0.6 + ow, INK),
                    1 => c.disc(cx, cy, r0 * 0.6, mid),
                    _ => c.disc(cx - r0 * 0.15, cy - r0 * 0.2, r0 * 0.35, alpha(light, 0.6)),
                }
            }
            if bush {
                for i in 0..5 {
                    c.disc(
                        cx + (hash(i, 7, 1) - 0.5) * r0 * 1.2,
                        cy + (hash(i, 8, 1) - 0.5) * r0 * 1.2,
                        1.8 * s,
                        c3(170, 40, 50),
                    );
                }
            }
        }
        "boulder" | "stalagmite" => {
            let stal = id == "stalagmite";
            let n = 9;
            let pts: Vec<(f32, f32)> = (0..n)
                .map(|i| {
                    let a = i as f32 / n as f32 * std::f32::consts::TAU;
                    let rr = fw * (0.36 + hash(i, 4, 2) * 0.08);
                    (cx + a.cos() * rr, cy + a.sin() * rr)
                })
                .collect();
            if stal {
                for k in 0..5 {
                    let f = 1.0 - k as f32 * 0.18;
                    c.disc(
                        cx - (1.0 - f) * 6.0 * s,
                        cy - (1.0 - f) * 6.0 * s,
                        fw * 0.4 * f,
                        mix(c3(70, 64, 60), c3(150, 142, 132), k as f32 / 4.0),
                    );
                }
                c.disc(cx - 6.0 * s, cy - 6.0 * s, 3.0 * s, c3(200, 194, 184));
            } else {
                c.poly(&pts, c3(118, 112, 104), ow);
                c.blob(
                    cx - 6.0 * s,
                    cy - 7.0 * s,
                    fw * 0.24,
                    fh * 0.2,
                    alpha(c3(200, 194, 182), 0.5),
                );
                c.blob(
                    cx + 8.0 * s,
                    cy + 9.0 * s,
                    fw * 0.24,
                    fh * 0.18,
                    alpha(c3(40, 36, 34), 0.45),
                );
                c.blob(
                    cx + 4.0 * s,
                    cy - 10.0 * s,
                    6.0 * s,
                    4.0 * s,
                    alpha(c3(90, 120, 60), 0.8),
                );
            }
        }
        "crystal" => {
            for (dx, dy, hh, col) in [
                (-9.0, 4.0, 18.0, c3(120, 80, 170)),
                (8.0, 6.0, 16.0, c3(90, 140, 190)),
                (0.0, -2.0, 26.0, c3(150, 110, 210)),
            ] {
                let (x, y) = (cx + dx * s, cy + dy * s);
                let (hw, hh) = (6.0 * s, hh * s);
                c.poly(
                    &[
                        (x, y - hh),
                        (x + hw, y - hh * 0.55),
                        (x + hw, y + hh * 0.35),
                        (x, y + hh * 0.6),
                        (x - hw, y + hh * 0.35),
                        (x - hw, y - hh * 0.55),
                    ],
                    col,
                    ow,
                );
                c.poly(
                    &[(x, y - hh), (x + hw, y - hh * 0.55), (x, y + hh * 0.6)],
                    alpha(c3(240, 230, 255), 0.35),
                    0.0,
                );
            }
        }
        "table" | "altar" => {
            let altar = id == "altar";
            let (x0, y0, x1, y1) = (6.0 * s, 10.0 * s, fw - 6.0 * s, fh - 10.0 * s);
            c.poly(
                &[(x0, y0), (x1, y0), (x1, y1), (x0, y1)],
                if altar {
                    c3(130, 124, 116)
                } else {
                    c3(126, 86, 52)
                },
                ow,
            );
            if altar {
                c.poly(
                    &[
                        (cx - 8.0 * s, y0),
                        (cx + 8.0 * s, y0),
                        (cx + 8.0 * s, y1),
                        (cx - 8.0 * s, y1),
                    ],
                    c3(110, 40, 40),
                    0.0,
                );
                for x in [x0 + 6.0 * s, x1 - 6.0 * s] {
                    c.disc(x, cy, 3.0 * s, c3(236, 226, 200));
                    c.disc(x, cy, 1.3 * s, c3(255, 200, 90));
                }
            } else {
                let mut y = y0 + 9.0 * s;
                while y < y1 {
                    c.line(
                        (x0 + 2.0, y),
                        (x1 - 2.0, y),
                        1.0,
                        alpha(c3(70, 46, 28), 0.8),
                    );
                    y += 9.0 * s;
                }
                c.disc(cx + 10.0 * s, cy - 4.0 * s, 4.0 * s + ow, INK);
                c.disc(cx + 10.0 * s, cy - 4.0 * s, 4.0 * s, c3(170, 170, 160));
                c.disc(cx - 10.0 * s, cy + 2.0 * s, 5.0 * s, c3(230, 220, 190));
            }
        }
        "chair" => {
            c.poly(
                &[
                    (cx - 13.0 * s, cy - 8.0 * s),
                    (cx + 13.0 * s, cy - 8.0 * s),
                    (cx + 13.0 * s, cy + 14.0 * s),
                    (cx - 13.0 * s, cy + 14.0 * s),
                ],
                c3(126, 86, 52),
                ow,
            );
            c.poly(
                &[
                    (cx - 15.0 * s, cy - 16.0 * s),
                    (cx + 15.0 * s, cy - 16.0 * s),
                    (cx + 15.0 * s, cy - 8.0 * s),
                    (cx - 15.0 * s, cy - 8.0 * s),
                ],
                c3(96, 64, 40),
                ow,
            );
        }
        "bed" => {
            c.poly(
                &[
                    (4.0 * s, 4.0 * s),
                    (fw - 4.0 * s, 4.0 * s),
                    (fw - 4.0 * s, fh - 4.0 * s),
                    (4.0 * s, fh - 4.0 * s),
                ],
                c3(100, 66, 40),
                ow,
            );
            c.poly(
                &[
                    (8.0 * s, fh * 0.3),
                    (fw - 8.0 * s, fh * 0.3),
                    (fw - 8.0 * s, fh - 8.0 * s),
                    (8.0 * s, fh - 8.0 * s),
                ],
                c3(120, 46, 44),
                ow * 0.6,
            );
            c.poly(
                &[
                    (10.0 * s, 9.0 * s),
                    (fw - 10.0 * s, 9.0 * s),
                    (fw - 10.0 * s, fh * 0.26),
                    (10.0 * s, fh * 0.26),
                ],
                c3(232, 224, 204),
                ow * 0.6,
            );
        }
        "bookshelf" => {
            c.poly(
                &[
                    (2.0, 4.0 * s),
                    (fw - 2.0, 4.0 * s),
                    (fw - 2.0, fh - 4.0 * s),
                    (2.0, fh - 4.0 * s),
                ],
                c3(84, 56, 34),
                ow,
            );
            let mut x = 6.0 * s;
            let mut i = 0;
            while x < fw - 8.0 * s {
                let bw = (3.0 + hash(i, 1, 4) * 3.0) * s;
                let col = [
                    c3(120, 40, 40),
                    c3(50, 80, 60),
                    c3(60, 70, 110),
                    c3(150, 120, 60),
                ][i as usize % 4];
                c.poly(
                    &[
                        (x, 9.0 * s),
                        (x + bw, 9.0 * s),
                        (x + bw, fh - 9.0 * s),
                        (x, fh - 9.0 * s),
                    ],
                    col,
                    0.0,
                );
                x += bw + 1.0;
                i += 1;
            }
        }
        "barrel" => {
            ring(&mut c, cx, cy, fw * 0.4, c3(116, 78, 46));
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                c.line(
                    (cx, cy),
                    (cx + a.cos() * fw * 0.38, cy + a.sin() * fw * 0.38),
                    0.8,
                    alpha(c3(70, 46, 28), 0.7),
                );
            }
            c.disc(cx, cy, fw * 0.36, [0.0; 4]);
            for rr in [0.4, 0.28] {
                for k in 0..90 {
                    let a = k as f32 / 90.0 * std::f32::consts::TAU;
                    c.disc(
                        cx + a.cos() * fw * rr,
                        cy + a.sin() * fw * rr,
                        1.3 * s,
                        c3(70, 68, 64),
                    );
                }
            }
            c.blob(
                cx - 6.0 * s,
                cy - 6.0 * s,
                9.0 * s,
                7.0 * s,
                alpha(c3(255, 230, 190), 0.25),
            );
        }
        "crate" | "chest" => {
            let chest = id == "chest";
            let (x0, y0, x1, y1) = (
                8.0 * s,
                if chest { 14.0 } else { 8.0 } * s,
                fw - 8.0 * s,
                fh - if chest { 14.0 } else { 8.0 } * s,
            );
            c.poly(
                &[(x0, y0), (x1, y0), (x1, y1), (x0, y1)],
                if chest {
                    c3(120, 76, 42)
                } else {
                    c3(150, 110, 66)
                },
                ow,
            );
            if chest {
                for x in [x0 + 10.0 * s, x1 - 10.0 * s] {
                    c.line((x, y0), (x, y1), 4.0 * s, c3(66, 64, 62));
                }
                c.poly(
                    &[
                        (cx - 5.0 * s, cy - 4.0 * s),
                        (cx + 5.0 * s, cy - 4.0 * s),
                        (cx + 5.0 * s, cy + 5.0 * s),
                        (cx - 5.0 * s, cy + 5.0 * s),
                    ],
                    c3(200, 160, 70),
                    ow * 0.5,
                );
            } else {
                c.line((x0, y0), (x1, y1), 4.0 * s, c3(110, 76, 44));
                c.line((x1, y0), (x0, y1), 4.0 * s, c3(110, 76, 44));
                c.poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], [0.0; 4], ow);
            }
        }
        "door" => {
            c.poly(
                &[
                    (2.0, 14.0 * s),
                    (fw - 2.0, 14.0 * s),
                    (fw - 2.0, fh - 14.0 * s),
                    (2.0, fh - 14.0 * s),
                ],
                c3(112, 74, 42),
                ow,
            );
            let mut x = 12.0 * s;
            while x < fw - 4.0 {
                c.line(
                    (x, 16.0 * s),
                    (x, fh - 16.0 * s),
                    1.0,
                    alpha(c3(60, 40, 24), 0.9),
                );
                x += 12.0 * s;
            }
            c.line((4.0, cy), (fw - 4.0, cy), 3.0 * s, c3(60, 58, 56));
            c.disc(fw - 12.0 * s, cy + 6.0 * s, 2.6 * s, c3(200, 160, 70));
        }
        "torch" | "brazier" => {
            if id == "brazier" {
                ring(&mut c, cx, cy, fw * 0.36, c3(62, 60, 58));
                c.disc(cx, cy, fw * 0.28, c3(60, 24, 16));
                for i in 0..10 {
                    c.disc(
                        cx + (hash(i, 1, 6) - 0.5) * fw * 0.4,
                        cy + (hash(i, 2, 6) - 0.5) * fw * 0.4,
                        3.0 * s,
                        mix(c3(200, 70, 20), c3(255, 190, 80), hash(i, 3, 6)),
                    );
                }
            } else {
                c.poly(
                    &[
                        (cx - 4.0 * s, cy + 6.0 * s),
                        (cx + 4.0 * s, cy + 6.0 * s),
                        (cx + 3.0 * s, cy + 22.0 * s),
                        (cx - 3.0 * s, cy + 22.0 * s),
                    ],
                    c3(100, 70, 42),
                    ow * 0.6,
                );
                c.poly(
                    &[
                        (cx - 8.0 * s, cy + 2.0 * s),
                        (cx + 8.0 * s, cy + 2.0 * s),
                        (cx + 6.0 * s, cy + 8.0 * s),
                        (cx - 6.0 * s, cy + 8.0 * s),
                    ],
                    c3(70, 68, 64),
                    ow * 0.6,
                );
            }
            c.blob(
                cx,
                cy - 2.0 * s,
                12.0 * s,
                14.0 * s,
                alpha(c3(255, 140, 40), 0.55),
            );
            c.blob(cx, cy, 6.0 * s, 9.0 * s, alpha(c3(255, 220, 120), 0.9));
        }
        "pillar" => {
            ring(&mut c, cx, cy, fw * 0.4, c3(150, 144, 134));
            for k in 0..12 {
                let a = k as f32 * std::f32::consts::FRAC_PI_6;
                c.disc(
                    cx + a.cos() * fw * 0.33,
                    cy + a.sin() * fw * 0.33,
                    2.5 * s,
                    c3(120, 114, 106),
                );
            }
            c.disc(cx, cy, fw * 0.24, c3(162, 156, 146));
            c.blob(
                cx - 6.0 * s,
                cy - 6.0 * s,
                10.0 * s,
                8.0 * s,
                alpha(c3(255, 250, 240), 0.35),
            );
        }
        "lever" => {
            c.poly(
                &[
                    (cx - 12.0 * s, cy - 6.0 * s),
                    (cx + 12.0 * s, cy - 6.0 * s),
                    (cx + 12.0 * s, cy + 6.0 * s),
                    (cx - 12.0 * s, cy + 6.0 * s),
                ],
                c3(80, 78, 74),
                ow,
            );
            c.line(
                (cx, cy),
                (cx + 12.0 * s, cy - 18.0 * s),
                3.5 * s,
                c3(110, 76, 44),
            );
            ring(
                &mut c,
                cx + 12.0 * s,
                cy - 18.0 * s,
                3.5 * s,
                c3(150, 40, 36),
            );
        }
        "rune_circle" => {
            for (r, w, a) in [(0.44, 3.0, 0.9), (0.34, 1.5, 0.7)] {
                for k in 0..140 {
                    let ang = k as f32 / 140.0 * std::f32::consts::TAU;
                    c.disc(
                        cx + ang.cos() * fw * r,
                        cy + ang.sin() * fw * r,
                        w * s * 0.5,
                        [0.55, 0.85, 1.0, a],
                    );
                }
            }
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                let (x, y) = (cx + a.cos() * fw * 0.39, cy + a.sin() * fw * 0.39);
                c.line(
                    (x - 2.0 * s, y - 2.0 * s),
                    (x + 2.0 * s, y + 2.0 * s),
                    1.4 * s,
                    [0.8, 0.95, 1.0, 1.0],
                );
            }
            c.blob(cx, cy, fw * 0.4, fh * 0.4, [0.4, 0.7, 1.0, 0.25]);
        }
        "bones" => {
            let d = decor(Decor::Bones);
            let dd = d.data.expect("cpu");
            for y in 0..ph {
                for x in 0..pw {
                    let (sx, sy) = (x * 32 / pw, y * 32 / ph);
                    let i = ((sy * 32 + sx) * 4) as usize;
                    c.blend(
                        x as i32,
                        y as i32,
                        [
                            dd[i] as f32 / 255.0,
                            dd[i + 1] as f32 / 255.0,
                            dd[i + 2] as f32 / 255.0,
                            dd[i + 3] as f32 / 255.0,
                        ],
                    );
                }
            }
            ring(
                &mut c,
                cx + 10.0 * s,
                cy - 8.0 * s,
                7.0 * s,
                c3(226, 218, 196),
            );
            c.disc(cx + 8.0 * s, cy - 9.0 * s, 1.6 * s, INK);
            c.disc(cx + 12.0 * s, cy - 9.0 * s, 1.6 * s, INK);
        }
        _ => {
            let [r, g, b] = fallback;
            ring(&mut c, cx, cy, fw.min(fh) * 0.4, c3(r, g, b));
        }
    }
    c.into_image()
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_catalog_object_has_its_own_art() {
        let cat = dnd_world::catalog::Catalog::builtin();
        for a in cat.assets.iter().filter(|a| a.is_object) {
            let art = object(&a.id, a.size.0, a.size.1, a.color);
            let generic = object("__no_such_object__", a.size.0, a.size.1, a.color);
            assert!(
                art.data != generic.data,
                "{} falls back to the generic placeholder",
                a.id
            );
        }
    }

    use super::*;

    #[test]
    fn every_builtin_asset_has_art_of_the_right_size() {
        for a in dnd_world::catalog::Catalog::builtin().assets {
            let img = if a.is_object {
                object(&a.id, a.size.0, a.size.1, a.color)
            } else {
                tile(&a.id, 1, a.color)
            };
            let (w, h) = (img.width(), img.height());
            assert_eq!(
                img.data.as_ref().unwrap().len() as u32,
                w * h * 4,
                "{}",
                a.id
            );
            if !a.is_object {
                assert_eq!((w, h), (T, T));
            }
        }
    }

    #[test]
    fn variants_share_seamless_base_noise() {
        // Opposite edges of the same material must match (tileable), so cells join without seams.
        let img = tile("dirt", 0, [0, 0, 0]);
        let d = img.data.unwrap();
        let px = |x: u32, y: u32| d[((y * T + x) * 4) as usize] as i32;
        let mut diff = 0;
        for y in 0..T {
            diff += (px(0, y) - px(T - 1, y)).abs();
        }
        assert!(diff / (T as i32) < 18, "left/right edge mismatch {diff}");
    }

    #[test]
    fn edge_masks_are_opaque_at_their_edge_and_clear_inside() {
        for dir in 0..8 {
            let m = edge_mask(dir, 1);
            let center = m[(32 * T + 32) as usize];
            assert!(center < 0.05, "dir {dir} centre {center}");
        }
        let n = edge_mask(0, 1);
        assert!(n[(T + 32) as usize] > 0.9);
    }
}
