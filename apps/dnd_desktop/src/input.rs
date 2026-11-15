//! Camera controls, map clicks and editor tools.

use crate::state::{CELL, GameView, Net, Tool, UiState, cell_to_world, world_to_cell};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use dnd_session::command::{Command, DmCommand};
use dnd_world::path::{EmptyContext, find_path};

#[derive(Resource, Default)]
pub struct Hover {
    pub cell: Option<(i32, i32)>,
    pub world: Vec2,
    /// Cells already painted during the current pencil/eraser drag.
    pub stroke: Vec<(i32, i32)>,
}

pub fn camera_controls(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    motion: Res<AccumulatedMouseMotion>,
    egui: Res<EguiWantsInput>,
    time: Res<Time>,
    cam: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let (mut t, mut proj) = cam.into_inner();
    let Projection::Orthographic(o) = &mut *proj else {
        return;
    };
    if !egui.wants_any_keyboard_input() {
        let mut d = Vec2::ZERO;
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            d.y += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            d.y -= 1.0;
        }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            d.x -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            d.x += 1.0;
        }
        t.translation += (d * 700.0 * o.scale * time.delta_secs()).extend(0.0);
    }
    if !egui.wants_any_pointer_input() {
        if scroll.delta.y != 0.0 {
            o.scale = (o.scale * (1.0 - scroll.delta.y.signum() * 0.1)).clamp(0.25, 4.0);
        }
        if buttons.pressed(MouseButton::Middle)
            || buttons.pressed(MouseButton::Right) && keys.pressed(KeyCode::Space)
        {
            t.translation.x -= motion.delta.x * o.scale;
            t.translation.y += motion.delta.y * o.scale;
        }
    }
}

pub fn track_hover(
    window: Single<&Window>,
    cam: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    gv: Res<GameView>,
    mut hover: ResMut<Hover>,
) {
    let (camera, ct) = *cam;
    hover.cell = None;
    let Some(map) = &gv.map else { return };
    if let Some(p) = window
        .cursor_position()
        .and_then(|c| camera.viewport_to_world_2d(ct, c).ok())
    {
        hover.world = p;
        let c = world_to_cell(p);
        if map.in_bounds(c) {
            hover.cell = Some(c);
        }
    }
}

