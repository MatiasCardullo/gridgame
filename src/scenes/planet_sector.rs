use macroquad::prelude::*;
use std::collections::HashMap;

use crate::core::{
    axial_neighbors, block_color, block_ports, draw_hex_filled, draw_hex_outline, draw_port_marker,
    hex_to_pixel, save_map, tile_color,
    add_item, build_requirements, is_under_construction, item_from_tile, new_placed_block, AppConfig, Axial,
    BuildBlockType, FrameContext, ItemType, PlacedBlock, RuntimeColors, Scene, TileData, TileType,
};
use crate::core::ui::{ui_button, WindowState, WINDOW_TITLE_HEIGHT};
use crate::{TRI_LENGHT, HEX_SIZE};
use crate::scenes::map_common::{
    build_panel_layout,
    confirm_label_for_target,
    draw_window_frame,
    is_ui_capturing,
    popup_rect_near_mouse,
    run_confirm_window,
    ConfirmAction,
    MapOutline,
    draw_common_hud,
    handle_camera_drag,
    handle_cursor_zoom,
    hex_screen_center,
    hover_hex_from_mouse,
    in_bounds,
};

// Friendly label for a block type.
fn block_label(kind: BuildBlockType) -> &'static str {
    match kind {
        BuildBlockType::Base => "Base",
        BuildBlockType::Housing => "Housing",
        BuildBlockType::Factory => "Factory",
        BuildBlockType::Mine => "Mine",
        BuildBlockType::Warehouse => "Warehouse",
        BuildBlockType::Logistics => "Logistics",
        BuildBlockType::Route => "Route",
    }
}

// Friendly label for an item type.
fn item_label(kind: ItemType) -> &'static str {
    match kind {
        ItemType::Gangue => "Gangue",
        ItemType::Iron => "Iron",
        ItemType::Copper => "Copper",
        ItemType::Gold => "Gold",
        ItemType::Zinc => "Zinc",
        ItemType::Lead => "Lead",
        ItemType::Water => "Water",
    }
}

