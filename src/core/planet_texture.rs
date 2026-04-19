use macroquad::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;

const HEIGHTMAP_IMAGE_PREFIX: &str = "planet_data/planet_heightmap";

// Hashes a 3D integer coordinate to a deterministic [0,1] value.
fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut n = (x as i64) * 374761393
        + (y as i64) * 668265263
        + (z as i64) * 2147483647
        + seed as i64 * 69069;
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    let n = n ^ (n >> 16);
    (n as u32) as f32 / u32::MAX as f32
}

// Produces smooth value noise for a 3D position.
fn value_noise3(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let z0 = z.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let z1 = z0 + 1;
    let sx = x - x0 as f32;
    let sy = y - y0 as f32;
    let sz = z - z0 as f32;
    let u = sx * sx * (3.0 - 2.0 * sx);
    let v = sy * sy * (3.0 - 2.0 * sy);
    let w = sz * sz * (3.0 - 2.0 * sz);
    let n000 = hash3(x0, y0, z0, seed);
    let n100 = hash3(x1, y0, z0, seed);
    let n010 = hash3(x0, y1, z0, seed);
    let n110 = hash3(x1, y1, z0, seed);
    let n001 = hash3(x0, y0, z1, seed);
    let n101 = hash3(x1, y0, z1, seed);
    let n011 = hash3(x0, y1, z1, seed);
    let n111 = hash3(x1, y1, z1, seed);
    let nx00 = n000 + (n100 - n000) * u;
    let nx10 = n010 + (n110 - n010) * u;
    let nx01 = n001 + (n101 - n001) * u;
    let nx11 = n011 + (n111 - n011) * u;
    let nxy0 = nx00 + (nx10 - nx00) * v;
    let nxy1 = nx01 + (nx11 - nx01) * v;
    nxy0 + (nxy1 - nxy0) * w
}

