//! Renders the cached room map (tiles, objects), fog of war and light glows.

use crate::state::{CELL, GameView, Net, cell_to_world};
use crate::textures::TextureCache;
use bevy::prelude::*;
use dnd_world::catalog::Catalog;

#[derive(Resource)]
pub struct CatalogRes(pub Catalog);

#[derive(Component)]
pub struct MapSprite;

#[derive(Component)]
pub struct FogCell(pub usize);

#[derive(Component)]
pub struct LightGlow {
    pub flicker: bool,
    pub phase: f32,
    pub base_alpha: f32,
}

pub fn rebuild_map(
    mut commands: Commands,
    mut gv: ResMut<GameView>,
    old: Query<Entity, Or<(With<MapSprite>, With<FogCell>)>>,
    mut tex: ResMut<TextureCache>,
    mut images: ResMut<Assets<Image>>,
    catalog: Res<CatalogRes>,
) {
    if !gv.map_dirty {
        return;
    }
    gv.map_dirty = false;
    gv.view_dirty = true;
    for e in &old {
        commands.entity(e).despawn();
    }
    let Some(map) = gv.map.clone() else { return };
    let size = Vec2::new(map.width as f32 * CELL, map.height as f32 * CELL);
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
    for (li, layer) in map.layers.iter().enumerate() {
        if !layer.visible {
            continue;
        }
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
            let cell = ((i as i32) % map.width, (i as i32) / map.width);
            let p = cell_to_world(cell);
            commands.spawn((
                MapSprite,
                Sprite {
                    image: tex.asset(def, &mut images),
                    custom_size: Some(Vec2::splat(CELL)),
                    ..default()
                },
                Transform::from_xyz(p.x, p.y, li as f32 * 0.1),
            ));
        }
    }
    for o in &map.objects {
        let Some(def) = catalog.0.get(&o.asset_id) else {
            continue;
        };
        let (w, h) = if o.rotation % 2 == 1 {
            (o.h, o.w)
        } else {
            (o.w, o.h)
        };
        let center = cell_to_world((o.x, o.y))
            + Vec2::new(
                (w as f32 - 1.0) * CELL / 2.0,
                -(h as f32 - 1.0) * CELL / 2.0,
            );
        let alpha = if o.hidden {
            0.45
        } else if o.open {
            0.35
        } else {
            1.0
        };
        commands.spawn((
            MapSprite,
            Sprite {
                image: tex.asset(def, &mut images),
                custom_size: Some(Vec2::new(o.w as f32 * CELL, o.h as f32 * CELL)),
                color: Color::srgba(1.0, 1.0, 1.0, alpha),
                ..default()
            },
            Transform::from_xyz(center.x, center.y, 0.8).with_rotation(Quat::from_rotation_z(
                -(o.rotation as f32) * std::f32::consts::FRAC_PI_2,
            )),
        ));
    }
    for i in 0..(map.width * map.height) as usize {
        let cell = ((i as i32) % map.width, (i as i32) / map.width);
        let p = cell_to_world(cell);
        commands.spawn((
            FogCell(i),
            Sprite {
                color: Color::srgba(0.0, 0.0, 0.0, 0.0),
                custom_size: Some(Vec2::splat(CELL + 0.5)),
                ..default()
            },
            Transform::from_xyz(p.x, p.y, 3.0),
        ));
    }
}

/// Fog of war and light glows from the latest view. Gameplay visibility comes from the host;
/// this only paints it.
pub fn update_fog_and_lights(
    mut commands: Commands,
    gv: Res<GameView>,
    net: Res<Net>,
    mut fog: Query<(&FogCell, &mut Sprite)>,
    glows: Query<Entity, With<LightGlow>>,
    mut tex: ResMut<TextureCache>,
    mut images: ResMut<Assets<Image>>,
    catalog: Res<CatalogRes>,
) {
    if !gv.view_dirty {
        return;
    }
    let Some(view) = &gv.view else { return };
    let Some(room) = &view.room else { return };
    let dm = net.is_dm();
    for (cell, mut sprite) in &mut fog {
        let explored = room.explored.get(cell.0).copied().unwrap_or(false);
        let (rgb, a) = match &room.visible {
            None => {
                // DM: everything visible; unexplored areas get a faint veil so the DM knows what players haven't seen.
                (
                    (0.0, 0.0, 0.0),
                    if explored || !room.is_party_room {
                        0.0
                    } else {
                        0.18
                    },
                )
            }
            Some(vis) => match vis.get(cell.0).copied().unwrap_or(0) {
                3 => ((0.0, 0.0, 0.0), 0.0),
                2 => ((0.02, 0.02, 0.06), 0.38),
                1 => ((0.12, 0.13, 0.16), 0.55), // darkvision: grey wash
                _ if explored => ((0.0, 0.0, 0.0), 0.8),
                _ => ((0.0, 0.0, 0.0), 1.0),
            },
        };
        sprite.color = Color::srgba(rgb.0, rgb.1, rgb.2, a);
    }
    for e in &glows {
        commands.entity(e).despawn();
    }
    let map = gv.map.as_ref();
    for l in &room.lights {
        let radius = (l.bright_ft + l.dim_ft) as f32 / 5.0 * CELL;
        let (color, flicker) = map
            .and_then(|m| m.objects.iter().find(|o| (o.x, o.y) == l.cell))
            .and_then(|o| catalog.0.get(&o.asset_id))
            .and_then(|d| d.light.as_ref())
            .map(|le| (le.color, le.flicker))
            .unwrap_or(([255, 230, 170], false));
        let p = cell_to_world(l.cell);
        let base_alpha = if dm { 0.12 } else { 0.22 };
        commands.spawn((
            LightGlow {
                flicker,
                phase: (l.cell.0 * 7 + l.cell.1 * 13) as f32,
                base_alpha,
            },
            Sprite {
                image: tex.glow(&mut images),
                custom_size: Some(Vec2::splat(radius * 2.0)),
                color: Color::srgba_u8(color[0], color[1], color[2], (base_alpha * 255.0) as u8),
                ..default()
            },
            Transform::from_xyz(p.x, p.y, 3.5),
        ));
    }
}

/// Cosmetic flicker for torches and braziers.
pub fn flicker_lights(time: Res<Time>, mut q: Query<(&LightGlow, &mut Sprite)>) {
    let t = time.elapsed_secs();
    for (g, mut s) in &mut q {
        if g.flicker {
            let f = 1.0 + 0.18 * (t * 9.0 + g.phase).sin() * (t * 3.3 + g.phase * 0.5).cos();
            s.color.set_alpha(g.base_alpha * f);
        }
    }
}
