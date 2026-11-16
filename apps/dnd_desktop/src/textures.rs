//! Portrait token images and the plain token disc (map art lives in `terrain_art`).

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;

#[derive(Resource, Default)]
pub struct TextureCache {
    /// portrait hash -> circular token image
    pub portraits: HashMap<String, Handle<Image>>,
    pub requested: std::collections::HashSet<String>,
    pub disc: Option<Handle<Image>>,
}

pub(crate) fn new_image(w: u32, h: u32, data: Vec<u8>) -> Image {
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

impl TextureCache {
    pub fn disc(&mut self, images: &mut Assets<Image>) -> Handle<Image> {
        self.disc.get_or_insert_with(|| images.add(disc())).clone()
    }
}
