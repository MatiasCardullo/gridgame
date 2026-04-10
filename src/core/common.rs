use std::collections::HashMap;
use std::fs;

use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

use crate::SQRT_3;
use crate::core::ui::UiButtonColors;

// Axial hex coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Axial {
    pub q: i32,
    pub r: i32,
}

// Available building types for planet sector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildBlockType {
    Base,
    Builder,
    Housing,
    Factory,
    Mine,
    Warehouse,
    Logistics,
    Route,
}

// Machine building types for the build template scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MachineBlockType {
    ConveyorBelt,
    #[serde(alias = "Inserter")]
    MechanicArm,
    Chest,
    Assembler,
    Furnace,
    Pallet,
    Crate,
}



// Available terrain/resource types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TileType {
    Iron,
    Copper,
    Gold,
    Zinc,
    Lead,
    Aluminum,
    Water,
}

// Item types that can be stored and transported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    Gangue,
    Iron,
    Copper,
    Gold,
    Zinc,
    Lead,
    Aluminum,
    Water,
}

// Persisted block entry for map JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub hex: Axial,
    pub kind: BuildBlockType,
    #[serde(default)]
    pub rotation: u8,
    #[serde(default)]
    pub stored: Vec<ItemStack>,
    #[serde(default)]
    pub mine_progress: f32,
    #[serde(default)]
    pub capacity: i32,
    #[serde(default)]
    pub build_progress: f32,
    #[serde(default)]
    pub build_time: f32,
    #[serde(default)]
    pub build_paid: bool,
    #[serde(default)]
    pub build_claimed: bool,
    #[serde(default)]
    pub builder_units_desired: i32,
    #[serde(default)]
    pub builder_units_created: i32,
    #[serde(default)]
    pub mine_extra: Vec<Axial>,
}

// Persisted tile entry for map JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile {
    pub hex: Axial,
    pub kind: TileType,
    #[serde(default)]
    pub amount: i32,
}

// Runtime tile data with remaining amount.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileData {
    pub kind: TileType,
    pub amount: i32,
}

// Stored item stack for blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    pub kind: ItemType,
    pub amount: i32,
}

// In-memory placed block data.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedBlock {
    pub kind: BuildBlockType,
    pub rotation: u8,
    pub mine_progress: f32,
    pub capacity: i32,
    pub stored: Vec<ItemStack>,
    pub build_progress: f32,
    pub build_time: f32,
    pub build_paid: bool,
    pub build_claimed: bool,
    pub builder_units_desired: i32,
    pub builder_units_created: i32,
    pub mine_extra: Vec<Axial>,
}

// Template block for the build template scene (factory/production building).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MachineBlock {
    pub kind: MachineBlockType,
    pub rotation: u8,
}

// Simple unit moving along a hex path.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unit {
    pub path: Vec<Axial>,
    pub index: usize,
    pub progress: f32,
    pub speed: f32,
    pub forward: bool,
    pub capacity: i32,
    pub cargo: Vec<ItemStack>,
    pub depot: Axial,
    pub station_in: Axial,
    pub station_out: Axial,
    #[serde(default)]
    pub auto_supply: bool,
    #[serde(default)]
    pub supply_types: Vec<ItemType>,
    #[serde(default)]
    pub auto_supply_role: AutoSupplyRole,
    #[serde(default)]
    pub supply_reqs: Vec<ItemStack>,
    #[serde(default)]
    pub waiting_for_supply: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum AutoSupplyRole {
    #[default]
    Base,
    Builder,
}

// Scene routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    MainMenu,
    Config,
    Planet,
    BuildTemplate,
}

// UI/frame context passed into scenes.
#[derive(Clone, Copy, Debug)]
pub struct FrameContext {
    pub mouse: Vec2,
    pub screen_center: Vec2,
    pub font_sm: f32,
    pub font_md: f32,
    pub font_lg: f32,
    pub font_title: f32,
    pub button_colors: UiButtonColors,
    pub colors_rt: RuntimeColors,
}

