//! The main menu's animated backdrop, painted procedurally (original artwork, no image files).
//! One fixed composition: a starry sky, a large moon rising behind a castle on a rocky crag in
//! the middle of a mountain lake, pine-covered shores with a few lit cottages, and a dark
//! foreground frame. Only the sky and atmosphere move (twinkling stars, clouds, mist on the
//! water, the moon's reflection, fireflies); the landscape itself never shifts.
//! With Reduced Motion the scene is a still painting.

use crate::theme;
use bevy_egui::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, epaint::Mesh, pos2, vec2};

fn h1(i: u32) -> f32 {
    let mut h = i.wrapping_mul(2_654_435_761) ^ 0x9E37_79B9;
    h = (h ^ (h >> 15)).wrapping_mul(2_246_822_519);
    ((h ^ (h >> 13)) & 0xffff) as f32 / 65535.0
}

fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Vertical gradient through several (fraction, colour) stops.
fn sky_gradient(p: &Painter, r: Rect, stops: &[(f32, Color32)]) {
    let mut mesh = Mesh::default();
    for (i, (f, c)) in stops.iter().enumerate() {
        let y = r.top() + r.height() * f;
        mesh.colored_vertex(pos2(r.left(), y), *c);
        mesh.colored_vertex(pos2(r.right(), y), *c);
        if i > 0 {
            let k = i as u32 * 2;
            mesh.add_triangle(k - 2, k - 1, k);
            mesh.add_triangle(k - 1, k + 1, k);
        }
    }
    p.add(Shape::mesh(mesh));
}

/// Soft radial glow: `color` at the centre fading to transparent at `radius`.
fn glow(p: &Painter, c: Pos2, radius: f32, color: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(c, color);
    let n = 48u32;
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(c + vec2(a.cos(), a.sin()) * radius, Color32::TRANSPARENT);
        if i > 0 {
            mesh.add_triangle(0, i, i + 1);
        }
    }
    p.add(Shape::mesh(mesh));
}

/// A filled silhouette between a curve `y(x)` and the bottom of `r`, shaded top → bottom.
fn silhouette(
    p: &Painter,
    r: Rect,
    floor: f32,
    x0: f32,
    x1: f32,
    y: impl Fn(f32) -> f32,
    top: Color32,
    bottom: Color32,
) {
    let step = 3.0;
    let n = ((x1 - x0) / step).ceil() as u32 + 1;
    let mut mesh = Mesh::default();
    for i in 0..n {
        let x = (x0 + i as f32 * step).min(x1);
        mesh.colored_vertex(pos2(x, y(x)), top);
        mesh.colored_vertex(pos2(x, floor.min(r.bottom())), bottom);
        if i > 0 {
            let k = i * 2;
            mesh.add_triangle(k - 2, k - 1, k);
            mesh.add_triangle(k - 1, k + 1, k);
        }
    }
    p.add(Shape::mesh(mesh));
}

/// Smooth mountain ridge in 0..1 (no high-frequency noise, so edges stay clean).
fn ridge(x: f32, seed: f32) -> f32 {
    let a = (x * 0.0031 + seed).sin() * 0.45;
    let b = (x * 0.0083 + seed * 2.3).sin() * 0.22;
    let c = (x * 0.019 + seed * 5.1).sin() * 0.07;
    (0.5 + a + b + c).clamp(0.0, 1.0)
}

/// Pine: four stacked tiers with a thin moonlit edge on the right.
fn pine(p: &Painter, base: Pos2, h: f32, col: Color32, rim: Color32) {
    let w = h * 0.46;
    p.line_segment(
        [base, pos2(base.x, base.y - h * 0.15)],
        Stroke::new((h * 0.06).max(1.0), col),
    );
    for k in 0..4 {
        let f = k as f32 / 4.0;
        let top = base.y - h * (0.38 + f * 0.62);
        let bottom = base.y - h * (0.1 + f * 0.58);
        let ww = w * (1.0 - f * 0.68);
        let tri = vec![
            pos2(base.x, top),
            pos2(base.x + ww / 2.0, bottom),
            pos2(base.x - ww / 2.0, bottom),
        ];
        p.add(Shape::convex_polygon(tri, col, Stroke::NONE));
        p.line_segment(
            [pos2(base.x, top), pos2(base.x + ww / 2.0, bottom)],
            Stroke::new(1.0, rim),
        );
    }
}

