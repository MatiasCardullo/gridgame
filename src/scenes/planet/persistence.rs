use super::*;

impl PlanetState {
    pub(super) fn load_buildings_snapshot(&mut self, path: &str) {
        let Ok(contents) = fs::read_to_string(path) else {
            return;
        };
        let Ok(snapshot) = serde_json::from_str::<PlanetBuildingsSnapshot>(&contents) else {
            return;
        };
        if snapshot.format_version != 2 {
            eprintln!(
                "GridGame ignored incompatible building save (version {}); recreate building placements",
                snapshot.format_version
            );
            return;
        }
        self.buildings.clear();
        let mut ignored_legacy_route = false;
        for record in snapshot.buildings.into_iter() {
            if record.kind == PlanetBuildingKind::LegacyRoute {
                ignored_legacy_route = true;
                continue;
            }
            let mut storage: HashMap<PlanetResourceKind, u32> = HashMap::new();
            for entry in record.storage.into_iter() {
                let Some(kind) = resource_kind_from_id(entry.resource_id) else {
                    continue;
                };
                if entry.amount > 0 {
                    storage.insert(kind, entry.amount);
                }
            }
            let build_time = if record.build_time <= 0.0 {
                planet_build_time(record.kind)
            } else {
                record.build_time
            };
            let build_progress = if record.build_paid {
                record.build_progress.min(build_time)
            } else {
                record.build_progress
            };
            let interior = if record.kind == PlanetBuildingKind::Base {
                Some(record.interior.unwrap_or_default())
            } else {
                None
            };
            self.buildings.insert(
                BuildingSlot {
                    cell_index: record.cell_index,
                    triangle_index: record.triangle_index,
                },
                PlanetBuildingState {
                    kind: record.kind,
                    mine_extra: record.mine_extra,
                    storage,
                    build_progress,
                    build_time,
                    build_paid: record.build_paid,
                    build_claimed: record.build_claimed,
                    builder_units_desired: record.builder_units_desired,
                    builder_units_created: record.builder_units_created,
                    auto_units_enabled: record.auto_units_enabled,
                    auto_pick_source: record.auto_pick_source,
                    auto_pick_build: record.auto_pick_build,
                    auto_pick_refuel: record.auto_pick_refuel,
                    interior,
                },
            );
        }
        self.buildings_dirty = ignored_legacy_route;
    }

    pub(super) fn save_buildings_if_dirty(&mut self, path: &str) {
        if !self.buildings_dirty {
            return;
        }
        ensure_planet_data_dir();
        let snapshot = PlanetBuildingsSnapshot {
            format_version: 2,
            buildings: self
                .buildings
                .iter()
                .map(|(slot, building)| PlanetBuildingRecord {
                    cell_index: slot.cell_index,
                    triangle_index: slot.triangle_index,
                    kind: building.kind,
                    mine_extra: building.mine_extra.clone(),
                    storage: building
                        .storage
                        .iter()
                        .filter(|(_, amount)| **amount > 0)
                        .map(|(kind, amount)| PlanetBuildingStorageRecord {
                            resource_id: resource_kind_to_id(*kind),
                            amount: *amount,
                        })
                        .collect(),
                    build_progress: building.build_progress,
                    build_time: building.build_time,
                    build_paid: building.build_paid,
                    build_claimed: building.build_claimed,
                    builder_units_desired: building.builder_units_desired,
                    builder_units_created: building.builder_units_created,
                    auto_units_enabled: building.auto_units_enabled,
                    auto_pick_source: building.auto_pick_source,
                    auto_pick_build: building.auto_pick_build,
                    auto_pick_refuel: building.auto_pick_refuel,
                    interior: building.interior.clone(),
                })
                .collect(),
        };
        match serde_json::to_string_pretty(&snapshot) {
            Ok(json) => match fs::write(path, json) {
                Ok(()) => self.buildings_dirty = false,
                Err(error) => eprintln!("GridGame could not save planet buildings: {error}"),
            },
            Err(error) => eprintln!("GridGame could not serialize planet buildings: {error}"),
        }
    }