// Combines multiple octaves of value noise into fBm.
fn fbm3(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    for i in 0..5 {
        sum += value_noise3(x * freq, y * freq, z * freq, seed + i) * amp;
        freq *= 2.0;
        amp *= 0.5;
    }
    sum.clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PlanetNoiseConfig {
    pub noise_scale: f32,
    pub height_amp: f32,
    pub height_bias: f32,
    pub lat_bias: f32,
    pub sea_level: f32,
    pub ice_start: f32,
    pub ice_strength: f32,
    pub subdivisions: u8,
}

impl Default for PlanetNoiseConfig {
    // Provides a baseline noise configuration for the planet.
    fn default() -> Self {
        Self {
            noise_scale: 2.0,
            height_amp: 0.8,
            height_bias: 0.1,
            lat_bias: 0.1,
            sea_level: 0.44,
            ice_start: 0.55,
            ice_strength: 1.6,
            subdivisions: 4,
        }
    }
}

// Calculates a height value for a given surface normal.
pub fn height_value(normal: Vec3, config: &PlanetNoiseConfig) -> f32 {
    let noise = fbm3(
        normal.x * config.noise_scale,
        normal.y * config.noise_scale,
        normal.z * config.noise_scale,
        1337,
    );
    (noise * config.height_amp + config.height_bias + normal.y * config.lat_bias).clamp(0.0, 1.0)
}

// Maps height and latitude to a terrain color.
pub fn color_from_height(height: f32, normal: Vec3, config: &PlanetNoiseConfig) -> Color {
    let ice = ((normal.y.abs() - config.ice_start) * config.ice_strength).clamp(0.0, 1.0);
    let (mut r, mut g, mut b) = if height < config.sea_level {
        (0.08, 0.18, 0.42)
    } else if height < config.sea_level + 0.08 {
        (0.12, 0.32, 0.24)
    } else if height < config.sea_level + 0.22 {
        (0.22, 0.44, 0.26)
    } else {
        (0.48, 0.46, 0.40)
    };
    if ice > 0.0 {
        let t = ice * ice;
        r = r * (1.0 - t) + 0.85 * t;
        g = g * (1.0 - t) + 0.9 * t;
        b = b * (1.0 - t) + 0.95 * t;
    }
    Color::new(r, g, b, 1.0)
}

// Builds an RGBA heightmap texture using equirectangular sampling.
fn build_heightmap_pixels(config: &PlanetNoiseConfig, size: u16) -> Vec<u8> {
    let size = size as usize;
    let mut pixels = vec![0u8; size * size * 4];
    for y in 0..size {
        let v = y as f32 / (size - 1) as f32;
        let phi = v * std::f32::consts::PI;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        for x in 0..size {
            let u = x as f32 / (size - 1) as f32;
            // Match uv_from_normal(): u = 0.5 + atan2(z, x) / TAU
            let theta = (u - 0.5) * std::f32::consts::TAU;
            let normal = vec3(sin_phi * theta.cos(), cos_phi, sin_phi * theta.sin());
            let height = height_value(normal, config);
            let color = color_from_height(height, normal, config);
            let idx = (y * size + x) * 4;
            pixels[idx] = (color.r * 255.0) as u8;
            pixels[idx + 1] = (color.g * 255.0) as u8;
            pixels[idx + 2] = (color.b * 255.0) as u8;
            pixels[idx + 3] = (color.a * 255.0) as u8;
        }
    }
    pixels
}

// Checks if texture-relevant noise parameters changed.
pub fn texture_params_match(a: &PlanetNoiseConfig, b: &PlanetNoiseConfig) -> bool {
    let eps = 1e-4;
    (a.noise_scale - b.noise_scale).abs() < eps
        && (a.height_amp - b.height_amp).abs() < eps
        && (a.height_bias - b.height_bias).abs() < eps
        && (a.lat_bias - b.lat_bias).abs() < eps
        && (a.sea_level - b.sea_level).abs() < eps
        && (a.ice_start - b.ice_start).abs() < eps
        && (a.ice_strength - b.ice_strength).abs() < eps
}

// Builds or loads a cached heightmap PNG for current noise parameters.
pub fn load_or_build_heightmap(config: &PlanetNoiseConfig, size: u16) -> Vec<u8> {
    if let Some(pixels) = load_heightmap_png(config, size) {
        return pixels;
    }
    let pixels = build_heightmap_pixels(config, size);
    save_heightmap_png(config, size, &pixels);
    pixels
}

fn heightmap_cache_key(config: &PlanetNoiseConfig, size: u16) -> u64 {
    // Texture generation does not depend on LOD subdivisions.
    let words = [
        config.noise_scale.to_bits() as u64,
        config.height_amp.to_bits() as u64,
        config.height_bias.to_bits() as u64,
        config.lat_bias.to_bits() as u64,
        config.sea_level.to_bits() as u64,
        config.ice_start.to_bits() as u64,
        config.ice_strength.to_bits() as u64,
        size as u64,
    ];
    let mut hash: u64 = 1469598103934665603;
    for word in words {
        hash ^= word;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

fn heightmap_cache_path(config: &PlanetNoiseConfig, size: u16) -> String {
    format!(
        "{}_{:016x}_{}x{}.png",
        HEIGHTMAP_IMAGE_PREFIX,
        heightmap_cache_key(config, size),
        size,
        size
    )
}

fn load_heightmap_png(config: &PlanetNoiseConfig, size: u16) -> Option<Vec<u8>> {
    let path = heightmap_cache_path(config, size);
    let file_bytes = fs::read(path).ok()?;
    let mut image = Image::from_file_with_format(&file_bytes, None).ok()?;
    if image.width != size || image.height != size {
        return None;
    }
    flip_rgba_vertical(&mut image.bytes, image.width as usize, image.height as usize);
    Some(image.bytes)
}

fn save_heightmap_png(config: &PlanetNoiseConfig, size: u16, pixels: &[u8]) {
    if pixels.len() != size as usize * size as usize * 4 {
        return;
    }
    let path = heightmap_cache_path(config, size);
    let image = Image {
        bytes: pixels.to_vec(),
        width: size,
        height: size,
    };
    image.export_png(&path);
}

fn flip_rgba_vertical(bytes: &mut [u8], width: usize, height: usize) {
    let stride = width * 4;
    for y in 0..(height / 2) {
        let top = y * stride;
        let bottom = (height - 1 - y) * stride;
        for x in 0..stride {
            bytes.swap(top + x, bottom + x);
        }
    }
}
