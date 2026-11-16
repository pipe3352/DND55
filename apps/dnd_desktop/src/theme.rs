//! DND55 visual design system: medieval gothic fantasy.
//!
//! One place for every color, font, material and reusable widget so screens stay consistent:
//! * **Palette** — restrained earthy tones (deep background, dark wood, aged parchment, antique gold,
//!   bronze, burgundy, forest green, slate blue, warm ivory, dark ink).
//! * **Fonts** (SIL OFL, embedded, see `assets/fonts/OFL-*.txt`): *UnifrakturCook* Blackletter for the
//!   logo and big titles, *Grenze Gotisch* for headings, *Crimson Text* (book serif) for everything that
//!   must be read quickly. All cover Italian accents; egui's default fonts stay as symbol fallbacks.
//! * **Materials** — procedural, tileable parchment / wood / iron textures (original, generated at
//!   startup) painted under panels and windows, with ornamental gold/bronze borders.
//! * **Components** — ornamental headings, plaque buttons, health bars, spell-slot gems, framed
//!   windows, banners. Prefer these helpers over hard-coded colors in screens.

use bevy_egui::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Pos2, Rect, Shape, Stroke,
    TextureHandle, Vec2, Visuals, epaint::Mesh, pos2, vec2,
};

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

// ---------------------------------------------------------------- palette (design tokens)
/// Deep background behind everything.
pub const BG: Color32 = rgb(0x17, 0x17, 0x15);
/// Dark carved wood (frames, toolbars).
pub const WOOD: Color32 = rgb(0x35, 0x25, 0x1D);
pub const WOOD_LIGHT: Color32 = rgb(0x5A, 0x40, 0x2C);
/// Aged parchment (panel surfaces).
pub const PARCHMENT: Color32 = rgb(0xE5, 0xD5, 0xB5);
pub const PARCHMENT_DARK: Color32 = rgb(0xD0, 0xBA, 0x8F);
pub const IVORY: Color32 = rgb(0xF1, 0xE7, 0xD2);
/// Antique gold (accents, borders, titles on dark).
pub const GOLD: Color32 = rgb(0xB5, 0x96, 0x54);
pub const GOLD_BRIGHT: Color32 = rgb(0xDE, 0xBD, 0x6E);
pub const BRONZE: Color32 = rgb(0x88, 0x63, 0x3F);
/// Burgundy: danger, damage, the party's room.
pub const BURGUNDY: Color32 = rgb(0x69, 0x30, 0x2E);
pub const BLOOD: Color32 = BURGUNDY;
pub const FOREST: Color32 = rgb(0x40, 0x53, 0x41);
/// Health fill (forest green, brightened for bars).
pub const HEALTH: Color32 = rgb(0x4E, 0x73, 0x45);
pub const SLATE: Color32 = rgb(0x46, 0x5C, 0x64);
pub const MANA: Color32 = rgb(0x3E, 0x62, 0x8C);
/// Dark ink (body text).
pub const INK: Color32 = rgb(0x24, 0x1D, 0x1A);
pub const INK_SOFT: Color32 = rgb(0x5C, 0x4A, 0x3A);
pub const IRON: Color32 = rgb(0x3B, 0x3A, 0x38);

// ---------------------------------------------------------------- fonts

pub fn display_family() -> FontFamily {
    FontFamily::Name("display".into())
}
pub fn gothic_family() -> FontFamily {
    FontFamily::Name("gothic".into())
}
pub fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}
pub fn italic_family() -> FontFamily {
    FontFamily::Name("italic".into())
}

