//! "Dark gothic" interface chrome: dark wood panels with gold borders, tile buttons, ornate tabs,
//! status pills, gold action buttons, red gem dice and painted decorations (dragon crest, hanging
//! banner, candle on books, skull, wax seal, compass). All original vector art painted with egui.

use crate::icons::{self, Icon};
use crate::theme::{self, BRONZE, GOLD, GOLD_BRIGHT, INK, IVORY, Materials};
use bevy_egui::egui::{
    self, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, pos2,
    vec2,
};
use std::f32::consts::{PI, TAU};

pub const DARK: Color32 = Color32::from_rgb(0x1D, 0x16, 0x12);
pub const DARK_2: Color32 = Color32::from_rgb(0x2A, 0x1F, 0x18);
pub const RED: Color32 = Color32::from_rgb(0x8E, 0x1E, 0x1B);
pub const RED_DARK: Color32 = Color32::from_rgb(0x55, 0x10, 0x10);

fn a(c: Color32, k: f32) -> Color32 {
    c.gamma_multiply(k)
}

// ---------------------------------------------------------------- panels and frames

/// Dark wood panel with a double gold border and corner fleurons. Call first in a panel closure,
/// together with [`theme::dark_scope`].
pub fn dark_panel_bg(ui: &Ui) {
    let rect = ui.max_rect().expand(8.0);
    let p = ui.painter().with_clip_rect(rect.expand(1.0));
    let m = Materials::get(ui.ctx());
    theme::paint_material(&p, rect, &m.wood, Color32::from_gray(120));
    theme::paint_vignette(&p, rect, 40.0, Color32::from_black_alpha(120));
    gold_frame(&p, rect.shrink(2.0), 1.0);
}

/// Double gold rule with diamond corners (panels, map viewport, minimap).
pub fn gold_frame(p: &Painter, r: Rect, scale: f32) {
    p.rect_stroke(
        r,
        3.0,
        Stroke::new(2.0 * scale, a(GOLD, 0.9)),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        r.shrink(4.0 * scale),
        2.0,
        Stroke::new(0.8 * scale, a(BRONZE, 0.9)),
        egui::StrokeKind::Inside,
    );
    for c in [
        r.left_top(),
        r.right_top(),
        r.left_bottom(),
        r.right_bottom(),
    ] {
        let d = 5.0 * scale;
        p.add(Shape::convex_polygon(
            vec![
                c + vec2(0.0, -d),
                c + vec2(d, 0.0),
                c + vec2(0.0, d),
                c + vec2(-d, 0.0),
            ],
            GOLD_BRIGHT,
            Stroke::new(1.0, INK),
        ));
    }
}

/// Ornate frame around the map canvas: a dark band, gold rules and fleurons at the midpoints.
pub fn map_frame(p: &Painter, r: Rect) {
    let band = 9.0;
    for (min, max) in [
        (r.left_top(), pos2(r.right(), r.top() + band)),
        (pos2(r.left(), r.bottom() - band), r.right_bottom()),
        (r.left_top(), pos2(r.left() + band, r.bottom())),
        (pos2(r.right() - band, r.top()), r.right_bottom()),
    ] {
        p.rect_filled(Rect::from_min_max(min, max), 0.0, DARK);
    }
    gold_frame(p, r.shrink(1.0), 1.0);
    p.rect_stroke(
        r.shrink(band),
        0.0,
        Stroke::new(1.2, a(GOLD, 0.7)),
        egui::StrokeKind::Outside,
    );
    for c in [
        pos2(r.center().x, r.top() + band / 2.0),
        pos2(r.center().x, r.bottom() - band / 2.0),
    ] {
        p.circle_filled(c, 3.0, GOLD_BRIGHT);
        p.line_segment(
            [c - vec2(26.0, 0.0), c + vec2(26.0, 0.0)],
            Stroke::new(1.0, GOLD),
        );
    }
}

/// Section heading for dark panels: gold gothic text between rules, with a leading icon.
pub fn heading(ui: &mut Ui, icon: Option<Icon>, text: &str) {
    ui.add_space(2.0);
    let font = FontId::new(21.0, theme::gothic_family());
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, GOLD_BRIGHT);
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, galley.size().y + 6.0), Sense::hover());
    let p = ui.painter();
    let mut x = rect.left() + 2.0;
    if let Some(i) = icon {
        icons::paint(
            p,
            Rect::from_min_size(pos2(x, rect.center().y - 12.0), vec2(24.0, 24.0)),
            i,
        );
        x += 30.0;
    }
    let cy = rect.center().y;
    p.galley(
        pos2(x, cy - galley.size().y / 2.0),
        galley.clone(),
        GOLD_BRIGHT,
    );
    let x0 = x + galley.size().x + 10.0;
    diamond(p, pos2(x0, cy), 3.5);
    if rect.right() - x0 > 30.0 {
        p.line_segment(
            [pos2(x0 + 6.0, cy), pos2(rect.right() - 8.0, cy)],
            Stroke::new(1.0, a(GOLD, 0.6)),
        );
    }
    ui.add_space(2.0);
}

