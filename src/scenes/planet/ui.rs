use super::*;

pub(super) fn cell_deposit_summary(cell: &PlanetCellState) -> String {
    if cell.deposits.is_empty() {
        return "No deposits".to_string();
    }
    cell.deposits
        .iter()
        .map(|deposit| {
            format!(
                "{} {}/{}",
                resource_label(deposit.kind),
                deposit.remaining_amount,
                deposit.initial_amount
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

pub(super) fn hover_deposit_summary(cell: &PlanetCellState) -> String {
    if cell.deposits.is_empty() {
        return "No deposits".to_string();
    }
    cell.deposits
        .iter()
        .map(|deposit| {
            format!(
                "{} {}/{} grade:{}",
                resource_label(deposit.kind),
                deposit.remaining_amount,
                deposit.initial_amount,
                deposit.grade_permille
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

pub(super) fn visible_owned_unit_count(units: &[PlanetUnit], depot: u32) -> usize {
    units
        .iter()
        .filter(|unit| unit.depot == depot)
        .count()
        .min(6)
}

pub(super) fn extra_owned_unit_line_count(units: &[PlanetUnit], depot: u32) -> usize {
    usize::from(units.iter().filter(|unit| unit.depot == depot).count() > 6)
}

pub(super) fn builder_unit_action_text(unit: &PlanetUnit) -> String {
    let action = unit
        .current_action
        .as_ref()
        .map(|action| action.label())
        .unwrap_or("Idle");
    let target = unit
        .current_target
        .map(|target| target.to_string())
        .unwrap_or_else(|| "-".to_string());
    format!("{action} -> {target}")
}

pub(super) fn unit_parts_summary(definition: &PlanetUnitDefinition) -> String {
    definition
        .required_parts
        .iter()
        .map(|part| {
            if part.amount > 1 {
                format!("{}x {}", part.amount, part.item_id.label())
            } else {
                part.item_id.label().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn unit_attachments_summary(definition: &PlanetUnitDefinition) -> String {
    if definition.supported_attachments.is_empty() {
        "Attachments: fixed".to_string()
    } else {
        format!(
            "Attachments: {}",
            definition
                .supported_attachments
                .iter()
                .map(|attachment| attachment.label())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

pub(super) fn builder_unit_debug_state(unit: &PlanetUnit) -> BuilderUnitDebugState {
    BuilderUnitDebugState {
        depot: unit.depot,
        cell: unit.current_cell(),
        action: Some(builder_unit_action_text(unit)),
        target: unit.current_target,
        cargo: storage_total(&unit.cargo),
        path_len: unit.path.len(),
    }
}

pub(super) fn info_window_height_for_building(
    state: &PlanetState,
    cell_index: u32,
    building: &PlanetBuildingState,
) -> f32 {
    let mut content_height = 20.0 + 20.0;
    if state
        .sim_world
        .as_ref()
        .and_then(|world| world.cells.get(cell_index as usize))
        .is_some()
    {
        content_height += 20.0 + 26.0;
    }

    if planet_is_under_construction(building) {
        content_height += 18.0;
        content_height += 18.0;
        content_height += 18.0;
        if building.build_time > 0.0 {
            content_height += 20.0;
        }
    }

    content_height += 20.0 + 22.0;

    match building.kind {
        PlanetBuildingKind::Base => {
            content_height += 30.0;
        }
        PlanetBuildingKind::Mine => {
            if state
                .sim_world
                .as_ref()
                .and_then(|world| world.cells.get(cell_index as usize))
                .map(|cell| !cell.deposits.is_empty())
                .unwrap_or(false)
            {
                content_height += 18.0 + 24.0;
            }
            if !planet_is_under_construction(building) {
                content_height += 34.0;
            }
        }
        PlanetBuildingKind::Logistics => {
            if planet_is_under_construction(building) {
                content_height += 20.0;
            } else {
                content_height += 18.0 + 18.0 + 22.0;
                content_height += 34.0 + 34.0 + 30.0;
                if state.info_window.show_units {
                    let visible_units = visible_owned_unit_count(&state.units, cell_index) as f32;
                    let extra_lines = extra_owned_unit_line_count(&state.units, cell_index) as f32;
                    content_height += 18.0 + 18.0 * visible_units + 18.0 * extra_lines;
                }
            }
        }
        PlanetBuildingKind::Builder => {
            if planet_is_under_construction(building) {
                content_height += 20.0;
            } else {
                content_height += 24.0 + 24.0 + 28.0;
                content_height += 18.0 + 18.0 + 22.0;
                content_height += 34.0 + 34.0 + 30.0;
                if state.info_window.show_units {
                    let visible_units = visible_owned_unit_count(&state.units, cell_index) as f32;
                    let extra_lines = extra_owned_unit_line_count(&state.units, cell_index) as f32;
                    content_height += 18.0 + 18.0 * visible_units + 18.0 * extra_lines;
                }
                let log_lines = state
                    .builder_unit_log
                    .iter()
                    .filter(|entry| entry.starts_with(&format!("B{cell_index} ")))
                    .count()
                    .min(6) as f32;
                if log_lines > 0.0 {
                    content_height += 22.0 + 18.0 * log_lines;
                }
            }
        }
        _ => {}
    }

    WINDOW_TITLE_HEIGHT + 18.0 + content_height + 28.0 + 12.0
}

pub(super) fn sync_info_window_layout(state: &mut PlanetState) {
    if !state.info_window.open {
        return;
    }
    let Some(cell_index) = state.info_target_cell else {
        return;
    };
    let Some(building) = state.buildings.get(&BuildingSlot {
        cell_index,
        triangle_index: state.info_target_triangle,
    }) else {
        return;
    };
    if !matches!(
        building.kind,
        PlanetBuildingKind::Logistics | PlanetBuildingKind::Builder | PlanetBuildingKind::Base
    ) {
        state.info_window.show_units = false;
    }
    state.info_window.rect.h = info_window_height_for_building(state, cell_index, building);
}

pub(super) fn surface_label(surface: PlanetSurfaceClass) -> &'static str {
    match surface {
        PlanetSurfaceClass::Land => "Land",
        PlanetSurfaceClass::Coast => "Coast",
        PlanetSurfaceClass::Water => "Water",
    }
}

pub(super) fn resource_label(kind: PlanetResourceKind) -> &'static str {
    match kind {
        PlanetResourceKind::Iron => "Iron",
        PlanetResourceKind::Copper => "Copper",
        PlanetResourceKind::Gold => "Gold",
        PlanetResourceKind::Lead => "Lead",
        PlanetResourceKind::Zinc => "Zinc",
        PlanetResourceKind::Aluminum => "Aluminum",
        PlanetResourceKind::Lithium => "Lithium",
        PlanetResourceKind::Phosphate => "Phosphate",
        PlanetResourceKind::Stone => "Stone",
        PlanetResourceKind::FreshWater => "FreshWater",
        PlanetResourceKind::Salt => "Salt",
    }
}
