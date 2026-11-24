use super::*;

#[test]
fn triangle_interaction_preview_has_five_and_six_slots() {
    let pentagon = triangle_slot_preview_placements(Vec3::ZERO, 1.0, 5);
    let hexagon = triangle_slot_preview_placements(Vec3::ZERO, 1.0, 6);

    assert_eq!(pentagon.len(), 5);
    assert_eq!(hexagon.len(), 6);
    assert!(triangle_slot_preview_placements(Vec3::ZERO, 1.0, 4).is_empty());
    assert!(hexagon.iter().all(|(center, _)| {
        ((center.x * center.x + center.z * center.z).sqrt() - 0.46).abs() < 1e-5
    }));
}

#[test]
fn paint_hex_accepts_rgb_and_rgba() {
    let rgb = parse_paint_hex("#12ab34").unwrap();
    assert_eq!((rgb.r * 255.0).round() as u8, 18);
    assert_eq!((rgb.g * 255.0).round() as u8, 171);
    assert_eq!((rgb.b * 255.0).round() as u8, 52);
    assert_eq!((rgb.a * 255.0).round() as u8, 255);

    let rgba = parse_paint_hex("12ab34cd").unwrap();
    assert_eq!((rgba.a * 255.0).round() as u8, 205);
    assert_eq!(format_paint_hex(rgba), "12AB34CD");
    assert!(parse_paint_hex("xyz").is_err());
}

#[test]
fn painting_targets_only_selected_faces_or_the_selected_group() {
    let mut metadata = FaceMetadata::for_face_count(3);
    let group = metadata.create_group(&[0, 2]).unwrap();
    let mut state = CadSceneState {
        selection_mode: SelectionMode::Face,
        selected_faces: vec![1],
        ..CadSceneState::default()
    };
    assert_eq!(paint_selection_faces(&state, &metadata).unwrap(), vec![1]);

    state.selected_group = Some(group);
    assert_eq!(
        paint_selection_faces(&state, &metadata).unwrap(),
        vec![0, 2]
    );

    state.selection_mode = SelectionMode::Vertex;
    assert!(paint_selection_faces(&state, &metadata).is_err());
}

#[test]
fn status_bar_reports_trim_instructions_and_prioritizes_errors() {
    let mut state = CadSceneState {
        trim_active: true,
        trim_phase: TrimPhase::Cutters,
        trim_path_active: true,
        ..CadSceneState::default()
    };
    let (message, is_error) = status_bar_message(&state, None);
    assert!(message.contains("Trim path"));
    assert!(message.contains("snaps are active"));
    assert!(!is_error);

    state.error = Some("Trim failed".to_string());
    assert_eq!(
        status_bar_message(&state, None),
        ("Trim failed".to_string(), true)
    );
}

#[test]
fn wheel_zoom_uses_small_relative_steps_and_respects_limits() {
    let zoomed_in = zoom_distance(10.0, 1.0);
    assert!(zoomed_in < 10.0);
    assert!(zoomed_in > 9.0);
    assert_eq!(
        zoom_distance(CAMERA_MIN_DISTANCE, 100.0),
        CAMERA_MIN_DISTANCE
    );
    assert_eq!(
        zoom_distance(CAMERA_MAX_DISTANCE, -100.0),
        CAMERA_MAX_DISTANCE
    );
}

#[test]
fn mesh_view_camera_matches_interaction_preview_cell_framing() {
    let preview_distance = 7.2;
    let mesh_view_distance = mesh_view_camera_distance(preview_distance);
    let mesh_view_extent = crate::core::object_assets::NORMALIZED_MODEL_EXTENT;
    let preview_extent = mesh_view_extent * crate::core::object_assets::BUILDING_PREVIEW_SCALE;

    assert!(
        (mesh_view_extent / mesh_view_distance - preview_extent / preview_distance).abs() < 1e-6
    );
    assert!(
        (BUILDING_PREVIEW_CELL_GUIDE_RADIUS / mesh_view_distance
            - BUILDING_PREVIEW_CELL_RADIUS / preview_distance)
            .abs()
            < 1e-6
    );
}

#[test]
fn mesh_view_allows_closer_zoom_than_interaction_preview() {
    let minimum_mesh_distance = mesh_view_preview_distance(CAMERA_MIN_DISTANCE);
    let minimum_preview_distance = 5.5;

    assert!(minimum_mesh_distance < minimum_preview_distance);
    assert!(minimum_mesh_distance >= MESH_VIEW_MIN_PREVIEW_DISTANCE);
}

#[test]
fn placement_dimensions_parse_local_primitive_measurements() {
    assert_eq!(
        parse_primitive_dimensions(MeshPrimitive::Cube, "2.5").unwrap(),
        vec3(2.5, 2.5, 2.5)
    );
    assert_eq!(
        parse_primitive_dimensions(MeshPrimitive::Polygon, "6").unwrap(),
        vec3(1.0, 1.0, 1.0)
    );
    assert_eq!(parse_polygon_sides("16").unwrap(), 16);
    assert!(parse_polygon_sides("2").is_err());
    assert!(parse_polygon_sides("17").is_err());
    assert!(parse_polygon_sides("6.5").is_err());
    assert_eq!(
        parse_primitive_dimensions(MeshPrimitive::Sphere, "0.75").unwrap(),
        vec3(1.5, 1.5, 1.5)
    );
    assert_eq!(
        parse_primitive_dimensions(MeshPrimitive::Cylinder, "0.5, 2").unwrap(),
        vec3(1.0, 2.0, 1.0)
    );
    assert!(parse_primitive_dimensions(MeshPrimitive::Cylinder, "0.5").is_err());
    assert!(parse_primitive_dimensions(MeshPrimitive::Cube, "NaN").is_err());
}

#[test]
fn arrow_adjustment_uses_requested_steps_and_selects_multi_value_fields() {
    let mut placement = PrimitivePlacement {
        primitive: MeshPrimitive::Cylinder,
        center: Some(Vec3::ZERO),
        dimensions: "0.5, 1".to_string(),
        input_focused: true,
        input_selected: false,
        dimension_index: 0,
    };
    adjust_placement_dimension(&mut placement, 0.1).unwrap();
    assert_eq!(placement.dimensions, "0.6, 1");
    adjust_placement_dimension(&mut placement, -0.01).unwrap();
    assert_eq!(placement.dimensions, "0.59, 1");
    select_placement_dimension(&mut placement, 1);
    assert_eq!(placement_dimension_label(&placement), "height");
    adjust_placement_dimension(&mut placement, 0.01).unwrap();
    assert_eq!(placement.dimensions, "0.59, 1.01");
    select_placement_dimension(&mut placement, -1);
    assert_eq!(placement_dimension_label(&placement), "radius");
}