/// Blackletter text (logo, campaign/room titles, announcements). Not for body text.
pub fn display(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(text.into())
        .family(display_family())
        .size(size)
}
/// Readable gothic heading text.
pub fn gothic(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(text.into())
        .family(gothic_family())
        .size(size)
}
/// Semi-bold serif (numbers, labels that need weight).
pub fn bold(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into()).family(bold_family())
}
pub fn italic(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into()).family(italic_family())
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let add = |f: &mut egui::FontDefinitions, name: &str, bytes: &'static [u8]| {
        f.font_data.insert(
            name.to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
    };
    add(
        &mut fonts,
        "unifraktur",
        include_bytes!("../assets/fonts/UnifrakturCook-Bold.ttf"),
    );
    add(
        &mut fonts,
        "grenze",
        include_bytes!("../assets/fonts/GrenzeGotisch.ttf"),
    );
    add(
        &mut fonts,
        "crimson",
        include_bytes!("../assets/fonts/CrimsonText-Regular.ttf"),
    );
    add(
        &mut fonts,
        "crimson-sb",
        include_bytes!("../assets/fonts/CrimsonText-SemiBold.ttf"),
    );
    add(
        &mut fonts,
        "crimson-it",
        include_bytes!("../assets/fonts/CrimsonText-Italic.ttf"),
    );
    // egui's defaults (Ubuntu + emoji/icon fonts) stay as fallbacks for symbols.
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let chain = |first: &str| {
        let mut v = vec![first.to_owned()];
        if first != "crimson" {
            v.push("crimson".to_owned());
        }
        v.extend(fallback.iter().cloned());
        v
    };
    fonts
        .families
        .insert(FontFamily::Proportional, chain("crimson"));
    fonts.families.insert(display_family(), chain("unifraktur"));
    fonts.families.insert(gothic_family(), chain("grenze"));
    fonts.families.insert(bold_family(), chain("crimson-sb"));
    fonts.families.insert(italic_family(), chain("crimson-it"));
    ctx.set_fonts(fonts);
}

// ---------------------------------------------------------------- materials

/// Procedural, tileable material textures (original; generated once per context).
#[derive(Clone)]
pub struct Materials {
    pub parchment: TextureHandle,
    pub wood: TextureHandle,
    pub iron: TextureHandle,
}

fn hash(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x.wrapping_mul(374_761_393)
        ^ y.wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

/// Tileable value noise (period `p` cells over a `size` texture).
fn tnoise(x: f32, y: f32, p: u32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let w = |a: i64| (a.rem_euclid(p as i64)) as u32;
    let v = |a: i64, b: i64| hash(w(a), w(b), seed);
    let (a, b, c, d) = (v(xi, yi), v(xi + 1, yi), v(xi, yi + 1), v(xi + 1, yi + 1));
    let (u, t) = (s(fx), s(fy));
    a + (b - a) * u + (c - a) * t + (a - b - c + d) * u * t
}

/// Fractal tileable noise in 0..1 for a pixel of a `size`×`size` texture.
pub fn fbm(px: u32, py: u32, size: u32, base_period: u32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut period) = (0.0, 1.0, 0.0, base_period);
    for o in 0..octaves {
        let scale = period as f32 / size as f32;
        sum += amp
            * tnoise(
                px as f32 * scale,
                py as f32 * scale,
                period,
                seed.wrapping_add(o * 977),
            );
        norm += amp;
        amp *= 0.5;
        period *= 2;
    }
    sum / norm
}

fn make_tex(
    ctx: &egui::Context,
    name: &str,
    size: u32,
    f: impl Fn(u32, u32) -> [u8; 3],
) -> TextureHandle {
    let mut pixels = Vec::with_capacity((size * size) as usize);
    for y in 0..size {
        for x in 0..size {
            let [r, g, b] = f(x, y);
            pixels.push(Color32::from_rgb(r, g, b));
        }
    }
    let img = egui::ColorImage::new([size as usize, size as usize], pixels);
    ctx.load_texture(name, img, egui::TextureOptions::LINEAR_REPEAT)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
fn to_u8(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| v.clamp(0.0, 255.0) as u8)
}

