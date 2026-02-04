use macroquad::prelude::*;
use std::collections::HashMap;

use crate::core::{
    axial_neighbors, block_color, block_ports, draw_hex_filled, draw_hex_outline, draw_port_marker,
    hex_distance, hex_to_pixel, pixel_to_hex, save_map, tile_color,
    add_item, build_requirements, is_under_construction, item_from_tile, new_placed_block, AppConfig, Axial,
    BlockType, FrameContext, ItemType, PlacedBlock, RuntimeColors, Scene, TileData, TileType,
};
use crate::core::ui::{
    draw_window, ui_button, window_close_rect, window_title_rect, WindowState, WindowStyle,
    WINDOW_TITLE_HEIGHT,
};
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

fn mine_targets(hex: Axial, block: &PlacedBlock) -> Vec<Axial> {
    let mut targets = Vec::with_capacity(1 + block.mine_extra.len());
    targets.push(hex);
    targets.extend(block.mine_extra.iter().copied());
    targets
}

fn mine_total_remaining(
    hex: Axial,
    block: &PlacedBlock,
    tiles: &HashMap<Axial, TileData>,
) -> i32 {
    mine_targets(hex, block)
        .iter()
        .filter_map(|h| tiles.get(h))
        .map(|t| t.amount)
        .sum()
}

fn expand_mine_area(
    hex: Axial,
    block: &mut PlacedBlock,
    tiles: &HashMap<Axial, TileData>,
) -> bool {
    let base_tile = match tiles.get(&hex) {
        Some(tile) => tile,
        None => return false,
    };
    let base_kind = base_tile.kind;
    let mut existing: std::collections::HashSet<Axial> =
        block.mine_extra.iter().copied().collect();
    existing.insert(hex);
    let mut to_add: Vec<Axial> = Vec::new();
    for current in existing.iter().copied() {
        for neighbor in axial_neighbors(current) {
            if existing.contains(&neighbor) {
                continue;
            }
            if let Some(tile) = tiles.get(&neighbor) {
                if tile.kind == base_kind {
                    to_add.push(neighbor);
                }
            }
        }
    }
    let mut added = false;
    for hex in to_add {
        if !existing.contains(&hex) {
            block.mine_extra.push(hex);
            existing.insert(hex);
            added = true;
        }
    }
    added
}

fn available_item_amount(
    blocks: &HashMap<Axial, PlacedBlock>,
    kind: ItemType,
) -> i32 {
    blocks
        .values()
        .filter(|b| !is_under_construction(b))
        .flat_map(|b| b.stored.iter())
        .filter(|s| s.kind == kind)
        .map(|s| s.amount)
        .sum()
}

fn has_requirements(blocks: &HashMap<Axial, PlacedBlock>, reqs: &[crate::core::ItemStack]) -> bool {
    reqs.iter()
        .all(|req| available_item_amount(blocks, req.kind) >= req.amount)
}

fn consume_requirements(
    blocks: &mut HashMap<Axial, PlacedBlock>,
    reqs: &[crate::core::ItemStack],
) -> bool {
    if !has_requirements(blocks, reqs) {
        return false;
    }
    for req in reqs {
        let mut remaining = req.amount;
        for block in blocks.values_mut() {
            if remaining <= 0 {
                break;
            }
            if is_under_construction(block) {
                continue;
            }
            let mut index = 0usize;
            while index < block.stored.len() && remaining > 0 {
                if block.stored[index].kind == req.kind {
                    let take = block.stored[index].amount.min(remaining);
                    block.stored[index].amount -= take;
                    remaining -= take;
                    if block.stored[index].amount <= 0 {
                        block.stored.remove(index);
                        continue;
                    }
                }
                index += 1;
            }
        }
    }
    true
}