#[test]
fn grid_snapping_and_construction_plane_intersection_are_stable() {
    assert_eq!(
        snap_model_point(vec3(0.26, 3.0, -0.74), 0.5),
        vec3(0.5, 3.0, -0.5)
    );
    let mesh = EditableMesh::primitive(MeshPrimitive::Cube)
        .transformed(|point| point + vec3(3.0, 2.0, -4.0));
    let model_point = vec3(3.25, 2.4, -3.5);
    let displayed = model_to_display_point(&mesh, model_point);
    assert!((display_to_model_point(&mesh, displayed) - model_point).length() < 1e-5);
    assert_eq!(
        construction_plane_hit(vec3(0.0, 2.0, 0.0), vec3(0.0, -1.0, 0.0)),
        Some(Vec3::ZERO)
    );
    assert_eq!(construction_plane_hit(vec3(0.0, 2.0, 0.0), Vec3::X), None);
}

#[test]
fn object_snap_modes_are_independent_and_enabled_by_default() {
    let mut settings = ObjectSnapSettings::default();
    for kind in [
        ObjectSnapKind::Center,
        ObjectSnapKind::Tangent,
        ObjectSnapKind::Quadrant,
        ObjectSnapKind::Intersection,
        ObjectSnapKind::Endpoint,
        ObjectSnapKind::Midpoint,
        ObjectSnapKind::Perpendicular,
    ] {
        assert!(settings.enabled(kind));
    }

    settings.set(ObjectSnapKind::Endpoint, false);
    assert!(!settings.enabled(ObjectSnapKind::Endpoint));
    assert!(settings.enabled(ObjectSnapKind::Midpoint));
    assert!(settings.enabled(ObjectSnapKind::Center));
}

#[test]
fn polar_tracking_uses_nearest_fifteen_degree_direction() {
    let (tracked, angle) = polar_track_delta(vec3(1.0, 0.0, 1.0), Vec3::X, Vec3::Z, 15.0);
    assert!((angle - 45.0).abs() < 1e-4);
    assert!((tracked - vec3(1.0, 0.0, 1.0)).length() < 1e-4);

    let (_, angle) = polar_track_delta(vec3(1.0, 0.0, 0.4), Vec3::X, Vec3::Z, 15.0);
    assert!((angle - 15.0).abs() < 1e-4);
}

#[test]
fn grid_snapping_respects_transform_axis_constraints() {
    assert_eq!(
        snap_transform_to_grid(
            vec3(0.2, 0.4, 0.3),
            vec3(0.5, 0.0, 0.4),
            TransformAxis::All,
            1.0
        ),
        vec3(1.0, 0.4, 1.0)
    );
    let x_snapped = snap_transform_to_grid(
        vec3(0.2, 0.4, 0.3),
        vec3(0.5, 0.0, 0.4),
        TransformAxis::X,
        1.0,
    );
    assert!((x_snapped - vec3(1.0, 0.4, 0.7)).length() < 1e-5);
}

#[test]
fn recognized_circular_loops_fit_and_non_circular_loops_are_rejected() {
    let points = (0..12)
        .map(|step| {
            let angle = std::f32::consts::TAU * step as f32 / 12.0;
            vec3(angle.cos() * 2.0, 0.0, angle.sin() * 2.0)
        })
        .collect::<Vec<_>>();
    let circle = fit_circular_loop(&points).unwrap();
    assert!(circle.center.length() < 1e-5);
    assert!((circle.radius - 2.0).abs() < 1e-5);
    let snap_points = circular_loop_snap_points(&circle, Some(vec3(4.0, 0.0, 0.0)));
    assert_eq!(
        snap_points
            .iter()
            .filter(|candidate| candidate.kind == ObjectSnapKind::Tangent)
            .count(),
        2
    );
    assert_eq!(
        snap_points
            .iter()
            .filter(|candidate| candidate.kind == ObjectSnapKind::Quadrant)
            .count(),
        4
    );
    assert_eq!(
        snap_points
            .iter()
            .find(|candidate| candidate.kind == ObjectSnapKind::Center)
            .unwrap()
            .position,
        circle.center
    );
    assert!(
        fit_circular_loop(&[
            vec3(-1.0, 0.0, -1.0),
            vec3(1.0, 0.0, -1.0),
            vec3(1.0, 0.0, 1.0),
            vec3(-1.0, 0.0, 1.0),
        ])
        .is_none()
    );
}

#[test]
fn circular_snap_detection_requires_a_closed_mesh_edge_loop() {
    let mut vertices = vec![Vertex::new2(Vec3::ZERO, Vec2::ZERO, WHITE)];
    for step in 0..8 {
        let angle = std::f32::consts::TAU * step as f32 / 8.0;
        vertices.push(Vertex::new2(
            vec3(angle.cos(), 0.0, angle.sin()),
            Vec2::ZERO,
            WHITE,
        ));
    }
    let mut indices = Vec::new();
    for step in 0..8 {
        indices.extend_from_slice(&[0, (step + 1) as u32, ((step + 1) % 8 + 1) as u32]);
    }
    let mesh = EditableMesh { vertices, indices };
    assert_eq!(circular_edge_loops(&mesh).len(), 1);
    let triangle = EditableMesh {
        vertices: vec![
            Vertex::new2(Vec3::ZERO, Vec2::ZERO, WHITE),
            Vertex::new2(Vec3::X, Vec2::ZERO, WHITE),
            Vertex::new2(Vec3::Y, Vec2::ZERO, WHITE),
        ],
        indices: vec![0, 1, 2],
    };
    assert!(circular_edge_loops(&triangle).is_empty());
}

#[test]
fn circular_snap_detection_recognizes_capped_cylinder_rims() {
    let segments = 8;
    let mut vertices = Vec::new();
    for y in [1.0, -1.0] {
        for step in 0..segments {
            let angle = std::f32::consts::TAU * step as f32 / segments as f32;
            vertices.push(Vertex::new2(
                vec3(angle.cos(), y, angle.sin()),
                Vec2::ZERO,
                WHITE,
            ));
        }
    }
    vertices.push(Vertex::new2(Vec3::Y, Vec2::ZERO, WHITE));
    vertices.push(Vertex::new2(-Vec3::Y, Vec2::ZERO, WHITE));
    let mut indices = Vec::new();
    for step in 0..segments {
        let next = (step + 1) % segments;
        let top = step as u32;
        let top_next = next as u32;
        let bottom = (step + segments) as u32;
        let bottom_next = (next + segments) as u32;
        indices.extend_from_slice(&[top, bottom, bottom_next, top, bottom_next, top_next]);
        indices.extend_from_slice(&[16, top_next, top, 17, bottom, bottom_next]);
    }
    let mesh = EditableMesh { vertices, indices };
    let circles = circular_edge_loops(&mesh);
    assert_eq!(circles.len(), 2);
    assert!(
        circles
            .iter()
            .all(|circle| (circle.radius - 1.0).abs() < 1e-5)
    );
}

