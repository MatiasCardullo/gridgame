use macroquad::prelude::*;
use std::collections::HashMap;

use crate::{
    Axial, BlockType, FrameContext, PlacedBlock, Scene, TileType, GRID_RADIUS, HEX_SIZE,
};
use crate::core::{
    block_color, block_ports, draw_hex_filled, draw_hex_outline, draw_port_marker, generate_tiles,
    hex_distance, hex_to_pixel, load_map, pixel_to_hex, save_map, tile_color, axial_neighbors,
};

// Render and handle input for the game scene.
pub fn run(
    ctx: &FrameContext,
    blocks: &mut HashMap<Axial, PlacedBlock>,
    tiles: &mut HashMap<Axial, TileType>,
    cam_offset: &mut Vec2,
    cam_zoom: &mut f32,
    dragging: &mut bool,
    last_mouse: &mut Vec2,
    selected: &mut Option<BlockType>,
    placement_rotation: &mut u8,
    panel_collapsed: &mut bool,
    dirty: &mut bool,
    scene: &mut Scene,
    map_path: &str,
    config: &crate::AppConfig,
    colors: &crate::RuntimeColors,
) {
    let wheel = mouse_wheel().1;
    if wheel.abs() > 0.01 {
        let factor = 1.0 + wheel * config.zoom_speed;
        *cam_zoom = (*cam_zoom * factor).clamp(0.3, 3.0);
    }

    if is_mouse_button_pressed(MouseButton::Middle) {
        *dragging = true;
        *last_mouse = ctx.mouse;
    }
    if is_mouse_button_down(MouseButton::Middle) && *dragging {
        let delta = ctx.mouse - *last_mouse;
        *cam_offset += delta;
        *last_mouse = ctx.mouse;
    }
    if is_mouse_button_released(MouseButton::Middle) {
        *dragging = false;
    }

    let world_mouse = (ctx.mouse - ctx.screen_center - *cam_offset) / *cam_zoom;
    let hover_hex = pixel_to_hex(world_mouse, HEX_SIZE, Vec2::ZERO);

    let panel_pos = vec2(16.0, screen_height() - 110.0);
    let panel_size = if *panel_collapsed {
        vec2(170.0, 36.0)
    } else {
        vec2(392.0, 94.0)
    };
    let panel_rect = Rect::new(panel_pos.x, panel_pos.y, panel_size.x, panel_size.y);

    let mut tooltip: Option<&str> = None;
    let mut ui_capturing = panel_rect.contains(ctx.mouse);

    if is_mouse_button_pressed(MouseButton::Left) && !ui_capturing {
        if hex_distance(hover_hex, Axial { q: 0, r: 0 }) <= GRID_RADIUS {
            if let Some(kind) = *selected {
                let can_mine = matches!(
                    tiles.get(&hover_hex),
                    Some(TileType::Piedra) | Some(TileType::Hierro) | Some(TileType::Cobre)
                );
                if kind != BlockType::Mina || can_mine {
                    let rotation = if kind == BlockType::Ruta { 0 } else { *placement_rotation };
                    blocks.insert(
                        hover_hex,
                        PlacedBlock {
                            kind,
                            rotation,
                        },
                    );
                    *dirty = true;
                }
            } else if blocks.remove(&hover_hex).is_some() {
                *dirty = true;
            }
        }
    }

    if is_key_pressed(KeyCode::S) {
        save_map(map_path, blocks, tiles);
        *dirty = false;
    }

    if is_key_pressed(KeyCode::L) {
        let (loaded_blocks, loaded_tiles) = load_map(map_path);
        *blocks = loaded_blocks;
        *tiles = loaded_tiles;
        if tiles.is_empty() {
            *tiles = generate_tiles(GRID_RADIUS, config);
        }
        *dirty = false;
    }

    if *dirty {
        save_map(map_path, blocks, tiles);
        *dirty = false;
    }

    if is_key_pressed(KeyCode::Escape) {
        save_map(map_path, blocks, tiles);
        *scene = Scene::MainMenu;
    }

    if is_key_pressed(KeyCode::R) {
        if let Some(block) = blocks.get_mut(&hover_hex) {
            if block.kind != BlockType::Ruta {
                block.rotation = (block.rotation + 1) % 6;
                *dirty = true;
            }
        } else {
            *placement_rotation = (*placement_rotation + 1) % 6;
        }
    }

    for r in -GRID_RADIUS..=GRID_RADIUS {
        for q in -GRID_RADIUS..=GRID_RADIUS {
            let hex = Axial { q, r };
            if hex_distance(hex, Axial { q: 0, r: 0 }) > GRID_RADIUS {
                continue;
            }
            let world_center = hex_to_pixel(hex, HEX_SIZE, Vec2::ZERO);
            let center = ctx.screen_center + *cam_offset + world_center * *cam_zoom;

            let size = HEX_SIZE * *cam_zoom;
            if center.x < -size
                || center.y < -size
                || center.x > screen_width() + size
                || center.y > screen_height() + size
            {
                continue;
            }

            if let Some(tile) = tiles.get(&hex) {
                draw_hex_filled(center, (HEX_SIZE - 4.5) * *cam_zoom, tile_color(*tile, colors));
            }

            if let Some(placed) = blocks.get(&hex) {
                draw_hex_filled(
                    center,
                    (HEX_SIZE - 2.5) * *cam_zoom,
                    block_color(placed.kind, colors),
                );
                let (inputs, outputs) = block_ports(placed.kind, placed.rotation);
                for dir in inputs {
                    draw_port_marker(
                        center,
                        (HEX_SIZE - 3.0) * *cam_zoom * config.arrow_scale,
                        dir,
                        colors.port_in,
                    );
                }
                for dir in outputs {
                    draw_port_marker(
                        center,
                        (HEX_SIZE - 3.0) * *cam_zoom * config.arrow_scale,
                        dir,
                        colors.port_out,
                    );
                }
            }

            draw_hex_outline(center, size, colors.grid, config.line_thickness);
        }
    }

    for (hex, placed) in blocks.iter() {
        if placed.kind != BlockType::Ruta {
            continue;
        }
        let base_center = ctx.screen_center + *cam_offset + hex_to_pixel(*hex, HEX_SIZE, Vec2::ZERO) * *cam_zoom;
        let neighbors = axial_neighbors(*hex);
        for neighbor in neighbors {
            if blocks.contains_key(&neighbor) {
                let neighbor_center = ctx.screen_center
                    + *cam_offset
                    + hex_to_pixel(neighbor, HEX_SIZE, Vec2::ZERO) * *cam_zoom;
                draw_line(
                    base_center.x,
                    base_center.y,
                    neighbor_center.x,
                    neighbor_center.y,
                    (2.0 + config.line_thickness) * *cam_zoom,
                    colors.route_line,
                );
            }
        }
    }

    let hover_center = ctx.screen_center + *cam_offset + hex_to_pixel(hover_hex, HEX_SIZE, Vec2::ZERO) * *cam_zoom;
    draw_hex_outline(
        hover_center,
        HEX_SIZE * *cam_zoom,
        colors.hover,
        config.line_thickness * 2.0,
    );

    draw_text(
        "Click: colocar  |  Rueda: zoom  |  Boton medio: mover  |  R: rotar  |  Panel: <<  |  S: guardar  |  L: cargar  |  Esc: menu",
        16.0,
        28.0,
        ctx.font_md,
        colors.text_primary,
    );

    let coord_text = format!("Hex: q={} r={}", hover_hex.q, hover_hex.r);
    draw_text(
        &coord_text,
        16.0,
        52.0,
        ctx.font_sm,
        colors.text_secondary,
    );

    let rot_text = format!("Rotacion: {}", placement_rotation);
    draw_text(&rot_text, 16.0, 74.0, ctx.font_sm, colors.text_secondary);

    draw_rectangle(panel_pos.x, panel_pos.y, panel_size.x, panel_size.y, colors.panel_bg);
    draw_rectangle_lines(
        panel_pos.x,
        panel_pos.y,
        panel_size.x,
        panel_size.y,
        config.line_thickness.max(1.0),
        colors.panel_border,
    );

    let toggle_rect = Rect::new(panel_pos.x + panel_size.x - 32.0, panel_pos.y + 6.0, 26.0, 24.0);
    let toggle_hover = toggle_rect.contains(ctx.mouse);
    if toggle_hover {
        ui_capturing = true;
    }
    let toggle_color = if toggle_hover { colors.button_hover } else { colors.button_base };
    draw_rectangle(toggle_rect.x, toggle_rect.y, toggle_rect.w, toggle_rect.h, toggle_color);
    draw_rectangle_lines(
        toggle_rect.x,
        toggle_rect.y,
        toggle_rect.w,
        toggle_rect.h,
        config.line_thickness.max(1.0),
        colors.button_border,
    );
    let toggle_label = if *panel_collapsed { ">>" } else { "<<" };
    let toggle_dim = measure_text(toggle_label, None, ctx.font_sm as u16, 1.0);
    draw_text(
        toggle_label,
        toggle_rect.x + (toggle_rect.w - toggle_dim.width) * 0.5,
        toggle_rect.y + (toggle_rect.h + toggle_dim.height) * 0.5 - 2.0,
        ctx.font_sm,
        colors.button_text,
    );
    if toggle_hover && is_mouse_button_pressed(MouseButton::Left) {
        *panel_collapsed = !*panel_collapsed;
        ui_capturing = true;
    }

    if !*panel_collapsed {
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
            let hover = rect.contains(ctx.mouse);
            if hover {
                tooltip = Some(tip);
                ui_capturing = true;
            }
            let selected_now = *selected == option;

            let base_color = if selected_now { colors.button_hover } else { colors.button_base };
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, base_color);
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                config.line_thickness.max(1.0),
                colors.button_border,
            );

            if hover && is_mouse_button_pressed(MouseButton::Left) {
                *selected = option;
            }

            if let Some(kind) = option {
                let icon_center = vec2(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
                draw_hex_filled(icon_center, 16.0, block_color(kind, colors));
                draw_hex_outline(
                    icon_center,
                    16.0,
                    colors.panel_border,
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
                    colors.port_out,
                );
                draw_line(
                    rect.x + rect.w - pad,
                    rect.y + pad,
                    rect.x + pad,
                    rect.y + rect.h - pad,
                    3.0,
                    colors.port_out,
                );
            }

            if hover {
                draw_rectangle_lines(
                    rect.x - 1.0,
                    rect.y - 1.0,
                    rect.w + 2.0,
                    rect.h + 2.0,
                    config.line_thickness.max(1.0),
                    colors.hover,
                );
            }

            bx += button_size + gap;
        }
    }

    if let Some(tip) = tooltip {
        let pad = 6.0;
        let font_size = ctx.font_sm;
        let dim = measure_text(tip, None, font_size as u16, 1.0);
        let x = (ctx.mouse.x + 14.0).min(screen_width() - dim.width - 2.0 * pad);
        let y = (ctx.mouse.y + 16.0).min(screen_height() - dim.height - 2.0 * pad);
        draw_rectangle(
            x,
            y,
            dim.width + 2.0 * pad,
            dim.height + 2.0 * pad,
            colors.tooltip_bg,
        );
        draw_rectangle_lines(
            x,
            y,
            dim.width + 2.0 * pad,
            dim.height + 2.0 * pad,
            config.line_thickness.max(1.0),
            colors.tooltip_border,
        );
        draw_text(
            tip,
            x + pad,
            y + dim.height + pad - 2.0,
            font_size,
            colors.text_primary,
        );
    }
}