impl Materials {
    fn generate(ctx: &egui::Context) -> Self {
        const S: u32 = 256;
        let parchment = make_tex(ctx, "mat-parchment", S, |x, y| {
            let blot = fbm(x, y, S, 4, 4, 11);
            let grain = hash(x, y, 3);
            let fiber = fbm(x, y * 4 % S, S, 32, 2, 19);
            let base = mix(
                [0xE9 as f32, 0xDB as f32, 0xBC as f32],
                [0xD3 as f32, 0xBD as f32, 0x93 as f32],
                (blot - 0.35).max(0.0) * 1.6,
            );
            let c = mix(
                base,
                [0xC4 as f32, 0xA8 as f32, 0x7C as f32],
                (fiber - 0.6).max(0.0) * 0.8,
            );
            to_u8(c.map(|v| v * (0.97 + grain * 0.05)))
        });
        let wood = make_tex(ctx, "mat-wood", S, |x, y| {
            // Long horizontal grain: noise stretched along x, gentle warp, plank seams every 64 px.
            let warp = fbm(x / 4, y, S, 4, 3, 41) * 6.0;
            let grain = ((y as f32 * 0.45 + warp).sin() * 0.5 + 0.5).powf(2.0);
            let streak = fbm(x / 8, y, S, 16, 3, 7);
            let fleck = hash(x, y, 9);
            let seam = if y % 64 < 2 { 0.55 } else { 1.0 };
            let c = mix(
                [0x2A as f32, 0x1C as f32, 0x15 as f32],
                [0x4C as f32, 0x35 as f32, 0x26 as f32],
                grain * 0.45 + streak * 0.55,
            );
            to_u8(c.map(|v| v * (0.95 + fleck * 0.08) * seam))
        });
        let iron = make_tex(ctx, "mat-iron", S, |x, y| {
            let n = fbm(x, y, S, 8, 4, 77);
            let g = hash(x, y, 5);
            let v = 46.0 + n * 34.0 + g * 8.0;
            to_u8([v, v * 0.98, v * 0.94])
        });
        Materials {
            parchment,
            wood,
            iron,
        }
    }

    pub fn get(ctx: &egui::Context) -> Materials {
        let id = egui::Id::new("dnd55-materials");
        if let Some(m) = ctx.data(|d| d.get_temp::<Materials>(id)) {
            return m;
        }
        let m = Materials::generate(ctx);
        ctx.data_mut(|d| d.insert_temp(id, m.clone()));
        m
    }
}

/// Paints a tiling texture over `rect` (texture pixels ≈ screen points).
pub fn paint_material(painter: &egui::Painter, rect: Rect, tex: &TextureHandle, tint: Color32) {
    let uv = Rect::from_min_size(
        pos2(rect.min.x / 256.0, rect.min.y / 256.0),
        vec2(rect.width() / 256.0, rect.height() / 256.0),
    );
    painter.image(tex.id(), rect, uv, tint);
}

