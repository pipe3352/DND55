//! Renders the cached room map: illustrated terrain with per-cell variants and organic transitions,
//! ambient occlusion along walls, contact shadows, scattered ground details, animated water/lava,
//! soft fog of war and colored light glows.
//!
//! Gameplay authority stays on the host: this module only *paints* the map and the visibility
//! state it receives. Unseen cells are always fully covered; soft fog edges only darken the
//! visible side of a boundary, so nothing hidden is revealed by the smoothing.

use crate::liquid::{LiquidMaterial, LiquidParams, cell_texture, liquid_of};
use crate::settings::GameSettings;
use crate::state::{CELL, GameView, Net, cell_to_world};
use crate::terrain_art::{self as art, Decor};
use bevy::prelude::*;
use dnd_world::catalog::Catalog;
use dnd_world::map::{GridMap, LayerKind};
use std::collections::HashMap;

#[derive(Resource)]
pub struct CatalogRes(pub Catalog);

#[derive(Component)]
pub struct MapSprite;

/// Tile sprite of the Overhead (roof) layer: drawn above tokens, faded when the party is under it.
#[derive(Component)]
pub struct OverheadTile(pub bool);

/// Roofs become see-through while a party member stands under any roof tile (DM can force).
pub fn roof_visibility(
    gv: Res<GameView>,
    ui: Res<crate::state::UiState>,
    mut q: Query<(&OverheadTile, &mut Sprite)>,
) {
    let Some(map) = &gv.map else { return };
    let under = gv.view.as_ref().is_some_and(|v| {
        v.creatures
            .iter()
            .filter(|c| c.faction == dnd_session::model::Faction::Party)
            .any(|c| {
                map.in_bounds(c.pos)
                    && map.layer(LayerKind::Overhead).tiles
                        [(c.pos.1 * map.width + c.pos.0) as usize]
                        != 0
            })
    });
    let alpha = match ui.roof_mode {
        1 => 1.0,
        2 => 0.0,
        _ if under => 0.15,
        _ => 1.0,
    };
    for (o, mut s) in &mut q {
        if o.0 && (s.color.alpha() - alpha).abs() > 0.01 {
            s.color.set_alpha(alpha);
        }
    }
}

/// The fog-of-war overlay (one textured sprite for the whole room).
#[derive(Component)]
pub struct FogCell;

#[derive(Component)]
pub struct LightGlow {
    pub flicker: bool,
    pub phase: f32,
    pub base_alpha: f32,
}

/// Generated environment art, created on first use.
#[derive(Resource, Default)]
pub struct ArtCache {
    tiles: HashMap<(String, u32), Handle<Image>>,
    transitions: HashMap<(String, usize), Handle<Image>>,
    objects: HashMap<String, Handle<Image>>,
    decor: HashMap<Decor, Handle<Image>>,
    ao: HashMap<usize, Handle<Image>>,
    shadow: Option<Handle<Image>>,
    glow: Option<Handle<Image>>,
    fog: Option<Handle<Image>>,
    /// Object art replaced by imported packs (loaded on first use; see `asset_packs`).
    packs: Option<HashMap<String, std::path::PathBuf>>,
}

