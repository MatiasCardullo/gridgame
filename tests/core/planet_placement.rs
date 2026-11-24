use super::{BuildingSlot, CellRadius, NetworkAnchor, NetworkPlacement, building_slot_available};
use std::collections::HashSet;

#[test]
fn building_slots_cover_pentagon_and_hexagon_triangles() {
    assert!(
        BuildingSlot {
            cell_index: 12,
            triangle_index: 4
        }
        .is_valid(5)
    );
    assert!(
        !BuildingSlot {
            cell_index: 12,
            triangle_index: 5
        }
        .is_valid(5)
    );
    assert!(
        BuildingSlot {
            cell_index: 18,
            triangle_index: 5
        }
        .is_valid(6)
    );
}

#[test]
fn building_slots_reject_duplicate_and_out_of_range_occupancy() {
    let occupied = HashSet::from([BuildingSlot {
        cell_index: 12,
        triangle_index: 2,
    }]);
    assert!(!building_slot_available(
        BuildingSlot {
            cell_index: 12,
            triangle_index: 2
        },
        6,
        occupied.iter().copied()
    ));
    assert!(building_slot_available(
        BuildingSlot {
            cell_index: 12,
            triangle_index: 3
        },
        6,
        occupied.iter().copied()
    ));
    assert!(!building_slot_available(
        BuildingSlot {
            cell_index: 12,
            triangle_index: 5
        },
        5,
        occupied.iter().copied()
    ));
}

#[test]
fn network_placements_validate_cell_references_and_connectivity() {
    assert!(
        NetworkPlacement::SharedSide {
            first_cell: 2,
            second_cell: 3
        }
        .is_valid(6)
    );
    assert!(
        !NetworkPlacement::SharedSide {
            first_cell: 2,
            second_cell: 2
        }
        .is_valid(6)
    );
    let midpoint = NetworkPlacement::MidpointSegment {
        cell_index: 4,
        first_neighbor: 0,
        second_neighbor: 2,
    };
    assert!(midpoint.is_valid(5));
    assert_eq!(midpoint.connected_cells(), vec![0, 4, 2]);
    assert!(
        !NetworkPlacement::MidpointSegment {
            cell_index: 4,
            first_neighbor: 0,
            second_neighbor: 4,
        }
        .is_valid(5)
    );
    let radial = NetworkPlacement::RadialSpan {
        cell_index: 4,
        neighbor_cell: 0,
        radius: CellRadius::Short,
    };
    assert!(radial.is_valid(5));
    assert_eq!(radial.connected_cells(), vec![4, 0]);
    assert_eq!(
        NetworkPlacement::SharedSide {
            first_cell: 4,
            second_cell: 2
        }
        .canonicalized(),
        NetworkPlacement::SharedSide {
            first_cell: 2,
            second_cell: 4
        }
    );

    let neighbors = vec![vec![1, 2], vec![0], vec![0]];
    assert!(
        NetworkPlacement::SharedSide {
            first_cell: 0,
            second_cell: 1
        }
        .is_valid_for_neighbors(&neighbors)
    );
    assert!(
        NetworkPlacement::MidpointSegment {
            cell_index: 0,
            first_neighbor: 1,
            second_neighbor: 2
        }
        .is_valid_for_neighbors(&neighbors)
    );
    assert!(
        !NetworkPlacement::SharedSide {
            first_cell: 1,
            second_cell: 2
        }
        .is_valid_for_neighbors(&neighbors)
    );
}

#[test]
fn snapped_network_anchors_validate_and_connect_adjacent_cells() {
    let neighbors = vec![
        vec![1, 2, 3, 4],
        vec![0, 3, 4],
        vec![0, 4],
        vec![0, 1, 4],
        vec![0, 1, 2, 3],
    ];
    let mut faces = vec![[0, 0, 0]; 15];
    faces[10] = [0, 1, 3];
    faces[11] = [0, 1, 2];
    faces[12] = [0, 2, 4];
    faces[13] = [1, 3, 4];
    faces[14] = [2, 3, 4];
    let center = NetworkAnchor::CellCenter { cell_index: 0 };
    let corner = NetworkAnchor::CellCorner { face_index: 11 };
    let midpoint = NetworkAnchor::SideMidpoint {
        first_face: 11,
        second_face: 10,
    };
    assert!(center.is_valid_for_topology(&neighbors, &faces));
    assert!(corner.is_valid_for_topology(&neighbors, &faces));
    assert!(midpoint.is_valid_for_topology(&neighbors, &faces));
    assert_eq!(corner.incident_cells(&faces), vec![0, 1, 2]);
    assert_eq!(midpoint.incident_cells(&faces), vec![0, 1]);

    let segment = NetworkPlacement::AnchorSegment {
        start: center,
        end: midpoint,
    };
    assert!(segment.is_valid_for_topology(&neighbors, &faces));
    assert_eq!(
        segment.connected_cell_edges(&neighbors, &faces),
        vec![(0, 1)]
    );
    assert_eq!(
        segment.canonicalized(),
        NetworkPlacement::AnchorSegment {
            start: center,
            end: midpoint.canonicalized(),
        }
    );
    assert!(
        !NetworkAnchor::SideMidpoint {
            first_face: 10,
            second_face: 12
        }
        .is_valid_for_topology(&neighbors, &faces)
    );
}

#[test]
fn network_placement_json_keeps_legacy_and_snapped_forms_readable() {
    let legacy: NetworkPlacement =
        serde_json::from_str(r#"{"shared_side":{"first_cell":3,"second_cell":7}}"#).unwrap();
    assert_eq!(
        legacy,
        NetworkPlacement::SharedSide {
            first_cell: 3,
            second_cell: 7
        }
    );

    let snapped: NetworkPlacement = serde_json::from_str(
            r#"{"anchor_segment":{"start":{"cell_center":{"cell_index":3}},"end":{"side_midpoint":{"first_face":9,"second_face":12}}}}"#,
        )
        .unwrap();
    assert_eq!(
        snapped.canonicalized(),
        NetworkPlacement::AnchorSegment {
            start: NetworkAnchor::CellCenter { cell_index: 3 },
            end: NetworkAnchor::SideMidpoint {
                first_face: 9,
                second_face: 12
            }
        }
        .canonicalized()
    );
}
