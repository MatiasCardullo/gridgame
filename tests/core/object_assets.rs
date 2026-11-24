use super::*;

#[test]
fn shared_building_scale_matches_the_cad_cell_and_normalizes_mesh_extent() {
    let color = Color::from_rgba(255, 255, 255, 255);
    let mesh = EditableMesh {
        vertices: vec![
            Vertex::new2(vec3(-1.0, 0.0, -2.0), Vec2::ZERO, color),
            Vertex::new2(vec3(3.0, 2.0, 2.0), Vec2::ZERO, color),
        ],
        indices: Vec::new(),
    };
    let (center, scale) = mesh_center_and_scale(&mesh);

    assert_eq!(center, vec3(1.0, 1.0, 0.0));
    assert!((scale - NORMALIZED_MODEL_EXTENT / 4.0).abs() < 1e-6);
    assert!(
        (building_model_scale_for_cell(BUILDING_PREVIEW_CELL_RADIUS) - BUILDING_PREVIEW_SCALE)
            .abs()
            < 1e-6
    );
    assert!(
        (NORMALIZED_MODEL_EXTENT / BUILDING_PREVIEW_CELL_GUIDE_RADIUS
            - building_model_scale_for_cell(BUILDING_PREVIEW_CELL_RADIUS)
                * NORMALIZED_MODEL_EXTENT
                / BUILDING_PREVIEW_CELL_RADIUS)
            .abs()
            < 1e-6
    );
    assert!(
        (building_model_scale_for_cell(2.0) / building_model_scale_for_cell(1.0) - 2.0).abs()
            < 1e-6
    );
}

#[test]
fn catalog_has_unique_ids_and_paths() {
    for (index, definition) in OBJECT_DEFINITIONS.iter().enumerate() {
        assert!(
            !OBJECT_DEFINITIONS[index + 1..]
                .iter()
                .any(|later| later.id == definition.id || later.path == definition.path),
            "duplicate object id or asset path for {}",
            definition.label
        );
    }
}

#[test]
fn machine_types_have_catalog_assets() {
    let kinds = [
        MachineBlockType::ConveyorBelt,
        MachineBlockType::MechanicArm,
        MachineBlockType::Chest,
        MachineBlockType::Assembler,
        MachineBlockType::Furnace,
        MachineBlockType::Pallet,
        MachineBlockType::Crate,
    ];
    for kind in kinds {
        assert!(
            OBJECT_DEFINITIONS
                .iter()
                .any(|definition| definition.id == machine_object_id(kind))
        );
    }
}

#[test]
fn every_catalog_asset_loads_from_glb() {
    let catalog = ObjectCatalog::load();
    for entry in catalog.entries() {
        assert!(
            entry.mesh.is_some(),
            "{} failed to load: {:?}",
            entry.definition.path,
            entry.error
        );
    }
}

#[test]
fn building_models_have_detailed_multicolor_geometry() {
    let catalog = ObjectCatalog::load();
    let building_ids = [
        ObjectId::PlanetBase,
        ObjectId::PlanetBuilder,
        ObjectId::PlanetHousing,
        ObjectId::PlanetFactory,
        ObjectId::PlanetMine,
        ObjectId::PlanetWarehouse,
        ObjectId::PlanetRefuel,
        ObjectId::PlanetLogistics,
    ];
    for id in building_ids {
        let mesh = catalog
            .get(id)
            .expect("building asset should load before checking detail");
        let color_count = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.color)
            .collect::<HashSet<_>>()
            .len();
        assert!(
            mesh.face_count() >= 100,
            "{id:?} only has {} faces",
            mesh.face_count()
        );
        assert!(color_count >= 3, "{id:?} only has {color_count} colors");
    }
}

