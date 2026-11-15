//! Line of sight, light levels and per-creature vision.
//!
//! Gameplay visibility (what a creature can actually see, used for hidden-information filtering and
//! rules such as "Unseen Attackers") is computed here, on the host. Cosmetic lighting (glow, flicker,
//! color) is rendered client-side from the same light sources but never decides rules outcomes.
//!
//! Rules (SRD 5.2.1 Rules Glossary): Bright Light is normal illumination; Dim Light is Lightly Obscured;
//! Darkness is Heavily Obscured (effectively Blinded when trying to see into it). Darkvision: within
//! range, Dim Light counts as Bright and Darkness as Dim, seen only in shades of gray.

use crate::map::{AmbientLight, Cell, GridMap};
use crate::path::grid_distance_ft;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LightLevel {
    Darkness = 0,
    Dim = 1,
    Bright = 2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointLight {
    pub cell: Cell,
    pub bright_ft: u32,
    /// Dim light extends this many feet beyond the bright radius.
    pub dim_ft: u32,
}

/// Cells strictly between `a` and `b` on a Bresenham line.
fn line_between(a: Cell, b: Cell) -> Vec<Cell> {
    let (mut x0, mut y0) = a;
    let (x1, y1) = b;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut out = Vec::new();
    loop {
        if (x0, y0) == (x1, y1) {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
        if (x0, y0) != (x1, y1) {
            out.push((x0, y0));
        }
    }
    out
}

/// Whether sight passes from the center of `a` to the center of `b`. Checked in both directions so the
/// result is symmetric (if I can see you, you can see me). Blocking cells themselves can be seen.
pub fn line_of_sight(map: &GridMap, a: Cell, b: Cell) -> bool {
    let clear = |from: Cell, to: Cell| line_between(from, to).into_iter().all(|c| !map.flags(c).blocks_sight);
    clear(a, b) || clear(b, a)
}

/// Light level of every cell, from the map's ambient light plus point lights blocked by walls.
pub fn light_map(map: &GridMap, lights: &[PointLight]) -> Vec<LightLevel> {
    let ambient = match map.ambient {
        AmbientLight::Bright => LightLevel::Bright,
        AmbientLight::Dim => LightLevel::Dim,
        AmbientLight::Darkness => LightLevel::Darkness,
    };
    let mut levels = vec![ambient; (map.width * map.height) as usize];
    if ambient == LightLevel::Bright {
        return levels;
    }
    for l in lights {
        let reach = (l.bright_ft + l.dim_ft) as i32 / crate::FEET_PER_SQUARE;
        for y in (l.cell.1 - reach).max(0)..=(l.cell.1 + reach).min(map.height - 1) {
            for x in (l.cell.0 - reach).max(0)..=(l.cell.0 + reach).min(map.width - 1) {
                let d = grid_distance_ft(l.cell, (x, y)) as u32;
                let lvl = if d <= l.bright_ft {
                    LightLevel::Bright
                } else if d <= l.bright_ft + l.dim_ft {
                    LightLevel::Dim
                } else {
                    continue;
                };
                let i = (y * map.width + x) as usize;
                if lvl > levels[i] && line_of_sight(map, l.cell, (x, y)) {
                    levels[i] = lvl;
                }
            }
        }
    }
    levels
}

/// All point lights in a map: light-emitting objects (except hidden ones) plus extra lights (spells, carried torches).
pub fn collect_lights(map: &GridMap, extra: &[PointLight]) -> Vec<PointLight> {
    let mut v: Vec<PointLight> = map
        .objects
        .iter()
        .filter(|o| !o.hidden)
        .filter_map(|o| o.light.as_ref().map(|l| PointLight { cell: (o.x, o.y), bright_ft: l.bright_ft, dim_ft: l.dim_ft }))
        .collect();
    v.extend_from_slice(extra);
    v
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Senses {
    pub darkvision_ft: u32,
    pub blindsight_ft: u32,
    pub blinded: bool,
}

/// What a viewer perceives in a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Seen {
    NotVisible,
    /// Visible at this effective light level; `grayscale` when seen only through Darkvision in Darkness.
    Visible { effective: LightLevel, grayscale: bool },
}

pub fn perceive(map: &GridMap, lights: &[LightLevel], viewer: Cell, senses: Senses, c: Cell) -> Seen {
    if !map.in_bounds(c) {
        return Seen::NotVisible;
    }
    let d = grid_distance_ft(viewer, c) as u32;
    let los = c == viewer || line_of_sight(map, viewer, c);
    if !los {
        return Seen::NotVisible;
    }
    if senses.blindsight_ft > 0 && d <= senses.blindsight_ft {
        return Seen::Visible { effective: LightLevel::Bright, grayscale: false };
    }
    if senses.blinded {
        return Seen::NotVisible;
    }
    let lvl = lights[(c.1 * map.width + c.0) as usize];
    let in_dv = senses.darkvision_ft > 0 && d <= senses.darkvision_ft;
    match (lvl, in_dv) {
        (LightLevel::Bright, _) => Seen::Visible { effective: LightLevel::Bright, grayscale: false },
        (LightLevel::Dim, true) => Seen::Visible { effective: LightLevel::Bright, grayscale: false },
        (LightLevel::Dim, false) => Seen::Visible { effective: LightLevel::Dim, grayscale: false },
        (LightLevel::Darkness, true) => Seen::Visible { effective: LightLevel::Dim, grayscale: true },
        (LightLevel::Darkness, false) => Seen::NotVisible,
    }
}

/// Visibility of every cell for one viewer (row-major).
pub fn visibility_map(map: &GridMap, lights: &[LightLevel], viewer: Cell, senses: Senses) -> Vec<Seen> {
    let mut out = Vec::with_capacity((map.width * map.height) as usize);
    for y in 0..map.height {
        for x in 0..map.width {
            out.push(perceive(map, lights, viewer, senses, (x, y)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::map::LayerKind;

    fn dark_room() -> GridMap {
        GridMap::new(20, 5, AmbientLight::Darkness).unwrap()
    }

    #[test]
    fn walls_block_sight_symmetrically() {
        let cat = Catalog::builtin();
        let mut m = GridMap::new(7, 3, AmbientLight::Bright).unwrap();
        for y in 0..3 {
            m.set_tile(LayerKind::Walls, (3, y), cat.get("stone_wall")).unwrap();
        }
        assert!(!line_of_sight(&m, (0, 1), (6, 1)));
        assert!(!line_of_sight(&m, (6, 1), (0, 1)));
        assert!(line_of_sight(&m, (0, 1), (3, 1)), "the wall face itself is visible");
        assert!(line_of_sight(&m, (0, 0), (2, 2)));
    }

    #[test]
    fn darkness_hides_everything_without_light_or_darkvision() {
        let m = dark_room();
        let lights = light_map(&m, &[]);
        assert_eq!(perceive(&m, &lights, (0, 2), Senses::default(), (3, 2)), Seen::NotVisible);
    }

    #[test]
    fn darkvision_sees_darkness_as_dim_in_gray_within_range_only() {
        let m = dark_room();
        let lights = light_map(&m, &[]);
        let dv = Senses { darkvision_ft: 60, ..Default::default() };
        assert_eq!(perceive(&m, &lights, (0, 2), dv, (12, 2)), Seen::Visible { effective: LightLevel::Dim, grayscale: true });
        assert_eq!(perceive(&m, &lights, (0, 2), dv, (13, 2)), Seen::NotVisible, "65 ft is out of range");
    }

    #[test]
    fn torch_light_radii_and_wall_shadow() {
        let cat = Catalog::builtin();
        let mut m = dark_room();
        m.set_tile(LayerKind::Walls, (6, 1), cat.get("stone_wall")).unwrap();
        m.set_tile(LayerKind::Walls, (6, 2), cat.get("stone_wall")).unwrap();
        m.set_tile(LayerKind::Walls, (6, 3), cat.get("stone_wall")).unwrap();
        let torch = PointLight { cell: (2, 2), bright_ft: 20, dim_ft: 20 };
        let lights = light_map(&m, &[torch]);
        let at = |x: i32, y: i32| lights[(y * m.width + x) as usize];
        assert_eq!(at(2, 2), LightLevel::Bright);
        assert_eq!(at(0, 2), LightLevel::Bright); // 10 ft
        assert_eq!(at(4, 0), LightLevel::Bright); // 10 ft diagonal-ish
        assert_eq!(at(7, 2), LightLevel::Darkness, "behind the wall");
        let human = Senses::default();
        assert_eq!(perceive(&m, &lights, (0, 2), human, (4, 2)), Seen::Visible { effective: LightLevel::Bright, grayscale: false });
        // Darkvision upgrades dim light to bright.
        let mut open = dark_room();
        open.ambient = AmbientLight::Dim;
        let lights = light_map(&open, &[]);
        let dv = Senses { darkvision_ft: 60, ..Default::default() };
        assert_eq!(perceive(&open, &lights, (0, 0), dv, (5, 0)), Seen::Visible { effective: LightLevel::Bright, grayscale: false });
        assert_eq!(perceive(&open, &lights, (0, 0), human, (5, 0)), Seen::Visible { effective: LightLevel::Dim, grayscale: false });
    }

    #[test]
    fn blinded_sees_nothing_except_blindsight() {
        let m = GridMap::new(5, 5, AmbientLight::Bright).unwrap();
        let lights = light_map(&m, &[]);
        let blind = Senses { blinded: true, ..Default::default() };
        assert_eq!(perceive(&m, &lights, (0, 0), blind, (1, 0)), Seen::NotVisible);
        let bs = Senses { blinded: true, blindsight_ft: 10, ..Default::default() };
        assert!(matches!(perceive(&m, &lights, (0, 0), bs, (2, 0)), Seen::Visible { .. }));
        assert_eq!(perceive(&m, &lights, (0, 0), bs, (3, 0)), Seen::NotVisible);
    }
}