/// Soft darkening towards the edges of a rect (aged-paper burn / vignette).
pub fn paint_vignette(painter: &egui::Painter, rect: Rect, width: f32, color: Color32) {
    let inner = rect.shrink(width.min(rect.width() / 2.0).min(rect.height() / 2.0));
    let clear = Color32::TRANSPARENT;
    let mut mesh = Mesh::default();
    let o = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    let i = [
        inner.left_top(),
        inner.right_top(),
        inner.right_bottom(),
        inner.left_bottom(),
    ];
    for k in 0..4 {
        let n = (k + 1) % 4;
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(o[k], color);
        mesh.colored_vertex(o[n], color);
        mesh.colored_vertex(i[n], clear);
        mesh.colored_vertex(i[k], clear);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    painter.add(Shape::mesh(mesh));
}

/// Vertical two-color gradient.
pub fn paint_gradient(painter: &egui::Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
}

/// Gold corner flourish (a diamond with two curling tendrils), pointing into the rect from `corner`.
fn corner_flourish(painter: &egui::Painter, corner: Pos2, dir: Vec2, size: f32, color: Color32) {
    let (dx, dy) = (dir.x, dir.y);
    let p = |x: f32, y: f32| corner + vec2(x * dx, y * dy) * size;
    let stroke = Stroke::new((size * 0.09).max(1.0), color);
    // Two curls along the edges.
    for (horizontal, len) in [(true, 2.6), (false, 2.6)] {
        let pts: Vec<Pos2> = (0..=14)
            .map(|i| {
                let t = i as f32 / 14.0;
                let along = 0.55 + t * len;
                let wave = (t * std::f32::consts::PI * 1.5).sin() * 0.28 * (1.0 - t);
                if horizontal {
                    p(along, 0.55 + wave)
                } else {
                    p(0.55 + wave, along)
                }
            })
            .collect();
        painter.add(Shape::line(pts, stroke));
        let end = if horizontal {
            p(0.55 + len, 0.55)
        } else {
            p(0.55, 0.55 + len)
        };
        painter.circle_filled(end, size * 0.11, color);
    }
    let c = p(0.55, 0.55);
    let r = size * 0.32;
    painter.add(Shape::convex_polygon(
        vec![
            c + vec2(0.0, -r),
            c + vec2(r, 0.0),
            c + vec2(0.0, r),
            c + vec2(-r, 0.0),
        ],
        color,
        Stroke::new(1.0, INK),
    ));
}

/// Ornamental border: dark band, bronze rule, thin gold inner rule and corner flourishes.
pub fn paint_ornate_border(painter: &egui::Painter, rect: Rect, scale: f32) {
    painter.rect_stroke(
        rect.shrink(1.5),
        CornerRadius::same(3),
        Stroke::new(3.0, WOOD),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(4.5),
        CornerRadius::same(2),
        Stroke::new(1.2, BRONZE),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(7.0),
        CornerRadius::ZERO,
        Stroke::new(0.8, GOLD.gamma_multiply(0.8)),
        egui::StrokeKind::Inside,
    );
    let s = 7.0 * scale;
    let inset = 5.0;
    for (corner, dir) in [
        (rect.left_top() + vec2(inset, inset), vec2(1.0, 1.0)),
        (rect.right_top() + vec2(-inset, inset), vec2(-1.0, 1.0)),
        (rect.right_bottom() + vec2(-inset, -inset), vec2(-1.0, -1.0)),
        (rect.left_bottom() + vec2(inset, -inset), vec2(1.0, -1.0)),
    ] {
        corner_flourish(painter, corner, dir, s, GOLD);
    }
}

/// Parchment surface with burned edges for the area behind a window/panel's content.
pub fn paint_parchment(painter: &egui::Painter, rect: Rect) {
    let m = Materials::get(painter.ctx());
    paint_material(painter, rect, &m.parchment, Color32::WHITE);
    paint_vignette(
        painter,
        rect,
        26.0,
        Color32::from_rgba_unmultiplied(110, 72, 30, 70),
    );
}

/// Call first inside a window's content closure: parchment + ornamental border under the widgets.
pub fn window_bg(ui: &egui::Ui) {
    let rect = ui.max_rect().expand(MARGIN as f32);
    let painter = ui.painter().with_clip_rect(rect.expand(2.0));
    paint_parchment(&painter, rect);
    paint_ornate_border(&painter, rect, 1.0);
}

/// Call first inside a side panel's closure.
pub fn panel_bg(ui: &egui::Ui) {
    let rect = ui.max_rect().expand(PANEL_MARGIN as f32);
    let painter = ui.painter().with_clip_rect(rect.expand(1.0));
    paint_parchment(&painter, rect);
    painter.rect_stroke(
        rect.shrink(1.0),
        CornerRadius::ZERO,
        Stroke::new(2.0, WOOD),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.shrink(4.0),
        CornerRadius::ZERO,
        Stroke::new(0.8, BRONZE),
        egui::StrokeKind::Inside,
    );
}

/// Dark carved-wood strip (toolbars, the top bar).
pub fn wood_bg(ui: &egui::Ui) {
    let rect = ui.max_rect().expand(6.0);
    let painter = ui.painter().with_clip_rect(rect);
    let m = Materials::get(ui.ctx());
    paint_material(&painter, rect, &m.wood, Color32::WHITE);
    painter.hline(
        rect.x_range(),
        rect.bottom() - 1.5,
        Stroke::new(2.0, GOLD.gamma_multiply(0.7)),
    );
    painter.hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, BRONZE));
}

// ---------------------------------------------------------------- global style

const MARGIN: i8 = 14;
const PANEL_MARGIN: i8 = 10;

/// Reduced-motion preference (shared through egui memory so any screen can query it).
pub fn set_reduced_motion(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("dnd55-reduced-motion"), on));
}
pub fn reduced_motion(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new("dnd55-reduced-motion")))
        .unwrap_or(false)
}