#[test]
fn catalog_assets_retain_vertex_colors() {
    let catalog = ObjectCatalog::load();
    for entry in catalog.entries() {
        let mesh = entry
            .mesh
            .as_ref()
            .expect("catalog asset should load before checking colors");
        assert!(
            mesh.vertices
                .iter()
                .any(|vertex| vertex.color != [255, 255, 255, 255]),
            "{} has no non-white vertex color: {:?}",
            entry.definition.path,
            mesh.vertices
                .iter()
                .take(4)
                .map(|vertex| vertex.color)
                .collect::<Vec<_>>()
        );
        assert_eq!(mesh.indices.len() % 3, 0);
        assert!(
            mesh.indices
                .iter()
                .enumerate()
                .any(|(position, &index)| position as u32 != index),
            "{} did not retain its GLB triangle indices",
            entry.definition.path
        );
    }
}

#[test]
fn catalog_assets_preserve_triangle_indices() {
    let catalog = ObjectCatalog::load();
    let mesh = catalog
        .get(ObjectId::PlanetBase)
        .expect("base model should load");
    assert!(
        mesh.indices
            .iter()
            .enumerate()
            .any(|(position, &index)| position as u32 != index),
        "base model indices should come from its GLB accessor"
    );
}

#[test]
fn conveyor_neighbor_states_load_from_subfolder() {
    let catalog = ObjectCatalog::load();
    let conveyor_states = &catalog
        .entries()
        .iter()
        .find(|entry| entry.definition.id == ObjectId::ConveyorBelt)
        .expect("conveyor entry should exist")
        .states;
    assert_eq!(conveyor_states.len(), 784);
    assert!(
        catalog
            .get_state(ObjectId::ConveyorBelt, "f0-b0-s00")
            .is_some()
    );
    assert!(
        catalog
            .get_state(ObjectId::ConveyorBelt, "f1-b0-s00")
            .is_some()
    );
    for state in &catalog
        .entries()
        .iter()
        .find(|entry| entry.definition.id == ObjectId::ConveyorBelt)
        .expect("conveyor state object should exist")
        .states
    {
        let mesh = state
            .mesh
            .as_ref()
            .unwrap_or_else(|| panic!("{} failed to load: {:?}", state.key, state.error));
        assert!(
            mesh.vertices
                .iter()
                .any(|vertex| vertex.color != [255, 255, 255, 255]),
            "{} lost its vertex colors",
            state.key
        );
        for triangle in mesh.indices.chunks_exact(3) {
            let a = mesh.vertices[triangle[0] as usize].position;
            let b = mesh.vertices[triangle[1] as usize].position;
            let c = mesh.vertices[triangle[2] as usize].position;
            assert!(
                (b - a).cross(c - a).length_squared() > 1e-10,
                "{} contains a degenerate triangle",
                state.key
            );
        }
    }
}

