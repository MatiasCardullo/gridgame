use std::collections::HashMap;

use crate::core::{
    Axial, AutoSupplyRole, BuildBlockType, ItemType, PlacedBlock, TileData, TileType, Unit, add_item,
    build_requirements, hex_distance, is_under_construction, item_from_tile,
};

// Helpers for route/build unit simulation shared by 2D build-oriented scenes.
fn storage_total(stored: &[crate::core::ItemStack]) -> i32 {
    stored.iter().map(|s| s.amount).sum()
}

fn mine_targets(hex: Axial, block: &PlacedBlock) -> Vec<Axial> {
    let mut targets = Vec::with_capacity(1 + block.mine_extra.len());
    targets.push(hex);
    targets.extend(block.mine_extra.iter().copied());
    targets
}

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

fn apply_cargo_to_construction(
    blocks: &mut HashMap<Axial, PlacedBlock>,
    target: Axial,
    cargo: &mut Vec<crate::core::ItemStack>,
) {
    let reqs = match blocks.get(&target) {
        Some(block) => {
            if block.build_paid {
                return;
            }
            build_requirements(block.kind)
        }
        None => return,
    };
    if reqs.is_empty() {
        return;
    }
    for req in reqs.iter() {
        let available = cargo
            .iter()
            .filter(|s| s.kind == req.kind)
            .map(|s| s.amount)
            .sum::<i32>();
        if available < req.amount {
            return;
        }
    }
    for req in reqs.iter() {
        let mut remaining = req.amount;
        let mut index = 0usize;
        while index < cargo.len() && remaining > 0 {
            if cargo[index].kind == req.kind {
                let take = cargo[index].amount.min(remaining);
                cargo[index].amount -= take;
                remaining -= take;
                if cargo[index].amount <= 0 {
                    cargo.remove(index);
                    continue;
                }
            }
            index += 1;
        }
    }
    if let Some(block) = blocks.get_mut(&target) {
        block.build_paid = true;
        block.build_claimed = false;
    }
}

fn can_fulfill_reqs_from_block(block: &PlacedBlock, reqs: &[crate::core::ItemStack]) -> bool {
    reqs.iter().all(|req| {
        block
            .stored
            .iter()
            .filter(|s| s.kind == req.kind)
            .map(|s| s.amount)
            .sum::<i32>()
            >= req.amount
    })
}

fn take_reqs_from_block(
    block: &mut PlacedBlock,
    cargo: &mut Vec<crate::core::ItemStack>,
    capacity: i32,
    reqs: &[crate::core::ItemStack],
) -> bool {
    if !can_fulfill_reqs_from_block(block, reqs) {
        return false;
    }
    for req in reqs {
        let mut remaining = req.amount;
        let mut index = 0usize;
        while index < block.stored.len() && remaining > 0 {
            if block.stored[index].kind == req.kind {
                let take = block.stored[index].amount.min(remaining);
                let added = add_item(cargo, req.kind, take, capacity);
                block.stored[index].amount -= added;
                remaining -= added;
                if block.stored[index].amount <= 0 {
                    block.stored.remove(index);
                    continue;
                }
            }
            index += 1;
        }
    }
    true
}

