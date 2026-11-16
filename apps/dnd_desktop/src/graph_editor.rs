//! Campaign graph editor (Dungeon Master only).
//!
//! Rooms are illustrated cards (map thumbnail, room type, status badges) connected by curves whose
//! style encodes the connection type without relying on color alone:
//! * normal passage — solid line;
//! * one-way — arrowheads along the curve;
//! * locked — lock emblem at the midpoint (and a double line);
//! * secret — dashed line with an eye emblem.
//!
//! Workflow: click a room to preview it, double-click to edit its map, drag from a card's ◆ port to
//! another card to connect them, click a connection to edit it, and move the party only through
//! the explicit confirmation dialog. Pan with the mouse, zoom with the wheel, Fit/Center/Arrange in
//! the toolbar, minimap and search for large campaigns, region groups that can be collapsed.
//!
//! This data never reaches players: room cards are part of the DM-only view.

use crate::icons::{self, Icon};
use crate::state::DmTab;
use crate::theme;
use crate::ui_game::GameUi;
use bevy::prelude::Resource;
use bevy_egui::egui::{self, Color32, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};
use dnd_session::command::{Command, DmCommand};
use dnd_session::view::{ClientView, RoomCard};
use dnd_world::{EdgeId, RoomId, WorldGraph};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

const NODE: Vec2 = vec2(210.0, 150.0);

#[derive(Resource)]
pub struct GraphEditor {
    pub zoom: f32,
    pub pan: Vec2,
    pub selected_node: Option<RoomId>,
    pub selected_edge: Option<EdgeId>,
    pub search: String,
    pub minimap: bool,
    pub groups: bool,
    pub collapsed: BTreeSet<String>,
    pub confirm_travel: Option<RoomId>,
    pub confirm_delete: Option<RoomId>,
    drag: Drag,
    thumbs: HashMap<RoomId, (u64, egui::TextureHandle)>,
    fit_pending: bool,
    canvas_half: Vec2,
    // edit buffers for the side panel
    edit_room: Option<(RoomId, String, String, String, String, bool)>,
    edit_edge: Option<(EdgeId, bool, bool, bool, String)>,
}

