use macroquad::prelude::*;

use crate::core::{
    generate_tiles, save_config, AppColors, AppConfig, Axial, ConfigData, FrameContext, Scene,
    TileData,
};
use crate::core::ui::{
    adjust_color_channel, color_target_list, color_target_mut, color_target_name, ui_button,
};
use crate::GRID_RADIUS;
use std::collections::HashMap;

// Render and handle input for the config scene.
pub fn run(
    ctx: &FrameContext,
    config: &mut AppConfig,
    colors: &mut AppColors,
    color_target_index: &mut usize,
    tiles: &mut HashMap<Axial, TileData>,
    dirty: &mut bool,
    scene: &mut Scene,
    config_path: &str,
) {
    let title = "Configuracion";
    let title_size = ctx.font_lg;
    let title_dim = measure_text(title, None, title_size as u16, 1.0);
    let mut changed = false;
    draw_text(
        title,
        (screen_width() - title_dim.width) * 0.5,
        120.0,
        title_size,
        ctx.colors_rt.text_primary,
    );

    let label_x = 100.0;
    let mut y = 200.0;
    let btn_w = 40.0;
    let btn_h = 34.0;
    let btn_gap = 8.0;
    let btn_x = 330.0;
    let value_x = btn_x + btn_w * 2.0 + btn_gap + 12.0;
    let row_gap = 44.0;

    let stepper_text = ctx.colors_rt.text_secondary;
    let mut draw_stepper = |label: &str, value: &str, y: f32| -> (bool, bool) {
        draw_text(label, label_x, y, ctx.font_md, stepper_text);
        let rect_dec = Rect::new(btn_x, y - 24.0, btn_w, btn_h);
        let rect_inc = Rect::new(btn_x + btn_w + btn_gap, y - 24.0, btn_w, btn_h);
        let (dec, _) = ui_button(rect_dec, "-", ctx.mouse, ctx.font_md, ctx.button_colors);
        let (inc, _) = ui_button(rect_inc, "+", ctx.mouse, ctx.font_md, ctx.button_colors);
        draw_text(value, value_x, y, ctx.font_md, stepper_text);
        (dec, inc)
    };

    let (dec, inc) = draw_stepper("Velocidad de zoom", &format!("{:.2}", config.zoom_speed), y);
    if dec {
        changed = true;
        config.zoom_speed = (config.zoom_speed - 0.02).max(0.1);
    }
    if inc {
        changed = true;
        config.zoom_speed = (config.zoom_speed + 0.02).min(1.0);
    }
    y += row_gap;

    let (dec, inc) = draw_stepper("Escala de texto", &format!("{:.2}", config.text_scale), y);
    if dec {
        changed = true;
        config.text_scale = (config.text_scale - 0.05).max(0.7);
    }
    if inc {
        changed = true;
        config.text_scale = (config.text_scale + 0.05).min(1.6);
    }
    y += row_gap;

    let (dec, inc) = draw_stepper("Grosor de lineas", &format!("{:.2}", config.line_thickness), y);
    if dec {
        changed = true;
        config.line_thickness = (config.line_thickness - 0.25).max(0.5);
    }
    if inc {
        changed = true;
        config.line_thickness = (config.line_thickness + 0.25).min(4.0);
    }
    y += row_gap;

    let (dec, inc) = draw_stepper("Escala flechas", &format!("{:.2}", config.arrow_scale), y);
    if dec {
        changed = true;
        config.arrow_scale = (config.arrow_scale - 0.1).max(0.5);
    }
    if inc {
        changed = true;
        config.arrow_scale = (config.arrow_scale + 0.1).min(2.0);
    }
    y += row_gap;

    let (dec, inc) = draw_stepper("Clusters tiles", &format!("{}", config.tile_clusters), y);
    if dec {
        changed = true;
        config.tile_clusters = config.tile_clusters.saturating_sub(5).max(5);
    }
    if inc {
        changed = true;
        config.tile_clusters = (config.tile_clusters + 5).min(300);
    }
    y += row_gap;

    let (dec, inc) = draw_stepper("Cluster min", &format!("{}", config.tile_cluster_min), y);
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

    let (dec, inc) = draw_stepper("Cluster max", &format!("{}", config.tile_cluster_max), y);
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

    let (dec, inc) = draw_stepper("Chance vecinos", &format!("{:.2}", config.tile_neighbor_chance), y);
    if dec {
        changed = true;
        config.tile_neighbor_chance = (config.tile_neighbor_chance - 0.05).max(0.05);
    }
    if inc {
        changed = true;
        config.tile_neighbor_chance = (config.tile_neighbor_chance + 0.05).min(0.95);
    }
    y += row_gap;

    let (dec, inc) = draw_stepper("Bonus centro", &format!("{:.2}", config.tile_center_bonus), y);
    if dec {
        changed = true;
        config.tile_center_bonus = (config.tile_center_bonus - 0.05).max(0.0);
    }
    if inc {
        changed = true;
        config.tile_center_bonus = (config.tile_center_bonus + 0.05).min(1.0);
    }
    y += row_gap;

    let (dec, inc) = draw_stepper("Peso piedra", &format!("{:.2}", config.weight_piedra), y);
    if dec {
        changed = true;
        config.weight_piedra = (config.weight_piedra - 0.05).max(0.0);
    }
    if inc {
        changed = true;
        config.weight_piedra = (config.weight_piedra + 0.05).min(1.0);
    }
    y += row_gap;

    let (dec, inc) = draw_stepper("Peso hierro", &format!("{:.2}", config.weight_hierro), y);
    if dec {
        changed = true;
        config.weight_hierro = (config.weight_hierro - 0.05).max(0.0);
    }
    if inc {
        changed = true;
        config.weight_hierro = (config.weight_hierro + 0.05).min(1.0);
    }
    y += row_gap;

    let (dec, inc) = draw_stepper("Peso cobre", &format!("{:.2}", config.weight_cobre), y);
    if dec {
        changed = true;
        config.weight_cobre = (config.weight_cobre - 0.05).max(0.0);
    }
    if inc {
        changed = true;
        config.weight_cobre = (config.weight_cobre + 0.05).min(1.0);
    }
    y += row_gap;

    let (dec, inc) = draw_stepper("Peso agua", &format!("{:.2}", config.weight_agua), y);
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
    let (regen, _) = ui_button(rect_regen, "Regenerar tiles", ctx.mouse, ctx.font_md, ctx.button_colors);
    if regen {
        *tiles = generate_tiles(GRID_RADIUS, config);
        *dirty = true;
    }

    let color_targets = color_target_list();
    let rect_prev = Rect::new(380.0, y - 16.0, 32.0, 38.0);
    let rect_next = Rect::new(420.0, y - 16.0, 32.0, 38.0);
    let (prev, _) = ui_button(rect_prev, "<", ctx.mouse, ctx.font_md, ctx.button_colors);
    let (next, _) = ui_button(rect_next, ">", ctx.mouse, ctx.font_md, ctx.button_colors);
    if prev {
        if *color_target_index == 0 {
            *color_target_index = color_targets.len() - 1;
        } else {
            *color_target_index -= 1;
        }
    }
    if next {
        *color_target_index = (*color_target_index + 1) % color_targets.len();
    }
    let target = color_targets[*color_target_index % color_targets.len()];
    let target_name = color_target_name(target);
    draw_text(
        &format!("Color: {}", target_name),
        470.0,
        y + 10.0,
        ctx.font_md,
        ctx.colors_rt.text_secondary,
    );
    y += row_gap;

    let text_secondary = ctx.colors_rt.text_secondary;
    let color = color_target_mut(target, colors);
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
        ctx.colors_rt.panel_border,
    );
    for (idx, (label, value)) in channels.iter().enumerate() {
        let row_y = y + idx as f32 * row_gap;
        draw_text(label, label_x, row_y, ctx.font_md, text_secondary);
        let rect_dec = Rect::new(btn_x, row_y - 24.0, btn_w, btn_h);
        let rect_inc = Rect::new(btn_x + btn_w + btn_gap, row_y - 24.0, btn_w, btn_h);
        let (dec, _) = ui_button(rect_dec, "-", ctx.mouse, ctx.font_md, ctx.button_colors);
        let (inc, _) = ui_button(rect_inc, "+", ctx.mouse, ctx.font_md, ctx.button_colors);
        if dec {
            changed = true;
            adjust_color_channel(color, idx, -8);
        }
        if inc {
            changed = true;
            adjust_color_channel(color, idx, 8);
        }
        draw_text(&format!("{}", value), value_x, row_y, ctx.font_md, text_secondary);
    }

    if changed {
        save_config(
            config_path,
            &ConfigData {
                config: *config,
                colors: *colors,
            },
        );
    }

    let rect_back = Rect::new(100.0, screen_height() - 80.0, 180.0, 48.0);
    let (back, _) = ui_button(rect_back, "Volver", ctx.mouse, ctx.font_md, ctx.button_colors);
    if back {
        *scene = Scene::MainMenu;
    }
}
