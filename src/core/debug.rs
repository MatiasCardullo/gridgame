use macroquad::prelude::*;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::ui::ui_button;
use crate::core::FrameContext;
use crate::scenes::planet::PlanetState;

pub const PLANET_DEBUG_UI_ENABLED_DEFAULT: bool = true;
pub const PLANET_PERF_LOG_PATH: &str = "planet_data/planet_perf.log";
pub const SAVE_MESH_POINTS_RUNTIME: bool = false;

pub fn format_log_timestamp(now: SystemTime) -> String {
    let since_epoch = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = since_epoch % 86_400;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("[{:02}:{:02}:{:02}]", h, m, s)
}

pub fn planet_perf_log(line: &str) {
    let _ = fs::create_dir_all("planet_data");
    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(PLANET_PERF_LOG_PATH)
    else {
        return;
    };
    let _ = writeln!(file, "{} {}", format_log_timestamp(SystemTime::now()), line);
}

pub fn draw_planet_controls(ctx: &FrameContext, state: &mut PlanetState, noise_path: &str) {
    let panel = Rect::new(18.0, 18.0, 260.0, 254.0);
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, ctx.colors_rt.panel_bg);
    draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 1.5, ctx.colors_rt.panel_border);
    draw_text(
        "Planet Terrain",
        panel.x + 12.0,
        panel.y + 24.0,
        ctx.font_md,
        ctx.colors_rt.text_primary,
    );

    let mut y = panel.y + 54.0;
    let x = panel.x + 12.0;
    let value_x = panel.x + panel.w - 90.0;
    let step = 26.0;

    let mut changed = false;
    changed |= draw_adjust_row(
        ctx,
        "Noise",
        &mut state.config.noise_scale,
        0.2,
        0.5,
        5.0,
        x,
        value_x,
        y,
    );
    y += step;
    changed |= draw_adjust_row(
        ctx,
        "Amp",
        &mut state.config.height_amp,
        0.1,
        0.5,
        2.5,
        x,
        value_x,
        y,
    );
    y += step;
    changed |= draw_adjust_row(
        ctx,
        "Bias",
        &mut state.config.height_bias,
        0.05,
        -1.0,
        1.0,
        x,
        value_x,
        y,
    );
    y += step;
    changed |= draw_adjust_row(
        ctx,
        "Lat",
        &mut state.config.lat_bias,
        0.05,
        -0.5,
        0.5,
        x,
        value_x,
        y,
    );
    y += step;
    changed |= draw_adjust_row(
        ctx,
        "Sea",
        &mut state.config.sea_level,
        0.02,
        0.2,
        0.9,
        x,
        value_x,
        y,
    );
    y += step;
    changed |= draw_adjust_row(
        ctx,
        "Ice",
        &mut state.config.ice_start,
        0.02,
        0.2,
        0.9,
        x,
        value_x,
        y,
    );
    y += step;
    changed |= draw_adjust_row(
        ctx,
        "IcePow",
        &mut state.config.ice_strength,
        0.1,
        0.5,
        3.0,
        x,
        value_x,
        y,
    );
    y += step + 6.0;

    let rect_toggle = Rect::new(x, y, 230.0, 26.0);
    let (clicked_toggle, _) = ui_button(
        rect_toggle,
        if state.show_relief {
            "Relief: On"
        } else {
            "Relief: Off"
        },
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    if clicked_toggle {
        state.show_relief = !state.show_relief;
    }
    y += step;

    let rect_load = Rect::new(x, y, 110.0, 26.0);
    let (clicked_load, _) = ui_button(rect_load, "Load JSON", ctx.mouse, ctx.font_sm, ctx.button_colors);
    let rect_save = Rect::new(x + 120.0, y, 110.0, 26.0);
    let (clicked_save, _) = ui_button(rect_save, "Save JSON", ctx.mouse, ctx.font_sm, ctx.button_colors);

    if changed {
        planet_perf_log("panel changed -> clear relief cache and regenerate");
        state.debug_on_controls_changed();
    }
    if clicked_load {
        planet_perf_log("load json clicked");
        state.debug_load_heightmap(noise_path);
    }
    if clicked_save {
        state.debug_save_heightmap(noise_path);
    }
}

fn draw_adjust_row(
    ctx: &FrameContext,
    label: &str,
    value: &mut f32,
    step: f32,
    min: f32,
    max: f32,
    x: f32,
    value_x: f32,
    y: f32,
) -> bool {
    draw_text(label, x, y, ctx.font_sm, ctx.colors_rt.text_secondary);
    let rect_minus = Rect::new(value_x, y - 16.0, 22.0, 20.0);
    let rect_plus = Rect::new(value_x + 54.0, y - 16.0, 22.0, 20.0);
    let (clicked_minus, _) = ui_button(rect_minus, "-", ctx.mouse, ctx.font_sm, ctx.button_colors);
    let (clicked_plus, _) = ui_button(rect_plus, "+", ctx.mouse, ctx.font_sm, ctx.button_colors);
    if clicked_minus {
        *value = (*value - step).max(min);
    }
    if clicked_plus {
        *value = (*value + step).min(max);
    }
    draw_text(
        &format!("{:.2}", *value),
        value_x + 26.0,
        y,
        ctx.font_sm,
        ctx.colors_rt.text_secondary,
    );
    clicked_minus || clicked_plus
}