impl Default for GraphEditor {
    fn default() -> Self {
        GraphEditor {
            zoom: 0.8,
            pan: Vec2::ZERO,
            selected_node: None,
            selected_edge: None,
            search: String::new(),
            minimap: true,
            groups: true,
            collapsed: BTreeSet::new(),
            confirm_travel: None,
            confirm_delete: None,
            drag: Drag::None,
            thumbs: HashMap::new(),
            fit_pending: true,
            canvas_half: vec2(400.0, 300.0),
            edit_room: None,
            edit_edge: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    None,
    Pan,
    Node(RoomId, Vec2),
    Connect(RoomId),
}

struct Xf {
    origin: Pos2,
    pan: Vec2,
    zoom: f32,
}
impl Xf {
    fn to_screen(&self, p: Vec2) -> Pos2 {
        self.origin + (p + self.pan) * self.zoom
    }
    fn to_world(&self, s: Pos2) -> Vec2 {
        (s - self.origin) / self.zoom - self.pan
    }
}

fn region_color(name: &str) -> Color32 {
    let h = name
        .bytes()
        .fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let palette = [
        theme::FOREST,
        theme::SLATE,
        theme::BRONZE,
        theme::BURGUNDY,
        rgb(0x5A, 0x4A, 0x6E),
        rgb(0x4E, 0x5E, 0x3A),
    ];
    palette[(h % palette.len() as u32) as usize]
}
fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn kind_icon(kind: &str) -> Icon {
    match kind {
        "outdoor" => Icon::Tree,
        "cave" => Icon::Skull,
        "building" => Icon::Door,
        "water" => Icon::Potion,
        "dungeon" => Icon::Torch,
        _ => Icon::Scroll,
    }
}

/// Rooms reachable by the party right now (unlocked edges, respecting one-way direction).
fn reachable(graph: &WorldGraph, from: RoomId) -> BTreeSet<RoomId> {
    let mut seen = BTreeSet::from([from]);
    let mut q = VecDeque::from([from]);
    while let Some(r) = q.pop_front() {
        for e in graph.edges.iter().filter(|e| !e.locked) {
            let next = if e.from == r {
                Some(e.to)
            } else if e.to == r && !e.one_way {
                Some(e.from)
            } else {
                None
            };
            if let Some(n) = next
                && seen.insert(n)
            {
                q.push_back(n);
            }
        }
    }
    seen
}

/// Layered layout: breadth-first distance from the party room gives the column; two barycentre
/// sweeps order each column to reduce crossings; disconnected parts are stacked below.
pub fn auto_layout(graph: &WorldGraph, start: Option<RoomId>) -> BTreeMap<RoomId, (f32, f32)> {
    let mut adj: BTreeMap<RoomId, Vec<RoomId>> = graph.nodes.keys().map(|k| (*k, vec![])).collect();
    for e in &graph.edges {
        adj.entry(e.from).or_default().push(e.to);
        adj.entry(e.to).or_default().push(e.from);
    }
    let mut out = BTreeMap::new();
    let mut done: BTreeSet<RoomId> = BTreeSet::new();
    let mut y_offset = 0.0f32;
    let mut roots: Vec<RoomId> = start
        .into_iter()
        .filter(|s| graph.nodes.contains_key(s))
        .collect();
    roots.extend(graph.nodes.keys().copied());
    for root in roots {
        if done.contains(&root) {
            continue;
        }
        // BFS layers of this component.
        let mut layer: BTreeMap<RoomId, usize> = BTreeMap::from([(root, 0)]);
        let mut q = VecDeque::from([root]);
        while let Some(r) = q.pop_front() {
            for n in &adj[&r] {
                if !layer.contains_key(n) {
                    layer.insert(*n, layer[&r] + 1);
                    q.push_back(*n);
                }
            }
        }
        let depth = layer.values().max().copied().unwrap_or(0);
        let mut cols: Vec<Vec<RoomId>> = vec![vec![]; depth + 1];
        for (r, l) in &layer {
            cols[*l].push(*r);
        }
        for _ in 0..2 {
            for l in 1..cols.len() {
                let prev: HashMap<RoomId, usize> = cols[l - 1]
                    .iter()
                    .enumerate()
                    .map(|(i, r)| (*r, i))
                    .collect();
                cols[l].sort_by(|a, b| {
                    let bary = |x: &RoomId| {
                        let ps: Vec<f32> = adj[x]
                            .iter()
                            .filter_map(|n| prev.get(n).map(|i| *i as f32))
                            .collect();
                        if ps.is_empty() {
                            f32::MAX
                        } else {
                            ps.iter().sum::<f32>() / ps.len() as f32
                        }
                    };
                    bary(a)
                        .partial_cmp(&bary(b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(a.cmp(b))
                });
            }
        }
        let tallest = cols.iter().map(Vec::len).max().unwrap_or(1) as f32;
        for (l, col) in cols.iter().enumerate() {
            let col_h = col.len() as f32 * (NODE.y + 50.0);
            let top = y_offset + (tallest * (NODE.y + 50.0) - col_h) / 2.0;
            for (i, r) in col.iter().enumerate() {
                out.insert(
                    *r,
                    (
                        l as f32 * (NODE.x + 110.0),
                        top + i as f32 * (NODE.y + 50.0),
                    ),
                );
                done.insert(*r);
            }
        }
        y_offset += tallest * (NODE.y + 50.0) + 80.0;
    }
    out
}

fn cubic(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, t: f32) -> Pos2 {
    let u = 1.0 - t;
    let v = p0.to_vec2() * (u * u * u)
        + p1.to_vec2() * (3.0 * u * u * t)
        + p2.to_vec2() * (3.0 * u * t * t)
        + p3.to_vec2() * (t * t * t);
    v.to_pos2()
}

/// Curve between two node rectangles (leaving/entering on the facing sides).
fn edge_curve(a: Rect, b: Rect) -> [Pos2; 4] {
    let forward = b.center().x >= a.center().x;
    let (s, e) = if forward {
        (a.right_center(), b.left_center())
    } else {
        (a.left_center(), b.right_center())
    };
    let d = ((e.x - s.x).abs() * 0.5).max(60.0);
    let dir = if forward { 1.0 } else { -1.0 };
    [s, s + vec2(d * dir, 0.0), e - vec2(d * dir, 0.0), e]
}

fn arrow(p: &egui::Painter, at: Pos2, dir: Vec2, size: f32, color: Color32) {
    let d = dir.normalized();
    let n = vec2(-d.y, d.x);
    p.add(Shape::convex_polygon(
        vec![
            at + d * size,
            at - d * size * 0.6 + n * size * 0.7,
            at - d * size * 0.6 - n * size * 0.7,
        ],
        color,
        Stroke::new(1.0, theme::INK),
    ));
}

pub fn window(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    if !g.ui.show_graph {
        return;
    }
    let Some(dm) = &view.dm else { return };
    let mut open = true;
    theme::win(format!("🗺 {}", g.i18n.t("graph.title")))
        .id(egui::Id::new("graph-editor"))
        .open(&mut open)
        .default_size(vec2(1180.0, 700.0))
        .min_size(vec2(700.0, 420.0))
        .resizable(true)
        .show(ctx, |ui| {
            theme::window_bg(ui);
            toolbar(ui, g, view);
            ui.add_space(4.0);
            let avail = ui.available_size();
            ui.horizontal_top(|ui| {
                let canvas_size = vec2((avail.x - 270.0).max(300.0), avail.y.max(300.0));
                canvas(ui, g, view, &dm.room_cards, dm.party_room, canvas_size);
                ui.vertical(|ui| {
                    ui.set_width(256.0);
                    egui::ScrollArea::vertical()
                        .id_salt("graph-side")
                        .max_height(avail.y)
                        .show(ui, |ui| {
                            side_panel(ui, g, view, &dm.room_cards, dm.party_room)
                        });
                });
            });
        });
    if !open {
        g.ui.show_graph = false;
    }
    dialogs(ctx, g, view);
}

fn toolbar(ui: &mut egui::Ui, g: &mut GameUi, view: &ClientView) {
    ui.horizontal(|ui| {
        crate::icons::paint(
            ui.painter(),
            Rect::from_min_size(ui.cursor().min, vec2(22.0, 22.0)),
            Icon::Search,
        );
        ui.add_space(26.0);
        let resp = ui.add(
            egui::TextEdit::singleline(&mut g.graph.search)
                .hint_text(g.i18n.t("graph.search"))
                .desired_width(170.0),
        );
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let q = g.graph.search.to_lowercase();
            if let Some((id, _)) = view
                .graph
                .nodes
                .iter()
                .find(|(_, n)| n.name.to_lowercase().contains(&q))
            {
                g.graph.selected_node = Some(*id);
                center_on(g, view, *id);
            }
        }
        ui.separator();
        if theme::icon_button(ui, Icon::Fit, false, g.i18n.t("graph.fit")).clicked() {
            g.graph.fit_pending = true;
        }
        if theme::icon_button(ui, Icon::Target, false, g.i18n.t("graph.center_party")).clicked()
            && let Some(dm) = &view.dm
        {
            center_on(g, view, dm.party_room);
        }
        if theme::icon_button(ui, Icon::Layers, false, g.i18n.t("graph.arrange")).clicked() {
            let party = view.dm.as_ref().map(|d| d.party_room);
            for (room, pos) in auto_layout(&view.graph, party) {
                g.net.cmd(Command::Dm(DmCommand::SetRoomPos { room, pos }));
            }
            g.graph.fit_pending = true;
        }
        if theme::icon_button(ui, Icon::Plus, false, g.i18n.t("graph.new_room")).clicked() {
            g.net.cmd(Command::Dm(DmCommand::CreateRoom {
                name: g.i18n.t("graph.new_room_name").into(),
                width: 20,
                height: 15,
                ambient: dnd_world::map::AmbientLight::Dim,
            }));
        }
        let mm = g.graph.minimap;
        if theme::icon_button(ui, Icon::Map, mm, g.i18n.t("graph.minimap")).clicked() {
            g.graph.minimap = !mm;
        }
        let gr = g.graph.groups;
        if theme::icon_button(ui, Icon::Banner, gr, g.i18n.t("graph.groups")).clicked() {
            g.graph.groups = !gr;
        }
        ui.separator();
        ui.label(egui::RichText::new(format!("{:.0}%", g.graph.zoom * 100.0)).small());
        ui.separator();
        // Legend: line styles are distinguishable without color.
        legend(ui, g);
    });
}

fn legend(ui: &mut egui::Ui, g: &GameUi) {
    for (kind, key) in [
        (0, "graph.legend_normal"),
        (1, "graph.legend_oneway"),
        (2, "graph.legend_locked"),
        (3, "graph.legend_secret"),
    ] {
        let (r, _) = ui.allocate_exact_size(vec2(34.0, 18.0), egui::Sense::hover());
        let p = ui.painter();
        let (a, b) = (r.left_center(), r.right_center());
        match kind {
            0 => {
                p.line_segment([a, b], Stroke::new(2.5, theme::GOLD));
            }
            1 => {
                p.line_segment([a, b], Stroke::new(2.5, theme::GOLD));
                arrow(
                    p,
                    r.center() + vec2(4.0, 0.0),
                    vec2(1.0, 0.0),
                    6.0,
                    theme::GOLD_BRIGHT,
                );
            }
            2 => {
                p.line_segment(
                    [a + vec2(0.0, -2.0), b + vec2(0.0, -2.0)],
                    Stroke::new(1.5, theme::BURGUNDY),
                );
                p.line_segment(
                    [a + vec2(0.0, 2.0), b + vec2(0.0, 2.0)],
                    Stroke::new(1.5, theme::BURGUNDY),
                );
                icons::paint(
                    p,
                    Rect::from_center_size(r.center(), vec2(14.0, 14.0)),
                    Icon::Lock,
                );
            }
            _ => {
                for i in 0..4 {
                    let x = a.x + i as f32 * 9.0;
                    p.line_segment(
                        [pos2(x, a.y), pos2(x + 5.0, a.y)],
                        Stroke::new(2.0, rgb(0x8C, 0x70, 0xB8)),
                    );
                }
            }
        }
        ui.label(egui::RichText::new(g.i18n.t(key)).small());
    }
}

fn center_on(g: &mut GameUi, view: &ClientView, id: RoomId) {
    if let Some(n) = view.graph.nodes.get(&id) {
        g.graph.pan =
            -vec2(n.editor_pos.0, n.editor_pos.1) - NODE / 2.0 + g.graph.canvas_half / g.graph.zoom;
    }
}

#[allow(clippy::too_many_arguments)]
fn canvas(
    ui: &mut egui::Ui,
    g: &mut GameUi,
    view: &ClientView,
    cards: &[RoomCard],
    party: RoomId,
    size: Vec2,
) {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    g.graph.canvas_half = size / 2.0;
    let painter = ui.painter_at(rect);
    // Background: dark leather with a faint grid.
    painter.rect_filled(rect, 4.0, rgb(0x22, 0x1A, 0x15));
    let ge = &mut *g.graph;
    if ge.fit_pending && !view.graph.nodes.is_empty() {
        ge.fit_pending = false;
        let (mut lo, mut hi) = (vec2(f32::MAX, f32::MAX), vec2(f32::MIN, f32::MIN));
        for n in view.graph.nodes.values() {
            let p = vec2(n.editor_pos.0, n.editor_pos.1);
            lo = lo.min(p);
            hi = hi.max(p + NODE);
        }
        let span = (hi - lo) + vec2(120.0, 120.0);
        ge.zoom = (size.x / span.x).min(size.y / span.y).clamp(0.25, 1.4);
        ge.pan = -lo + vec2(60.0, 60.0) + (size / ge.zoom - span) / 2.0;
    }
    // Zoom around the cursor.
    if let Some(hover) = resp.hover_pos() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let xf = Xf {
                origin: rect.min,
                pan: ge.pan,
                zoom: ge.zoom,
            };
            let world = xf.to_world(hover);
            ge.zoom = (ge.zoom * (scroll * 0.0018).exp()).clamp(0.2, 2.5);
            ge.pan = (hover - rect.min) / ge.zoom - world;
        }
    }
    let xf = Xf {
        origin: rect.min,
        pan: ge.pan,
        zoom: ge.zoom,
    };
    let step = 40.0 * xf.zoom;
    if step > 8.0 {
        let off = vec2(
            (xf.pan.x * xf.zoom).rem_euclid(step),
            (xf.pan.y * xf.zoom).rem_euclid(step),
        );
        let mut x = rect.left() + off.x;
        while x < rect.right() {
            painter.vline(
                x,
                rect.y_range(),
                Stroke::new(0.5, Color32::from_white_alpha(10)),
            );
            x += step;
        }
        let mut y = rect.top() + off.y;
        while y < rect.bottom() {
            painter.hline(
                rect.x_range(),
                y,
                Stroke::new(0.5, Color32::from_white_alpha(10)),
            );
            y += step;
        }
    }

    let card_of: HashMap<RoomId, &RoomCard> = cards.iter().map(|c| (c.id, c)).collect();
    let reach = reachable(&view.graph, party);
    let viewing = view.room.as_ref().map(|r| r.id);
    let query = ge.search.to_lowercase();
    // Node world rects (collapsed regions become one group card).
    let mut node_rect: BTreeMap<RoomId, Rect> = BTreeMap::new();
    let mut regions: BTreeMap<String, Vec<RoomId>> = BTreeMap::new();
    for (id, n) in &view.graph.nodes {
        let p = vec2(n.editor_pos.0, n.editor_pos.1);
        let p = match ge.drag {
            Drag::Node(d, grab) if d == *id => resp
                .interact_pointer_pos()
                .map(|ptr| xf.to_world(ptr) - grab)
                .unwrap_or(p),
            _ => p,
        };
        node_rect.insert(*id, Rect::from_min_size(p.to_pos2(), NODE));
        if ge.groups && !n.region.trim().is_empty() {
            regions
                .entry(n.region.trim().to_string())
                .or_default()
                .push(*id);
        }
    }
    let mut group_rect: BTreeMap<String, Rect> = BTreeMap::new();
    for (name, members) in &regions {
        let mut r = Rect::NOTHING;
        for m in members {
            r = r.union(node_rect[m]);
        }
        group_rect.insert(name.clone(), r.expand(26.0));
    }
    let collapsed_of = |id: &RoomId| -> Option<String> {
        let n = view.graph.nodes.get(id)?;
        let r = n.region.trim();
        (ge.groups && !r.is_empty() && ge.collapsed.contains(r)).then(|| r.to_string())
    };
    let display_rect = |id: &RoomId| -> Rect {
        match collapsed_of(id) {
            Some(r) => Rect::from_center_size(group_rect[&r].center(), vec2(NODE.x, 90.0)),
            None => node_rect[id],
        }
    };
    let sr =
        |r: Rect| Rect::from_min_max(xf.to_screen(r.min.to_vec2()), xf.to_screen(r.max.to_vec2()));

    // Region frames.
    let mut group_toggle = None;
    for (name, r) in &group_rect {
        let s = sr(*r);
        let c = region_color(name);
        if ge.collapsed.contains(name) {
            continue;
        }
        painter.rect_filled(s, 10.0 * xf.zoom, c.gamma_multiply(0.18));
        painter.rect_stroke(
            s,
            10.0 * xf.zoom,
            Stroke::new(1.5, c.gamma_multiply(0.8)),
            egui::StrokeKind::Inside,
        );
        let head = Rect::from_min_size(s.min, vec2(s.width(), (22.0 * xf.zoom).max(14.0)));
        painter.text(
            head.left_center() + vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            format!("⏷ {name}"),
            egui::FontId::new((15.0 * xf.zoom).max(10.0), theme::gothic_family()),
            theme::PARCHMENT,
        );
        if resp.clicked()
            && resp
                .interact_pointer_pos()
                .is_some_and(|p| head.contains(p))
        {
            group_toggle = Some(name.clone());
        }
    }

    // Hover detection.
    let pointer = resp.hover_pos();
    let mut hover_node = None;
    let mut hover_port = None;
    for id in view.graph.nodes.keys() {
        if collapsed_of(id).is_some() {
            continue;
        }
        let s = sr(display_rect(id));
        if let Some(p) = pointer {
            let port = s.right_center() + vec2(2.0, 0.0);
            if p.distance(port) < 11.0 {
                hover_port = Some(*id);
            }
            if s.contains(p) {
                hover_node = Some(*id);
            }
        }
    }
    let mut hover_edge = None;
    if hover_node.is_none()
        && hover_port.is_none()
        && let Some(p) = pointer
    {
        for e in &view.graph.edges {
            if !node_rect.contains_key(&e.from) || !node_rect.contains_key(&e.to) {
                continue;
            }
            let c = edge_curve(sr(display_rect(&e.from)), sr(display_rect(&e.to)));
            if (0..=24).any(|i| cubic(c[0], c[1], c[2], c[3], i as f32 / 24.0).distance(p) < 8.0) {
                hover_edge = Some(e.id);
            }
        }
    }

    // Edges.
    for e in &view.graph.edges {
        if !node_rect.contains_key(&e.from) || !node_rect.contains_key(&e.to) {
            continue;
        }
        if collapsed_of(&e.from).is_some() && collapsed_of(&e.from) == collapsed_of(&e.to) {
            continue;
        }
        let c = edge_curve(sr(display_rect(&e.from)), sr(display_rect(&e.to)));
        let selected = ge.selected_edge == Some(e.id);
        let hovered = hover_edge == Some(e.id);
        let base = if e.secret {
            rgb(0x8C, 0x70, 0xB8)
        } else if e.locked {
            rgb(0xA8, 0x44, 0x3E)
        } else {
            theme::GOLD
        };
        let color = if selected {
            theme::IVORY
        } else if hovered {
            theme::GOLD_BRIGHT
        } else {
            base
        };
        let w = (if selected || hovered { 4.0 } else { 2.6 }) * xf.zoom.clamp(0.6, 1.4);
        let pts: Vec<Pos2> = (0..=40)
            .map(|i| cubic(c[0], c[1], c[2], c[3], i as f32 / 40.0))
            .collect();
        // Dark under-stroke for contrast on any background.
        painter.add(Shape::line(
            pts.clone(),
            Stroke::new(w + 3.0, Color32::from_black_alpha(140)),
        ));
        if e.secret {
            for seg in pts.chunks(2).step_by(2) {
                if seg.len() == 2 {
                    painter.line_segment([seg[0], seg[1]], Stroke::new(w, color));
                }
            }
        } else if e.locked {
            let n: Vec<Pos2> = pts.windows(2).map(|s| s[0]).collect();
            let off = |sign: f32| -> Vec<Pos2> {
                pts.windows(2)
                    .map(|s| {
                        let d = (s[1] - s[0]).normalized();
                        s[0] + vec2(-d.y, d.x) * sign * w * 0.7
                    })
                    .collect()
            };
            let _ = n;
            painter.add(Shape::line(off(1.0), Stroke::new(w * 0.6, color)));
            painter.add(Shape::line(off(-1.0), Stroke::new(w * 0.6, color)));
        } else {
            painter.add(Shape::line(pts.clone(), Stroke::new(w, color)));
        }
        if e.one_way {
            for t in [0.35, 0.65] {
                let at = cubic(c[0], c[1], c[2], c[3], t);
                let dir = cubic(c[0], c[1], c[2], c[3], t + 0.02) - at;
                arrow(
                    &painter,
                    at,
                    dir,
                    9.0 * xf.zoom.clamp(0.7, 1.3),
                    theme::GOLD_BRIGHT,
                );
            }
        }
        let mid = cubic(c[0], c[1], c[2], c[3], 0.5);
        let badge = 18.0 * xf.zoom.clamp(0.7, 1.3);
        if e.locked {
            painter.circle_filled(mid, badge * 0.7, theme::WOOD);
            icons::paint(
                &painter,
                Rect::from_center_size(mid, vec2(badge, badge)),
                Icon::Lock,
            );
        } else if e.secret {
            painter.circle_filled(mid, badge * 0.7, theme::WOOD);
            icons::paint(
                &painter,
                Rect::from_center_size(mid, vec2(badge, badge)),
                Icon::Eye,
            );
        }
        if !e.label.is_empty() && xf.zoom > 0.5 {
            painter.text(
                mid + vec2(0.0, -badge),
                egui::Align2::CENTER_BOTTOM,
                &e.label,
                egui::FontId::new(12.0, theme::italic_family()),
                theme::PARCHMENT,
            );
        }
    }
    // Connection being dragged.
    if let (Drag::Connect(from), Some(p)) = (ge.drag, pointer.or(resp.interact_pointer_pos())) {
        let a = sr(display_rect(&from));
        let c = edge_curve(a, Rect::from_center_size(p, vec2(2.0, 2.0)));
        let pts: Vec<Pos2> = (0..=30)
            .map(|i| cubic(c[0], c[1], c[2], c[3], i as f32 / 30.0))
            .collect();
        painter.add(Shape::line(pts, Stroke::new(3.0, theme::GOLD_BRIGHT)));
    }

    // Nodes.
    let font = |s: f32| egui::FontId::new((s * xf.zoom).max(8.0), theme::gothic_family());
    let small = |s: f32| egui::FontId::new((s * xf.zoom).max(7.0), egui::FontFamily::Proportional);
    let mut drawn_groups = BTreeSet::new();
    for (id, n) in &view.graph.nodes {
        if let Some(region) = collapsed_of(id) {
            if drawn_groups.insert(region.clone()) {
                let s = sr(display_rect(id));
                let c = region_color(&region);
                painter.rect_filled(
                    s.translate(vec2(4.0, 6.0)),
                    8.0,
                    Color32::from_black_alpha(120),
                );
                painter.rect_filled(s, 8.0, c.gamma_multiply(0.85));
                painter.rect_stroke(
                    s,
                    8.0,
                    Stroke::new(2.0, theme::GOLD),
                    egui::StrokeKind::Inside,
                );
                let count = regions.get(&region).map(Vec::len).unwrap_or(0);
                painter.text(
                    s.center() - vec2(0.0, 10.0 * xf.zoom),
                    egui::Align2::CENTER_CENTER,
                    format!("▶ {region}"),
                    font(18.0),
                    theme::IVORY,
                );
                painter.text(
                    s.center() + vec2(0.0, 16.0 * xf.zoom),
                    egui::Align2::CENTER_CENTER,
                    format!("{count} {}", g.i18n.t("graph.rooms")),
                    small(13.0),
                    theme::PARCHMENT,
                );
                if resp.clicked() && resp.interact_pointer_pos().is_some_and(|p| s.contains(p)) {
                    group_toggle = Some(region.clone());
                }
            }
            continue;
        }
        let s = sr(display_rect(id));
        if !s.intersects(rect) {
            continue;
        }
        let card = card_of.get(id);
        let is_party = *id == party;
        let selected = ge.selected_node == Some(*id);
        let matches = !query.is_empty() && n.name.to_lowercase().contains(&query);
        let dim = (!query.is_empty() && !matches) || card.is_some_and(|c| !c.visited && !is_party);
        // Shadow, glow and frame.
        painter.rect_filled(
            s.translate(vec2(4.0, 7.0) * xf.zoom),
            6.0,
            Color32::from_black_alpha(130),
        );
        if is_party || selected || matches {
            let glow = if is_party {
                theme::BURGUNDY
            } else {
                theme::GOLD_BRIGHT
            };
            for k in 1..4 {
                painter.rect_stroke(
                    s.expand(k as f32 * 2.5),
                    8.0,
                    Stroke::new(2.0, glow.gamma_multiply(0.35 / k as f32)),
                    egui::StrokeKind::Outside,
                );
            }
        }
        let m = theme::Materials::get(ui.ctx());
        theme::paint_material(
            &painter.with_clip_rect(s.intersect(rect)),
            s,
            &m.parchment,
            Color32::WHITE,
        );
        let head = Rect::from_min_size(s.min, vec2(s.width(), 28.0 * xf.zoom));
        theme::paint_material(
            &painter.with_clip_rect(head.intersect(rect)),
            head,
            &m.wood,
            Color32::WHITE,
        );
        let border = if is_party {
            theme::BURGUNDY
        } else if selected {
            theme::GOLD_BRIGHT
        } else if Some(*id) == viewing {
            theme::GOLD
        } else {
            theme::BRONZE
        };
        if n.secret {
            // Dashed border marks secret rooms (shape, not only color).
            let corners = [
                s.left_top(),
                s.right_top(),
                s.right_bottom(),
                s.left_bottom(),
                s.left_top(),
            ];
            for w in corners.windows(2) {
                let len = w[0].distance(w[1]);
                let dir = (w[1] - w[0]) / len;
                let mut t = 0.0;
                while t < len {
                    painter.line_segment(
                        [w[0] + dir * t, w[0] + dir * (t + 7.0).min(len)],
                        Stroke::new(2.5, rgb(0x8C, 0x70, 0xB8)),
                    );
                    t += 12.0;
                }
            }
        } else {
            painter.rect_stroke(
                s,
                6.0,
                Stroke::new(if is_party || selected { 3.0 } else { 1.8 }, border),
                egui::StrokeKind::Inside,
            );
        }
        let icon_r = Rect::from_center_size(
            head.left_center() + vec2(16.0 * xf.zoom, 0.0),
            vec2(22.0, 22.0) * xf.zoom,
        );
        icons::paint(
            &painter,
            icon_r,
            kind_icon(card.map(|c| c.kind.as_str()).unwrap_or("")),
        );
        let name = if n.name.chars().count() > 22 {
            format!("{}…", n.name.chars().take(21).collect::<String>())
        } else {
            n.name.clone()
        };
        painter.text(
            head.left_center() + vec2(32.0 * xf.zoom, 0.0),
            egui::Align2::LEFT_CENTER,
            name,
            font(16.0),
            theme::IVORY,
        );
        // Thumbnail.
        let thumb_r = Rect::from_min_max(
            head.left_bottom() + vec2(8.0, 6.0) * xf.zoom,
            s.right_bottom() - vec2(8.0, 30.0) * xf.zoom,
        );
        if let Some(c) = card {
            let tex = thumbnail(&mut ge.thumbs, ui.ctx(), c);
            painter.image(
                tex.id(),
                thumb_r,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            painter.rect_filled(thumb_r, 2.0, theme::PARCHMENT_DARK);
        }
        painter.rect_stroke(
            thumb_r,
            2.0,
            Stroke::new(1.2, theme::INK),
            egui::StrokeKind::Outside,
        );
        // Footer badges.
        let foot = Rect::from_min_max(pos2(s.left(), thumb_r.bottom() + 2.0), s.right_bottom());
        let mut x = foot.left() + 8.0 * xf.zoom;
        let bs = 18.0 * xf.zoom;
        let mut badge = |icon: Icon, text: String, color: Color32| {
            icons::paint(
                &painter,
                Rect::from_min_size(pos2(x, foot.center().y - bs / 2.0), vec2(bs, bs)),
                icon,
            );
            x += bs + 2.0;
            if !text.is_empty() {
                let r = painter.text(
                    pos2(x, foot.center().y),
                    egui::Align2::LEFT_CENTER,
                    text,
                    small(12.5),
                    color,
                );
                x = r.right() + 8.0 * xf.zoom;
            } else {
                x += 4.0;
            }
        };
        if is_party {
            badge(
                Icon::Banner,
                g.i18n.t("graph.party_here").into(),
                theme::BURGUNDY,
            );
        } else if card.is_some_and(|c| c.visited) {
            badge(Icon::Flag, g.i18n.t("graph.visited").into(), theme::FOREST);
        } else {
            badge(
                Icon::Hourglass,
                g.i18n.t("graph.unexplored").into(),
                theme::INK_SOFT,
            );
        }
        if let Some(c) = card
            && c.monsters > 0
        {
            badge(Icon::Skull, c.monsters.to_string(), theme::INK);
        }
        if !reach.contains(id) {
            badge(Icon::Lock, String::new(), theme::INK);
        }
        if dim {
            painter.rect_filled(s, 6.0, Color32::from_black_alpha(70));
        }
        // Connection port (drag from here to another room).
        let port = s.right_center() + vec2(2.0, 0.0);
        let pr = if hover_port == Some(*id) { 9.0 } else { 6.5 };
        painter.add(Shape::convex_polygon(
            vec![
                port + vec2(0.0, -pr),
                port + vec2(pr, 0.0),
                port + vec2(0.0, pr),
                port + vec2(-pr, 0.0),
            ],
            theme::GOLD_BRIGHT,
            Stroke::new(1.2, theme::INK),
        ));
    }

    // Minimap.
    if ge.minimap && !view.graph.nodes.is_empty() {
        let mm = Rect::from_min_size(rect.right_bottom() - vec2(196.0, 126.0), vec2(186.0, 116.0));
        let mut wb = Rect::NOTHING;
        for r in node_rect.values() {
            wb = wb.union(*r);
        }
        let wb = wb.expand(80.0);
        let k = (mm.width() / wb.width()).min(mm.height() / wb.height());
        let to_mm = |p: Pos2| mm.min + (p - wb.min) * k;
        painter.rect_filled(mm, 4.0, Color32::from_black_alpha(170));
        painter.rect_stroke(
            mm,
            4.0,
            Stroke::new(1.0, theme::BRONZE),
            egui::StrokeKind::Inside,
        );
        for e in &view.graph.edges {
            if let (Some(a), Some(b)) = (node_rect.get(&e.from), node_rect.get(&e.to)) {
                painter.line_segment(
                    [to_mm(a.center()), to_mm(b.center())],
                    Stroke::new(1.0, theme::GOLD.gamma_multiply(0.6)),
                );
            }
        }
        for (id, r) in &node_rect {
            let c = if *id == party {
                theme::BURGUNDY
            } else {
                theme::PARCHMENT_DARK
            };
            painter.rect_filled(Rect::from_min_max(to_mm(r.min), to_mm(r.max)), 1.0, c);
        }
        let vis = Rect::from_min_max(
            xf.to_world(rect.min).to_pos2(),
            xf.to_world(rect.max).to_pos2(),
        );
        painter.rect_stroke(
            Rect::from_min_max(to_mm(vis.min), to_mm(vis.max)).intersect(mm),
            0.0,
            Stroke::new(1.5, theme::IVORY),
            egui::StrokeKind::Inside,
        );
        if resp.clicked()
            && let Some(p) = resp.interact_pointer_pos()
            && mm.contains(p)
        {
            let world = wb.min + (p - mm.min) / k;
            ge.pan = -world.to_vec2() + size / 2.0 / ge.zoom;
            return;
        }
    }

    // Interaction.
    if resp.drag_started() {
        let start = resp.interact_pointer_pos();
        ge.drag = match (hover_port, hover_node, start) {
            (Some(p), _, _) => Drag::Connect(p),
            (None, Some(n), Some(ptr)) => {
                let world = xf.to_world(ptr);
                Drag::Node(n, world - node_rect[&n].min.to_vec2())
            }
            _ => Drag::Pan,
        };
    }
    if resp.dragged() && ge.drag == Drag::Pan {
        ge.pan += resp.drag_delta() / ge.zoom;
    }
    if resp.drag_stopped() {
        match ge.drag {
            Drag::Node(id, _) => {
                let r = node_rect[&id];
                g.net.cmd(Command::Dm(DmCommand::SetRoomPos {
                    room: id,
                    pos: (r.min.x, r.min.y),
                }));
            }
            Drag::Connect(from) => {
                if let Some(to) = hover_node.filter(|t| *t != from) {
                    g.net.cmd(Command::Dm(DmCommand::ConnectRooms {
                        from,
                        to,
                        one_way: false,
                    }));
                }
            }
            _ => {}
        }
        g.graph.drag = Drag::None;
        return;
    }
    let ge = &mut *g.graph;
    if let Some(name) = group_toggle {
        if !ge.collapsed.remove(&name) {
            ge.collapsed.insert(name);
        }
        return;
    }
    if resp.double_clicked() {
        if let Some(id) = hover_node {
            g.net.cmd(Command::Dm(DmCommand::ViewRoom { room: id }));
            g.ui.dm_tab = DmTab::Editor;
            g.ui.show_graph = false;
        }
    } else if resp.clicked() {
        ge.selected_node = hover_node;
        ge.selected_edge = if hover_node.is_none() {
            hover_edge
        } else {
            None
        };
    }
    if resp.secondary_clicked() {
        ge.selected_node = hover_node;
        ge.selected_edge = None;
    }
    let menu_node = ge.selected_node;
    resp.context_menu(|ui| {
        let Some(id) = menu_node else {
            ui.label(g.i18n.t("graph.help_short"));
            return;
        };
        if ui.button(g.i18n.t("graph.edit_map")).clicked() {
            g.net.cmd(Command::Dm(DmCommand::ViewRoom { room: id }));
            g.ui.dm_tab = DmTab::Editor;
            ui.close();
        }
        if id != party && ui.button(g.i18n.t("graph.move_party")).clicked() {
            g.graph.confirm_travel = Some(id);
            ui.close();
        }
        if id != party
            && ui
                .button(egui::RichText::new(g.i18n.t("graph.delete_room")).color(theme::BURGUNDY))
                .clicked()
        {
            g.graph.confirm_delete = Some(id);
            ui.close();
        }
    });
}

/// Map thumbnail texture for a room card (shared with the editor minimap).
pub fn thumbnail_tex(
    ge: &mut GraphEditor,
    ctx: &egui::Context,
    c: &RoomCard,
) -> egui::TextureHandle {
    thumbnail(&mut ge.thumbs, ctx, c).clone()
}

fn thumbnail<'a>(
    cache: &'a mut HashMap<RoomId, (u64, egui::TextureHandle)>,
    ctx: &egui::Context,
    c: &RoomCard,
) -> &'a egui::TextureHandle {
    let hash = c
        .thumb
        .iter()
        .fold(c.thumb_w as u64 * 31 + c.thumb_h as u64, |a, b| {
            a.wrapping_mul(1_099_511_628_211).wrapping_add(*b as u64)
        });
    let stale = cache.get(&c.id).is_none_or(|(h, _)| *h != hash);
    if stale {
        let (w, h) = (c.thumb_w.max(1) as usize, c.thumb_h.max(1) as usize);
        let px: Vec<Color32> = c
            .thumb
            .chunks(3)
            .map(|p| Color32::from_rgb(p[0], p[1], p.get(2).copied().unwrap_or(0)))
            .collect();
        let img = egui::ColorImage::new(
            [w, h],
            if px.len() == w * h {
                px
            } else {
                vec![theme::PARCHMENT_DARK; w * h]
            },
        );
        let tex = ctx.load_texture(
            format!("room-thumb-{}", c.id.0),
            img,
            egui::TextureOptions::NEAREST,
        );
        cache.insert(c.id, (hash, tex));
    }
    &cache[&c.id].1
}

