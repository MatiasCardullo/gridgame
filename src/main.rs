use std::collections::HashMap;
use std::path::Path;

use macroquad::prelude::*;

mod scenes;
mod core;
use core::{
    load_config, save_config, Axial, BuildBlockType, MachineBlock, MachineBlockType, ConfigData, FrameContext, PlacedBlock, Scene,
    StationPick, TileData, Unit,
};
use core::ui::{UiButtonColors, WindowState};

const HEX_SIZE: f32 = 15.0;
const HEX_RADIUS: i32 = 64;
const TRI_LENGHT: i32 = 320;
const SQRT_3: f32 = 1.732_050_8;

// Entry point and scene dispatcher.
#[macroquad::main("GridGame")]
async fn main() {
    let mut blocks: HashMap<Axial, PlacedBlock> = HashMap::new();
    let mut tiles: HashMap<Axial, TileData> = HashMap::new();
    let mut cam_offset = Vec2::ZERO;
    let mut cam_zoom: f32;
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
    let _template_path = "template.json";
    let mut selected: Option<BuildBlockType> = Some(BuildBlockType::Housing);
    let mut template_blocks: HashMap<Axial, MachineBlock> = HashMap::new();
    let mut template_selected: Option<MachineBlockType> = Some(MachineBlockType::ConveyorBelt);
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
    let mut planet_state: Option<scenes::planet::PlanetState> = None;
    let mut planet_loader = scenes::planet::PlanetLoader::start();
    let greek_font = load_ttf_font("C:/Windows/Fonts/CascadiaMono.ttf").await.ok();
    let mut dirty = false;
    let mut scene = Scene::MainMenu;

    loop {
        if let Some(data) = planet_loader.poll() {
            if let Some(state) = planet_state.as_mut() {
                state.apply_relief_build_data(data);
            } else {
                planet_state = Some(scenes::planet::PlanetState::from_build_data(data));
            }
        }
        if let Some(texture_data) = planet_loader.take_texture_data() {
            if planet_state.is_none() {
                planet_state = Some(scenes::planet::PlanetState::from_texture_data(texture_data));
            }
        }
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
                    &mut template_blocks,
                    &mut cam_offset,
                    &mut cam_zoom,
                    &mut placement_rotation,
                    &mut scene,
                    &mut dirty,
                    map_path,
                    &config,
                    planet_loader.is_loading(),
                    planet_loader.is_ready(),
                    planet_loader.progress(),
                    planet_loader.log_entries(),
                    planet_loader.error(),
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
                    scenes::map_common::MapOutline::Triangle,
                );
            }
            Scene::Planet => {
                if let Some(state) = planet_state.as_mut() {
                    scenes::planet::run(&ctx, state, &mut scene, greek_font.as_ref());
                } else {
                    draw_planet_loading_screen(&ctx, &planet_loader);
                    if is_key_pressed(KeyCode::Escape) {
                        scene = Scene::MainMenu;
                    }
                }
            }
            Scene::BuildTemplate => {
                scenes::build_template::run(
                    &ctx,
                    &mut cam_offset,
                    &mut cam_zoom,
                    &mut dragging,
                    &mut last_mouse,
                    &mut template_selected,
                    &mut placement_rotation,
                    &mut panel_collapsed,
                    &mut block_window,
                    &mut confirm_window,
                    &mut scene,
                    &config,
                    &colors_rt,
                    &mut template_blocks,
                );
            }
        }

        next_frame().await;
    }
}

fn draw_planet_loading_screen(ctx: &FrameContext, loader: &scenes::planet::PlanetLoader) {
    let title = "Cargando planeta";
    let title_dim = measure_text(title, None, ctx.font_title as u16, 1.0);
    draw_text(
        title,
        (screen_width() - title_dim.width) * 0.5,
        120.0,
        ctx.font_title,
        ctx.colors_rt.text_primary,
    );
    let bar_rect = Rect::new(
        (screen_width() - 420.0) * 0.5,
        160.0,
        420.0,
        18.0,
    );
    core::ui::draw_progress_bar(
        bar_rect,
        loader.progress(),
        ctx.colors_rt.button_hover,
        ctx.colors_rt.button_base,
    );
    let log_rect = Rect::new(
        (screen_width() - 520.0) * 0.5,
        200.0,
        520.0,
        260.0,
    );
    if let Some(message) = loader.error() {
        let lines = vec![message.to_string()];
        core::ui::draw_log_panel(
            log_rect,
            &lines,
            ctx.font_sm,
            ctx.colors_rt.tooltip_bg,
            ctx.colors_rt.tooltip_border,
            ctx.colors_rt.text_primary,
        );
    } else {
        core::ui::draw_log_panel(
            log_rect,
            loader.log_entries(),
            ctx.font_sm,
            ctx.colors_rt.tooltip_bg,
            ctx.colors_rt.tooltip_border,
            ctx.colors_rt.text_secondary,
        );
    }
    let hint = "ESC para volver al menu";
    let hint_dim = measure_text(hint, None, ctx.font_sm as u16, 1.0);
    draw_text(
        hint,
        (screen_width() - hint_dim.width) * 0.5,
        log_rect.y + log_rect.h + 28.0,
        ctx.font_sm,
        ctx.colors_rt.text_secondary,
    );
}
