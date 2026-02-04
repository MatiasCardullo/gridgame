use std::collections::HashMap;
use std::fs;
use std::path::Path;

use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

mod ui;
use ui::{
    adjust_color_channel, color_target_list, color_target_mut, color_target_name, ui_button,
    AppColors, AppConfig, RuntimeColors, UiButtonColors,
};

const HEX_SIZE: f32 = 16.0;
const GRID_RADIUS: i32 = 64;
const SQRT_3: f32 = 1.732_050_8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Axial {
    q: i32,
    r: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum BlockType {
    Vivienda,
    Fabrica,
    Mina,
    Almacen,
    Logistica,
    Ruta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum TileType {
    Piedra,
    Hierro,
    Cobre,
    Agua,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Block {
    hex: Axial,
    kind: BlockType,
    #[serde(default)]
    rotation: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Tile {
    hex: Axial,
    kind: TileType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PlacedBlock {
    kind: BlockType,
    rotation: u8,
}

#[derive(Serialize, Deserialize)]
struct MapData {
    #[serde(default)]
    tiles: Vec<Tile>,
    blocks: Vec<Block>,
}

#[derive(Serialize, Deserialize)]
struct ConfigData {
    #[serde(default)]
    config: AppConfig,
    #[serde(default)]
    colors: AppColors,
}

impl Default for ConfigData {
    fn default() -> Self {
        Self {
            config: AppConfig::default(),
            colors: AppColors::default(),
        }
    }
}

fn hex_to_pixel(hex: Axial, size: f32, origin: Vec2) -> Vec2 {
    let q = hex.q as f32;
    let r = hex.r as f32;
    let x = size * (SQRT_3 * q + (SQRT_3 / 2.0) * r);
    let y = size * (1.5 * r);
    origin + vec2(x, y)
}

fn pixel_to_hex(pos: Vec2, size: f32, origin: Vec2) -> Axial {
    let p = pos - origin;
    let q = (SQRT_3 / 3.0 * p.x - 1.0 / 3.0 * p.y) / size;
    let r = (2.0 / 3.0 * p.y) / size;
    axial_round(q, r)
}

fn axial_round(q: f32, r: f32) -> Axial {
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

fn hex_corners(center: Vec2, size: f32) -> [Vec2; 6] {
    let mut points = [Vec2::ZERO; 6];
    for i in 0..6 {
        let angle = (60.0 * i as f32 - 30.0).to_radians();
        points[i] = center + vec2(size * angle.cos(), size * angle.sin());
    }
    points
}

fn draw_hex_outline(center: Vec2, size: f32, color: Color, thickness: f32) {
    let points = hex_corners(center, size);
    for i in 0..6 {
        let a = points[i];
        let b = points[(i + 1) % 6];
        draw_line(a.x, a.y, b.x, b.y, thickness, color);
    }
}

fn draw_hex_filled(center: Vec2, size: f32, color: Color) {
    let points = hex_corners(center, size);
    for i in 0..6 {
        let a = points[i];
        let b = points[(i + 1) % 6];
        draw_triangle(center, a, b, color);
    }
}

fn hex_distance(a: Axial, b: Axial) -> i32 {
    let dq = (a.q - b.q).abs();
    let dr = (a.r - b.r).abs();
    let ds = (a.q + a.r - b.q - b.r).abs();
    (dq + dr + ds) / 2
}

fn save_map(path: &str, blocks: &HashMap<Axial, PlacedBlock>, tiles: &HashMap<Axial, TileType>) {
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

fn save_config(path: &str, data: &ConfigData) {
    if let Ok(json) = serde_json::to_string_pretty(data) {
        let _ = fs::write(path, json);
    }
}

fn load_config(path: &str) -> ConfigData {
    if let Ok(json) = fs::read_to_string(path) {
        if let Ok(data) = serde_json::from_str::<ConfigData>(&json) {
            return data;
        }
    }
    ConfigData::default()
}

fn load_map(path: &str) -> (HashMap<Axial, PlacedBlock>, HashMap<Axial, TileType>) {
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

fn block_color(kind: BlockType, colors: &RuntimeColors) -> Color {
    match kind {
        BlockType::Vivienda => colors.block_vivienda,
        BlockType::Fabrica => colors.block_fabrica,
        BlockType::Mina => colors.block_mina,
        BlockType::Almacen => colors.block_almacen,
        BlockType::Logistica => colors.block_logistica,
        BlockType::Ruta => colors.block_ruta,
    }
}

fn tile_color(kind: TileType, colors: &RuntimeColors) -> Color {
    match kind {
        TileType::Piedra => colors.tile_piedra,
        TileType::Hierro => colors.tile_hierro,
        TileType::Cobre => colors.tile_cobre,
        TileType::Agua => colors.tile_agua,
    }
}

fn tile_kind_weighted(dist: i32, radius: i32, config: &AppConfig) -> TileType {
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

fn generate_tiles(radius: i32, config: &AppConfig) -> HashMap<Axial, TileType> {
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

fn rotate_dir(dir: i32, rotation: u8) -> i32 {
    (dir + rotation as i32).rem_euclid(6)
}

fn block_ports(kind: BlockType, rotation: u8) -> (Vec<i32>, Vec<i32>) {
    let (inputs, outputs) = match kind {
        BlockType::Vivienda => (vec![0,2,4], vec![1,3,5]),
        BlockType::Fabrica => (vec![0,1,2], vec![3,4,5]),
        BlockType::Mina => (vec![], vec![0,1,2,3,4,5]),
        BlockType::Almacen => (vec![0,3], vec![0,3]),
        BlockType::Logistica => (vec![0,3], vec![0,3]),
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

fn edge_point(center: Vec2, size: f32, dir: i32) -> Vec2 {
    let angle = (60.0 * dir as f32).to_radians();
    center + vec2(size * angle.cos(), size * angle.sin())
}

fn draw_port_marker(center: Vec2, size: f32, dir: i32, color: Color) {
    let mid = edge_point(center, size, dir);
    let angle = (60.0 * dir as f32).to_radians();
    let forward = vec2(angle.cos(), angle.sin());
    let right = vec2(-forward.y, forward.x);
    let tip = mid + forward * (size * 0.12);
    let left = mid - forward * (size * 0.08) + right * (size * 0.08);
    let right_pt = mid - forward * (size * 0.08) - right * (size * 0.08);
    draw_triangle(tip, left, right_pt, color);
}

fn axial_neighbors(hex: Axial) -> [Axial; 6] {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scene {
    MainMenu,
    Config,
    Game,
}

#[macroquad::main("GridGame")]
async fn main() {
    let mut blocks: HashMap<Axial, PlacedBlock> = HashMap::new();
    let mut tiles: HashMap<Axial, TileType> = HashMap::new();
    let mut cam_offset = Vec2::ZERO;
    let mut cam_zoom: f32 = 1.0;
    let config_path = "config.json";
    let config_data = load_config(config_path);
    let mut config = config_data.config;
    let mut colors = config_data.colors;
    if !Path::new(config_path).exists() {
        save_config(
            config_path,
            &ConfigData {
                config,
                colors,
            },
        );
    }
    let mut color_target_index: usize = 0;
    let mut dragging = false;
    let mut last_mouse = Vec2::ZERO;
    let map_path = "map.json";
    let mut selected: Option<BlockType> = Some(BlockType::Vivienda);
    let mut placement_rotation: u8 = 0;
    let mut panel_collapsed = false;
    let mut dirty = false;
    let mut scene = Scene::MainMenu;

    loop {
        let mouse = vec2(mouse_position().0, mouse_position().1);
        let screen_center = vec2(screen_width() * 0.5, screen_height() * 0.5);
        let has_save = Path::new(map_path).exists();
        let colors_rt = colors.runtime();
        clear_background(colors_rt.background);
        let text_scale = config.text_scale;
        let font_sm = 18.0 * text_scale;
        let font_md = 22.0 * text_scale;
        let font_lg = 40.0 * text_scale;
        let font_title = 48.0 * text_scale;
        let button_colors = UiButtonColors {
            base: colors_rt.button_base,
            hover: colors_rt.button_hover,
            border: colors_rt.button_border,
            text: colors_rt.button_text,
        };

        if scene == Scene::MainMenu {
            let title = "GridGame";
            let title_size = font_title;
            let title_dim = measure_text(title, None, title_size as u16, 1.0);
            draw_text(
                title,
                (screen_width() - title_dim.width) * 0.5,
                120.0,
                title_size,
                colors_rt.text_primary,
            );

            let btn_w = 260.0;
            let btn_h = 52.0;
            let start_y = 200.0;
            let mut y = start_y;

            if has_save {
                let rect = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
                let (clicked, _) = ui_button(rect, "Continuar", mouse, font_md, button_colors);
                if clicked {
                    let (loaded_blocks, loaded_tiles) = load_map(map_path);
                    blocks = loaded_blocks;
                    tiles = loaded_tiles;
                    if tiles.is_empty() {
                        tiles = generate_tiles(GRID_RADIUS, &config);
                        dirty = true;
                    }
                    scene = Scene::Game;
                }
                y += btn_h + 12.0;
            }

            let rect_new = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
            let (clicked_new, _) = ui_button(rect_new, "Nueva Partida", mouse, font_md, button_colors);
            if clicked_new {
                blocks.clear();
                tiles = generate_tiles(GRID_RADIUS, &config);
                cam_offset = Vec2::ZERO;
                cam_zoom = 1.0;
                placement_rotation = 0;
                scene = Scene::Game;
            }
            y += btn_h + 12.0;

            let rect_cfg = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
            let (clicked_cfg, _) = ui_button(rect_cfg, "Configuracion", mouse, font_md, button_colors);
            if clicked_cfg {
                scene = Scene::Config;
            }

            let rect_exit = Rect::new((screen_width() - btn_w) * 0.5, y + btn_h + 12.0, btn_w, btn_h);
            let (clicked_exit, _) = ui_button(rect_exit, "Salir", mouse, font_md, button_colors);
            if clicked_exit {
                if !blocks.is_empty() || !tiles.is_empty() {
                    save_map(map_path, &blocks, &tiles);
                }
                break;
            }
        }

        if scene == Scene::Config {
            let title = "Configuracion";
            let title_size = font_lg;
            let title_dim = measure_text(title, None, title_size as u16, 1.0);
            let mut changed = false;
            draw_text(
                title,
                (screen_width() - title_dim.width) * 0.5,
                120.0,
                title_size,
                colors_rt.text_primary,
            );

            let label_x = 100.0;
            let mut y = 200.0;
            let btn_w = 40.0;
            let btn_h = 34.0;
            let btn_gap = 8.0;
            let btn_x = 330.0;
            let value_x = btn_x + btn_w * 2.0 + btn_gap + 12.0;
            let row_gap = 44.0;

            let stepper_text = colors_rt.text_secondary;
            let mut draw_stepper = |label: &str, value: &str, y: f32| -> (bool, bool) {
                draw_text(label, label_x, y, font_md, stepper_text);
                let rect_dec = Rect::new(btn_x, y - 24.0, btn_w, btn_h);
                let rect_inc = Rect::new(btn_x + btn_w + btn_gap, y - 24.0, btn_w, btn_h);
                let (dec, _) = ui_button(rect_dec, "-", mouse, font_md, button_colors);
                let (inc, _) = ui_button(rect_inc, "+", mouse, font_md, button_colors);
                draw_text(value, value_x, y, font_md, stepper_text);
                (dec, inc)
            };

            let (dec, inc) = draw_stepper("Velocidad de zoom", &format!("{:.2}", config.zoom_speed), y);
            if dec {
                changed = true;
                config.zoom_speed = (config.zoom_speed - 0.02).max(0.02);
            }
            if inc {
                changed = true;
                config.zoom_speed = (config.zoom_speed + 0.02).min(0.3);
            }
            y += row_gap;

            let (dec, inc) = draw_stepper(
                "Escala de texto",
                &format!("{:.2}", config.text_scale),
                y,
            );
            if dec {
                changed = true;
                config.text_scale = (config.text_scale - 0.05).max(0.7);
            }
            if inc {
                changed = true;
                config.text_scale = (config.text_scale + 0.05).min(1.6);
            }
            y += row_gap;

            let (dec, inc) = draw_stepper(
                "Grosor de lineas",
                &format!("{:.2}", config.line_thickness),
                y,
            );
            if dec {
                changed = true;
                config.line_thickness = (config.line_thickness - 0.25).max(0.5);
            }
            if inc {
                changed = true;
                config.line_thickness = (config.line_thickness + 0.25).min(4.0);
            }
            y += row_gap;

            let (dec, inc) =
                draw_stepper("Escala flechas", &format!("{:.2}", config.arrow_scale), y);
            if dec {
                changed = true;
                config.arrow_scale = (config.arrow_scale - 0.1).max(0.5);
            }
            if inc {
                changed = true;
                config.arrow_scale = (config.arrow_scale + 0.1).min(2.0);
            }
            y += row_gap;

            let (dec, inc) =
                draw_stepper("Clusters tiles", &format!("{}", config.tile_clusters), y);
            if dec {
                changed = true;
                config.tile_clusters = config.tile_clusters.saturating_sub(5).max(5);
            }
            if inc {
                changed = true;
                config.tile_clusters = (config.tile_clusters + 5).min(300);
            }
            y += row_gap;

            let (dec, inc) = draw_stepper(
                "Cluster min",
                &format!("{}", config.tile_cluster_min),
                y,
            );
            if dec && config.tile_cluster_min > 1 {
                changed = true;
                config.tile_cluster_min -= 1;
            }
            if inc {
                changed = true;
                config.tile_cluster_min += 1;
            }
            if config.tile_cluster_min > config.tile_cluster_max {
                config.tile_cluster_max = config.tile_cluster_min;
            }
            y += row_gap;

            let (dec, inc) = draw_stepper(
                "Cluster max",
                &format!("{}", config.tile_cluster_max),
                y,
            );
            if dec && config.tile_cluster_max > 1 {
                changed = true;
                config.tile_cluster_max -= 1;
            }
            if inc {
                changed = true;
                config.tile_cluster_max += 1;
            }
            if config.tile_cluster_max < config.tile_cluster_min {
                config.tile_cluster_min = config.tile_cluster_max;
            }
            y += row_gap;

            let (dec, inc) = draw_stepper(
                "Chance vecinos",
                &format!("{:.2}", config.tile_neighbor_chance),
                y,
            );
            if dec {
                changed = true;
                config.tile_neighbor_chance = (config.tile_neighbor_chance - 0.05).max(0.05);
            }
            if inc {
                changed = true;
                config.tile_neighbor_chance = (config.tile_neighbor_chance + 0.05).min(0.95);
            }
            y += row_gap;

            let (dec, inc) = draw_stepper(
                "Bonus centro",
                &format!("{:.2}", config.tile_center_bonus),
                y,
            );
            if dec {
                changed = true;
                config.tile_center_bonus = (config.tile_center_bonus - 0.05).max(0.0);
            }
            if inc {
                changed = true;
                config.tile_center_bonus = (config.tile_center_bonus + 0.05).min(1.0);
            }
            y += row_gap;

            let (dec, inc) =
                draw_stepper("Peso piedra", &format!("{:.2}", config.weight_piedra), y);
            if dec {
                changed = true;
                config.weight_piedra = (config.weight_piedra - 0.05).max(0.0);
            }
            if inc {
                changed = true;
                config.weight_piedra = (config.weight_piedra + 0.05).min(1.0);
            }
            y += row_gap;

            let (dec, inc) =
                draw_stepper("Peso hierro", &format!("{:.2}", config.weight_hierro), y);
            if dec {
                changed = true;
                config.weight_hierro = (config.weight_hierro - 0.05).max(0.0);
            }
            if inc {
                changed = true;
                config.weight_hierro = (config.weight_hierro + 0.05).min(1.0);
            }
            y += row_gap;

            let (dec, inc) =
                draw_stepper("Peso cobre", &format!("{:.2}", config.weight_cobre), y);
            if dec {
                changed = true;
                config.weight_cobre = (config.weight_cobre - 0.05).max(0.0);
            }
            if inc {
                changed = true;
                config.weight_cobre = (config.weight_cobre + 0.05).min(1.0);
            }
            y += row_gap;

            let (dec, inc) =
                draw_stepper("Peso agua", &format!("{:.2}", config.weight_agua), y);
            if dec {
                changed = true;
                config.weight_agua = (config.weight_agua - 0.05).max(0.0);
            }
            if inc {
                changed = true;
                config.weight_agua = (config.weight_agua + 0.05).min(1.0);
            }
            y += row_gap;

            let rect_regen = Rect::new(label_x, y - 16.0, 240.0, 38.0);
            let (regen, _) = ui_button(rect_regen, "Regenerar tiles", mouse, font_md, button_colors);
            if regen {
                tiles = generate_tiles(GRID_RADIUS, &config);
                dirty = true;
            }

            let color_targets = color_target_list();
            let rect_prev = Rect::new(380.0, y - 16.0, 32.0, 38.0);
            let rect_next = Rect::new(420.0, y - 16.0, 32.0, 38.0);
            let (prev, _) = ui_button(rect_prev, "<", mouse, font_md, button_colors);
            let (next, _) = ui_button(rect_next, ">", mouse, font_md, button_colors);
            if prev {
                if color_target_index == 0 {
                    color_target_index = color_targets.len() - 1;
                } else {
                    color_target_index -= 1;
                }
            }
            if next {
                color_target_index = (color_target_index + 1) % color_targets.len();
            }
            let target = color_targets[color_target_index % color_targets.len()];
            let target_name = color_target_name(target);
            draw_text(
                &format!("Color: {}", target_name),
                470.0,
                y + 10.0,
                font_md,
                colors_rt.text_secondary,
            );
            y += row_gap;

            let text_secondary = colors_rt.text_secondary;
            let color = color_target_mut(target, &mut colors);
            let channels = [("R", color.r), ("G", color.g), ("B", color.b)];
            let swatch_rect = Rect::new(680.0, y - 8.0, 36.0, 36.0);
            draw_rectangle(
                swatch_rect.x,
                swatch_rect.y,
                swatch_rect.w,
                swatch_rect.h,
                color.to_color(),
            );
            draw_rectangle_lines(
                swatch_rect.x,
                swatch_rect.y,
                swatch_rect.w,
                swatch_rect.h,
                config.line_thickness.max(1.0),
                colors_rt.panel_border,
            );
            for (idx, (label, value)) in channels.iter().enumerate() {
                let row_y = y + idx as f32 * row_gap;
                draw_text(label, label_x, row_y, font_md, text_secondary);
                let rect_dec = Rect::new(btn_x, row_y - 24.0, btn_w, btn_h);
                let rect_inc = Rect::new(btn_x + btn_w + btn_gap, row_y - 24.0, btn_w, btn_h);
                let (dec, _) = ui_button(rect_dec, "-", mouse, font_md, button_colors);
                let (inc, _) = ui_button(rect_inc, "+", mouse, font_md, button_colors);
                if dec {
                    changed = true;
                    adjust_color_channel(color, idx, -8);
                }
                if inc {
                    changed = true;
                    adjust_color_channel(color, idx, 8);
                }
                draw_text(&format!("{}", value), value_x, row_y, font_md, text_secondary);
            }

            if changed {
                save_config(
                    config_path,
                    &ConfigData {
                        config,
                        colors,
                    },
                );
            }

            let rect_back = Rect::new(100.0, screen_height() - 80.0, 180.0, 48.0);
            let (back, _) = ui_button(rect_back, "Volver", mouse, font_md, button_colors);
            if back {
                scene = Scene::MainMenu;
            }
        }

        if scene == Scene::Game {
            let wheel = mouse_wheel().1;
            if wheel.abs() > 0.01 {
                let factor = 1.0 + wheel * config.zoom_speed;
                cam_zoom = (cam_zoom * factor).clamp(0.3, 3.0);
            }

            if is_mouse_button_pressed(MouseButton::Middle) {
                dragging = true;
                last_mouse = mouse;
            }
            if is_mouse_button_down(MouseButton::Middle) && dragging {
                let delta = mouse - last_mouse;
                cam_offset += delta;
                last_mouse = mouse;
            }
            if is_mouse_button_released(MouseButton::Middle) {
                dragging = false;
            }

            let world_mouse = (mouse - screen_center - cam_offset) / cam_zoom;
            let hover_hex = pixel_to_hex(world_mouse, HEX_SIZE, Vec2::ZERO);

            let panel_pos = vec2(16.0, screen_height() - 110.0);
            let panel_size = if panel_collapsed {
                vec2(170.0, 36.0)
            } else {
                vec2(392.0, 94.0)
            };
            let panel_rect = Rect::new(panel_pos.x, panel_pos.y, panel_size.x, panel_size.y);

            let mut tooltip: Option<&str> = None;
            let mut ui_capturing = panel_rect.contains(mouse);

            if is_mouse_button_pressed(MouseButton::Right) && !ui_capturing {
                if hex_distance(hover_hex, Axial { q: 0, r: 0 }) <= GRID_RADIUS {
                    if let Some(kind) = selected {
                        let can_mine = matches!(
                            tiles.get(&hover_hex),
                            Some(TileType::Piedra) | Some(TileType::Hierro) | Some(TileType::Cobre)
                        );
                        if kind != BlockType::Mina || can_mine {
                            let rotation = if kind == BlockType::Ruta { 0 } else { placement_rotation };
                            blocks.insert(
                                hover_hex,
                                PlacedBlock {
                                    kind,
                                    rotation,
                                },
                            );
                            dirty = true;
                        }
                    } else if blocks.remove(&hover_hex).is_some() {
                        dirty = true;
                    }
                }
            }

            if is_key_pressed(KeyCode::S) {
                save_map(map_path, &blocks, &tiles);
                dirty = false;
            }

            if is_key_pressed(KeyCode::L) {
                let (loaded_blocks, loaded_tiles) = load_map(map_path);
                blocks = loaded_blocks;
                tiles = loaded_tiles;
                if tiles.is_empty() {
                    tiles = generate_tiles(GRID_RADIUS, &config);
                }
                dirty = false;
            }

            if dirty {
                save_map(map_path, &blocks, &tiles);
                dirty = false;
            }

            if is_key_pressed(KeyCode::Escape) {
                save_map(map_path, &blocks, &tiles);
                scene = Scene::MainMenu;
            }

            if is_key_pressed(KeyCode::R) {
                if let Some(block) = blocks.get_mut(&hover_hex) {
                    if block.kind != BlockType::Ruta {
                        block.rotation = (block.rotation + 1) % 6;
                        dirty = true;
                    }
                } else {
                    placement_rotation = (placement_rotation + 1) % 6;
                }
            }

            for r in -GRID_RADIUS..=GRID_RADIUS {
                for q in -GRID_RADIUS..=GRID_RADIUS {
                    let hex = Axial { q, r };
                    if hex_distance(hex, Axial { q: 0, r: 0 }) > GRID_RADIUS {
                        continue;
                    }
                    let world_center = hex_to_pixel(hex, HEX_SIZE, Vec2::ZERO);
                    let center = screen_center + cam_offset + world_center * cam_zoom;

                    let size = HEX_SIZE * cam_zoom;
                    if center.x < -size
                        || center.y < -size
                        || center.x > screen_width() + size
                        || center.y > screen_height() + size
                    {
                        continue;
                    }

                    if let Some(tile) = tiles.get(&hex) {
                        draw_hex_filled(
                            center,
                            (HEX_SIZE - 4.5) * cam_zoom,
                            tile_color(*tile, &colors_rt),
                        );
                    }

                    if let Some(placed) = blocks.get(&hex) {
                        draw_hex_filled(
                            center,
                            (HEX_SIZE - 2.5) * cam_zoom,
                            block_color(placed.kind, &colors_rt),
                        );
                        let (inputs, outputs) = block_ports(placed.kind, placed.rotation);
                        for dir in inputs {
                            draw_port_marker(
                                center,
                                (HEX_SIZE - 3.0) * cam_zoom * config.arrow_scale,
                                dir,
                                colors_rt.port_in,
                            );
                        }
                        for dir in outputs {
                            draw_port_marker(
                                center,
                                (HEX_SIZE - 3.0) * cam_zoom * config.arrow_scale,
                                dir,
                                colors_rt.port_out,
                            );
                        }
                    }

                    draw_hex_outline(center, size, colors_rt.grid, config.line_thickness);
                }
            }

            for (hex, placed) in blocks.iter() {
                if placed.kind != BlockType::Ruta {
                    continue;
                }
                let base_center =
                    screen_center + cam_offset + hex_to_pixel(*hex, HEX_SIZE, Vec2::ZERO) * cam_zoom;
                let neighbors = axial_neighbors(*hex);
                for neighbor in neighbors {
                    if blocks.contains_key(&neighbor) {
                        let neighbor_center = screen_center
                            + cam_offset
                            + hex_to_pixel(neighbor, HEX_SIZE, Vec2::ZERO) * cam_zoom;
                        draw_line(
                            base_center.x,
                            base_center.y,
                            neighbor_center.x,
                            neighbor_center.y,
                            (2.0 + config.line_thickness) * cam_zoom,
                            colors_rt.route_line,
                        );
                    }
                }
            }

            let hover_center =
                screen_center + cam_offset + hex_to_pixel(hover_hex, HEX_SIZE, Vec2::ZERO) * cam_zoom;
            draw_hex_outline(
                hover_center,
                HEX_SIZE * cam_zoom,
                colors_rt.hover,
                config.line_thickness * 2.0,
            );

            draw_text(
                "Click: colocar  |  Rueda: zoom  |  Boton medio: mover  |  R: rotar  |  Panel: <<  |  S: guardar  |  L: cargar  |  Esc: menu",
                16.0,
                28.0,
                font_md,
                colors_rt.text_primary,
            );

            let coord_text = format!("Hex: q={} r={}", hover_hex.q, hover_hex.r);
            draw_text(
                &coord_text,
                16.0,
                52.0,
                font_sm,
                colors_rt.text_secondary,
            );

            let rot_text = format!("Rotacion: {}", placement_rotation);
            draw_text(
                &rot_text,
                16.0,
                74.0,
                font_sm,
                colors_rt.text_secondary,
            );

            draw_rectangle(
                panel_pos.x,
                panel_pos.y,
                panel_size.x,
                panel_size.y,
                colors_rt.panel_bg,
            );
            draw_rectangle_lines(
                panel_pos.x,
                panel_pos.y,
                panel_size.x,
                panel_size.y,
                config.line_thickness.max(1.0),
                colors_rt.panel_border,
            );

            let toggle_rect = Rect::new(
                panel_pos.x + panel_size.x - 32.0,
                panel_pos.y + 6.0,
                26.0,
                24.0,
            );
            let toggle_hover = toggle_rect.contains(mouse);
            if toggle_hover {
                ui_capturing = true;
            }
            let toggle_color = if toggle_hover {
                colors_rt.button_hover
            } else {
                colors_rt.button_base
            };
            draw_rectangle(
                toggle_rect.x,
                toggle_rect.y,
                toggle_rect.w,
                toggle_rect.h,
                toggle_color,
            );
            draw_rectangle_lines(
                toggle_rect.x,
                toggle_rect.y,
                toggle_rect.w,
                toggle_rect.h,
                config.line_thickness.max(1.0),
                    colors_rt.button_border,
            );
            let toggle_label = if panel_collapsed { ">>" } else { "<<" };
            let toggle_dim = measure_text(toggle_label, None, font_sm as u16, 1.0);
            draw_text(
                toggle_label,
                toggle_rect.x + (toggle_rect.w - toggle_dim.width) * 0.5,
                toggle_rect.y + (toggle_rect.h + toggle_dim.height) * 0.5 - 2.0,
                font_sm,
                colors_rt.button_text,
            );
            if toggle_hover && is_mouse_button_pressed(MouseButton::Left) {
                panel_collapsed = !panel_collapsed;
                ui_capturing = true;
            }

            if !panel_collapsed {
                let button_size = 52.0;
                let gap = 8.0;
                let mut bx = panel_pos.x + 12.0;
                let by = panel_pos.y + 16.0;

                let buttons: [(Option<BlockType>, &str); 6] = [
                    (None, "Deconstruir"),
                    (Some(BlockType::Vivienda), "Vivienda"),
                    (Some(BlockType::Fabrica), "Fabrica"),
                    (Some(BlockType::Mina), "Minas"),
                    (Some(BlockType::Almacen), "Almacen"),
                    (Some(BlockType::Ruta), "Ruta"),
                ];

                for (option, tip) in buttons {
                    let rect = Rect::new(bx, by, button_size, button_size);
                    let hover = rect.contains(mouse);
                    if hover {
                        tooltip = Some(tip);
                        ui_capturing = true;
                    }
                    let selected_now = selected == option;

                    let base_color = if selected_now {
                        colors_rt.button_hover
                    } else {
                        colors_rt.button_base
                    };
                    draw_rectangle(rect.x, rect.y, rect.w, rect.h, base_color);
                    draw_rectangle_lines(
                        rect.x,
                        rect.y,
                        rect.w,
                        rect.h,
                        config.line_thickness.max(1.0),
                        colors_rt.button_border,
                    );

                    if hover && is_mouse_button_pressed(MouseButton::Left) {
                        selected = option;
                    }

                    if let Some(kind) = option {
                        let icon_center = vec2(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
                        draw_hex_filled(icon_center, 16.0, block_color(kind, &colors_rt));
                        draw_hex_outline(
                            icon_center,
                            16.0,
                            colors_rt.panel_border,
                            config.line_thickness.max(1.0),
                        );
                    } else {
                        let pad = 10.0;
                        draw_line(
                            rect.x + pad,
                            rect.y + pad,
                            rect.x + rect.w - pad,
                            rect.y + rect.h - pad,
                            3.0,
                            colors_rt.port_out,
                        );
                        draw_line(
                            rect.x + rect.w - pad,
                            rect.y + pad,
                            rect.x + pad,
                            rect.y + rect.h - pad,
                            3.0,
                            colors_rt.port_out,
                        );
                    }

                    if hover {
                        draw_rectangle_lines(
                            rect.x - 1.0,
                            rect.y - 1.0,
                            rect.w + 2.0,
                            rect.h + 2.0,
                            config.line_thickness.max(1.0),
                            colors_rt.hover,
                        );
                    }

                    bx += button_size + gap;
                }
            }

            if let Some(tip) = tooltip {
                let pad = 6.0;
                let font_size = font_sm;
                let dim = measure_text(tip, None, font_size as u16, 1.0);
                let x = (mouse.x + 14.0).min(screen_width() - dim.width - 2.0 * pad);
                let y = (mouse.y + 16.0).min(screen_height() - dim.height - 2.0 * pad);
                draw_rectangle(
                    x,
                    y,
                    dim.width + 2.0 * pad,
                    dim.height + 2.0 * pad,
                    colors_rt.tooltip_bg,
                );
                draw_rectangle_lines(
                    x,
                    y,
                    dim.width + 2.0 * pad,
                    dim.height + 2.0 * pad,
                    config.line_thickness.max(1.0),
                    colors_rt.tooltip_border,
                );
                draw_text(
                    tip,
                    x + pad,
                    y + dim.height + pad - 2.0,
                    font_size,
                    colors_rt.text_primary,
                );
            }
        }

        next_frame().await;
    }
}
