use macroquad::prelude::*;

use crate::{
    AppConfig, Axial, FrameContext, PlacedBlock, Scene, TileType, GRID_RADIUS,
};
use crate::core::{generate_tiles, load_map, save_map};
use crate::ui::ui_button;
use std::collections::HashMap;

// Render and handle input for the main menu scene.
pub fn run(
    ctx: &FrameContext,
    blocks: &mut HashMap<Axial, PlacedBlock>,
    tiles: &mut HashMap<Axial, TileType>,
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
            let (loaded_blocks, loaded_tiles) = load_map(map_path);
            *blocks = loaded_blocks;
            *tiles = loaded_tiles;
            if tiles.is_empty() {
                *tiles = generate_tiles(GRID_RADIUS, config);
                *dirty = true;
            }
            *scene = Scene::Game;
        }
        y += btn_h + 12.0;
    }

    let rect_new = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
    let (clicked_new, _) = ui_button(rect_new, "Nueva Partida", ctx.mouse, ctx.font_md, ctx.button_colors);
    if clicked_new {
        blocks.clear();
        *tiles = generate_tiles(GRID_RADIUS, config);
        *cam_offset = Vec2::ZERO;
        *cam_zoom = 1.0;
        *placement_rotation = 0;
        *scene = Scene::Game;
    }
    y += btn_h + 12.0;

    let rect_cfg = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
    let (clicked_cfg, _) = ui_button(rect_cfg, "Configuracion", ctx.mouse, ctx.font_md, ctx.button_colors);
    if clicked_cfg {
        *scene = Scene::Config;
    }

    let rect_exit = Rect::new((screen_width() - btn_w) * 0.5, y + btn_h + 12.0, btn_w, btn_h);
    let (clicked_exit, _) = ui_button(rect_exit, "Salir", ctx.mouse, ctx.font_md, ctx.button_colors);
    if clicked_exit {
        if !blocks.is_empty() || !tiles.is_empty() {
            save_map(map_path, blocks, tiles);
        }
        return true;
    }

    false
}
