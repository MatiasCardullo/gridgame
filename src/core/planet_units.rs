use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::core::AutoSupplyRole;
use crate::core::planet_grid::HexGrid;
use crate::core::planet_resources::PlanetResourceKind;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PlanetResourceStack {
    pub resource_id: u8,
    pub amount: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanetUnitRecord {
    pub depot: u32,
    pub station_in: u32,
    pub station_out: u32,
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub progress: f32,
    #[serde(default)]
    pub speed: f32,
    #[serde(default)]
    pub forward: bool,
    #[serde(default)]
    pub capacity: u32,
    #[serde(default)]
    pub auto_supply: bool,
    #[serde(default)]
    pub auto_supply_role: AutoSupplyRole,
    #[serde(default)]
    pub waiting_for_supply: bool,
    #[serde(default)]
    pub path: Vec<u32>,
    #[serde(default)]
    pub cargo: Vec<PlanetResourceStack>,
    #[serde(default)]
    pub supply_types: Vec<u8>,
    #[serde(default)]
    pub supply_reqs: Vec<PlanetResourceStack>,
}

#[derive(Clone, Debug)]
pub struct PlanetUnit {
    pub path: Vec<u32>,
    pub index: usize,
    pub progress: f32,
    pub speed: f32,
    pub forward: bool,
    pub capacity: u32,
    pub cargo: Vec<PlanetResourceStack>,
    pub depot: u32,
    pub station_in: u32,
    pub station_out: u32,
    pub auto_supply: bool,
    pub supply_types: Vec<PlanetResourceKind>,
    pub auto_supply_role: AutoSupplyRole,
    pub supply_reqs: Vec<PlanetResourceStack>,
    pub waiting_for_supply: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanetUnitsSnapshot {
    #[serde(default)]
    pub units: Vec<PlanetUnitRecord>,
}

pub trait PlanetBuildingAccess {
    type Kind: Copy + Eq;

    fn kind(&self) -> Self::Kind;
    fn storage(&self) -> &HashMap<PlanetResourceKind, u32>;
    fn storage_mut(&mut self) -> &mut HashMap<PlanetResourceKind, u32>;
    fn build_progress(&self) -> f32;
    fn build_time(&self) -> f32;
    fn set_build_progress(&mut self, value: f32);
    fn build_paid(&self) -> bool;
    fn set_build_paid(&mut self, value: bool);
    fn build_claimed(&self) -> bool;
    fn set_build_claimed(&mut self, value: bool);
    fn builder_units_desired(&self) -> i32;
}

pub struct UnitOps<K> {
    pub is_route: fn(K) -> bool,
    pub is_builder: fn(K) -> bool,
    pub is_base: fn(K) -> bool,
    pub is_warehouse: fn(K) -> bool,
    pub is_logistics: fn(K) -> bool,
    pub build_requirements: fn(K) -> Vec<PlanetResourceStack>,
    pub storage_capacity: fn(K) -> u32,
}

pub struct UnitUpdateResult {
    pub units_dirty: bool,
    pub buildings_dirty: bool,
}

pub fn resource_kind_to_id(kind: PlanetResourceKind) -> u8 {
    match kind {
        PlanetResourceKind::Iron => 0,
        PlanetResourceKind::Copper => 1,
        PlanetResourceKind::Gold => 2,
        PlanetResourceKind::Lead => 3,
        PlanetResourceKind::Zinc => 4,
        PlanetResourceKind::Aluminum => 10,
        PlanetResourceKind::Lithium => 5,
        PlanetResourceKind::Phosphate => 6,
        PlanetResourceKind::Stone => 7,
        PlanetResourceKind::FreshWater => 8,
        PlanetResourceKind::Salt => 9,
    }
}

pub fn resource_kind_from_id(id: u8) -> Option<PlanetResourceKind> {
    match id {
        0 => Some(PlanetResourceKind::Iron),
        1 => Some(PlanetResourceKind::Copper),
        2 => Some(PlanetResourceKind::Gold),
        3 => Some(PlanetResourceKind::Lead),
        4 => Some(PlanetResourceKind::Zinc),
        10 => Some(PlanetResourceKind::Aluminum),
        5 => Some(PlanetResourceKind::Lithium),
        6 => Some(PlanetResourceKind::Phosphate),
        7 => Some(PlanetResourceKind::Stone),
        8 => Some(PlanetResourceKind::FreshWater),
        9 => Some(PlanetResourceKind::Salt),
        _ => None,
    }
}

pub fn storage_total(stacks: &[PlanetResourceStack]) -> u32 {
    stacks.iter().map(|stack| stack.amount).sum()
}

fn stack_amount(stacks: &[PlanetResourceStack], kind: PlanetResourceKind) -> u32 {
    let id = resource_kind_to_id(kind);
    stacks
        .iter()
        .filter(|stack| stack.resource_id == id)
        .map(|stack| stack.amount)
        .sum()
}

fn add_stack(
    stacks: &mut Vec<PlanetResourceStack>,
    kind: PlanetResourceKind,
    amount: u32,
    capacity: u32,
) -> u32 {
    let available = capacity.saturating_sub(storage_total(stacks));
    if available == 0 || amount == 0 {
        return 0;
    }
    let add = available.min(amount);
    let id = resource_kind_to_id(kind);
    if let Some(stack) = stacks.iter_mut().find(|stack| stack.resource_id == id) {
        stack.amount += add;
    } else {
        stacks.push(PlanetResourceStack {
            resource_id: id,
            amount: add,
        });
    }
    add
}

fn remove_stack(
    stacks: &mut Vec<PlanetResourceStack>,
    kind: PlanetResourceKind,
    amount: u32,
) -> u32 {
    if amount == 0 {
        return 0;
    }
    let id = resource_kind_to_id(kind);
    let mut removed = 0;
    let mut index = 0usize;
    while index < stacks.len() && removed < amount {
        if stacks[index].resource_id == id {
            let take = (amount - removed).min(stacks[index].amount);
            stacks[index].amount -= take;
            removed += take;
            if stacks[index].amount == 0 {
                stacks.remove(index);
                continue;
            }
        }
        index += 1;
    }
    removed
}

fn add_storage_amount(
    storage: &mut HashMap<PlanetResourceKind, u32>,
    kind: PlanetResourceKind,
    amount: u32,
) {
    if amount == 0 {
        return;
    }
    *storage.entry(kind).or_insert(0) += amount;
}

fn planet_is_under_construction<B: PlanetBuildingAccess>(building: &B) -> bool {
    building.build_time() > 0.0 && building.build_progress() < building.build_time()
}
fn load_unit_from_building<B: PlanetBuildingAccess>(
    building: &mut B,
    cargo: &mut Vec<PlanetResourceStack>,
    capacity: u32,
) {
    let mut remaining = capacity.saturating_sub(storage_total(cargo));
    if remaining == 0 {
        return;
    }
    let kinds: Vec<PlanetResourceKind> = building.storage().keys().copied().collect();
    for kind in kinds {
        if remaining == 0 {
            break;
        }
        let stored = building.storage().get(&kind).copied().unwrap_or(0);
        if stored == 0 {
            continue;
        }
        let take = stored.min(remaining);
        let added = add_stack(cargo, kind, take, capacity);
        if added > 0 {
            let entry = building.storage_mut().entry(kind).or_insert(0);
            *entry = entry.saturating_sub(added);
            if *entry == 0 {
                building.storage_mut().remove(&kind);
            }
            remaining = remaining.saturating_sub(added);
        }
    }
}

fn unload_unit_to_building<B: PlanetBuildingAccess>(
    cargo: &mut Vec<PlanetResourceStack>,
    building: &mut B,
    capacity: u32,
) {
    let mut index = 0usize;
    while index < cargo.len() {
        let stack = cargo[index];
        let Some(kind) = resource_kind_from_id(stack.resource_id) else {
            index += 1;
            continue;
        };
        let used: u32 = building.storage().values().copied().sum();
        let free = capacity.saturating_sub(used);
        if free == 0 {
            break;
        }
        let take = stack.amount.min(free);
        add_storage_amount(building.storage_mut(), kind, take);
        cargo[index].amount -= take;
        if cargo[index].amount == 0 {
            cargo.remove(index);
        } else {
            index += 1;
        }
    }
}

fn can_fulfill_reqs_from_building<B: PlanetBuildingAccess>(
    building: &B,
    reqs: &[PlanetResourceStack],
) -> bool {
    reqs.iter().all(|req| {
        let Some(kind) = resource_kind_from_id(req.resource_id) else {
            return false;
        };
        building.storage().get(&kind).copied().unwrap_or(0) >= req.amount
    })
}

fn take_reqs_from_building<B: PlanetBuildingAccess>(
    building: &mut B,
    cargo: &mut Vec<PlanetResourceStack>,
    capacity: u32,
    reqs: &[PlanetResourceStack],
) -> bool {
    if !can_fulfill_reqs_from_building(building, reqs) {
        return false;
    }
    for req in reqs {
        let Some(kind) = resource_kind_from_id(req.resource_id) else {
            continue;
        };
        let mut remaining = req.amount;
        while remaining > 0 {
            let stored = building.storage().get(&kind).copied().unwrap_or(0);
            if stored == 0 {
                break;
            }
            let take = remaining.min(stored);
            let added = add_stack(cargo, kind, take, capacity);
            if added == 0 {
                break;
            }
            let entry = building.storage_mut().entry(kind).or_insert(0);
            *entry = entry.saturating_sub(added);
            if *entry == 0 {
                building.storage_mut().remove(&kind);
            }
            remaining -= added;
        }
    }
    true
}

fn apply_cargo_to_construction<B: PlanetBuildingAccess>(
    buildings: &mut HashMap<u32, B>,
    target: u32,
    cargo: &mut Vec<PlanetResourceStack>,
    ops: &UnitOps<B::Kind>,
) {
    let reqs = match buildings.get(&target) {
        Some(building) => {
            if building.build_paid() {
                return;
            }
            (ops.build_requirements)(building.kind())
        }
        None => return,
    };
    if reqs.is_empty() {
        return;
    }
    for req in reqs.iter() {
        let Some(kind) = resource_kind_from_id(req.resource_id) else {
            return;
        };
        if stack_amount(cargo, kind) < req.amount {
            return;
        }
    }
    for req in reqs.iter() {
        let Some(kind) = resource_kind_from_id(req.resource_id) else {
            continue;
        };
        let _ = remove_stack(cargo, kind, req.amount);
    }
    if let Some(building) = buildings.get_mut(&target) {
        building.set_build_paid(true);
        building.set_build_claimed(false);
    }
}

fn missing_requirements_for_target<B: PlanetBuildingAccess>(
    buildings: &HashMap<u32, B>,
    target: u32,
    reqs: &[PlanetResourceStack],
) -> Vec<PlanetResourceKind> {
    let mut missing = Vec::new();
    let available_from_target = buildings.get(&target);
    for req in reqs {
        let Some(kind) = resource_kind_from_id(req.resource_id) else {
            continue;
        };
        let available = available_from_target
            .and_then(|building| building.storage().get(&kind).copied())
            .unwrap_or(0);
        if available < req.amount {
            missing.push(kind);
        }
    }
    missing
}

fn nearest_supply_with_items<B: PlanetBuildingAccess>(
    buildings: &HashMap<u32, B>,
    sim_grid: &HexGrid,
    target: u32,
    types: &[PlanetResourceKind],
    ops: &UnitOps<B::Kind>,
) -> Option<u32> {
    let mut best: Option<(f32, u32, i32)> = None;
    for (cell_index, building) in buildings.iter() {
        let kind = building.kind();
        if !(ops.is_warehouse)(kind) && !(ops.is_base)(kind) {
            continue;
        }
        let mut total = 0;
        for kind in types {
            total += building.storage().get(kind).copied().unwrap_or(0);
        }
        if total == 0 {
            continue;
        }
        let priority = if (ops.is_warehouse)(kind) { 0 } else { 1 };
        let dist = cell_distance(sim_grid, *cell_index, target);
        let better = match best {
            None => true,
            Some((best_dist, _, best_prio)) => {
                if priority < best_prio {
                    true
                } else if priority > best_prio {
                    false
                } else {
                    dist < best_dist
                }
            }
        };
        if better {
            best = Some((dist, *cell_index, priority));
        }
    }
    best.map(|(_, cell_index, _)| cell_index)
}

fn nearest_builder_with_items<B: PlanetBuildingAccess>(
    buildings: &HashMap<u32, B>,
    sim_grid: &HexGrid,
    target: u32,
    types: &[PlanetResourceKind],
    ops: &UnitOps<B::Kind>,
) -> Option<u32> {
    let mut best: Option<(f32, u32)> = None;
    for (cell_index, building) in buildings.iter() {
        if !(ops.is_builder)(building.kind()) {
            continue;
        }
        let mut total = 0;
        for kind in types {
            total += building.storage().get(kind).copied().unwrap_or(0);
        }
        if total == 0 {
            continue;
        }
        let dist = cell_distance(sim_grid, *cell_index, target);
        if best.map(|(best_dist, _)| dist < best_dist).unwrap_or(true) {
            best = Some((dist, *cell_index));
        }
    }
    best.map(|(_, cell_index)| cell_index)
}

pub fn find_route_path_cells<B: PlanetBuildingAccess>(
    start: u32,
    end: u32,
    buildings: &HashMap<u32, B>,
    neighbors: &[Vec<u32>],
    ops: &UnitOps<B::Kind>,
) -> Option<Vec<u32>> {
    if start == end {
        return Some(vec![start]);
    }
    let mut queue = std::collections::VecDeque::new();
    let mut came_from: HashMap<u32, u32> = HashMap::new();
    queue.push_back(start);
    came_from.insert(start, start);
    while let Some(current) = queue.pop_front() {
        for neighbor in neighbors.get(current as usize).into_iter().flatten() {
            if came_from.contains_key(neighbor) {
                continue;
            }
            let passable = *neighbor == end
                || buildings
                    .get(neighbor)
                    .map(|b| (ops.is_route)(b.kind()) && !planet_is_under_construction(b))
                    .unwrap_or(false);
            if !passable {
                continue;
            }
            came_from.insert(*neighbor, current);
            if *neighbor == end {
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
            queue.push_back(*neighbor);
        }
    }
    None
}

pub fn find_direct_path_cells(
    start: u32,
    end: u32,
    neighbors: &[Vec<u32>],
) -> Option<Vec<u32>> {
    if start == end {
        return Some(vec![start]);
    }
    let mut queue = std::collections::VecDeque::new();
    let mut came_from: HashMap<u32, u32> = HashMap::new();
    queue.push_back(start);
    came_from.insert(start, start);
    while let Some(current) = queue.pop_front() {
        for neighbor in neighbors.get(current as usize).into_iter().flatten() {
            if came_from.contains_key(neighbor) {
                continue;
            }
            came_from.insert(*neighbor, current);
            if *neighbor == end {
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
            queue.push_back(*neighbor);
        }
    }
    None
}

pub fn combine_paths(a: Vec<u32>, b: Vec<u32>) -> Vec<u32> {
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

fn cell_distance(sim_grid: &HexGrid, a: u32, b: u32) -> f32 {
    let Some(va) = sim_grid.vertices.get(a as usize) else {
        return f32::MAX;
    };
    let Some(vb) = sim_grid.vertices.get(b as usize) else {
        return f32::MAX;
    };
    let dot = va.normalize().dot(vb.normalize()).clamp(-1.0, 1.0);
    dot.acos()
}
pub fn update_units_and_construction<B: PlanetBuildingAccess>(
    units: &mut Vec<PlanetUnit>,
    buildings: &mut HashMap<u32, B>,
    sim_grid: &HexGrid,
    neighbors: &[Vec<u32>],
    dt: f32,
    ops: &UnitOps<B::Kind>,
) -> UnitUpdateResult {
    let mut buildings_dirty = false;
    let mut units_dirty = false;
    let build_cells: Vec<u32> = buildings
        .iter()
        .filter_map(|(cell_index, building)| {
            if planet_is_under_construction(building) {
                Some(*cell_index)
            } else {
                None
            }
        })
        .collect();
    for cell_index in build_cells.iter() {
        if let Some(building) = buildings.get_mut(cell_index) {
            if building.build_paid() {
                building.set_build_progress((building.build_progress() + dt).min(building.build_time()));
                buildings_dirty = true;
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
        let needs_supply = buildings
            .get(&unit.station_out)
            .map(|building| planet_is_under_construction(building) && !building.build_paid())
            .unwrap_or(false);
        if needs_supply {
            index += 1;
            continue;
        }
        let at_depot = unit.path.get(unit.index).copied() == Some(unit.depot);
        if at_depot {
            if let Some(building) = buildings.get_mut(&unit.station_out) {
                building.set_build_claimed(false);
                buildings_dirty = true;
            }
            units.remove(index);
            units_dirty = true;
            continue;
        }
        index += 1;
    }

    let has_auto_base = units.iter().any(|unit| {
        unit.auto_supply && unit.auto_supply_role == AutoSupplyRole::Base
    });
    let has_builder = buildings.values().any(|building| {
        (ops.is_builder)(building.kind()) && !planet_is_under_construction(building)
    });

    let mut builder_active: HashMap<u32, usize> = HashMap::new();
    for unit in units.iter() {
        if unit.auto_supply && unit.auto_supply_role == AutoSupplyRole::Builder {
            *builder_active.entry(unit.depot).or_insert(0) += 1;
        }
    }

    let mut spawn_builder_depot = None;
    for (cell_index, building) in buildings.iter() {
        if !(ops.is_builder)(building.kind()) {
            continue;
        }
        let desired = building.builder_units_desired().max(0) as usize;
        let active = *builder_active.get(cell_index).unwrap_or(&0);
        if desired > active {
            spawn_builder_depot = Some(*cell_index);
            break;
        }
    }

    if let Some(depot) = spawn_builder_depot {
        let mut best: Option<(i32, f32, u32, u32, Vec<PlanetResourceKind>, Vec<u32>)> = None;
        for target in build_cells.iter().copied() {
            let building = match buildings.get(&target) {
                Some(building) => building,
                None => continue,
            };
            if building.build_paid() || building.build_claimed() {
                continue;
            }
            let priority = if (ops.is_route)(building.kind()) { 0 } else { 1 };
            let reqs = (ops.build_requirements)(building.kind());
            if reqs.is_empty() {
                continue;
            }
            let missing = missing_requirements_for_target(buildings, target, &reqs);
            if missing.is_empty() {
                continue;
            }
            let Some(source) = nearest_supply_with_items(buildings, sim_grid, target, &missing, ops)
                .or_else(|| nearest_builder_with_items(buildings, sim_grid, target, &missing, ops))
            else {
                continue;
            };
            if let Some(source_building) = buildings.get(&source) {
                if !can_fulfill_reqs_from_building(source_building, &reqs) {
                    continue;
                }
            }
            let path = if depot == source {
                let first = match find_route_path_cells(depot, target, buildings, neighbors, ops) {
                    Some(path) => path,
                    None => continue,
                };
                let second = match find_route_path_cells(target, depot, buildings, neighbors, ops) {
                    Some(path) => path,
                    None => continue,
                };
                combine_paths(first, second)
            } else {
                let first = match find_route_path_cells(depot, source, buildings, neighbors, ops) {
                    Some(path) => path,
                    None => continue,
                };
                let second = match find_route_path_cells(source, target, buildings, neighbors, ops) {
                    Some(path) => path,
                    None => continue,
                };
                let third = match find_route_path_cells(target, depot, buildings, neighbors, ops) {
                    Some(path) => path,
                    None => continue,
                };
                combine_paths(combine_paths(first, second), third)
            };
            let dist = cell_distance(sim_grid, depot, target);
            match best {
                Some((best_prio, _, _, _, _, _)) if priority > best_prio => {}
                Some((best_prio, best_dist, _, _, _, _))
                    if priority == best_prio && dist >= best_dist => {}
                _ => best = Some((priority, dist, target, source, missing, path)),
            }
        }
        if let Some((_, _, target, source, missing, path)) = best {
            let supply_reqs = (ops.build_requirements)(
                buildings
                    .get(&target)
                    .map(|b| b.kind())
                    .unwrap_or_else(|| buildings.get(&source).map(|b| b.kind()).unwrap()),
            );
            let mut unit = PlanetUnit {
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
                if let Some(building) = buildings.get_mut(&unit.station_in) {
                    if !take_reqs_from_building(
                        building,
                        &mut unit.cargo,
                        unit.capacity,
                        &unit.supply_reqs,
                    ) {
                        unit.waiting_for_supply = true;
                    }
                }
            }
            if let Some(building) = buildings.get_mut(&target) {
                building.set_build_claimed(true);
                buildings_dirty = true;
            }
            units.push(unit);
            units_dirty = true;
        }
    }

    if !has_auto_base {
        let base_cells: Vec<u32> = buildings
            .iter()
            .filter_map(|(cell_index, building)| {
                if (ops.is_base)(building.kind()) {
                    Some(*cell_index)
                } else {
                    None
                }
            })
            .collect();
        let mut best: Option<(i32, f32, u32, u32, Vec<PlanetResourceKind>, Vec<u32>)> = None;
        for target in build_cells.iter().copied() {
            let building = match buildings.get(&target) {
                Some(building) => building,
                None => continue,
            };
            if building.build_paid() || building.build_claimed() {
                continue;
            }
            let priority = if (ops.is_route)(building.kind()) { 1 } else { 0 };
            let reqs = (ops.build_requirements)(building.kind());
            if reqs.is_empty() {
                continue;
            }
            let missing = missing_requirements_for_target(buildings, target, &reqs);
            if missing.is_empty() {
                continue;
            }
            let Some(source) = nearest_supply_with_items(buildings, sim_grid, target, &missing, ops)
            else {
                continue;
            };
            if let Some(source_building) = buildings.get(&source) {
                if !can_fulfill_reqs_from_building(source_building, &reqs) {
                    continue;
                }
            }
            let Some(base_depot) = base_cells
                .iter()
                .copied()
                .min_by(|a, b| {
                    cell_distance(sim_grid, *a, target)
                        .partial_cmp(&cell_distance(sim_grid, *b, target))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            else {
                continue;
            };
            let path = if source == base_depot {
                let first = match find_direct_path_cells(base_depot, target, neighbors) {
                    Some(path) => path,
                    None => continue,
                };
                let second = match find_direct_path_cells(target, base_depot, neighbors) {
                    Some(path) => path,
                    None => continue,
                };
                combine_paths(first, second)
            } else {
                let first = match find_direct_path_cells(base_depot, source, neighbors) {
                    Some(path) => path,
                    None => continue,
                };
                let second = match find_direct_path_cells(source, target, neighbors) {
                    Some(path) => path,
                    None => continue,
                };
                let third = match find_direct_path_cells(target, base_depot, neighbors) {
                    Some(path) => path,
                    None => continue,
                };
                combine_paths(combine_paths(first, second), third)
            };
            let dist = cell_distance(sim_grid, source, target);
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
                best = Some((priority, dist, target, base_depot, missing, path));
            }
        }
        if let Some((_, _, target, base_depot, missing, path)) = best {
            let supply_reqs = (ops.build_requirements)(
                buildings
                    .get(&target)
                    .map(|b| b.kind())
                    .unwrap_or_else(|| {
                        buildings
                            .get(&base_depot)
                            .map(|b| b.kind())
                            .unwrap()
                    }),
            );
            let mut unit = PlanetUnit {
                path,
                index: 0,
                progress: 0.0,
                speed: 3.0,
                forward: true,
                capacity: 30,
                cargo: Vec::new(),
                depot: base_depot,
                station_in: base_depot,
                station_out: target,
                auto_supply: true,
                supply_types: missing,
                auto_supply_role: AutoSupplyRole::Base,
                supply_reqs,
                waiting_for_supply: false,
            };
            if unit.path.first().copied() == Some(unit.station_in) {
                if let Some(building) = buildings.get_mut(&unit.station_in) {
                    if !take_reqs_from_building(
                        building,
                        &mut unit.cargo,
                        unit.capacity,
                        &unit.supply_reqs,
                    ) {
                        unit.waiting_for_supply = true;
                    }
                }
            }
            if let Some(building) = buildings.get_mut(&target) {
                building.set_build_claimed(true);
                buildings_dirty = true;
            }
            units.push(unit);
            units_dirty = true;
        }
    }

    for unit in units.iter_mut() {
        if unit.auto_supply && unit.waiting_for_supply {
            if unit.path.get(unit.index).copied() == Some(unit.station_in) {
                if let Some(building) = buildings.get_mut(&unit.station_in) {
                    if take_reqs_from_building(
                        building,
                        &mut unit.cargo,
                        unit.capacity,
                        &unit.supply_reqs,
                    ) {
                        unit.waiting_for_supply = false;
                        buildings_dirty = true;
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
        let is_logistics = buildings
            .get(&unit.depot)
            .map(|building| (ops.is_logistics)(building.kind()))
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
                    let target_build_paid = buildings
                        .get(&unit.station_out)
                        .map(|b| b.build_paid())
                        .unwrap_or(false);
                    let target_needs = buildings
                        .get(&unit.station_out)
                        .map(|b| planet_is_under_construction(b) && !b.build_paid())
                        .unwrap_or(false);
                    if !target_needs {
                        if let Some(building) = buildings.get_mut(&unit.station_out) {
                            building.set_build_claimed(false);
                            buildings_dirty = true;
                        }
                    }
                    if let Some(building) = buildings.get_mut(&arrived) {
                        if target_build_paid && !unit.cargo.is_empty() {
                            unload_unit_to_building(
                                &mut unit.cargo,
                                building,
                                (ops.storage_capacity)(building.kind()),
                            );
                            buildings_dirty = true;
                        }
                        if unit.cargo.is_empty()
                            && target_needs
                            && !take_reqs_from_building(
                                building,
                                &mut unit.cargo,
                                unit.capacity,
                                &unit.supply_reqs,
                            )
                        {
                            unit.waiting_for_supply = true;
                            buildings_dirty = true;
                        }
                    }
                } else if is_logistics {
                    if unit.cargo.is_empty() {
                        if let Some(building) = buildings.get_mut(&arrived) {
                            load_unit_from_building(
                                building,
                                &mut unit.cargo,
                                unit.capacity,
                            );
                            buildings_dirty = true;
                        }
                    }
                } else if let Some(building) = buildings.get_mut(&arrived) {
                    load_unit_from_building(
                        building,
                        &mut unit.cargo,
                        unit.capacity,
                    );
                    buildings_dirty = true;
                }
            }
            let is_neighbor = neighbors
                .get(unit.station_out as usize)
                .map(|list| list.contains(&arrived))
                .unwrap_or(false);
            if unit.auto_supply && (arrived == unit.station_out || is_neighbor) {
                let under_construction = buildings
                    .get(&unit.station_out)
                    .map(planet_is_under_construction)
                    .unwrap_or(false);
                if under_construction {
                    apply_cargo_to_construction(buildings, unit.station_out, &mut unit.cargo, ops);
                    buildings_dirty = true;
                }
            } else if is_logistics && arrived == unit.station_out {
                if let Some(building) = buildings.get_mut(&arrived) {
                    unload_unit_to_building(
                        &mut unit.cargo,
                        building,
                        (ops.storage_capacity)(building.kind()),
                    );
                    buildings_dirty = true;
                }
            } else if arrived == unit.station_out {
                let under_construction = buildings
                    .get(&arrived)
                    .map(planet_is_under_construction)
                    .unwrap_or(false);
                if under_construction {
                    apply_cargo_to_construction(buildings, arrived, &mut unit.cargo, ops);
                    buildings_dirty = true;
                } else if let Some(building) = buildings.get_mut(&arrived) {
                    unload_unit_to_building(
                        &mut unit.cargo,
                        building,
                        (ops.storage_capacity)(building.kind()),
                    );
                    buildings_dirty = true;
                }
            }
        }
    }

    UnitUpdateResult {
        units_dirty: units_dirty || !units.is_empty(),
        buildings_dirty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum TestKind {
        Base,
        Builder,
        Housing,
        Warehouse,
        Logistics,
        Route,
    }

    #[derive(Clone, Debug)]
    struct TestBuilding {
        kind: TestKind,
        storage: HashMap<PlanetResourceKind, u32>,
        build_progress: f32,
        build_time: f32,
        build_paid: bool,
        build_claimed: bool,
        builder_units_desired: i32,
    }

    impl PlanetBuildingAccess for TestBuilding {
        type Kind = TestKind;

        fn kind(&self) -> Self::Kind {
            self.kind
        }

        fn storage(&self) -> &HashMap<PlanetResourceKind, u32> {
            &self.storage
        }

        fn storage_mut(&mut self) -> &mut HashMap<PlanetResourceKind, u32> {
            &mut self.storage
        }

        fn build_progress(&self) -> f32 {
            self.build_progress
        }

        fn build_time(&self) -> f32 {
            self.build_time
        }

        fn set_build_progress(&mut self, value: f32) {
            self.build_progress = value;
        }

        fn build_paid(&self) -> bool {
            self.build_paid
        }

        fn set_build_paid(&mut self, value: bool) {
            self.build_paid = value;
        }

        fn build_claimed(&self) -> bool {
            self.build_claimed
        }

        fn set_build_claimed(&mut self, value: bool) {
            self.build_claimed = value;
        }

        fn builder_units_desired(&self) -> i32 {
            self.builder_units_desired
        }
    }

    fn test_ops() -> UnitOps<TestKind> {
        UnitOps {
            is_route: |kind| kind == TestKind::Route,
            is_builder: |kind| kind == TestKind::Builder,
            is_base: |kind| kind == TestKind::Base,
            is_warehouse: |kind| kind == TestKind::Warehouse,
            is_logistics: |kind| kind == TestKind::Logistics,
            build_requirements: |kind| match kind {
                TestKind::Housing => vec![PlanetResourceStack {
                    resource_id: resource_kind_to_id(PlanetResourceKind::Stone),
                    amount: 3,
                }],
                _ => Vec::new(),
            },
            storage_capacity: |_| 100,
        }
    }

    fn make_building(kind: TestKind) -> TestBuilding {
        TestBuilding {
            kind,
            storage: HashMap::new(),
            build_progress: 0.0,
            build_time: 1.0,
            build_paid: true,
            build_claimed: false,
            builder_units_desired: 0,
        }
    }

    #[test]
    fn route_path_uses_route_cells_between_endpoints() {
        let mut buildings: HashMap<u32, TestBuilding> = HashMap::new();
        let mut route = make_building(TestKind::Route);
        route.build_paid = true;
        route.build_progress = route.build_time;
        buildings.insert(1, route);

        let neighbors = vec![vec![1], vec![0, 2], vec![1]];
        let path = find_route_path_cells(0, 2, &buildings, &neighbors, &test_ops())
            .expect("expected route path");
        assert_eq!(path, vec![0, 1, 2]);
    }

    #[test]
    fn apply_cargo_pays_construction_when_requirements_met() {
        let target = 3;
        let mut buildings: HashMap<u32, TestBuilding> = HashMap::new();
        let mut building = make_building(TestKind::Housing);
        building.build_paid = false;
        building.build_progress = 0.0;
        buildings.insert(target, building);

        let mut cargo = vec![PlanetResourceStack {
            resource_id: resource_kind_to_id(PlanetResourceKind::Stone),
            amount: 3,
        }];
        apply_cargo_to_construction(&mut buildings, target, &mut cargo, &test_ops());

        let updated = buildings.get(&target).unwrap();
        assert!(updated.build_paid(), "construction should be marked paid");
        assert!(cargo.iter().all(|stack| stack.amount == 0));
    }
}