#[test]
fn projected_intersection_snap_ignores_parallel_and_endpoint_contacts() {
    let crossing = projected_segment_intersection(
        vec2(-1.0, 0.0),
        vec2(1.0, 0.0),
        vec2(0.0, -1.0),
        vec2(0.0, 1.0),
    )
    .unwrap();
    assert!((crossing.0 - 0.5).abs() < 1e-5);
    assert!((crossing.1 - 0.5).abs() < 1e-5);
    assert_eq!(crossing.2, Vec2::ZERO);
    assert!(
        projected_segment_intersection(Vec2::ZERO, Vec2::X, vec2(0.0, 1.0), vec2(1.0, 1.0))
            .is_none()
    );
    assert_eq!(
        dimensions_from_click(MeshPrimitive::Sphere, Vec3::ZERO, Vec3::X),
        vec3(2.0, 2.0, 2.0)
    );
}

#[test]
fn trim_snap_projects_nearby_cursor_positions_to_face_edges() {
    let (amount, distance) =
        project_screen_point_to_segment(vec2(5.0, 3.0), vec2(0.0, 0.0), vec2(10.0, 0.0)).unwrap();
    assert!((amount - 0.5).abs() < 1e-6);
    assert!((distance - 3.0).abs() < 1e-6);

    let (amount, distance) =
        project_screen_point_to_segment(vec2(-2.0, 1.0), vec2(0.0, 0.0), vec2(10.0, 0.0)).unwrap();
    assert_eq!(amount, 0.0);
    assert!((distance - 5.0_f32.sqrt()).abs() < 1e-6);
    assert!(project_screen_point_to_segment(Vec2::ZERO, Vec2::ZERO, Vec2::ZERO).is_none());
}

#[test]
fn triangular_grid_snapping_is_centered_on_the_radial_pivot() {
    let center = vec3(2.0, 4.0, -3.0);
    let snapped = snap_model_point_to_triangular_grid(vec3(2.74, 5.0, -2.2), center, 1.0);

    assert!((snapped - vec3(2.5, 5.0, -3.0 + 3.0_f32.sqrt() * 0.5)).length() < 1e-5);
}

#[test]
fn grid_snapping_pulls_nearby_points_to_square_and_triangular_floors() {
    let floor = vec3(0.0, 2.0, 0.0);
    let near_floor = vec3(0.26, 2.2, 0.24);
    assert_eq!(
        snap_model_point_to_grid_layout(near_floor, floor, 1.0, false),
        vec3(0.0, 2.0, 0.0)
    );
    assert_eq!(
        snap_model_point_to_grid_layout(near_floor, floor, 1.0, true),
        vec3(0.0, 2.0, 0.0)
    );

    let above_threshold = vec3(0.26, 2.26, 0.24);
    assert_eq!(
        snap_model_point_to_grid_layout(above_threshold, floor, 1.0, false).y,
        above_threshold.y
    );
    assert_eq!(
        snap_model_point_to_grid_layout(above_threshold, floor, 1.0, true).y,
        above_threshold.y
    );
}

#[test]
fn radial_gizmo_snapping_uses_the_triangular_grid_and_preserves_axis_constraints() {
    let center = vec3(2.0, 4.0, -3.0);
    let pivot = vec3(2.74, 5.0, -2.2);
    let expected = vec3(2.5, 5.0, -3.0 + 3.0_f32.sqrt() * 0.5);

    assert!(
        (snap_transform_to_triangular_grid(pivot, Vec3::ZERO, TransformAxis::All, 1.0, center)
            - expected)
            .length()
            < 1e-5
    );
    assert_eq!(
        snap_transform_to_triangular_grid(pivot, Vec3::ZERO, TransformAxis::X, 1.0, center),
        vec3(expected.x, pivot.y, pivot.z)
    );
    assert_eq!(
        snap_transform_to_triangular_grid(pivot, Vec3::ZERO, TransformAxis::Y, 1.0, center),
        pivot
    );
}

#[test]
fn radial_ucs_and_grid_are_limited_to_a_sixty_degree_wedge() {
    let origin = vec3(2.0, 1.0, -4.0);
    let x_axis = Vec3::X;
    let z_axis = vec3(0.5, 0.0, 3.0_f32.sqrt() * 0.5);
    assert!((x_axis.dot(z_axis) - 0.5).abs() < 1e-6);

    let segments = triangular_grid_segments(origin, 1.0, 4.0);
    assert_eq!(segments.len(), 12);
    for (start, end) in segments {
        for point in [start, end] {
            let relative = point - origin;
            let r = 2.0 * relative.z / 3.0_f32.sqrt();
            let q = relative.x - r * 0.5;
            assert!(q >= -1e-5);
            assert!(r >= -1e-5);
            assert!(q + r <= 4.0 + 1e-5);
        }
    }
}

#[test]
fn radial_rotation_uses_the_ucs_origin_as_its_pivot() {
    let mesh = EditableMesh {
        vertices: vec![
            Vertex::new2(Vec3::X, Vec2::ZERO, WHITE),
            Vertex::new2(vec3(2.0, 0.0, 0.0), Vec2::ZERO, WHITE),
        ],
        indices: vec![],
    };

    let rotated = rotate_mesh_y_about(&mesh, std::f32::consts::FRAC_PI_2, Vec3::X);

    assert!((rotated.vertices[0].position - Vec3::X).length() < 1e-5);
    assert!((rotated.vertices[1].position - vec3(1.0, 0.0, 1.0)).length() < 1e-5);
}

#[test]
fn interaction_preview_randomizes_only_configured_neighbor_conditions() {
    let mut metadata = FaceMetadata::for_face_count(3);
    let main = metadata.create_group(&[0]).unwrap();
    metadata
        .set_group_program(main, FaceGroupProgram::RadialRepeat)
        .unwrap();
    let empty = metadata.create_group_under(&[1], Some(main)).unwrap();
    metadata
        .set_group_program(
            empty,
            FaceGroupProgram::IfNeighbor(RadialNeighborState::Empty),
        )
        .unwrap();
    let route = metadata.create_group_under(&[2], Some(main)).unwrap();
    metadata
        .set_group_program(
            route,
            FaceGroupProgram::IfNeighbor(RadialNeighborState::Route),
        )
        .unwrap();

    for seed in 1..16 {
        assert!(
            random_preview_neighbors(&metadata, seed)
                .iter()
                .all(|state| matches!(
                    state,
                    RadialNeighborState::Empty | RadialNeighborState::Route
                ))
        );
    }
}

#[test]
fn interaction_preview_neighbor_buildings_exclude_machine_blocks() {
    let catalog = ObjectCatalog::load();
    let selected = ObjectId::PlanetBase;
    for neighbor_index in 0..6 {
        let neighbor = preview_neighbor_object(&catalog, selected, 1, neighbor_index)
            .expect("catalog should contain another building");
        let entry = catalog
            .entries()
            .iter()
            .find(|entry| entry.definition.id == neighbor)
            .unwrap();
        assert_eq!(entry.definition.category, ObjectCategory::Buildings);
    }
}