pub fn apply(ctx: &egui::Context, scale: f32) {
    install_fonts(ctx);
    let mut v = Visuals::light();
    v.override_text_color = Some(INK);
    v.panel_fill = PARCHMENT;
    v.window_fill = PARCHMENT;
    v.extreme_bg_color = IVORY; // text edits, scroll troughs
    v.faint_bg_color = PARCHMENT_DARK;
    v.code_bg_color = PARCHMENT_DARK;
    v.window_stroke = Stroke::new(2.0, WOOD);
    v.window_corner_radius = CornerRadius::same(4);
    v.window_shadow = egui::Shadow {
        offset: [4, 8],
        blur: 22,
        spread: 2,
        color: Color32::from_black_alpha(140),
    };
    v.popup_shadow = egui::Shadow {
        offset: [2, 4],
        blur: 10,
        spread: 0,
        color: Color32::from_black_alpha(110),
    };
    v.selection.bg_fill = GOLD_BRIGHT.gamma_multiply(0.85);
    v.selection.stroke = Stroke::new(1.0, INK);
    v.hyperlink_color = BURGUNDY;
    v.warn_fg_color = BRONZE;
    v.error_fg_color = BURGUNDY;
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.55 };
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(3);
        w.fg_stroke = Stroke::new(1.0, INK);
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.bg_fill = PARCHMENT;
    v.widgets.noninteractive.weak_bg_fill = PARCHMENT;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BRONZE.gamma_multiply(0.6));
    v.widgets.inactive.bg_fill = rgb(0xCB, 0xB3, 0x86);
    v.widgets.inactive.weak_bg_fill = rgb(0xD8, 0xC3, 0x98);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BRONZE);
    v.widgets.hovered.bg_fill = rgb(0xE6, 0xC8, 0x82);
    v.widgets.hovered.weak_bg_fill = rgb(0xEB, 0xD1, 0x93);
    v.widgets.hovered.bg_stroke = Stroke::new(1.5, GOLD_BRIGHT);
    v.widgets.hovered.expansion = 1.0;
    v.widgets.active.bg_fill = GOLD;
    v.widgets.active.weak_bg_fill = GOLD;
    v.widgets.active.bg_stroke = Stroke::new(1.5, WOOD);
    v.widgets.open.weak_bg_fill = GOLD_BRIGHT;
    ctx.set_visuals(v);
    ctx.set_zoom_factor(scale);
    let anim = if reduced_motion(ctx) { 0.0 } else { 0.12 };
    ctx.global_style_mut(|s| {
        use egui::TextStyle::*;
        s.text_styles = [
            (Small, FontId::new(13.0, FontFamily::Proportional)),
            (Body, FontId::new(16.5, FontFamily::Proportional)),
            (Button, FontId::new(16.5, FontFamily::Proportional)),
            (Heading, FontId::new(23.0, gothic_family())),
            (Monospace, FontId::new(13.5, FontFamily::Monospace)),
        ]
        .into();
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.spacing.button_padding = vec2(10.0, 3.0);
        s.spacing.window_margin = Margin::same(MARGIN);
        s.spacing.interact_size.y = 24.0;
        s.spacing.slider_width = 140.0;
        s.spacing.scroll = egui::style::ScrollStyle::solid();
        s.animation_time = anim;
    });
}

/// Frame for side panels (content is textured by [`panel_bg`]).
pub fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PARCHMENT)
        .inner_margin(Margin::same(PANEL_MARGIN))
}

/// Frame for the top/bottom toolbars (dark wood).
pub fn toolbar_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(WOOD)
        .inner_margin(Margin::symmetric(10, 5))
}

/// A themed window: parchment frame, gothic title on a darker band. Call [`window_bg`] first in the
/// content closure to get the texture and ornaments.
pub fn win(title: impl Into<egui::WidgetText>) -> egui::Window<'static> {
    egui::Window::new(title).frame(
        egui::Frame::window(&egui::Style::default())
            .fill(PARCHMENT)
            .stroke(Stroke::new(2.0, WOOD))
            .corner_radius(4)
            .inner_margin(Margin::same(MARGIN))
            .shadow(egui::Shadow {
                offset: [4, 8],
                blur: 22,
                spread: 2,
                color: Color32::from_black_alpha(150),
            }),
    )
}