pub fn map_click(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    egui: Res<EguiWantsInput>,
    mut hover: ResMut<Hover>,
    mut ui: ResMut<UiState>,
    mut net: ResMut<Net>,
    gv: Res<GameView>,
) {
    if buttons.just_pressed(MouseButton::Right) && !keys.pressed(KeyCode::Space) {
        ui.targeting = None;
        ui.rect_start = None;
        if ui.tool != Tool::Play {
            ui.tool = Tool::Play;
        } else {
            ui.selected = None;
        }
        return;
    }
    if egui.wants_any_pointer_input() {
        return;
    }
    let Some(cell) = hover.cell else { return };
    let Some(view) = &gv.view else { return };
    let Some(room) = &view.room else { return };
    let room_id = room.id;
    let layer = ui.layer;
    let asset = ui.asset.clone();
    match ui.tool {
        Tool::Pencil | Tool::Eraser => {
            if buttons.just_pressed(MouseButton::Left) {
                hover.stroke.clear();
            }
            if buttons.pressed(MouseButton::Left) && !hover.stroke.contains(&cell) {
                hover.stroke.push(cell);
                let a = if ui.tool == Tool::Pencil {
                    Some(asset)
                } else {
                    None
                };
                net.cmd(Command::Dm(DmCommand::PaintTiles {
                    room: room_id,
                    layer,
                    cells: vec![cell],
                    asset: a,
                }));
            }
        }
        Tool::Rect => {
            if buttons.just_pressed(MouseButton::Left) {
                ui.rect_start = Some(cell);
            }
            if buttons.just_released(MouseButton::Left)
                && let Some(a) = ui.rect_start.take()
            {
                net.cmd(Command::Dm(DmCommand::FillRect {
                    room: room_id,
                    layer,
                    a,
                    b: cell,
                    asset: Some(asset),
                }));
            }
        }
        _ if !buttons.just_pressed(MouseButton::Left) => {}
        Tool::Fill => {
            net.cmd(Command::Dm(DmCommand::FloodFill {
                room: room_id,
                layer,
                start: cell,
                asset: Some(asset),
            }));
        }
        Tool::Object => {
            let hidden = ui.object_hidden;
            net.cmd(Command::Dm(DmCommand::PlaceObject {
                room: room_id,
                asset,
                cell,
                hidden,
            }));
        }
        Tool::RemoveObject => {
            if let Some(o) = gv
                .map
                .as_ref()
                .and_then(|m| m.objects.iter().rev().find(|o| o.covers(cell)))
            {
                net.cmd(Command::Dm(DmCommand::RemoveObject {
                    room: room_id,
                    object: o.id,
                }));
            }
        }
        Tool::Spawn => {
            let (template, hidden) = (ui.monster.clone(), ui.spawn_hidden);
            net.cmd(Command::Dm(DmCommand::SpawnMonster {
                template,
                room: room_id,
                cell,
                hidden,
            }));
        }
        Tool::Entry => {
            net.cmd(Command::Dm(DmCommand::SetEntryPoint {
                room: room_id,
                cell,
            }));
        }
        Tool::Light => {
            let (bright_ft, dim_ft) = (ui.light_bright, ui.light_dim);
            net.cmd(Command::Dm(DmCommand::AddLight {
                room: room_id,
                cell,
                bright_ft,
                dim_ft,
            }));
        }
        Tool::Play => {
            let clicked = view.creatures.iter().find(|c| {
                c.pos == cell
                    && !matches!(
                        c.hp,
                        dnd_session::view::HpView::Status(dnd_session::view::HealthStatus::Dead)
                    )
            });
            if let (Some(attack), Some(target), Some(me)) = (ui.targeting, clicked, ui.selected)
                && target.id != me
            {
                net.cmd(Command::Attack {
                    attacker: me,
                    target: target.id,
                    attack,
                });
                ui.targeting = None;
                return;
            }
            if let Some(c) = clicked {
                if c.yours || net.is_dm() {
                    ui.selected = Some(c.id);
                }
                return;
            }
            let Some(me) = ui.selected else { return };
            if keys.pressed(KeyCode::ShiftLeft) && net.is_dm() {
                net.cmd(Command::Dm(DmCommand::Teleport { entity: me, cell }));
                return;
            }
            let interactable = gv.map.as_ref().and_then(|m| {
                m.objects
                    .iter()
                    .find(|o| o.interactable && !o.hidden && o.covers(cell))
            });
            match interactable {
                Some(o) => {
                    net.cmd(Command::Interact {
                        entity: me,
                        object: o.id,
                    });
                }
                None => {
                    net.cmd(Command::Move {
                        entity: me,
                        to: cell,
                    });
                }
            }
        }
    }
}

