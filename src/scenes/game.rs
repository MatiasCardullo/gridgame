use macroquad::prelude::*;
use std::collections::HashMap;

use crate::core::{
    axial_neighbors, block_color, block_ports, draw_hex_filled, draw_hex_outline, draw_port_marker,
    generate_tiles, hex_distance, hex_to_pixel, load_map, pixel_to_hex, save_map, tile_color,
    add_item, item_from_tile, new_placed_block, AppConfig, Axial, BlockType, FrameContext,
    ItemType, PlacedBlock, RuntimeColors, Scene, TileData, TileType,
};
use crate::core::ui::{draw_window, ui_button, WindowState, WindowStyle};
use crate::{GRID_RADIUS, HEX_SIZE};

// Friendly label for a block type.
fn block_label(kind: BlockType) -> &'static str {
    match kind {
        BlockType::Vivienda => "Vivienda",
        BlockType::Fabrica => "Fabrica",
        BlockType::Mina => "Mina",
        BlockType::Almacen => "Almacen",
        BlockType::Logistica => "Logistica",
        BlockType::Ruta => "Ruta",
    }
}

// Friendly label for an item type.
fn item_label(kind: ItemType) -> &'static str {
    match kind {
        ItemType::Piedra => "Piedra",
        ItemType::Hierro => "Hierro",
        ItemType::Cobre => "Cobre",
        ItemType::Agua => "Agua",
    }
}

// Friendly label for a tile kind.
fn tile_label(kind: TileType) -> &'static str {
    match kind {
        TileType::Piedra => "Piedra",
        TileType::Hierro => "Hierro",
        TileType::Cobre => "Cobre",
        TileType::Agua => "Agua",
    }
}

// Sum all stored items.
fn storage_total(stored: &[crate::core::ItemStack]) -> i32 {
    stored.iter().map(|s| s.amount).sum()
}

// Find a path from start to end traveling only along route tiles (plus endpoints).
fn find_route_path(start: Axial, end: Axial, blocks: &HashMap<Axial, PlacedBlock>) -> Option<Vec<Axial>> {
    if start == end {
        return Some(vec![start]);
    }
    let mut queue = std::collections::VecDeque::new();
    let mut came_from: HashMap<Axial, Axial> = HashMap::new();
    queue.push_back(start);
    came_from.insert(start, start);

    while let Some(current) = queue.pop_front() {
        for neighbor in axial_neighbors(current) {
            if came_from.contains_key(&neighbor) {
                continue;
            }
            let passable = neighbor == end
                || blocks
                    .get(&neighbor)
                    .map(|b| b.kind == BlockType::Ruta)
                    .unwrap_or(false);
            if !passable {
                continue;
            }
            came_from.insert(neighbor, current);
            if neighbor == end {
                let mut path = vec![end];
                let mut step = current;
                while step != start {
                    path.push(step);
                    step = *came_from.get(&step).unwrap();
                }
                path.push(start);
                path.reverse();
                return Some(path);
            }
            queue.push_back(neighbor);
        }
    }
    None
}

