//! Medieval fantasy look for egui: parchment panels, dark wood frames, gold accents, serif-like spacing.

use bevy_egui::egui::{self, Color32, CornerRadius, Margin, Stroke, Visuals};

pub const PARCHMENT: Color32 = Color32::from_rgb(232, 216, 178);
pub const PARCHMENT_DARK: Color32 = Color32::from_rgb(214, 193, 148);
pub const INK: Color32 = Color32::from_rgb(52, 36, 22);
pub const WOOD: Color32 = Color32::from_rgb(58, 38, 24);
pub const WOOD_LIGHT: Color32 = Color32::from_rgb(92, 62, 38);
pub const GOLD: Color32 = Color32::from_rgb(201, 162, 72);
pub const BLOOD: Color32 = Color32::from_rgb(150, 30, 26);
pub const MANA: Color32 = Color32::from_rgb(52, 92, 160);
pub const HEALTH: Color32 = Color32::from_rgb(70, 130, 60);

pub fn apply(ctx: &egui::Context, scale: f32) {
    let mut v = Visuals::light();
    v.override_text_color = Some(INK);
    v.panel_fill = PARCHMENT;
    v.window_fill = PARCHMENT;
    v.extreme_bg_color = Color32::from_rgb(244, 233, 206);
    v.faint_bg_color = PARCHMENT_DARK;
    v.window_stroke = Stroke::new(2.0, WOOD_LIGHT);
    v.window_corner_radius = CornerRadius::same(6);
    v.selection.bg_fill = GOLD;
    v.selection.stroke = Stroke::new(1.0, INK);
    v.hyperlink_color = BLOOD;
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(4);
        w.fg_stroke = Stroke::new(1.0, INK);
    }
    v.widgets.noninteractive.bg_fill = PARCHMENT;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(170, 140, 96));
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, INK);
    v.widgets.inactive.bg_fill = PARCHMENT_DARK;
    v.widgets.inactive.weak_bg_fill = PARCHMENT_DARK;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, WOOD_LIGHT);
    v.widgets.hovered.bg_fill = Color32::from_rgb(236, 206, 140);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(236, 206, 140);
    v.widgets.hovered.bg_stroke = Stroke::new(1.5, GOLD);
    v.widgets.active.bg_fill = GOLD;
    v.widgets.active.weak_bg_fill = GOLD;
    ctx.set_visuals(v);
    ctx.set_zoom_factor(scale);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 4.0);
        s.spacing.window_margin = Margin::same(12);
    });
}

/// Frame for side panels: dark wood border around parchment.
pub fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PARCHMENT)
        .stroke(Stroke::new(3.0, WOOD))
        .inner_margin(Margin::same(10))
}

/// Heading with flourishes, e.g. "— Skills —" (glyphs limited to egui's bundled fonts).
pub fn heading(ui: &mut egui::Ui, text: &str) {
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(format!("— {text} —"))
                .size(18.0)
                .color(WOOD)
                .strong(),
        );
    });
    ui.add_space(2.0);
    let r = ui.available_rect_before_wrap();
    ui.painter()
        .hline(r.x_range(), r.top(), Stroke::new(1.0, GOLD));
    ui.add_space(6.0);
}

/// A labelled resource bar (Health, etc.).
pub fn bar(ui: &mut egui::Ui, label: &str, cur: i32, max: i32, color: Color32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("{cur} / {max}"))
        });
    });
    let frac = if max > 0 {
        (cur.max(0) as f32 / max as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ui.add(
        egui::ProgressBar::new(frac)
            .fill(color)
            .desired_height(10.0),
    );
}

/// Replaces characters the bundled UI fonts can't draw (they would show as empty boxes).
pub fn glyph_safe(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains('→') {
        std::borrow::Cow::Owned(text.replace('→', "»"))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}
