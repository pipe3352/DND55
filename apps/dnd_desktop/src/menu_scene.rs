//! The main menu's animated backdrop, painted procedurally (original artwork, no image files):
//! a moonlit night sky with twinkling stars, three parallax mountain ranges, a ruined castle with
//! flickering windows, drifting mist and rising embers, finished with a vignette.
//! With Reduced Motion the scene is a still painting.

use crate::theme;
use bevy_egui::egui::{self, Color32, Painter, Pos2, Rect, Shape, epaint::Mesh, pos2, vec2};

fn h1(i: u32) -> f32 {
    let mut h = i.wrapping_mul(2_654_435_761) ^ 0x9E37_79B9;
    h = (h ^ (h >> 15)).wrapping_mul(2_246_822_519);
    ((h ^ (h >> 13)) & 0xffff) as f32 / 65535.0
}

/// Smooth ridge height in 0..1 for a mountain layer.
fn ridge(x: f32, seed: f32) -> f32 {
    let a = (x * 0.0042 + seed).sin() * 0.5;
    let b = (x * 0.011 + seed * 2.3).sin() * 0.25;
    let c = (x * 0.027 + seed * 5.1).sin() * 0.1;
    let d = ((x * 0.061 + seed * 7.7).sin()).abs() * 0.06; // jagged peaks
    0.5 + a + b + c + d
}

fn mountain_layer(
    p: &Painter,
    r: Rect,
    base_y: f32,
    amp: f32,
    seed: f32,
    offset: f32,
    top: Color32,
    bottom: Color32,
) {
    let step = 12.0;
    let n = (r.width() / step) as usize + 2;
    let mut mesh = Mesh::default();
    for i in 0..n {
        let x = r.left() + i as f32 * step;
        let y = base_y - ridge(x + offset, seed) * amp;
        mesh.colored_vertex(pos2(x, y), top);
        mesh.colored_vertex(pos2(x, r.bottom()), bottom);
        if i > 0 {
            let k = (i as u32) * 2;
            mesh.add_triangle(k - 2, k - 1, k);
            mesh.add_triangle(k - 1, k + 1, k);
        }
    }
    p.add(Shape::mesh(mesh));
}

fn castle(p: &Painter, base: Pos2, s: f32, t: f32) {
    let dark = Color32::from_rgb(14, 12, 14);
    let rect = |x: f32, y: f32, w: f32, h: f32| {
        Rect::from_min_size(base + vec2(x * s, -(y + h) * s), vec2(w * s, h * s))
    };
    // Keep, towers and curtain wall.
    for (x, y, w, h) in [
        (-60.0, 0.0, 120.0, 40.0),
        (-20.0, 40.0, 40.0, 60.0),
        (-70.0, 0.0, 22.0, 78.0),
        (48.0, 0.0, 22.0, 70.0),
        (-8.0, 100.0, 16.0, 24.0),
    ] {
        p.rect_filled(rect(x, y, w, h), 0.0, dark);
    }
    // Battlements.
    for i in 0..6 {
        p.rect_filled(rect(-60.0 + i as f32 * 21.0, 40.0, 10.0, 6.0), 0.0, dark);
    }
    // Pointed roofs.
    for (cx, y, w, h) in [
        (-59.0, 78.0, 26.0, 34.0),
        (59.0, 70.0, 26.0, 30.0),
        (0.0, 124.0, 20.0, 30.0),
    ] {
        let b = base + vec2(cx * s, -y * s);
        p.add(Shape::convex_polygon(
            vec![
                b + vec2(-w * s / 2.0, 0.0),
                b + vec2(w * s / 2.0, 0.0),
                b + vec2(0.0, -h * s),
            ],
            dark,
            egui::Stroke::NONE,
        ));
    }
    // Banner on the spire.
    let flag = base + vec2(0.0, -154.0 * s);
    p.line_segment(
        [flag, flag + vec2(0.0, -14.0 * s)],
        egui::Stroke::new(1.2, dark),
    );
    let wave = (t * 3.0).sin() * 2.0 * s;
    p.add(Shape::convex_polygon(
        vec![
            flag + vec2(0.0, -14.0 * s),
            flag + vec2(14.0 * s, -11.0 * s + wave),
            flag + vec2(0.0, -7.0 * s),
        ],
        theme::BURGUNDY,
        egui::Stroke::NONE,
    ));
    // Lit windows, flickering like firelight.
    for (i, (x, y)) in [
        (-63.0, 50.0),
        (-63.0, 30.0),
        (55.0, 45.0),
        (-6.0, 70.0),
        (6.0, 70.0),
        (-35.0, 18.0),
        (25.0, 18.0),
        (-2.0, 108.0),
    ]
    .iter()
    .enumerate()
    {
        let f = 0.75
            + 0.25 * ((t * (5.0 + i as f32)) + i as f32 * 1.7).sin() * ((t * 2.3 + i as f32).cos());
        let c = Color32::from_rgba_unmultiplied(255, (170.0 * f) as u8, 70, (220.0 * f) as u8);
        let w = rect(*x, *y, 4.0, 7.0);
        p.circle_filled(
            w.center(),
            9.0 * s,
            Color32::from_rgba_unmultiplied(255, 150, 60, (30.0 * f) as u8),
        );
        p.rect_filled(w, 1.0, c);
    }
}