impl ArtCache {
    fn tile(
        &mut self,
        id: &str,
        variant: u32,
        color: [u8; 3],
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        self.tiles
            .entry((id.to_string(), variant))
            .or_insert_with(|| images.add(crate::pixel_tiles::tile_image(id, variant, color)))
            .clone()
    }
    fn transition(
        &mut self,
        id: &str,
        dir: usize,
        color: [u8; 3],
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        if let Some(h) = self.transitions.get(&(id.to_string(), dir)) {
            return h.clone();
        }
        let base = crate::pixel_tiles::transition_base(id, color);
        // Dithered pixel edge: organic transitions in the same pixel grid as the tiles.
        let h = images.add(crate::pixel::pixelize(&art::transition(&base, dir), true));
        self.transitions.insert((id.to_string(), dir), h.clone());
        h
    }
    fn object(
        &mut self,
        id: &str,
        w: u8,
        h: u8,
        color: [u8; 3],
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let packs = self.packs.get_or_insert_with(|| {
            let root = crate::platform::data_dir().join("asset_packs");
            let ids: Vec<String> = dnd_world::catalog::Catalog::builtin()
                .assets
                .into_iter()
                .filter(|a| a.is_object)
                .map(|a| a.id)
                .collect();
            crate::asset_packs::write_template(&root, &ids);
            let p = crate::asset_packs::scan(&root, &ids);
            for (name, license) in &p.loaded {
                info!("art pack '{name}' loaded ({license})");
            }
            for problem in &p.problems {
                warn!("art pack: {problem}");
            }
            p.art
        });
        let pack = packs.get(id).cloned();
        self.objects
            .entry(format!("{id}:{w}x{h}"))
            .or_insert_with(|| {
                let img = pack
                    .and_then(|p| crate::asset_packs::load_png(&p))
                    .unwrap_or_else(|| crate::pixel_objects::image(id, w, h, color));
                images.add(img)
            })
            .clone()
    }
    fn decor(&mut self, d: Decor, images: &mut Assets<Image>) -> Handle<Image> {
        self.decor
            .entry(d)
            .or_insert_with(|| images.add(crate::pixel::pixelize(&art::decor(d), false)))
            .clone()
    }
    fn ao(&mut self, dir: usize, images: &mut Assets<Image>) -> Handle<Image> {
        self.ao
            .entry(dir)
            .or_insert_with(|| images.add(art::ambient_occlusion(dir)))
            .clone()
    }
    fn shadow(&mut self, images: &mut Assets<Image>) -> Handle<Image> {
        self.shadow
            .get_or_insert_with(|| images.add(art::shadow()))
            .clone()
    }
    pub fn glow(&mut self, images: &mut Assets<Image>) -> Handle<Image> {
        self.glow
            .get_or_insert_with(|| images.add(art::glow()))
            .clone()
    }
}

/// How strongly a terrain spreads onto its neighbours (higher spreads over lower).
/// `None`: structured or liquid surfaces that never spread (they can still receive overlays).
fn spread_priority(id: &str) -> Option<u8> {
    Some(match id {
        "ice" => 1,
        "mud" => 2,
        "cave_floor" => 3,
        "gravel" => 4,
        "sand" => 5,
        "snow" => 6,
        "dirt" => 7,
        "rubble" => 8,
        "grass" => 9,
        "undergrowth" => 10,
        _ => return None,
    })
}

fn receive_priority(id: Option<&str>) -> i32 {
    match id {
        None => -1,
        Some(i) => spread_priority(i).map(|p| p as i32).unwrap_or(0),
    }
}

/// Smooth low-frequency brightness variation across the map (breaks up large uniform areas).
fn macro_tint(x: i32, y: i32) -> f32 {
    let f = |x: f32, y: f32| {
        let (xi, yi) = (x.floor() as i32, y.floor() as i32);
        let (fx, fy) = (x - x.floor(), y - y.floor());
        let s = |t: f32| t * t * (3.0 - 2.0 * t);
        let h = |a: i32, b: i32| art::cell_hash(a, b, 4242);
        let (a, b, c, d) = (h(xi, yi), h(xi + 1, yi), h(xi, yi + 1), h(xi + 1, yi + 1));
        let (u, v) = (s(fx), s(fy));
        a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
    };
    0.93 + f(x as f32 / 5.0, y as f32 / 5.0) * 0.14
}

fn terrain_ids(map: &GridMap) -> Vec<Option<String>> {
    let layer = map.layers.iter().find(|l| l.kind == LayerKind::Terrain);
    (0..(map.width * map.height) as usize)
        .map(|i| {
            let l = layer.filter(|l| l.visible)?;
            let v = l.tiles[i];
            (v != 0)
                .then(|| map.palette.get(v as usize - 1).map(|p| p.asset_id.clone()))
                .flatten()
        })
        .collect()
}