/// Centered heading with fleurons, for any background (`color` = text colour).
pub fn center_heading(ui: &mut Ui, text: &str, color: Color32) {
    let font = FontId::new(21.0, theme::gothic_family());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), galley.size().y + 6.0),
        Sense::hover(),
    );
    let p = ui.painter();
    let c = rect.center();
    p.galley(c - galley.size() / 2.0, galley.clone(), color);
    for s in [-1.0f32, 1.0] {
        let x = c.x + s * (galley.size().x / 2.0 + 12.0);
        diamond(p, pos2(x, c.y), 3.5);
        p.line_segment(
            [pos2(x + s * 6.0, c.y), pos2(x + s * 30.0, c.y)],
            Stroke::new(1.0, a(GOLD, 0.8)),
        );
    }
}

pub fn diamond(p: &Painter, c: Pos2, r: f32) {
    p.add(Shape::convex_polygon(
        vec![
            c + vec2(0.0, -r),
            c + vec2(r, 0.0),
            c + vec2(0.0, r),
            c + vec2(-r, 0.0),
        ],
        GOLD,
        Stroke::new(0.8, INK),
    ));
}

// ---------------------------------------------------------------- buttons

fn hover_t(ui: &Ui, resp: &Response) -> f32 {
    if theme::reduced_motion(ui.ctx()) {
        if resp.hovered() { 1.0 } else { 0.0 }
    } else {
        ui.ctx()
            .animate_bool_with_time(resp.id, resp.hovered(), 0.15)
    }
}

/// Square tile with an icon above a label (left-panel tabs, paint modes).
pub fn tile_button(ui: &mut Ui, icon: Icon, label: &str, selected: bool, size: Vec2) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = hover_t(ui, &resp);
    let p = ui.painter();
    let fill = if selected {
        Color32::from_rgb(0x5A, 0x3E, 0x22)
    } else {
        DARK_2.lerp_to_gamma(Color32::from_rgb(0x3A, 0x2A, 0x1E), t)
    };
    if selected {
        p.rect_filled(rect.expand(3.0), 6.0, a(GOLD_BRIGHT, 0.18));
    }
    p.rect_filled(rect, 4.0, fill);
    p.rect_stroke(
        rect,
        4.0,
        Stroke::new(
            if selected { 2.0 } else { 1.0 },
            if selected {
                GOLD_BRIGHT
            } else {
                a(BRONZE, 0.8 + 0.2 * t)
            },
        ),
        egui::StrokeKind::Inside,
    );
    let isz = (size.y * 0.42).min(30.0);
    icons::paint(
        p,
        Rect::from_center_size(
            pos2(rect.center().x, rect.top() + 6.0 + isz / 2.0),
            vec2(isz, isz),
        ),
        icon,
    );
    let font = FontId::new(
        if label.len() > 10 { 13.0 } else { 15.5 },
        egui::FontFamily::Proportional,
    );
    let galley = p.layout(
        label.to_owned(),
        font,
        if selected { GOLD_BRIGHT } else { IVORY },
        size.x - 6.0,
    );
    let pos = pos2(
        rect.center().x - galley.size().x / 2.0,
        rect.bottom() - galley.size().y - 5.0,
    );
    p.galley(pos, galley, IVORY);
    if selected {
        let c = pos2(rect.center().x, rect.bottom() + 1.0);
        p.add(Shape::convex_polygon(
            vec![c + vec2(-6.0, 0.0), c + vec2(6.0, 0.0), c + vec2(0.0, 6.0)],
            GOLD_BRIGHT,
            Stroke::NONE,
        ));
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(label)
}

