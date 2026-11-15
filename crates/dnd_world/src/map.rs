//! Layered tile maps with multi-cell objects and per-cell collision.

use crate::catalog::{AssetDef, LightEmission};
use crate::ids::ObjectId;
use serde::{Deserialize, Serialize};

pub type Cell = (i32, i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LayerKind {
    Terrain,
    FloorDecoration,
    Walls,
    Furniture,
    Overhead,
    Effects,
}

impl LayerKind {
    pub const DEFAULT_ORDER: [LayerKind; 6] =
        [LayerKind::Terrain, LayerKind::FloorDecoration, LayerKind::Walls, LayerKind::Furniture, LayerKind::Overhead, LayerKind::Effects];
}

/// Palette entry: the asset plus the collision flags this map uses for it. Flags are copied from the
/// catalog when first painted and can then be edited per map ("collision editing").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaletteEntry {
    pub asset_id: String,
    pub blocks_movement: bool,
    pub blocks_sight: bool,
    pub difficult: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub kind: LayerKind,
    pub visible: bool,
    pub locked: bool,
    /// Row-major; 0 = empty, otherwise palette index + 1.
    pub tiles: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AmbientLight {
    Bright,
    Dim,
    Darkness,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapObject {
    pub id: ObjectId,
    pub asset_id: String,
    pub x: i32,
    pub y: i32,
    pub w: u8,
    pub h: u8,
    /// Quarter turns clockwise.
    pub rotation: u8,
    /// Hidden objects are never sent to players (secret doors, unrevealed traps).
    pub hidden: bool,
    pub blocks_movement: bool,
    pub blocks_sight: bool,
    pub difficult: bool,
    pub light: Option<LightEmission>,
    pub interactable: bool,
    /// Open doors stop blocking; chests track their opened state.
    pub open: bool,
    pub label: String,
    /// DM-only notes.
    pub dm_notes: String,
}

impl MapObject {
    pub fn from_asset(id: ObjectId, a: &AssetDef, x: i32, y: i32) -> Self {
        MapObject {
            id,
            asset_id: a.id.clone(),
            x,
            y,
            w: a.size.0,
            h: a.size.1,
            rotation: 0,
            hidden: false,
            blocks_movement: a.blocks_movement,
            blocks_sight: a.blocks_sight,
            difficult: a.difficult,
            light: a.light.clone(),
            interactable: matches!(a.id.as_str(), "door" | "chest" | "lever"),
            open: false,
            label: String::new(),
            dm_notes: String::new(),
        }
    }

    pub fn footprint(&self) -> impl Iterator<Item = Cell> + '_ {
        let (w, h) = if self.rotation % 2 == 1 { (self.h, self.w) } else { (self.w, self.h) };
        (0..h as i32).flat_map(move |dy| (0..w as i32).map(move |dx| (self.x + dx, self.y + dy)))
    }
    pub fn covers(&self, c: Cell) -> bool {
        self.footprint().any(|f| f == c)
    }
    fn effective_blocks_movement(&self) -> bool {
        self.blocks_movement && !(self.open && self.asset_id == "door")
    }
    fn effective_blocks_sight(&self) -> bool {
        self.blocks_sight && !(self.open && self.asset_id == "door")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridMap {
    pub width: i32,
    pub height: i32,
    pub palette: Vec<PaletteEntry>,
    pub layers: Vec<Layer>,
    pub objects: Vec<MapObject>,
    pub ambient: AmbientLight,
    pub background: [u8; 3],
    pub indoor: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CellFlags {
    pub blocks_movement: bool,
    pub blocks_sight: bool,
    pub difficult: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MapError {
    #[error("map size must be between 1x1 and 256x256")]
    BadSize,
    #[error("cell out of bounds")]
    OutOfBounds,
    #[error("layer is locked")]
    LayerLocked,
    #[error("palette is full")]
    PaletteFull,
}

pub const MAX_MAP_SIDE: i32 = 256;

impl GridMap {
    pub fn new(width: i32, height: i32, ambient: AmbientLight) -> Result<Self, MapError> {
        if !(1..=MAX_MAP_SIDE).contains(&width) || !(1..=MAX_MAP_SIDE).contains(&height) {
            return Err(MapError::BadSize);
        }
        let n = (width * height) as usize;
        Ok(GridMap {
            width,
            height,
            palette: vec![],
            layers: LayerKind::DEFAULT_ORDER.iter().map(|k| Layer { kind: *k, visible: true, locked: false, tiles: vec![0; n] }).collect(),
            objects: vec![],
            ambient,
            background: [20, 18, 16],
            indoor: true,
        })
    }

    pub fn in_bounds(&self, (x, y): Cell) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }
    fn idx(&self, (x, y): Cell) -> usize {
        (y * self.width + x) as usize
    }

    pub fn layer_mut(&mut self, kind: LayerKind) -> &mut Layer {
        self.layers.iter_mut().find(|l| l.kind == kind).expect("all layers exist")
    }
    pub fn layer(&self, kind: LayerKind) -> &Layer {
        self.layers.iter().find(|l| l.kind == kind).expect("all layers exist")
    }

    fn palette_index(&mut self, a: &AssetDef) -> Result<u16, MapError> {
        if let Some(i) = self.palette.iter().position(|p| p.asset_id == a.id) {
            return Ok(i as u16 + 1);
        }
        if self.palette.len() >= u16::MAX as usize - 1 {
            return Err(MapError::PaletteFull);
        }
        self.palette.push(PaletteEntry {
            asset_id: a.id.clone(),
            blocks_movement: a.blocks_movement,
            blocks_sight: a.blocks_sight,
            difficult: a.difficult,
        });
        Ok(self.palette.len() as u16)
    }

    /// Paints (`Some`) or erases (`None`) a tile. Returns the previous palette value for undo.
    pub fn set_tile(&mut self, kind: LayerKind, c: Cell, asset: Option<&AssetDef>) -> Result<u16, MapError> {
        if !self.in_bounds(c) {
            return Err(MapError::OutOfBounds);
        }
        if self.layer(kind).locked {
            return Err(MapError::LayerLocked);
        }
        let v = match asset {
            Some(a) => self.palette_index(a)?,
            None => 0,
        };
        let i = self.idx(c);
        let layer = self.layer_mut(kind);
        Ok(std::mem::replace(&mut layer.tiles[i], v))
    }

    pub fn tile(&self, kind: LayerKind, c: Cell) -> Option<&PaletteEntry> {
        if !self.in_bounds(c) {
            return None;
        }
        match self.layer(kind).tiles[self.idx(c)] {
            0 => None,
            v => self.palette.get(v as usize - 1),
        }
    }

    /// Rectangle fill (inclusive corners).
    pub fn fill_rect(&mut self, kind: LayerKind, a: Cell, b: Cell, asset: Option<&AssetDef>) -> Result<(), MapError> {
        for y in a.1.min(b.1)..=a.1.max(b.1) {
            for x in a.0.min(b.0)..=a.0.max(b.0) {
                if self.in_bounds((x, y)) {
                    self.set_tile(kind, (x, y), asset)?;
                }
            }
        }
        Ok(())
    }

    /// Bucket fill: replaces the contiguous (4-connected) region that has the same tile as `start`.
    pub fn flood_fill(&mut self, kind: LayerKind, start: Cell, asset: Option<&AssetDef>) -> Result<usize, MapError> {
        if !self.in_bounds(start) {
            return Err(MapError::OutOfBounds);
        }
        let target = self.layer(kind).tiles[self.idx(start)];
        let new = match asset {
            Some(a) => self.palette_index(a)?,
            None => 0,
        };
        if target == new {
            return Ok(0);
        }
        if self.layer(kind).locked {
            return Err(MapError::LayerLocked);
        }
        let mut stack = vec![start];
        let mut changed = 0;
        while let Some(c) = stack.pop() {
            if !self.in_bounds(c) {
                continue;
            }
            let i = self.idx(c);
            let layer = self.layer_mut(kind);
            if layer.tiles[i] != target {
                continue;
            }
            layer.tiles[i] = new;
            changed += 1;
            stack.extend([(c.0 + 1, c.1), (c.0 - 1, c.1), (c.0, c.1 + 1), (c.0, c.1 - 1)]);
        }
        Ok(changed)
    }

    /// Collision and sight flags for a cell, combining every tile layer and every object covering it.
    /// Out-of-bounds cells block everything.
    pub fn flags(&self, c: Cell) -> CellFlags {
        if !self.in_bounds(c) {
            return CellFlags { blocks_movement: true, blocks_sight: true, difficult: false };
        }
        let mut f = CellFlags::default();
        let i = self.idx(c);
        for l in &self.layers {
            let v = l.tiles[i];
            if v != 0 {
                if let Some(p) = self.palette.get(v as usize - 1) {
                    f.blocks_movement |= p.blocks_movement;
                    f.blocks_sight |= p.blocks_sight;
                    f.difficult |= p.difficult;
                }
            }
        }
        for o in self.objects.iter().filter(|o| o.covers(c)) {
            f.blocks_movement |= o.effective_blocks_movement();
            f.blocks_sight |= o.effective_blocks_sight();
            f.difficult |= o.difficult;
        }
        f
    }

    pub fn object(&self, id: ObjectId) -> Option<&MapObject> {
        self.objects.iter().find(|o| o.id == id)
    }
    pub fn object_mut(&mut self, id: ObjectId) -> Option<&mut MapObject> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    /// Copy of the map safe to send to players: hidden objects and DM notes removed.
    pub fn player_view(&self) -> GridMap {
        let mut m = self.clone();
        m.objects.retain(|o| !o.hidden);
        for o in &mut m.objects {
            o.dm_notes.clear();
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;

    #[test]
    fn paint_erase_and_flags() {
        let cat = Catalog::builtin();
        let mut m = GridMap::new(10, 8, AmbientLight::Bright).unwrap();
        m.fill_rect(LayerKind::Terrain, (0, 0), (9, 7), cat.get("stone_floor")).unwrap();
        m.set_tile(LayerKind::Walls, (3, 3), cat.get("stone_wall")).unwrap();
        assert!(m.flags((3, 3)).blocks_movement && m.flags((3, 3)).blocks_sight);
        assert!(!m.flags((4, 3)).blocks_movement);
        assert!(m.flags((-1, 0)).blocks_movement);
        m.set_tile(LayerKind::Walls, (3, 3), None).unwrap();
        assert!(!m.flags((3, 3)).blocks_movement);
        assert_eq!(m.palette.len(), 2);
    }

    #[test]
    fn locked_layer_rejects_edits() {
        let cat = Catalog::builtin();
        let mut m = GridMap::new(4, 4, AmbientLight::Bright).unwrap();
        m.layer_mut(LayerKind::Terrain).locked = true;
        assert_eq!(m.set_tile(LayerKind::Terrain, (0, 0), cat.get("grass")), Err(MapError::LayerLocked));
    }

    #[test]
    fn flood_fill_stays_in_region() {
        let cat = Catalog::builtin();
        let mut m = GridMap::new(5, 5, AmbientLight::Bright).unwrap();
        for y in 0..5 {
            m.set_tile(LayerKind::Terrain, (2, y), cat.get("stone_wall")).unwrap();
        }
        let n = m.flood_fill(LayerKind::Terrain, (0, 0), cat.get("grass")).unwrap();
        assert_eq!(n, 10);
        assert!(m.tile(LayerKind::Terrain, (4, 4)).is_none());
    }

    #[test]
    fn multi_cell_objects_and_doors() {
        let cat = Catalog::builtin();
        let mut m = GridMap::new(6, 6, AmbientLight::Bright).unwrap();
        m.objects.push(MapObject::from_asset(ObjectId(1), cat.get("big_tree").unwrap(), 1, 1));
        for c in [(1, 1), (2, 1), (1, 2), (2, 2)] {
            assert!(m.flags(c).blocks_movement);
        }
        let mut door = MapObject::from_asset(ObjectId(2), cat.get("door").unwrap(), 4, 4);
        assert!(door.interactable);
        door.open = true;
        m.objects.push(door);
        assert!(!m.flags((4, 4)).blocks_movement && !m.flags((4, 4)).blocks_sight);
    }

    #[test]
    fn player_view_strips_hidden() {
        let cat = Catalog::builtin();
        let mut m = GridMap::new(3, 3, AmbientLight::Bright).unwrap();
        let mut secret = MapObject::from_asset(ObjectId(9), cat.get("chest").unwrap(), 0, 0);
        secret.hidden = true;
        secret.dm_notes = "200 gp".into();
        let mut shown = MapObject::from_asset(ObjectId(10), cat.get("barrel").unwrap(), 1, 1);
        shown.dm_notes = "mimic!".into();
        m.objects.extend([secret, shown]);
        let pv = m.player_view();
        assert_eq!(pv.objects.len(), 1);
        assert!(pv.objects[0].dm_notes.is_empty());
    }
}