fn wall_cells(map: &GridMap) -> Vec<bool> {
    let layer = map.layers.iter().find(|l| l.kind == LayerKind::Walls);
    (0..(map.width * map.height) as usize)
        .map(|i| layer.is_some_and(|l| l.visible && l.tiles[i] != 0))
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn rebuild_map(
    mut commands: Commands,
    mut gv: ResMut<GameView>,
    old: Query<Entity, Or<(With<MapSprite>, With<FogCell>)>>,
    mut cache: ResMut<ArtCache>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut liquids: ResMut<Assets<LiquidMaterial>>,
    catalog: Res<CatalogRes>,
    settings: Res<GameSettings>,
    ui: Res<crate::state::UiState>,
    mut last_quality: Local<Option<crate::settings::Quality>>,
) {
    if *last_quality != Some(settings.quality) {
        *last_quality = Some(settings.quality);
        gv.map_dirty = true;
    }
    if !gv.map_dirty {
        return;
    }
    gv.map_dirty = false;
    gv.view_dirty = true;
    for e in &old {
        commands.entity(e).despawn();
    }
    let Some(map) = gv.map.clone() else { return };
    let q = settings.quality;
    let (w, h) = (map.width, map.height);
    let size = Vec2::new(w as f32 * CELL, h as f32 * CELL);
    let [r, g, b] = map.background;
    commands.spawn((
        MapSprite,
        Sprite {
            color: Color::srgb_u8(r, g, b),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(size.x / 2.0, -size.y / 2.0, -1.0),
    ));
    let terrain = terrain_ids(&map);
    let walls = wall_cells(&map);
    let at = |x: i32, y: i32| -> Option<&str> {
        if x < 0 || y < 0 || x >= w || y >= h {
            return None;
        }
        terrain[(y * w + x) as usize].as_deref()
    };
    let wall_at =
        |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && walls[(y * w + x) as usize];

    // Tiles of every visible layer.
    for (li, layer) in map.layers.iter().enumerate() {
        if !layer.visible {
            continue;
        }
        let is_wall_layer = layer.kind == LayerKind::Walls;
        for (i, v) in layer.tiles.iter().enumerate() {
            if *v == 0 {
                continue;
            }
            let Some(entry) = map.palette.get(*v as usize - 1) else {
                continue;
            };
            let Some(def) = catalog.0.get(&entry.asset_id) else {
                continue;
            };
            let cell = ((i as i32) % w, (i as i32) / w);
            let p = cell_to_world(cell);
            let variant =
                (art::cell_hash(cell.0, cell.1, 17) * art::VARIANTS as f32) as u32 % art::VARIANTS;
            let k = macro_tint(cell.0, cell.1);
            if is_wall_layer && q.shadows() {
                // Walls are raised: a soft shadow falls down-right onto the floor.
                commands.spawn((
                    MapSprite,
                    Sprite {
                        color: Color::srgba(0.02, 0.01, 0.01, 0.42),
                        custom_size: Some(Vec2::splat(CELL)),
                        ..default()
                    },
                    Transform::from_xyz(
                        p.x + CELL * 0.14,
                        p.y - CELL * 0.14,
                        li as f32 * 0.1 - 0.02,
                    ),
                ));
            }
            commands.spawn((
                MapSprite,
                Sprite {
                    image: cache.tile(&def.id, variant, def.color, &mut images),
                    custom_size: Some(Vec2::splat(CELL + 0.6)), // tiny overlap hides seams
                    color: Color::srgb(k, k, k),
                    ..default()
                },
                Transform::from_xyz(
                    p.x,
                    p.y,
                    if layer.kind == LayerKind::Overhead {
                        2.85
                    } else {
                        li as f32 * 0.1
                    },
                ),
                OverheadTile(layer.kind == LayerKind::Overhead),
            ));
        }
    }

    // Organic transitions: neighbours with a higher spread priority creep over this cell's edges.
    if q.transitions() {
        for y in 0..h {
            for x in 0..w {
                let here = receive_priority(at(x, y));
                let mut overlays: Vec<(u8, usize, String)> = vec![];
                for (dir, (dx, dy)) in art::DIRS.iter().enumerate() {
                    let Some(n) = at(x + dx, y + dy) else {
                        continue;
                    };
                    let Some(pn) = spread_priority(n) else {
                        continue;
                    };
                    if pn as i32 <= here || Some(n) == at(x, y) {
                        continue;
                    }
                    if dir >= 4 {
                        // Outer corner only when neither adjacent edge already carries this material.
                        if at(x + dx, y) == Some(n) || at(x, y + dy) == Some(n) {
                            continue;
                        }
                    }
                    overlays.push((pn, dir, n.to_string()));
                }
                overlays.sort();
                let p = cell_to_world((x, y));
                let k = macro_tint(x, y);
                for (i, (_, dir, id)) in overlays.into_iter().enumerate() {
                    let color = catalog
                        .0
                        .get(&id)
                        .map(|d| d.color)
                        .unwrap_or([128, 128, 128]);
                    commands.spawn((
                        MapSprite,
                        Sprite {
                            image: cache.transition(&id, dir, color, &mut images),
                            custom_size: Some(Vec2::splat(CELL + 0.6)),
                            color: Color::srgb(k, k, k),
                            ..default()
                        },
                        Transform::from_xyz(p.x, p.y, 0.02 + i as f32 * 0.002),
                    ));
                }
            }
        }
    }

    // Ambient occlusion along walls, and scattered ground details.
    for y in 0..h {
        for x in 0..w {
            if wall_at(x, y) {
                continue;
            }
            let p = cell_to_world((x, y));
            if q.shadows() {
                for (dir, (dx, dy)) in art::DIRS.iter().take(4).enumerate() {
                    if wall_at(x + dx, y + dy) {
                        commands.spawn((
                            MapSprite,
                            Sprite {
                                image: cache.ao(dir, &mut images),
                                custom_size: Some(Vec2::splat(CELL)),
                                ..default()
                            },
                            Transform::from_xyz(p.x, p.y, 0.05),
                        ));
                    }
                }
            }
            let density = q.decor_density();
            if density > 0.0
                && let Some(t) = at(x, y)
                && liquid_of(t).is_none()
            {
                let hsh = art::cell_hash(x, y, 99);
                if let Some(d) = art::decor_for(t, hsh / density) {
                    let ox = (art::cell_hash(x, y, 7) - 0.5) * CELL * 0.4;
                    let oy = (art::cell_hash(x, y, 8) - 0.5) * CELL * 0.4;
                    let rot = art::cell_hash(x, y, 9) * std::f32::consts::TAU;
                    commands.spawn((
                        MapSprite,
                        Sprite {
                            image: cache.decor(d, &mut images),
                            custom_size: Some(Vec2::splat(CELL * 0.55)),
                            ..default()
                        },
                        Transform::from_xyz(p.x + ox, p.y + oy, 0.08)
                            .with_rotation(Quat::from_rotation_z(rot)),
                    ));
                }
            }
        }
    }

    // Animated liquids: one quad over the room, driven by a per-cell texture.
    let cells: Vec<Option<[u8; 3]>> = terrain
        .iter()
        .map(|t| t.as_deref().and_then(liquid_of))
        .collect();
    if cells.iter().any(Option::is_some) {
        let tex = images.add(cell_texture(w as u32, h as u32, &cells));
        let mat = liquids.add(LiquidMaterial {
            params: LiquidParams {
                a: Vec4::new(0.0, q.liquid(), w as f32, h as f32),
                b: Vec4::ZERO,
            },
            cells: tex,
        });
        commands.spawn((
            MapSprite,
            Mesh2d(meshes.add(Rectangle::new(size.x, size.y))),
            MeshMaterial2d(mat),
            Transform::from_xyz(size.x / 2.0, -size.y / 2.0, 0.07),
        ));
    }

    // Objects with contact shadows.
    for o in &map.objects {
        // Local editor filter: hide hidden objects, or all objects.
        if ui.obj_filter == 2 || ui.obj_filter == 1 && o.hidden {
            continue;
        }
        let Some(def) = catalog.0.get(&o.asset_id) else {
            continue;
        };
        let (ow, oh) = if o.rotation % 2 == 1 {
            (o.h, o.w)
        } else {
            (o.w, o.h)
        };
        let center = cell_to_world((o.x, o.y))
            + Vec2::new(
                (ow as f32 - 1.0) * CELL / 2.0,
                -(oh as f32 - 1.0) * CELL / 2.0,
            );
        let alpha = if o.hidden {
            0.45
        } else if o.open {
            0.35
        } else {
            1.0
        };
        let flat = matches!(
            o.asset_id.as_str(),
            "rune_circle"
                | "pressure_plate"
                | "bones"
                | "carpet"
                | "rug"
                | "puddle"
                | "bloodstain"
                | "ritual_symbol"
                | "leaf_pile"
                | "small_stones"
                | "tall_grass"
                | "cobweb"
                | "spike_trap"
                | "pit_trap"
                | "scroll"
                | "wildflowers"
                | "roots"
                | "vines"
                | "fern"
                | "secret_door"
                | "portal"
                | "chains"
                | "stone_stairs"
        );
        use dnd_world::map::ObjectLayer;
        let flat = flat || o.layer == ObjectLayer::Floor;
        // Floor decals under everything, standard objects under tokens (z 2), overhead above them.
        let (z, layer_alpha) = match o.layer {
            ObjectLayer::Floor => (0.62, 1.0),
            ObjectLayer::Standard => (0.8, 1.0),
            ObjectLayer::Overhead => (2.8, 0.85),
        };
        // Pixel sprites carry their own ground shadow.
        if q.shadows() && !flat && !crate::pixel_objects::has(&o.asset_id) {
            let tall = matches!(
                o.asset_id.as_str(),
                "tree"
                    | "big_tree"
                    | "pillar"
                    | "stalagmite"
                    | "bookshelf"
                    | "pine_tree"
                    | "birch_tree"
                    | "dead_tree"
                    | "ancient_tree"
                    | "statue"
                    | "wardrobe"
                    | "hanging_cage"
                    | "stone_arch"
                    | "magic_crystal"
            );
            let off = if tall { 0.18 } else { 0.09 };
            commands.spawn((
                MapSprite,
                Sprite {
                    image: cache.shadow(&mut images),
                    custom_size: Some(Vec2::new(ow as f32 * CELL * 0.95, oh as f32 * CELL * 0.8)),
                    color: Color::srgba(1.0, 1.0, 1.0, 0.55 * alpha),
                    ..default()
                },
                Transform::from_xyz(center.x + CELL * off, center.y - CELL * off, 0.75),
            ));
        }
        commands.spawn((
            MapSprite,
            Sprite {
                image: cache.object(&def.id, o.w, o.h, def.color, &mut images),
                custom_size: Some(Vec2::new(o.w as f32 * CELL, o.h as f32 * CELL)),
                color: Color::srgba(1.0, 1.0, 1.0, alpha * layer_alpha),
                ..default()
            },
            Transform::from_xyz(center.x, center.y, z).with_rotation(Quat::from_rotation_z(
                -(o.rotation as f32) * std::f32::consts::FRAC_PI_2,
            )),
        ));
    }

    // Fog of war: a single texture (8 px per cell) refreshed from each view.
    let fog = images.add(fog_image(w as u32, h as u32));
    cache.fog = Some(fog.clone());
    commands.spawn((
        FogCell,
        Sprite {
            image: fog,
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(size.x / 2.0, -size.y / 2.0, 3.0),
    ));
}

const FOG_PX: u32 = 8;

fn fog_image(w: u32, h: u32) -> Image {
    let mut img = Image::new_fill(
        bevy::render::render_resource::Extent3d {
            width: w * FOG_PX,
            height: h * FOG_PX,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        &[0, 0, 0, 255],
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = bevy::image::ImageSampler::linear();
    img
}

/// Fog color and opacity for one cell from the authoritative view.
fn fog_cell(room: &dnd_session::view::RoomView, i: usize) -> ([f32; 3], f32) {
    let explored = room.explored.get(i).copied().unwrap_or(false);
    match &room.visible {
        None => (
            [0.0; 3],
            if explored || !room.is_party_room {
                0.0
            } else {
                0.18
            },
        ),
        Some(vis) => match vis.get(i).copied().unwrap_or(0) {
            3 => ([0.0; 3], 0.0),
            2 => ([0.02, 0.02, 0.06], 0.38),
            1 => ([0.12, 0.13, 0.16], 0.55), // darkvision: grey wash
            _ if explored => ([0.0; 3], 0.8),
            _ => ([0.0; 3], 1.0),
        },
    }
}

/// Paints the fog texture. Each cell keeps at least its own opacity; with soft edges a cell also
/// darkens towards *darker* neighbours (never lightens), so an unseen cell is never uncovered.
fn paint_fog(data: &mut [u8], room: &dnd_session::view::RoomView, w: i32, h: i32, soft: bool) {
    let cell = |x: i32, y: i32| -> Option<([f32; 3], f32)> {
        (x >= 0 && y >= 0 && x < w && y < h).then(|| fog_cell(room, (y * w + x) as usize))
    };
    let stride = (w as u32 * FOG_PX) as usize;
    for cy in 0..h {
        for cx in 0..w {
            let (rgb, a) = cell(cx, cy).expect("in bounds");
            let neighbours: Vec<((i32, i32), [f32; 3], f32)> = if soft {
                art::DIRS
                    .iter()
                    .filter_map(|(dx, dy)| {
                        cell(cx + dx, cy + dy)
                            .filter(|n| n.1 > a + 0.01)
                            .map(|n| ((*dx, *dy), n.0, n.1))
                    })
                    .collect()
            } else {
                vec![]
            };
            for sy in 0..FOG_PX {
                for sx in 0..FOG_PX {
                    let (u, v) = (
                        (sx as f32 + 0.5) / FOG_PX as f32,
                        (sy as f32 + 0.5) / FOG_PX as f32,
                    );
                    let (mut col, mut al) = (rgb, a);
                    for ((dx, dy), nrgb, na) in &neighbours {
                        let ex = match dx {
                            1 => 1.0 - u,
                            -1 => u,
                            _ => 0.0,
                        };
                        let ey = match dy {
                            1 => 1.0 - v,
                            -1 => v,
                            _ => 0.0,
                        };
                        let d = if *dx != 0 && *dy != 0 {
                            (ex * ex + ey * ey).sqrt()
                        } else {
                            ex + ey
                        };
                        let k = (1.0 - (d / 0.5).min(1.0)).powf(1.5) * 0.85;
                        let target = a + (na - a) * k;
                        if target > al {
                            al = target;
                            for c in 0..3 {
                                col[c] = rgb[c] + (nrgb[c] - rgb[c]) * k;
                            }
                        }
                    }
                    let i = ((cy as u32 * FOG_PX + sy) as usize * stride
                        + (cx as u32 * FOG_PX + sx) as usize)
                        * 4;
                    data[i] = (col[0] * 255.0) as u8;
                    data[i + 1] = (col[1] * 255.0) as u8;
                    data[i + 2] = (col[2] * 255.0) as u8;
                    data[i + 3] = (al.clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
        }
    }
}

/// Fog of war and light glows from the latest view. Gameplay visibility comes from the host;
/// this only paints it.
#[allow(clippy::too_many_arguments)]
pub fn update_fog_and_lights(
    mut commands: Commands,
    gv: Res<GameView>,
    net: Res<Net>,
    glows: Query<Entity, With<LightGlow>>,
    mut cache: ResMut<ArtCache>,
    mut images: ResMut<Assets<Image>>,
    catalog: Res<CatalogRes>,
    settings: Res<GameSettings>,
) {
    if !gv.view_dirty {
        return;
    }
    let Some(view) = &gv.view else { return };
    let Some(room) = &view.room else { return };
    let dm = net.is_dm();
    if let (Some(map), Some(h)) = (gv.map.as_ref(), cache.fog.clone())
        && let Some(mut img) = images.get_mut(&h)
        && let Some(data) = img.data.as_mut()
        && data.len() == (map.width as u32 * FOG_PX * map.height as u32 * FOG_PX * 4) as usize
    {
        paint_fog(
            data,
            room,
            map.width,
            map.height,
            settings.quality.soft_fog(),
        );
    }
    for e in &glows {
        commands.entity(e).despawn();
    }
    let map = gv.map.as_ref();
    let terrain = map.map(terrain_ids);
    for l in &room.lights {
        let radius = (l.bright_ft + l.dim_ft) as f32 / 5.0 * CELL;
        let tile_light = map.zip(terrain.as_ref()).and_then(|(m, t)| {
            let i = (l.cell.1 * m.width + l.cell.0) as usize;
            t.get(i).cloned().flatten().filter(|id| id == "lava")
        });
        let (color, flicker) = if tile_light.is_some() {
            ([255, 110, 40], true)
        } else {
            map.and_then(|m| m.objects.iter().find(|o| (o.x, o.y) == l.cell))
                .and_then(|o| catalog.0.get(&o.asset_id))
                .and_then(|d| d.light.as_ref())
                .map(|le| (le.color, le.flicker))
                .unwrap_or(([200, 210, 255], false)) // spells / DM lights: cool magical white
        };
        let p = cell_to_world(l.cell);
        let base_alpha =
            if dm { 0.14 } else { 0.24 } * if tile_light.is_some() { 0.55 } else { 1.0 };
        commands.spawn((
            LightGlow {
                flicker,
                phase: (l.cell.0 * 7 + l.cell.1 * 13) as f32,
                base_alpha,
            },
            Sprite {
                image: cache.glow(&mut images),
                custom_size: Some(Vec2::splat(radius * 2.0)),
                color: Color::srgba_u8(color[0], color[1], color[2], (base_alpha * 255.0) as u8),
                ..default()
            },
            Transform::from_xyz(p.x, p.y, 3.5),
        ));
    }
}

/// Cosmetic flicker for torches, braziers and lava.
pub fn flicker_lights(
    time: Res<Time>,
    settings: Res<GameSettings>,
    ui: Res<crate::state::UiState>,
    mut q: Query<(&LightGlow, &mut Sprite)>,
) {
    let t = time.elapsed_secs();
    for (g, mut s) in &mut q {
        if ui.fx_filter == 2 {
            s.color.set_alpha(0.0);
        } else if settings.reduced_motion || !g.flicker {
            s.color.set_alpha(g.base_alpha);
        } else {
            let f = 1.0 + 0.18 * (t * 9.0 + g.phase).sin() * (t * 3.3 + g.phase * 0.5).cos();
            s.color.set_alpha(g.base_alpha * f);
        }
    }
}

/// Translucent preview of the asset that the Pencil/Rect/Fill/Object tool would place under the cursor.
#[derive(Component)]
pub struct PlacementGhost;

#[allow(clippy::too_many_arguments)]
pub fn placement_ghost(
    mut commands: Commands,
    ui: Res<crate::state::UiState>,
    hover: Res<crate::input::Hover>,
    net: Res<Net>,
    catalog: Res<CatalogRes>,
    mut cache: ResMut<ArtCache>,
    mut images: ResMut<Assets<Image>>,
    mut ghost: Query<(Entity, &mut Sprite, &mut Transform), With<PlacementGhost>>,
) {
    use crate::state::Tool;
    let placing = net.is_dm()
        && matches!(
            ui.tool,
            Tool::Pencil | Tool::Rect | Tool::Fill | Tool::Object
        );
    let def = catalog.0.get(&ui.asset).cloned();
    let (Some(cell), true, Some(def)) = (hover.cell, placing, def) else {
        for (e, ..) in &ghost {
            commands.entity(e).despawn();
        }
        return;
    };
    let (w, h) = if def.is_object {
        (def.size.0, def.size.1)
    } else {
        (1, 1)
    };
    // Rotation chosen before placing (R / Shift+R): the footprint swaps on odd quarter turns.
    let rot = if def.is_object {
        ui.place_rotation % 4
    } else {
        0
    };
    let (fw, fh) = if rot % 2 == 1 { (h, w) } else { (w, h) };
    let center = cell_to_world(cell)
        + Vec2::new(
            (fw as f32 - 1.0) * CELL / 2.0,
            -(fh as f32 - 1.0) * CELL / 2.0,
        );
    let quat = Quat::from_rotation_z(-(rot as f32) * std::f32::consts::FRAC_PI_2);
    let image = if def.is_object {
        cache.object(&def.id, w, h, def.color, &mut images)
    } else {
        cache.tile(&def.id, 0, def.color, &mut images)
    };
    let size = Vec2::new(w as f32 * CELL, h as f32 * CELL);
    match ghost.single_mut() {
        Ok((_, mut s, mut t)) => {
            s.image = image;
            s.custom_size = Some(size);
            t.translation = center.extend(2.9);
            t.rotation = quat;
        }
        Err(_) => {
            commands.spawn((
                PlacementGhost,
                Sprite {
                    image,
                    custom_size: Some(size),
                    color: Color::srgba(1.0, 1.0, 1.0, 0.55),
                    ..default()
                },
                Transform::from_translation(center.extend(2.9)).with_rotation(quat),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_fog_never_uncovers_an_unseen_cell() {
        // 3×1 room: visible | visible | unseen. Every pixel of the unseen cell must stay opaque.
        let room = dnd_session::view::RoomView {
            id: dnd_world::RoomId(1),
            name: String::new(),
            description: String::new(),
            map_rev: 0,
            map: None,
            width: 3,
            height: 1,
            visible: Some(vec![3, 3, 0]),
            explored: vec![true, true, false],
            lights: vec![],
            entry: (0, 0),
            is_party_room: true,
            drawings: vec![],
            containers: vec![],
            music: None,
            ambience: None,
            weather: Default::default(),
        };
        let mut data = vec![0u8; (3 * FOG_PX * FOG_PX * 4) as usize];
        paint_fog(&mut data, &room, 3, 1, true);
        let stride = 3 * FOG_PX as usize;
        for sy in 0..FOG_PX as usize {
            for sx in 0..FOG_PX as usize {
                let unseen = data[(sy * stride + 2 * FOG_PX as usize + sx) * 4 + 3];
                assert_eq!(unseen, 255, "unseen cell pixel {sx},{sy} is not opaque");
            }
            // The visible cell next to it darkens towards the edge but its far side stays clear.
            let near = data[(sy * stride + 2 * FOG_PX as usize - 1) * 4 + 3];
            let far = data[(sy * stride + FOG_PX as usize) * 4 + 3];
            assert!(near > 100 && far < 10, "near {near} far {far}");
        }
    }

    #[test]
    fn transitions_follow_priorities() {
        assert!(spread_priority("grass") > spread_priority("dirt"));
        assert_eq!(
            spread_priority("stone_floor"),
            None,
            "structured floors never spread"
        );
        assert_eq!(
            spread_priority("water_deep"),
            None,
            "liquids are drawn by the shader"
        );
    }
}