/// Ornate top-bar tab (pointed ends). Selected tabs glow gold.
pub fn top_tab(ui: &mut Ui, icon: Icon, label: &str, selected: bool) -> Response {
    let font = FontId::new(19.0, theme::gothic_family());
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, IVORY);
    let size = vec2(galley.size().x + 74.0, 40.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = hover_t(ui, &resp);
    let p = ui.painter();
    let (l, r, cy) = (rect.left(), rect.right(), rect.center().y);
    let k = 14.0;
    let pts = vec![
        pos2(l, cy),
        pos2(l + k, rect.top()),
        pos2(r - k, rect.top()),
        pos2(r, cy),
        pos2(r - k, rect.bottom()),
        pos2(l + k, rect.bottom()),
    ];
    if selected {
        for i in 1..5 {
            let e = i as f32 * 2.0;
            let glow: Vec<Pos2> = pts
                .iter()
                .map(|q| rect.center() + (*q - rect.center()) * (1.0 + e / size.x * 2.0))
                .collect();
            p.add(Shape::convex_polygon(
                glow,
                Color32::from_rgba_unmultiplied(230, 170, 70, 14),
                Stroke::NONE,
            ));
        }
    }
    let fill = if selected {
        Color32::from_rgb(0x4E, 0x34, 0x1C)
    } else {
        DARK_2.lerp_to_gamma(Color32::from_rgb(0x3C, 0x2B, 0x1E), t)
    };
    p.add(Shape::convex_polygon(
        pts.clone(),
        fill,
        Stroke::new(
            if selected { 2.0 } else { 1.0 },
            if selected { GOLD_BRIGHT } else { BRONZE },
        ),
    ));
    icons::paint(
        p,
        Rect::from_center_size(pos2(l + 30.0, cy), vec2(24.0, 24.0)),
        icon,
    );
    let col = if selected {
        GOLD_BRIGHT
    } else {
        IVORY.lerp_to_gamma(GOLD_BRIGHT, t)
    };
    p.galley(pos2(l + 48.0, cy - galley.size().y / 2.0), galley, col);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Dark rounded button with an optional icon (top bar actions: Invite, Save, Leave).
pub fn bar_button(ui: &mut Ui, icon: Option<Icon>, text: &str, danger: bool) -> Response {
    let font = FontId::new(16.0, egui::FontFamily::Proportional);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, IVORY);
    let w = galley.size().x + if icon.is_some() { 46.0 } else { 24.0 };
    let w = if text.is_empty() { 36.0 } else { w };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
    let t = hover_t(ui, &resp);
    let p = ui.painter();
    let base = if danger { RED_DARK } else { DARK_2 };
    p.rect_filled(
        rect,
        4.0,
        base.lerp_to_gamma(Color32::from_rgb(0x4A, 0x34, 0x24), t * 0.6),
    );
    p.rect_stroke(
        rect,
        4.0,
        Stroke::new(
            1.2,
            if danger {
                Color32::from_rgb(0xB0, 0x40, 0x34)
            } else {
                a(BRONZE, 0.8 + 0.2 * t)
            },
        ),
        egui::StrokeKind::Inside,
    );
    let mut x = rect.left() + 12.0;
    if let Some(i) = icon {
        let sz = 22.0;
        let ir = if text.is_empty() {
            Rect::from_center_size(rect.center(), vec2(sz, sz))
        } else {
            Rect::from_min_size(pos2(x, rect.center().y - sz / 2.0), vec2(sz, sz))
        };
        icons::paint(p, ir, i);
        x += 30.0;
    }
    if !text.is_empty() {
        p.galley(
            pos2(x, rect.center().y - galley.size().y / 2.0),
            galley,
            IVORY.lerp_to_gamma(GOLD_BRIGHT, t),
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Bright gold call-to-action (Roll, Send).
pub fn gold_button(ui: &mut Ui, icon: Option<Icon>, text: &str, min_w: f32) -> Response {
    let font = FontId::new(19.0, theme::bold_family());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, INK);
    let w = (galley.size().x + if icon.is_some() { 56.0 } else { 30.0 }).max(min_w);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 38.0), Sense::click());
    let t = hover_t(ui, &resp);
    let p = ui.painter();
    theme::paint_gradient(
        &p.with_clip_rect(rect.shrink(1.0)),
        rect,
        Color32::from_rgb(0xE8, 0xC4, 0x72).lerp_to_gamma(Color32::from_rgb(0xF6, 0xDA, 0x8E), t),
        Color32::from_rgb(0xA8, 0x78, 0x30),
    );
    p.rect_stroke(
        rect,
        5.0,
        Stroke::new(1.5, Color32::from_rgb(0x5C, 0x3C, 0x14)),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        rect.shrink(3.0),
        3.0,
        Stroke::new(0.8, a(Color32::from_rgb(0xFF, 0xF0, 0xC0), 0.7)),
        egui::StrokeKind::Inside,
    );
    let mut x = rect.center().x - (galley.size().x + if icon.is_some() { 30.0 } else { 0.0 }) / 2.0;
    if let Some(i) = icon {
        icons::paint(
            p,
            Rect::from_min_size(pos2(x, rect.center().y - 12.0), vec2(24.0, 24.0)),
            i,
        );
        x += 30.0;
    }
    p.galley(
        pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        INK,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Rounded status pill with a coloured dot ("Session Active").
pub fn pill(ui: &mut Ui, dot: Color32, text: &str) -> Response {
    let font = FontId::new(14.0, egui::FontFamily::Proportional);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, IVORY);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 34.0, 30.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 15.0, Color32::from_rgb(0x1A, 0x15, 0x12));
    p.rect_stroke(
        rect,
        15.0,
        Stroke::new(1.0, a(BRONZE, 0.8)),
        egui::StrokeKind::Inside,
    );
    let c = pos2(rect.left() + 15.0, rect.center().y);
    p.circle_filled(c, 7.0, a(dot, 0.25));
    p.circle_filled(c, 4.0, dot);
    p.galley(
        pos2(rect.left() + 26.0, rect.center().y - galley.size().y / 2.0),
        galley,
        IVORY,
    );
    resp
}