#[test]
fn same_building_preview_neighbors_reuse_the_selected_model() {
    let catalog = ObjectCatalog::load();

    assert_eq!(
        preview_neighbor_for_condition(
            &catalog,
            ObjectId::PlanetBase,
            1,
            0,
            RadialNeighborState::Route,
        ),
        Some(ObjectId::PlanetBase)
    );
}

#[test]
fn conditioned_interaction_preview_uses_canonical_mesh_scale() {
    let color = Color::from_rgba(255, 255, 255, 255);
    let reference = EditableMesh {
        vertices: vec![
            Vertex::new2(vec3(-1.0, 0.0, -1.0), Vec2::ZERO, color),
            Vertex::new2(vec3(1.0, 0.0, -1.0), Vec2::ZERO, color),
            Vertex::new2(vec3(0.0, 1.0, 1.0), Vec2::ZERO, color),
        ],
        indices: vec![0, 1, 2],
    };
    let small_variant = reference.clone();
    let mut large_variant = reference.clone();
    large_variant
        .append(&EditableMesh {
            vertices: vec![
                Vertex::new2(vec3(8.0, 0.0, 0.0), Vec2::ZERO, color),
                Vertex::new2(vec3(9.0, 0.0, 0.0), Vec2::ZERO, color),
                Vertex::new2(vec3(8.0, 1.0, 0.0), Vec2::ZERO, color),
            ],
            indices: vec![0, 1, 2],
        })
        .unwrap();

    let small_preview =
        preview_mesh_with_reference_at(&small_variant, &reference, Some(Vec3::ZERO));
    let large_preview =
        preview_mesh_with_reference_at(&large_variant, &reference, Some(Vec3::ZERO));
    assert_eq!(
        small_preview.vertices[0].position,
        large_preview.vertices[0].position
    );
    assert_eq!(
        small_preview.vertices[1].position,
        large_preview.vertices[1].position
    );
}

#[test]
fn pending_primitive_is_added_only_after_explicit_confirmation() {
    let mut catalog = ObjectCatalog::load();
    let mut state = CadSceneState::default();
    let original_faces = catalog.get(state.selected).unwrap().face_count();
    start_primitive_placement(&mut state, MeshPrimitive::Cube);
    assert_eq!(
        catalog.get(state.selected).unwrap().face_count(),
        original_faces
    );
    confirm_primitive_placement(&mut catalog, &mut state);
    assert_eq!(
        catalog.get(state.selected).unwrap().face_count(),
        original_faces
    );

    let placement = state.primitive_placement.as_mut().unwrap();
    placement.center = Some(vec3(2.0, 0.0, -1.0));
    placement.dimensions = "2".to_string();
    confirm_primitive_placement(&mut catalog, &mut state);

    assert!(state.primitive_placement.is_none());
    assert_eq!(
        catalog.get(state.selected).unwrap().face_count(),
        original_faces + 12
    );
    let inserted_faces = (original_faces..original_faces + 12).collect::<Vec<_>>();
    let primitive_group = catalog
        .metadata(state.selected, None)
        .unwrap()
        .groups
        .iter()
        .find(|group| group.faces == inserted_faces)
        .unwrap();
    assert_eq!(primitive_group.name, "Cube");
    assert!(state.dirty);
    assert!(undo(&mut catalog, &mut state).unwrap());
    assert_eq!(
        catalog.get(state.selected).unwrap().face_count(),
        original_faces
    );
    assert!(
        catalog
            .metadata(state.selected, None)
            .unwrap()
            .groups
            .iter()
            .all(|group| group.faces != inserted_faces)
    );
}

#[test]
fn interaction_preview_composes_the_neighbor_branch_for_each_sector() {
    let color = Color::from_rgba(255, 255, 255, 255);
    let mut vertices = Vec::new();
    for offset in [0.0, 2.0, 4.0] {
        vertices.extend([
            Vertex::new2(vec3(offset, 0.0, 0.0), Vec2::ZERO, color),
            Vertex::new2(vec3(offset + 1.0, 0.0, 0.0), Vec2::ZERO, color),
            Vertex::new2(vec3(offset, 1.0, 0.0), Vec2::ZERO, color),
        ]);
    }
    let mesh = EditableMesh {
        vertices,
        indices: vec![0, 1, 2, 3, 4, 5, 6, 7, 8],
    };
    let mut metadata = FaceMetadata::for_face_count(3);
    let main = metadata.create_group(&[0]).unwrap();
    metadata
        .set_group_program(main, FaceGroupProgram::RadialRepeat)
        .unwrap();
    let empty = metadata.create_group_under(&[1], Some(main)).unwrap();
    metadata
        .set_group_program(
            empty,
            FaceGroupProgram::IfNeighbor(RadialNeighborState::Empty),
        )
        .unwrap();
    let route = metadata.create_group_under(&[2], Some(main)).unwrap();
    metadata
        .set_group_program(
            route,
            FaceGroupProgram::IfNeighbor(RadialNeighborState::Route),
        )
        .unwrap();

    let preview = compose_conditioned_preview(
        &mesh,
        &metadata,
        &[
            RadialNeighborState::Empty,
            RadialNeighborState::Route,
            RadialNeighborState::OtherBuilding,
            RadialNeighborState::Empty,
            RadialNeighborState::Route,
            RadialNeighborState::OtherBuilding,
        ],
        Vec3::ZERO,
    )
    .unwrap();
    assert_eq!(preview.face_count(), 10);
}

#[test]
fn radial_neighbor_preview_uses_the_center_and_two_adjacent_ring_cells() {
    let neighbors = [
        None,
        Some(ObjectId::PlanetFactory),
        Some(ObjectId::PlanetBase),
        None,
        Some(ObjectId::PlanetBase),
        Some(ObjectId::PlanetFactory),
    ];
    let conditions =
        radial_neighbor_conditions(ObjectId::PlanetBase, ObjectId::PlanetBase, &neighbors, 0);

    assert_eq!(
        conditions,
        [
            RadialNeighborState::Empty,
            RadialNeighborState::Empty,
            RadialNeighborState::OtherBuilding,
            RadialNeighborState::Route,
            RadialNeighborState::OtherBuilding,
            RadialNeighborState::Empty,
        ]
    );
    let condition_for = |owner: ObjectId, neighbor: Option<ObjectId>| match neighbor {
        None => RadialNeighborState::Empty,
        Some(neighbor) if neighbor == owner => RadialNeighborState::Route,
        Some(_) => RadialNeighborState::OtherBuilding,
    };
    for neighbor_index in 0..6 {
        let conditions = radial_neighbor_conditions(
            ObjectId::PlanetBase,
            ObjectId::PlanetBase,
            &neighbors,
            neighbor_index,
        );
        assert_eq!(
            conditions[(neighbor_index + 3) % 6],
            RadialNeighborState::Route
        );
        assert_eq!(
            conditions[(neighbor_index + 2) % 6],
            condition_for(ObjectId::PlanetBase, neighbors[(neighbor_index + 1) % 6])
        );
        assert_eq!(
            conditions[(neighbor_index + 4) % 6],
            condition_for(ObjectId::PlanetBase, neighbors[(neighbor_index + 5) % 6])
        );
        for (sector, condition) in conditions.iter().enumerate() {
            if sector != (neighbor_index + 2) % 6
                && sector != (neighbor_index + 3) % 6
                && sector != (neighbor_index + 4) % 6
            {
                assert_eq!(*condition, RadialNeighborState::Empty);
            }
        }
    }
    assert_eq!(
        radial_neighbor_conditions(ObjectId::PlanetBase, ObjectId::PlanetFactory, &neighbors, 0)[3],
        RadialNeighborState::OtherBuilding
    );
}