/// Castle silhouette standing on `base` (centre of its foundation).
fn castle(p: &Painter, base: Pos2, s: f32, t: f32) {
    let dark = rgb(15, 14, 22);
    let rim = rgba(170, 186, 225, 120);
    let rect = |x: f32, y: f32, w: f32, h: f32| {
        Rect::from_min_size(base + vec2(x * s, -(y + h) * s), vec2(w * s, h * s))
    };
    let blocks = [
        (-62.0, 0.0, 124.0, 42.0), // curtain wall
        (-22.0, 42.0, 44.0, 62.0), // keep
        (-74.0, 0.0, 24.0, 80.0),  // left tower
        (50.0, 0.0, 24.0, 72.0),   // right tower
        (-9.0, 104.0, 18.0, 26.0), // spire base
    ];
    for (x, y, w, h) in blocks {
        let r = rect(x, y, w, h);
        p.rect_filled(r, 0.0, dark);
        // Moonlight catches the right-hand edges.
        p.line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.2, rim));
    }
    // Battlements on wall and keep.
    for i in 0..6 {
        p.rect_filled(rect(-60.0 + i as f32 * 21.0, 42.0, 10.0, 6.0), 0.0, dark);
    }
    for i in 0..3 {
        p.rect_filled(rect(-22.0 + i as f32 * 17.0, 104.0, 9.0, 6.0), 0.0, dark);
    }
    // Gate.
    p.rect_filled(rect(-9.0, 0.0, 18.0, 20.0), 6.0 * s, rgb(40, 26, 18));
    // Conical roofs.
    for (cx, y, w, h) in [
        (-62.0, 80.0, 30.0, 38.0),
        (62.0, 72.0, 30.0, 34.0),
        (0.0, 130.0, 24.0, 36.0),
    ] {
        let b = base + vec2(cx * s, -y * s);
        let apex = b + vec2(0.0, -h * s);
        p.add(Shape::convex_polygon(
            vec![
                b + vec2(-w * s / 2.0, 0.0),
                b + vec2(w * s / 2.0, 0.0),
                apex,
            ],
            dark,
            Stroke::NONE,
        ));
        p.line_segment([apex, b + vec2(w * s / 2.0, 0.0)], Stroke::new(1.2, rim));
    }
    // Banner on the spire.
    let flag = base + vec2(0.0, -166.0 * s);
    p.line_segment([flag, flag + vec2(0.0, -16.0 * s)], Stroke::new(1.4, dark));
    let wave = (t * 2.5).sin() * 2.0 * s;
    p.add(Shape::convex_polygon(
        vec![
            flag + vec2(0.0, -16.0 * s),
            flag + vec2(16.0 * s, -12.5 * s + wave),
            flag + vec2(0.0, -9.0 * s),
        ],
        theme::BURGUNDY,
        Stroke::NONE,
    ));
    // Warm windows with a gentle flicker.
    for (i, (x, y)) in [
        (-65.0, 54.0),
        (-65.0, 30.0),
        (59.0, 48.0),
        (-8.0, 76.0),
        (4.0, 76.0),
        (-40.0, 20.0),
        (30.0, 20.0),
        (-2.0, 112.0),
    ]
    .iter()
    .enumerate()
    {
        let f = 0.82 + 0.18 * ((t * (1.5 + i as f32 * 0.3)) + i as f32 * 1.7).sin();
        let w = rect(*x, *y, 4.0, 7.0);
        glow(
            p,
            w.center(),
            14.0 * s,
            rgba(255, 150, 60, (60.0 * f) as u8),
        );
        p.rect_filled(w, 1.5, rgba(255, (180.0 * f) as u8, 80, 240));
    }
}