/// Category row: icon, label, count and a chevron (asset browser).
pub fn list_row(ui: &mut Ui, icon: Icon, label: &str, selected: bool, chevron: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    let t = hover_t(ui, &resp);
    let p = ui.painter();
    if selected {
        theme::paint_gradient(
            &p.with_clip_rect(rect),
            rect,
            Color32::from_rgb(0xD8, 0xB0, 0x5E),
            Color32::from_rgb(0xA0, 0x76, 0x34),
        );
        p.rect_stroke(
            rect,
            3.0,
            Stroke::new(1.0, GOLD_BRIGHT),
            egui::StrokeKind::Inside,
        );
    } else if t > 0.0 {
        p.rect_filled(rect, 3.0, a(Color32::from_rgb(0x5A, 0x40, 0x2C), t * 0.7));
    }
    icons::paint(
        p,
        Rect::from_center_size(pos2(rect.left() + 15.0, rect.center().y), vec2(20.0, 20.0)),
        icon,
    );
    let col = if selected { INK } else { IVORY };
    p.text(
        pos2(rect.left() + 32.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::new(15.0, egui::FontFamily::Proportional),
        col,
    );
    if chevron {
        let c = pos2(rect.right() - 10.0, rect.center().y);
        p.line_segment([c + vec2(-3.0, -5.0), c], Stroke::new(1.5, col));
        p.line_segment([c, c + vec2(-3.0, 5.0)], Stroke::new(1.5, col));
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Joined segmented control (Ambient light). Returns the index clicked.
pub fn segmented(ui: &mut Ui, labels: &[&str], selected: usize) -> Option<usize> {
    let w = ui.available_width().min(300.0);
    let (rect, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
    let n = labels.len().max(1) as f32;
    let mut out = None;
    let p = ui.painter().clone();
    p.rect_filled(rect, 4.0, theme::PARCHMENT_DARK);
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(rect.left() + rect.width() / n * i as f32, rect.top()),
            vec2(rect.width() / n, rect.height()),
        );
        let resp = ui.interact(r, ui.id().with(("seg", i)), Sense::click());
        if i == selected {
            theme::paint_gradient(
                &p.with_clip_rect(r.shrink(2.0)),
                r.shrink(2.0),
                Color32::from_rgb(0xF2, 0xD3, 0x80),
                Color32::from_rgb(0xC8, 0x96, 0x40),
            );
            p.rect_stroke(
                r.shrink(2.0),
                3.0,
                Stroke::new(1.0, Color32::from_rgb(0x7A, 0x52, 0x1E)),
                egui::StrokeKind::Inside,
            );
        } else if resp.hovered() {
            p.rect_filled(r.shrink(2.0), 3.0, a(GOLD, 0.25));
        }
        if i > 0 {
            p.line_segment(
                [
                    r.left_top() + vec2(0.0, 5.0),
                    r.left_bottom() - vec2(0.0, 5.0),
                ],
                Stroke::new(1.0, BRONZE),
            );
        }
        p.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            *l,
            FontId::new(15.0, egui::FontFamily::Proportional),
            INK,
        );
        if resp.clicked() {
            out = Some(i);
        }
    }
    p.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, BRONZE),
        egui::StrokeKind::Inside,
    );
    out
}

// ---------------------------------------------------------------- dice

