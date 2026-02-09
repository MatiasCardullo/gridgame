use macroquad::prelude::*;

use crate::core::{
    generate_tiles, load_map, save_map, AppConfig, Axial, FrameContext, PlacedBlock, MachineBlock, Scene,
    TileData, Unit,
};
use crate::core::ui::ui_button;
use crate::TRI_LENGHT;
use crate::scenes::map_common::{outline_start_offset, MapOutline};
use std::collections::HashMap;

// Render and handle input for the main menu scene.
pub fn run(
    ctx: &FrameContext,
    blocks: &mut HashMap<Axial, PlacedBlock>,
    tiles: &mut HashMap<Axial, TileData>,
    units: &mut Vec<Unit>,
    template_blocks: &mut HashMap<Axial, MachineBlock>,
    cam_offset: &mut Vec2,
    cam_zoom: &mut f32,
    placement_rotation: &mut u8,
    scene: &mut Scene,
    dirty: &mut bool,
    map_path: &str,
    config: &AppConfig,
) -> bool {
    let title = "GridGame";
    let title_size = ctx.font_title;
    let title_dim = measure_text(title, None, title_size as u16, 1.0);
    draw_text(
        title,
        (screen_width() - title_dim.width) * 0.5,
        120.0,
        title_size,
        ctx.colors_rt.text_primary,
    );

    let btn_w = 260.0;
    let btn_h = 52.0;
    let start_y = 200.0;
    let mut y = start_y;

    if ctx.has_save {
        let rect = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
        let (clicked, _) = ui_button(rect, "Continuar", ctx.mouse, ctx.font_md, ctx.button_colors);
        if clicked {
            let (loaded_blocks, loaded_tiles, loaded_units) = load_map(map_path);
            *blocks = loaded_blocks;
            *tiles = loaded_tiles;
            *units = loaded_units;
            if tiles.is_empty() {
                *tiles = generate_tiles(TRI_LENGHT, config);
                *dirty = true;
            }
            *cam_offset = outline_start_offset(MapOutline::Triangle, config.zoom_level);
            *cam_zoom = config.zoom_level;
            *scene = Scene::PlanetSector;
        }
        y += btn_h + 12.0;
    }

    let rect_new = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
    let (clicked_new, _) = ui_button(rect_new, "Planet Sector", ctx.mouse, ctx.font_md, ctx.button_colors);
    if clicked_new {
        blocks.clear();
        *tiles = generate_tiles(TRI_LENGHT, config);
        units.clear();
        *cam_offset = outline_start_offset(MapOutline::Triangle, config.zoom_level);
        *cam_zoom = config.zoom_level;
        *placement_rotation = 0;
        *scene = Scene::PlanetSector;
    }
    y += btn_h + 12.0;

    let rect_cfg = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
    let (clicked_cfg, _) = ui_button(rect_cfg, "Configuracion", ctx.mouse, ctx.font_md, ctx.button_colors);
    if clicked_cfg {
        *scene = Scene::Config;
    }
    y += btn_h + 12.0;

    let rect_template = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
    let (clicked_template, _) =
        ui_button(rect_template, "Build Template", ctx.mouse, ctx.font_md, ctx.button_colors);
    if clicked_template {
        *cam_offset = Vec2::ZERO;
        *cam_zoom = config.zoom_level;
        template_blocks.clear();
        *scene = Scene::BuildTemplate;
    }

    let rect_exit = Rect::new((screen_width() - btn_w) * 0.5, y + btn_h + 12.0, btn_w, btn_h);
    let (clicked_exit, _) = ui_button(rect_exit, "Salir", ctx.mouse, ctx.font_md, ctx.button_colors);
    if clicked_exit {
        if !blocks.is_empty() || !tiles.is_empty() {
            save_map(map_path, blocks, tiles, units);
        }
        return true;
    }

    false
}