fn side_panel(
    ui: &mut egui::Ui,
    g: &mut GameUi,
    view: &ClientView,
    cards: &[RoomCard],
    party: RoomId,
) {
    if let Some(id) = g
        .graph
        .selected_node
        .filter(|id| view.graph.nodes.contains_key(id))
    {
        let n = &view.graph.nodes[&id];
        let buf = g.graph.edit_room.get_or_insert_with(|| {
            (
                id,
                n.name.clone(),
                n.description.clone(),
                n.region.clone(),
                n.chapter.clone(),
                n.secret,
            )
        });
        if buf.0 != id {
            *buf = (
                id,
                n.name.clone(),
                n.description.clone(),
                n.region.clone(),
                n.chapter.clone(),
                n.secret,
            );
        }
        theme::heading(ui, g.i18n.t("graph.room"));
        if let Some(c) = cards.iter().find(|c| c.id == id) {
            let tex = thumbnail(&mut g.graph.thumbs, ui.ctx(), c).clone();
            let w = ui.available_width();
            let h = w * c.thumb_h as f32 / c.thumb_w.max(1) as f32;
            ui.image((tex.id(), vec2(w, h.min(170.0))));
            ui.label(
                egui::RichText::new(format!(
                    "{}×{} · {} {} · {}",
                    c.size.0,
                    c.size.1,
                    c.monsters,
                    g.i18n.t("graph.monsters"),
                    if c.visited {
                        g.i18n.t("graph.visited")
                    } else {
                        g.i18n.t("graph.unexplored")
                    }
                ))
                .small(),
            );
        }
        let buf = g.graph.edit_room.as_mut().expect("set above");
        ui.add(egui::TextEdit::singleline(&mut buf.1).hint_text(g.i18n.t("graph.name")));
        ui.add(
            egui::TextEdit::multiline(&mut buf.2)
                .desired_rows(2)
                .hint_text(g.i18n.t("world.description")),
        );
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut buf.3)
                    .hint_text(g.i18n.t("graph.region"))
                    .desired_width(110.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut buf.4)
                    .hint_text(g.i18n.t("graph.chapter"))
                    .desired_width(110.0),
            );
        });
        ui.checkbox(&mut buf.5, g.i18n.t("graph.secret_room"));
        let b = buf.clone();
        if ui.button(g.i18n.t("world.apply")).clicked() {
            g.net.cmd(Command::Dm(DmCommand::RenameRoom {
                room: id,
                name: b.1.clone(),
                description: b.2.clone(),
            }));
            g.net.cmd(Command::Dm(DmCommand::UpdateRoomMeta {
                room: id,
                region: b.3.clone(),
                chapter: b.4.clone(),
                secret: b.5,
            }));
        }
        ui.separator();
        if theme::plaque_button(
            ui,
            Icon::Brush,
            g.i18n.t("graph.edit_map"),
            ui.available_width(),
        )
        .clicked()
        {
            g.net.cmd(Command::Dm(DmCommand::ViewRoom { room: id }));
            g.ui.dm_tab = DmTab::Editor;
            g.ui.show_graph = false;
        }
        if id != party
            && theme::plaque_button(
                ui,
                Icon::Banner,
                g.i18n.t("graph.move_party"),
                ui.available_width(),
            )
            .clicked()
        {
            g.graph.confirm_travel = Some(id);
        }
        // Connections of this room.
        theme::heading(ui, g.i18n.t("world.connections"));
        for e in view
            .graph
            .edges
            .iter()
            .filter(|e| e.from == id || e.to == id)
        {
            let other = if e.from == id { e.to } else { e.from };
            let name = view
                .graph
                .nodes
                .get(&other)
                .map(|n| n.name.clone())
                .unwrap_or_default();
            let arrow = if e.one_way {
                if e.from == id { "»" } else { "«" }
            } else {
                "« »"
            };
            if ui
                .selectable_label(
                    g.graph.selected_edge == Some(e.id),
                    format!("{arrow} {name}"),
                )
                .clicked()
            {
                g.graph.selected_edge = Some(e.id);
                g.graph.selected_node = None;
            }
        }
        if id != party
            && ui
                .button(egui::RichText::new(g.i18n.t("graph.delete_room")).color(theme::BURGUNDY))
                .clicked()
        {
            g.graph.confirm_delete = Some(id);
        }
    } else if let Some(eid) = g.graph.selected_edge {
        let Some(e) = view.graph.edges.iter().find(|e| e.id == eid) else {
            g.graph.selected_edge = None;
            return;
        };
        let buf = g
            .graph
            .edit_edge
            .get_or_insert_with(|| (eid, e.one_way, e.locked, e.secret, e.label.clone()));
        if buf.0 != eid {
            *buf = (eid, e.one_way, e.locked, e.secret, e.label.clone());
        }
        theme::heading(ui, g.i18n.t("graph.connection"));
        let name = |id: RoomId| {
            view.graph
                .nodes
                .get(&id)
                .map(|n| n.name.clone())
                .unwrap_or_default()
        };
        ui.label(theme::bold(format!(
            "{} {} {}",
            name(e.from),
            if buf.1 { "»" } else { "« »" },
            name(e.to)
        )));
        ui.checkbox(&mut buf.1, g.i18n.t("graph.legend_oneway"));
        ui.checkbox(&mut buf.2, g.i18n.t("graph.legend_locked"));
        ui.checkbox(&mut buf.3, g.i18n.t("graph.legend_secret"));
        ui.add(egui::TextEdit::singleline(&mut buf.4).hint_text(g.i18n.t("graph.label")));
        let b = buf.clone();
        if b.1 != e.one_way || b.2 != e.locked || b.3 != e.secret || b.4 != e.label {
            g.net.cmd(Command::Dm(DmCommand::EditEdge {
                edge: eid,
                one_way: b.1,
                locked: b.2,
                secret: b.3,
                label: b.4,
            }));
        }
        ui.label(egui::RichText::new(g.i18n.t("graph.edge_help")).small());
        if ui
            .button(egui::RichText::new(g.i18n.t("graph.delete_edge")).color(theme::BURGUNDY))
            .clicked()
        {
            g.net.cmd(Command::Dm(DmCommand::DeleteEdge { edge: eid }));
            g.graph.selected_edge = None;
        }
    } else {
        theme::heading(ui, g.i18n.t("graph.how_title"));
        for key in [
            "graph.how_1",
            "graph.how_2",
            "graph.how_3",
            "graph.how_4",
            "graph.how_5",
        ] {
            ui.label(egui::RichText::new(g.i18n.t(key)).small());
        }
    }
}