// Render and handle input for the game scene.
pub fn run(
    ctx: &FrameContext,
    blocks: &mut HashMap<Axial, PlacedBlock>,
    tiles: &mut HashMap<Axial, TileData>,
    cam_offset: &mut Vec2,
    cam_zoom: &mut f32,
    dragging: &mut bool,
    last_mouse: &mut Vec2,
    selected: &mut Option<BlockType>,
    placement_rotation: &mut u8,
    panel_collapsed: &mut bool,
    window: &mut WindowState,
    units: &mut Vec<crate::core::Unit>,
    unit_spawn_from: &mut Option<Axial>,
    dirty: &mut bool,
    scene: &mut Scene,
    map_path: &str,
    config: &AppConfig,
    colors: &RuntimeColors,
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

    if window.open && window.rect.contains(ctx.mouse) {
        ui_capturing = true;
    }

    if is_mouse_button_pressed(MouseButton::Left) && !ui_capturing {
        if let Some(source) = *unit_spawn_from {
            if let Some(target_block) = blocks.get(&hover_hex) {
                if hover_hex != source {
                    if let Some(path) = find_route_path(source, hover_hex, blocks) {
                        units.push(crate::core::Unit {
                            path,
                            index: 0,
                            progress: 0.0,
                            speed: 3.0,
                        });
                    }
                }
                window.title = block_label(target_block.kind).to_string();
                window.rect = Rect::new(ctx.mouse.x.max(8.0), ctx.mouse.y.max(8.0), 220.0, 120.0);
                window.open = true;
                window.target = Some(hover_hex);
                *unit_spawn_from = None;
            }
        } else if let Some(existing) = blocks.get(&hover_hex) {
            let win_w = 220.0;
            let win_h = 120.0;
            let mut x = ctx.mouse.x + 12.0;
            let mut y = ctx.mouse.y + 12.0;
            if x + win_w > screen_width() {
                x = screen_width() - win_w - 8.0;
            }
            if y + win_h > screen_height() {
                y = screen_height() - win_h - 8.0;
            }
            window.title = block_label(existing.kind).to_string();
            window.rect = Rect::new(x.max(8.0), y.max(8.0), win_w, win_h);
            window.open = true;
            window.target = Some(hover_hex);
        }
    }

    if is_mouse_button_pressed(MouseButton::Right) && !ui_capturing {
        if hex_distance(hover_hex, Axial { q: 0, r: 0 }) <= GRID_RADIUS {
            if let Some(kind) = *selected {
                let can_mine = tiles
                    .get(&hover_hex)
                    .map(|t| matches!(t.kind, TileType::Piedra | TileType::Hierro | TileType::Cobre) && t.amount > 0)
                    .unwrap_or(false);
                if kind != BlockType::Mina || can_mine {
                    let rotation = if kind == BlockType::Ruta { 0 } else { *placement_rotation };
                    blocks.insert(hover_hex, new_placed_block(kind, rotation));
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

    let dt = get_frame_time();
    for (hex, block) in blocks.iter_mut() {
        if block.kind != BlockType::Mina {
            continue;
        }
        if let Some(tile) = tiles.get_mut(hex) {
            if tile.amount <= 0 {
                continue;
            }
            block.mine_progress += dt;
            while block.mine_progress >= 1.0 && tile.amount > 0 {
                let added = add_item(
                    &mut block.stored,
                    item_from_tile(tile.kind),
                    1,
                    block.capacity,
                );
                if added <= 0 {
                    break;
                }
                tile.amount -= added;
                block.mine_progress -= added as f32;
            }
        }
    }

    for unit in units.iter_mut() {
        if unit.path.len() < 2 || unit.index + 1 >= unit.path.len() {
            continue;
        }
        unit.progress += dt * unit.speed;
        while unit.progress >= 1.0 && unit.index + 1 < unit.path.len() - 1 {
            unit.progress -= 1.0;
            unit.index += 1;
        }
    }
    units.retain(|u| u.path.len() >= 2 && u.index + 1 < u.path.len());

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
                draw_hex_filled(
                    center,
                    (HEX_SIZE - 4.5) * *cam_zoom,
                    tile_color(tile.kind, colors),
                );
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

    for unit in units.iter() {
        if unit.path.len() < 2 || unit.index + 1 >= unit.path.len() {
            continue;
        }
        let a = hex_to_pixel(unit.path[unit.index], HEX_SIZE, Vec2::ZERO);
        let b = hex_to_pixel(unit.path[unit.index + 1], HEX_SIZE, Vec2::ZERO);
        let t = unit.progress.clamp(0.0, 1.0);
        let world = a.lerp(b, t);
        let center = ctx.screen_center + *cam_offset + world * *cam_zoom;
        draw_circle(center.x, center.y, 3.5 * *cam_zoom, colors.port_out);
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

    let buttons: [(Option<BlockType>, &str); 6] = [
        (None, "Deconstruir"),
        (Some(BlockType::Vivienda), "Vivienda"),
        (Some(BlockType::Fabrica), "Fabrica"),
        (Some(BlockType::Mina), "Minas"),
        (Some(BlockType::Almacen), "Almacen"),
        (Some(BlockType::Ruta), "Ruta"),
    ];
    let panel_result = crate::core::ui::draw_build_panel(
        panel_pos,
        panel_size,
        *panel_collapsed,
        ctx.mouse,
        config.line_thickness,
        colors,
        &buttons,
        *selected,
    );
    if panel_result.toggle_hovered {
        ui_capturing = true;
    }
    if panel_result.toggled {
        *panel_collapsed = !*panel_collapsed;
        ui_capturing = true;
    }
    if let Some(tip) = panel_result.hovered_tip {
        tooltip = Some(tip);
        ui_capturing = true;
    }
    if let Some(option) = panel_result.clicked_option {
        *selected = option;
    }

    if window.open {
        let style = WindowStyle {
            bg: colors.panel_bg,
            border: colors.panel_border,
            title: colors.text_primary,
            title_bg: colors.button_base,
        };
        if draw_window(window, style, ctx.font_sm, config.line_thickness, ctx.mouse) {
            window.open = false;
            window.target = None;
        }
        if let Some(target) = window.target {
            if let Some(block) = blocks.get(&target) {
                let content_x = window.rect.x + 10.0;
                let mut content_y = window.rect.y + 48.0;
                match block.kind {
                    BlockType::Logistica => {
                        let rect_spawn = Rect::new(content_x, content_y, 150.0, 28.0);
                        let (clicked, _) = ui_button(
                            rect_spawn,
                            "Crear unidad",
                            ctx.mouse,
                            ctx.font_sm,
                            ctx.button_colors,
                        );
                        if clicked {
                            *unit_spawn_from = Some(target);
                        }
                    }
                    BlockType::Mina => {
                        if let Some(tile) = tiles.get(&target) {
                            let txt =
                                format!("Recurso: {} ({})", tile_label(tile.kind), tile.amount);
                            draw_text(&txt, content_x, content_y, ctx.font_sm, colors.text_secondary);
                            content_y += 20.0;
                        }
                        let total = storage_total(&block.stored);
                        draw_text(
                            &format!("Guardado: {}/{}", total, block.capacity),
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                    }
                    BlockType::Almacen => {
                        let total = storage_total(&block.stored);
                        draw_text(
                            &format!("Capacidad: {}/{}", total, block.capacity),
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 20.0;
                        for stack in block.stored.iter() {
                            draw_text(
                                &format!("{}: {}", item_label(stack.kind), stack.amount),
                                content_x,
                                content_y,
                                ctx.font_sm,
                                colors.text_secondary,
                            );
                            content_y += 18.0;
                        }
                    }
                    _ => {}
                }
            }
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
