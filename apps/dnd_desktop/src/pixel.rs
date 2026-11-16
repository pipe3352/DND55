//! Tiny pixel-art engine: a low-resolution RGBA canvas with 3-tone colour ramps, hard-edged
//! primitives, automatic ink outline and ground shadow. Everything is drawn at native resolution
//! (32 px per map cell) and shown with nearest-neighbour sampling, so pixels stay crisp.
//!
//! Art convention (all sprites): light from the top-left, 1 px dark outline, 3 tones per material.

use bevy::image::{Image, ImageSampler};

pub type C = [u8; 4];
pub const INK: C = [34, 24, 30, 255];
pub const CLEAR: C = [0, 0, 0, 0];

pub const fn c(r: u8, g: u8, b: u8) -> C {
    [r, g, b, 255]
}

/// Light / mid / dark tones of one material.
#[derive(Clone, Copy, PartialEq)]
pub struct R {
    pub l: C,
    pub m: C,
    pub d: C,
}
/// Ramp from a base colour: warm highlight, cool shadow.
pub const fn ramp(r: u8, g: u8, b: u8) -> R {
    const fn mx(x: u8, y: u8, num: u16) -> u8 {
        ((x as u16 * (100 - num) + y as u16 * num) / 100) as u8
    }
    R {
        l: [mx(r, 255, 34), mx(g, 246, 34), mx(b, 214, 34), 255],
        m: [r, g, b, 255],
        d: [mx(r, 36, 42), mx(g, 22, 42), mx(b, 56, 42), 255],
    }
}

pub const STEEL: R = ramp(146, 158, 176);
pub const IRON: R = ramp(92, 98, 112);
pub const GOLD: R = ramp(222, 172, 62);
pub const WOOD: R = ramp(134, 86, 50);
pub const WOOD_D: R = ramp(96, 60, 38);
pub const BONE: R = ramp(222, 214, 190);
pub const STONE: R = ramp(128, 126, 124);
pub const LEAF: R = ramp(74, 128, 60);
pub const WHITE: C = c(250, 248, 240);

#[derive(Clone)]
pub struct Pix {
    pub w: i32,
    pub h: i32,
    pub px: Vec<C>,
}

