use std::collections::HashMap;
use std::path::Path;

use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

mod ui;
mod scenes;
mod core;
use core::{load_config, save_config, ConfigData};
use ui::{AppColors, AppConfig, RuntimeColors, UiButtonColors};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scene {
    MainMenu,
    Config,
    Game,
}

#[derive(Clone, Copy, Debug)]
struct FrameContext {
    mouse: Vec2,
    screen_center: Vec2,
    has_save: bool,
    font_sm: f32,
    font_md: f32,
    font_lg: f32,
    font_title: f32,
    button_colors: UiButtonColors,
    colors_rt: RuntimeColors,
}

// Entry point and scene dispatcher.
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

        let ctx = FrameContext {
            mouse,
            screen_center,
            has_save,
            font_sm,
            font_md,
            font_lg,
            font_title,
            button_colors,
            colors_rt,
        };

        match scene {
            Scene::MainMenu => {
                if scenes::main_menu::run(
                    &ctx,
                    &mut blocks,
                    &mut tiles,
                    &mut cam_offset,
                    &mut cam_zoom,
                    &mut placement_rotation,
                    &mut scene,
                    &mut dirty,
                    map_path,
                    &config,
                ) {
                    break;
                }
            }
            Scene::Config => {
                scenes::config::run(
                    &ctx,
                    &mut config,
                    &mut colors,
                    &mut color_target_index,
                    &mut tiles,
                    &mut dirty,
                    &mut scene,
                    config_path,
                );
            }
            Scene::Game => {
                scenes::game::run(
                    &ctx,
                    &mut blocks,
                    &mut tiles,
                    &mut cam_offset,
                    &mut cam_zoom,
                    &mut dragging,
                    &mut last_mouse,
                    &mut selected,
                    &mut placement_rotation,
                    &mut panel_collapsed,
                    &mut dirty,
                    &mut scene,
                    map_path,
                    &config,
                    &colors_rt,
                );
            }
        }

        next_frame().await;
    }
}