/// Restyles widgets inside `ui` for dark wood/leather surfaces (light text, bronze buttons).
pub fn dark_scope(ui: &mut egui::Ui) {
    let v = ui.visuals_mut();
    v.override_text_color = Some(IVORY);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, IVORY);
    v.widgets.inactive.weak_bg_fill = rgb(0x4A, 0x34, 0x26);
    v.widgets.inactive.bg_fill = rgb(0x4A, 0x34, 0x26);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BRONZE);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, IVORY);
    v.widgets.hovered.weak_bg_fill = rgb(0x6B, 0x4C, 0x33);
    v.widgets.hovered.bg_fill = rgb(0x6B, 0x4C, 0x33);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, GOLD_BRIGHT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, INK);
    v.extreme_bg_color = rgb(0x24, 0x1A, 0x14);
    v.selection.bg_fill = GOLD.gamma_multiply(0.7);
}

// ---------------------------------------------------------------- components

/// Ornamental section heading: gothic text between gold rules with diamond fleurons.
pub fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    let font = FontId::new(20.0, gothic_family());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, WOOD);
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, galley.size().y + 4.0), egui::Sense::hover());
    let painter = ui.painter();
    let text_pos = pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 2.0);
    let cy = rect.center().y + 2.0;
    let gap = galley.size().x / 2.0 + 14.0;
    for side in [-1.0f32, 1.0] {
        let inner = rect.center().x + side * gap;
        let outer = if side < 0.0 {
            rect.left() + 4.0
        } else {
            rect.right() - 4.0
        };
        if (outer - inner) * side > 8.0 {
            painter.line_segment([pos2(inner, cy), pos2(outer, cy)], Stroke::new(1.2, GOLD));
            painter.line_segment(
                [
                    pos2(inner, cy + 3.0),
                    pos2(inner + (outer - inner) * 0.6, cy + 3.0),
                ],
                Stroke::new(0.6, BRONZE),
            );
            let d = pos2(inner - side * 3.0, cy);
            painter.add(Shape::convex_polygon(
                vec![
                    d + vec2(0.0, -4.0),
                    d + vec2(4.0, 0.0),
                    d + vec2(0.0, 4.0),
                    d + vec2(-4.0, 0.0),
                ],
                GOLD,
                Stroke::new(0.8, INK),
            ));
        }
    }
    painter.galley(text_pos, galley, WOOD);
    ui.add_space(4.0);
}

/// Large Blackletter title with a soft shadow (menus, campaign and room names).
pub fn title(ui: &mut egui::Ui, text: &str, size: f32, color: Color32) -> egui::Response {
    let font = FontId::new(size, display_family());
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), color);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(4.0, 4.0), egui::Sense::hover());
    let shadow = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, Color32::from_black_alpha(170));
    ui.painter().galley(
        rect.min + vec2(3.0, 3.0),
        shadow,
        Color32::from_black_alpha(170),
    );
    ui.painter().galley(rect.min, galley, color);
    resp
}

/// Painted resource bar: iron frame, recessed trough, glossy fill and centered value.
pub fn bar(ui: &mut egui::Ui, label: &str, cur: i32, max: i32, color: Color32) {
    ui.horizontal(|ui| {
        ui.label(bold(label));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(bold(format!("{cur} / {max}")))
        });
    });
    let frac = if max > 0 {
        (cur.max(0) as f32 / max as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 12.0), egui::Sense::hover());
    paint_bar(ui.painter(), rect, frac, color);
}