impl Pix {
    pub fn new(w: i32, h: i32) -> Self {
        Pix {
            w,
            h,
            px: vec![CLEAR; (w * h) as usize],
        }
    }
    pub fn get(&self, x: i32, y: i32) -> C {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            CLEAR
        } else {
            self.px[(y * self.w + x) as usize]
        }
    }
    pub fn set(&mut self, x: i32, y: i32, col: C) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize] = col;
        }
    }
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, col: C) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, col);
            }
        }
    }
    /// Shaded box: light top/left edge, dark bottom/right edge.
    pub fn srect(&mut self, x: i32, y: i32, w: i32, h: i32, r: R) {
        self.rect(x, y, w, h, r.m);
        if w > 2 && h > 2 {
            self.rect(x, y, w - 1, 1, r.l);
            self.rect(x, y, 1, h - 1, r.l);
        }
        self.rect(x, y + h - 1, w, 1, r.d);
        self.rect(x + w - 1, y, 1, h, r.d);
    }
    /// Hard-edged filled ellipse.
    #[allow(dead_code)]
    pub fn ell(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, col: C) {
        self.ell_with(cx, cy, rx, ry, |_| col);
    }
    /// Shaded ellipse (sphere-like): light upper-left, dark lower-right.
    pub fn sell(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, r: R) {
        self.ell_with(cx, cy, rx, ry, |t| {
            if t < -0.38 {
                r.l
            } else if t > 0.34 {
                r.d
            } else {
                r.m
            }
        });
    }
    fn ell_with(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, f: impl Fn(f32) -> C) {
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                if dx * dx + dy * dy <= 1.0 {
                    self.set(x, y, f((dx + dy) * 0.5));
                }
            }
        }
    }
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, col: C) {
        let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for i in 0..=n {
            let t = i as f32 / n as f32;
            self.set(
                (x0 as f32 + (x1 - x0) as f32 * t).round() as i32,
                (y0 as f32 + (y1 - y0) as f32 * t).round() as i32,
                col,
            );
        }
    }
    /// Filled polygon (even-odd, sampled at pixel centres).
    pub fn poly(&mut self, pts: &[(i32, i32)], col: C) {
        let (y0, y1) = (
            pts.iter().map(|p| p.1).min().unwrap_or(0),
            pts.iter().map(|p| p.1).max().unwrap_or(0),
        );
        let (x0, x1) = (
            pts.iter().map(|p| p.0).min().unwrap_or(0),
            pts.iter().map(|p| p.0).max().unwrap_or(0),
        );
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.01, y as f32 + 0.01);
                let mut inside = false;
                for i in 0..pts.len() {
                    let (a, b) = (pts[i], pts[(i + pts.len() - 1) % pts.len()]);
                    let (ax, ay, bx, by) = (a.0 as f32, a.1 as f32, b.0 as f32, b.1 as f32);
                    if (ay > py) != (by > py) && px < (bx - ax) * (py - ay) / (by - ay) + ax {
                        inside = !inside;
                    }
                }
                if inside {
                    self.set(x, y, col);
                }
            }
        }
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            self.line(a.0, a.1, b.0, b.1, col);
        }
    }
    /// 1 px ink outline around everything drawn so far.
    pub fn outline(&mut self) {
        let src = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                if src.get(x, y)[3] == 0
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .any(|(dx, dy)| src.get(x + dx, y + dy)[3] > 0)
                {
                    self.set(x, y, INK);
                }
            }
        }
    }
    /// Soft ground shadow under the sprite's feet (only on empty pixels).
    pub fn ground_shadow(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) {
        for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
            for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
                let (dx, dy) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
                if dx * dx + dy * dy <= 1.0 && self.get(x, y)[3] == 0 {
                    self.set(x, y, [20, 12, 24, 96]);
                }
            }
        }
    }
    pub fn mirror(&self) -> Pix {
        let mut o = Pix::new(self.w, self.h);
        for y in 0..self.h {
            for x in 0..self.w {
                o.set(self.w - 1 - x, y, self.get(x, y));
            }
        }
        o
    }
    /// Draws `o` over this canvas at (dx, dy), skipping empty pixels.
    pub fn blit(&mut self, o: &Pix, dx: i32, dy: i32) {
        for y in 0..o.h {
            for x in 0..o.w {
                let p = o.get(x, y);
                if p[3] > 0 {
                    self.set(x + dx, y + dy, p);
                }
            }
        }
    }
    /// Multiplies every opaque pixel's colour (tinting, e.g. spectral creatures).
    pub fn tint(&mut self, k: [f32; 3], alpha: u8) {
        for p in &mut self.px {
            if p[3] > 0 {
                for i in 0..3 {
                    p[i] = (p[i] as f32 * k[i]).min(255.0) as u8;
                }
                p[3] = ((p[3] as u16 * alpha as u16) / 255) as u8;
            }
        }
    }
    pub fn rgba(&self) -> Vec<u8> {
        self.px.iter().flatten().copied().collect()
    }
    pub fn into_image(self) -> Image {
        nearest(crate::textures::new_image(
            self.w as u32,
            self.h as u32,
            self.rgba(),
        ))
    }
}

pub fn nearest(mut img: Image) -> Image {
    img.sampler = ImageSampler::nearest();
    img
}

/// Converts a smooth procedural image into pixel art: 2×2 downsample, hard (optionally dithered)
/// alpha, reduced colour depth, nearest sampling. Keeps tile seams intact.
pub fn pixelize(img: &Image, dither: bool) -> Image {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let Some(src) = img.data.as_ref() else {
        return img.clone();
    };
    let (nw, nh) = (w / 2, h / 2);
    const BAYER: [[f32; 4]; 4] = [
        [0.03, 0.53, 0.16, 0.66],
        [0.78, 0.28, 0.91, 0.41],
        [0.22, 0.72, 0.09, 0.59],
        [0.97, 0.47, 0.84, 0.34],
    ];
    let mut out = Vec::with_capacity(nw * nh * 4);
    for y in 0..nh {
        for x in 0..nw {
            let (mut acc, mut a) = ([0.0f32; 3], 0.0f32);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let i = ((y * 2 + dy) * w + x * 2 + dx) * 4;
                let pa = src[i + 3] as f32 / 255.0;
                for k in 0..3 {
                    acc[k] += src[i + k] as f32 * pa;
                }
                a += pa;
            }
            let alpha = a / 4.0;
            let cut = if dither { BAYER[y % 4][x % 4] } else { 0.45 };
            if alpha <= cut || a <= 0.0 {
                out.extend_from_slice(&CLEAR);
            } else {
                for k in acc {
                    // 5 levels per 64 → 20 tones per channel: flat, readable colour areas.
                    let v = k / a;
                    out.push(((v / 13.0).round() * 13.0).min(255.0) as u8);
                }
                out.push(255);
            }
        }
    }
    nearest(crate::textures::new_image(nw as u32, nh as u32, out))
}
