use std::collections::HashMap;
use std::path::Path;

use macroquad::prelude::*;

mod scenes;
mod core;
use core::{
    load_config, save_config, Axial, BlockType, ConfigData, FrameContext, PlacedBlock, Scene,
    StationPick, TileData, Unit,
};
use core::ui::{UiButtonColors, WindowState};

const HEX_SIZE: f32 = 16.0;
const GRID_RADIUS: i32 = 64;
const SQRT_3: f32 = 1.732_050_8;

// Entry point and scene dispatcher.
#[macroquad::main("GridGame")]
async fn main() {
    let mut blocks: HashMap<Axial, PlacedBlock> = HashMap::new();
    let mut tiles: HashMap<Axial, TileData> = HashMap::new();
    let mut cam_offset = Vec2::ZERO;
    let mut cam_zoom: f32 = 1.0;
    let config_path = "config.json";
    let config_data = load_config(config_path);
    let mut config = config_data.config;
    let mut colors = config_data.colors;
    cam_zoom = config.zoom_level;
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
    let mut block_window = WindowState {
        title: String::new(),
        rect: Rect::new(40.0, 120.0, 220.0, 120.0),
        open: false,
        target: None,
        dragging: false,
        drag_offset: Vec2::ZERO,
        show_units: false,
    };
    let mut confirm_window = WindowState {
        title: String::new(),
        rect: Rect::new(40.0, 120.0, 240.0, 120.0),
        open: false,
        target: None,
        dragging: false,
        drag_offset: Vec2::ZERO,
        show_units: false,
    };
    let mut units: Vec<Unit> = Vec::new();
    let mut station_in: Option<Axial> = None;
    let mut station_out: Option<Axial> = None;
    let mut station_pick: Option<StationPick> = None;
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
                    &mut units,
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
            Scene::PlanetSector => {
                scenes::planet_sector::run(
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
                    &mut block_window,
                    &mut confirm_window,
                    &mut units,
                    &mut station_in,
                    &mut station_out,
                    &mut station_pick,
                    &mut dirty,
                    &mut scene,
                    map_path,
                    &config,
                    &colors_rt,
                    scenes::planet_sector::MapOutline::Triangle,
                );
            }
            Scene::BuildTemplate => {
                scenes::build_template::run(
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
                    &mut block_window,
                    &mut confirm_window,
                    &mut units,
                    &mut station_in,
                    &mut station_out,
                    &mut station_pick,
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