/// Red gem die (d4 triangle … d20 hexagon) with its label. Click = roll.
pub fn die(ui: &mut Ui, sides: u32, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let t = hover_t(ui, &resp);
    let p = ui.painter();
    let c = rect.center() - vec2(0.0, 2.0 * t);
    let r = size * 0.46;
    let n = match sides {
        4 => 3,
        6 => 6,
        8 => 4,
        10 => 5,
        12 => 5,
        _ => 6,
    };
    let rot = if sides == 4 || sides == 12 || sides == 10 {
        -PI / 2.0
    } else if sides == 8 {
        0.0
    } else {
        PI / 6.0
    };
    let ry = if sides == 10 { r * 1.05 } else { r };
    let pts: Vec<Pos2> = (0..n)
        .map(|i| {
            let ang = rot + i as f32 / n as f32 * TAU;
            c + vec2(ang.cos() * r, ang.sin() * ry)
        })
        .collect();
    p.add(Shape::convex_polygon(
        pts.iter().map(|q| *q + vec2(2.0, 3.0)).collect(),
        Color32::from_black_alpha(110),
        Stroke::NONE,
    ));
    p.add(Shape::convex_polygon(
        pts.clone(),
        RED.lerp_to_gamma(Color32::from_rgb(0xB8, 0x2E, 0x26), t),
        Stroke::new(1.5, Color32::from_rgb(0x2A, 0x08, 0x08)),
    ));
    // Inner facet + highlight.
    let inner: Vec<Pos2> = pts.iter().map(|q| c + (*q - c) * 0.55).collect();
    p.add(Shape::convex_polygon(
        inner.clone(),
        Color32::from_rgb(0xA8, 0x2A, 0x24),
        Stroke::new(0.8, Color32::from_rgb(0x4A, 0x0C, 0x0C)),
    ));
    for (o, i) in pts.iter().zip(&inner) {
        p.line_segment(
            [*o, *i],
            Stroke::new(0.8, Color32::from_rgb(0x4A, 0x0C, 0x0C)),
        );
    }
    p.line_segment(
        [pts[0], pts[pts.len() - 1]],
        Stroke::new(1.0, a(Color32::from_rgb(0xFF, 0xB0, 0x90), 0.5)),
    );
    p.text(
        c + vec2(0.0, 1.0),
        egui::Align2::CENTER_CENTER,
        format!("d{sides}"),
        FontId::new(size * 0.3, theme::bold_family()),
        IVORY,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

// ---------------------------------------------------------------- decorations

/// Gold dragon crest (head in profile with horns and a swept wing).
pub fn dragon(p: &Painter, r: Rect) {
    let s = r.width().min(r.height()) / 100.0;
    let o = r.min;
    let q = |x: f32, y: f32| o + vec2(x * s, y * s);
    let gold = Color32::from_rgb(0xD8, 0xA8, 0x48);
    let dark = Color32::from_rgb(0x6A, 0x46, 0x16);
    let st = Stroke::new(1.2 * s.max(0.6), dark);
    // Wing: fan of blades behind the neck.
    for i in 0..5 {
        let a0 = -2.6 + i as f32 * 0.32;
        let base = q(40.0, 58.0);
        let tip = base + vec2(a0.cos(), a0.sin()) * (44.0 - i as f32 * 4.0) * s;
        let side = base + vec2((a0 + 0.35).cos(), (a0 + 0.35).sin()) * 22.0 * s;
        p.add(Shape::convex_polygon(
            vec![base, tip, side],
            gold.gamma_multiply(0.75 + i as f32 * 0.05),
            st,
        ));
    }
    // Neck: thick curling stroke.
    let neck: Vec<Pos2> = (0..=16)
        .map(|i| {
            let t = i as f32 / 16.0;
            q(30.0 + t * 34.0 + (t * PI).sin() * -8.0, 92.0 - t * 52.0)
        })
        .collect();
    p.add(Shape::line(neck.clone(), Stroke::new(14.0 * s, dark)));
    p.add(Shape::line(neck, Stroke::new(10.0 * s, gold)));
    // Head and jaw.
    p.add(Shape::convex_polygon(
        vec![
            q(54.0, 30.0),
            q(72.0, 24.0),
            q(96.0, 34.0),
            q(94.0, 40.0),
            q(70.0, 44.0),
            q(58.0, 46.0),
        ],
        gold,
        st,
    ));
    p.add(Shape::convex_polygon(
        vec![q(62.0, 44.0), q(90.0, 42.0), q(80.0, 52.0), q(64.0, 52.0)],
        gold.gamma_multiply(0.85),
        st,
    ));
    // Horns and crest spikes.
    for (b0, b1, tip) in [
        ((58.0, 30.0), (64.0, 27.0), (40.0, 6.0)),
        ((64.0, 27.0), (70.0, 25.0), (52.0, 2.0)),
        ((52.0, 46.0), (56.0, 40.0), (40.0, 40.0)),
    ] {
        p.add(Shape::convex_polygon(
            vec![q(b0.0, b0.1), q(b1.0, b1.1), q(tip.0, tip.1)],
            gold,
            st,
        ));
    }
    p.circle_filled(q(76.0, 32.0), 2.4 * s, Color32::from_rgb(0xFF, 0x6A, 0x20));
    p.line_segment([q(84.0, 39.0), q(96.0, 37.0)], Stroke::new(1.0 * s, dark));
}

/// Hanging red banner with gold rod, trim and an emblem.
pub fn banner(p: &Painter, r: Rect, icon: Icon) {
    let rod = r.top() + 6.0;
    let cloth = Rect::from_min_max(
        pos2(r.left() + 6.0, rod),
        pos2(r.right() - 6.0, r.bottom() - r.width() * 0.35),
    );
    p.rect_filled(
        cloth.translate(vec2(3.0, 4.0)),
        0.0,
        Color32::from_black_alpha(90),
    );
    p.rect_filled(cloth, 0.0, RED_DARK);
    let tail = r.bottom();
    // Swallowtail: two convex triangles.
    p.add(Shape::convex_polygon(
        vec![
            cloth.left_bottom(),
            pos2(cloth.center().x, cloth.bottom()),
            pos2(cloth.left(), tail),
        ],
        RED_DARK,
        Stroke::NONE,
    ));
    p.add(Shape::convex_polygon(
        vec![
            pos2(cloth.center().x, cloth.bottom()),
            cloth.right_bottom(),
            pos2(cloth.right(), tail),
        ],
        RED_DARK,
        Stroke::NONE,
    ));
    theme::paint_gradient(
        &p.with_clip_rect(cloth),
        cloth,
        a(Color32::from_rgb(0xB0, 0x30, 0x2A), 0.6),
        Color32::TRANSPARENT,
    );
    for x in [cloth.left() + 4.0, cloth.right() - 4.0] {
        p.line_segment(
            [pos2(x, cloth.top()), pos2(x, cloth.bottom())],
            Stroke::new(1.5, GOLD),
        );
    }
    p.line_segment(
        [pos2(r.left(), rod), pos2(r.right(), rod)],
        Stroke::new(4.0, GOLD),
    );
    p.circle_filled(pos2(r.left(), rod), 4.0, GOLD_BRIGHT);
    p.circle_filled(pos2(r.right(), rod), 4.0, GOLD_BRIGHT);
    let isz = cloth.width() * 0.62;
    icons::paint(
        p,
        Rect::from_center_size(
            pos2(cloth.center().x, cloth.top() + cloth.height() * 0.42),
            vec2(isz, isz),
        ),
        icon,
    );
}

/// Candle on a stack of books, with a flickering flame and glow (`t` = seconds).
pub fn candle(p: &Painter, base: Pos2, s: f32, t: f32) {
    // Books.
    for (i, (w, col)) in [
        (118.0, Color32::from_rgb(0x4A, 0x2A, 0x1A)),
        (104.0, Color32::from_rgb(0x5E, 0x1E, 0x1A)),
        (92.0, Color32::from_rgb(0x3A, 0x2C, 0x22)),
    ]
    .iter()
    .enumerate()
    {
        let y = base.y - (i as f32 + 1.0) * 18.0 * s;
        let r = Rect::from_min_size(
            pos2(base.x - w * s / 2.0 + i as f32 * 4.0 * s, y),
            vec2(w * s, 17.0 * s),
        );
        p.rect_filled(r, 2.0, *col);
        p.line_segment(
            [
                r.right_top() + vec2(-4.0 * s, 3.0 * s),
                r.right_bottom() + vec2(-4.0 * s, -3.0 * s),
            ],
            Stroke::new(3.0 * s, Color32::from_rgb(0xD8, 0xC8, 0xA0)),
        );
        p.line_segment(
            [
                r.left_top() + vec2(8.0 * s, 8.0 * s),
                r.right_top() + vec2(-12.0 * s, 8.0 * s),
            ],
            Stroke::new(1.0, a(GOLD, 0.6)),
        );
    }
    let top = base.y - 54.0 * s;
    // Holder and wax.
    p.add(Shape::convex_polygon(
        vec![
            pos2(base.x - 26.0 * s, top),
            pos2(base.x + 26.0 * s, top),
            pos2(base.x + 16.0 * s, top - 6.0 * s),
            pos2(base.x - 16.0 * s, top - 6.0 * s),
        ],
        Color32::from_rgb(0x8A, 0x6A, 0x30),
        Stroke::new(1.0, INK),
    ));
    let wax = Rect::from_min_max(
        pos2(base.x - 10.0 * s, top - 52.0 * s),
        pos2(base.x + 10.0 * s, top - 5.0 * s),
    );
    theme::paint_gradient(
        &p.with_clip_rect(wax),
        wax,
        Color32::from_rgb(0xF2, 0xE2, 0xB8),
        Color32::from_rgb(0xC8, 0xA8, 0x70),
    );
    p.circle_filled(
        pos2(wax.left() + 3.0 * s, wax.top() + 12.0 * s),
        3.0 * s,
        Color32::from_rgb(0xF2, 0xE2, 0xB8),
    );
    p.rect_filled(
        Rect::from_min_size(
            pos2(wax.left() + 1.0, wax.top() + 6.0 * s),
            vec2(4.0 * s, 14.0 * s),
        ),
        2.0,
        Color32::from_rgb(0xF4, 0xE6, 0xC0),
    );
    // Flame.
    let f = 1.0 + 0.08 * (t * 13.0).sin() + 0.05 * (t * 23.0).sin();
    let sway = 1.5 * (t * 5.0).sin() * s;
    let fc = pos2(base.x + sway, wax.top() - 12.0 * s * f);
    for k in (1..10).rev() {
        let kk = k as f32 / 9.0;
        p.circle_filled(
            fc,
            60.0 * s * kk * f,
            Color32::from_rgba_unmultiplied(255, 170, 70, (6.0 * (1.0 - kk) + 2.0) as u8),
        );
    }
    let flame = |rx: f32, ry: f32, col: Color32| {
        let pts: Vec<Pos2> = (0..16)
            .map(|i| {
                let ang = i as f32 / 16.0 * TAU;
                let pinch = if ang.sin() < 0.0 {
                    1.0 + ang.sin().abs() * 0.6
                } else {
                    1.0
                };
                fc + vec2(ang.cos() * rx * s / pinch, ang.sin() * ry * s * f * pinch)
            })
            .collect();
        p.add(Shape::convex_polygon(pts, col, Stroke::NONE));
    };
    flame(7.0, 12.0, Color32::from_rgb(0xF0, 0x80, 0x28));
    flame(4.5, 8.0, Color32::from_rgb(0xFF, 0xD0, 0x60));
    flame(2.2, 4.0, Color32::from_rgb(0xFF, 0xF8, 0xE0));
    p.line_segment(
        [pos2(base.x, wax.top()), pos2(base.x, wax.top() - 4.0 * s)],
        Stroke::new(1.2, INK),
    );
}

/// Skull resting on a ledge (bottom-right corner decoration).
pub fn skull(p: &Painter, c: Pos2, s: f32) {
    let bone = Color32::from_rgb(0xC8, 0xBC, 0x9C);
    let shade = Color32::from_rgb(0x8A, 0x7E, 0x64);
    p.circle_filled(
        c + vec2(4.0, 6.0) * s,
        34.0 * s,
        Color32::from_black_alpha(90),
    );
    p.add(Shape::convex_polygon(
        vec![
            c + vec2(-18.0, 16.0) * s,
            c + vec2(18.0, 16.0) * s,
            c + vec2(14.0, 38.0) * s,
            c + vec2(-14.0, 38.0) * s,
        ],
        shade,
        Stroke::new(1.0, INK),
    ));
    p.circle_filled(c, 32.0 * s, bone);
    theme::paint_vignette(
        &p.with_clip_rect(Rect::from_center_size(c, vec2(64.0, 64.0) * s)),
        Rect::from_center_size(c, vec2(64.0, 64.0) * s),
        14.0 * s,
        a(shade, 0.5),
    );
    for x in [-12.0, 12.0] {
        p.circle_filled(
            c + vec2(x, 6.0) * s,
            9.0 * s,
            Color32::from_rgb(0x1A, 0x12, 0x0E),
        );
    }
    p.add(Shape::convex_polygon(
        vec![
            c + vec2(0.0, 14.0) * s,
            c + vec2(4.0, 22.0) * s,
            c + vec2(-4.0, 22.0) * s,
        ],
        Color32::from_rgb(0x2A, 0x1E, 0x16),
        Stroke::NONE,
    ));
    for i in 0..5 {
        let x = -10.0 + i as f32 * 5.0;
        p.line_segment(
            [c + vec2(x, 28.0) * s, c + vec2(x, 36.0) * s],
            Stroke::new(1.2, INK),
        );
    }
    p.circle_filled(c + vec2(-10.0, -14.0) * s, 8.0 * s, a(Color32::WHITE, 0.25));
}

/// Red wax seal with ribbon tails.
pub fn wax_seal(p: &Painter, c: Pos2, r: f32) {
    for (dx, ang) in [(-0.35f32, 0.25f32), (0.35, -0.25)] {
        let b = c + vec2(dx * r, r * 0.4);
        let d = vec2(ang.sin(), ang.cos());
        p.add(Shape::convex_polygon(
            vec![
                b - vec2(r * 0.22, 0.0),
                b + vec2(r * 0.22, 0.0),
                b + d * r * 1.6 + vec2(r * 0.2, 0.0),
                b + d * r * 1.6 - vec2(r * 0.2, 0.0),
            ],
            RED_DARK,
            Stroke::new(1.0, INK),
        ));
    }
    let n = 18;
    let pts: Vec<Pos2> = (0..n)
        .map(|i| {
            let ang = i as f32 / n as f32 * TAU;
            let rr = r * if i % 2 == 0 { 1.0 } else { 0.9 };
            c + vec2(ang.cos(), ang.sin()) * rr
        })
        .collect();
    p.add(Shape::convex_polygon(pts, RED, Stroke::new(1.0, RED_DARK)));
    p.circle_stroke(
        c,
        r * 0.68,
        Stroke::new(2.0, Color32::from_rgb(0x5C, 0x0E, 0x0E)),
    );
    p.circle_filled(
        c - vec2(r * 0.3, r * 0.35),
        r * 0.18,
        a(Color32::from_rgb(0xFF, 0x90, 0x80), 0.4),
    );
    icons::paint(p, Rect::from_center_size(c, vec2(r, r)), Icon::Crown);
}

/// Faint ruined archway sketch (parchment panel corner).
pub fn archway(p: &Painter, r: Rect) {
    let col = Color32::from_rgba_unmultiplied(0x6A, 0x50, 0x34, 70);
    let st = Stroke::new(1.5, col);
    let (l, rr, b) = (
        r.left() + r.width() * 0.2,
        r.right() - r.width() * 0.2,
        r.bottom(),
    );
    let top = r.top() + r.height() * 0.35;
    for x in [l, rr] {
        p.line_segment([pos2(x, b), pos2(x, top)], st);
        p.line_segment(
            [
                pos2(x + if x == l { -8.0 } else { 8.0 }, b),
                pos2(x + if x == l { -8.0 } else { 8.0 }, top + 6.0),
            ],
            st,
        );
    }
    let arch: Vec<Pos2> = (0..=20)
        .map(|i| {
            let t = PI + i as f32 / 20.0 * PI;
            pos2(
                (l + rr) / 2.0 + t.cos() * (rr - l) / 2.0,
                top + t.sin() * (rr - l) / 2.0,
            )
        })
        .collect();
    p.add(Shape::line(arch, st));
    for i in 0..5 {
        let y = top + 10.0 + i as f32 * (b - top) / 5.0;
        p.line_segment([pos2(l - 8.0, y), pos2(l, y)], st);
        p.line_segment([pos2(rr, y + 6.0), pos2(rr + 8.0, y + 6.0)], st);
    }
}

/// Compass rose badge ("N") above the minimap.
pub fn compass(p: &Painter, c: Pos2, r: f32) {
    p.circle_filled(c, r, DARK);
    p.circle_stroke(c, r, Stroke::new(1.5, GOLD));
    p.add(Shape::convex_polygon(
        vec![
            c + vec2(0.0, -r - 6.0),
            c + vec2(4.0, -r + 2.0),
            c + vec2(-4.0, -r + 2.0),
        ],
        GOLD_BRIGHT,
        Stroke::NONE,
    ));
    p.text(
        c,
        egui::Align2::CENTER_CENTER,
        "N",
        FontId::new(r * 1.1, theme::bold_family()),
        GOLD_BRIGHT,
    );
}

/// Small "i" info badge (notes).
pub fn info_badge(p: &Painter, c: Pos2) {
    p.circle_stroke(c, 9.0, Stroke::new(1.3, theme::INK_SOFT));
    p.text(
        c,
        egui::Align2::CENTER_CENTER,
        "i",
        FontId::new(13.0, theme::bold_family()),
        theme::INK_SOFT,
    );
}

/// Speech-bubble glyph (chat input).
pub fn bubble(p: &Painter, c: Pos2) {
    p.circle_stroke(c, 8.0, Stroke::new(1.4, IVORY));
    p.add(Shape::convex_polygon(
        vec![
            c + vec2(-6.0, 5.0),
            c + vec2(-2.0, 7.0),
            c + vec2(-8.0, 11.0),
        ],
        IVORY,
        Stroke::NONE,
    ));
}
