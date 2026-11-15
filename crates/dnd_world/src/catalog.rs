//! Asset catalog: the library of tiles and objects the map editor can place.
//!
//! The built-in entries are **original placeholder art generated procedurally by the client**
//! (flat colors + simple patterns), so the project ships no third-party art. Imported PNG/spritesheet
//! packs register additional entries with the same structure (see `docs/DESIGN.md` §8).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Terrain,
    Vegetation,
    Architecture,
    Furniture,
    Natural,
    Dungeon,
    Fantasy,
}

/// Procedural placeholder look; a real sprite path replaces it when an asset pack provides one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pattern {
    Flat,
    Speckle,
    Bricks,
    Planks,
    Waves,
    Circle,
    Cross,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightEmission {
    pub bright_ft: u32,
    pub dim_ft: u32,
    pub color: [u8; 3],
    pub flicker: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetDef {
    pub id: String,
    /// Localization key for the display name (`asset.<id>`); `name` is the English fallback.
    pub name: String,
    pub category: Category,
    pub color: [u8; 3],
    pub pattern: Pattern,
    /// Tiles cover one cell; objects can span several cells.
    pub is_object: bool,
    pub size: (u8, u8),
    pub blocks_movement: bool,
    pub blocks_sight: bool,
    pub difficult: bool,
    pub light: Option<LightEmission>,
    /// Optional sprite path inside an asset pack (None = procedural placeholder).
    pub sprite: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub assets: Vec<AssetDef>,
}

impl Catalog {
    pub fn get(&self, id: &str) -> Option<&AssetDef> {
        self.assets.iter().find(|a| a.id == id)
    }

    pub fn search<'a>(
        &'a self,
        text: &str,
        category: Option<Category>,
    ) -> impl Iterator<Item = &'a AssetDef> + 'a {
        let text = text.to_lowercase();
        self.assets.iter().filter(move |a| {
            category.is_none_or(|c| a.category == c)
                && (text.is_empty()
                    || a.name.to_lowercase().contains(&text)
                    || a.tags.iter().any(|t| t.contains(&text)))
        })
    }

    /// Built-in placeholder library.
    pub fn builtin() -> Self {
        use Category::*;
        use Pattern::*;
        let mut v = Vec::new();
        let mut tile =
            |id: &str, name: &str, cat, color, pat, mv: bool, sight: bool, diff: bool| {
                v.push(AssetDef {
                    id: id.into(),
                    name: name.into(),
                    category: cat,
                    color,
                    pattern: pat,
                    is_object: false,
                    size: (1, 1),
                    blocks_movement: mv,
                    blocks_sight: sight,
                    difficult: diff,
                    light: None,
                    sprite: None,
                    tags: vec![],
                });
            };
        tile(
            "grass",
            "Grass",
            Terrain,
            [86, 125, 70],
            Speckle,
            false,
            false,
            false,
        );
        tile(
            "dirt",
            "Dirt",
            Terrain,
            [121, 92, 62],
            Speckle,
            false,
            false,
            false,
        );
        tile(
            "sand",
            "Sand",
            Terrain,
            [196, 176, 128],
            Speckle,
            false,
            false,
            false,
        );
        tile(
            "snow",
            "Snow",
            Terrain,
            [228, 232, 238],
            Speckle,
            false,
            false,
            true,
        );
        tile(
            "ice",
            "Ice",
            Terrain,
            [176, 214, 230],
            Flat,
            false,
            false,
            true,
        );
        tile(
            "stone_floor",
            "Stone Floor",
            Terrain,
            [110, 108, 104],
            Bricks,
            false,
            false,
            false,
        );
        tile(
            "cave_floor",
            "Cave Floor",
            Terrain,
            [78, 72, 68],
            Speckle,
            false,
            false,
            false,
        );
        tile(
            "wood_floor",
            "Wooden Floor",
            Terrain,
            [133, 96, 60],
            Planks,
            false,
            false,
            false,
        );
        tile(
            "gravel",
            "Gravel",
            Terrain,
            [140, 136, 128],
            Speckle,
            false,
            false,
            false,
        );
        tile(
            "mud",
            "Mud",
            Terrain,
            [92, 70, 48],
            Speckle,
            false,
            false,
            true,
        );
        tile(
            "rubble",
            "Rubble",
            Terrain,
            [102, 96, 90],
            Cross,
            false,
            false,
            true,
        );
        tile(
            "road",
            "Road",
            Terrain,
            [150, 130, 100],
            Bricks,
            false,
            false,
            false,
        );
        tile(
            "water_shallow",
            "Shallow Water",
            Terrain,
            [70, 120, 160],
            Waves,
            false,
            false,
            true,
        );
        tile(
            "water_deep",
            "Deep Water",
            Terrain,
            [36, 70, 120],
            Waves,
            true,
            false,
            false,
        );
        tile(
            "lava",
            "Lava",
            Terrain,
            [210, 80, 20],
            Waves,
            true,
            false,
            false,
        );
        tile(
            "stone_wall",
            "Stone Wall",
            Architecture,
            [62, 60, 58],
            Bricks,
            true,
            true,
            false,
        );
        tile(
            "wood_wall",
            "Wooden Wall",
            Architecture,
            [90, 64, 40],
            Planks,
            true,
            true,
            false,
        );
        tile(
            "undergrowth",
            "Undergrowth",
            Vegetation,
            [60, 100, 50],
            Cross,
            false,
            false,
            true,
        );

        let mut obj = |id: &str,
                       name: &str,
                       cat,
                       color,
                       pat,
                       size,
                       mv: bool,
                       sight: bool,
                       diff: bool,
                       light: Option<LightEmission>| {
            v.push(AssetDef {
                id: id.into(),
                name: name.into(),
                category: cat,
                color,
                pattern: pat,
                is_object: true,
                size,
                blocks_movement: mv,
                blocks_sight: sight,
                difficult: diff,
                light,
                sprite: None,
                tags: vec![],
            });
        };
        let torch = Some(LightEmission {
            bright_ft: 20,
            dim_ft: 20,
            color: [255, 170, 80],
            flicker: true,
        });
        obj(
            "tree",
            "Tree",
            Vegetation,
            [40, 90, 40],
            Circle,
            (1, 1),
            true,
            true,
            false,
            None,
        );
        obj(
            "big_tree",
            "Large Tree",
            Vegetation,
            [34, 80, 36],
            Circle,
            (2, 2),
            true,
            true,
            false,
            None,
        );
        obj(
            "bush",
            "Bush",
            Vegetation,
            [70, 120, 60],
            Circle,
            (1, 1),
            false,
            false,
            true,
            None,
        );
        obj(
            "boulder",
            "Boulder",
            Natural,
            [120, 116, 110],
            Circle,
            (1, 1),
            true,
            false,
            false,
            None,
        );
        obj(
            "crystal",
            "Glowing Crystal",
            Fantasy,
            [120, 200, 255],
            Cross,
            (1, 1),
            true,
            false,
            false,
            Some(LightEmission {
                bright_ft: 10,
                dim_ft: 10,
                color: [140, 200, 255],
                flicker: false,
            }),
        );
        obj(
            "table",
            "Table",
            Furniture,
            [120, 84, 50],
            Planks,
            (2, 1),
            false,
            false,
            true,
            None,
        );
        obj(
            "chair",
            "Chair",
            Furniture,
            [110, 80, 50],
            Planks,
            (1, 1),
            false,
            false,
            true,
            None,
        );
        obj(
            "bed",
            "Bed",
            Furniture,
            [150, 60, 60],
            Planks,
            (1, 2),
            false,
            false,
            true,
            None,
        );
        obj(
            "bookshelf",
            "Bookshelf",
            Furniture,
            [96, 66, 40],
            Planks,
            (2, 1),
            true,
            true,
            false,
            None,
        );
        obj(
            "barrel",
            "Barrel",
            Furniture,
            [128, 90, 52],
            Circle,
            (1, 1),
            true,
            false,
            false,
            None,
        );
        obj(
            "crate",
            "Crate",
            Furniture,
            [150, 112, 70],
            Cross,
            (1, 1),
            true,
            false,
            false,
            None,
        );
        obj(
            "chest",
            "Chest",
            Dungeon,
            [160, 120, 40],
            Planks,
            (1, 1),
            true,
            false,
            false,
            None,
        );
        obj(
            "door",
            "Door",
            Architecture,
            [120, 80, 40],
            Planks,
            (1, 1),
            true,
            true,
            false,
            None,
        );
        obj(
            "torch",
            "Wall Torch",
            Dungeon,
            [240, 160, 60],
            Circle,
            (1, 1),
            false,
            false,
            false,
            torch.clone(),
        );
        obj(
            "brazier",
            "Brazier",
            Dungeon,
            [200, 100, 40],
            Circle,
            (1, 1),
            true,
            false,
            false,
            Some(LightEmission {
                bright_ft: 20,
                dim_ft: 20,
                color: [255, 150, 60],
                flicker: true,
            }),
        );
        obj(
            "altar",
            "Altar",
            Dungeon,
            [180, 180, 190],
            Cross,
            (2, 1),
            true,
            false,
            false,
            None,
        );
        obj(
            "pillar",
            "Pillar",
            Architecture,
            [130, 128, 124],
            Circle,
            (1, 1),
            true,
            true,
            false,
            None,
        );
        obj(
            "lever",
            "Lever",
            Dungeon,
            [150, 150, 150],
            Cross,
            (1, 1),
            false,
            false,
            false,
            None,
        );
        obj(
            "pressure_plate",
            "Pressure Plate",
            Dungeon,
            [100, 100, 96],
            Flat,
            (1, 1),
            false,
            false,
            false,
            None,
        );
        obj(
            "rune_circle",
            "Summoning Circle",
            Fantasy,
            [150, 90, 200],
            Circle,
            (3, 3),
            false,
            false,
            false,
            Some(LightEmission {
                bright_ft: 0,
                dim_ft: 10,
                color: [180, 120, 255],
                flicker: false,
            }),
        );
        obj(
            "bones",
            "Bones",
            Natural,
            [220, 214, 196],
            Cross,
            (1, 1),
            false,
            false,
            false,
            None,
        );
        obj(
            "stalagmite",
            "Stalagmite",
            Natural,
            [96, 90, 84],
            Circle,
            (1, 1),
            true,
            false,
            false,
            None,
        );
        Catalog { assets: v }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_ids_are_unique_and_searchable() {
        let c = Catalog::builtin();
        let mut ids: Vec<_> = c.assets.iter().map(|a| a.id.clone()).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(n, ids.len());
        assert!(c.search("tree", None).count() >= 2);
        assert!(
            c.search("", Some(Category::Furniture))
                .all(|a| a.category == Category::Furniture)
        );
    }
}
