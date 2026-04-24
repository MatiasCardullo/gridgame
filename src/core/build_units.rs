use std::collections::{HashMap, VecDeque};

use crate::core::base_interior::{
    BaseInteriorState, InteriorBlockRecord, InteriorContainerRecord, InteriorLogisticsRequest,
    InteriorPartKind, InteriorPartStack,
};
use crate::core::map_common::{MapOutline, in_bounds};
use crate::core::{Axial, MachineBlock, MachineBlockType, axial_neighbors};

fn hex_distance(a: Axial, b: Axial) -> i32 {
    let dq = a.q - b.q;
    let dr = a.r - b.r;
    let ds = (a.q + a.r) - (b.q + b.r);
    dq.abs().max(dr.abs()).max(ds.abs())
}

pub fn request_container_delivery(
    interior: &mut BaseInteriorState,
    requester_hex: Axial,
    target_hex: Axial,
    required_filter: Option<InteriorPartKind>,
) {
    if interior.block_at(target_hex).is_some() {
        return;
    }
    if interior
        .logistics_requests
        .iter()
        .any(|request| request.target_hex == target_hex)
    {
        return;
    }
    interior.logistics_requests.push(InteriorLogisticsRequest {
        requester_hex,
        target_hex,
        required_filter,
        assigned_forklift: None,
    });
}

fn nearest_container_for_request(
    interior: &BaseInteriorState,
    target_hex: Axial,
    required_filter: Option<InteriorPartKind>,
) -> Option<Axial> {
    let mut best: Option<(i32, Axial)> = None;
    for container in &interior.containers {
        let Some(block) = interior.block_at(container.hex) else {
            continue;
        };
        if !matches!(
            block.kind,
            MachineBlockType::Pallet | MachineBlockType::Crate
        ) {
            continue;
        }
        let has_match = match required_filter {
            Some(kind) => container
                .items
                .iter()
                .any(|stack| stack.kind == kind && stack.amount > 0),
            None => container.items.iter().any(|stack| stack.amount > 0),
        };
        if !has_match {
            continue;
        }
        if interior.forklifts.iter().any(|forklift| {
            forklift.source_hex == Some(container.hex) && forklift.request_target.is_some()
        }) {
            continue;
        }
        let dist = hex_distance(container.hex, target_hex);
        match best {
            Some((best_dist, _)) if dist >= best_dist => {}
            _ => best = Some((dist, container.hex)),
        }
    }
    best.map(|(_, hex)| hex)
}

fn take_container_block(
    interior: &mut BaseInteriorState,
    source_hex: Axial,
) -> Option<(MachineBlockType, Vec<InteriorPartStack>)> {
    let block_index = interior
        .blocks
        .iter()
        .position(|record| record.hex == source_hex)?;
    let container_index = interior
        .containers
        .iter()
        .position(|record| record.hex == source_hex)?;
    let block = interior.blocks.remove(block_index).block;
    if !matches!(
        block.kind,
        MachineBlockType::Pallet | MachineBlockType::Crate
    ) {
        return None;
    }
    let container = interior.containers.remove(container_index);
    Some((block.kind, container.items))
}

fn place_container_block(
    interior: &mut BaseInteriorState,
    target_hex: Axial,
    block_kind: MachineBlockType,
    items: Vec<InteriorPartStack>,
) -> bool {
    if interior.block_at(target_hex).is_some() {
        return false;
    }
    interior.blocks.push(InteriorBlockRecord {
        hex: target_hex,
        block: MachineBlock {
            kind: block_kind,
            rotation: 0,
        },
    });
    interior.containers.push(InteriorContainerRecord {
        hex: target_hex,
        items,
    });
    true
}

fn is_blocked_for_forklift(
    interior: &BaseInteriorState,
    hex: Axial,
    moving_id: u32,
    allow_block_cells: &[Axial],
) -> bool {
    if !in_bounds(hex, MapOutline::Hexagon) {
        return true;
    }
    let static_blocked = interior.block_at(hex).is_some() && !allow_block_cells.contains(&hex);
    if static_blocked {
        return true;
    }
    interior.forklifts.iter().any(|forklift| {
        forklift.id != moving_id
            && (forklift.hex == hex
                || (forklift.target == Some(hex) && forklift.move_progress > 0.55))
    })
}

