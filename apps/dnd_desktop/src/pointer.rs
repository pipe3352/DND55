//! Decides whether the mouse is over the **map canvas** or over the user interface.
//!
//! The game lays out its panels inside a hand-made root `Ui` on egui's background layer. For that
//! layer, egui's own `is_pointer_over_egui()` / bevy_egui's `EguiWantsInput` compare the pointer
//! with a root "available rect" that this layout never updates, so a pointer over the asset library
//! looked like a pointer over the map: the wheel scrolled the list *and* zoomed the map, and clicks
//! could reach the map.
//!
//! Fix: after the panels are laid out, the remaining rectangle *is* the map canvas. The pointer is
//! over the map only when it is inside that rectangle, no window/popup/menu (a non-background layer)
//! is on top of it, and egui is not dragging a widget. Map input (zoom, pan, clicks, hover) reads
//! [`MapPointer`] instead of egui's generic flags.

use bevy::prelude::Resource;
use bevy_egui::egui;

/// Written by the game UI each frame; read by camera and map input systems.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct MapPointer {
    /// True only when wheel/clicks/drags should reach the map.
    pub over_map: bool,
    /// The map canvas in egui points (left, top, right, bottom), when the game UI is shown.
    pub map_rect: Option<[f32; 4]>,
}

/// Is `pos` over the map canvas `map_rect`, with nothing of the UI on top?
pub fn over_map_at(ctx: &egui::Context, map_rect: egui::Rect, pos: egui::Pos2) -> bool {
    if !map_rect.contains(pos) {
        return false;
    }
    // Windows, popups, menus, tooltips and dialogs live on non-background layers.
    if ctx
        .layer_id_at(pos)
        .is_some_and(|l| l.order != egui::Order::Background)
    {
        return false;
    }
    true
}

/// Full check for the current pointer (also blocks while egui drags a widget or a menu is open).
pub fn compute(ctx: &egui::Context, map_rect: egui::Rect) -> MapPointer {
    let over = ctx
        .pointer_latest_pos()
        .is_some_and(|p| over_map_at(ctx, map_rect, p))
        && ctx.dragged_id().is_none()
        && !ctx.any_popup_open();
    MapPointer {
        over_map: over,
        map_rect: Some([
            map_rect.left(),
            map_rect.top(),
            map_rect.right(),
            map_rect.bottom(),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Pos2, RawInput, Rect, pos2, vec2};

    /// Runs one frame of a layout shaped like the game's: a hand-made background root `Ui`, a left
    /// asset panel with a scroll area, a right properties panel, optionally a window over the map.
    /// Returns (map pointer state, how far the asset list scrolled).
    fn frame(
        ctx: &egui::Context,
        pointer: Pos2,
        wheel: f32,
        window_over_map: bool,
    ) -> (MapPointer, f32) {
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0))),
            time: Some(ctx.cumulative_pass_nr() as f64 / 30.0),
            predicted_dt: 1.0 / 30.0,
            ..Default::default()
        };
        input.events.push(Event::PointerMoved(pointer));
        if wheel != 0.0 {
            input.events.push(Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, wheel),
                modifiers: Default::default(),
                phase: egui::TouchPhase::Move,
            });
        }
        ctx.begin_pass(input);
        let mut root = egui::Ui::new(
            ctx.clone(),
            "game-root".into(),
            egui::UiBuilder::new()
                .layer_id(egui::LayerId::background())
                .max_rect(ctx.viewport_rect()),
        );
        let mut scrolled = 0.0;
        egui::Panel::left("assets")
            .exact_size(300.0)
            .show(&mut root, |ui| {
                let out = egui::ScrollArea::vertical()
                    .id_salt("asset-list")
                    .show(ui, |ui| {
                        for i in 0..200 {
                            ui.label(format!("asset {i}"));
                        }
                    });
                scrolled = out.state.offset.y;
            });
        egui::Panel::right("props")
            .exact_size(300.0)
            .show(&mut root, |ui| {
                ui.label("properties");
            });
        let map_rect = root.available_rect_before_wrap();
        if window_over_map {
            egui::Window::new("dialog")
                .fixed_pos(pos2(700.0, 400.0))
                .fixed_size(vec2(200.0, 200.0))
                .show(ctx, |ui| {
                    ui.label("modal");
                });
        }
        let mp = compute(ctx, map_rect);
        ctx.end_pass().textures_delta.clear();
        (mp, scrolled)
    }

    #[test]
    fn wheel_over_the_asset_library_never_reaches_the_map() {
        let ctx = egui::Context::default();
        let over_assets = pos2(150.0, 400.0);
        for _ in 0..3 {
            frame(&ctx, over_assets, 0.0, false); // let layers settle
        }
        for y in [60.0, 400.0, 880.0] {
            let (mp, _) = frame(&ctx, pos2(150.0, y), -120.0, false);
            assert!(
                !mp.over_map,
                "pointer over the asset panel must not count as the map"
            );
        }
    }

    #[test]
    fn wheel_over_the_map_reaches_the_map_but_not_through_windows() {
        let ctx = egui::Context::default();
        let over_map = pos2(800.0, 450.0);
        for _ in 0..3 {
            frame(&ctx, over_map, 0.0, false);
        }
        let (mp, off) = frame(&ctx, over_map, -120.0, false);
        assert!(mp.over_map, "the free centre area is the map canvas");
        assert_eq!(
            off, 0.0,
            "the asset list must not scroll when the wheel is over the map"
        );
        // The properties panel on the right is UI too.
        let (mp, _) = frame(&ctx, pos2(1500.0, 450.0), 0.0, false);
        assert!(!mp.over_map);
        // A window (dialog) over the map blocks it.
        for _ in 0..3 {
            frame(&ctx, over_map, 0.0, true);
        }
        let (mp, _) = frame(&ctx, over_map, -120.0, true);
        assert!(!mp.over_map, "a window over the map must capture the wheel");
    }
}