#[test]
fn mesh_view_hex_uses_explicit_or_default_radial_ucs_origin() {
    let mesh = EditableMesh {
        vertices: vec![
            Vertex::new2(vec3(1.0, 2.0, 3.0), Vec2::ZERO, WHITE),
            Vertex::new2(vec3(3.0, 4.0, 5.0), Vec2::ZERO, WHITE),
        ],
        indices: Vec::new(),
    };
    let (model_center, scale) = mesh_center_and_scale(&mesh);
    let mut metadata = FaceMetadata::for_face_count(1);
    let main = metadata.create_group(&[0]).unwrap();
    metadata
        .set_group_program(main, FaceGroupProgram::RadialRepeat)
        .unwrap();

    assert_eq!(
        mesh_cell_guide_center(&mesh, Some(&metadata)),
        (Vec3::ZERO - model_center) * scale
    );
    metadata.radial_pivot = Some([5.0, 6.0, 7.0]);
    assert_eq!(
        mesh_cell_guide_center(&mesh, Some(&metadata)),
        (vec3(5.0, 6.0, 7.0) - model_center) * scale
    );
}

#[test]
fn radial_neighbor_groups_appear_as_a_main_group_with_condition_children() {
    let mut metadata = FaceMetadata::for_face_count(3);
    let main_id = metadata.create_group(&[0]).unwrap();
    assert_eq!(
        metadata.group(main_id).unwrap().program,
        FaceGroupProgram::Geometry
    );
    metadata
        .set_group_program(main_id, FaceGroupProgram::RadialRepeat)
        .unwrap();
    let route_group = metadata.create_group_under(&[1], Some(main_id)).unwrap();
    metadata
        .set_group_program(
            route_group,
            FaceGroupProgram::IfNeighbor(RadialNeighborState::Route),
        )
        .unwrap();

    let items = sidebar_items(&metadata, 3);
    assert_eq!(items.first(), Some(&SidebarItem::Group(main_id)));
    assert_eq!(
        items,
        [
            SidebarItem::Group(main_id),
            SidebarItem::Face {
                index: 0,
                group: Some(main_id),
            },
            SidebarItem::Group(route_group),
            SidebarItem::Face {
                index: 1,
                group: Some(route_group),
            },
            SidebarItem::Face {
                index: 2,
                group: None,
            },
        ]
    );
}

#[test]
fn discarding_dirty_mesh_restores_original_and_leaves_edit_mode() {
    let mut catalog = ObjectCatalog::load();
    let original = catalog.get(ObjectId::PlanetBase).unwrap().clone();
    let original_metadata = catalog
        .metadata(ObjectId::PlanetBase, None)
        .unwrap()
        .clone();
    catalog.get_mut(ObjectId::PlanetBase).unwrap().vertices[0].position += Vec3::X;
    let mut state = CadSceneState {
        editing: true,
        dirty: true,
        original_mesh: Some(original.clone()),
        original_metadata: Some(original_metadata),
        ..CadSceneState::default()
    };

    restore_original_mesh(&mut catalog, &mut state).unwrap();

    assert_eq!(
        catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position,
        original.vertices[0].position
    );
    assert!(!state.editing);
    assert!(!state.dirty);
}

#[test]
fn closing_clean_editor_keeps_mesh_and_clears_edit_state() {
    let mut catalog = ObjectCatalog::load();
    let position = {
        let mesh = catalog.get_mut(ObjectId::PlanetBase).unwrap();
        mesh.vertices[0].position += Vec3::X;
        mesh.vertices[0].position
    };
    let mut state = CadSceneState {
        editing: true,
        selected_faces: vec![0],
        selected_group: Some(1),
        selected_vertices: vec![0],
        dirty: false,
        interaction_preview: true,
        primitive_placement: Some(PrimitivePlacement {
            primitive: MeshPrimitive::Cube,
            center: Some(Vec3::ZERO),
            dimensions: "1".to_string(),
            input_focused: true,
            input_selected: false,
            dimension_index: 0,
        }),
        ..CadSceneState::default()
    };

    close_editor(&mut state);

    assert_eq!(
        catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position,
        position
    );
    assert!(!state.editing);
    assert!(state.selected_faces.is_empty());
    assert!(state.selected_group.is_none());
    assert!(state.selected_vertices.is_empty());
    assert!(state.undo_stack.is_empty());
    assert!(!state.interaction_preview);
    assert!(state.primitive_placement.is_none());
}

#[test]
fn cad_api_can_add_and_transform_building_geometry_with_validation() {
    let mut catalog = ObjectCatalog::load();
    let mut state = CadSceneState {
        editing: true,
        ..CadSceneState::default()
    };
    let id = ObjectId::PlanetBase;
    let starting_faces = catalog.get(id).unwrap().face_count();
    let added = handle_api_command(
        &mut state,
        &mut catalog,
        ApiCommand::AddCadPrimitive {
            kind: ApiBuildingKind::Base,
            primitive: MeshPrimitive::Polygon,
            center: [0.0, 0.5, 0.0],
            dimensions: [2.0, 1.0, 2.0],
            rotation_degrees: [0.0; 3],
            sides: Some(5),
            color: None,
            replace: false,
        },
    );
    assert_eq!(added.status, 200);
    assert_eq!(catalog.get(id).unwrap().face_count(), starting_faces + 5);
    assert_eq!(added.body["saved"], false);

    let before = catalog.get(id).unwrap().vertices[0].position;
    let transformed = handle_api_command(
        &mut state,
        &mut catalog,
        ApiCommand::TransformCadBuilding {
            kind: ApiBuildingKind::Base,
            translation: [1.0, 0.0, 0.0],
            rotation_degrees: [0.0; 3],
            scale: [2.0, 1.0, 1.0],
        },
    );
    assert_eq!(transformed.status, 200);
    let after = catalog.get(id).unwrap().vertices[0].position;
    assert!((after.x - (before.x * 2.0 + 1.0)).abs() < 1e-5);
    assert!((after.y - before.y).abs() < 1e-5);

    let invalid = handle_api_command(
        &mut state,
        &mut catalog,
        ApiCommand::AddCadPrimitive {
            kind: ApiBuildingKind::Base,
            primitive: MeshPrimitive::Polygon,
            center: [0.0; 3],
            dimensions: [1.0; 3],
            rotation_degrees: [0.0; 3],
            sides: Some(17),
            color: None,
            replace: false,
        },
    );
    assert_eq!(invalid.status, 400);
    assert_eq!(catalog.get(id).unwrap().face_count(), starting_faces + 5);
    assert!(undo(&mut catalog, &mut state).unwrap());
    assert_eq!(catalog.get(id).unwrap().face_count(), starting_faces + 5);
    assert!(undo(&mut catalog, &mut state).unwrap());
    assert_eq!(catalog.get(id).unwrap().face_count(), starting_faces);
}

