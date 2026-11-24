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
    builder_tender_kits: u32,
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
    fn has_builder_tender_kit(&self) -> bool {
        self.builder_tender_kits > 0
    }
    fn consume_builder_tender_kit(&mut self) -> bool {
        if self.builder_tender_kits == 0 {
            return false;
        }
        self.builder_tender_kits -= 1;
        true
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
        route_edges: HashSet::new(),
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
        builder_tender_kits: 0,
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
fn route_path_respects_explicit_network_edges() {
    let buildings: HashMap<u32, TestBuilding> = HashMap::new();
    let neighbors = vec![vec![1], vec![0, 2], vec![1, 3], vec![2]];
    let mut ops = test_ops();
    ops.route_edges.extend([(0, 1), (1, 2)]);
    assert_eq!(
        find_route_path_cells(0, 2, &buildings, &neighbors, &ops),
        Some(vec![0, 1, 2])
    );
    assert_eq!(
        find_route_path_cells(0, 3, &buildings, &neighbors, &ops),
        None
    );
}

#[test]
fn automatic_base_builder_requires_and_consumes_a_completed_kit() {
    let mut buildings = HashMap::new();
    buildings.insert(0, make_building(TestKind::Base));
    let mut supply = make_building(TestKind::Builder);
    supply.storage.insert(PlanetResourceKind::Stone, 10);
    buildings.insert(1, supply);
    let mut target = make_building(TestKind::Housing);
    target.build_paid = false;
    target.build_progress = 0.0;
    buildings.insert(2, target);
    let mut second_target = make_building(TestKind::Housing);
    second_target.build_paid = false;
    second_target.build_progress = 0.0;
    buildings.insert(3, second_target);
    let neighbors = vec![vec![1, 2, 3], vec![0, 2, 3], vec![0, 1, 3], vec![0, 1, 2]];
    let grid = HexGrid {
        freq: 1,
        vertices: vec![],
        faces: vec![],
        face_centers: vec![],
        vertex_faces: vec![],
        base_face_buckets: vec![],
        base_face_neighbors: vec![],
    };
    let mut units = Vec::new();

    update_units_and_construction(
        &mut units,
        &mut buildings,
        &grid,
        &neighbors,
        0.0,
        &test_ops(),
    );
    assert!(units.is_empty());
    assert_eq!(buildings[&0].builder_tender_kits, 0);

    buildings.get_mut(&0).unwrap().builder_tender_kits = 1;
    update_units_and_construction(
        &mut units,
        &mut buildings,
        &grid,
        &neighbors,
        0.0,
        &test_ops(),
    );
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].preset_id, PlanetUnitPresetId::BuilderSupply);
    assert_eq!(buildings[&0].builder_tender_kits, 0);

    buildings.get_mut(&2).unwrap().build_paid = true;
    update_units_and_construction(
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
fn builder_assignment_selection_is_stable() {
    let mut buildings: HashMap<u32, TestBuilding> = HashMap::new();
    let mut builder = make_building(TestKind::Builder);
    builder.storage.insert(PlanetResourceKind::Stone, 10);
    buildings.insert(1, builder);

    let mut target_a = make_building(TestKind::Housing);
    target_a.build_paid = false;
    target_a.build_progress = 0.0;
    buildings.insert(3, target_a);

    let mut target_b = make_building(TestKind::Housing);
    target_b.build_paid = false;
    target_b.build_progress = 0.0;
    buildings.insert(2, target_b);

    let neighbors = vec![vec![1], vec![0, 2, 3], vec![1], vec![1]];

    let first = next_builder_assignment(1, &buildings, None, &[], &neighbors, &test_ops(), false);
    let second = next_builder_assignment(1, &buildings, None, &[], &neighbors, &test_ops(), false);

    assert_eq!(first, Some((2, 1)));
    assert_eq!(second, first);
}

#[test]
fn builder_keeps_same_assignment_without_resetting_path() {
    let mut buildings: HashMap<u32, TestBuilding> = HashMap::new();
    let mut builder = make_building(TestKind::Builder);
    builder.storage.insert(PlanetResourceKind::Stone, 10);
    buildings.insert(1, builder);

    let mut target = make_building(TestKind::Housing);
    target.build_paid = false;
    target.build_progress = 0.0;
    buildings.insert(2, target);

    let mut unit = unit_for_preset(
        PlanetUnitPresetId::BuilderSupply,
        1,
        PlanetUnitNodeConfig {
            pickup: Some(1),
            dropoff: None,
            refuel: None,
            construction: Some(2),
            wait: None,
        },
    );
    unit.current_action = Some(UnitAction::PickupAtNode(UnitNodeSlot::Pickup));
    unit.current_target = Some(1);
    unit.path = vec![1, 2];
    unit.progress = 0.4;

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

    assert_eq!(units[0].nodes.construction, Some(2));
    assert_eq!(
        units[0].current_action,
        Some(UnitAction::PickupAtNode(UnitNodeSlot::Pickup))
    );
    assert_eq!(units[0].current_target, Some(1));
    assert_eq!(units[0].path, vec![1, 2]);
    assert_eq!(units[0].progress, 0.4);
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