// Friendly label for a tile kind.
fn tile_label(kind: TileType) -> &'static str {
    match kind {
        TileType::Iron => "Iron",
        TileType::Copper => "Copper",
        TileType::Gold => "Gold",
        TileType::Zinc => "Zinc",
        TileType::Lead => "Lead",
        TileType::Water => "Water",
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

fn requirement_sources(target: Axial) -> Vec<Axial> {
    let mut sources = Vec::with_capacity(7);
    sources.push(target);
    sources.extend(axial_neighbors(target));
    sources
}

fn available_item_amount_near(
    blocks: &HashMap<Axial, PlacedBlock>,
    target: Axial,
    kind: ItemType,
) -> i32 {
    requirement_sources(target)
        .into_iter()
        .filter_map(|hex| blocks.get(&hex).map(|b| (hex, b)))
        .filter(|(hex, block)| *hex == target || !is_under_construction(block))
        .flat_map(|(_, block)| block.stored.iter())
        .filter(|s| s.kind == kind)
        .map(|s| s.amount)
        .sum()
}

fn has_requirements_near(
    blocks: &HashMap<Axial, PlacedBlock>,
    target: Axial,
    reqs: &[crate::core::ItemStack],
) -> bool {
    reqs.iter()
        .all(|req| available_item_amount_near(blocks, target, req.kind) >= req.amount)
}

fn consume_requirements_near(
    blocks: &mut HashMap<Axial, PlacedBlock>,
    target: Axial,
    reqs: &[crate::core::ItemStack],
) -> bool {
    if !has_requirements_near(blocks, target, reqs) {
        return false;
    }
    let sources = requirement_sources(target);
    for req in reqs {
        let mut remaining = req.amount;
        for hex in sources.iter().copied() {
            if remaining <= 0 {
                break;
            }
            let take_from = match blocks.get_mut(&hex) {
                Some(block) => {
                    if hex != target && is_under_construction(block) {
                        continue;
                    }
                    block
                }
                None => continue,
            };
            let mut index = 0usize;
            while index < take_from.stored.len() && remaining > 0 {
                if take_from.stored[index].kind == req.kind {
                    let take = take_from.stored[index].amount.min(remaining);
                    take_from.stored[index].amount -= take;
                    remaining -= take;
                    if take_from.stored[index].amount <= 0 {
                        take_from.stored.remove(index);
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

// Move only allowed items from a block storage into unit cargo.
fn load_unit_from_block_filtered(
    block: &mut PlacedBlock,
    cargo: &mut Vec<crate::core::ItemStack>,
    capacity: i32,
    allowed: &[ItemType],
) {
    if allowed.is_empty() {
        return;
    }
    let mut remaining = capacity - storage_total(cargo);
    if remaining <= 0 {
        return;
    }
    let mut index = 0usize;
    while index < block.stored.len() && remaining > 0 {
        let stack = block.stored[index];
        if !allowed.iter().any(|k| *k == stack.kind) {
            index += 1;
            continue;
        }
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
                    .map(|b| b.kind == BuildBlockType::Route && !is_under_construction(b))
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

fn find_direct_path(start: Axial, end: Axial) -> Option<Vec<Axial>> {
    if start == end {
        return Some(vec![start]);
    }
    let mut path = Vec::new();
    path.push(start);
    let mut current = start;
    let mut guard = 0;
    while current != end && guard < 4096 {
        let mut best = current;
        let mut best_dist = crate::core::hex_distance(current, end);
        for neighbor in axial_neighbors(current) {
            let dist = crate::core::hex_distance(neighbor, end);
            if dist < best_dist {
                best = neighbor;
                best_dist = dist;
            }
        }
        if best == current {
            break;
        }
        current = best;
        path.push(current);
        guard += 1;
    }
    if current != end {
        return None;
    }
    Some(path)
}

fn missing_requirements_near(
    blocks: &HashMap<Axial, PlacedBlock>,
    target: Axial,
    reqs: &[crate::core::ItemStack],
) -> Vec<ItemType> {
    let mut missing = Vec::new();
    for req in reqs {
        let available = available_item_amount_near(blocks, target, req.kind);
        if available < req.amount {
            missing.push(req.kind);
        }
    }
    missing
}

fn nearest_supply_with_items(
    blocks: &HashMap<Axial, PlacedBlock>,
    target: Axial,
    needed: &[ItemType],
) -> Option<Axial> {
    let mut best_warehouse: Option<(i32, Axial)> = None;
    let mut best_base: Option<(i32, Axial)> = None;
    for (hex, block) in blocks.iter() {
        if is_under_construction(block) {
            continue;
        }
        if block.kind != BuildBlockType::Base && block.kind != BuildBlockType::Warehouse {
            continue;
        }
        let has_any = block
            .stored
            .iter()
            .any(|s| s.amount > 0 && needed.iter().any(|k| *k == s.kind));
        if !has_any {
            continue;
        }
        let dist = crate::core::hex_distance(*hex, target);
        match block.kind {
            BuildBlockType::Warehouse => match best_warehouse {
                Some((best_dist, _)) if dist >= best_dist => {}
                _ => best_warehouse = Some((dist, *hex)),
            },
            BuildBlockType::Base => match best_base {
                Some((best_dist, _)) if dist >= best_dist => {}
                _ => best_base = Some((dist, *hex)),
            },
            _ => {}
        }
    }
    best_warehouse
        .or(best_base)
        .map(|(_, hex)| hex)
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
    selected: &mut Option<BuildBlockType>,
    placement_rotation: &mut u8,
    panel_collapsed: &mut bool,
    window: &mut WindowState,
    confirm_window: &mut WindowState,
    units: &mut Vec<crate::core::Unit>,
    station_in: &mut Option<Axial>,
    station_out: &mut Option<Axial>,
    station_pick: &mut Option<crate::core::StationPick>,
    dirty: &mut bool,
    scene: &mut Scene,
    map_path: &str,
    config: &AppConfig,
    colors: &RuntimeColors,
    outline: MapOutline,
) {
    let base_hex = Axial { q: TRI_LENGHT/3, r: TRI_LENGHT/3 };
    if !blocks.contains_key(&base_hex) || blocks.get(&base_hex).map(|b| b.kind) != Some(BuildBlockType::Base) {
        let mut base = new_placed_block(BuildBlockType::Base, 0);
        base.build_time = 0.0;
        base.build_progress = 0.0;
        base.build_paid = true;
        let initial_items = [
            (ItemType::Gangue, 1800),
            (ItemType::Iron, 80),
            (ItemType::Copper, 40),
            (ItemType::Lead, 20),
            (ItemType::Zinc, 10),
        ];
        for (kind, amount) in initial_items {
            add_item(&mut base.stored, kind, amount, base.capacity);
        }
        blocks.insert(base_hex, base);
        *dirty = true;
    }

    handle_cursor_zoom(ctx, cam_offset, cam_zoom, config.zoom_speed);
    handle_camera_drag(ctx, cam_offset, dragging, last_mouse);

    let hover_hex = hover_hex_from_mouse(ctx, *cam_offset, *cam_zoom);

    let buttons: [(Option<BuildBlockType>, &str); 7] = [
        (None, "Demolish"),
        (Some(BuildBlockType::Housing), "Housing"),
        (Some(BuildBlockType::Factory), "Factory"),
        (Some(BuildBlockType::Mine), "Mines"),
        (Some(BuildBlockType::Warehouse), "Warehouse"),
        (Some(BuildBlockType::Logistics), "Logistics"),
        (Some(BuildBlockType::Route), "Route"),
    ];

    let panel_layout = build_panel_layout(buttons.len(), *panel_collapsed);
    let panel_pos = panel_layout.pos;
    let panel_size = panel_layout.size;
    let panel_rect = panel_layout.rect;

    let mut tooltip: Option<&str> = None;
    let ui_capturing = is_ui_capturing(panel_rect, window, confirm_window, ctx.mouse);

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
            let rect = popup_rect_near_mouse(ctx.mouse, win_w, win_h);
            window.title = block_label(existing.kind).to_string();
            window.rect = rect;
            window.open = true;
            window.target = Some(hover_hex);
            window.show_units = false;
        }
    }

    if is_mouse_button_pressed(MouseButton::Right) && !ui_capturing {
        if in_bounds(hover_hex, outline) {
            if let Some(kind) = *selected {
                if !blocks.contains_key(&hover_hex) {
                    let can_mine = tiles
                        .get(&hover_hex)
                        .map(|t| {
                            matches!(
                                t.kind,
                                TileType::Iron
                                    | TileType::Copper
                                    | TileType::Gold
                                    | TileType::Zinc
                                    | TileType::Lead
                            ) && t.amount > 0
                        })
                        .unwrap_or(false);
                    if kind != BuildBlockType::Mine || can_mine {
                        let rotation = if kind == BuildBlockType::Route { 0 } else { *placement_rotation };
                        blocks.insert(hover_hex, new_placed_block(kind, rotation));
                        *dirty = true;
                    }
                }
            } else if blocks.contains_key(&hover_hex) {
                let win_w = 240.0;
                let win_h = 120.0;
                let rect = popup_rect_near_mouse(ctx.mouse, win_w, win_h);
                confirm_window.title = "Confirm".to_string();
                confirm_window.rect = rect;
                confirm_window.open = true;
                confirm_window.target = Some(hover_hex);
                confirm_window.dragging = false;
            }
        }
    }

    if *dirty {
        save_map(map_path, blocks, tiles, units);
        *dirty = false;
    }

    if is_key_pressed(KeyCode::Escape) {
        save_map(map_path, blocks, tiles, units);
        *scene = Scene::MainMenu;
    }

    if is_key_pressed(KeyCode::R) {
        if let Some(block) = blocks.get_mut(&hover_hex) {
            if block.kind != BuildBlockType::Route {
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
    for hex in build_hexes.iter().copied() {
        let (kind, needs_pay) = match blocks.get(&hex) {
            Some(block) => (block.kind, !block.build_paid),
            None => continue,
        };
        if needs_pay {
            let reqs = build_requirements(kind);
            let paid = reqs.is_empty() || consume_requirements_near(blocks, hex, &reqs);
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

    units.retain(|unit| {
        if !unit.auto_supply {
            return true;
        }
        let needs_supply = blocks
            .get(&unit.station_out)
            .map(|block| is_under_construction(block) && !block.build_paid)
            .unwrap_or(false);
        if needs_supply {
            return true;
        }
        unit.path.get(unit.index).copied() != Some(base_hex)
    });

    if !units.iter().any(|unit| unit.auto_supply) {
        let mut best: Option<(i32, Axial, Axial, Vec<ItemType>, Vec<Axial>)> = None;
        for target in build_hexes.iter().copied() {
            let block = match blocks.get(&target) {
                Some(block) => block,
                None => continue,
            };
            if block.build_paid {
                continue;
            }
            let reqs = build_requirements(block.kind);
            if reqs.is_empty() {
                continue;
            }
            let missing = missing_requirements_near(blocks, target, &reqs);
            if missing.is_empty() {
                continue;
            }
            let Some(source) = nearest_supply_with_items(blocks, target, &missing) else {
                continue;
            };
            let path = if source == base_hex {
                match find_direct_path(base_hex, target) {
                    Some(path) => path,
                    None => continue,
                }
            } else {
                let first = match find_direct_path(base_hex, source) {
                    Some(path) => path,
                    None => continue,
                };
                let second = match find_direct_path(source, target) {
                    Some(path) => path,
                    None => continue,
                };
                let mut combined = first;
                combined.extend(second.into_iter().skip(1));
                combined
            };
            let dist = crate::core::hex_distance(source, target);
            match best {
                Some((best_dist, _, _, _, _)) if dist >= best_dist => {}
                _ => best = Some((dist, target, source, missing, path)),
            }
        }
        if let Some((_, target, source, missing, path)) = best {
            let mut unit = crate::core::Unit {
                path,
                index: 0,
                progress: 0.0,
                speed: 3.0,
                forward: true,
                capacity: 30,
                cargo: Vec::new(),
                depot: base_hex,
                station_in: source,
                station_out: target,
                auto_supply: true,
                supply_types: missing,
            };
            if source == base_hex {
                if let Some(block) = blocks.get_mut(&source) {
                    load_unit_from_block_filtered(
                        block,
                        &mut unit.cargo,
                        unit.capacity,
                        &unit.supply_types,
                    );
                }
            }
            units.push(unit);
        }
    }
    for (hex, block) in blocks.iter_mut() {
        if block.kind != BuildBlockType::Mine || is_under_construction(block) {
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
            if tile.kind != TileType::Water {
                let _ = add_item(&mut block.stored, ItemType::Gangue, added * 2, block.capacity);
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
                    if unit.auto_supply {
                        load_unit_from_block_filtered(
                            block,
                            &mut unit.cargo,
                            unit.capacity,
                            &unit.supply_types,
                        );
                    } else {
                        load_unit_from_block(block, &mut unit.cargo, unit.capacity);
                    }
                }
            }
            if arrived == unit.station_out {
                if let Some(block) = blocks.get_mut(&arrived) {
                    unload_unit_to_block(&mut unit.cargo, block);
                }
            }
        }
    }

    for r in -TRI_LENGHT..=TRI_LENGHT {
        for q in -TRI_LENGHT..=TRI_LENGHT {
            let hex = Axial { q, r };
            if !in_bounds(hex, outline) {
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
            if *cam_zoom > 1.2 {
                draw_hex_outline(center, size, colors.grid, config.line_thickness);
            }
        }
    }

    for (hex, placed) in blocks.iter() {
        if placed.kind != BuildBlockType::Route || is_under_construction(placed) {
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

    if let Some(block) = blocks.get(&hover_hex) {
        if block.kind == BuildBlockType::Mine {
            let mut highlight = colors.hover;
            highlight.a = 0.55;
            for target in mine_targets(hover_hex, block) {
                let center = ctx.screen_center
                    + *cam_offset
                    + hex_to_pixel(target, HEX_SIZE, Vec2::ZERO) * *cam_zoom;
                draw_hex_outline(
                    center,
                    (HEX_SIZE + 2.0) * *cam_zoom,
                    highlight,
                    config.line_thickness.max(1.0) * 2.0,
                );
            }
        }
    }

    let hover_center = hex_screen_center(ctx, *cam_offset, *cam_zoom, hover_hex);
    if let Some(kind) = *selected {
        let within_bounds = in_bounds(hover_hex, outline);
        let empty = !blocks.contains_key(&hover_hex);
        let can_mine = tiles
            .get(&hover_hex)
            .map(|t| {
                matches!(
                    t.kind,
                    TileType::Iron
                        | TileType::Copper
                        | TileType::Gold
                        | TileType::Zinc
                        | TileType::Lead
                ) && t.amount > 0
            })
            .unwrap_or(false);
        let can_place = within_bounds && empty && (kind != BuildBlockType::Mine || can_mine);
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
    if in_bounds(hover_hex, outline) {
        draw_hex_outline(
            hover_center,
            HEX_SIZE * *cam_zoom,
            colors.hover,
            config.line_thickness * 2.0,
        );
    }

    draw_common_hud(ctx, colors, *placement_rotation, hover_hex);

    let panel_result = crate::core::ui::draw_build_panel(
        panel_pos,
        panel_size,
        *panel_collapsed,
        ctx.mouse,
        config.line_thickness,
        colors,
        &buttons,
        *selected,
        |kind| block_color(kind, colors),
    );
    if panel_result.toggled {
        *panel_collapsed = !*panel_collapsed;
    }
    if let Some(tip) = panel_result.hovered_tip {
        tooltip = Some(tip);
    }
    if let Some(option) = panel_result.clicked_option {
        *selected = option;
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

    if window.open {
        if let Some(target) = window.target {
            if let Some(block) = blocks.get(&target) {
                if block.kind == BuildBlockType::Logistics {
                    let owned_count = units.iter().filter(|u| u.depot == target).count();
                    let visible_lines = owned_count.min(6) as f32;
                    let extra = if window.show_units {
                        let extra_lines = if owned_count > 6 { 1.0 } else { 0.0 };
                        38.0 + (visible_lines + extra_lines) * 18.0
                    } else {
                        0.0
                    };
                    window.rect.h = 170.0 + extra;
                } else {
                    window.rect.h = window.rect.h.min(170.0);
                    window.show_units = false;
                }
            }
        }

        let closed = draw_window_frame(window, ctx, config, colors);
        if !closed {
            if let Some(target) = window.target {
                if let Some(block) = blocks.get_mut(&target) {
                    let content_x = window.rect.x + 10.0;
                    let mut content_y = window.rect.y + WINDOW_TITLE_HEIGHT + 20.0;
                    let under_construction = is_under_construction(block);
                    if under_construction {
                        draw_text(
                            "Under construction",
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 18.0;
                        let reqs = build_requirements(block.kind);
                        if reqs.is_empty() {
                            draw_text(
                                "Materials: free",
                                content_x,
                                content_y,
                                ctx.font_sm,
                                colors.text_secondary,
                            );
                            content_y += 18.0;
                        } else {
                            draw_text(
                                "Materials:",
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
                                    "Waiting for materials",
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
                            &format!("Time remaining: {:.1}s", remaining),
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 22.0;
                    }
                    match block.kind {
                        BuildBlockType::Logistics => {
                            if under_construction {
                                draw_text(
                                    "Available after completion",
                                    content_x,
                                    content_y,
                                    ctx.font_sm,
                                    colors.text_secondary,
                                );
                            } else {
                                let a_text = match station_in {
                                    Some(a) => format!("In: q={} r={}", a.q, a.r),
                                    None => "In: (none)".to_string(),
                                };
                                let b_text = match station_out {
                                    Some(b) => format!("Out: q={} r={}", b.q, b.r),
                                    None => "Out: (none)".to_string(),
                                };
                                draw_text(
                                    &a_text,
                                    content_x,
                                    content_y,
                                    ctx.font_sm,
                                    colors.text_secondary,
                                );
                                content_y += 18.0;
                                draw_text(
                                    &b_text,
                                    content_x,
                                    content_y,
                                    ctx.font_sm,
                                    colors.text_secondary,
                                );
                                content_y += 22.0;

                                let rect_set_a = Rect::new(content_x, content_y, 80.0, 26.0);
                                let rect_set_b = Rect::new(content_x + 90.0, content_y, 80.0, 26.0);
                                let (clicked_a, _) = ui_button(
                                    rect_set_a,
                                    "Set In",
                                    ctx.mouse,
                                    ctx.font_sm,
                                    ctx.button_colors,
                                );
                                let (clicked_b, _) = ui_button(
                                    rect_set_b,
                                    "Set Out",
                                    ctx.mouse,
                                    ctx.font_sm,
                                    ctx.button_colors,
                                );
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
                                    "Create unit",
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
                                                auto_supply: false,
                                                supply_types: Vec::new(),
                                            });
                                        }
                                    }
                                }
                                content_y += 34.0;

                                let label = if window.show_units {
                                    "Hide list"
                                } else {
                                    "Unit list"
                                };
                                let rect_list = Rect::new(content_x, content_y, 150.0, 26.0);
                                let (clicked_list, _) = ui_button(
                                    rect_list,
                                    label,
                                    ctx.mouse,
                                    ctx.font_sm,
                                    ctx.button_colors,
                                );
                                if clicked_list {
                                    window.show_units = !window.show_units;
                                }
                                content_y += 30.0;

                                if window.show_units {
                                    let owned_units: Vec<(usize, &crate::core::Unit)> = units
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, unit)| unit.depot == target)
                                        .collect();
                                    let total_units = owned_units.len();
                                    draw_text(
                                        &format!("Units: {}", total_units),
                                        content_x,
                                        content_y,
                                        ctx.font_sm,
                                        colors.text_secondary,
                                    );
                                    content_y += 18.0;
                                    for (index, unit) in owned_units.iter().take(6) {
                                        let total = storage_total(&unit.cargo);
                                        let info = format!(
                                            "#{} load {}/{}",
                                            index + 1,
                                            total,
                                            unit.capacity
                                        );
                                        draw_text(
                                            &info,
                                            content_x,
                                            content_y,
                                            ctx.font_sm,
                                            colors.text_secondary,
                                        );
                                        content_y += 18.0;
                                    }
                                    if total_units > 6 {
                                        draw_text(
                                            &format!("+{} more", total_units - 6),
                                            content_x,
                                            content_y,
                                            ctx.font_sm,
                                            colors.text_secondary,
                                        );
                                        content_y += 18.0;
                                    }
                                }
                            }
                        }
                        BuildBlockType::Mine => {
                            if let Some(tile) = tiles.get(&target) {
                                let total_remaining = mine_total_remaining(target, block, tiles);
                                let zones = 1 + block.mine_extra.len();
                                let txt = format!("Resource: {}", tile_label(tile.kind));
                                draw_text(&txt, content_x, content_y, ctx.font_sm, colors.text_secondary);
                                content_y += 20.0;
                                draw_text(
                                    &format!("Zones: {}  Total: {}", zones, total_remaining),
                                    content_x,
                                    content_y,
                                    ctx.font_sm,
                                    colors.text_secondary,
                                );
                                content_y += 20.0;
                            }
                            let total = storage_total(&block.stored);
                            draw_text(
                                &format!("Stored: {}/{}", total, block.capacity),
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
                                    "Expand",
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
                        BuildBlockType::Base | BuildBlockType::Warehouse => {
                            let total = storage_total(&block.stored);
                            draw_text(
                                &format!("Capacity: {}/{}", total, block.capacity),
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
    }

    let confirm_target = confirm_window.target;
    let confirm_label = confirm_label_for_target(confirm_target, |target| {
        blocks
            .get(&target)
            .map(|block| block_label(block.kind).to_string())
    });
    if run_confirm_window(confirm_window, ctx, config, colors, &confirm_label)
        == ConfirmAction::Confirm
    {
        if let Some(target) = confirm_target {
            if blocks.remove(&target).is_some() {
                *dirty = true;
            }
        }
    }
}