#[test]
fn cad_api_can_replace_building_geometry_and_set_primitive_color() {
    let mut catalog = ObjectCatalog::load();
    let mut state = CadSceneState::default();
    let id = ObjectId::PlanetFactory;
    let original_pivot = catalog.metadata(id, None).unwrap().radial_pivot;
    let reply = handle_api_command(
        &mut state,
        &mut catalog,
        ApiCommand::AddCadPrimitive {
            kind: ApiBuildingKind::Factory,
            primitive: MeshPrimitive::Cube,
            center: [0.0, 0.5, 0.0],
            dimensions: [2.0, 1.0, 2.0],
            rotation_degrees: [0.0; 3],
            sides: None,
            color: Some([40, 80, 160, 255]),
            replace: true,
        },
    );

    assert_eq!(reply.status, 200);
    assert_eq!(reply.body["saved"], false);
    let mesh = catalog.get(id).unwrap();
    assert_eq!(mesh.face_count(), 12);
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.color == [40, 80, 160, 255])
    );
    assert_eq!(
        catalog.metadata(id, None).unwrap().radial_pivot,
        original_pivot
    );
}

#[test]
fn vertex_edits_include_duplicate_face_corners() {
    let mut mesh = EditableMesh {
        vertices: vec![
            Vertex::new2(Vec3::ZERO, Vec2::ZERO, WHITE),
            Vertex::new2(Vec3::ZERO, Vec2::ZERO, WHITE),
            Vertex::new2(Vec3::X, Vec2::ZERO, WHITE),
        ],
        indices: vec![0, 1, 2],
    };
    let state = CadSceneState {
        selected_vertices: vec![0],
        ..CadSceneState::default()
    };
    let command = TransformCommand {
        mode: TransformMode::Move,
        pivot: Vec3::ZERO,
        values: Vec3::Y,
    };

    assert_eq!(selected_indices(&mesh, &state), vec![0, 1]);
    apply_transform_to_selection(&mut mesh, &state, command).unwrap();
    assert_eq!(mesh.vertices[0].position, Vec3::Y);
    assert_eq!(mesh.vertices[1].position, Vec3::Y);
    assert_eq!(mesh.vertices[2].position, Vec3::X);
}

#[test]
fn transform_input_requires_three_finite_values() {
    assert_eq!(
        parse_transform_values(TransformMode::Move, "-1.5, 2, 0").unwrap(),
        vec3(-1.5, 2.0, 0.0)
    );
    assert!(parse_transform_values(TransformMode::Move, "1, 2").is_err());
    assert!(parse_transform_values(TransformMode::Move, "1,,2,3").is_err());
    assert!(parse_transform_values(TransformMode::Move, "1, NaN, 0").is_err());
    assert!(parse_transform_values(TransformMode::Scale, "1, 0, 1").is_err());
}

#[test]
fn transform_axis_constraint_leaves_other_axes_unchanged() {
    assert_eq!(
        constrained_transform_values(TransformMode::Move, TransformAxis::Y, vec3(3.0, 4.0, 5.0)),
        vec3(0.0, 4.0, 0.0)
    );
    assert_eq!(
        constrained_transform_values(TransformMode::Scale, TransformAxis::X, vec3(2.0, 3.0, 4.0)),
        vec3(2.0, 1.0, 1.0)
    );
}

#[test]
fn rotation_and_scale_use_pivot_and_only_affect_selected_vertices() {
    let mut mesh = EditableMesh {
        vertices: vec![
            Vertex::new2(Vec3::X, Vec2::ZERO, WHITE),
            Vertex::new2(Vec3::ZERO, Vec2::ZERO, WHITE),
        ],
        indices: vec![],
    };
    let state = CadSceneState {
        selected_vertices: vec![0],
        ..CadSceneState::default()
    };

    apply_transform_to_selection(
        &mut mesh,
        &state,
        TransformCommand {
            mode: TransformMode::Rotate,
            pivot: Vec3::ZERO,
            values: vec3(0.0, 0.0, 90.0),
        },
    )
    .unwrap();
    assert!((mesh.vertices[0].position - Vec3::Y).length() < 1e-5);
    assert_eq!(mesh.vertices[1].position, Vec3::ZERO);

    apply_transform_to_selection(
        &mut mesh,
        &state,
        TransformCommand {
            mode: TransformMode::Scale,
            pivot: vec3(-1.0, 1.0, 0.0),
            values: vec3(2.0, 1.0, 1.0),
        },
    )
    .unwrap();
    assert!((mesh.vertices[0].position - vec3(1.0, 1.0, 0.0)).length() < 1e-5);
    assert_eq!(mesh.vertices[1].position, Vec3::ZERO);
}

#[test]
fn gizmo_preview_is_non_destructive_and_commit_is_undoable() {
    let mut catalog = ObjectCatalog::load();
    let original = catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position;
    let mut state = CadSceneState {
        selected_vertices: vec![0],
        ..CadSceneState::default()
    };
    let source = selected_mesh(&catalog, &state).unwrap().clone();
    let center = selected_bounds_center(&source, &selected_indices(&source, &state)).unwrap();
    let command = TransformCommand {
        mode: TransformMode::Move,
        pivot: center,
        values: Vec3::X,
    };
    let mut preview = source.clone();
    apply_transform_to_selection(&mut preview, &state, command).unwrap();
    state.gizmo_preview = Some(command);

    assert_eq!(
        catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position,
        original
    );

    commit_transform_command(&mut catalog, &mut state, command).unwrap();
    assert!(state.gizmo_preview.is_none());
    assert_eq!(state.undo_stack.len(), 1);
    let transformed = catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position;
    assert_ne!(transformed, original);
    assert!(undo(&mut catalog, &mut state).unwrap());
    assert_eq!(
        catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position,
        original
    );
}