// Config values persisted to config.json.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub zoom_speed: f32,
    pub zoom_level: f32,
    pub text_scale: f32,
    pub tile_clusters: usize,
    pub tile_cluster_min: usize,
    pub tile_cluster_max: usize,
    pub tile_neighbor_chance: f32,
    pub tile_center_bonus: f32,
    pub weight_iron: f32,
    pub weight_copper: f32,
    pub weight_gold: f32,
    pub weight_zinc: f32,
    pub weight_lead: f32,
    pub weight_aluminum: f32,
    pub weight_water: f32,
    pub line_thickness: f32,
    pub arrow_scale: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            zoom_speed: 0.1,
            zoom_level: 1.0,
            text_scale: 1.0,
            tile_clusters: 80,
            tile_cluster_min: 8,
            tile_cluster_max: 20,
            tile_neighbor_chance: 0.45,
            tile_center_bonus: 0.35,
            weight_iron: 0.2,
            weight_copper: 0.2,
            weight_gold: 0.15,
            weight_zinc: 0.15,
            weight_lead: 0.1,
            weight_aluminum: 0.1,
            weight_water: 0.25,
            line_thickness: 1.0,
            arrow_scale: 1.0,
        }
    }
}

// Serialized RGBA color.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ColorRgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorRgba {
    // Convert RGBA bytes to macroquad Color.
    pub fn to_color(self) -> Color {
        Color::from_rgba(self.r, self.g, self.b, self.a)
    }
}

// Runtime palette (macroquad Colors).
#[derive(Clone, Copy, Debug)]
pub struct RuntimeColors {
    pub background: Color,
    pub grid: Color,
    pub hover: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub button_base: Color,
    pub button_hover: Color,
    pub button_border: Color,
    pub button_text: Color,
    pub panel_bg: Color,
    pub panel_border: Color,
    pub tooltip_bg: Color,
    pub tooltip_border: Color,
    pub route_line: Color,
    pub port_in: Color,
    pub port_out: Color,
    pub block_housing: Color,
    pub block_factory: Color,
    pub block_base: Color,
    pub block_builder: Color,
    pub block_mine: Color,
    pub block_warehouse: Color,
    pub block_logistics: Color,
    pub block_route: Color,
    pub tile_iron: Color,
    pub tile_copper: Color,
    pub tile_gold: Color,
    pub tile_zinc: Color,
    pub tile_lead: Color,
    pub tile_aluminum: Color,
    pub tile_water: Color,
}

// Configurable palette stored in config.json.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppColors {
    pub background: ColorRgba,
    pub grid: ColorRgba,
    pub hover: ColorRgba,
    pub text_primary: ColorRgba,
    pub text_secondary: ColorRgba,
    pub button_base: ColorRgba,
    pub button_hover: ColorRgba,
    pub button_border: ColorRgba,
    pub button_text: ColorRgba,
    pub panel_bg: ColorRgba,
    pub panel_border: ColorRgba,
    pub tooltip_bg: ColorRgba,
    pub tooltip_border: ColorRgba,
    pub route_line: ColorRgba,
    pub port_in: ColorRgba,
    pub port_out: ColorRgba,
    pub block_housing: ColorRgba,
    pub block_factory: ColorRgba,
    pub block_base: ColorRgba,
    pub block_builder: ColorRgba,
    pub block_mine: ColorRgba,
    pub block_warehouse: ColorRgba,
    pub block_logistics: ColorRgba,
    pub block_route: ColorRgba,
    pub tile_iron: ColorRgba,
    pub tile_copper: ColorRgba,
    pub tile_gold: ColorRgba,
    pub tile_zinc: ColorRgba,
    pub tile_lead: ColorRgba,
    pub tile_aluminum: ColorRgba,
    pub tile_water: ColorRgba,
}

