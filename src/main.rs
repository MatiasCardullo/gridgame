use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};

use macroquad::prelude::*;

mod core;
mod scenes;
use core::ui::{UiButtonColors, WindowState};
use core::{
    Axial, ConfigData, FrameContext, MachineBlockType, PlacedBlock, Scene, TileData, Unit,
    load_config, save_config,
};

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
        save_config(config_path, &ConfigData { config, colors });
    }
    let mut color_target_index: usize = 0;
    let mut dragging = false;
    let mut last_mouse = Vec2::ZERO;
    let map_path = "map.json";
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
    let mut planet_state: Option<scenes::planet::PlanetState> = None;
    let mut planet_loader = scenes::planet::PlanetLoader::start();
    let (api_tx, api_rx) = mpsc::channel();
    if let Err(error) = core::api::start(api_tx) {
        eprintln!("GridGame local API unavailable: {error}");
    }
    let greek_font = load_ttf_font("C:/Windows/Fonts/CascadiaMono.ttf")
        .await
        .ok();
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
        let mut screenshot_replies = Vec::new();
        for _ in 0..32 {
            let Some(message) = core::api::receive_request(&api_rx) else {
                break;
            };
            let reply = match message.command {
                core::api::ApiCommand::Screenshot => {
                    screenshot_replies.push(message.reply);
                    continue;
                }
                core::api::ApiCommand::EnterPlanet => {
                    scene = Scene::Planet;
                    core::api::ApiReply::json(
                        200,
                        serde_json::json!({
                            "scene": "planet",
                            "planet_ready": planet_state.is_some(),
                        }),
                    )
                }
                command => match planet_state.as_mut() {
                    Some(state) => state.handle_api_command(command),
                    None => match command {
                        core::api::ApiCommand::Status => core::api::ApiReply::json(
                            200,
                            serde_json::json!({
                                "planet_ready": false,
                                "cell_count": null,
                                "building_count": 0,
                                "unit_count": 0,
                            }),
                        ),
                        _ => core::api::ApiReply::error(
                            503,
                            "planet_unavailable",
                            "planet state is not available yet",
                        ),
                    },
                },
            };
            if message.reply.send(reply).is_err() {
                eprintln!("GridGame API client disconnected before receiving a reply");
            }
        }
        let mouse = vec2(mouse_position().0, mouse_position().1);
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
                    &mut scene,
                    map_path,
                    planet_loader.is_loading(),
                    planet_loader.is_ready(),
                    planet_loader.progress(),
                    planet_loader.log_entries(),
                    planet_loader.error(),
                ) {
                    if let Some(state) = planet_state.as_mut() {
                        state.save_gameplay_state();
                    }
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
                if let Some(state) = planet_state.as_mut() {
                    scenes::build_template::run(
                        &ctx,
                        state,
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
                    );
                } else {
                    scene = Scene::Planet;
                }
            }
        }

        if !screenshot_replies.is_empty() {
            let reply = capture_screenshot();
            for screenshot_reply in screenshot_replies {
                if screenshot_reply.send(reply.clone()).is_err() {
                    eprintln!(
                        "GridGame API screenshot client disconnected before receiving a reply"
                    );
                }
            }
        }
        next_frame().await;
    }
}

fn capture_screenshot() -> core::api::ApiReply {
    let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => {
            return core::api::ApiReply::error(
                500,
                "screenshot_failed",
                format!("system clock is before the Unix epoch: {error}"),
            );
        }
    };
    let path = std::env::temp_dir().join(format!(
        "gridgame-screenshot-{}-{timestamp}.png",
        std::process::id()
    ));
    get_screen_data().export_png(&path.to_string_lossy());
    match std::fs::read(&path) {
        Ok(png) => {
            if let Err(error) = std::fs::remove_file(&path) {
                eprintln!("GridGame API could not remove temporary screenshot: {error}");
            }
            core::api::ApiReply::png(png)
        }
        Err(error) => {
            if let Err(remove_error) = std::fs::remove_file(&path) {
                if remove_error.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("GridGame API could not remove temporary screenshot: {remove_error}");
                }
            }
            core::api::ApiReply::error(
                500,
                "screenshot_failed",
                format!("could not read captured screenshot: {error}"),
            )
        }
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
    let bar_rect = Rect::new((screen_width() - 420.0) * 0.5, 160.0, 420.0, 18.0);
    core::ui::draw_progress_bar(
        bar_rect,
        loader.progress(),
        ctx.colors_rt.button_hover,
        ctx.colors_rt.button_base,
    );
    let log_rect = Rect::new((screen_width() - 520.0) * 0.5, 200.0, 520.0, 260.0);
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