#[test]
fn removing_faces_compacts_vertices_and_remaps_face_metadata() {
    let vertex = |position| Vertex::new2(position, Vec2::ZERO, Color::from_rgba(80, 120, 160, 255));
    let mut mesh = EditableMesh {
        vertices: vec![
            vertex(Vec3::ZERO),
            vertex(Vec3::X),
            vertex(Vec3::Y),
            vertex(Vec3::Z),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
    };
    let mut metadata = FaceMetadata::for_face_count(2);
    metadata.face_names[0] = Some("front".to_string());
    metadata.face_names[1] = Some("side".to_string());
    let group = metadata.create_group(&[0, 1]).unwrap();
    metadata.radial_pivot = Some([2.0, 3.0, 4.0]);

    let remap = mesh.remove_faces(&[0]).unwrap();
    metadata.remap_after_face_removal(&remap);

    assert_eq!(remap, vec![None, Some(0)]);
    assert_eq!(metadata.radial_pivot, Some([2.0, 3.0, 4.0]));
    assert_eq!(mesh.vertices.len(), 3);
    assert_eq!(mesh.indices, vec![0, 1, 2]);
    assert_eq!(metadata.face_names, vec![Some("side".to_string())]);
    assert_eq!(metadata.group(group).unwrap().faces, vec![0]);
}

#[test]
fn trim_splits_intersected_target_face_and_keeps_both_sides() {
    let vertex =
        |position| Vertex::new2(position, Vec2::ZERO, Color::from_rgba(255, 255, 255, 255));
    let mut mesh = EditableMesh {
        vertices: vec![
            vertex(vec3(0.0, 0.0, 0.0)),
            vertex(vec3(2.0, 0.0, 0.0)),
            vertex(vec3(0.0, 2.0, 0.0)),
            vertex(vec3(0.5, -1.0, -1.0)),
            vertex(vec3(0.5, 3.0, -1.0)),
            vertex(vec3(0.5, 1.0, 1.0)),
        ],
        indices: vec![0, 1, 2, 3, 4, 5],
    };
    let face_map = mesh.split_faces_by_intersections(&[0], &[1], None).unwrap();

    assert_eq!(mesh.face_count(), 4);
    assert_eq!(face_map[0].len(), 3);
    assert_eq!(face_map[1].len(), 1);
    let centroids = face_map[0]
        .iter()
        .map(|&face| {
            let start = face * 3;
            (0..3)
                .map(|slot| {
                    mesh.vertices[mesh.indices[start + slot] as usize]
                        .position
                        .x
                })
                .sum::<f32>()
                / 3.0
        })
        .collect::<Vec<_>>();
    assert!(centroids.iter().any(|&x| x < 0.5));
    assert!(centroids.iter().any(|&x| x > 0.5));
}

#[test]
fn trim_rejects_non_intersecting_cutter_faces_without_mutating_mesh() {
    let vertex =
        |position| Vertex::new2(position, Vec2::ZERO, Color::from_rgba(255, 255, 255, 255));
    let mut mesh = EditableMesh {
        vertices: vec![
            vertex(vec3(0.0, 0.0, 0.0)),
            vertex(vec3(2.0, 0.0, 0.0)),
            vertex(vec3(0.0, 2.0, 0.0)),
            vertex(vec3(5.5, -1.0, -1.0)),
            vertex(vec3(5.5, 3.0, -1.0)),
            vertex(vec3(5.5, 1.0, 1.0)),
        ],
        indices: vec![0, 1, 2, 3, 4, 5],
    };
    let original = mesh.clone();

    assert!(mesh.split_faces_by_intersections(&[0], &[1], None).is_err());
    assert_eq!(mesh.indices, original.indices);
    assert_eq!(
        mesh.vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>(),
        original
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>()
    );
}

#[test]
fn trim_multiline_splits_only_the_selected_target_face() {
    let vertex =
        |position| Vertex::new2(position, Vec2::ZERO, Color::from_rgba(255, 255, 255, 255));
    let mut mesh = EditableMesh {
        vertices: vec![
            vertex(vec3(0.0, 0.0, 0.0)),
            vertex(vec3(2.0, 0.0, 0.0)),
            vertex(vec3(0.0, 2.0, 0.0)),
            vertex(vec3(0.0, 0.0, 1.0)),
            vertex(vec3(2.0, 0.0, 1.0)),
            vertex(vec3(0.0, 2.0, 1.0)),
        ],
        indices: vec![0, 1, 2, 3, 4, 5],
    };
    let path = vec![
        vec3(0.0, 1.0, 0.0),
        vec3(0.4, 0.4, 0.0),
        vec3(1.0, 0.0, 0.0),
    ];
    let face_map = mesh
        .split_faces_by_intersections(&[0], &[], Some((0, &path)))
        .unwrap();

    assert_eq!(face_map[0].len(), 5);
    assert_eq!(face_map[1], vec![5]);
    let target_area = face_map[0]
        .iter()
        .map(|&face| {
            let start = face * 3;
            let a = mesh.vertices[mesh.indices[start] as usize].position;
            let b = mesh.vertices[mesh.indices[start + 1] as usize].position;
            let c = mesh.vertices[mesh.indices[start + 2] as usize].position;
            (b - a).cross(c - a).length() * 0.5
        })
        .sum::<f32>();
    assert!((target_area - 2.0).abs() < 1e-4);
}

#[test]
fn reflected_and_offset_face_copies_keep_source_geometry_and_report_face_mapping() {
    let mut mesh = EditableMesh::regular_polygon(3).unwrap();
    let original_indices = mesh.indices.clone();
    let original_positions = mesh
        .vertices
        .iter()
        .map(|vertex| vertex.position)
        .collect::<Vec<_>>();
    let reflected = mesh
        .copy_faces_reflected(&[0], Vec3::ZERO, Vec3::X)
        .unwrap();
    assert_eq!(mesh.face_count(), 4);
    assert_eq!(&mesh.indices[..original_indices.len()], original_indices);
    assert_eq!(reflected, vec![(0, 3)]);
    let source = &original_indices[..3];
    let mirrored = &mesh.indices[9..12];
    for (mirrored_index, source_index) in mirrored.iter().zip([source[0], source[2], source[1]]) {
        assert_eq!(
            mesh.vertices[*mirrored_index as usize].position.x,
            -original_positions[source_index as usize].x
        );
    }

    let before_offset = mesh.face_count();
    let offset = mesh.copy_faces_offset(&[1], 0.5).unwrap();
    assert_eq!(mesh.face_count(), before_offset + 1);
    assert_eq!(offset, vec![(1, before_offset)]);
    for &index in &mesh.indices[before_offset * 3..before_offset * 3 + 3] {
        assert!((mesh.vertices[index as usize].position.y - 0.5).abs() < 1e-5);
    }
}

#[test]
fn fillet_replaces_a_manifold_crease_with_a_rounded_triangle_strip() {
    let mut mesh = EditableMesh::primitive(MeshPrimitive::Cube);
    let original_face_count = mesh.face_count();
    let face_map = mesh.fillet_edges(&[(0, 1)], 0.1, 4).unwrap();

    assert_eq!(mesh.face_count(), original_face_count + 8);
    assert_eq!(face_map.len(), original_face_count);
    assert_eq!(face_map[0].len() + face_map[4].len(), 10);
    assert!(!mesh.indices.chunks_exact(3).any(|triangle| {
        (0..3).any(|side| {
            let a = triangle[side];
            let b = triangle[(side + 1) % 3];
            (a == 0 && b == 1) || (a == 1 && b == 0)
        })
    }));
}

#[test]
fn fillet_rejects_non_manifold_or_oversized_edges_without_mutation() {
    let mut mesh = EditableMesh::primitive(MeshPrimitive::Cube);
    let original = mesh.indices.clone();

    assert!(mesh.fillet_edges(&[(0, 1)], 10.0, 4).is_err());
    assert_eq!(mesh.indices, original);
}

#[test]
fn extracting_and_composing_face_groups_compacts_and_offsets_indices() {
    let white = Color::from_rgba(255, 255, 255, 255);
    let vertex = |position| Vertex::new2(position, Vec2::ZERO, white);
    let mesh = EditableMesh {
        vertices: vec![
            vertex(Vec3::ZERO),
            vertex(Vec3::X),
            vertex(Vec3::Y),
            vertex(Vec3::Z),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
    };
    let mut composed = mesh.extract_faces(&[0]).unwrap();
    let second = mesh.extract_faces(&[1]).unwrap();
    composed.append(&second).unwrap();

    assert_eq!(composed.vertices.len(), 6);
    assert_eq!(composed.indices, vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn reflected_belt_state_swaps_sides_and_preserves_winding() {
    let key = "f1-b2-s01";
    let reflected_key = reflected_belt_state_key(key).unwrap();
    assert_eq!(reflected_key, "f6-b5-s08");
    assert_eq!(reflected_belt_state_key(&reflected_key).unwrap(), key);

    let white = Color::from_rgba(255, 255, 255, 255);
    let mesh = EditableMesh {
        vertices: vec![
            Vertex::new2(Vec3::ZERO, Vec2::ZERO, white),
            Vertex::new2(vec3(1.0, 0.0, 2.0), Vec2::ZERO, white),
            Vertex::new2(vec3(0.0, 1.0, -1.0), Vec2::ZERO, white),
        ],
        indices: vec![0, 1, 2],
    };
    let reflected = mesh.reflected_longitudinal().unwrap();
    assert_eq!(
        reflected.vertices[1].position.z,
        -mesh.vertices[1].position.z
    );
    assert_eq!(reflected.indices, vec![0, 2, 1]);
    assert_eq!(reflected_belt_state_key("f0-b0-s00").unwrap(), "f0-b0-s00");
}

#[test]
fn variant_configuration_round_trips_and_checks_radial_slots() {
    let directory =
        std::env::temp_dir().join(format!("gridgame-variant-rules-{}", std::process::id()));
    let mut config = VariantConfig {
        mode: VariantMode::Radial,
        ..VariantConfig::default()
    };
    for sides in [5, 6] {
        for sector in 0..sides {
            config.radial_sector_groups.push(RadialSectorGroup {
                sides,
                sector,
                face_group_id: sector as u64 + 1,
            });
        }
    }
    config.validate().unwrap();
    save_variant_config(&directory, &config).unwrap();
    let (loaded, error) = load_variant_config(&directory);

    assert!(error.is_none());
    assert_eq!(loaded, config);
    fs::remove_file(variant_config_path(&directory)).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn placement_profile_round_trips_in_variant_configuration() {
    let directory =
        std::env::temp_dir().join(format!("gridgame-placement-profile-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let config = VariantConfig {
        placement_profile: Some(PlacementProfile::NetworkSegments),
        ..VariantConfig::default()
    };
    save_variant_config(&directory, &config).unwrap();
    let (loaded, error) = load_variant_config(&directory);
    assert!(error.is_none());
    assert_eq!(
        loaded.placement_profile,
        Some(PlacementProfile::NetworkSegments)
    );
    fs::remove_file(variant_config_path(&directory)).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn planet_buildings_use_triangle_slots() {
    let mut catalog = ObjectCatalog::load();
    assert_eq!(
        catalog.placement_profile(ObjectId::PlanetBase),
        Some(PlacementProfile::TriangleSlots)
    );
    assert!(
        catalog
            .entries()
            .iter()
            .all(|entry| entry.definition.label != "Route")
    );
    assert!(
        catalog
            .set_placement_profile(ObjectId::PlanetBase, PlacementProfile::CellCenter)
            .is_err()
    );
}

#[test]
fn custom_variants_require_exact_condition_key_match() {
    let mut catalog = ObjectCatalog::load();
    let entry = catalog
        .entries
        .iter_mut()
        .find(|entry| entry.definition.id == ObjectId::PlanetBase)
        .unwrap();
    entry.variant_config.mode = VariantMode::Custom;
    entry.variant_config.custom_patterns = vec![CustomVariantPattern {
        name: "Straight route".to_string(),
        condition_key: "p6-002200".to_string(),
        state_key: "custom-straight-route".to_string(),
    }];

    assert_eq!(
        catalog.custom_state_key(ObjectId::PlanetBase, "p6-002200"),
        Some("custom-straight-route")
    );
    assert_eq!(
        catalog.custom_state_key(ObjectId::PlanetBase, "p6-002201"),
        None
    );
}

#[test]
fn face_groups_move_members_and_can_be_renamed() {
    let mut metadata = FaceMetadata::for_face_count(4);
    let first = metadata.create_group(&[0, 1]).unwrap();
    let second = metadata.create_group(&[2]).unwrap();
    metadata.assign_faces_to_group(&[1, 2], second).unwrap();
    metadata.rename_group(second, "Trim".to_string()).unwrap();

    assert_eq!(metadata.group(first).unwrap().faces, vec![0]);
    assert_eq!(metadata.group(second).unwrap().name, "Trim");
    assert_eq!(metadata.group(second).unwrap().faces, vec![1, 2]);
}

#[test]
fn legacy_face_groups_load_as_unparented_geometry_groups() {
    let group: FaceGroup =
        serde_json::from_str(r#"{"id":7,"name":"Existing","faces":[0]}"#).unwrap();
    let metadata: FaceMetadata =
        serde_json::from_str(r#"{"face_names":[],"groups":[],"next_group_id":1}"#).unwrap();

    assert_eq!(group.parent_id, None);
    assert_eq!(group.program, FaceGroupProgram::Geometry);
    assert!(metadata.groups.is_empty());
}

#[test]
fn radial_program_repeats_main_geometry_and_adds_matching_condition_geometry() {
    let vertex = |x| {
        Vertex::new2(
            vec3(x, 0.0, 0.0),
            Vec2::ZERO,
            Color::from_rgba(80, 120, 160, 255),
        )
    };
    let mesh = EditableMesh {
        vertices: (0..12).map(|index| vertex(index as f32)).collect(),
        indices: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    };
    let mut metadata = FaceMetadata::for_face_count(4);
    let main_id = metadata.create_group(&[0]).unwrap();
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
    let other_group = metadata.create_group_under(&[2], Some(main_id)).unwrap();
    metadata
        .set_group_program(
            other_group,
            FaceGroupProgram::IfNeighbor(RadialNeighborState::OtherBuilding),
        )
        .unwrap();
    metadata.validate(4).unwrap();

    assert_eq!(metadata.faces_for_group(main_id), Some(vec![0]));
    assert_eq!(metadata.faces_for_group(route_group), Some(vec![1]));
    assert_eq!(
        metadata
            .radial_sector_mesh(&mesh, RadialNeighborState::Empty)
            .unwrap()
            .unwrap()
            .face_count(),
        1
    );
    assert_eq!(
        metadata
            .radial_sector_mesh(&mesh, RadialNeighborState::Route)
            .unwrap()
            .unwrap()
            .face_count(),
        2
    );
    assert_eq!(
        metadata
            .radial_sector_mesh(&mesh, RadialNeighborState::OtherBuilding)
            .unwrap()
            .unwrap()
            .face_count(),
        2
    );
    assert!(
        metadata
            .radial_sector_mesh(&mesh, RadialNeighborState::Empty)
            .unwrap()
            .is_some()
    );
    let mut without_route = metadata.clone();
    assert_eq!(without_route.remove_group(route_group).unwrap(), vec![1]);
    assert!(without_route.group(route_group).is_none());
    assert_eq!(without_route.radial_main_group_id(), Some(main_id));

    let mut without_radial = metadata.clone();
    without_radial.radial_pivot = Some([1.0, 2.0, 3.0]);
    assert_eq!(without_radial.remove_group(main_id).unwrap(), vec![0, 1, 2]);
    assert!(without_radial.groups.is_empty());
    assert_eq!(without_radial.radial_main_group_id(), None);
    assert_eq!(without_radial.radial_pivot, None);
}

#[test]
fn model_program_groups_can_be_renamed() {
    let mut metadata = FaceMetadata::for_face_count(2);
    let radial = metadata.create_group(&[0]).unwrap();
    metadata
        .set_group_program(radial, FaceGroupProgram::RadialRepeat)
        .unwrap();
    metadata
        .rename_group(radial, "Main sector".to_string())
        .unwrap();

    assert_eq!(metadata.group(radial).unwrap().name, "Main sector");
    assert_eq!(
        metadata.group(radial).unwrap().program,
        FaceGroupProgram::RadialRepeat
    );
}

#[test]
fn group_lock_and_hide_apply_to_descendants_and_serialize() {
    let mut metadata = FaceMetadata::for_face_count(2);
    let parent = metadata.create_group(&[0]).unwrap();
    let child = metadata.create_group_under(&[1], Some(parent)).unwrap();
    metadata.set_group_locked(parent, true).unwrap();
    metadata.set_group_hidden(parent, true).unwrap();

    assert!(metadata.group_is_locked(child));
    assert!(metadata.group_is_hidden(child));
    assert_eq!(metadata.locked_faces(), HashSet::from([0, 1]));
    assert_eq!(metadata.hidden_faces(), HashSet::from([0, 1]));
    assert!(metadata.group_has_locked_ancestor(child));
    assert!(metadata.group_has_hidden_ancestor(child));

    let json = serde_json::to_string(&metadata).unwrap();
    let loaded: FaceMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.locked_faces(), HashSet::from([0, 1]));
    assert_eq!(loaded.hidden_faces(), HashSet::from([0, 1]));
}

#[test]
fn group_programs_are_configured_incrementally_and_reject_duplicate_conditions() {
    let mut metadata = FaceMetadata::for_face_count(3);
    let main = metadata.create_group(&[0]).unwrap();
    let condition = metadata.create_group_under(&[1], Some(main)).unwrap();

    assert_eq!(
        metadata.group(condition).unwrap().program,
        FaceGroupProgram::Geometry
    );
    assert!(
        metadata
            .set_group_program(
                condition,
                FaceGroupProgram::IfNeighbor(RadialNeighborState::Empty)
            )
            .is_err()
    );
    metadata
        .set_group_program(main, FaceGroupProgram::RadialRepeat)
        .unwrap();
    metadata
        .set_group_program(
            condition,
            FaceGroupProgram::IfNeighbor(RadialNeighborState::Empty),
        )
        .unwrap();
    let duplicate = metadata.create_group_under(&[2], Some(main)).unwrap();
    assert!(
        metadata
            .set_group_program(
                duplicate,
                FaceGroupProgram::IfNeighbor(RadialNeighborState::Empty)
            )
            .is_err()
    );
    assert_eq!(
        metadata.group(duplicate).unwrap().program,
        FaceGroupProgram::Geometry
    );
}

#[test]
fn primitive_meshes_have_valid_non_degenerate_triangles() {
    for (primitive, expected_faces) in [
        (MeshPrimitive::Cube, 12),
        (MeshPrimitive::Polygon, 6),
        (MeshPrimitive::Sphere, 224),
        (MeshPrimitive::Cylinder, 64),
    ] {
        let mesh = EditableMesh::primitive(primitive);
        assert_eq!(mesh.face_count(), expected_faces);
        assert!(
            mesh.indices
                .iter()
                .all(|&index| (index as usize) < mesh.vertices.len())
        );
        for triangle in mesh.indices.chunks_exact(3) {
            let a = mesh.vertices[triangle[0] as usize].position;
            let b = mesh.vertices[triangle[1] as usize].position;
            let c = mesh.vertices[triangle[2] as usize].position;
            assert!((b - a).cross(c - a).length_squared() > 1e-10);
        }
    }
}

#[test]
fn regular_polygon_validates_side_count_and_generates_one_triangle_per_side() {
    for sides in [3, 5, 6, 16] {
        let mesh = EditableMesh::regular_polygon(sides).unwrap();
        assert_eq!(mesh.face_count(), sides);
        assert_eq!(mesh.vertices.len(), sides + 1);
    }
    assert!(EditableMesh::regular_polygon(2).is_err());
    assert!(EditableMesh::regular_polygon(17).is_err());
}

#[test]
fn extrusion_creates_group_for_source_and_new_faces_without_names() {
    let mut metadata = FaceMetadata::for_face_count(2);
    metadata.rename_face(0, "source".to_string()).unwrap();
    metadata.create_group(&[0]).unwrap();

    let group = metadata.inherit_group_for_extrusion(0, 2, 7).unwrap();

    assert_eq!(
        metadata.group(group).unwrap().faces,
        vec![0, 2, 3, 4, 5, 6, 7, 8]
    );
    assert_eq!(metadata.face_names.len(), 9);
    assert!(metadata.face_names[2..].iter().all(Option::is_none));
}

#[test]
fn legacy_loose_lines_are_ignored_and_omitted_when_sidecar_is_saved() {
    let directory =
        std::env::temp_dir().join(format!("gridgame-cad-metadata-{}", std::process::id()));
    let mesh_path = directory.join("model.glb");
    let legacy_path = face_metadata_path(&mesh_path);
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        &legacy_path,
        r#"{"face_names":[null],"groups":[],"loose_lines":[{"start":[0,0,0],"end":[1,0,0]}]}"#,
    )
    .unwrap();
    let (loaded, error) = load_face_metadata(&mesh_path, 3);

    assert!(error.is_none());
    assert_eq!(loaded.face_names, vec![None; 3]);
    assert_eq!(loaded.groups, Vec::new());
    save_face_metadata(&mesh_path, &loaded).unwrap();
    let saved = fs::read_to_string(&legacy_path).unwrap();
    assert!(!saved.contains("loose_lines"));
    fs::remove_file(legacy_path).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn painting_vertices_changes_color_without_changing_topology() {
    let white = Color::from_rgba(255, 255, 255, 255);
    let mut mesh = EditableMesh {
        vertices: vec![
            Vertex::new2(Vec3::ZERO, Vec2::ZERO, white),
            Vertex::new2(Vec3::X, Vec2::ZERO, white),
            Vertex::new2(Vec3::Y, Vec2::ZERO, white),
        ],
        indices: vec![0, 1, 2],
    };
    let original_indices = mesh.indices.clone();
    mesh.paint_vertices(&[0, 2], Color::from_rgba(12, 34, 56, 78))
        .unwrap();

    assert_eq!(mesh.vertices[0].color, [12, 34, 56, 78]);
    assert_eq!(mesh.vertices[1].color, [255, 255, 255, 255]);
    assert_eq!(mesh.vertices[2].color, [12, 34, 56, 78]);
    assert_eq!(mesh.indices, original_indices);
}

#[test]
fn painting_a_face_does_not_recolor_an_adjacent_face() {
    let white = Color::from_rgba(255, 255, 255, 255);
    let red = Color::from_rgba(220, 70, 70, 255);
    let mut mesh = EditableMesh {
        vertices: vec![
            Vertex::new2(Vec3::ZERO, Vec2::ZERO, white),
            Vertex::new2(Vec3::X, Vec2::ZERO, white),
            Vertex::new2(Vec3::Y, Vec2::ZERO, white),
            Vertex::new2(Vec3::Z, Vec2::ZERO, white),
        ],
        indices: vec![0, 1, 2, 1, 3, 2],
    };
    mesh.paint_faces(&[0], red).unwrap();

    let first = &mesh.indices[..3];
    let adjacent = &mesh.indices[3..];
    assert!(
        first
            .iter()
            .all(|&index| mesh.vertices[index as usize].color == [220, 70, 70, 255])
    );
    assert!(
        adjacent
            .iter()
            .all(|&index| mesh.vertices[index as usize].color == [255, 255, 255, 255])
    );
    assert_eq!(mesh.face_count(), 2);
}

#[test]
fn glb_round_trip_preserves_triangle_positions_and_colors() {
    let mesh = EditableMesh {
        vertices: vec![
            Vertex::new2(
                vec3(-1.0, 0.0, 0.0),
                Vec2::ZERO,
                Color::from_rgba(12, 34, 56, 255),
            ),
            Vertex::new2(
                vec3(1.0, 0.0, 0.0),
                Vec2::ZERO,
                Color::from_rgba(12, 34, 56, 255),
            ),
            Vertex::new2(
                vec3(0.0, 1.0, 0.0),
                Vec2::ZERO,
                Color::from_rgba(12, 34, 56, 255),
            ),
        ],
        indices: vec![0, 1, 2],
    };
    let path = std::env::temp_dir().join(format!(
        "gridgame-object-roundtrip-{}.glb",
        std::process::id()
    ));
    save_glb(&path, &mesh).expect("write test GLB");
    let loaded = load_glb(&path).expect("read test GLB");
    let _ = fs::remove_file(path);
    assert_eq!(loaded.indices, mesh.indices);
    assert_eq!(loaded.vertices.len(), mesh.vertices.len());
    for (actual, expected) in loaded.vertices.iter().zip(mesh.vertices.iter()) {
        assert_eq!(actual.position, expected.position);
        assert_eq!(actual.color, expected.color);
    }
}

#[test]
fn extruding_face_adds_top_and_side_triangles() {
    let mut mesh = EditableMesh {
        vertices: vec![
            Vertex::new2(Vec3::ZERO, Vec2::ZERO, Color::from_rgba(255, 255, 255, 255)),
            Vertex::new2(Vec3::X, Vec2::ZERO, Color::from_rgba(255, 255, 255, 255)),
            Vertex::new2(Vec3::Y, Vec2::ZERO, Color::from_rgba(255, 255, 255, 255)),
        ],
        indices: vec![0, 1, 2],
    };
    mesh.extrude_face(0, 1.0).expect("extrude valid face");
    assert_eq!(mesh.vertices.len(), 6);
    assert_eq!(mesh.face_count(), 8);
}
