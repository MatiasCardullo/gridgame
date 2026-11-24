use super::*;

#[test]
fn belt_joins_trim_straight_ends_and_miter_angled_ends() {
    let direction = vec3(1.0, 0.0, 0.0);
    let straight = belt_end_side_lengths(direction, direction, 26.0, true);
    assert!((straight.0 - 13.0).abs() < 0.001);
    assert!((straight.1 - 13.0).abs() < 0.001);

    let angled_direction = vec3(0.5, 0.0, 0.866_025_4);
    let front = belt_end_side_lengths(direction, angled_direction, 26.0, true);
    let back = belt_end_side_lengths(direction, angled_direction, 26.0, false);
    assert!(front.0 < 13.0 && front.1 > 13.0);
    assert!(back.0 > 13.0 && back.1 < 13.0);
    let wall_front = belt_end_side_lengths_for_width(
        direction,
        angled_direction,
        26.0,
        true,
        BELT_HALF_WIDTH + BELT_WALL_THICKNESS,
    );
    assert!(wall_front.0 < BELT_HALF_LENGTH);
    assert!(wall_front.1 > BELT_HALF_LENGTH);
    let connected_side = vec3(-angled_direction.z, 0.0, angled_direction.x);
    let belt_side = vec3(0.0, 0.0, 1.0);
    for (is_front, lengths) in [(true, front), (false, back)] {
        let axis = if is_front { direction } else { -direction };
        let endpoints = [
            axis * lengths.0 - belt_side * BELT_HALF_WIDTH,
            axis * lengths.1 + belt_side * BELT_HALF_WIDTH,
        ];
        let angled_cut_edge = endpoints[1] - endpoints[0];
        assert!(angled_cut_edge.cross(angled_direction).length() < 0.001);
        for endpoint in endpoints {
            let relative_to_neighbor = endpoint - axis * 26.0;
            let facing_side = (-axis * 26.0).dot(connected_side).signum();
            assert!(
                (relative_to_neighbor.dot(connected_side) - facing_side * BELT_HALF_WIDTH).abs()
                    < 0.001
            );
        }
    }
}

#[test]
fn belt_side_wall_gaps_cover_all_directions_and_rotations() {
    let hex = Axial { q: 0, r: 0 };
    for rotation in 0..6 {
        for side_offset in [1, 2, 4, 5] {
            let neighbor_index = (rotation + side_offset) % 6;
            let neighbor_hex = axial_neighbors(hex)[neighbor_index];
            let expected_side = if side_offset == 1 || side_offset == 2 {
                0
            } else {
                1
            };
            for neighbor_rotation in [neighbor_index, (neighbor_index + 3) % 6] {
                let mut interior = BaseInteriorState::default();
                interior.blocks.clear();
                interior.upsert_block(
                    neighbor_hex,
                    MachineBlock {
                        kind: MachineBlockType::ConveyorBelt,
                        rotation: neighbor_rotation as u8,
                    },
                );

                let gaps = belt_side_wall_gaps(&interior, hex, rotation);
                assert_eq!(gaps[expected_side].len(), 1);
                assert!(gaps[expected_side][0].0 < gaps[expected_side][0].1);
                assert!(
                    gaps[1 - expected_side].is_empty(),
                    "rotation {rotation}, side offset {side_offset}"
                );
            }

            let mut interior = BaseInteriorState::default();
            interior.blocks.clear();
            interior.upsert_block(
                neighbor_hex,
                MachineBlock {
                    kind: MachineBlockType::ConveyorBelt,
                    rotation: ((neighbor_index + 1) % 6) as u8,
                },
            );
            assert_eq!(
                belt_side_wall_gaps(&interior, hex, rotation),
                [Vec::new(), Vec::new()],
                "non-connecting rotation {rotation}, side offset {side_offset}"
            );
        }
    }
}

#[test]
fn conveyor_state_key_tracks_front_back_and_side_connections() {
    let hex = Axial { q: 0, r: 0 };
    let neighbors = axial_neighbors(hex);
    let mut interior = BaseInteriorState::default();
    interior.blocks.clear();
    interior.upsert_block(
        neighbors[0],
        MachineBlock {
            kind: MachineBlockType::ConveyorBelt,
            rotation: 0,
        },
    );
    assert_eq!(conveyor_state_key(&interior, hex, 0), "f1-b0-s00");

    interior.upsert_block(
        neighbors[1],
        MachineBlock {
            kind: MachineBlockType::ConveyorBelt,
            rotation: 1,
        },
    );
    assert_eq!(conveyor_state_key(&interior, hex, 0), "f1-b0-s01");

    interior.upsert_block(
        neighbors[3],
        MachineBlock {
            kind: MachineBlockType::ConveyorBelt,
            rotation: 3,
        },
    );
    assert_eq!(conveyor_state_key(&interior, hex, 0), "f1-b4-s01");
}

#[test]
fn belt_wall_sections_keep_wall_on_both_sides_of_a_merge_gap() {
    assert_eq!(
        belt_wall_sections(-17.0, 17.0, &[(-4.0, 5.0)]),
        vec![(-17.0, -4.0), (5.0, 17.0)]
    );
}

#[test]
fn angled_front_belt_connection_gets_merge_arrow() {
    let hex = Axial { q: 0, r: 0 };
    let front_hex = axial_neighbors(hex)[0];
    let mut interior = BaseInteriorState::default();
    interior.upsert_block(
        front_hex,
        MachineBlock {
            kind: MachineBlockType::ConveyorBelt,
            rotation: 1,
        },
    );

    assert!(belt_has_angled_front_connection(&interior, hex, 0));
    interior.block_at_mut(front_hex).unwrap().rotation = 0;
    assert!(!belt_has_angled_front_connection(&interior, hex, 0));
}