    pub(super) fn load_networks_snapshot(&mut self, path: &str) {
        let Ok(contents) = fs::read_to_string(path) else {
            return;
        };
        let Ok(snapshot) = serde_json::from_str::<PlanetNetworksSnapshot>(&contents) else {
            return;
        };
        if snapshot.format_version != 1 {
            eprintln!(
                "GridGame ignored incompatible network save (version {})",
                snapshot.format_version
            );
            return;
        }
        self.network_segments = snapshot.segments;
        self.next_network_id = snapshot.next_id.max(
            self.network_segments
                .iter()
                .map(|segment| segment.id.saturating_add(1))
                .max()
                .unwrap_or(0),
        );
        self.networks_dirty = false;
    }

    pub(super) fn save_networks_if_dirty(&mut self, path: &str) {
        if !self.networks_dirty {
            return;
        }
        ensure_planet_data_dir();
        let snapshot = PlanetNetworksSnapshot {
            format_version: 1,
            next_id: self.next_network_id,
            segments: self.network_segments.clone(),
        };
        match serde_json::to_string_pretty(&snapshot) {
            Ok(json) => match fs::write(path, json) {
                Ok(()) => self.networks_dirty = false,
                Err(error) => eprintln!("GridGame could not save planet networks: {error}"),
            },
            Err(error) => eprintln!("GridGame could not serialize planet networks: {error}"),
        }
    }

    pub(super) fn load_units_snapshot(&mut self, path: &str) {
        let Ok(contents) = fs::read_to_string(path) else {
            return;
        };
        let Ok(snapshot) = serde_json::from_str::<PlanetUnitsSnapshot>(&contents) else {
            self.units.clear();
            self.units_dirty = false;
            return;
        };
        self.units.clear();
        for record in snapshot.units.into_iter() {
            let cargo = record
                .cargo
                .into_iter()
                .filter(|stack| stack.amount > 0)
                .collect::<Vec<_>>();
            self.units.push(PlanetUnit {
                depot: record.depot,
                preset_id: record.preset_id,
                definition_id: record.definition_id.unwrap_or_else(|| {
                    unit_for_preset(record.preset_id, record.depot, record.nodes.clone())
                        .definition_id
                }),
                behavior: record.behavior,
                nodes: record.nodes,
                path: record.path,
                index: record.index,
                progress: record.progress,
                speed: record.speed,
                capacity: record.capacity,
                fuel: record.fuel,
                fuel_capacity: record.fuel_capacity,
                fuel_burn_rate: record.fuel_burn_rate,
                cargo,
                current_action: record.current_action,
                current_target: record.current_target,
                assignment_check_timer: record.assignment_check_timer,
            });
        }
        self.units_dirty = false;
    }

    pub(super) fn save_units_if_dirty(&mut self, path: &str) {
        if !self.units_dirty {
            return;
        }
        ensure_planet_data_dir();
        let snapshot = PlanetUnitsSnapshot {
            units: self
                .units
                .iter()
                .map(|unit| PlanetUnitRecord {
                    depot: unit.depot,
                    preset_id: unit.preset_id,
                    definition_id: Some(unit.definition_id),
                    behavior: unit.behavior.clone(),
                    nodes: unit.nodes.clone(),
                    index: unit.index,
                    progress: unit.progress,
                    speed: unit.speed,
                    capacity: unit.capacity,
                    fuel: unit.fuel,
                    fuel_capacity: unit.fuel_capacity,
                    fuel_burn_rate: unit.fuel_burn_rate,
                    path: unit.path.clone(),
                    cargo: unit.cargo.clone(),
                    current_action: unit.current_action.clone(),
                    current_target: unit.current_target,
                    assignment_check_timer: unit.assignment_check_timer,
                })
                .collect(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
            let _ = fs::write(path, json);
            self.units_dirty = false;
        }
    }
}
