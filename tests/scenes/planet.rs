use super::*;
use crate::core::planet_resources::{
    LithologyKind, PlanetCellState, PlanetDeposit, sim_world_for_tests,
};

#[test]
fn planet_noise_default_matches_saved_planet_snapshot() {
    let config = PlanetNoiseConfig::default();

    assert_eq!(config.noise_scale, 3.7);
    assert_eq!(config.height_amp, 0.9);
    assert_eq!(config.height_bias, -0.1);
    assert_eq!(config.lat_bias, 0.0);
    assert_eq!(config.sea_level, 0.3);
    assert_eq!(config.ice_start, 0.61);
    assert_eq!(config.ice_strength, 1.5);
    assert_eq!(config.subdivisions, 3);
}

#[test]
fn focus_orbit_clamps_camera_outside_relief_surface() {
    let config = PlanetNoiseConfig::default();
    let normal = Vec3::Z;
    let pivot = normal * focus_surface_radius(normal, true, &config);
    let (position, _, _) =
        focus_camera_pose(pivot, normal, Vec3::Y, 0.8, 1.25, 1.0, 0.12, true, &config);
    let minimum_radius = focus_surface_radius(position.normalize_or_zero(), true, &config) + 0.034;

    assert!(position.length() >= minimum_radius);
}

#[test]
fn focus_camera_roll_fades_from_incoming_up_to_local_up() {
    let normal = Vec3::new(0.8, 0.0, 0.6).normalize();
    let reference_up = Vec3::Y;
    let direction = focus_orbit_direction(normal, 0.8, 0.8);
    let incoming_up = focus_camera_up(-direction, normal, reference_up, 1.0);
    let halfway_up = focus_camera_up(-direction, normal, reference_up, 0.5);
    let local_up = focus_camera_up(-direction, normal, reference_up, 0.0);
    let expected_incoming_up = camera_up_from_reference(-direction, reference_up, normal);
    let expected_local_up = camera_up_from_reference(-direction, normal, reference_up);

    assert!(incoming_up.distance(expected_incoming_up) < 1e-5);
    assert!(local_up.distance(expected_local_up) < 1e-5);
    assert!(halfway_up.dot(incoming_up) > local_up.dot(incoming_up));
    assert!(decay_focus_orientation_weight(1.0, 0.5) < 1.0);
    assert!(decay_focus_orientation_weight(1.0, 2.0) < 0.0 + f32::EPSILON);
}

#[test]
fn focus_orbit_angles_recreate_camera_direction() {
    let normal = Vec3::new(0.3, 0.7, 0.6).normalize();
    let expected = focus_orbit_direction(normal, -1.1, 0.9);
    let (yaw, pitch) = focus_orbit_angles(normal, expected);

    assert!(focus_orbit_direction(normal, yaw, pitch).distance(expected) < 1e-5);
}

#[test]
fn focus_camera_pan_follows_screen_space_axes() {
    let normal = Vec3::new(0.3, 0.7, 0.6).normalize();
    let reference_up = Vec3::Y;
    let direction = focus_orbit_direction(normal, 0.7, 0.8);
    let up = focus_camera_up(-direction, normal, reference_up, 0.5);
    let right = up.cross(direction).normalize();

    let (yaw, pitch) =
        orbit_focus_camera_from_screen_delta(normal, 0.7, 0.8, reference_up, 0.5, -0.01, 0.0);
    let horizontal_direction = focus_orbit_direction(normal, yaw, pitch);
    let (yaw, pitch) =
        orbit_focus_camera_from_screen_delta(normal, 0.7, 0.8, reference_up, 0.5, 0.0, -0.01);
    let vertical_direction = focus_orbit_direction(normal, yaw, pitch);

    assert!((horizontal_direction - direction).dot(right) < 0.0);
    assert!((vertical_direction - direction).dot(up) > 0.0);
}

#[test]
fn focus_camera_blend_interpolates_from_planet_orbit() {
    let planet_position = Vec3::Z * 1.7;
    let focus_position = Vec3::X * 1.7;
    let focus_pivot = Vec3::X * 1.6;
    let (position, target, _) = blend_focus_camera(
        planet_position,
        Vec3::Y,
        focus_pivot,
        focus_position,
        Vec3::Y,
        0.5,
    );

    assert!(position.distance((planet_position + focus_position) * 0.5) < 1e-5);
    assert!(target.distance(focus_pivot * 0.5) < 1e-5);
}

#[test]
fn focus_camera_blend_preserves_planet_camera_roll() {
    let (_, _, up) = blend_focus_camera(
        Vec3::Z * 1.7,
        Vec3::Y,
        Vec3::X * 1.6,
        Vec3::X * 1.7,
        Vec3::Y,
        0.5,
    );

    assert!(up.distance(Vec3::Y) < 1e-5);
}