/// Small cottage with a pitched roof, a lit window and a wisp of chimney smoke.
fn cottage(p: &Painter, base: Pos2, s: f32, t: f32, seed: u32) {
    let dark = rgb(14, 15, 20);
    let body = Rect::from_min_size(base + vec2(-14.0 * s, -14.0 * s), vec2(28.0 * s, 14.0 * s));
    p.rect_filled(body, 0.0, dark);
    p.add(Shape::convex_polygon(
        vec![
            base + vec2(-18.0 * s, -14.0 * s),
            base + vec2(18.0 * s, -14.0 * s),
            base + vec2(0.0, -28.0 * s),
        ],
        dark,
        Stroke::NONE,
    ));
    p.rect_filled(
        Rect::from_min_size(base + vec2(8.0 * s, -30.0 * s), vec2(4.0 * s, 10.0 * s)),
        0.0,
        dark,
    );
    let f = 0.85 + 0.15 * (t * 1.3 + seed as f32).sin();
    let win = Rect::from_min_size(base + vec2(-7.0 * s, -10.0 * s), vec2(5.0 * s, 5.0 * s));
    glow(
        p,
        win.center(),
        14.0 * s,
        rgba(255, 160, 70, (70.0 * f) as u8),
    );
    p.rect_filled(win, 1.0, rgba(255, 196, 110, 235));
    // Smoke drifting up and to the right.
    for k in 0..6 {
        let ph = (t * 0.25 + k as f32 / 6.0 + h1(seed)) % 1.0;
        let c = base + vec2(10.0 * s + ph * 26.0 * s, -32.0 * s - ph * 46.0 * s);
        p.circle_filled(
            c,
            (2.5 + ph * 7.0) * s,
            rgba(150, 160, 185, ((1.0 - ph) * 26.0) as u8),
        );
    }
}