// Find a path from start to end traveling only along route tiles (plus endpoints).
pub fn find_route_path(
    start: Axial,
    end: Axial,
    blocks: &HashMap<Axial, PlacedBlock>,
) -> Option<Vec<Axial>> {
    if start == end {
        return Some(vec![start]);
    }
    let mut queue = std::collections::VecDeque::new();
    let mut came_from: HashMap<Axial, Axial> = HashMap::new();
    queue.push_back(start);
    came_from.insert(start, start);

    while let Some(current) = queue.pop_front() {
        for neighbor in crate::core::axial_neighbors(current) {
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

pub fn find_direct_path(start: Axial, end: Axial) -> Option<Vec<Axial>> {
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
        for neighbor in crate::core::axial_neighbors(current) {
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

pub fn combine_paths(a: Vec<Axial>, b: Vec<Axial>) -> Vec<Axial> {
    if a.is_empty() {
        return b;
    }
    if b.is_empty() {
        return a;
    }
    let mut combined = a;
    combined.extend(b.into_iter().skip(1));
    combined
}

fn missing_requirements_for_target(
    blocks: &HashMap<Axial, PlacedBlock>,
    target: Axial,
    reqs: &[crate::core::ItemStack],
) -> Vec<ItemType> {
    let mut missing = Vec::new();
    let available_from_target = blocks.get(&target);
    for req in reqs {
        let available = available_from_target
            .map(|block| {
                block
                    .stored
                    .iter()
                    .filter(|s| s.kind == req.kind)
                    .map(|s| s.amount)
                    .sum::<i32>()
            })
            .unwrap_or(0);
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
            .any(|s| s.amount > 0 && needed.contains(&s.kind));
        if !has_any {
            continue;
        }
        let dist = hex_distance(*hex, target);
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
    best_warehouse.or(best_base).map(|(_, hex)| hex)
}

fn nearest_builder_with_items(
    blocks: &HashMap<Axial, PlacedBlock>,
    target: Axial,
    needed: &[ItemType],
) -> Option<Axial> {
    let mut best: Option<(i32, Axial)> = None;
    for (hex, block) in blocks.iter() {
        if is_under_construction(block) {
            continue;
        }
        if block.kind != BuildBlockType::Builder {
            continue;
        }
        let has_any = block
            .stored
            .iter()
            .any(|s| s.amount > 0 && needed.contains(&s.kind));
        if !has_any {
            continue;
        }
        let dist = hex_distance(*hex, target);
        match best {
            Some((best_dist, _)) if dist >= best_dist => {}
            _ => best = Some((dist, *hex)),
        }
    }
    best.map(|(_, hex)| hex)
}

pub fn update_units_and_mining(
    blocks: &mut HashMap<Axial, PlacedBlock>,
    tiles: &mut HashMap<Axial, TileData>,
    units: &mut Vec<Unit>,
    base_hex: Axial,
    dt: f32,
) {
    let build_hexes: Vec<Axial> = blocks
        .iter()
        .filter(|(_, block)| is_under_construction(block))
        .map(|(hex, _)| *hex)
        .collect();
    for hex in build_hexes.iter() {
        if let Some(block) = blocks.get_mut(hex) {
            if block.build_paid {
                block.build_progress += dt;
                if block.build_progress > block.build_time {
                    block.build_progress = block.build_time;
                }
            }
        }
    }

    let mut index = 0usize;
    while index < units.len() {
        let unit = &units[index];
        if !unit.auto_supply {
            index += 1;
            continue;
        }
        let needs_supply = blocks
            .get(&unit.station_out)
            .map(|block| is_under_construction(block) && !block.build_paid)
            .unwrap_or(false);
        if needs_supply {
            index += 1;
            continue;
        }
        let at_depot = unit.path.get(unit.index).copied() == Some(unit.depot);
        if at_depot {
            if let Some(block) = blocks.get_mut(&unit.station_out) {
                block.build_claimed = false;
            }
            units.remove(index);
            continue;
        }
        index += 1;
    }

    let has_auto_base = units
        .iter()
        .any(|unit| unit.auto_supply && unit.auto_supply_role == AutoSupplyRole::Base);
    let has_builder = blocks
        .values()
        .any(|block| block.kind == BuildBlockType::Builder && !is_under_construction(block));

    let mut builder_active: HashMap<Axial, usize> = HashMap::new();
    for unit in units.iter() {
        if unit.auto_supply && unit.auto_supply_role == AutoSupplyRole::Builder {
            *builder_active.entry(unit.depot).or_insert(0) += 1;
        }
    }

    let mut spawn_builder_depot = None;

    for (hex, block) in blocks.iter() {
        if block.kind != BuildBlockType::Builder {
            continue;
        }
        let desired = block.builder_units_desired.max(0) as usize;
        let active = *builder_active.get(hex).unwrap_or(&0);
        if desired > active {
            spawn_builder_depot = Some(*hex);
            break;
        }
    }

    if let Some(depot) = spawn_builder_depot {
        let mut best: Option<(i32, i32, Axial, Axial, Vec<ItemType>, Vec<Axial>)> = None;
        for target in build_hexes.iter().copied() {
            let block = match blocks.get(&target) {
                Some(block) => block,
                None => continue,
            };
            if block.build_paid {
                continue;
            }
            if block.build_claimed {
                continue;
            }
            let priority = if block.kind == BuildBlockType::Route { 0 } else { 1 };
            let reqs = build_requirements(block.kind);
            if reqs.is_empty() {
                continue;
            }
            let missing = missing_requirements_for_target(blocks, target, &reqs);
            if missing.is_empty() {
                continue;
            }
            let Some(source) = nearest_supply_with_items(blocks, target, &missing)
                .or_else(|| nearest_builder_with_items(blocks, target, &missing))
            else {
                continue;
            };
            if let Some(block) = blocks.get(&source) {
                if !can_fulfill_reqs_from_block(block, &reqs) {
                    continue;
                }
            }
            let path = if depot == source {
                let first = match find_route_path(depot, target, blocks) {
                    Some(path) => path,
                    None => continue,
                };
                let second = match find_route_path(target, depot, blocks) {
                    Some(path) => path,
                    None => continue,
                };
                combine_paths(first, second)
            } else {
                let first = match find_route_path(depot, source, blocks) {
                    Some(path) => path,
                    None => continue,
                };
                let second = match find_route_path(source, target, blocks) {
                    Some(path) => path,
                    None => continue,
                };
                let third = match find_route_path(target, depot, blocks) {
                    Some(path) => path,
                    None => continue,
                };
                combine_paths(combine_paths(first, second), third)
            };
            let dist = hex_distance(depot, target);
            match best {
                Some((best_prio, _, _, _, _, _)) if priority > best_prio => {}
                Some((best_prio, best_dist, _, _, _, _))
                    if priority == best_prio && dist >= best_dist => {}
                _ => best = Some((priority, dist, target, source, missing, path)),
            }
        }
        if let Some((_, _, target, source, missing, path)) = best {
            let supply_reqs = build_requirements(
                blocks
                    .get(&target)
                    .map(|b| b.kind)
                    .unwrap_or(BuildBlockType::Route),
            );
            let mut unit = Unit {
                path,
                index: 0,
                progress: 0.0,
                speed: 4.5,
                forward: true,
                capacity: 30,
                cargo: Vec::new(),
                depot,
                station_in: source,
                station_out: target,
                auto_supply: true,
                supply_types: missing,
                auto_supply_role: AutoSupplyRole::Builder,
                supply_reqs,
                waiting_for_supply: false,
            };
            if unit.path.first().copied() == Some(unit.station_in) {
                if let Some(block) = blocks.get_mut(&unit.station_in) {
                    if !take_reqs_from_block(block, &mut unit.cargo, unit.capacity, &unit.supply_reqs)
                    {
                        unit.waiting_for_supply = true;
                    }
                }
            }
            if let Some(block) = blocks.get_mut(&target) {
                block.build_claimed = true;
            }
            units.push(unit);
        }
    }

    if !has_auto_base {
        let mut best: Option<(i32, i32, Axial, Axial, Vec<ItemType>, Vec<Axial>)> = None;
        for target in build_hexes.iter().copied() {
            let block = match blocks.get(&target) {
                Some(block) => block,
                None => continue,
            };
            if block.build_paid {
                continue;
            }
            if block.build_claimed {
                continue;
            }
            let priority = if block.kind == BuildBlockType::Route { 1 } else { 0 };
            let reqs = build_requirements(block.kind);
            if reqs.is_empty() {
                continue;
            }
            let missing = missing_requirements_for_target(blocks, target, &reqs);
            if missing.is_empty() {
                continue;
            }
            let Some(source) = nearest_supply_with_items(blocks, target, &missing) else {
                continue;
            };
            if let Some(block) = blocks.get(&source) {
                if !can_fulfill_reqs_from_block(block, &reqs) {
                    continue;
                }
            }
            let path = if source == base_hex {
                let first = match find_direct_path(base_hex, target) {
                    Some(path) => path,
                    None => continue,
                };
                let second = match find_direct_path(target, base_hex) {
                    Some(path) => path,
                    None => continue,
                };
                combine_paths(first, second)
            } else {
                let first = match find_direct_path(base_hex, source) {
                    Some(path) => path,
                    None => continue,
                };
                let second = match find_direct_path(source, target) {
                    Some(path) => path,
                    None => continue,
                };
                let third = match find_direct_path(target, base_hex) {
                    Some(path) => path,
                    None => continue,
                };
                combine_paths(combine_paths(first, second), third)
            };
            let dist = hex_distance(source, target);
            let better = match best {
                None => true,
                Some((best_prio, best_dist, _, _, _, _)) => {
                    if priority < best_prio {
                        true
                    } else if priority > best_prio {
                        false
                    } else if has_builder {
                        dist > best_dist
                    } else {
                        dist < best_dist
                    }
                }
            };
            if better {
                best = Some((priority, dist, target, source, missing, path));
            }
        }
        if let Some((_, _, target, source, missing, path)) = best {
            let supply_reqs = build_requirements(
                blocks
                    .get(&target)
                    .map(|b| b.kind)
                    .unwrap_or(BuildBlockType::Housing),
            );
            let mut unit = Unit {
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
                auto_supply_role: AutoSupplyRole::Base,
                supply_reqs,
                waiting_for_supply: false,
            };
            if unit.path.first().copied() == Some(unit.station_in) {
                if let Some(block) = blocks.get_mut(&unit.station_in) {
                    if !take_reqs_from_block(block, &mut unit.cargo, unit.capacity, &unit.supply_reqs)
                    {
                        unit.waiting_for_supply = true;
                    }
                }
            }
            if let Some(block) = blocks.get_mut(&target) {
                block.build_claimed = true;
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
            let extra_gangue = if tile.kind == TileType::Water { 0 } else { 2 };
            let required_space = 1 + extra_gangue;
            let current_total = storage_total(&block.stored);
            let capacity = if block.capacity <= 0 {
                i32::MAX
            } else {
                block.capacity
            };
            let free_space = (capacity - current_total).max(0);
            if free_space < required_space {
                break;
            }
            let added = add_item(&mut block.stored, item_from_tile(tile.kind), 1, block.capacity);
            if added <= 0 {
                break;
            }
            if extra_gangue > 0 {
                let _ = add_item(&mut block.stored, ItemType::Gangue, added * 2, block.capacity);
            }
            tile.amount -= added;
            block.mine_progress -= added as f32;
        }
    }

    for unit in units.iter_mut() {
        if unit.auto_supply && unit.waiting_for_supply {
            if unit.path.get(unit.index).copied() == Some(unit.station_in) {
                if let Some(block) = blocks.get_mut(&unit.station_in) {
                    if take_reqs_from_block(block, &mut unit.cargo, unit.capacity, &unit.supply_reqs)
                    {
                        unit.waiting_for_supply = false;
                    } else {
                        continue;
                    }
                } else {
                    unit.waiting_for_supply = false;
                }
            } else {
                unit.waiting_for_supply = false;
            }
        }
        if unit.path.len() < 2 {
            continue;
        }
        let is_logistics = blocks
            .get(&unit.depot)
            .map(|block| block.kind == BuildBlockType::Logistics)
            .unwrap_or(false);
        unit.progress += dt * unit.speed;
        while unit.progress >= 1.0 {
            unit.progress -= 1.0;
            if unit.auto_supply || is_logistics {
                if unit.index + 1 < unit.path.len() {
                    unit.index += 1;
                } else if is_logistics {
                    let start = unit.path.first().copied();
                    if start == Some(unit.depot) {
                        unit.index = 1.min(unit.path.len() - 1);
                    } else {
                        unit.index = 0;
                    }
                } else {
                    unit.index = 0;
                }
            } else if unit.forward {
                if unit.index + 1 < unit.path.len() {
                    unit.index += 1;
                }
                if unit.index >= unit.path.len() - 1 {
                    unit.forward = false;
                }
            } else if unit.index > 0 {
                unit.index -= 1;
            } else {
                unit.forward = true;
            }
            let arrived = unit.path[unit.index];
            if is_logistics
                && unit.path.len() > 1
                && unit.path.first().copied() == Some(unit.depot)
                && arrived == unit.station_in
            {
                let arrived_index = unit.index;
                if arrived_index > 0 {
                    unit.path.drain(0..arrived_index);
                }
                unit.index = 0;
            }
            if arrived == unit.station_in {
                if unit.auto_supply {
                    let target_build_paid = blocks
                        .get(&unit.station_out)
                        .map(|b| b.build_paid)
                        .unwrap_or(false);
                    let target_needs = blocks
                        .get(&unit.station_out)
                        .map(|b| is_under_construction(b) && !b.build_paid)
                        .unwrap_or(false);
                    if !target_needs {
                        if let Some(block) = blocks.get_mut(&unit.station_out) {
                            block.build_claimed = false;
                        }
                    }
                    if let Some(block) = blocks.get_mut(&arrived) {
                        if target_build_paid && !unit.cargo.is_empty() {
                            unload_unit_to_block(&mut unit.cargo, block);
                        }
                        if unit.cargo.is_empty()
                            && target_needs
                            && !take_reqs_from_block(
                                block,
                                &mut unit.cargo,
                                unit.capacity,
                                &unit.supply_reqs,
                            )
                        {
                            unit.waiting_for_supply = true;
                        }
                    }
                } else if is_logistics {
                    if unit.cargo.is_empty() {
                        if let Some(block) = blocks.get_mut(&arrived) {
                            load_unit_from_block(block, &mut unit.cargo, unit.capacity);
                        }
                    }
                } else if let Some(block) = blocks.get_mut(&arrived) {
                    load_unit_from_block(block, &mut unit.cargo, unit.capacity);
                }
            }
            if unit.auto_supply && hex_distance(arrived, unit.station_out) <= 1 {
                let under_construction = blocks
                    .get(&unit.station_out)
                    .map(is_under_construction)
                    .unwrap_or(false);
                if under_construction {
                    apply_cargo_to_construction(blocks, unit.station_out, &mut unit.cargo);
                }
            } else if is_logistics && arrived == unit.station_out {
                if let Some(block) = blocks.get_mut(&arrived) {
                    unload_unit_to_block(&mut unit.cargo, block);
                }
            } else if arrived == unit.station_out {
                let under_construction = blocks
                    .get(&arrived)
                    .map(is_under_construction)
                    .unwrap_or(false);
                if under_construction {
                    apply_cargo_to_construction(blocks, arrived, &mut unit.cargo);
                } else if let Some(block) = blocks.get_mut(&arrived) {
                    unload_unit_to_block(&mut unit.cargo, block);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{new_placed_block, ItemStack};

    #[test]
    fn route_path_uses_route_tiles_between_endpoints() {
        let start = Axial { q: 0, r: 0 };
        let mid = Axial { q: 1, r: 0 };
        let end = Axial { q: 2, r: 0 };
        let mut blocks: HashMap<Axial, PlacedBlock> = HashMap::new();
        let mut route = new_placed_block(BuildBlockType::Route, 0);
        route.build_time = 0.0;
        route.build_progress = 0.0;
        route.build_paid = true;
        blocks.insert(mid, route);

        let path = find_route_path(start, end, &blocks).expect("expected valid route path");
        assert_eq!(path, vec![start, mid, end]);
    }

    #[test]
    fn apply_cargo_pays_construction_when_requirements_met() {
        let target = Axial { q: 3, r: -1 };
        let mut blocks: HashMap<Axial, PlacedBlock> = HashMap::new();
        let mut build_kind = BuildBlockType::Housing;
        if build_requirements(build_kind).is_empty() {
            build_kind = BuildBlockType::Factory;
        }
        let mut placed = new_placed_block(build_kind, 0);
        placed.build_paid = false;
        blocks.insert(target, placed);

        let mut cargo: Vec<ItemStack> = build_requirements(build_kind);
        apply_cargo_to_construction(&mut blocks, target, &mut cargo);

        let updated = blocks.get(&target).unwrap();
        assert!(updated.build_paid, "construction should be marked as paid");
        assert!(
            cargo.iter().all(|stack| stack.amount == 0),
            "cargo should be consumed for requirements"
        );
    }
}