#[test]
fn planet_orbit_rotation_preserves_camera_up_when_leaving_focus() {
    let outward = Vec3::new(0.3, 0.7, 0.6).normalize();
    let desired_up = (Vec3::Y - outward * Vec3::Y.dot(outward)).normalize();
    let rotation = quat_from_forward_up(outward, desired_up);

    assert!((rotation * Vec3::Z).distance(outward) < 1e-5);
    assert!((rotation * Vec3::Y).distance(desired_up) < 1e-5);
}

#[test]
fn network_cursor_snaps_to_center_corner_and_side_midpoint() {
    let face_centers = (0..6)
        .map(|index| {
            let angle = std::f32::consts::TAU * index as f32 / 6.0;
            vec3(angle.cos() * 0.3, angle.sin() * 0.3, 0.95).normalize()
        })
        .collect::<Vec<_>>();
    let grid = HexGrid {
        freq: 0,
        vertices: vec![Vec3::Z],
        faces: Vec::new(),
        face_centers: face_centers.clone(),
        vertex_faces: vec![(0..6).collect()],
        base_face_buckets: Vec::new(),
        base_face_neighbors: Vec::new(),
    };
    assert_eq!(
        snapped_network_anchor(&grid, 0, Vec3::Z),
        Some(NetworkAnchor::CellCenter { cell_index: 0 })
    );
    assert_eq!(
        snapped_network_anchor(&grid, 0, face_centers[2]),
        Some(NetworkAnchor::CellCorner { face_index: 2 })
    );
    let midpoint = (face_centers[0] + face_centers[1]).normalize();
    assert_eq!(
        snapped_network_anchor(&grid, 0, midpoint),
        Some(NetworkAnchor::SideMidpoint {
            first_face: 0,
            second_face: 1
        })
    );
}

#[test]
fn network_arc_detail_scales_with_angular_length() {
    let short_end = vec3(0.99995, 0.0, 0.01).normalize();
    let medium_end = vec3(0.98, 0.0, 0.2).normalize();

    assert_eq!(network_arc_subdivisions(Vec3::X, short_end, 32), 1);
    assert!(network_arc_subdivisions(Vec3::X, medium_end, 32) < 32);
    assert_eq!(network_arc_subdivisions(Vec3::X, Vec3::NEG_X, 32), 32);
    assert_eq!(network_arc_subdivisions(Vec3::X, short_end, 0), 1);
}

#[test]
fn network_lines_render_above_the_planet_surface() {
    let radius = 1.6;

    assert!(network_overlay_radius(radius) > radius + TEXTURE_LAYER_OFFSET);
}

#[test]
fn network_hit_testing_runs_only_for_selection_or_network_actions() {
    assert!(!should_hit_test_network_segments(
        Some(PlanetTool::RouteLine),
        false,
        false,
        false
    ));
    assert!(!should_hit_test_network_segments(
        Some(PlanetTool::RouteLine),
        true,
        false,
        false
    ));
    assert!(should_hit_test_network_segments(
        Some(PlanetTool::RouteLine),
        false,
        true,
        false
    ));
    assert!(should_hit_test_network_segments(
        Some(PlanetTool::RouteLine),
        false,
        false,
        true
    ));
    assert!(!should_hit_test_network_segments(
        Some(PlanetTool::Building(PlanetBuildingKind::Housing)),
        false,
        false,
        false
    ));
    assert!(should_hit_test_network_segments(
        Some(PlanetTool::Building(PlanetBuildingKind::Housing)),
        true,
        false,
        false
    ));
    assert!(should_hit_test_network_segments(None, false, false, false));
}

#[test]
fn route_chain_clicks_add_consecutive_segments_and_small_anchor_markers() {
    let start = NetworkAnchor::CellCenter { cell_index: 3 };
    let corner = NetworkAnchor::CellCorner { face_index: 7 };
    let midpoint = NetworkAnchor::SideMidpoint {
        first_face: 7,
        second_face: 9,
    };

    assert_eq!(advance_network_chain(None, start), (Some(start), None));
    assert_eq!(
        advance_network_chain(Some(start), corner),
        (
            Some(corner),
            Some(NetworkPlacement::AnchorSegment { start, end: corner })
        )
    );
    assert_eq!(
        advance_network_chain(Some(corner), corner),
        (Some(corner), None)
    );
    assert_eq!(
        advance_network_chain(Some(corner), midpoint),
        (
            Some(midpoint),
            Some(NetworkPlacement::AnchorSegment {
                start: corner,
                end: midpoint,
            })
        )
    );
    assert!(network_anchor_marker_radius(0.01, 1.0) < 0.012);
    assert_eq!(network_anchor_marker_radius(1.0, 1.0), 0.004);
}