// Finds shortest route on hex grid while respecting dynamic blockers.
fn find_forklift_route(
    interior: &BaseInteriorState,
    start: Axial,
    goal: Axial,
    moving_id: u32,
    allow_block_cells: &[Axial],
) -> Option<Vec<Axial>> {
    if start == goal {
        return Some(vec![start]);
    }
    let mut queue = VecDeque::new();
    let mut came_from: HashMap<Axial, Axial> = HashMap::new();
    queue.push_back(start);
    came_from.insert(start, start);

    while let Some(current) = queue.pop_front() {
        for next in axial_neighbors(current) {
            if came_from.contains_key(&next) {
                continue;
            }
            if next != goal && is_blocked_for_forklift(interior, next, moving_id, allow_block_cells)
            {
                continue;
            }
            if next == goal
                && is_blocked_for_forklift(interior, next, moving_id, allow_block_cells)
                && !allow_block_cells.contains(&goal)
            {
                continue;
            }
            came_from.insert(next, current);
            if next == goal {
                let mut path = vec![goal];
                let mut walk = goal;
                while walk != start {
                    walk = *came_from.get(&walk)?;
                    path.push(walk);
                }
                path.reverse();
                return Some(path);
            }
            queue.push_back(next);
        }
    }
    None
}

fn reset_route(forklift: &mut crate::core::base_interior::InteriorForkliftState) {
    forklift.route.clear();
    forklift.route_index = 0;
    forklift.target = None;
    forklift.move_progress = 0.0;
}

fn current_goal(forklift: &crate::core::base_interior::InteriorForkliftState) -> Option<Axial> {
    if forklift.carried_block.is_some() {
        forklift.request_target
    } else if forklift.source_hex.is_some() {
        forklift.source_hex
    } else if forklift.hex != forklift.home {
        Some(forklift.home)
    } else {
        None
    }
}