pub fn paint_bar(painter: &egui::Painter, rect: Rect, frac: f32, color: Color32) {
    painter.rect_filled(rect, 3.0, IRON);
    let trough = rect.shrink(2.0);
    painter.rect_filled(trough, 2.0, rgb(0x1E, 0x18, 0x14));
    if frac > 0.0 {
        let fill = Rect::from_min_size(trough.min, vec2(trough.width() * frac, trough.height()));
        let light = color.lerp_to_gamma(Color32::WHITE, 0.28);
        let dark = color.lerp_to_gamma(Color32::BLACK, 0.3);
        paint_gradient(painter, fill, light, dark);
        painter.hline(
            fill.x_range(),
            fill.top() + 1.5,
            Stroke::new(1.0, Color32::from_white_alpha(70)),
        );
    }
    // Segment ticks every 10%.
    for i in 1..10 {
        let x = trough.left() + trough.width() * i as f32 / 10.0;
        painter.vline(
            x,
            trough.y_range(),
            Stroke::new(0.6, Color32::from_black_alpha(70)),
        );
    }
    painter.rect_stroke(rect, 3.0, Stroke::new(1.0, INK), egui::StrokeKind::Inside);
}

/// Spell-slot / resource gems: filled gems for what's left, empty sockets for what's spent.
pub fn pips(ui: &mut egui::Ui, left: u32, total: u32, color: Color32) {
    let d = 13.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total as f32 * (d + 3.0), d), egui::Sense::hover());
    let p = ui.painter();
    for i in 0..total {
        let c = pos2(
            rect.left() + d / 2.0 + i as f32 * (d + 3.0),
            rect.center().y,
        );
        p.circle_filled(c, d / 2.0, BRONZE);
        if i < left {
            p.circle_filled(c, d / 2.0 - 1.8, color);
            p.circle_filled(c + vec2(-1.8, -1.8), 1.8, Color32::from_white_alpha(170));
        } else {
            p.circle_filled(c, d / 2.0 - 1.8, rgb(0x2A, 0x22, 0x1D));
        }
        p.circle_stroke(c, d / 2.0, Stroke::new(1.0, INK));
    }
}

