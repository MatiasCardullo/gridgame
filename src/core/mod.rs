use std::collections::HashMap;
use std::fs;

use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ui::{AppColors, AppConfig, RuntimeColors};
use crate::{Axial, Block, BlockType, PlacedBlock, Tile, TileType, SQRT_3};

#[derive(Serialize, Deserialize)]
struct MapData {
    #[serde(default)]
    tiles: Vec<Tile>,
    blocks: Vec<Block>,
}

#[derive(Serialize, Deserialize)]
pub struct ConfigData {
    #[serde(default)]
    pub config: AppConfig,
    #[serde(default)]
    pub colors: AppColors,
}

impl Default for ConfigData {
    fn default() -> Self {
        Self {
            config: AppConfig::default(),
            colors: AppColors::default(),
        }
    }
}

// Convert axial hex coords to pixel space.
pub fn hex_to_pixel(hex: Axial, size: f32, origin: Vec2) -> Vec2 {
    let q = hex.q as f32;
    let r = hex.r as f32;
    let x = size * (SQRT_3 * q + (SQRT_3 / 2.0) * r);
    let y = size * (1.5 * r);
    origin + vec2(x, y)
}

// Convert pixel position to axial hex coords.
pub fn pixel_to_hex(pos: Vec2, size: f32, origin: Vec2) -> Axial {
    let p = pos - origin;
    let q = (SQRT_3 / 3.0 * p.x - 1.0 / 3.0 * p.y) / size;
    let r = (2.0 / 3.0 * p.y) / size;
    axial_round(q, r)
}

// Round fractional axial coords to the nearest hex.
pub fn axial_round(q: f32, r: f32) -> Axial {
    let mut x = q.round();
    let mut z = r.round();
    let mut y = (-q - r).round();

    let x_diff = (x - q).abs();
    let y_diff = (y + q + r).abs();
    let z_diff = (z - r).abs();

    if x_diff > y_diff && x_diff > z_diff {
        x = -y - z;
    } else if y_diff > z_diff {
        y = -x - z;
    } else {
        z = -x - y;
    }

    Axial {
        q: x as i32,
        r: z as i32,
    }
}

// Get the 6 corner points of a hex.
pub fn hex_corners(center: Vec2, size: f32) -> [Vec2; 6] {
    let mut points = [Vec2::ZERO; 6];
    for i in 0..6 {
        let angle = (60.0 * i as f32 - 30.0).to_radians();
        points[i] = center + vec2(size * angle.cos(), size * angle.sin());
    }
    points
}

// Draw a hex outline.
pub fn draw_hex_outline(center: Vec2, size: f32, color: Color, thickness: f32) {
    let points = hex_corners(center, size);
    for i in 0..6 {
        let a = points[i];
        let b = points[(i + 1) % 6];
        draw_line(a.x, a.y, b.x, b.y, thickness, color);
    }
}

// Draw a filled hex.
pub fn draw_hex_filled(center: Vec2, size: f32, color: Color) {
    let points = hex_corners(center, size);
    for i in 0..6 {
        let a = points[i];
        let b = points[(i + 1) % 6];
        draw_triangle(center, a, b, color);
    }
}

// Hex distance in axial coordinates.
pub fn hex_distance(a: Axial, b: Axial) -> i32 {
    let dq = (a.q - b.q).abs();
    let dr = (a.r - b.r).abs();
    let ds = (a.q + a.r - b.q - b.r).abs();
    (dq + dr + ds) / 2
}