pub fn tick_interior_forklifts(interior: &mut BaseInteriorState, dt: f32) {
    for request in interior.logistics_requests.iter_mut() {
        if let Some(id) = request.assigned_forklift {
            if interior.forklifts.iter().all(|forklift| forklift.id != id) {
                request.assigned_forklift = None;
            }
        }
    }

    for request_index in 0..interior.logistics_requests.len() {
        if interior.logistics_requests[request_index]
            .assigned_forklift
            .is_some()
        {
            continue;
        }
        let target_hex = interior.logistics_requests[request_index].target_hex;
        let required_filter = interior.logistics_requests[request_index].required_filter;
        let Some(source_hex) = nearest_container_for_request(interior, target_hex, required_filter)
        else {
            continue;
        };
        let Some(forklift) = interior.forklifts.iter_mut().find(|forklift| {
            forklift.request_target.is_none()
                && forklift.source_hex.is_none()
                && forklift.carried_block.is_none()
        }) else {
            break;
        };
        interior.logistics_requests[request_index].assigned_forklift = Some(forklift.id);
        forklift.request_target = Some(target_hex);
        forklift.source_hex = Some(source_hex);
        reset_route(forklift);
    }

    let mut pickups: Vec<(usize, Axial, Axial)> = Vec::new();
    let mut deliveries: Vec<(usize, Axial)> = Vec::new();

    for index in 0..interior.forklifts.len() {
        let (id, source_hex, request_target, carried_block, home, hex) = {
            let forklift = &interior.forklifts[index];
            (
                forklift.id,
                forklift.source_hex,
                forklift.request_target,
                forklift.carried_block,
                forklift.home,
                forklift.hex,
            )
        };
        let Some(goal) = current_goal(&interior.forklifts[index]) else {
            let forklift = &mut interior.forklifts[index];
            reset_route(forklift);
            continue;
        };

        let mut allow_block_cells = Vec::new();
        if carried_block.is_some() {
            if let Some(target) = request_target {
                allow_block_cells.push(target);
            }
        } else if let Some(source) = source_hex {
            allow_block_cells.push(source);
        }

        let needs_repath = {
            let forklift = &interior.forklifts[index];
            forklift.route.is_empty()
                || forklift.route.last().copied() != Some(goal)
                || forklift.route_index >= forklift.route.len().saturating_sub(1)
                || forklift
                    .route
                    .get(forklift.route_index + 1)
                    .copied()
                    .map(|next| is_blocked_for_forklift(interior, next, id, &allow_block_cells))
                    .unwrap_or(true)
        };

        if needs_repath {
            let Some(route) = find_forklift_route(interior, hex, goal, id, &allow_block_cells)
            else {
                let forklift = &mut interior.forklifts[index];
                forklift.target = None;
                forklift.move_progress = 0.0;
                continue;
            };
            let forklift = &mut interior.forklifts[index];
            forklift.route = route;
            forklift.route_index = 0;
            forklift.move_progress = 0.0;
            forklift.target = forklift.route.get(1).copied();
        }

        let Some(next_hex) = interior.forklifts[index]
            .route
            .get(interior.forklifts[index].route_index + 1)
            .copied()
        else {
            continue;
        };

        let speed = 1.75;
        let mut reached_goal = false;
        {
            let forklift = &mut interior.forklifts[index];
            forklift.target = Some(next_hex);
            forklift.move_progress += dt * speed;
            if forklift.move_progress >= 1.0 {
                forklift.move_progress = 0.0;
                forklift.hex = next_hex;
                forklift.route_index = forklift.route_index.saturating_add(1);
                if let Some(step) = forklift.route.get(forklift.route_index + 1).copied() {
                    forklift.target = Some(step);
                } else {
                    forklift.target = Some(forklift.hex);
                    reached_goal = forklift.hex == goal;
                }
            }
        }

        if !reached_goal {
            continue;
        }
        if carried_block.is_none() {
            if let (Some(source), Some(request_target)) = (source_hex, request_target) {
                if goal == source {
                    pickups.push((index, source, request_target));
                }
            }
        } else if let Some(request_target) = request_target {
            if goal == request_target {
                deliveries.push((index, request_target));
            }
        } else if goal == home {
            let forklift = &mut interior.forklifts[index];
            reset_route(forklift);
        }
    }

    let mut completed_targets: Vec<Axial> = Vec::new();
    let mut release_assignments: Vec<(u32, Axial)> = Vec::new();

    for (index, source_hex, request_target) in pickups {
        if let Some((block_kind, items)) = take_container_block(interior, source_hex) {
            if let Some(forklift) = interior.forklifts.get_mut(index) {
                forklift.carried_block = Some(block_kind);
                forklift.carried_items = items;
                forklift.source_hex = None;
                reset_route(forklift);
            }
        } else if let Some(forklift) = interior.forklifts.get_mut(index) {
            let id = forklift.id;
            forklift.request_target = None;
            forklift.source_hex = None;
            forklift.carried_block = None;
            forklift.carried_items.clear();
            reset_route(forklift);
            release_assignments.push((id, request_target));
        }
    }

    for (index, request_target) in deliveries {
        let maybe_payload = match interior.forklifts.get_mut(index) {
            Some(forklift) => forklift
                .carried_block
                .take()
                .map(|block_kind| (block_kind, std::mem::take(&mut forklift.carried_items))),
            None => None,
        };
        let Some((block_kind, items)) = maybe_payload else {
            continue;
        };

        if place_container_block(interior, request_target, block_kind, items.clone()) {
            if let Some(forklift) = interior.forklifts.get_mut(index) {
                forklift.request_target = None;
                forklift.source_hex = None;
                reset_route(forklift);
            }
            completed_targets.push(request_target);
        } else if let Some(forklift) = interior.forklifts.get_mut(index) {
            forklift.carried_block = Some(block_kind);
            forklift.carried_items = items;
            reset_route(forklift);
        }
    }

    if !completed_targets.is_empty() {
        interior
            .logistics_requests
            .retain(|request| !completed_targets.contains(&request.target_hex));
    }

    for (forklift_id, request_target) in release_assignments {
        for request in interior.logistics_requests.iter_mut() {
            if request.target_hex == request_target
                && request.assigned_forklift == Some(forklift_id)
            {
                request.assigned_forklift = None;
            }
        }
    }
}