/// A big plaque-style button with an emblem icon (main menu, primary actions). Animated hover glow.
pub fn plaque_button(
    ui: &mut egui::Ui,
    icon: crate::icons::Icon,
    text: &str,
    width: f32,
) -> egui::Response {
    let height = 48.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), egui::Sense::click());
    let t = if reduced_motion(ui.ctx()) {
        if resp.hovered() { 1.0 } else { 0.0 }
    } else {
        ui.ctx()
            .animate_bool_with_time(resp.id, resp.hovered(), 0.18)
    };
    let p = ui.painter();
    let m = Materials::get(ui.ctx());
    let r = rect.translate(vec2(4.0 * t, 0.0));
    if t > 0.0 {
        p.rect_filled(
            r.expand(4.0 + 3.0 * t),
            8.0,
            GOLD_BRIGHT.gamma_multiply(0.18 * t),
        );
    }
    p.rect_filled(
        r.translate(vec2(2.0, 3.0)),
        5.0,
        Color32::from_black_alpha(120),
    );
    let inner = r.shrink(1.0);
    paint_material(&p.with_clip_rect(inner), inner, &m.wood, Color32::WHITE);
    p.rect_stroke(
        r,
        5.0,
        Stroke::new(2.0, if t > 0.5 { GOLD_BRIGHT } else { BRONZE }),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        r.shrink(4.0),
        3.0,
        Stroke::new(0.8, GOLD.gamma_multiply(0.6 + 0.4 * t)),
        egui::StrokeKind::Inside,
    );
    for x in [r.left() + 9.0, r.right() - 9.0] {
        for y in [r.top() + 9.0, r.bottom() - 9.0] {
            p.circle_filled(pos2(x, y), 2.2, GOLD.gamma_multiply(0.8));
        }
    }
    let icon_rect = Rect::from_center_size(pos2(r.left() + 34.0, r.center().y), vec2(34.0, 34.0));
    crate::icons::paint(p, icon_rect, icon);
    let color = IVORY.lerp_to_gamma(GOLD_BRIGHT, t);
    p.text(
        pos2(r.left() + 62.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        FontId::new(22.0, gothic_family()),
        color,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Small icon button with tooltip (toolbars).
pub fn icon_button(
    ui: &mut egui::Ui,
    icon: crate::icons::Icon,
    selected: bool,
    tooltip: &str,
) -> egui::Response {
    let size = vec2(32.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let p = ui.painter();
    let hovered = resp.hovered();
    let fill = if selected {
        GOLD
    } else if hovered {
        rgb(0x6B, 0x4C, 0x33)
    } else {
        rgb(0x4A, 0x34, 0x26)
    };
    p.rect_filled(rect, 4.0, fill);
    p.rect_stroke(
        rect,
        4.0,
        Stroke::new(
            if selected || hovered { 1.6 } else { 1.0 },
            if selected { IVORY } else { BRONZE },
        ),
        egui::StrokeKind::Inside,
    );
    crate::icons::paint(p, rect.shrink(4.0), icon);
    resp.on_hover_text(tooltip)
}

/// Banner (notification) painted as a hanging cloth strip with a gothic caption.
pub fn banner(painter: &egui::Painter, center: Pos2, text: &str, size: f32, alpha: f32) {
    let font = FontId::new(size, display_family());
    let galley = painter.layout_no_wrap(text.to_owned(), font, GOLD_BRIGHT.gamma_multiply(alpha));
    let w = galley.size().x + size * 2.2;
    let h = galley.size().y + size * 0.6;
    let rect = Rect::from_center_size(center, vec2(w, h));
    let a = |c: Color32| c.gamma_multiply(alpha);
    painter.rect_filled(
        rect.translate(vec2(3.0, 5.0)),
        2.0,
        Color32::from_black_alpha((110.0 * alpha) as u8),
    );
    // Swallow-tail ends.
    for side in [-1.0f32, 1.0] {
        let x = if side < 0.0 {
            rect.left()
        } else {
            rect.right()
        };
        let tail = vec![
            pos2(x, rect.top()),
            pos2(x + side * size * 0.9, rect.top()),
            pos2(x + side * size * 0.45, rect.center().y),
            pos2(x + side * size * 0.9, rect.bottom()),
            pos2(x, rect.bottom()),
        ];
        painter.add(Shape::convex_polygon(
            tail[..3].to_vec(),
            a(rgb(0x4E, 0x22, 0x20)),
            Stroke::NONE,
        ));
        painter.add(Shape::convex_polygon(
            vec![tail[0], tail[2], tail[3], tail[4]],
            a(rgb(0x4E, 0x22, 0x20)),
            Stroke::NONE,
        ));
    }
    painter.rect_filled(rect, 1.0, a(BURGUNDY));
    painter.rect_stroke(
        rect.shrink(3.0),
        0.0,
        Stroke::new(1.2, a(GOLD)),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.center() - galley.size() / 2.0, galley, GOLD_BRIGHT);
}

/// Replaces characters the bundled UI fonts can't draw (they would show as empty boxes).
pub fn glyph_safe(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains('→') {
        std::borrow::Cow::Owned(text.replace('→', "»"))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn noise_tiles_seamlessly() {
        // Left/right and top/bottom edges must match for tileable materials.
        for i in 0..64 {
            let a = super::fbm(0, i * 4, 256, 4, 3, 1);
            let b = super::fbm(256, i * 4, 256, 4, 3, 1);
            assert!((a - b).abs() < 1e-4, "{a} vs {b}");
        }
    }

    #[test]
    fn fonts_cover_italian_and_ui_symbols() {
        // Every embedded font must contain the Italian accented letters.
        for (name, bytes) in [
            (
                "UnifrakturCook",
                &include_bytes!("../assets/fonts/UnifrakturCook-Bold.ttf")[..],
            ),
            (
                "GrenzeGotisch",
                &include_bytes!("../assets/fonts/GrenzeGotisch.ttf")[..],
            ),
            (
                "CrimsonText",
                &include_bytes!("../assets/fonts/CrimsonText-Regular.ttf")[..],
            ),
        ] {
            let face = ttf_parser::Face::parse(bytes, 0).expect(name);
            for c in "àèéìòùÀÈÉÌÒÙ’—«»".chars() {
                assert!(face.glyph_index(c).is_some(), "{name} lacks {c}");
            }
        }
    }
}
