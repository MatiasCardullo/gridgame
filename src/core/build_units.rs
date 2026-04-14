use crate::core::base_interior::{
    BaseInteriorState, InteriorBlockRecord, InteriorContainerRecord, InteriorLogisticsRequest,
    InteriorPartKind, InteriorPartStack,
};
use crate::core::{Axial, MachineBlock, MachineBlockType};

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
        let dist = (container.hex.q - target_hex.q).abs() + (container.hex.r - target_hex.r).abs();
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

pub fn tick_interior_forklifts(interior: &mut BaseInteriorState, dt: f32) {
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
        let Some(forklift) = interior
            .forklifts
            .iter_mut()
            .find(|forklift| forklift.request_target.is_none())
        else {
            break;
        };
        interior.logistics_requests[request_index].assigned_forklift = Some(forklift.id);
        forklift.request_target = Some(target_hex);
        forklift.source_hex = Some(source_hex);
        forklift.target = Some(source_hex);
    }

    let mut completed_targets: Vec<Axial> = Vec::new();
    let mut pickups: Vec<(usize, Axial, Axial)> = Vec::new();
    let mut deliveries: Vec<(usize, Axial)> = Vec::new();
    for (index, forklift) in interior.forklifts.iter_mut().enumerate() {
        forklift.move_progress += dt * 0.55;
        if forklift.move_progress < 1.0 {
            continue;
        }
        forklift.move_progress = 0.0;
        match (
            forklift.target,
            forklift.source_hex,
            forklift.request_target,
        ) {
            (Some(target), Some(source_hex), Some(request_target))
                if forklift.hex == target && target == source_hex =>
            {
                pickups.push((index, source_hex, request_target));
            }
            (Some(target), Some(_), Some(request_target))
                if forklift.hex == target && target == request_target =>
            {
                deliveries.push((index, request_target));
            }
            (Some(target), _, _) if forklift.hex == target => {
                forklift.target = Some(forklift.home);
            }
            (Some(target), _, _) => {
                forklift.hex = target;
            }
            (None, _, _) => {
                forklift.target = Some(forklift.home);
            }
        }
    }

    for (index, source_hex, request_target) in pickups {
        if let Some((block_kind, items)) = take_container_block(interior, source_hex) {
            if let Some(forklift) = interior.forklifts.get_mut(index) {
                forklift.carried_block = Some(block_kind);
                forklift.carried_items = items;
                forklift.target = Some(request_target);
            }
        } else if let Some(forklift) = interior.forklifts.get_mut(index) {
            forklift.target = Some(forklift.home);
            forklift.source_hex = None;
            forklift.request_target = None;
            forklift.carried_block = None;
            forklift.carried_items.clear();
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
                forklift.target = Some(forklift.home);
                forklift.source_hex = None;
                forklift.request_target = None;
            }
            completed_targets.push(request_target);
        } else if let Some(forklift) = interior.forklifts.get_mut(index) {
            forklift.carried_block = Some(block_kind);
            forklift.carried_items = items;
            forklift.target = Some(request_target);
        }
    }

    if !completed_targets.is_empty() {
        interior
            .logistics_requests
            .retain(|request| !completed_targets.contains(&request.target_hex));
    }
}