#[test]
fn planet_buildings_fill_the_actual_local_cell_at_cad_scale() {
    for sides in [5, 6] {
        for radius in [0.7, 1.4] {
            let points = (0..sides)
                .map(|index| {
                    let angle = std::f32::consts::TAU * index as f32 / sides as f32;
                    vec2(angle.cos() * radius, angle.sin() * radius)
                })
                .collect::<Vec<_>>();
            let (_, cell_radius) = cell_building_geometry(&points).unwrap();

            assert!((cell_radius - radius).abs() < 1e-6);
            let scaled_extent =
                building_model_scale_for_cell(cell_radius) * NORMALIZED_MODEL_EXTENT;
            assert!((scaled_extent / cell_radius - BUILDING_CELL_FILL_RATIO).abs() < 1e-6);
        }
    }
}

#[test]
fn planet_building_scale_is_stable_across_conditioned_geometry() {
    let small = EditableMesh {
        vertices: vec![
            Vertex::new2(vec3(-1.0, 2.0, -1.0), Vec2::ZERO, WHITE),
            Vertex::new2(vec3(1.0, 4.0, 1.0), Vec2::ZERO, WHITE),
        ],
        indices: Vec::new(),
    };
    let mut large = small.clone();
    large.vertices.extend([
        Vertex::new2(vec3(-10.0, 2.0, -10.0), Vec2::ZERO, WHITE),
        Vertex::new2(vec3(10.0, 4.0, 10.0), Vec2::ZERO, WHITE),
    ]);
    let reference = EditableMesh {
        vertices: vec![
            Vertex::new2(vec3(-10.0, -5.0, -10.0), Vec2::ZERO, WHITE),
            Vertex::new2(vec3(10.0, 15.0, 10.0), Vec2::ZERO, WHITE),
        ],
        indices: Vec::new(),
    };

    let small = normalize_building_mesh(&small, &reference);
    let large = normalize_building_mesh(&large, &reference);

    assert_eq!(small.vertices[0].position, large.vertices[0].position);
    assert_eq!(small.vertices[1].position, large.vertices[1].position);
    assert!((small.vertices[0].position.y).abs() < 1e-6);
    assert!((small.vertices[1].position.y - 0.24).abs() < 1e-6);
    assert!(
        (building_model_scale_for_cell(1.0) * NORMALIZED_MODEL_EXTENT - BUILDING_CELL_FILL_RATIO)
            .abs()
            < 1e-6
    );
}

#[test]
fn radial_neighbors_distinguish_same_building_from_other_buildings() {
    assert_eq!(
        radial_neighbor_state(PlanetBuildingKind::Base, Some(PlanetBuildingKind::Base)),
        RadialNeighborState::Route
    );
    assert_eq!(
        radial_neighbor_state(PlanetBuildingKind::Base, Some(PlanetBuildingKind::Mine)),
        RadialNeighborState::OtherBuilding
    );
    assert_eq!(
        radial_neighbor_state(PlanetBuildingKind::Base, None),
        RadialNeighborState::Empty
    );
}

#[test]
fn legacy_route_buildings_deserialize_as_unsupported() {
    let kind: PlanetBuildingKind = serde_json::from_str("\"Route\"").unwrap();

    assert_eq!(kind, PlanetBuildingKind::LegacyRoute);
    assert_eq!(building_object_id(kind), None);
}

fn test_building() -> PlanetBuildingState {
    PlanetBuildingState {
        kind: PlanetBuildingKind::Mine,
        mine_extra: vec![1],
        storage: HashMap::new(),
        build_progress: 0.0,
        build_time: 1.0,
        build_paid: true,
        build_claimed: false,
        builder_units_desired: 0,
        builder_units_created: 0,
        auto_units_enabled: true,
        auto_pick_source: false,
        auto_pick_build: false,
        auto_pick_refuel: false,
        interior: None,
    }
}

fn test_cell(deposits: Vec<PlanetDeposit>) -> PlanetCellState {
    PlanetCellState {
        surface: PlanetSurfaceClass::Land,
        height: 0.0,
        slope: 0.0,
        lithology: LithologyKind::Clastic,
        deposits,
        buildable: true,
    }
}