// Move items from a block storage into unit cargo.
fn load_unit_from_block(
    block: &mut PlacedBlock,
    cargo: &mut Vec<crate::core::ItemStack>,
    capacity: i32,
) {
    let mut remaining = capacity - storage_total(cargo);
    if remaining <= 0 {
        return;
    }
    let mut index = 0usize;
    while index < block.stored.len() && remaining > 0 {
        let stack = block.stored[index];
        let take = stack.amount.min(remaining);
        if take > 0 {
            let added = add_item(cargo, stack.kind, take, capacity);
            if added > 0 {
                block.stored[index].amount -= added;
                remaining -= added;
            }
        }
        if block.stored[index].amount <= 0 {
            block.stored.remove(index);
        } else {
            index += 1;
        }
    }
}

// Move items from unit cargo into a block storage.
fn unload_unit_to_block(cargo: &mut Vec<crate::core::ItemStack>, block: &mut PlacedBlock) {
    let mut index = 0usize;
    while index < cargo.len() {
        let stack = cargo[index];
        let added = add_item(&mut block.stored, stack.kind, stack.amount, block.capacity);
        if added > 0 {
            cargo[index].amount -= added;
        }
        if cargo[index].amount <= 0 {
            cargo.remove(index);
        } else {
            index += 1;
        }
    }
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
                    .map(|b| b.kind == BlockType::Ruta && !is_under_construction(b))
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
    station_in: &mut Option<Axial>,
    station_out: &mut Option<Axial>,
    station_pick: &mut Option<crate::core::StationPick>,
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

    let buttons: [(Option<BlockType>, &str); 7] = [
        (None, "Deconstruir"),
        (Some(BlockType::Vivienda), "Vivienda"),
        (Some(BlockType::Fabrica), "Fabrica"),
        (Some(BlockType::Mina), "Minas"),
        (Some(BlockType::Almacen), "Almacen"),
        (Some(BlockType::Logistica), "Logistica"),
        (Some(BlockType::Ruta), "Ruta"),
    ];

    let button_size = 52.0;
    let gap = 8.0;
    let panel_pad_x = 12.0;
    let panel_pad_y = 16.0;
    let toggle_space = 36.0;
    let button_count = buttons.len() as f32;
    let panel_width = panel_pad_x * 2.0
        + (button_size * button_count)
        + (gap * (button_count - 1.0))
        + toggle_space;
    let panel_height = panel_pad_y * 2.0 + button_size;
    let panel_size = if *panel_collapsed {
        vec2(170.0, 36.0)
    } else {
        vec2(panel_width, panel_height)
    };
    let panel_pos = vec2(16.0, screen_height() - panel_size.y - 16.0);
    let panel_rect = Rect::new(panel_pos.x, panel_pos.y, panel_size.x, panel_size.y);

    let mut tooltip: Option<&str> = None;
    let mut ui_capturing = panel_rect.contains(ctx.mouse);

    if window.open && (window.rect.contains(ctx.mouse) || window.dragging) {
        ui_capturing = true;
    }

    if is_mouse_button_pressed(MouseButton::Left) && !ui_capturing {
        if let Some(pick) = *station_pick {
            if let Some(block) = blocks.get(&hover_hex) {
                if !is_under_construction(block) {
                    match pick {
                        crate::core::StationPick::In => *station_in = Some(hover_hex),
                        crate::core::StationPick::Out => *station_out = Some(hover_hex),
                    }
                    *station_pick = None;
                }
            }
        } else if let Some(existing) = blocks.get(&hover_hex) {
            let win_w = 220.0;
            let win_h = 170.0;
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
                if !blocks.contains_key(&hover_hex) {
                    let can_mine = tiles
                        .get(&hover_hex)
                        .map(|t| matches!(t.kind, TileType::Piedra | TileType::Hierro | TileType::Cobre) && t.amount > 0)
                        .unwrap_or(false);
                    if kind != BlockType::Mina || can_mine {
                        let rotation = if kind == BlockType::Ruta { 0 } else { *placement_rotation };
                        blocks.insert(hover_hex, new_placed_block(kind, rotation));
                        *dirty = true;
                    }
                }
            } else if blocks.remove(&hover_hex).is_some() {
                *dirty = true;
            }
        }
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
    let build_hexes: Vec<Axial> = blocks
        .iter()
        .filter(|(_, block)| is_under_construction(block))
        .map(|(hex, _)| *hex)
        .collect();
    for hex in build_hexes {
        let (kind, needs_pay) = match blocks.get(&hex) {
            Some(block) => (block.kind, !block.build_paid),
            None => continue,
        };
        if needs_pay {
            let reqs = build_requirements(kind);
            let paid = reqs.is_empty() || consume_requirements(blocks, &reqs);
            if paid {
                if let Some(block) = blocks.get_mut(&hex) {
                    block.build_paid = true;
                }
            }
        }
        if let Some(block) = blocks.get_mut(&hex) {
            if block.build_paid {
                block.build_progress += dt;
                if block.build_progress > block.build_time {
                    block.build_progress = block.build_time;
                }
            }
        }
    }
    for (hex, block) in blocks.iter_mut() {
        if block.kind != BlockType::Mina || is_under_construction(block) {
            continue;
        }
        let targets = mine_targets(*hex, block);
        block.mine_progress += dt;
        while block.mine_progress >= 1.0 {
            let mut target_hex: Option<Axial> = None;
            for target in &targets {
                if let Some(tile) = tiles.get(target) {
                    if tile.amount > 0 {
                        target_hex = Some(*target);
                        break;
                    }
                }
            }
            let Some(target_hex) = target_hex else {
                break;
            };
            let tile = tiles.get_mut(&target_hex).unwrap();
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

    for unit in units.iter_mut() {
        if unit.path.len() < 2 {
            continue;
        }
        unit.progress += dt * unit.speed;
        while unit.progress >= 1.0 {
            unit.progress -= 1.0;
            if unit.forward {
                if unit.index + 1 < unit.path.len() {
                    unit.index += 1;
                }
                if unit.index >= unit.path.len() - 1 {
                    unit.forward = false;
                }
            } else {
                if unit.index > 0 {
                    unit.index -= 1;
                }
                if unit.index == 0 {
                    unit.forward = true;
                }
            }
            let arrived = unit.path[unit.index];
            if arrived == unit.station_in {
                if let Some(block) = blocks.get_mut(&arrived) {
                    load_unit_from_block(block, &mut unit.cargo, unit.capacity);
                }
            }
            if arrived == unit.station_out {
                if let Some(block) = blocks.get_mut(&arrived) {
                    unload_unit_to_block(&mut unit.cargo, block);
                }
            }
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
                draw_hex_filled(
                    center,
                    (HEX_SIZE - 4.5) * *cam_zoom,
                    tile_color(tile.kind, colors),
                );
            }

            if let Some(placed) = blocks.get(&hex) {
                let mut color = block_color(placed.kind, colors);
                if is_under_construction(placed) {
                    color.a = 0.35;
                }
                draw_hex_filled(center, (HEX_SIZE - 2.5) * *cam_zoom, color);
                if !is_under_construction(placed) {
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
            }

            draw_hex_outline(center, size, colors.grid, config.line_thickness);
        }
    }

    for (hex, placed) in blocks.iter() {
        if placed.kind != BlockType::Ruta || is_under_construction(placed) {
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
        if unit.path.len() < 2 {
            continue;
        }
        let next_index = if unit.forward {
            if unit.index + 1 < unit.path.len() {
                unit.index + 1
            } else {
                unit.index
            }
        } else if unit.index > 0 {
            unit.index - 1
        } else {
            unit.index
        };
        let a = hex_to_pixel(unit.path[unit.index], HEX_SIZE, Vec2::ZERO);
        let b = hex_to_pixel(unit.path[next_index], HEX_SIZE, Vec2::ZERO);
        let t = unit.progress.clamp(0.0, 1.0);
        let world = a.lerp(b, t);
        let center = ctx.screen_center + *cam_offset + world * *cam_zoom;
        draw_circle(center.x, center.y, 3.5 * *cam_zoom, colors.port_out);
    }

    let hover_center = ctx.screen_center + *cam_offset + hex_to_pixel(hover_hex, HEX_SIZE, Vec2::ZERO) * *cam_zoom;
    if let Some(kind) = *selected {
        let in_bounds = hex_distance(hover_hex, Axial { q: 0, r: 0 }) <= GRID_RADIUS;
        let empty = !blocks.contains_key(&hover_hex);
        let can_mine = tiles
            .get(&hover_hex)
            .map(|t| matches!(t.kind, TileType::Piedra | TileType::Hierro | TileType::Cobre) && t.amount > 0)
            .unwrap_or(false);
        let can_place = in_bounds && empty && (kind != BlockType::Mina || can_mine);
        if can_place {
            let mut ghost = block_color(kind, colors);
            ghost.a = 0.3;
            draw_hex_filled(
                hover_center,
                (HEX_SIZE - 2.5) * *cam_zoom,
                ghost,
            );
        }
    }
    draw_hex_outline(
        hover_center,
        HEX_SIZE * *cam_zoom,
        colors.hover,
        config.line_thickness * 2.0,
    );

    draw_text(
        "Click: colocar  |  Rueda: zoom  |  Boton medio: mover  |  R: rotar  |  Esc: menu",
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
        let title_rect = window_title_rect(window);
        let close_rect = window_close_rect(window);
        if is_mouse_button_pressed(MouseButton::Left)
            && title_rect.contains(ctx.mouse)
            && !close_rect.contains(ctx.mouse)
        {
            window.dragging = true;
            window.drag_offset = ctx.mouse - vec2(window.rect.x, window.rect.y);
        }
        if window.dragging && is_mouse_button_down(MouseButton::Left) {
            let mut x = ctx.mouse.x - window.drag_offset.x;
            let mut y = ctx.mouse.y - window.drag_offset.y;
            if x + window.rect.w > screen_width() {
                x = screen_width() - window.rect.w;
            }
            if y + window.rect.h > screen_height() {
                y = screen_height() - window.rect.h;
            }
            if x < 0.0 {
                x = 0.0;
            }
            if y < 0.0 {
                y = 0.0;
            }
            window.rect.x = x;
            window.rect.y = y;
        }
        if is_mouse_button_released(MouseButton::Left) {
            window.dragging = false;
        }

        let style = WindowStyle {
            bg: colors.panel_bg,
            border: colors.panel_border,
            title: colors.text_primary,
            title_bg: colors.button_base,
        };
        if draw_window(window, style, ctx.font_sm, config.line_thickness, ctx.mouse) {
            window.open = false;
            window.target = None;
            window.dragging = false;
        }
        if let Some(target) = window.target {
            if let Some(block) = blocks.get_mut(&target) {
                let content_x = window.rect.x + 10.0;
                let mut content_y = window.rect.y + WINDOW_TITLE_HEIGHT + 20.0;
                let under_construction = is_under_construction(block);
                if under_construction {
                    draw_text(
                        "En construccion",
                        content_x,
                        content_y,
                        ctx.font_sm,
                        colors.text_secondary,
                    );
                    content_y += 18.0;
                    let reqs = build_requirements(block.kind);
                    if reqs.is_empty() {
                        draw_text(
                            "Materiales: gratis",
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 18.0;
                    } else {
                        draw_text(
                            "Materiales:",
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 18.0;
                        for req in reqs {
                            draw_text(
                                &format!("- {}: {}", item_label(req.kind), req.amount),
                                content_x,
                                content_y,
                                ctx.font_sm,
                                colors.text_secondary,
                            );
                            content_y += 16.0;
                        }
                        if !block.build_paid {
                            draw_text(
                                "Esperando materiales",
                                content_x,
                                content_y,
                                ctx.font_sm,
                                colors.text_secondary,
                            );
                            content_y += 18.0;
                        }
                    }
                    let remaining = (block.build_time - block.build_progress).max(0.0);
                    draw_text(
                        &format!("Tiempo restante: {:.1}s", remaining),
                        content_x,
                        content_y,
                        ctx.font_sm,
                        colors.text_secondary,
                    );
                    content_y += 22.0;
                }
                match block.kind {
                    BlockType::Logistica => {
                        if under_construction {
                            draw_text(
                                "Disponible al terminar",
                                content_x,
                                content_y,
                                ctx.font_sm,
                                colors.text_secondary,
                            );
                        } else {
                            let a_text = match station_in {
                                Some(a) => format!("In: q={} r={}", a.q, a.r),
                                None => "In: (sin)".to_string(),
                            };
                            let b_text = match station_out {
                                Some(b) => format!("Out: q={} r={}", b.q, b.r),
                                None => "Out: (sin)".to_string(),
                            };
                            draw_text(&a_text, content_x, content_y, ctx.font_sm, colors.text_secondary);
                            content_y += 18.0;
                            draw_text(&b_text, content_x, content_y, ctx.font_sm, colors.text_secondary);
                            content_y += 22.0;

                            let rect_set_a = Rect::new(content_x, content_y, 80.0, 26.0);
                            let rect_set_b = Rect::new(content_x + 90.0, content_y, 80.0, 26.0);
                            let (clicked_a, _) =
                                ui_button(rect_set_a, "Set In", ctx.mouse, ctx.font_sm, ctx.button_colors);
                            let (clicked_b, _) =
                                ui_button(rect_set_b, "Set Out", ctx.mouse, ctx.font_sm, ctx.button_colors);
                            if clicked_a {
                                *station_pick = Some(crate::core::StationPick::In);
                            }
                            if clicked_b {
                                *station_pick = Some(crate::core::StationPick::Out);
                            }
                            content_y += 34.0;

                            let rect_spawn = Rect::new(content_x, content_y, 150.0, 28.0);
                            let (clicked, _) = ui_button(
                                rect_spawn,
                                "Crear unidad",
                                ctx.mouse,
                                ctx.font_sm,
                                ctx.button_colors,
                            );
                            if clicked {
                                if let (Some(a), Some(b)) = (*station_in, *station_out) {
                                    if let Some(path) = find_route_path(a, b, blocks) {
                                        units.push(crate::core::Unit {
                                            path,
                                            index: 0,
                                            progress: 0.0,
                                            speed: 3.0,
                                            forward: true,
                                            capacity: 20,
                                            cargo: Vec::new(),
                                            depot: target,
                                            station_in: a,
                                            station_out: b,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    BlockType::Mina => {
                        if let Some(tile) = tiles.get(&target) {
                            let total_remaining = mine_total_remaining(target, block, tiles);
                            let zones = 1 + block.mine_extra.len();
                            let txt = format!("Recurso: {}", tile_label(tile.kind));
                            draw_text(&txt, content_x, content_y, ctx.font_sm, colors.text_secondary);
                            content_y += 20.0;
                            draw_text(
                                &format!("Zonas: {}  Total: {}", zones, total_remaining),
                                content_x,
                                content_y,
                                ctx.font_sm,
                                colors.text_secondary,
                            );
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
                        content_y += 26.0;
                        if !under_construction {
                            let rect_expand = Rect::new(content_x, content_y, 150.0, 28.0);
                            let (clicked, _) = ui_button(
                                rect_expand,
                                "Expandir",
                                ctx.mouse,
                                ctx.font_sm,
                                ctx.button_colors,
                            );
                            if clicked {
                                if expand_mine_area(target, block, tiles) {
                                    *dirty = true;
                                }
                            }
                        }
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