fn dead_tree(p: &Painter, root: Pos2, h: f32, seed: u32) {
    let c = Color32::from_rgb(10, 9, 9);
    fn branch(
        p: &Painter,
        from: Pos2,
        angle: f32,
        len: f32,
        w: f32,
        depth: u32,
        seed: u32,
        c: Color32,
    ) {
        let to = from + vec2(angle.cos(), angle.sin()) * len;
        p.line_segment([from, to], egui::Stroke::new(w, c));
        if depth == 0 {
            return;
        }
        let spread = 0.35 + h1(seed) * 0.4;
        branch(
            p,
            to,
            angle - spread,
            len * 0.68,
            w * 0.62,
            depth - 1,
            seed.wrapping_mul(31).wrapping_add(1),
            c,
        );
        branch(
            p,
            to,
            angle + spread * 0.8,
            len * 0.62,
            w * 0.6,
            depth - 1,
            seed.wrapping_mul(17).wrapping_add(7),
            c,
        );
    }
    branch(
        p,
        root,
        -std::f32::consts::FRAC_PI_2 + (h1(seed) - 0.5) * 0.2,
        h * 0.38,
        h * 0.06,
        4,
        seed,
        c,
    );
}

/// Paints the full backdrop into `r`. `t` is the animation time in seconds.
pub fn paint(p: &Painter, r: Rect, t: f32) {
    let (w, h) = (r.width(), r.height());
    // Sky.
    let horizon = r.top() + h * 0.62;
    theme::paint_gradient(
        p,
        Rect::from_min_max(r.min, pos2(r.right(), horizon)),
        Color32::from_rgb(9, 11, 18),
        Color32::from_rgb(46, 36, 38),
    );
    theme::paint_gradient(
        p,
        Rect::from_min_max(pos2(r.left(), horizon), r.max),
        Color32::from_rgb(46, 36, 38),
        Color32::from_rgb(20, 16, 15),
    );
    // Stars.
    for i in 0..140u32 {
        let x = r.left() + h1(i) * w;
        let y = r.top() + h1(i + 500) * h * 0.55;
        let tw = 0.55 + 0.45 * (t * (0.8 + h1(i + 900) * 2.0) + i as f32).sin();
        let a = (tw * 200.0 * (1.0 - (y - r.top()) / (h * 0.6))).clamp(0.0, 255.0) as u8;
        p.circle_filled(
            pos2(x, y),
            0.6 + h1(i + 77) * 1.1,
            Color32::from_rgba_unmultiplied(235, 230, 210, a),
        );
    }
    // Moon with layered glow and craters.
    let moon = pos2(r.left() + w * 0.76, r.top() + h * 0.2);
    let mr = h * 0.07;
    for k in (1..24).rev() {
        let f = k as f32 / 24.0;
        p.circle_filled(
            moon,
            mr * (1.0 + f * 4.0),
            Color32::from_rgba_unmultiplied(
                200,
                210,
                230,
                (3.0 + 5.0 * (1.0 - f) * (1.0 - f)) as u8,
            ),
        );
    }
    p.circle_filled(moon, mr, Color32::from_rgb(232, 228, 210));
    for (dx, dy, cr) in [
        (-0.3, -0.2, 0.22),
        (0.25, 0.15, 0.16),
        (0.05, 0.4, 0.12),
        (-0.15, 0.3, 0.08),
    ] {
        p.circle_filled(
            moon + vec2(dx, dy) * mr,
            cr * mr,
            Color32::from_rgb(208, 203, 186),
        );
    }
    // Drifting cloud wisps across the moon.
    for i in 0..9u32 {
        let speed = 6.0 + h1(i + 40) * 10.0;
        let x = r.left() + ((h1(i) * w + t * speed) % (w + 400.0)) - 200.0;
        let y = r.top() + h * (0.12 + h1(i + 7) * 0.3);
        for j in 0..7 {
            p.circle_filled(
                pos2(x + j as f32 * 26.0, y + ((j as f32) * 1.3).sin() * 6.0),
                22.0 + h1(i * 7 + j) * 14.0,
                Color32::from_rgba_unmultiplied(70, 72, 90, 9),
            );
        }
    }
    // Mountains (far → near), slow parallax drift.
    let drift = t * 4.0;
    mountain_layer(
        p,
        r,
        r.top() + h * 0.66,
        h * 0.32,
        1.3,
        drift * 0.3,
        Color32::from_rgb(44, 48, 62),
        Color32::from_rgb(34, 33, 40),
    );
    mountain_layer(
        p,
        r,
        r.top() + h * 0.74,
        h * 0.28,
        4.2,
        drift * 0.6,
        Color32::from_rgb(31, 30, 37),
        Color32::from_rgb(24, 21, 22),
    );
    // Castle on the near ridge.
    castle(
        p,
        pos2(r.left() + w * 0.66, r.top() + h * 0.64),
        (h / 900.0).max(0.6),
        t,
    );
    mountain_layer(
        p,
        r,
        r.top() + h * 0.86,
        h * 0.2,
        8.7,
        drift,
        Color32::from_rgb(19, 17, 17),
        Color32::from_rgb(12, 10, 10),
    );
    for (i, fx) in [0.08, 0.52, 0.9].iter().enumerate() {
        let x = r.left() + w * fx;
        dead_tree(
            p,
            pos2(
                x,
                r.top() + h * 0.86 - ridge(x + drift, 8.7) * h * 0.2 + 8.0,
            ),
            h * 0.22,
            i as u32 * 7 + 3,
        );
    }
    // Low mist.
    for i in 0..14u32 {
        let speed = 10.0 + h1(i + 300) * 14.0;
        let x = r.left() + ((h1(i + 100) * w + t * speed) % (w + 500.0)) - 250.0;
        let y = r.top() + h * (0.78 + h1(i + 200) * 0.18);
        for j in 0..6 {
            p.circle_filled(
                pos2(x + j as f32 * 40.0, y + (j as f32 * 0.9).sin() * 8.0),
                50.0 + h1(i * 9 + j) * 30.0,
                Color32::from_rgba_unmultiplied(120, 120, 135, 7),
            );
        }
    }
    // Rising embers.
    for i in 0..70u32 {
        let speed = 18.0 + h1(i + 1000) * 40.0;
        let life = h * 0.7;
        let y0 = (h1(i + 2000) * life + t * speed) % life;
        let y = r.bottom() - y0;
        let x = r.left() + h1(i + 3000) * w + (t * 0.7 + i as f32).sin() * 14.0;
        let fade = 1.0 - y0 / life;
        let flick = 0.6 + 0.4 * (t * 9.0 + i as f32 * 2.1).sin();
        let a = (fade * flick * 230.0) as u8;
        p.circle_filled(
            pos2(x, y),
            4.0,
            Color32::from_rgba_unmultiplied(255, 120, 40, a / 8),
        );
        p.circle_filled(
            pos2(x, y),
            1.2 + h1(i) * 1.2,
            Color32::from_rgba_unmultiplied(255, 190, 90, a),
        );
    }
    // Readability shade behind the menu column, then the vignette.
    let mut mesh = Mesh::default();
    let cw = 640.0f32.min(w * 0.55);
    let dark = Color32::from_black_alpha(185);
    mesh.colored_vertex(r.left_top(), dark);
    mesh.colored_vertex(pos2(r.left() + cw, r.top()), Color32::TRANSPARENT);
    mesh.colored_vertex(pos2(r.left() + cw, r.bottom()), Color32::TRANSPARENT);
    mesh.colored_vertex(r.left_bottom(), dark);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(Shape::mesh(mesh));
    theme::paint_vignette(p, r, h * 0.28, Color32::from_black_alpha(190));
}
