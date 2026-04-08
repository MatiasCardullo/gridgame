use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::core::planet_grid::HexGrid;
use crate::core::planet_resources::PlanetResourceKind;
use crate::core::planet_unit_catalog::{
    PlanetUnitDefinition, PlanetUnitDefinitionId, runtime_definition_for_preset, unit_definition,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanetResourceStack {
    pub resource_id: u8,
    pub amount: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanetUnitPresetId {
    Hauler,
    BuilderSupply,
    Shuttle,
}

impl PlanetUnitPresetId {
    pub fn label(self) -> &'static str {
        match self {
            PlanetUnitPresetId::Hauler => "Hauler",
            PlanetUnitPresetId::BuilderSupply => "Builder Supply",
            PlanetUnitPresetId::Shuttle => "Shuttle",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitNodeSlot {
    Depot,
    Pickup,
    Dropoff,
    Refuel,
    Construction,
    Wait,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum UnitConditionExpr {
    CargoEmpty,
    CargoFull,
    CargoAtLeast(u32),
    HasAnyCargo,
    FuelBelowRatio(f32),
    AtTarget,
    CanLoadAtTarget,
    CanUnloadAtTarget,
    Always,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum UnitAction {
    GoToNode(UnitNodeSlot),
    PickupAtNode(UnitNodeSlot),
    DeliverAtNode(UnitNodeSlot),
    BuildAtNode(UnitNodeSlot),
    RefuelAtNode(UnitNodeSlot),
    WaitAtNode(UnitNodeSlot),
    Idle,
}

impl UnitAction {
    pub fn label(&self) -> &'static str {
        match self {
            UnitAction::GoToNode(_) => "Go",
            UnitAction::PickupAtNode(_) => "Pickup",
            UnitAction::DeliverAtNode(_) => "Deliver",
            UnitAction::BuildAtNode(_) => "Build",
            UnitAction::RefuelAtNode(_) => "Refuel",
            UnitAction::WaitAtNode(_) => "Wait",
            UnitAction::Idle => "Idle",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitRule {
    pub priority: i32,
    pub condition: UnitConditionExpr,
    pub action: UnitAction,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanetUnitBehavior {
    pub rules: Vec<UnitRule>,
    pub fallback: UnitAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanetUnitNodeConfig {
    pub pickup: Option<u32>,
    pub dropoff: Option<u32>,
    pub refuel: Option<u32>,
    pub construction: Option<u32>,
    pub wait: Option<u32>,
}

impl PlanetUnitNodeConfig {
    pub fn get(&self, slot: UnitNodeSlot, depot: u32) -> Option<u32> {
        match slot {
            UnitNodeSlot::Depot => Some(depot),
            UnitNodeSlot::Pickup => self.pickup,
            UnitNodeSlot::Dropoff => self.dropoff,
            UnitNodeSlot::Refuel => self.refuel,
            UnitNodeSlot::Construction => self.construction,
            UnitNodeSlot::Wait => self.wait.or(Some(depot)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanetUnitRecord {
    pub depot: u32,
    pub preset_id: PlanetUnitPresetId,
    #[serde(default)]
    pub definition_id: Option<PlanetUnitDefinitionId>,
    pub behavior: PlanetUnitBehavior,
    pub nodes: PlanetUnitNodeConfig,
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub progress: f32,
    #[serde(default)]
    pub speed: f32,
    #[serde(default)]
    pub capacity: u32,
    #[serde(default)]
    pub fuel: f32,
    #[serde(default)]
    pub fuel_capacity: f32,
    #[serde(default)]
    pub fuel_burn_rate: f32,
    #[serde(default)]
    pub path: Vec<u32>,
    #[serde(default)]
    pub cargo: Vec<PlanetResourceStack>,
    #[serde(default)]
    pub current_action: Option<UnitAction>,
    #[serde(default)]
    pub current_target: Option<u32>,
    #[serde(default)]
    pub assignment_check_timer: f32,
}

#[derive(Clone, Debug)]
pub struct PlanetUnit {
    pub depot: u32,
    pub preset_id: PlanetUnitPresetId,
    pub definition_id: PlanetUnitDefinitionId,
    pub behavior: PlanetUnitBehavior,
    pub nodes: PlanetUnitNodeConfig,
    pub path: Vec<u32>,
    pub index: usize,
    pub progress: f32,
    pub speed: f32,
    pub capacity: u32,
    pub fuel: f32,
    pub fuel_capacity: f32,
    pub fuel_burn_rate: f32,
    pub cargo: Vec<PlanetResourceStack>,
    pub current_action: Option<UnitAction>,
    pub current_target: Option<u32>,
    pub assignment_check_timer: f32,
}

impl PlanetUnit {
    pub fn definition(&self) -> &'static PlanetUnitDefinition {
        unit_definition(self.definition_id)
    }

    pub fn label(&self) -> &'static str {
        self.definition().label
    }

    pub fn current_cell(&self) -> u32 {
        self.path
            .get(self.index)
            .copied()
            .or_else(|| self.path.last().copied())
            .unwrap_or(self.depot)
    }

    pub fn fuel_ratio(&self) -> f32 {
        if self.fuel_capacity <= 0.0 {
            0.0
        } else {
            (self.fuel / self.fuel_capacity).clamp(0.0, 1.0)
        }
    }

    pub fn references_cell(&self, cell: u32) -> bool {
        self.depot == cell
            || self.nodes.pickup == Some(cell)
            || self.nodes.dropoff == Some(cell)
            || self.nodes.refuel == Some(cell)
            || self.nodes.construction == Some(cell)
            || self.nodes.wait == Some(cell)
            || self.current_target == Some(cell)
            || self.path.contains(&cell)
    }
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
    fn auto_units_enabled(&self) -> bool {
        true
    }
}

pub struct UnitOps<K> {
    pub is_route: fn(K) -> bool,
    pub is_builder: fn(K) -> bool,
    pub is_base: fn(K) -> bool,
    pub is_warehouse: fn(K) -> bool,
    pub is_refuel: fn(K) -> bool,
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

pub fn default_behavior_for_preset(preset_id: PlanetUnitPresetId) -> PlanetUnitBehavior {
    match preset_id {
        PlanetUnitPresetId::Hauler => PlanetUnitBehavior {
            rules: vec![
                UnitRule {
                    priority: 100,
                    condition: UnitConditionExpr::FuelBelowRatio(0.25),
                    action: UnitAction::RefuelAtNode(UnitNodeSlot::Refuel),
                },
                UnitRule {
                    priority: 80,
                    condition: UnitConditionExpr::CargoEmpty,
                    action: UnitAction::PickupAtNode(UnitNodeSlot::Pickup),
                },
                UnitRule {
                    priority: 70,
                    condition: UnitConditionExpr::HasAnyCargo,
                    action: UnitAction::DeliverAtNode(UnitNodeSlot::Dropoff),
                },
            ],
            fallback: UnitAction::WaitAtNode(UnitNodeSlot::Depot),
        },
        PlanetUnitPresetId::BuilderSupply => PlanetUnitBehavior {
            rules: vec![
                UnitRule {
                    priority: 100,
                    condition: UnitConditionExpr::FuelBelowRatio(0.25),
                    action: UnitAction::RefuelAtNode(UnitNodeSlot::Refuel),
                },
                UnitRule {
                    priority: 80,
                    condition: UnitConditionExpr::CargoEmpty,
                    action: UnitAction::PickupAtNode(UnitNodeSlot::Pickup),
                },
                UnitRule {
                    priority: 70,
                    condition: UnitConditionExpr::HasAnyCargo,
                    action: UnitAction::BuildAtNode(UnitNodeSlot::Construction),
                },
            ],
            fallback: UnitAction::WaitAtNode(UnitNodeSlot::Depot),
        },
        PlanetUnitPresetId::Shuttle => PlanetUnitBehavior {
            rules: vec![
                UnitRule {
                    priority: 100,
                    condition: UnitConditionExpr::FuelBelowRatio(0.25),
                    action: UnitAction::RefuelAtNode(UnitNodeSlot::Refuel),
                },
                UnitRule {
                    priority: 80,
                    condition: UnitConditionExpr::CargoEmpty,
                    action: UnitAction::GoToNode(UnitNodeSlot::Pickup),
                },
                UnitRule {
                    priority: 70,
                    condition: UnitConditionExpr::HasAnyCargo,
                    action: UnitAction::GoToNode(UnitNodeSlot::Dropoff),
                },
            ],
            fallback: UnitAction::WaitAtNode(UnitNodeSlot::Depot),
        },
    }
}

pub fn unit_for_preset(
    preset_id: PlanetUnitPresetId,
    depot: u32,
    nodes: PlanetUnitNodeConfig,
) -> PlanetUnit {
    let definition_id = runtime_definition_for_preset(preset_id).id;
    let speed = match preset_id {
        PlanetUnitPresetId::BuilderSupply => 4.5,
        PlanetUnitPresetId::Hauler => 3.0,
        PlanetUnitPresetId::Shuttle => 3.5,
    };
    let capacity = match preset_id {
        PlanetUnitPresetId::BuilderSupply => 40,
        PlanetUnitPresetId::Hauler | PlanetUnitPresetId::Shuttle => 20,
    };
    PlanetUnit {
        depot,
        preset_id,
        definition_id,
        behavior: default_behavior_for_preset(preset_id),
        nodes,
        path: vec![depot],
        index: 0,
        progress: 0.0,
        speed,
        capacity,
        fuel: 100.0,
        fuel_capacity: 100.0,
        fuel_burn_rate: 1.2,
        cargo: Vec::new(),
        current_action: None,
        current_target: None,
        assignment_check_timer: 0.0,
    }
}

pub fn unit_for_definition(
    definition_id: PlanetUnitDefinitionId,
    depot: u32,
    nodes: PlanetUnitNodeConfig,
) -> Option<PlanetUnit> {
    let definition = unit_definition(definition_id);
    definition.preset_id.map(|preset_id| {
        let mut unit = unit_for_preset(preset_id, depot, nodes);
        unit.definition_id = definition.id;
        unit
    })
}

fn next_builder_assignment<B: PlanetBuildingAccess>(
    depot: u32,
    buildings: &HashMap<u32, B>,
    exclude: Option<u32>,
    reserved_targets: &[u32],
    neighbors: &[Vec<u32>],
    ops: &UnitOps<B::Kind>,
) -> Option<(u32, u32)> {
    for target in buildings
        .iter()
        .filter(|(cell_index, building)| {
            Some(**cell_index) != exclude
                && !reserved_targets.contains(cell_index)
                && planet_is_under_construction(*building)
                && !building.build_paid()
                && !(ops.build_requirements)(building.kind()).is_empty()
        })
        .map(|(cell_index, _)| *cell_index)
    {
        let reqs = buildings
            .get(&target)
            .map(|building| (ops.build_requirements)(building.kind()))
            .unwrap_or_default();
        let missing = missing_requirements_for_target(buildings, target, &reqs);
        if let Some(source) = nearest_supply_with_items(buildings, target, &missing, ops) {
            if let Some(source_building) = buildings.get(&source) {
                let depot_to_source =
                    find_route_path_cells(depot, source, buildings, neighbors, ops);
                let source_to_target =
                    find_route_path_cells(source, target, buildings, neighbors, ops);
                if can_fulfill_reqs_from_building(source_building, &reqs)
                    && depot_to_source.is_some()
                    && source_to_target.is_some()
                {
                    return Some((target, source));
                }
            }
        }
    }
    None
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

fn apply_cargo_to_construction<B: PlanetBuildingAccess>(
    buildings: &mut HashMap<u32, B>,
    target: u32,
    cargo: &mut Vec<PlanetResourceStack>,
    ops: &UnitOps<B::Kind>,
) -> bool {
    let reqs = match buildings.get(&target) {
        Some(building) => {
            if building.build_paid() {
                return false;
            }
            (ops.build_requirements)(building.kind())
        }
        None => return false,
    };
    if reqs.is_empty() {
        return false;
    }
    for req in reqs.iter() {
        let Some(kind) = resource_kind_from_id(req.resource_id) else {
            return false;
        };
        if stack_amount(cargo, kind) < req.amount {
            return false;
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
    true
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
    let needed_total: u32 = reqs.iter().map(|req| req.amount).sum();
    if storage_total(cargo).saturating_add(needed_total) > capacity {
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
        if remaining > 0 {
            return false;
        }
    }
    true
}

fn nearest_supply_with_items<B: PlanetBuildingAccess>(
    buildings: &HashMap<u32, B>,
    target: u32,
    kinds: &[PlanetResourceKind],
    ops: &UnitOps<B::Kind>,
) -> Option<u32> {
    let mut best: Option<(u32, u32)> = None;
    for (cell_index, building) in buildings.iter() {
        let is_supply = (ops.is_warehouse)(building.kind())
            || (ops.is_base)(building.kind())
            || (ops.is_builder)(building.kind());
        if !is_supply || planet_is_under_construction(building) {
            continue;
        }
        let total: u32 = kinds
            .iter()
            .map(|kind| building.storage().get(kind).copied().unwrap_or(0))
            .sum();
        if total == 0 {
            continue;
        }
        let prio = if (ops.is_warehouse)(building.kind()) {
            0
        } else if (ops.is_builder)(building.kind()) {
            1
        } else {
            2
        };
        match best {
            Some((best_prio, _)) if prio > best_prio => {}
            _ => best = Some((prio, *cell_index)),
        }
    }
    best.map(|(_, cell)| if cell == target { target } else { cell })
}

fn first_refuel_station<B: PlanetBuildingAccess>(
    buildings: &HashMap<u32, B>,
    ops: &UnitOps<B::Kind>,
) -> Option<u32> {
    buildings
        .iter()
        .find(|(_, building)| {
            (ops.is_refuel)(building.kind()) && !planet_is_under_construction(*building)
        })
        .map(|(cell, _)| *cell)
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

fn find_direct_path_cells(start: u32, end: u32, neighbors: &[Vec<u32>]) -> Option<Vec<u32>> {
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

fn path_to_target<B: PlanetBuildingAccess>(
    start: u32,
    end: u32,
    buildings: &HashMap<u32, B>,
    neighbors: &[Vec<u32>],
    ops: &UnitOps<B::Kind>,
    allow_offroad: bool,
) -> Vec<u32> {
    if allow_offroad {
        find_route_path_cells(start, end, buildings, neighbors, ops)
            .or_else(|| find_direct_path_cells(start, end, neighbors))
            .unwrap_or_else(|| vec![start])
    } else {
        find_route_path_cells(start, end, buildings, neighbors, ops).unwrap_or_else(|| vec![start])
    }
}

fn resolved_target(unit: &PlanetUnit, action: &UnitAction) -> Option<u32> {
    match action {
        UnitAction::GoToNode(slot)
        | UnitAction::PickupAtNode(slot)
        | UnitAction::DeliverAtNode(slot)
        | UnitAction::BuildAtNode(slot)
        | UnitAction::RefuelAtNode(slot)
        | UnitAction::WaitAtNode(slot) => unit.nodes.get(*slot, unit.depot),
        UnitAction::Idle => None,
    }
}

fn can_load_from_target<B: PlanetBuildingAccess>(
    unit: &PlanetUnit,
    target: Option<u32>,
    buildings: &HashMap<u32, B>,
) -> bool {
    let Some(target) = target else {
        return false;
    };
    let Some(building) = buildings.get(&target) else {
        return false;
    };
    storage_total(&unit.cargo) < unit.capacity
        && building.storage().values().copied().sum::<u32>() > 0
        && !planet_is_under_construction(building)
}

fn can_unload_to_target<B: PlanetBuildingAccess>(
    unit: &PlanetUnit,
    target: Option<u32>,
    buildings: &HashMap<u32, B>,
    ops: &UnitOps<B::Kind>,
) -> bool {
    let Some(target) = target else {
        return false;
    };
    let Some(building) = buildings.get(&target) else {
        return false;
    };
    let used: u32 = building.storage().values().copied().sum();
    storage_total(&unit.cargo) > 0 && used < (ops.storage_capacity)(building.kind())
}

fn condition_matches<B: PlanetBuildingAccess>(
    unit: &PlanetUnit,
    condition: &UnitConditionExpr,
    target: Option<u32>,
    buildings: &HashMap<u32, B>,
    ops: &UnitOps<B::Kind>,
) -> bool {
    match condition {
        UnitConditionExpr::CargoEmpty => storage_total(&unit.cargo) == 0,
        UnitConditionExpr::CargoFull => storage_total(&unit.cargo) >= unit.capacity,
        UnitConditionExpr::CargoAtLeast(amount) => storage_total(&unit.cargo) >= *amount,
        UnitConditionExpr::HasAnyCargo => storage_total(&unit.cargo) > 0,
        UnitConditionExpr::FuelBelowRatio(ratio) => unit.fuel_ratio() < *ratio,
        UnitConditionExpr::AtTarget => target == Some(unit.current_cell()),
        UnitConditionExpr::CanLoadAtTarget => can_load_from_target(unit, target, buildings),
        UnitConditionExpr::CanUnloadAtTarget => can_unload_to_target(unit, target, buildings, ops),
        UnitConditionExpr::Always => true,
    }
}

fn select_action<B: PlanetBuildingAccess>(
    unit: &PlanetUnit,
    buildings: &HashMap<u32, B>,
    ops: &UnitOps<B::Kind>,
) -> UnitAction {
    if unit.preset_id == PlanetUnitPresetId::BuilderSupply {
        if let Some(target) = unit.nodes.construction {
            let target_paid = buildings
                .get(&target)
                .map(|b| b.build_paid())
                .unwrap_or(true);
            if target_paid {
                if storage_total(&unit.cargo) > 0 {
                    return UnitAction::DeliverAtNode(UnitNodeSlot::Depot);
                }
                return UnitAction::GoToNode(UnitNodeSlot::Depot);
            }
        }
    }
    let mut ordered = unit.behavior.rules.clone();
    ordered.sort_by(|a, b| b.priority.cmp(&a.priority));
    for rule in ordered.iter() {
        let target = resolved_target(unit, &rule.action);
        if condition_matches(unit, &rule.condition, target, buildings, ops) {
            return rule.action.clone();
        }
    }
    unit.behavior.fallback.clone()
}

fn current_target_invalid<B: PlanetBuildingAccess>(
    unit: &PlanetUnit,
    action: &UnitAction,
    buildings: &HashMap<u32, B>,
) -> bool {
    let expected = resolved_target(unit, action);
    if unit.current_target != expected {
        return true;
    }
    if let Some(target) = expected {
        !buildings.contains_key(&target)
    } else {
        false
    }
}

fn execute_action_at_target<B: PlanetBuildingAccess>(
    unit: &mut PlanetUnit,
    buildings: &mut HashMap<u32, B>,
    ops: &UnitOps<B::Kind>,
) -> bool {
    let Some(action) = unit.current_action.clone() else {
        return false;
    };
    let Some(target) = unit.current_target else {
        return false;
    };
    if unit.current_cell() != target {
        return false;
    }
    match action {
        UnitAction::PickupAtNode(_) => {
            if unit.preset_id == PlanetUnitPresetId::BuilderSupply {
                let Some(construction_target) = unit.nodes.construction else {
                    return false;
                };
                let Some(kind) = buildings.get(&construction_target).map(|b| b.kind()) else {
                    return false;
                };
                let reqs = (ops.build_requirements)(kind);
                if let Some(building) = buildings.get_mut(&target) {
                    let before = storage_total(&unit.cargo);
                    if take_reqs_from_building(building, &mut unit.cargo, unit.capacity, &reqs) {
                        return storage_total(&unit.cargo) != before;
                    }
                    return false;
                }
            }
            if let Some(building) = buildings.get_mut(&target) {
                let before = storage_total(&unit.cargo);
                load_unit_from_building(building, &mut unit.cargo, unit.capacity);
                return storage_total(&unit.cargo) != before;
            }
        }
        UnitAction::DeliverAtNode(_) => {
            if let Some(building) = buildings.get_mut(&target) {
                let before = storage_total(&unit.cargo);
                unload_unit_to_building(
                    &mut unit.cargo,
                    building,
                    (ops.storage_capacity)(building.kind()),
                );
                return storage_total(&unit.cargo) != before;
            }
        }
        UnitAction::BuildAtNode(_) => {
            return apply_cargo_to_construction(buildings, target, &mut unit.cargo, ops);
        }
        UnitAction::RefuelAtNode(_) => {
            if buildings
                .get(&target)
                .map(|building| {
                    (ops.is_refuel)(building.kind()) && !planet_is_under_construction(building)
                })
                .unwrap_or(false)
                && unit.fuel < unit.fuel_capacity
            {
                unit.fuel = unit.fuel_capacity;
                return true;
            }
        }
        UnitAction::GoToNode(_) | UnitAction::WaitAtNode(_) | UnitAction::Idle => {}
    }
    false
}

pub fn update_units_and_construction<B: PlanetBuildingAccess>(
    units: &mut Vec<PlanetUnit>,
    buildings: &mut HashMap<u32, B>,
    _sim_grid: &HexGrid,
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
                building.set_build_progress(
                    (building.build_progress() + dt).min(building.build_time()),
                );
                buildings_dirty = true;
            }
        }
    }

    let mut has_active_base_builder = units.iter().any(|unit| {
        unit.preset_id == PlanetUnitPresetId::BuilderSupply
            && buildings
                .get(&unit.depot)
                .map(|building| (ops.is_base)(building.kind()))
                .unwrap_or(false)
    });
    let active_construction_targets: Vec<u32> = units
        .iter()
        .filter_map(|unit| match unit.preset_id {
            PlanetUnitPresetId::BuilderSupply => unit.nodes.construction,
            _ => None,
        })
        .collect();

    for target in build_cells.iter().copied() {
        if has_active_base_builder {
            break;
        }
        let Some(building) = buildings.get(&target) else {
            continue;
        };
        if building.build_paid()
            || building.build_claimed()
            || active_construction_targets.contains(&target)
        {
            continue;
        }
        let reqs = (ops.build_requirements)(building.kind());
        if reqs.is_empty() {
            continue;
        }
        let missing = missing_requirements_for_target(buildings, target, &reqs);
        if missing.is_empty() {
            continue;
        }
        let Some(source) = nearest_supply_with_items(buildings, target, &missing, ops) else {
            continue;
        };
        let Some(source_building) = buildings.get(&source) else {
            continue;
        };
        if !can_fulfill_reqs_from_building(source_building, &reqs) {
            continue;
        }
        let Some((depot, _)) = buildings.iter().find(|(_, b)| {
            (ops.is_base)(b.kind()) && !planet_is_under_construction(*b) && b.auto_units_enabled()
        }) else {
            continue;
        };
        let mut unit = unit_for_preset(
            PlanetUnitPresetId::BuilderSupply,
            *depot,
            PlanetUnitNodeConfig {
                pickup: Some(source),
                dropoff: None,
                refuel: first_refuel_station(buildings, ops),
                construction: Some(target),
                wait: None,
            },
        );
        unit.path = vec![*depot];
        units.push(unit);
        if let Some(building) = buildings.get_mut(&target) {
            building.set_build_claimed(true);
        }
        has_active_base_builder = true;
        units_dirty = true;
        buildings_dirty = true;
    }

    let mut index = 0usize;
    while index < units.len() {
        if !buildings.contains_key(&units[index].depot) {
            units.remove(index);
            units_dirty = true;
            continue;
        }
        if units[index].preset_id == PlanetUnitPresetId::BuilderSupply {
            let depot_kind = buildings.get(&units[index].depot).map(|b| b.kind());
            let depot_is_base = depot_kind.map(|kind| (ops.is_base)(kind)).unwrap_or(false);
            let depot_is_builder = depot_kind
                .map(|kind| (ops.is_builder)(kind))
                .unwrap_or(false);

            if depot_is_base {
                units[index].speed = 2.75;
            } else if depot_is_builder {
                units[index].speed = 4.5;
            }

            let target_paid = units[index]
                .nodes
                .construction
                .and_then(|target| buildings.get(&target).map(|b| b.build_paid()))
                .unwrap_or(true);
            let needs_assignment = depot_is_builder
                && (target_paid
                    || units[index].nodes.construction.is_none()
                    || units[index].nodes.pickup.is_none()
                    || (storage_total(&units[index].cargo) == 0
                        && units[index].current_cell() == units[index].depot));
            if needs_assignment {
                let previous_target = units[index].nodes.construction;
                let reserved_targets: Vec<u32> = units
                    .iter()
                    .enumerate()
                    .filter(|(other_index, other_unit)| {
                        *other_index != index
                            && other_unit.preset_id == PlanetUnitPresetId::BuilderSupply
                    })
                    .filter_map(|(_, other_unit)| other_unit.nodes.construction)
                    .collect();
                if let Some((new_target, new_source)) = next_builder_assignment(
                    units[index].depot,
                    buildings,
                    previous_target,
                    &reserved_targets,
                    neighbors,
                    ops,
                ) {
                    units[index].nodes.construction = Some(new_target);
                    units[index].nodes.pickup = Some(new_source);
                    units[index].current_action = None;
                    units[index].current_target = None;
                    units[index].path = vec![units[index].current_cell()];
                    units[index].index = 0;
                    units[index].progress = 0.0;
                    units[index].assignment_check_timer = 0.0;
                    units_dirty = true;
                } else if storage_total(&units[index].cargo) == 0
                    && units[index].current_cell() == units[index].depot
                {
                    units[index].nodes.construction = None;
                    units[index].nodes.pickup = None;
                    units[index].current_action = Some(UnitAction::WaitAtNode(UnitNodeSlot::Depot));
                    units[index].current_target = Some(units[index].depot);
                    units[index].path = vec![units[index].depot];
                    units[index].index = 0;
                    units[index].progress = 0.0;
                }
            }
        }
        if units[index].preset_id == PlanetUnitPresetId::BuilderSupply
            && storage_total(&units[index].cargo) == 0
            && units[index].current_cell() == units[index].depot
            && buildings
                .get(&units[index].depot)
                .map(|building| (ops.is_base)(building.kind()))
                .unwrap_or(false)
            && units[index]
                .nodes
                .construction
                .and_then(|target| buildings.get(&target).map(|b| b.build_paid()))
                .unwrap_or(true)
        {
            units.remove(index);
            units_dirty = true;
            continue;
        }

        let action = select_action(&units[index], buildings, ops);
        let target = resolved_target(&units[index], &action);
        if current_target_invalid(&units[index], &action, buildings)
            || units[index].current_action.as_ref() != Some(&action)
        {
            units[index].current_action = Some(action.clone());
            units[index].current_target = target;
            units[index].progress = 0.0;
            let current_cell = units[index].current_cell();
            let allow_offroad = buildings
                .get(&units[index].depot)
                .map(|building| (ops.is_base)(building.kind()))
                .unwrap_or(false);
            units[index].path = match target {
                Some(target_cell) if target_cell != current_cell => path_to_target(
                    current_cell,
                    target_cell,
                    buildings,
                    neighbors,
                    ops,
                    allow_offroad,
                ),
                _ => vec![current_cell],
            };
            units[index].index = 0;
            units_dirty = true;
        }

        if execute_action_at_target(&mut units[index], buildings, ops) {
            buildings_dirty = true;
            units_dirty = true;
        }

        if units[index].preset_id == PlanetUnitPresetId::BuilderSupply
            && buildings
                .get(&units[index].depot)
                .map(|building| (ops.is_builder)(building.kind()))
                .unwrap_or(false)
        {
            let resting = storage_total(&units[index].cargo) == 0
                && units[index].current_cell() == units[index].depot
                && units[index].path.len() <= 1;
            if resting {
                units[index].assignment_check_timer += dt;
                if units[index].assignment_check_timer >= 1.0 {
                    units[index].assignment_check_timer = 0.0;
                    let previous_target = units[index].nodes.construction;
                    let reserved_targets: Vec<u32> = units
                        .iter()
                        .enumerate()
                        .filter(|(other_index, other_unit)| {
                            *other_index != index
                                && other_unit.preset_id == PlanetUnitPresetId::BuilderSupply
                        })
                        .filter_map(|(_, other_unit)| other_unit.nodes.construction)
                        .collect();
                    if let Some((new_target, new_source)) = next_builder_assignment(
                        units[index].depot,
                        buildings,
                        previous_target,
                        &reserved_targets,
                        neighbors,
                        ops,
                    ) {
                        units[index].nodes.construction = Some(new_target);
                        units[index].nodes.pickup = Some(new_source);
                        units[index].current_action = None;
                        units[index].current_target = None;
                        units[index].path = vec![units[index].current_cell()];
                        units[index].index = 0;
                        units[index].progress = 0.0;
                        units_dirty = true;
                    } else {
                        units[index].nodes.construction = None;
                        units[index].nodes.pickup = None;
                        units[index].current_action =
                            Some(UnitAction::WaitAtNode(UnitNodeSlot::Depot));
                        units[index].current_target = Some(units[index].depot);
                        units[index].path = vec![units[index].depot];
                        units[index].index = 0;
                        units[index].progress = 0.0;
                    }
                }
            } else {
                units[index].assignment_check_timer = 0.0;
            }
        }

        if units[index].fuel <= 0.0 || units[index].path.len() < 2 {
            index += 1;
            continue;
        }

        let distance_progress = dt * units[index].speed;
        let fuel_spend = dt * units[index].fuel_burn_rate;
        units[index].progress += distance_progress;
        units[index].fuel = (units[index].fuel - fuel_spend).max(0.0);

        while units[index].progress >= 1.0 && units[index].index + 1 < units[index].path.len() {
            units[index].progress -= 1.0;
            units[index].index += 1;
            units_dirty = true;
            if execute_action_at_target(&mut units[index], buildings, ops) {
                buildings_dirty = true;
                units_dirty = true;
            }
        }
        index += 1;
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
    }

    fn test_ops() -> UnitOps<TestKind> {
        UnitOps {
            is_route: |kind| kind == TestKind::Route,
            is_builder: |kind| kind == TestKind::Builder,
            is_base: |kind| kind == TestKind::Base,
            is_warehouse: |kind| kind == TestKind::Warehouse,
            is_refuel: |_kind| false,
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
            build_progress: 1.0,
            build_time: 1.0,
            build_paid: true,
            build_claimed: false,
        }
    }

    #[test]
    fn route_path_uses_route_cells_between_endpoints() {
        let mut buildings: HashMap<u32, TestBuilding> = HashMap::new();
        buildings.insert(1, make_building(TestKind::Route));
        let neighbors = vec![vec![1], vec![0, 2], vec![1]];
        let path = find_route_path_cells(0, 2, &buildings, &neighbors, &test_ops()).unwrap();
        assert_eq!(path, vec![0, 1, 2]);
    }

    #[test]
    fn fuel_rule_beats_cargo_rule() {
        let mut unit = unit_for_preset(
            PlanetUnitPresetId::Hauler,
            0,
            PlanetUnitNodeConfig {
                pickup: Some(1),
                dropoff: Some(2),
                refuel: Some(3),
                construction: None,
                wait: None,
            },
        );
        unit.fuel = 10.0;
        let buildings = HashMap::<u32, TestBuilding>::new();
        let action = select_action(&unit, &buildings, &test_ops());
        assert_eq!(action, UnitAction::RefuelAtNode(UnitNodeSlot::Refuel));
    }

    #[test]
    fn cargo_empty_picks_up() {
        let unit = unit_for_preset(
            PlanetUnitPresetId::Hauler,
            0,
            PlanetUnitNodeConfig {
                pickup: Some(1),
                dropoff: Some(2),
                refuel: Some(3),
                construction: None,
                wait: None,
            },
        );
        let buildings = HashMap::<u32, TestBuilding>::new();
        let action = select_action(&unit, &buildings, &test_ops());
        assert_eq!(action, UnitAction::PickupAtNode(UnitNodeSlot::Pickup));
    }

    #[test]
    fn cargo_loaded_delivers() {
        let mut unit = unit_for_preset(
            PlanetUnitPresetId::Hauler,
            0,
            PlanetUnitNodeConfig {
                pickup: Some(1),
                dropoff: Some(2),
                refuel: Some(3),
                construction: None,
                wait: None,
            },
        );
        unit.cargo.push(PlanetResourceStack {
            resource_id: resource_kind_to_id(PlanetResourceKind::Stone),
            amount: 2,
        });
        let buildings = HashMap::<u32, TestBuilding>::new();
        let action = select_action(&unit, &buildings, &test_ops());
        assert_eq!(action, UnitAction::DeliverAtNode(UnitNodeSlot::Dropoff));
    }

    #[test]
    fn build_action_pays_construction_when_requirements_met() {
        let mut buildings: HashMap<u32, TestBuilding> = HashMap::new();
        let mut housing = make_building(TestKind::Housing);
        housing.build_paid = false;
        housing.build_progress = 0.0;
        buildings.insert(2, housing);

        let mut unit = unit_for_preset(
            PlanetUnitPresetId::BuilderSupply,
            0,
            PlanetUnitNodeConfig {
                pickup: Some(1),
                dropoff: None,
                refuel: Some(3),
                construction: Some(2),
                wait: None,
            },
        );
        unit.current_action = Some(UnitAction::BuildAtNode(UnitNodeSlot::Construction));
        unit.current_target = Some(2);
        unit.path = vec![2];
        unit.cargo.push(PlanetResourceStack {
            resource_id: resource_kind_to_id(PlanetResourceKind::Stone),
            amount: 3,
        });

        assert!(execute_action_at_target(
            &mut unit,
            &mut buildings,
            &test_ops()
        ));
        assert!(buildings.get(&2).unwrap().build_paid());
    }

    #[test]
    fn builder_supply_capacity_covers_mine_requirements() {
        let unit = unit_for_preset(
            PlanetUnitPresetId::BuilderSupply,
            0,
            PlanetUnitNodeConfig {
                pickup: Some(1),
                dropoff: None,
                refuel: Some(0),
                construction: Some(2),
                wait: None,
            },
        );
        assert!(unit.capacity >= 22);
    }

    #[test]
    fn builder_supply_retargets_next_unpaid_work() {
        let mut buildings: HashMap<u32, TestBuilding> = HashMap::new();
        let mut builder = make_building(TestKind::Builder);
        builder.storage.insert(PlanetResourceKind::Stone, 10);
        buildings.insert(1, builder);

        let mut old_target = make_building(TestKind::Housing);
        old_target.build_paid = true;
        buildings.insert(2, old_target);

        let mut new_target = make_building(TestKind::Housing);
        new_target.build_paid = false;
        new_target.build_progress = 0.0;
        buildings.insert(3, new_target);

        let mut units = vec![unit_for_preset(
            PlanetUnitPresetId::BuilderSupply,
            1,
            PlanetUnitNodeConfig {
                pickup: Some(1),
                dropoff: None,
                refuel: Some(1),
                construction: Some(2),
                wait: None,
            },
        )];
        let neighbors = vec![vec![1], vec![0, 2, 3], vec![1], vec![1]];
        let grid = HexGrid {
            freq: 1,
            vertices: vec![],
            faces: vec![],
            face_centers: vec![],
            vertex_faces: vec![],
            base_face_buckets: vec![],
            base_face_neighbors: vec![],
        };

        let _ = update_units_and_construction(
            &mut units,
            &mut buildings,
            &grid,
            &neighbors,
            0.0,
            &test_ops(),
        );
        assert_eq!(units[0].nodes.construction, Some(3));
    }

    #[test]
    fn non_base_units_do_not_fall_back_to_direct_path() {
        let mut buildings: HashMap<u32, TestBuilding> = HashMap::new();
        buildings.insert(1, make_building(TestKind::Builder));
        let mut unit = unit_for_preset(
            PlanetUnitPresetId::BuilderSupply,
            1,
            PlanetUnitNodeConfig {
                pickup: Some(2),
                dropoff: None,
                refuel: None,
                construction: Some(2),
                wait: None,
            },
        );
        unit.current_action = None;
        unit.current_target = None;
        let mut units = vec![unit];
        let neighbors = vec![vec![1], vec![0, 2], vec![1]];
        let grid = HexGrid {
            freq: 1,
            vertices: vec![],
            faces: vec![],
            face_centers: vec![],
            vertex_faces: vec![],
            base_face_buckets: vec![],
            base_face_neighbors: vec![],
        };

        let _ = update_units_and_construction(
            &mut units,
            &mut buildings,
            &grid,
            &neighbors,
            0.0,
            &test_ops(),
        );
        assert_eq!(units[0].path, vec![1]);
    }
}