fn test_building_with_builder_kits(kits: u32) -> PlanetBuildingState {
    let mut building = test_building();
    building.kind = PlanetBuildingKind::Base;
    let mut interior = BaseInteriorState::default();
    interior.blocks.clear();
    interior.containers.clear();
    interior.belt_items.clear();
    interior.mechanic_arms.clear();
    interior.assembly_nodes.clear();
    interior.forklifts.clear();
    interior.logistics_requests.clear();
    if kits > 0 {
        interior
            .assembly_nodes
            .push(crate::core::base_interior::AssemblyNodeState {
                hex: Axial { q: 0, r: 0 },
                selected_recipe: AssemblerRecipeId::BuilderTender,
                inserted: Vec::new(),
                completed_outputs: vec![crate::core::base_interior::InteriorPartStack {
                    kind: InteriorPartKind::BuilderTenderKit,
                    amount: kits,
                }],
            });
    }
    building.interior = Some(interior);
    building
}

fn empty_unit_nodes() -> PlanetUnitNodeConfig {
    PlanetUnitNodeConfig {
        pickup: None,
        dropoff: None,
        refuel: None,
        construction: None,
        wait: None,
    }
}

#[test]
fn builder_tender_spawn_rejects_missing_kit_without_adding_unit() {
    let mut buildings = HashMap::from([(0, test_building_with_builder_kits(0))]);
    let mut units = Vec::new();

    let result = create_planet_unit(
        &mut buildings,
        &mut units,
        PlanetUnitDefinitionId::BuilderTender,
        0,
        empty_unit_nodes(),
    );

    assert_eq!(result, Err(UnitSpawnError::MissingBuilderTenderKit));
    assert!(units.is_empty());
    assert_eq!(
        buildings[&0]
            .interior
            .as_ref()
            .unwrap()
            .builder_tender_kits(),
        0
    );
}

#[test]
fn builder_tender_spawn_consumes_one_completed_kit() {
    let mut buildings = HashMap::from([(0, test_building_with_builder_kits(2))]);
    let mut units = Vec::new();

    let result = create_planet_unit(
        &mut buildings,
        &mut units,
        PlanetUnitDefinitionId::BuilderTender,
        0,
        empty_unit_nodes(),
    );

    assert_eq!(result, Ok((0, true)));
    assert_eq!(units.len(), 1);
    assert_eq!(
        buildings[&0]
            .interior
            .as_ref()
            .unwrap()
            .builder_tender_kits(),
        1
    );
}

#[test]
fn failed_unit_definition_does_not_consume_builder_tender_kit() {
    let mut buildings = HashMap::from([(0, test_building_with_builder_kits(1))]);
    let mut units = Vec::new();

    let result = create_planet_unit(
        &mut buildings,
        &mut units,
        PlanetUnitDefinitionId::ModularExcavator,
        0,
        empty_unit_nodes(),
    );

    assert_eq!(result, Err(UnitSpawnError::UnsupportedDefinition));
    assert!(units.is_empty());
    assert_eq!(
        buildings[&0]
            .interior
            .as_ref()
            .unwrap()
            .builder_tender_kits(),
        1
    );
}

#[test]
fn mine_total_remaining_sums_center_and_expanded_cells() {
    let building = test_building();
    let sim_world = sim_world_for_tests(vec![
        test_cell(vec![PlanetDeposit {
            kind: PlanetResourceKind::Iron,
            initial_amount: 12,
            remaining_amount: 9,
            grade_permille: 800,
        }]),
        test_cell(vec![PlanetDeposit {
            kind: PlanetResourceKind::Iron,
            initial_amount: 8,
            remaining_amount: 4,
            grade_permille: 800,
        }]),
    ]);

    assert_eq!(mine_total_remaining(Some(&sim_world), 0, &building), 13);
}

#[test]
fn expandable_mine_cells_only_add_matching_adjacent_deposits() {
    let building = test_building();
    let sim_world = sim_world_for_tests(vec![
        test_cell(vec![PlanetDeposit {
            kind: PlanetResourceKind::Iron,
            initial_amount: 10,
            remaining_amount: 10,
            grade_permille: 900,
        }]),
        test_cell(vec![PlanetDeposit {
            kind: PlanetResourceKind::Iron,
            initial_amount: 7,
            remaining_amount: 7,
            grade_permille: 900,
        }]),
        test_cell(vec![PlanetDeposit {
            kind: PlanetResourceKind::Iron,
            initial_amount: 5,
            remaining_amount: 5,
            grade_permille: 900,
        }]),
        test_cell(vec![PlanetDeposit {
            kind: PlanetResourceKind::Copper,
            initial_amount: 6,
            remaining_amount: 6,
            grade_permille: 900,
        }]),
    ]);
    let neighbors = vec![vec![1, 3], vec![0, 2], vec![1], vec![0]];

    assert_eq!(
        expandable_mine_cells(&sim_world, &neighbors, 0, &building),
        vec![2]
    );
}