impl Default for AppColors {
    fn default() -> Self {
        Self {
            background: ColorRgba { r: 18, g: 22, b: 26, a: 255 },
            grid: ColorRgba { r: 70, g: 78, b: 86, a: 255 },
            hover: ColorRgba { r: 255, g: 210, b: 90, a: 255 },
            text_primary: ColorRgba { r: 220, g: 220, b: 220, a: 255 },
            text_secondary: ColorRgba { r: 190, g: 200, b: 210, a: 255 },
            button_base: ColorRgba { r: 50, g: 62, b: 75, a: 255 },
            button_hover: ColorRgba { r: 70, g: 85, b: 100, a: 255 },
            button_border: ColorRgba { r: 110, g: 130, b: 150, a: 255 },
            button_text: ColorRgba { r: 220, g: 230, b: 240, a: 255 },
            panel_bg: ColorRgba { r: 28, g: 34, b: 40, a: 230 },
            panel_border: ColorRgba { r: 80, g: 90, b: 100, a: 255 },
            tooltip_bg: ColorRgba { r: 30, g: 36, b: 44, a: 240 },
            tooltip_border: ColorRgba { r: 100, g: 120, b: 140, a: 255 },
            route_line: ColorRgba { r: 120, g: 140, b: 160, a: 200 },
            port_in: ColorRgba { r: 80, g: 160, b: 220, a: 255 },
            port_out: ColorRgba { r: 220, g: 170, b: 90, a: 255 },
            block_housing: ColorRgba { r: 120, g: 200, b: 120, a: 255 },
            block_factory: ColorRgba { r: 220, g: 140, b: 80, a: 255 },
            block_base: ColorRgba { r: 150, g: 170, b: 220, a: 255 },
            block_builder: ColorRgba { r: 130, g: 200, b: 210, a: 255 },
            block_mine: ColorRgba { r: 110, g: 150, b: 200, a: 255 },
            block_warehouse: ColorRgba { r: 210, g: 190, b: 90, a: 255 },
            block_logistics: ColorRgba { r: 120, g: 140, b: 160, a: 255 },
            block_route: ColorRgba { r: 90, g: 100, b: 115, a: 255 },
            tile_iron: ColorRgba { r: 120, g: 95, b: 85, a: 255 },
            tile_copper: ColorRgba { r: 150, g: 95, b: 70, a: 255 },
            tile_gold: ColorRgba { r: 190, g: 150, b: 70, a: 255 },
            tile_zinc: ColorRgba { r: 135, g: 140, b: 150, a: 255 },
            tile_lead: ColorRgba { r: 95, g: 95, b: 115, a: 255 },
            tile_aluminum: ColorRgba { r: 175, g: 185, b: 200, a: 255 },
            tile_water: ColorRgba { r: 60, g: 110, b: 160, a: 255 },
        }
    }
}

