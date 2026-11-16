//! Animated water and lava (WGSL shader in `shaders/liquid.wgsl`, embedded in the executable).
//!
//! One quad covers the room; a tiny per-cell data texture tells the shader where shallow water,
//! deep water and lava are. Linear filtering of that texture plus noise gives organic shorelines,
//! while the grid underneath stays the authority for movement and hazards (the visuals never
//! decide what a cell is).

use bevy::asset::{RenderAssetUsages, uuid_handle};
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat,
};
use bevy::shader::{Shader, ShaderRef};
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin};

pub const LIQUID_SHADER: Handle<Shader> = uuid_handle!("6f0c2b0e-3b1d-4d8e-9a55-1c0dd55a11d0");

#[derive(ShaderType, Clone, Copy, Debug, Default)]
pub struct LiquidParams {
    /// time, quality (0..3), map width, map height
    pub a: Vec4,
    /// reduced motion, unused
    pub b: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct LiquidMaterial {
    #[uniform(0)]
    pub params: LiquidParams,
    #[texture(1)]
    #[sampler(2)]
    pub cells: Handle<Image>,
}

impl Material2d for LiquidMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(LIQUID_SHADER)
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

pub struct LiquidPlugin;

impl Plugin for LiquidPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<LiquidMaterial>::default());
        let shader = Shader::from_wgsl(include_str!("shaders/liquid.wgsl"), "dnd55/liquid.wgsl");
        let _ = app
            .world_mut()
            .resource_mut::<Assets<Shader>>()
            .insert(LIQUID_SHADER.id(), shader);
        app.add_systems(Update, animate_liquids);
    }
}

/// Kind of liquid in a cell, from its terrain asset id.
pub fn liquid_of(asset: &str) -> Option<[u8; 3]> {
    match asset {
        "water_shallow" => Some([255, 0, 0]),
        "water_deep" => Some([0, 255, 0]),
        "lava" => Some([0, 0, 255]),
        _ => None,
    }
}

/// Builds the per-cell data texture (RGBA8 linear: R shallow, G deep, B lava).
pub fn cell_texture(width: u32, height: u32, cells: &[Option<[u8; 3]>]) -> Image {
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for c in cells {
        let [r, g, b] = c.unwrap_or([0, 0, 0]);
        data.extend_from_slice(&[r, g, b, 255]);
    }
    let mut img = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    img.data = Some(data);
    img.sampler = ImageSampler::linear();
    img
}

/// Liquid quality and motion, set from the graphics settings.
#[derive(Resource, Default, Clone, Copy)]
pub struct LiquidSettings {
    pub quality: f32,
    pub reduced_motion: bool,
}

fn animate_liquids(
    time: Res<Time>,
    settings: Option<Res<LiquidSettings>>,
    mut mats: ResMut<Assets<LiquidMaterial>>,
) {
    let s = settings.map(|s| *s).unwrap_or(LiquidSettings {
        quality: 2.0,
        reduced_motion: false,
    });
    // Low quality also slows updates to every other frame's worth of motion (cheaper on old GPUs).
    let t = if s.reduced_motion {
        0.0
    } else {
        time.elapsed_secs() * if s.quality < 1.0 { 0.6 } else { 1.0 }
    };
    for (_, m) in mats.iter_mut() {
        m.params.a.x = t;
        m.params.a.y = s.quality;
        m.params.b.x = s.reduced_motion as u8 as f32;
    }
}