#[test]
fn grouping_and_group_deletion_are_undoable() {
    let mut catalog = ObjectCatalog::load();
    let face_count = catalog.get(ObjectId::PlanetBase).unwrap().face_count();
    assert!(face_count > 2);
    let mut state = CadSceneState {
        editing: true,
        selection_mode: SelectionMode::Face,
        selected_faces: vec![0, 1],
        ..CadSceneState::default()
    };

    create_selected_group(&mut catalog, &mut state).unwrap();
    let group_id = state.selected_group.unwrap();
    assert_eq!(
        selected_metadata(&catalog, &state)
            .unwrap()
            .group(group_id)
            .unwrap()
            .faces,
        vec![0, 1]
    );
    assert!(undo(&mut catalog, &mut state).unwrap());
    assert!(
        selected_metadata(&catalog, &state)
            .unwrap()
            .group(group_id)
            .is_none()
    );
    assert!(redo(&mut catalog, &mut state).unwrap());
    assert_eq!(
        selected_metadata(&catalog, &state)
            .unwrap()
            .group(group_id)
            .unwrap()
            .faces,
        vec![0, 1]
    );

    delete_selected(&mut catalog, &mut state).unwrap();
    assert_eq!(
        selected_mesh(&catalog, &state).unwrap().face_count(),
        face_count - 2
    );
    assert!(
        selected_metadata(&catalog, &state)
            .unwrap()
            .group(group_id)
            .is_none()
    );
    assert!(undo(&mut catalog, &mut state).unwrap());
    assert_eq!(
        selected_mesh(&catalog, &state).unwrap().face_count(),
        face_count
    );
    assert_eq!(
        selected_metadata(&catalog, &state)
            .unwrap()
            .group(group_id)
            .unwrap()
            .faces,
        vec![0, 1]
    );
}

#[test]
fn geometry_history_is_undoable_and_capped() {
    let mut catalog = ObjectCatalog::load();
    let original_position = catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position;
    let mut state = CadSceneState {
        editing: true,
        selected_vertices: vec![0],
        ..CadSceneState::default()
    };
    let snapshot = capture_snapshot(&catalog, &state).unwrap();
    apply_transform_to_selection(
        selected_mesh_mut(&mut catalog, &state).unwrap(),
        &state,
        TransformCommand {
            mode: TransformMode::Move,
            pivot: Vec3::ZERO,
            values: Vec3::X,
        },
    )
    .unwrap();
    push_undo(&mut state, snapshot.clone());

    assert!(undo(&mut catalog, &mut state).unwrap());
    assert_eq!(
        catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position,
        original_position
    );
    assert!(redo(&mut catalog, &mut state).unwrap());
    assert_eq!(
        catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position,
        original_position + Vec3::X
    );

    for _ in 0..=HISTORY_LIMIT {
        push_undo(&mut state, snapshot.clone());
    }
    assert_eq!(state.undo_stack.len(), HISTORY_LIMIT);
}

fn trim_test_mesh() -> EditableMesh {
    let vertex = |position| Vertex::new2(position, Vec2::ZERO, WHITE);
    EditableMesh {
        vertices: vec![
            vertex(vec3(0.0, 0.0, 0.0)),
            vertex(vec3(2.0, 0.0, 0.0)),
            vertex(vec3(0.0, 2.0, 0.0)),
            vertex(vec3(0.5, -1.0, -1.0)),
            vertex(vec3(0.5, 3.0, -1.0)),
            vertex(vec3(0.5, 1.0, 1.0)),
        ],
        indices: vec![0, 1, 2, 3, 4, 5],
    }
}

#[test]
fn trim_splits_targets_and_keeps_both_sides_with_undo() {
    let mut catalog = ObjectCatalog::load();
    let mut state = CadSceneState {
        editing: true,
        trim_active: true,
        trim_phase: TrimPhase::Cutters,
        trim_target_faces: vec![0],
        trim_cutter_faces: vec![1],
        ..CadSceneState::default()
    };
    *selected_mesh_mut(&mut catalog, &state).unwrap() = trim_test_mesh();
    let metadata = selected_metadata_mut(&mut catalog, &state).unwrap();
    *metadata = FaceMetadata::for_face_count(2);
    metadata.face_names[0] = Some("Target".to_string());
    let group = metadata.create_group(&[0]).unwrap();
    let original = selected_mesh(&catalog, &state).unwrap().clone();

    apply_trim(&mut catalog, &mut state).unwrap();
    assert_eq!(selected_mesh(&catalog, &state).unwrap().face_count(), 4);
    let metadata = selected_metadata(&catalog, &state).unwrap();
    assert_eq!(metadata.face_names.len(), 4);
    assert_eq!(
        metadata
            .face_names
            .iter()
            .filter(|name| name.is_some())
            .count(),
        3
    );
    assert!(
        metadata
            .group(group)
            .unwrap()
            .faces
            .iter()
            .all(|&face| face < 4)
    );
    let target_centroids = state
        .selected_faces
        .iter()
        .map(|&face| {
            let start = face * 3;
            (0..3)
                .map(|slot| {
                    selected_mesh(&catalog, &state).unwrap().vertices
                        [selected_mesh(&catalog, &state).unwrap().indices[start + slot] as usize]
                        .position
                        .x
                })
                .sum::<f32>()
                / 3.0
        })
        .collect::<Vec<_>>();
    assert!(target_centroids.iter().any(|&x| x < 0.5));
    assert!(target_centroids.iter().any(|&x| x > 0.5));
    assert!(undo(&mut catalog, &mut state).unwrap());
    assert_eq!(selected_mesh(&catalog, &state).unwrap().face_count(), 2);
    assert_eq!(
        selected_mesh(&catalog, &state).unwrap().indices,
        original.indices
    );
    assert_eq!(
        selected_metadata(&catalog, &state).unwrap().face_names[0].as_deref(),
        Some("Target")
    );
}

#[test]
fn deleting_a_radial_group_removes_its_program_descendants() {
    let mut catalog = ObjectCatalog::load();
    let mut state = CadSceneState {
        editing: true,
        ..CadSceneState::default()
    };
    let mesh = EditableMesh::primitive(MeshPrimitive::Cube);
    let face_count = mesh.face_count();
    *selected_mesh_mut(&mut catalog, &state).unwrap() = mesh;

    let mut metadata = FaceMetadata::for_face_count(face_count);
    let radial_group = metadata.create_group(&[0]).unwrap();
    metadata
        .set_group_program(radial_group, FaceGroupProgram::RadialRepeat)
        .unwrap();
    let condition_group = metadata
        .create_group_under(&[1], Some(radial_group))
        .unwrap();
    metadata
        .set_group_program(
            condition_group,
            FaceGroupProgram::IfNeighbor(RadialNeighborState::Route),
        )
        .unwrap();
    metadata.radial_pivot = Some([1.0, 2.0, 3.0]);
    *selected_metadata_mut(&mut catalog, &state).unwrap() = metadata;
    state.selected_group = Some(radial_group);

    delete_selected(&mut catalog, &mut state).unwrap();
    assert_eq!(
        selected_mesh(&catalog, &state).unwrap().face_count(),
        face_count - 2
    );
    let metadata = selected_metadata(&catalog, &state).unwrap();
    assert!(metadata.groups.is_empty());
    assert_eq!(metadata.radial_pivot, None);

    assert!(undo(&mut catalog, &mut state).unwrap());
    assert_eq!(
        selected_mesh(&catalog, &state).unwrap().face_count(),
        face_count
    );
    assert_eq!(
        selected_metadata(&catalog, &state)
            .unwrap()
            .radial_main_group_id(),
        Some(radial_group)
    );
}