// Save blocks/tiles to a map JSON file.
pub fn save_map(path: &str, blocks: &HashMap<Axial, PlacedBlock>, tiles: &HashMap<Axial, TileType>) {
    let data = MapData {
        tiles: tiles
            .iter()
            .map(|(hex, kind)| Tile {
                hex: *hex,
                kind: *kind,
            })
            .collect(),
        blocks: blocks
            .iter()
            .map(|(hex, placed)| Block {
                hex: *hex,
                kind: placed.kind,
                rotation: placed.rotation,
            })
            .collect(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&data) {
        let _ = fs::write(path, json);
    }
}

// Save config JSON.
pub fn save_config(path: &str, data: &ConfigData) {
    if let Ok(json) = serde_json::to_string_pretty(data) {
        let _ = fs::write(path, json);
    }
}

// Load config JSON or return defaults.
pub fn load_config(path: &str) -> ConfigData {
    if let Ok(json) = fs::read_to_string(path) {
        if let Ok(data) = serde_json::from_str::<ConfigData>(&json) {
            return data;
        }
    }
    ConfigData::default()
}

// Load blocks/tiles from map JSON.
pub fn load_map(path: &str) -> (HashMap<Axial, PlacedBlock>, HashMap<Axial, TileType>) {
    if let Ok(json) = fs::read_to_string(path) {
        if let Ok(data) = serde_json::from_str::<MapData>(&json) {
            let blocks = data
                .blocks
                .into_iter()
                .map(|b| {
                    (
                        b.hex,
                        PlacedBlock {
                            kind: b.kind,
                            rotation: b.rotation % 6,
                        },
                    )
                })
                .collect();
            let tiles = data
                .tiles
                .into_iter()
                .map(|t| (t.hex, t.kind))
                .collect();
            return (blocks, tiles);
        }
    }
    (HashMap::new(), HashMap::new())
}

// Resolve block color from runtime palette.
pub fn block_color(kind: BlockType, colors: &RuntimeColors) -> Color {
    match kind {
        BlockType::Vivienda => colors.block_vivienda,
        BlockType::Fabrica => colors.block_fabrica,
        BlockType::Mina => colors.block_mina,
        BlockType::Almacen => colors.block_almacen,
        BlockType::Logistica => colors.block_logistica,
        BlockType::Ruta => colors.block_ruta,
    }
}

// Resolve tile color from runtime palette.
pub fn tile_color(kind: TileType, colors: &RuntimeColors) -> Color {
    match kind {
        TileType::Piedra => colors.tile_piedra,
        TileType::Hierro => colors.tile_hierro,
        TileType::Cobre => colors.tile_cobre,
        TileType::Agua => colors.tile_agua,
    }
}

// Pick a tile type using weighted probabilities and center bias.
pub fn tile_kind_weighted(dist: i32, radius: i32, config: &AppConfig) -> TileType {
    let center_bias = 1.0 - (dist as f32 / radius as f32).clamp(0.0, 1.0);
    let mut w_p = config.weight_piedra.max(0.0);
    let mut w_h = config.weight_hierro.max(0.0);
    let mut w_c = config.weight_cobre.max(0.0);
    let mut w_a = config.weight_agua.max(0.0);
    let bonus = center_bias * config.tile_center_bonus.max(0.0);
    w_h += bonus * 0.6;
    w_c += bonus * 0.4;
    let total = (w_p + w_h + w_c + w_a).max(0.001);
    let roll = rand::gen_range(0.0, total);
    if roll < w_h {
        TileType::Hierro
    } else if roll < w_h + w_c {
        TileType::Cobre
    } else if roll < w_h + w_c + w_p {
        TileType::Piedra
    } else {
        TileType::Agua
    }
}

// Generate clustered tiles with a center-weighted distribution.
pub fn generate_tiles(radius: i32, config: &AppConfig) -> HashMap<Axial, TileType> {
    let mut tiles = HashMap::new();
    let max_clusters = config.tile_clusters.max(1);
    for _ in 0..max_clusters {
        let mut center = Axial { q: 0, r: 0 };
        for _ in 0..60 {
            let q = rand::gen_range(-radius, radius + 1);
            let r = rand::gen_range(-radius, radius + 1);
            let hex = Axial { q, r };
            if hex_distance(hex, Axial { q: 0, r: 0 }) <= radius {
                let dist = hex_distance(hex, Axial { q: 0, r: 0 });
                let weight = 1.0 - (dist as f32 / radius as f32).clamp(0.0, 1.0);
                let accept = rand::gen_range(0.0, 1.0) < (0.15 + weight * weight);
                if accept {
                    center = hex;
                    break;
                }
                center = hex;
            }
        }
        let dist = hex_distance(center, Axial { q: 0, r: 0 });
        let kind = tile_kind_weighted(dist, radius, config);
        let size_min = config.tile_cluster_min.max(1);
        let size_max = config.tile_cluster_max.max(size_min);
        let size = rand::gen_range(size_min as i32, (size_max + 1) as i32) as usize;
        let mut frontier = vec![center];
        let mut placed = 0;
        while let Some(current) = frontier.pop() {
            if placed >= size {
                break;
            }
            if hex_distance(current, Axial { q: 0, r: 0 }) > radius {
                continue;
            }
            if tiles.contains_key(&current) {
                continue;
            }
            tiles.insert(current, kind);
            placed += 1;
            let neighbors = axial_neighbors(current);
            for neighbor in neighbors {
                if rand::gen_range(0.0, 1.0) < config.tile_neighbor_chance {
                    frontier.push(neighbor);
                }
            }
        }
    }
    tiles
}

// Rotate a direction index (0-5) by a rotation step.
pub fn rotate_dir(dir: i32, rotation: u8) -> i32 {
    (dir + rotation as i32).rem_euclid(6)
}

// Return input/output port directions for a block.
pub fn block_ports(kind: BlockType, rotation: u8) -> (Vec<i32>, Vec<i32>) {
    let (inputs, outputs) = match kind {
        BlockType::Vivienda => (vec![0, 2, 4], vec![1, 3, 5]),
        BlockType::Fabrica => (vec![0, 1, 2], vec![3, 4, 5]),
        BlockType::Mina => (vec![], vec![0, 1, 2, 3, 4, 5]),
        BlockType::Almacen => (vec![0, 3], vec![0, 3]),
        BlockType::Logistica => (vec![0, 3], vec![0, 3]),
        BlockType::Ruta => (vec![], vec![]),
    };
    let inputs = inputs
        .into_iter()
        .map(|d| rotate_dir(d, rotation))
        .collect();
    let outputs = outputs
        .into_iter()
        .map(|d| rotate_dir(d, rotation))
        .collect();
    (inputs, outputs)
}

// Get the midpoint on a hex edge for a direction.
pub fn edge_point(center: Vec2, size: f32, dir: i32) -> Vec2 {
    let angle = (60.0 * dir as f32).to_radians();
    center + vec2(size * angle.cos(), size * angle.sin())
}

// Draw an input/output marker on a hex edge.
pub fn draw_port_marker(center: Vec2, size: f32, dir: i32, color: Color) {
    let mid = edge_point(center, size, dir);
    let angle = (60.0 * dir as f32).to_radians();
    let forward = vec2(angle.cos(), angle.sin());
    let right = vec2(-forward.y, forward.x);
    let tip = mid + forward * (size * 0.12);
    let left = mid - forward * (size * 0.08) + right * (size * 0.08);
    let right_pt = mid - forward * (size * 0.08) - right * (size * 0.08);
    draw_triangle(tip, left, right_pt, color);
}

// Get axial neighbor hexes in 6 directions.
pub fn axial_neighbors(hex: Axial) -> [Axial; 6] {
    let dirs = [
        Axial { q: 1, r: 0 },
        Axial { q: 1, r: -1 },
        Axial { q: 0, r: -1 },
        Axial { q: -1, r: 0 },
        Axial { q: -1, r: 1 },
        Axial { q: 0, r: 1 },
    ];
    [
        Axial { q: hex.q + dirs[0].q, r: hex.r + dirs[0].r },
        Axial { q: hex.q + dirs[1].q, r: hex.r + dirs[1].r },
        Axial { q: hex.q + dirs[2].q, r: hex.r + dirs[2].r },
        Axial { q: hex.q + dirs[3].q, r: hex.r + dirs[3].r },
        Axial { q: hex.q + dirs[4].q, r: hex.r + dirs[4].r },
        Axial { q: hex.q + dirs[5].q, r: hex.r + dirs[5].r },
    ]
}