/// Grid, hover, selection, path preview, turn marker and editor previews.
pub fn draw_overlays(
    mut gizmos: Gizmos,
    gv: Res<GameView>,
    ui: Res<UiState>,
    hover: Res<Hover>,
    net: Res<Net>,
    time: Res<Time>,
) {
    let Some(map) = &gv.map else { return };
    let Some(view) = &gv.view else { return };
    let (w, h) = (map.width as f32 * CELL, map.height as f32 * CELL);
    if ui.show_grid {
        let c = Color::srgba(0.0, 0.0, 0.0, 0.28);
        for x in 0..=map.width {
            gizmos.line_2d(
                Vec2::new(x as f32 * CELL, 0.0),
                Vec2::new(x as f32 * CELL, -h),
                c,
            );
        }
        for y in 0..=map.height {
            gizmos.line_2d(
                Vec2::new(0.0, -(y as f32) * CELL),
                Vec2::new(w, -(y as f32) * CELL),
                c,
            );
        }
    }
    gizmos.rect_2d(
        Vec2::new(w / 2.0, -h / 2.0),
        Vec2::new(w, h),
        Color::srgb(0.79, 0.64, 0.28),
    );

    // Active creature marker.
    if let Some(active) = view.combat.as_ref().and_then(|c| c.active)
        && let Some(c) = view.creatures.iter().find(|c| c.id == active)
    {
        let pulse = 0.5 + 0.5 * (time.elapsed_secs() * 4.0).sin();
        gizmos.circle_2d(
            cell_to_world(c.pos),
            CELL * (0.52 + 0.04 * pulse),
            Color::srgb(1.0, 0.85, 0.3),
        );
    }
    if let Some(sel) = ui
        .selected
        .and_then(|id| view.creatures.iter().find(|c| c.id == id))
    {
        gizmos.circle_2d(
            cell_to_world(sel.pos),
            CELL * 0.47,
            Color::srgb(0.4, 0.9, 1.0),
        );
        // Path preview with movement cost (client-side estimate; the host decides).
        if ui.tool == Tool::Play
            && ui.targeting.is_none()
            && let Some(cell) = hover.cell
            && cell != sel.pos
            && let Some(path) = find_path(
                map,
                &EmptyContext,
                sel.pos,
                cell,
                Default::default(),
                i32::MAX,
            )
        {
            let budget = view
                .combat
                .as_ref()
                .and_then(|c| c.turn.as_ref())
                .map(|t| t.movement_left_ft)
                .unwrap_or(i32::MAX);
            let color = if path.total_ft <= budget {
                Color::srgb(0.3, 0.9, 0.4)
            } else {
                Color::srgb(0.9, 0.3, 0.2)
            };
            let mut prev = cell_to_world(sel.pos);
            let mut spent = 0;
            for s in &path.steps {
                spent += s.cost_ft;
                let p = cell_to_world(s.cell);
                gizmos.line_2d(
                    prev,
                    p,
                    if spent <= budget {
                        color
                    } else {
                        Color::srgb(0.9, 0.3, 0.2)
                    },
                );
                prev = p;
            }
        }
        if let Some(attack) = ui.targeting {
            let reach = sel
                .attacks
                .get(attack)
                .map(|a| a.range_ft.map(|r| r.1).unwrap_or(a.reach_ft))
                .unwrap_or(5) as f32;
            let r = (reach / 5.0 + 0.5) * CELL;
            gizmos.rect_2d(
                cell_to_world(sel.pos),
                Vec2::splat(r * 2.0),
                Color::srgba(1.0, 0.3, 0.2, 0.6),
            );
        }
    }
    if let Some(c) = hover.cell {
        let color = match ui.tool {
            Tool::Play if ui.targeting.is_some() => Color::srgb(1.0, 0.25, 0.2),
            Tool::Play => Color::srgba(1.0, 1.0, 1.0, 0.5),
            Tool::Eraser | Tool::RemoveObject => Color::srgb(1.0, 0.4, 0.3),
            _ => Color::srgb(1.0, 0.85, 0.3),
        };
        gizmos.rect_2d(cell_to_world(c), Vec2::splat(CELL - 2.0), color);
        if let (Tool::Rect, Some(a)) = (ui.tool, ui.rect_start) {
            let (x0, x1) = (a.0.min(c.0), a.0.max(c.0));
            let (y0, y1) = (a.1.min(c.1), a.1.max(c.1));
            let center = (cell_to_world((x0, y0)) + cell_to_world((x1, y1))) / 2.0;
            gizmos.rect_2d(
                center,
                Vec2::new((x1 - x0 + 1) as f32 * CELL, (y1 - y0 + 1) as f32 * CELL),
                Color::srgb(1.0, 0.85, 0.3),
            );
        }
    }
    if net.is_dm()
        && let Some(r) = &view.room
    {
        let p = cell_to_world(r.entry);
        gizmos.circle_2d(p, CELL * 0.25, Color::srgb(0.3, 1.0, 0.5));
        gizmos.line_2d(
            p - Vec2::X * 8.0,
            p + Vec2::X * 8.0,
            Color::srgb(0.3, 1.0, 0.5),
        );
    }
}