#[test]
fn trim_requires_intersection_and_leaves_mesh_unchanged_on_failure() {
    let mut catalog = ObjectCatalog::load();
    let mut state = CadSceneState {
        editing: true,
        trim_active: true,
        trim_phase: TrimPhase::Cutters,
        trim_target_faces: vec![0],
        trim_cutter_faces: vec![1],
        ..CadSceneState::default()
    };
    let mut mesh = trim_test_mesh();
    for index in 3..6 {
        mesh.vertices[index].position.x += 5.0;
    }
    *selected_mesh_mut(&mut catalog, &state).unwrap() = mesh;
    let original = selected_mesh(&catalog, &state).unwrap().clone();

    assert!(apply_trim(&mut catalog, &mut state).is_err());
    assert_eq!(
        selected_mesh(&catalog, &state).unwrap().indices,
        original.indices
    );
    assert!(state.undo_stack.is_empty());
    assert!(state.trim_active);
}

#[test]
fn trim_path_splits_selected_face_and_is_undoable() {
    let mut catalog = ObjectCatalog::load();
    let mut state = CadSceneState {
        editing: true,
        trim_active: true,
        trim_phase: TrimPhase::Cutters,
        trim_target_faces: vec![0],
        trim_path_face: Some(0),
        trim_path_points: vec![vec3(0.5, 0.0, 0.0), vec3(0.0, 0.0, -0.5)],
        ..CadSceneState::default()
    };
    let mut mesh = EditableMesh {
        vertices: [
            vec3(-0.5, 0.0, -0.5),
            vec3(0.5, 0.0, -0.5),
            vec3(0.5, 0.0, 0.5),
            vec3(-0.5, 0.0, 0.5),
        ]
        .into_iter()
        .map(|position| Vertex::new2(position, Vec2::ZERO, WHITE))
        .collect(),
        indices: vec![0, 2, 1, 0, 3, 2],
    };
    *selected_mesh_mut(&mut catalog, &state).unwrap() = mesh.clone();

    apply_trim(&mut catalog, &mut state).unwrap();
    assert_eq!(selected_mesh(&catalog, &state).unwrap().face_count(), 4);
    assert!(undo(&mut catalog, &mut state).unwrap());
    mesh = selected_mesh(&catalog, &state).unwrap().clone();
    assert_eq!(mesh.face_count(), 2);
}

#[test]
fn offset_copy_commits_and_is_undoable() {
    let mut catalog = ObjectCatalog::load();
    let mut state = CadSceneState {
        editing: true,
        selected_faces: vec![0],
        operation_value: "0.1".to_string(),
        ..CadSceneState::default()
    };
    let original_face_count = selected_mesh(&catalog, &state).unwrap().face_count();
    apply_offset(&mut catalog, &mut state).unwrap();
    assert_eq!(
        selected_mesh(&catalog, &state).unwrap().face_count(),
        original_face_count + 1
    );
    assert_eq!(state.selected_faces, vec![original_face_count]);
    assert!(undo(&mut catalog, &mut state).unwrap());
    assert_eq!(
        selected_mesh(&catalog, &state).unwrap().face_count(),
        original_face_count
    );
}

#[test]
fn mirror_copy_inherits_face_metadata_and_is_undoable() {
    let mut catalog = ObjectCatalog::load();
    let mut state = CadSceneState {
        editing: true,
        selected_faces: vec![0],
        ..CadSceneState::default()
    };
    let original_face_count = selected_mesh(&catalog, &state).unwrap().face_count();
    let group = {
        let metadata = selected_metadata_mut(&mut catalog, &state).unwrap();
        metadata.face_names[0] = Some("Mirrored face".to_string());
        metadata.create_group(&[0]).unwrap()
    };

    apply_mirror(&mut catalog, &mut state, Vec3::ZERO, Vec3::X).unwrap();
    let metadata = selected_metadata(&catalog, &state).unwrap();
    assert_eq!(
        metadata.face_names[original_face_count].as_deref(),
        Some("Mirrored face")
    );
    assert!(
        metadata
            .group(group)
            .unwrap()
            .faces
            .contains(&original_face_count)
    );
    assert!(undo(&mut catalog, &mut state).unwrap());
    assert_eq!(
        selected_mesh(&catalog, &state).unwrap().face_count(),
        original_face_count
    );
    assert_eq!(
        selected_metadata(&catalog, &state).unwrap().face_names[0].as_deref(),
        Some("Mirrored face")
    );
}

#[test]
fn group_lock_blocks_edits_and_hidden_faces_are_removed_from_render_mesh() {
    let mut catalog = ObjectCatalog::load();
    let mut state = CadSceneState {
        editing: true,
        ..CadSceneState::default()
    };
    let group_id = selected_metadata_mut(&mut catalog, &state)
        .unwrap()
        .create_group(&[0])
        .unwrap();
    let metadata = selected_metadata(&catalog, &state).unwrap();
    select_group(&mut state, metadata, group_id).unwrap();

    toggle_group_flag(&mut catalog, &mut state, group_id, GroupFlag::Locked).unwrap();
    assert!(ensure_selection_unlocked(&catalog, &state).is_err());
    let before = selected_mesh(&catalog, &state).unwrap().clone();
    assert!(
        commit_transform_command(
            &mut catalog,
            &mut state,
            TransformCommand {
                mode: TransformMode::Move,
                pivot: Vec3::ZERO,
                values: Vec3::X,
            },
        )
        .is_err()
    );
    assert_eq!(
        selected_mesh(&catalog, &state).unwrap().indices,
        before.indices
    );
    toggle_group_flag(&mut catalog, &mut state, group_id, GroupFlag::Locked).unwrap();
    assert!(ensure_selection_unlocked(&catalog, &state).is_ok());

    toggle_group_flag(&mut catalog, &mut state, group_id, GroupFlag::Hidden).unwrap();
    assert!(ensure_selection_unlocked(&catalog, &state).is_err());
    let hidden_faces = selected_metadata(&catalog, &state).unwrap().hidden_faces();
    assert!(hidden_faces.contains(&0));
    let mesh = selected_mesh(&catalog, &state).unwrap();
    let visible_mesh = mesh_without_faces(mesh, &hidden_faces);
    assert_eq!(visible_mesh.face_count(), mesh.face_count() - 1);
    assert_eq!(&visible_mesh.indices[..3], &mesh.indices[3..6]);
}
