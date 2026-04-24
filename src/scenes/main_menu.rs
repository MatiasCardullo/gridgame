use macroquad::prelude::*;

use crate::core::ui::{draw_log_panel, draw_progress_bar, ui_button};
use crate::core::{Axial, FrameContext, PlacedBlock, Scene, TileData, Unit, save_map};
use std::collections::HashMap;

// Render and handle input for the main menu scene.
pub fn run(
    ctx: &FrameContext,
    blocks: &mut HashMap<Axial, PlacedBlock>,
    tiles: &mut HashMap<Axial, TileData>,
    units: &mut Vec<Unit>,
    scene: &mut Scene,
    map_path: &str,
    planet_loading: bool,
    planet_ready: bool,
    planet_progress: f32,
    planet_log: &[String],
    planet_error: Option<&str>,
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

    let rect_planet = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
    let planet_label = if planet_ready {
        "Planet"
    } else if planet_loading {
        "Planet (cargando)"
    } else {
        "Planet"
    };
    let (clicked_planet, _) = ui_button(
        rect_planet,
        planet_label,
        ctx.mouse,
        ctx.font_md,
        ctx.button_colors,
    );
    if clicked_planet {
        *scene = Scene::Planet;
    }
    y += btn_h + 12.0;

    let rect_cad = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
    let (clicked_cad, _) = ui_button(
        rect_cad,
        "CAD Prototype",
        ctx.mouse,
        ctx.font_md,
        ctx.button_colors,
    );
    if clicked_cad {
        *scene = Scene::CadPrototype;
    }
    y += btn_h + 12.0;

    let rect_cfg = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
    let (clicked_cfg, _) = ui_button(
        rect_cfg,
        "Configuracion",
        ctx.mouse,
        ctx.font_md,
        ctx.button_colors,
    );
    if clicked_cfg {
        *scene = Scene::Config;
    }
    y += btn_h + 12.0;

    let rect_exit = Rect::new((screen_width() - btn_w) * 0.5, y, btn_w, btn_h);
    let (clicked_exit, _) = ui_button(
        rect_exit,
        "Salir",
        ctx.mouse,
        ctx.font_md,
        ctx.button_colors,
    );
    if clicked_exit {
        if !blocks.is_empty() || !tiles.is_empty() {
            save_map(map_path, blocks, tiles, units);
        }
        return true;
    }

    draw_planet_loading_panel(
        ctx,
        planet_loading,
        planet_ready,
        planet_progress,
        planet_log,
        planet_error,
    );

    false
}

fn draw_planet_loading_panel(
    ctx: &FrameContext,
    loading: bool,
    ready: bool,
    progress: f32,
    log_lines: &[String],
    error: Option<&str>,
) {
    if ready && !loading && error.is_none() {
        return;
    }
    let panel = Rect::new(24.0, screen_height() - 220.0, 420.0, 190.0);
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, ctx.colors_rt.panel_bg);
    draw_rectangle_lines(
        panel.x,
        panel.y,
        panel.w,
        panel.h,
        1.5,
        ctx.colors_rt.panel_border,
    );
    draw_text(
        "Cargando planeta",
        panel.x + 12.0,
        panel.y + 24.0,
        ctx.font_md,
        ctx.colors_rt.text_primary,
    );

    let bar_rect = Rect::new(panel.x + 12.0, panel.y + 40.0, panel.w - 24.0, 16.0);
    draw_progress_bar(
        bar_rect,
        progress,
        ctx.colors_rt.button_hover,
        ctx.colors_rt.button_base,
    );

    let log_rect = Rect::new(
        panel.x + 12.0,
        panel.y + 64.0,
        panel.w - 24.0,
        panel.h - 76.0,
    );
    if let Some(message) = error {
        let lines = vec![message.to_string()];
        draw_log_panel(
            log_rect,
            &lines,
            ctx.font_sm,
            ctx.colors_rt.tooltip_bg,
            ctx.colors_rt.tooltip_border,
            ctx.colors_rt.text_primary,
        );
    } else {
        draw_log_panel(
            log_rect,
            log_lines,
            ctx.font_sm,
            ctx.colors_rt.tooltip_bg,
            ctx.colors_rt.tooltip_border,
            ctx.colors_rt.text_secondary,
        );
    }
}
