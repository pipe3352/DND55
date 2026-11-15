//! Procedurally generated placeholder art (original, license-free) and portrait token images.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use dnd_world::catalog::{AssetDef, Pattern};
use std::collections::HashMap;

const TILE_PX: u32 = 32;

#[derive(Resource, Default)]
pub struct TextureCache {
    pub tiles: HashMap<String, Handle<Image>>,
    /// portrait hash -> circular token image
    pub portraits: HashMap<String, Handle<Image>>,
    pub requested: std::collections::HashSet<String>,
    pub disc: Option<Handle<Image>>,
    pub ring: Option<Handle<Image>>,
}

fn new_image(w: u32, h: u32, data: Vec<u8>) -> Image {
    let mut img = Image::new_fill(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    img.data = Some(data);
    img
}

/// Cheap deterministic hash noise for texture variation.
fn noise(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x.wrapping_mul(374_761_393)
        ^ y.wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

pub fn render_asset(def: &AssetDef) -> Image {
    let (w, h) = if def.is_object {
        (TILE_PX * def.size.0 as u32, TILE_PX * def.size.1 as u32)
    } else {
        (TILE_PX, TILE_PX)
    };
    let seed = def
        .id
        .bytes()
        .fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    let [r, g, b] = def.color.map(|c| c as f32);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let n = noise(x, y, seed);
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let mut shade = 0.9 + n * 0.2;
            let mut alpha = 255u8;
            match def.pattern {
                Pattern::Flat => {}
                Pattern::Speckle => shade = 0.82 + n * 0.32,
                Pattern::Bricks => {
                    let row = y / 8;
                    let off = if row % 2 == 0 { 0 } else { 8 };
                    if y % 8 == 0 || (x + off) % 16 == 0 {
                        shade = 0.55;
                    }
                }
                Pattern::Planks => {
                    if x % 8 == 0 {
                        shade = 0.6;
                    } else {
                        shade *= 0.92 + 0.1 * ((y as f32 * 0.7 + x as f32 * 0.05).sin());
                    }
                }
                Pattern::Waves => shade = 0.8 + 0.2 * ((fx * 12.0 + (fy * 6.0).sin() * 2.0).sin()),
                Pattern::Circle | Pattern::Cross => {
                    let (dx, dy) = (fx - 0.5, fy - 0.5);
                    let d = (dx * dx + dy * dy).sqrt();
                    let inside = match def.pattern {
                        Pattern::Circle => d < 0.45,
                        _ => d < 0.47 && (dx.abs() < 0.16 || dy.abs() < 0.16 || d < 0.3),
                    };
                    if !inside {
                        alpha = 0;
                    } else {
                        shade = 1.05 - d * 0.9 + n * 0.1; // simple spherical shading
                        if d > 0.40 {
                            shade *= 0.6; // dark outline
                        }
                    }
                }
            }
            data.extend_from_slice(&[
                (r * shade).clamp(0.0, 255.0) as u8,
                (g * shade).clamp(0.0, 255.0) as u8,
                (b * shade).clamp(0.0, 255.0) as u8,
                alpha,
            ]);
        }
    }
    new_image(w, h, data)
}

/// Circular token image from portrait PNG bytes, with a gold rim.
pub fn portrait_token(png: &[u8]) -> Option<Image> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let size = 128u32;
    let sq = img
        .resize_to_fill(size, size, image::imageops::FilterType::Triangle)
        .to_rgba8();
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 - 63.5, y as f32 - 63.5);
            let d = (dx * dx + dy * dy).sqrt();
            let p = sq.get_pixel(x, y).0;
            if d > 63.5 {
                data.extend_from_slice(&[0, 0, 0, 0]);
            } else if d > 58.0 {
                data.extend_from_slice(&[201, 162, 72, 255]);
            } else {
                data.extend_from_slice(&p);
            }
        }
    }
    Some(new_image(size, size, data))
}

/// Plain white disc (tinted per creature when there's no portrait).
pub fn disc() -> Image {
    let size = 64u32;
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 - 31.5, y as f32 - 31.5);
            let d = (dx * dx + dy * dy).sqrt();
            let a = if d < 30.0 {
                255
            } else if d < 31.5 {
                128
            } else {
                0
            };
            let shade = if d > 26.0 { 150 } else { 255 };
            data.extend_from_slice(&[shade, shade, shade, a]);
        }
    }
    new_image(size, size, data)
}

/// Soft radial glow used for light sources.
pub fn ring() -> Image {
    let size = 64u32;
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (dx, dy) = (x as f32 - 31.5, y as f32 - 31.5);
            let d = ((dx * dx + dy * dy).sqrt() / 32.0).min(1.0);
            data.extend_from_slice(&[255, 255, 255, ((1.0 - d).powf(2.0) * 255.0) as u8]);
        }
    }
    new_image(size, size, data)
}

impl TextureCache {
    pub fn asset(&mut self, def: &AssetDef, images: &mut Assets<Image>) -> Handle<Image> {
        if let Some(h) = self.tiles.get(&def.id) {
            return h.clone();
        }
        let h = images.add(render_asset(def));
        self.tiles.insert(def.id.clone(), h.clone());
        h
    }
    pub fn disc(&mut self, images: &mut Assets<Image>) -> Handle<Image> {
        self.disc.get_or_insert_with(|| images.add(disc())).clone()
    }
    pub fn glow(&mut self, images: &mut Assets<Image>) -> Handle<Image> {
        self.ring.get_or_insert_with(|| images.add(ring())).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_asset_renders_with_correct_size() {
        for a in dnd_world::catalog::Catalog::builtin().assets {
            let img = render_asset(&a);
            let (w, h) = (img.width(), img.height());
            assert_eq!(
                img.data.as_ref().unwrap().len() as u32,
                w * h * 4,
                "{}",
                a.id
            );
        }
    }
}