impl AppColors {
    // Convert stored colors into runtime macroquad Colors.
    pub fn runtime(&self) -> RuntimeColors {
        RuntimeColors {
            background: self.background.to_color(),
            grid: self.grid.to_color(),
            hover: self.hover.to_color(),
            text_primary: self.text_primary.to_color(),
            text_secondary: self.text_secondary.to_color(),
            button_base: self.button_base.to_color(),
            button_hover: self.button_hover.to_color(),
            button_border: self.button_border.to_color(),
            button_text: self.button_text.to_color(),
            panel_bg: self.panel_bg.to_color(),
            panel_border: self.panel_border.to_color(),
            tooltip_bg: self.tooltip_bg.to_color(),
            tooltip_border: self.tooltip_border.to_color(),
            route_line: self.route_line.to_color(),
            port_in: self.port_in.to_color(),
            port_out: self.port_out.to_color(),
            block_housing: self.block_housing.to_color(),
            block_factory: self.block_factory.to_color(),
            block_base: self.block_base.to_color(),
            block_builder: self.block_builder.to_color(),
            block_mine: self.block_mine.to_color(),
            block_warehouse: self.block_warehouse.to_color(),
            block_logistics: self.block_logistics.to_color(),
            block_route: self.block_route.to_color(),
            tile_iron: self.tile_iron.to_color(),
            tile_copper: self.tile_copper.to_color(),
            tile_gold: self.tile_gold.to_color(),
            tile_zinc: self.tile_zinc.to_color(),
            tile_lead: self.tile_lead.to_color(),
            tile_aluminum: self.tile_aluminum.to_color(),
            tile_water: self.tile_water.to_color(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct MapData {
    #[serde(default)]
    tiles: Vec<Tile>,
    blocks: Vec<Block>,
    #[serde(default)]
    units: Vec<Unit>,
}

#[derive(Serialize, Deserialize)]
#[derive(Default)]
pub struct ConfigData {
    #[serde(default)]
    pub config: AppConfig,
    #[serde(default)]
    pub colors: AppColors,
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
    let _ = y;

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
pub fn save_map(
    path: &str,
    blocks: &HashMap<Axial, PlacedBlock>,
    tiles: &HashMap<Axial, TileData>,
    units: &[Unit],
) {
    let data = MapData {
        tiles: tiles
            .iter()
            .map(|(hex, data)| Tile {
                hex: *hex,
                kind: data.kind,
                amount: data.amount,
            })
            .collect(),
        blocks: blocks
            .iter()
            .map(|(hex, placed)| Block {
                hex: *hex,
                kind: placed.kind,
                rotation: placed.rotation,
                stored: placed.stored.clone(),
                mine_progress: placed.mine_progress,
                capacity: placed.capacity,
                build_progress: placed.build_progress,
                build_time: placed.build_time,
                build_paid: placed.build_paid,
                build_claimed: placed.build_claimed,
                builder_units_desired: placed.builder_units_desired,
                builder_units_created: placed.builder_units_created,
                mine_extra: placed.mine_extra.clone(),
            })
            .collect(),
        units: units.to_vec(),
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
pub fn load_map(path: &str) -> (
    HashMap<Axial, PlacedBlock>,
    HashMap<Axial, TileData>,
    Vec<Unit>,
) {
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
                            mine_progress: b.mine_progress,
                            capacity: if b.capacity == 0 {
                                default_capacity(b.kind)
                            } else {
                                b.capacity
                            },
                              stored: b.stored,
                              build_progress: b.build_progress,
                              build_time: b.build_time,
                              build_paid: b.build_paid,
                              build_claimed: b.build_claimed,
                              builder_units_desired: b.builder_units_desired,
                              builder_units_created: b.builder_units_created,
                              mine_extra: b.mine_extra,
                          },
                      )
                  })
                .collect();
            let tiles = data
                .tiles
                .into_iter()
                .map(|t| {
                    (
                        t.hex,
                        TileData {
                            kind: t.kind,
                            amount: t.amount,
                        },
                    )
                })
                .collect();
            return (blocks, tiles, data.units);
        }
    }
    (HashMap::new(), HashMap::new(), Vec::new())
}

// Resolve block color from runtime palette.
pub fn block_color(kind: BuildBlockType, colors: &RuntimeColors) -> Color {
    match kind {
        BuildBlockType::Base => colors.block_base,
        BuildBlockType::Builder => colors.block_builder,
        BuildBlockType::Housing => colors.block_housing,
        BuildBlockType::Factory => colors.block_factory,
        BuildBlockType::Mine => colors.block_mine,
        BuildBlockType::Warehouse => colors.block_warehouse,
        BuildBlockType::Logistics => colors.block_logistics,
        BuildBlockType::Route => colors.block_route,
    }
}

// Resolve tile color from runtime palette.
pub fn tile_color(kind: TileType, colors: &RuntimeColors) -> Color {
    match kind {
        TileType::Iron => colors.tile_iron,
        TileType::Copper => colors.tile_copper,
        TileType::Gold => colors.tile_gold,
        TileType::Zinc => colors.tile_zinc,
        TileType::Lead => colors.tile_lead,
        TileType::Aluminum => colors.tile_aluminum,
        TileType::Water => colors.tile_water,
    }
}

// Default storage capacities.
pub const DEFAULT_WAREHOUSE_CAPACITY: i32 = 2000;
pub const DEFAULT_MINE_CAPACITY: i32 = 60;
pub const DEFAULT_BASE_CAPACITY: i32 = 2000;
pub const DEFAULT_BUILDER_CAPACITY: i32 = 60;

// Default capacity by block type.
pub fn default_capacity(kind: BuildBlockType) -> i32 {
    match kind {
        BuildBlockType::Base => DEFAULT_BASE_CAPACITY,
        BuildBlockType::Builder => DEFAULT_BUILDER_CAPACITY,
        BuildBlockType::Warehouse => DEFAULT_WAREHOUSE_CAPACITY,
        BuildBlockType::Mine => DEFAULT_MINE_CAPACITY,
        _ => 0,
    }
}

// Convert a tile kind to an item kind.
pub fn item_from_tile(kind: TileType) -> ItemType {
    match kind {
        TileType::Iron => ItemType::Iron,
        TileType::Copper => ItemType::Copper,
        TileType::Gold => ItemType::Gold,
        TileType::Zinc => ItemType::Zinc,
        TileType::Lead => ItemType::Lead,
        TileType::Aluminum => ItemType::Aluminum,
        TileType::Water => ItemType::Water,
    }
}

// Build time (seconds) per block type.
pub fn build_time(kind: BuildBlockType) -> f32 {
    match kind {
        BuildBlockType::Base => 0.0,
        BuildBlockType::Builder => 5.0,
        BuildBlockType::Housing => 6.0,
        BuildBlockType::Factory => 8.0,
        BuildBlockType::Mine => 9.0,
        BuildBlockType::Warehouse => 7.0,
        BuildBlockType::Logistics => 8.0,
        BuildBlockType::Route => 0.5,
    }
}

// Build requirements per block type.
pub fn build_requirements(kind: BuildBlockType) -> Vec<ItemStack> {
    match kind {
        BuildBlockType::Base => vec![],
        BuildBlockType::Builder => vec![
            ItemStack {
                kind: ItemType::Gangue,
                amount: 8,
            },
            ItemStack {
                kind: ItemType::Iron,
                amount: 4,
            },
            ItemStack {
                kind: ItemType::Copper,
                amount: 2,
            },
        ],
        BuildBlockType::Housing => vec![
            ItemStack {
                kind: ItemType::Gangue,
                amount: 10,
            },
            ItemStack {
                kind: ItemType::Copper,
                amount: 4,
            },
        ],
        BuildBlockType::Factory => vec![
            ItemStack {
                kind: ItemType::Iron,
                amount: 12,
            },
            ItemStack {
                kind: ItemType::Copper,
                amount: 8,
            },
        ],
        BuildBlockType::Mine => vec![
            ItemStack {
                kind: ItemType::Gangue,
                amount: 12,
            },
            ItemStack {
                kind: ItemType::Iron,
                amount: 6,
            },
            ItemStack {
                kind: ItemType::Copper,
                amount: 4,
            },
        ],
        BuildBlockType::Warehouse => vec![
            ItemStack {
                kind: ItemType::Gangue,
                amount: 12,
            },
            ItemStack {
                kind: ItemType::Iron,
                amount: 6,
            },
        ],
        BuildBlockType::Logistics => vec![
            ItemStack {
                kind: ItemType::Iron,
                amount: 10,
            },
            ItemStack {
                kind: ItemType::Copper,
                amount: 6,
            },
        ],
        BuildBlockType::Route => vec![ItemStack {
            kind: ItemType::Gangue,
            amount: 3,
        }],
    }
}

// True if the block is still building.
pub fn is_under_construction(block: &PlacedBlock) -> bool {
    block.build_time > 0.0 && block.build_progress < block.build_time
}

// Pick a tile type using weighted probabilities and center bias.
pub fn tile_kind_weighted(dist: i32, radius: i32, config: &AppConfig) -> TileType {
    let center_bias = 1.0 - (dist as f32 / radius as f32).clamp(0.0, 1.0);
    let mut w_i = config.weight_iron.max(0.0);
    let mut w_c = config.weight_copper.max(0.0);
    let mut w_g = config.weight_gold.max(0.0);
    let mut w_z = config.weight_zinc.max(0.0);
    let mut w_l = config.weight_lead.max(0.0);
    let mut w_al = config.weight_aluminum.max(0.0);
    let w_a = config.weight_water.max(0.0);
    let bonus = center_bias * config.tile_center_bonus.max(0.0);
    w_i += bonus * 0.35;
    w_c += bonus * 0.25;
    w_g += bonus * 0.2;
    w_z += bonus * 0.12;
    w_l += bonus * 0.08;
    w_al += bonus * 0.15;
    let total = (w_i + w_c + w_g + w_z + w_l + w_al + w_a).max(0.001);
    let roll = rand::gen_range(0.0, total);
    if roll < w_i {
        TileType::Iron
    } else if roll < w_i + w_c {
        TileType::Copper
    } else if roll < w_i + w_c + w_g {
        TileType::Gold
    } else if roll < w_i + w_c + w_g + w_z {
        TileType::Zinc
    } else if roll < w_i + w_c + w_g + w_z + w_l {
        TileType::Lead
    } else if roll < w_i + w_c + w_g + w_z + w_l + w_al {
        TileType::Aluminum
    } else {
        TileType::Water
    }
}

// Base amount of resources for a tile kind with a center bonus.
pub fn tile_amount(kind: TileType, dist: i32, radius: i32) -> i32 {
    let center_bias = 1.0 - (dist as f32 / radius as f32).clamp(0.0, 1.0);
    let (min_base, max_base) = match kind {
        TileType::Iron => (70, 140),
        TileType::Copper => (60, 120),
        TileType::Gold => (45, 95),
        TileType::Zinc => (55, 110),
        TileType::Lead => (50, 105),
        TileType::Aluminum => (58, 116),
        TileType::Water => (100, 200),
    };
    let span = (max_base - min_base) as f32;
    let bonus = (span * 0.6 * center_bias) as i32;
    min_base + rand::gen_range(0, span as i32 + 1) + bonus
}

// Generate clustered tiles with a center-weighted distribution.
pub fn generate_tiles(radius: i32, config: &AppConfig) -> HashMap<Axial, TileData> {
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
            let amount = tile_amount(kind, dist, radius);
            tiles.insert(
                current,
                TileData {
                    kind,
                    amount,
                },
            );
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

// Add items into a storage list with an optional capacity limit.
pub fn add_item(
    stored: &mut Vec<ItemStack>,
    kind: ItemType,
    amount: i32,
    capacity: i32,
) -> i32 {
    if amount <= 0 {
        return 0;
    }
    let current_total = stored.iter().map(|s| s.amount).sum::<i32>();
    let limit = if capacity <= 0 { i32::MAX } else { capacity };
    let available = (limit - current_total).max(0);
    let add_amount = amount.min(available);
    if add_amount <= 0 {
        return 0;
    }
    if let Some(stack) = stored.iter_mut().find(|s| s.kind == kind) {
        stack.amount += add_amount;
    } else {
        stored.push(ItemStack {
            kind,
            amount: add_amount,
        });
    }
    add_amount
}

// Create a placed block with default storage settings.
pub fn new_placed_block(kind: BuildBlockType, rotation: u8) -> PlacedBlock {
    PlacedBlock {
        kind,
        rotation,
        mine_progress: 0.0,
        capacity: default_capacity(kind),
        stored: Vec::new(),
        build_progress: 0.0,
        build_time: build_time(kind),
        build_paid: false,
        build_claimed: false,
        builder_units_desired: 0,
        builder_units_created: 0,
        mine_extra: Vec::new(),
    }
}

// Rotate a direction index (0-5) by a rotation step.
pub fn rotate_dir(dir: i32, rotation: u8) -> i32 {
    (dir + rotation as i32).rem_euclid(6)
}

// Return input/output port directions for a block.
pub fn block_ports(kind: BuildBlockType, rotation: u8) -> (Vec<i32>, Vec<i32>) {
    let (inputs, outputs) = match kind {
        BuildBlockType::Base => (vec![0, 3], vec![0, 3]),
        BuildBlockType::Builder => (vec![0, 3], vec![0, 3]),
        BuildBlockType::Housing => (vec![0, 2, 4], vec![1, 3, 5]),
        BuildBlockType::Factory => (vec![0, 1, 2], vec![3, 4, 5]),
        BuildBlockType::Mine => (vec![], vec![0, 1, 2, 3, 4, 5]),
        BuildBlockType::Warehouse => (vec![0, 3], vec![0, 3]),
        BuildBlockType::Logistics => (vec![0, 3], vec![0, 3]),
        BuildBlockType::Route => (vec![], vec![]),
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