/// Paints the full backdrop into `r`. `t` is the animation time in seconds.
pub fn paint(p: &Painter, r: Rect, t: f32) {
    let (w, h) = (r.width(), r.height());
    let x = |f: f32| r.left() + w * f;
    let y = |f: f32| r.top() + h * f;
    let water = y(0.715);

    // Sky: deep indigo down to a violet glow above the mountains.
    sky_gradient(
        p,
        Rect::from_min_max(r.min, pos2(r.right(), water)),
        &[
            (0.0, rgb(5, 8, 20)),
            (0.45, rgb(16, 24, 52)),
            (0.8, rgb(44, 50, 88)),
            (1.0, rgb(74, 66, 104)),
        ],
    );
    // Stars (denser and brighter high up), a few with a cross sparkle.
    for i in 0..220u32 {
        let sx = x(h1(i));
        let fy = h1(i + 500).powf(1.4) * 0.6;
        let tw = 0.6 + 0.4 * (t * (0.6 + h1(i + 900) * 1.8) + i as f32).sin();
        let a = (tw * 230.0 * (1.0 - fy / 0.62)).clamp(0.0, 255.0) as u8;
        let size = 0.5 + h1(i + 77).powi(3) * 1.6;
        p.circle_filled(pos2(sx, y(fy)), size, rgba(240, 236, 220, a));
        if size > 1.6 {
            let c = pos2(sx, y(fy));
            let st = Stroke::new(0.7, rgba(240, 236, 220, a / 3));
            p.line_segment([c - vec2(5.0, 0.0), c + vec2(5.0, 0.0)], st);
            p.line_segment([c - vec2(0.0, 5.0), c + vec2(0.0, 5.0)], st);
        }
    }

    // Moon rising behind the castle.
    let moon = pos2(x(0.68), y(0.26));
    let mr = h * 0.095;
    glow(p, moon, mr * 6.0, rgba(150, 160, 210, 34));
    glow(p, moon, mr * 2.4, rgba(220, 220, 235, 50));
    p.circle_filled(moon, mr, rgb(236, 232, 214));
    // Soft maria, then a terminator shade on the lower left for roundness.
    for (dx, dy, cr) in [
        (-0.35, -0.15, 0.28),
        (0.2, 0.25, 0.2),
        (0.3, -0.3, 0.14),
        (-0.1, 0.45, 0.12),
    ] {
        p.circle_filled(moon + vec2(dx, dy) * mr, cr * mr, rgba(200, 196, 180, 32));
    }
    p.add(Shape::circle_stroke(
        moon,
        mr - 1.0,
        Stroke::new(2.0, rgba(255, 252, 235, 90)),
    ));

    // Thin clouds sliding slowly across the sky (they wrap around).
    for i in 0..7u32 {
        let speed = 3.0 + h1(i + 40) * 5.0;
        let cx = r.left() + ((h1(i) * (w + 600.0) + t * speed) % (w + 600.0)) - 300.0;
        let cy = y(0.1 + h1(i + 7) * 0.32);
        let len = 160.0 + h1(i + 11) * 220.0;
        for j in 0..5 {
            let o = j as f32 / 4.0 - 0.5;
            p.add(Shape::ellipse_filled(
                pos2(cx + o * len, cy + (j as f32 * 1.7).sin() * 4.0),
                vec2(len * 0.35, 9.0 + h1(i * 5 + j) * 8.0),
                rgba(120, 124, 160, 14),
            ));
        }
    }

    // Distant mountains: two static ranges with aerial perspective, snow-lit crests.
    let far = |px: f32| y(0.64) - ridge(px - r.left(), 1.3) * h * 0.24;
    silhouette(
        p,
        r,
        r.bottom(),
        r.left(),
        r.right(),
        far,
        rgb(82, 90, 136),
        rgb(46, 52, 86),
    );
    let crest = Stroke::new(1.6, rgba(205, 212, 240, 90));
    let pts: Vec<Pos2> = (0..=(w / 4.0) as u32)
        .map(|i| pos2(r.left() + i as f32 * 4.0, far(r.left() + i as f32 * 4.0)))
        .collect();
    p.add(Shape::line(pts, crest));
    let mid = |px: f32| y(0.69) - ridge(px - r.left(), 4.2) * h * 0.14;
    silhouette(
        p,
        r,
        r.bottom(),
        r.left(),
        r.right(),
        mid,
        rgb(44, 50, 82),
        rgb(28, 32, 54),
    );

    // Lake: reflects the sky colours, darker towards the viewer.
    sky_gradient(
        p,
        Rect::from_min_max(pos2(r.left(), water), r.max),
        &[
            (0.0, rgb(62, 60, 96)),
            (0.25, rgb(30, 36, 64)),
            (1.0, rgb(10, 12, 22)),
        ],
    );
    p.hline(
        r.x_range(),
        water,
        Stroke::new(1.0, rgba(190, 196, 230, 60)),
    );
    // Moon path on the water: shimmering horizontal strokes.
    for i in 0..34u32 {
        let fy = i as f32 / 34.0;
        let yy = water + 4.0 + fy * h * 0.2;
        let spread = mr * (0.5 + fy * 1.6);
        let ph = (t * 1.4 + i as f32 * 0.9).sin();
        let off = ph * spread * 0.25;
        let len = spread * (0.4 + 0.5 * h1(i + 600));
        let a = ((1.0 - fy) * 150.0 * (0.6 + 0.4 * ph.abs())) as u8;
        p.line_segment(
            [pos2(moon.x + off - len, yy), pos2(moon.x + off + len, yy)],
            Stroke::new(1.5 + fy * 2.0, rgba(236, 230, 205, a)),
        );
    }

    // The crag with the castle, rising out of the lake.
    let cs = (h / 900.0).max(0.6) * 1.4;
    let top_y = y(0.565);
    let crag = |px: f32| {
        let u = (px - x(0.67)) / (w * 0.105);
        if u.abs() < 0.55 {
            top_y + (u * 9.0).sin().abs() * 3.0
        } else {
            let k = ((u.abs() - 0.55) / 0.45).min(1.0);
            top_y
                + k.powf(1.3) * (water - top_y + 10.0)
                + ((u * 13.0).sin() * 5.0 + (u * 31.0).sin() * 2.5) * k
        }
    };
    silhouette(
        p,
        r,
        water + 2.0,
        x(0.565),
        x(0.775),
        crag,
        rgb(26, 26, 38),
        rgb(16, 16, 24),
    );
    // Rim light on the crag's top edge.
    let pts: Vec<Pos2> = (0..=60)
        .map(|i| {
            let px = x(0.6) + i as f32 / 60.0 * w * 0.14;
            pos2(px, crag(px))
        })
        .collect();
    p.add(Shape::line(pts, Stroke::new(1.3, rgba(170, 180, 220, 80))));
    // Rock ledges: short moonlit strokes and dark cracks across the crag face.
    for i in 0..18u32 {
        let fx = 0.585 + h1(i + 700) * 0.17;
        let px = x(fx);
        let top = crag(px);
        if top > water - 6.0 {
            continue;
        }
        let ly = top + (water - top) * (0.15 + h1(i + 710) * 0.75);
        let len = 8.0 + h1(i + 720) * 22.0;
        let lit = fx > 0.67;
        p.line_segment(
            [
                pos2(px - len * 0.5, ly),
                pos2(px + len * 0.5, ly - len * 0.25),
            ],
            Stroke::new(
                1.2,
                if lit {
                    rgba(150, 160, 205, 45)
                } else {
                    rgba(8, 8, 14, 120)
                },
            ),
        );
    }
    castle(p, pos2(x(0.67), top_y + 2.0), cs, t);
    // Lantern-lit path climbing the crag from the left shore.
    for i in 0..7u32 {
        let f = i as f32 / 6.0;
        let lx = x(0.585 + f * 0.055);
        let ly = crag(lx) - 3.0;
        let fl = 0.8 + 0.2 * (t * 2.0 + i as f32).sin();
        glow(p, pos2(lx, ly), 9.0, rgba(255, 160, 60, (70.0 * fl) as u8));
        p.circle_filled(pos2(lx, ly), 1.6, rgba(255, 210, 130, 240));
    }
    // Castle reflection: a few broken dark strokes under the crag.
    for i in 0..10u32 {
        let yy = water + 3.0 + i as f32 * 4.0;
        let half = w * 0.1 * (1.0 - i as f32 / 12.0);
        let off = (t * 1.1 + i as f32).sin() * 3.0;
        p.line_segment(
            [
                pos2(x(0.67) - half + off, yy),
                pos2(x(0.67) + half + off, yy),
            ],
            Stroke::new(2.0, rgba(14, 14, 24, 120)),
        );
    }

    // Wooded shores left and right of the lake.
    let left_shore = |px: f32| {
        let u = (px - r.left()) / (w * 0.52);
        water - (1.0 - u.clamp(0.0, 1.0)).powf(0.8) * h * 0.11 - (px * 0.013).sin() * 6.0
    };
    let right_shore = |px: f32| {
        let u = (r.right() - px) / (w * 0.2);
        water - (1.0 - u.clamp(0.0, 1.0)).powf(0.7) * h * 0.12 - (px * 0.017).sin() * 5.0
    };
    silhouette(
        p,
        r,
        water + 2.0,
        r.left(),
        x(0.54),
        left_shore,
        rgb(20, 26, 36),
        rgb(10, 12, 18),
    );
    silhouette(
        p,
        r,
        water + 2.0,
        x(0.79),
        r.right(),
        right_shore,
        rgb(20, 26, 36),
        rgb(10, 12, 18),
    );
    let rim = rgba(130, 150, 200, 60);
    for i in 0..60u32 {
        let fx = h1(i * 3 + 11);
        let px = x(fx * 0.5);
        let ph = h * (0.03 + 0.03 * h1(i + 97));
        pine(p, pos2(px, left_shore(px) + 4.0), ph, rgb(16, 22, 30), rim);
    }
    for i in 0..26u32 {
        let px = x(0.81 + h1(i * 5 + 3) * 0.19);
        let ph = h * (0.03 + 0.03 * h1(i + 41));
        pine(p, pos2(px, right_shore(px) + 4.0), ph, rgb(16, 22, 30), rim);
    }
    for (k, fx) in [0.835f32, 0.875].iter().enumerate() {
        let px = x(*fx);
        cottage(
            p,
            pos2(px, right_shore(px) + 2.0),
            cs * 0.75,
            t,
            k as u32 + 1,
        );
    }

    // Mist hanging over the water.
    for i in 0..10u32 {
        let speed = 5.0 + h1(i + 300) * 8.0;
        let mx = r.left() + ((h1(i + 100) * (w + 800.0) + t * speed) % (w + 800.0)) - 400.0;
        let my = water + h * (0.005 + h1(i + 200) * 0.06);
        p.add(Shape::ellipse_filled(
            pos2(mx, my),
            vec2(260.0 + h1(i + 7) * 200.0, 14.0 + h1(i + 9) * 12.0),
            rgba(170, 178, 215, 16),
        ));
    }

    // Foreground frame: dark hills with big pines on both sides.
    let fg = |px: f32| {
        let u = (px - r.left()) / w;
        let edge = ((u - 0.5).abs() * 2.0).powf(3.0);
        y(0.97) - edge * h * 0.3 - (px * 0.009).sin() * 8.0
    };
    silhouette(
        p,
        r,
        r.bottom(),
        r.left(),
        r.right(),
        fg,
        rgb(9, 10, 15),
        rgb(5, 5, 8),
    );
    let fg_rim = rgba(120, 140, 190, 45);
    for (i, fx) in [0.02f32, 0.06, 0.11, 0.9, 0.95, 0.985].iter().enumerate() {
        let px = x(*fx);
        let ph = h * (0.2 + 0.08 * h1(i as u32 + 31));
        pine(p, pos2(px, fg(px) + 10.0), ph, rgb(7, 8, 12), fg_rim);
    }

    // Fireflies drifting near the shore.
    for i in 0..36u32 {
        let fx = h1(i + 3000);
        let base_y = y(0.74 + h1(i + 2000) * 0.2);
        let px = x(fx) + (t * 0.35 + i as f32).sin() * 18.0;
        let py = base_y + (t * 0.5 + i as f32 * 2.1).cos() * 10.0;
        let blink = (0.5 + 0.5 * (t * (0.9 + h1(i) * 0.8) + i as f32 * 1.3).sin()).powi(2);
        glow(
            p,
            pos2(px, py),
            7.0,
            rgba(230, 220, 120, (60.0 * blink) as u8),
        );
        p.circle_filled(
            pos2(px, py),
            1.3,
            rgba(250, 245, 170, (230.0 * blink) as u8),
        );
    }

    // Readability shade behind the menu column, then the vignette.
    let mut mesh = Mesh::default();
    let cw = 700.0f32.min(w * 0.6);
    let dark = Color32::from_black_alpha(170);
    mesh.colored_vertex(r.left_top(), dark);
    mesh.colored_vertex(pos2(r.left() + cw, r.top()), Color32::TRANSPARENT);
    mesh.colored_vertex(pos2(r.left() + cw, r.bottom()), Color32::TRANSPARENT);
    mesh.colored_vertex(r.left_bottom(), dark);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(Shape::mesh(mesh));
    theme::paint_vignette(p, r, h * 0.25, Color32::from_black_alpha(170));
}
