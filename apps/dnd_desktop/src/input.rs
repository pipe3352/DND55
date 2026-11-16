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

/// Camera framing requests (fit the room on entering it, or from the editor's Fit button).
#[derive(Resource, Default)]
pub struct CameraFit {
    pub request: bool,
    /// Centre the camera on this world point (minimap clicks).
    pub goto: Option<(f32, f32)>,
    /// Visible world rectangle (left, top, right, bottom) for the minimap.
    pub camera_view: Option<(f32, f32, f32, f32)>,
    /// Smooth-zoom target (orthographic scale).
    pub target_scale: Option<f32>,
    last_room: Option<dnd_world::RoomId>,
}

#[allow(clippy::too_many_arguments)]
pub fn camera_controls(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    motion: Res<AccumulatedMouseMotion>,
    egui: Res<EguiWantsInput>,
    time: Res<Time>,
    cam: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
    window: Single<&Window>,
    gv: Res<GameView>,
    net: Res<Net>,
    mut fit: ResMut<CameraFit>,
) {
    let (mut t, mut proj) = cam.into_inner();
    let Projection::Orthographic(o) = &mut *proj else {
        return;
    };
    // Frame the whole room in the free area between the UI panels when it changes.
    let room = gv.view.as_ref().and_then(|v| v.room.as_ref()).map(|r| r.id);
    if room.is_some() && room != fit.last_room {
        fit.last_room = room;
        fit.request = true;
    }
    if fit.request
        && let Some(map) = &gv.map
    {
        fit.request = false;
        let (ww, wh) = (window.width(), window.height());
        let (left, right, top, bottom) =
            (if net.is_dm() { 380.0 } else { 20.0 }, 330.0, 44.0, 175.0);
        let (aw, ah) = (
            (ww - left - right).max(200.0),
            (wh - top - bottom).max(150.0),
        );
        let size = Vec2::new(map.width as f32 * CELL, map.height as f32 * CELL);
        let scale = (size.x / aw).max(size.y / ah).clamp(0.25, 4.0) * 1.04;
        o.scale = scale;
        fit.target_scale = Some(scale);
        // Offset so the map sits in the centre of the free area, not of the window.
        let off = Vec2::new((left - right) / 2.0, (bottom - top) / 2.0) * scale;
        t.translation.x = size.x / 2.0 - off.x;
        t.translation.y = -size.y / 2.0 - off.y;
    }
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
            let target = fit.target_scale.unwrap_or(o.scale);
            fit.target_scale =
                Some((target * (1.0 - scroll.delta.y.signum() * 0.12)).clamp(0.25, 4.0));
        }
        if buttons.pressed(MouseButton::Middle)
            || buttons.pressed(MouseButton::Right) && keys.pressed(KeyCode::Space)
        {
            t.translation.x -= motion.delta.x * o.scale;
            t.translation.y += motion.delta.y * o.scale;
        }
    }
    if let Some((x, y)) = fit.goto.take() {
        t.translation.x = x;
        t.translation.y = y;
    }
    let half = Vec2::new(window.width(), window.height()) * o.scale / 2.0;
    fit.camera_view = Some((
        t.translation.x - half.x,
        t.translation.y + half.y,
        t.translation.x + half.x,
        t.translation.y - half.y,
    ));
    // Smooth zoom towards the target.
    if let Some(target) = fit.target_scale {
        let k = (time.delta_secs() * 12.0).min(1.0);
        o.scale += (target - o.scale) * k;
        if (o.scale - target).abs() < 0.0005 {
            o.scale = target;
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
    window: Single<&Window>,
) {
    // Right-click on an object with the Select tool opens its context menu.
    if buttons.just_pressed(MouseButton::Right)
        && !keys.pressed(KeyCode::Space)
        && ui.tool == Tool::Select
        && net.is_dm()
        && !egui.wants_any_pointer_input()
        && let (Some(cell), Some(map), Some(cursor)) =
            (hover.cell, &gv.map, window.cursor_position())
        && let Some(o) = map.objects.iter().rev().find(|o| o.covers(cell))
    {
        let k = ui.ui_scale.max(0.1);
        ui.map_context = Some(((cursor.x / k, cursor.y / k), o.id));
        return;
    }
    if buttons.just_pressed(MouseButton::Right) && !keys.pressed(KeyCode::Space) {
        ui.targeting = None;
        ui.rect_start = None;
        if ui.casting.take().is_some() || ui.feature_target.take().is_some() {
            return;
        }
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
    let Some(view) = &gv.view else { return };
    let Some(room) = &view.room else { return };
    let room_id = room.id;
    // Drawing works with fractional positions and may end outside the map.
    if ui.tool == Tool::Draw {
        draw_input(&buttons, &hover, &mut ui, &mut net, room_id);
        return;
    }
    let Some(cell) = hover.cell else { return };
    if net.is_dm()
        && buttons.just_pressed(MouseButton::Left)
        && let Some(map) = &gv.map
        && crate::ui_dm::script_pick_click(&mut ui, map, cell)
    {
        return;
    }
    let layer = ui.layer;
    let asset = ui.asset.clone();
    match ui.tool {
        Tool::Select => {
            select_input(&buttons, &keys, cell, &mut ui, &mut net, &gv, room_id);
        }
        Tool::Draw => {}
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
            if (ui.casting.is_some() || ui.feature_target.is_some())
                && crate::ui_play::targeting_click(&mut ui, &mut net, cell, clicked.map(|c| c.id))
            {
                return;
            }
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

fn objects_at(map: &dnd_world::map::GridMap, cell: (i32, i32)) -> Option<dnd_world::ObjectId> {
    map.objects
        .iter()
        .rev()
        .find(|o| o.covers(cell))
        .map(|o| o.id)
}

fn select_input(
    buttons: &ButtonInput<MouseButton>,
    keys: &ButtonInput<KeyCode>,
    cell: (i32, i32),
    ui: &mut UiState,
    net: &mut Net,
    gv: &GameView,
    room: dnd_world::RoomId,
) {
    let Some(map) = &gv.map else { return };
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if buttons.just_pressed(MouseButton::Left) {
        match objects_at(map, cell) {
            Some(id) => {
                if shift {
                    if ui.sel_objects.contains(&id) {
                        ui.sel_objects.retain(|x| *x != id);
                    } else {
                        ui.sel_objects.push(id);
                    }
                } else if !ui.sel_objects.contains(&id) {
                    ui.sel_objects = vec![id];
                }
                ui.drag = Some((cell, ui.sel_objects.contains(&id)));
            }
            None => {
                if !shift {
                    ui.sel_objects.clear();
                }
                ui.drag = Some((cell, false));
            }
        }
    }
    if buttons.just_released(MouseButton::Left)
        && let Some((start, moving)) = ui.drag.take()
    {
        if moving {
            let (dx, dy) = (cell.0 - start.0, cell.1 - start.1);
            if (dx, dy) != (0, 0) && !ui.sel_objects.is_empty() {
                net.cmd(Command::Dm(DmCommand::MoveObjects {
                    room,
                    objects: ui.sel_objects.clone(),
                    dx,
                    dy,
                }));
            }
        } else if start != cell {
            let (x0, x1) = (start.0.min(cell.0), start.0.max(cell.0));
            let (y0, y1) = (start.1.min(cell.1), start.1.max(cell.1));
            for o in &map.objects {
                if o.footprint()
                    .any(|(x, y)| x >= x0 && x <= x1 && y >= y0 && y <= y1)
                    && !ui.sel_objects.contains(&o.id)
                {
                    ui.sel_objects.push(o.id);
                }
            }
        }
    }
}

fn to_cells(p: Vec2) -> (f32, f32) {
    (p.x / CELL, -p.y / CELL)
}

fn draw_input(
    buttons: &ButtonInput<MouseButton>,
    hover: &Hover,
    ui: &mut UiState,
    net: &mut Net,
    room: dnd_world::RoomId,
) {
    use crate::state::DrawKind;
    use dnd_session::model::DrawShape;
    let p = to_cells(hover.world);
    if buttons.just_pressed(MouseButton::Left) {
        ui.draw_points = vec![p];
        if ui.draw_kind == DrawKind::Text {
            let text = ui.draw_text.trim().to_string();
            if !text.is_empty() {
                send_drawing(ui, net, room, DrawShape::Text(p, text));
            }
            ui.draw_points.clear();
        }
        return;
    }
    if ui.draw_points.is_empty() {
        return;
    }
    if buttons.pressed(MouseButton::Left) {
        if ui.draw_kind == DrawKind::Stroke {
            let last = *ui.draw_points.last().expect("non-empty");
            if (last.0 - p.0).hypot(last.1 - p.1) > 0.15 && ui.draw_points.len() < 2000 {
                ui.draw_points.push(p);
            }
        } else {
            ui.draw_points.truncate(1);
            ui.draw_points.push(p);
        }
    }
    if buttons.just_released(MouseButton::Left) {
        let pts = std::mem::take(&mut ui.draw_points);
        let a = pts[0];
        let shape = match ui.draw_kind {
            DrawKind::Stroke if pts.len() > 1 => DrawShape::Stroke(pts),
            DrawKind::Line => DrawShape::Line(a, p),
            DrawKind::Arrow => DrawShape::Arrow(a, p),
            DrawKind::Circle => DrawShape::Circle(a, (a.0 - p.0).hypot(a.1 - p.1).max(0.25)),
            _ => return,
        };
        send_drawing(ui, net, room, shape);
    }
}

fn send_drawing(
    ui: &UiState,
    net: &mut Net,
    room: dnd_world::RoomId,
    shape: dnd_session::model::DrawShape,
) {
    use dnd_session::model::{DrawVisibility, Drawing};
    let drawing = Drawing {
        id: 0,
        shape,
        color: ui.draw_color,
        width: ui.draw_width,
        visibility: if ui.draw_dm_only && net.is_dm() {
            DrawVisibility::DmOnly
        } else {
            DrawVisibility::All
        },
        temporary: ui.draw_temporary,
        author: String::new(),
    };
    net.cmd(Command::AddDrawing {
        room,
        drawing: Box::new(drawing),
    });
}

/// Editor keyboard shortcuts (DM only): Ctrl+C/V/D, Delete, R, Esc, Ctrl+Z/Y.
pub fn editor_keys(
    keys: Res<ButtonInput<KeyCode>>,
    egui: Res<EguiWantsInput>,
    hover: Res<Hover>,
    mut ui: ResMut<UiState>,
    mut net: ResMut<Net>,
    mut gv: ResMut<GameView>,
) {
    if !net.is_dm() || egui.wants_any_keyboard_input() {
        return;
    }
    let Some(room) = gv.view.as_ref().and_then(|v| v.room.as_ref()).map(|r| r.id) else {
        return;
    };
    let ctrl = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft);
    // Tool hotkeys (number row) and Ctrl+S.
    if !ctrl {
        for (key, tool) in [
            (KeyCode::Digit1, Tool::Select),
            (KeyCode::Digit2, Tool::Pencil),
            (KeyCode::Digit3, Tool::Eraser),
            (KeyCode::Digit4, Tool::Rect),
            (KeyCode::Digit5, Tool::Fill),
            (KeyCode::Digit6, Tool::Object),
            (KeyCode::Digit7, Tool::RemoveObject),
            (KeyCode::Digit8, Tool::Entry),
            (KeyCode::Digit9, Tool::Light),
            (KeyCode::Digit0, Tool::Draw),
        ] {
            if keys.just_pressed(key) {
                ui.tool = tool;
                return;
            }
        }
        if keys.just_pressed(KeyCode::Escape) && ui.sel_objects.is_empty() && ui.tool != Tool::Play
        {
            ui.tool = Tool::Play;
            return;
        }
    }
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        net.save(String::new());
        gv.toast("Saving…");
        return;
    }
    if ctrl && keys.just_pressed(KeyCode::KeyZ) {
        net.cmd(Command::Dm(DmCommand::Undo { room }));
        return;
    }
    if ctrl && keys.just_pressed(KeyCode::KeyY) {
        net.cmd(Command::Dm(DmCommand::Redo { room }));
        return;
    }
    if ui.tool != Tool::Select {
        return;
    }
    let Some(map) = gv.map.clone() else { return };
    let selected: Vec<_> = ui
        .sel_objects
        .iter()
        .filter_map(|id| map.object(*id).cloned())
        .collect();
    if ctrl && keys.just_pressed(KeyCode::KeyC) && !selected.is_empty() {
        let n = selected.len();
        ui.clipboard = selected;
        gv.toast(format!("Copied {n}"));
    } else if ctrl && keys.just_pressed(KeyCode::KeyV) && !ui.clipboard.is_empty() {
        let at = hover.cell.unwrap_or((1, 1));
        let min_x = ui.clipboard.iter().map(|o| o.x).min().unwrap_or(0);
        let min_y = ui.clipboard.iter().map(|o| o.y).min().unwrap_or(0);
        let objects = ui
            .clipboard
            .iter()
            .cloned()
            .map(|mut o| {
                o.x = o.x - min_x + at.0;
                o.y = o.y - min_y + at.1;
                o
            })
            .collect();
        net.cmd(Command::Dm(DmCommand::AddObjects { room, objects }));
    } else if ctrl && keys.just_pressed(KeyCode::KeyD) && !selected.is_empty() {
        let objects = selected
            .into_iter()
            .map(|mut o| {
                o.x += 1;
                o.y += 1;
                o
            })
            .collect();
        net.cmd(Command::Dm(DmCommand::AddObjects { room, objects }));
    } else if (keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace))
        && !selected.is_empty()
    {
        let objects = std::mem::take(&mut ui.sel_objects);
        net.cmd(Command::Dm(DmCommand::RemoveObjects { room, objects }));
    } else if keys.just_pressed(KeyCode::KeyR) && !ctrl {
        for mut o in selected {
            o.rotation = (o.rotation + 1) % 4;
            std::mem::swap(&mut o.w, &mut o.h);
            net.cmd(Command::Dm(DmCommand::UpdateObject {
                room,
                object: Box::new(o),
            }));
        }
    } else if keys.just_pressed(KeyCode::Escape) {
        ui.sel_objects.clear();
    }
}

fn shape_color(c: [u8; 4]) -> Color {
    Color::srgba_u8(c[0], c[1], c[2], c[3])
}

fn cells_to_world(p: (f32, f32)) -> Vec2 {
    Vec2::new(p.0 * CELL, -p.1 * CELL)
}

fn draw_shape(gizmos: &mut Gizmos, shape: &dnd_session::model::DrawShape, color: Color) {
    use dnd_session::model::DrawShape;
    match shape {
        DrawShape::Stroke(pts) => {
            gizmos.linestrip_2d(pts.iter().map(|p| cells_to_world(*p)), color);
        }
        DrawShape::Line(a, b) => gizmos.line_2d(cells_to_world(*a), cells_to_world(*b), color),
        DrawShape::Arrow(a, b) => {
            gizmos
                .arrow_2d(cells_to_world(*a), cells_to_world(*b), color)
                .with_tip_length(CELL * 0.4);
        }
        DrawShape::Circle(c, r) => {
            gizmos.circle_2d(cells_to_world(*c), r * CELL, color);
        }
        DrawShape::Text(..) => {}
    }
}

/// Text annotations need real text entities (gizmos can't draw text).
#[derive(Component)]
pub struct DrawingLabel(pub u64);

pub fn sync_drawing_labels(
    mut commands: Commands,
    gv: Res<GameView>,
    labels: Query<(Entity, &DrawingLabel)>,
) {
    let wanted: Vec<(u64, (f32, f32), String, [u8; 4])> = gv
        .view
        .as_ref()
        .and_then(|v| v.room.as_ref())
        .map(|r| {
            r.drawings
                .iter()
                .filter_map(|d| match &d.shape {
                    dnd_session::model::DrawShape::Text(p, t) => {
                        Some((d.id, *p, t.clone(), d.color))
                    }
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    for (e, l) in &labels {
        if !wanted.iter().any(|w| w.0 == l.0) {
            commands.entity(e).despawn();
        }
    }
    for (id, p, text, color) in wanted {
        if labels.iter().any(|(_, l)| l.0 == id) {
            continue;
        }
        let w = cells_to_world(p);
        commands.spawn((
            DrawingLabel(id),
            Text2d::new(text),
            TextFont {
                font_size: bevy::text::FontSize::Px(20.0),
                ..default()
            },
            TextColor(shape_color(color)),
            Transform::from_xyz(w.x, w.y, 12.0),
        ));
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
        let c = Color::srgba(0.09, 0.06, 0.04, 0.16);
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
    // Spell area / range preview while casting.
    if let Some(intent) = &ui.casting
        && let Some(caster) = view.creatures.iter().find(|c| c.id == intent.caster)
    {
        if intent.range_ft != u32::MAX && intent.range_ft > 0 && !intent.self_origin {
            gizmos.circle_2d(
                cell_to_world(caster.pos),
                (intent.range_ft as f32 / 5.0 + 0.5) * CELL,
                Color::srgba(0.6, 0.5, 1.0, 0.5),
            );
        }
        if let (Some(area), Some(cell)) = (intent.area, hover.cell) {
            let cells = dnd_session::spellcasting::area_cells(
                area.shape,
                area.size_ft,
                caster.pos,
                cell,
                !intent.self_origin,
            );
            for c in cells {
                gizmos.rect_2d(
                    cell_to_world(c),
                    Vec2::splat(CELL - 6.0),
                    Color::srgba(1.0, 0.5, 0.2, 0.8),
                );
            }
        }
        for t in &intent.targets {
            if let Some(tc) = view.creatures.iter().find(|c| c.id == *t) {
                gizmos.circle_2d(
                    cell_to_world(tc.pos),
                    CELL * 0.4,
                    Color::srgb(0.7, 0.5, 1.0),
                );
            }
        }
    }
    if let Some(c) = hover.cell {
        let color = match ui.tool {
            Tool::Play if ui.casting.is_some() || ui.feature_target.is_some() => {
                Color::srgb(0.7, 0.5, 1.0)
            }
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
    // Map annotations.
    if let Some(r) = &view.room {
        for d in &r.drawings {
            draw_shape(&mut gizmos, &d.shape, shape_color(d.color));
        }
    }
    // Drawing in progress.
    if ui.tool == Tool::Draw && !ui.draw_points.is_empty() {
        use crate::state::DrawKind;
        use dnd_session::model::DrawShape;
        let pts = ui.draw_points.clone();
        let a = pts[0];
        let b = *pts.last().expect("non-empty");
        let shape = match ui.draw_kind {
            DrawKind::Stroke => DrawShape::Stroke(pts),
            DrawKind::Line => DrawShape::Line(a, b),
            DrawKind::Arrow => DrawShape::Arrow(a, b),
            DrawKind::Circle => DrawShape::Circle(a, (a.0 - b.0).hypot(a.1 - b.1)),
            DrawKind::Text => DrawShape::Text(a, String::new()),
        };
        draw_shape(&mut gizmos, &shape, shape_color(ui.draw_color));
    }
    // Selected objects, drag ghost and box selection.
    if ui.tool == Tool::Select {
        let offset = match (ui.drag, hover.cell) {
            (Some((start, true)), Some(c)) => (c.0 - start.0, c.1 - start.1),
            _ => (0, 0),
        };
        for id in &ui.sel_objects {
            if let Some(o) = map.object(*id) {
                let a = cell_to_world((o.x + offset.0, o.y + offset.1));
                let b = cell_to_world((
                    o.x + offset.0 + o.w as i32 - 1,
                    o.y + offset.1 + o.h as i32 - 1,
                ));
                gizmos.rect_2d(
                    (a + b) / 2.0,
                    (b - a).abs() + Vec2::splat(CELL - 4.0),
                    Color::srgb(0.3, 0.8, 1.0),
                );
            }
        }
        if let (Some((start, false)), Some(c)) = (ui.drag, hover.cell) {
            let (x0, x1) = (start.0.min(c.0), start.0.max(c.0));
            let (y0, y1) = (start.1.min(c.1), start.1.max(c.1));
            let center = (cell_to_world((x0, y0)) + cell_to_world((x1, y1))) / 2.0;
            gizmos.rect_2d(
                center,
                Vec2::new((x1 - x0 + 1) as f32 * CELL, (y1 - y0 + 1) as f32 * CELL),
                Color::srgba(0.3, 0.8, 1.0, 0.7),
            );
        }
    }
    // Event trigger areas (DM).
    if net.is_dm()
        && let (Some(r), Some(dm)) = (&view.room, &view.dm)
    {
        for s in &dm.scripts {
            if let dnd_session::scripts::Trigger::EnterArea { room, a, b } = s.trigger
                && room == r.id
            {
                let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
                let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
                let center = (cell_to_world((x0, y0)) + cell_to_world((x1, y1))) / 2.0;
                gizmos.rect_2d(
                    center,
                    Vec2::new((x1 - x0 + 1) as f32 * CELL, (y1 - y0 + 1) as f32 * CELL),
                    Color::srgba(1.0, 0.3, 0.8, 0.6),
                );
            }
        }
        if let Some(draft) = &ui.script_draft
            && let dnd_session::scripts::Trigger::EnterArea { a, b, .. } = draft.trigger
        {
            let b = if ui.script_pick == Some(crate::state::ScriptPick::AreaEnd) {
                hover.cell.unwrap_or(b)
            } else {
                b
            };
            let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
            let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
            let center = (cell_to_world((x0, y0)) + cell_to_world((x1, y1))) / 2.0;
            gizmos.rect_2d(
                center,
                Vec2::new((x1 - x0 + 1) as f32 * CELL, (y1 - y0 + 1) as f32 * CELL),
                Color::srgb(1.0, 0.3, 0.8),
            );
        }
    }
    // Editor overlays: collision (movement/sight/difficult) and light radii.
    if net.is_dm() && ui.show_collision {
        for y in 0..map.height {
            for x in 0..map.width {
                let f = map.flags((x, y));
                let c = cell_to_world((x, y));
                let s = CELL * 0.42;
                if f.blocks_movement {
                    gizmos.line_2d(
                        c + Vec2::new(-s, -s),
                        c + Vec2::new(s, s),
                        Color::srgb(0.9, 0.2, 0.15),
                    );
                    gizmos.line_2d(
                        c + Vec2::new(-s, s),
                        c + Vec2::new(s, -s),
                        Color::srgb(0.9, 0.2, 0.15),
                    );
                }
                if f.blocks_sight {
                    gizmos.rect_2d(c, Vec2::splat(CELL * 0.7), Color::srgb(0.3, 0.5, 1.0));
                }
                if f.difficult {
                    gizmos.circle_2d(c, CELL * 0.12, Color::srgb(1.0, 0.85, 0.2));
                }
            }
        }
    }
    if net.is_dm()
        && ui.show_light_radii
        && let Some(r) = &view.room
    {
        for l in &r.lights {
            let c = cell_to_world(l.cell);
            gizmos.circle_2d(
                c,
                (l.bright_ft as f32 / 5.0 + 0.5) * CELL,
                Color::srgb(1.0, 0.85, 0.35),
            );
            gizmos.circle_2d(
                c,
                ((l.bright_ft + l.dim_ft) as f32 / 5.0 + 0.5) * CELL,
                Color::srgba(1.0, 0.7, 0.3, 0.5),
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