fn dialogs(ctx: &egui::Context, g: &mut GameUi, view: &ClientView) {
    let party = view.dm.as_ref().map(|d| d.party_room);
    if let Some(to) = g.graph.confirm_travel {
        let name = view
            .graph
            .nodes
            .get(&to)
            .map(|n| n.name.clone())
            .unwrap_or_default();
        let from_name = party
            .and_then(|p| view.graph.nodes.get(&p))
            .map(|n| n.name.clone())
            .unwrap_or_default();
        let route = party.and_then(|p| {
            view.graph
                .edges
                .iter()
                .find(|e| (e.from == p && e.to == to) || (!e.one_way && e.from == to && e.to == p))
        });
        let mut close = false;
        theme::win(g.i18n.t("graph.confirm_travel"))
            .id(egui::Id::new("confirm-travel"))
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                theme::window_bg(ui);
                ui.label(theme::gothic(format!("{from_name}  »  {name}"), 22.0).color(theme::WOOD));
                match route {
                    Some(e) if e.locked => {
                        ui.label(
                            theme::italic(g.i18n.t("graph.route_locked")).color(theme::BURGUNDY),
                        );
                    }
                    Some(_) => {
                        ui.label(theme::italic(g.i18n.t("graph.route_ok")));
                    }
                    None => {
                        ui.label(
                            theme::italic(g.i18n.t("graph.route_none")).color(theme::BURGUNDY),
                        );
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let legal = route.is_some_and(|e| !e.locked);
                    if legal
                        && ui
                            .button(theme::bold(g.i18n.t("graph.travel_now")))
                            .clicked()
                    {
                        g.net.cmd(Command::Dm(DmCommand::TransitionParty {
                            to,
                            override_rules: false,
                        }));
                        close = true;
                    }
                    if !legal
                        && ui
                            .button(theme::bold(g.i18n.t("world.travel_override")))
                            .clicked()
                    {
                        g.net.cmd(Command::Dm(DmCommand::TransitionParty {
                            to,
                            override_rules: true,
                        }));
                        close = true;
                    }
                    if ui.button(g.i18n.t("cast.cancel")).clicked() {
                        close = true;
                    }
                });
            });
        if close {
            g.graph.confirm_travel = None;
        }
    }
    if let Some(id) = g.graph.confirm_delete {
        let name = view
            .graph
            .nodes
            .get(&id)
            .map(|n| n.name.clone())
            .unwrap_or_default();
        let mut close = false;
        theme::win(g.i18n.t("graph.delete_room"))
            .id(egui::Id::new("confirm-delete-room"))
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                theme::window_bg(ui);
                ui.label(format!("{} «{name}»?", g.i18n.t("graph.delete_confirm")));
                ui.horizontal(|ui| {
                    if ui
                        .button(
                            egui::RichText::new(g.i18n.t("graph.delete_room"))
                                .color(theme::BURGUNDY),
                        )
                        .clicked()
                    {
                        g.net.cmd(Command::Dm(DmCommand::DeleteRoom { room: id }));
                        g.graph.selected_node = None;
                        close = true;
                    }
                    if ui.button(g.i18n.t("cast.cancel")).clicked() {
                        close = true;
                    }
                });
            });
        if close {
            g.graph.confirm_delete = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_layout_puts_rooms_in_distance_columns_without_overlap() {
        let mut g = WorldGraph::default();
        let ids: Vec<RoomId> = ["Gate", "Hall", "Crypt", "Tower", "Lost Cave"]
            .iter()
            .map(|n| g.add_room(*n))
            .collect();
        g.connect(ids[0], ids[1], false);
        g.connect(ids[1], ids[2], false);
        g.connect(ids[1], ids[3], true);
        // ids[4] is disconnected.
        let pos = auto_layout(&g, Some(ids[0]));
        assert_eq!(pos.len(), 5);
        assert_eq!(pos[&ids[0]].0, 0.0);
        assert!(pos[&ids[1]].0 > pos[&ids[0]].0);
        assert_eq!(pos[&ids[2]].0, pos[&ids[3]].0, "same distance, same column");
        let rects: Vec<Rect> = pos
            .values()
            .map(|(x, y)| Rect::from_min_size(pos2(*x, *y), NODE))
            .collect();
        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                assert!(!rects[i].intersects(rects[j]), "cards {i} and {j} overlap");
            }
        }
    }

    #[test]
    fn reachability_respects_locks_and_one_way_edges() {
        let mut g = WorldGraph::default();
        let a = g.add_room("A");
        let b = g.add_room("B");
        let c = g.add_room("C");
        let d = g.add_room("D");
        g.connect(a, b, false);
        let e = g.connect(b, c, false).unwrap();
        g.edge_mut(e).unwrap().locked = true;
        g.connect(d, a, true); // one-way into A: D is not reachable from A
        let r = reachable(&g, a);
        assert!(r.contains(&b) && !r.contains(&c) && !r.contains(&d));
    }
}
