use super::*;

#[test]
fn default_base_contains_two_forklifts() {
    let interior = default_base_interior();
    assert_eq!(interior.forklifts.len(), 2);
}

#[test]
fn default_base_starts_with_only_loaded_containers() {
    let mut interior = default_base_interior();
    interior.ensure_runtime_state();
    assert!(interior.blocks.iter().all(|record| matches!(
        record.block.kind,
        MachineBlockType::Pallet | MachineBlockType::Crate
    )));
    assert!(
        interior
            .containers
            .iter()
            .all(|container| !container.items.is_empty())
    );
    assert!(interior.mechanic_arms.is_empty());
    assert!(interior.assembly_nodes.is_empty());
}

#[test]
fn assembly_node_produces_builder_tender_kit() {
    let mut node = AssemblyNodeState {
        hex: Axial { q: 0, r: 0 },
        selected_recipe: AssemblerRecipeId::BuilderTender,
        inserted: Vec::new(),
        completed_outputs: Vec::new(),
    };
    for part in assembly_recipe_parts(node.selected_recipe) {
        assert!(assembly_can_accept(&node, *part));
        node.inserted.push(*part);
    }
    push_part(
        &mut node.completed_outputs,
        node.selected_recipe.output_kind(),
    );
    node.inserted.clear();
    assert!(
        node.completed_outputs
            .iter()
            .any(|stack| stack.kind == InteriorPartKind::BuilderTenderKit)
    );
}

#[test]
fn mechanic_arm_picks_output_required_part_instead_of_first_available() {
    let mut interior = BaseInteriorState {
        blocks: vec![
            InteriorBlockRecord {
                hex: Axial { q: 0, r: 0 },
                block: MachineBlock {
                    kind: MachineBlockType::MechanicArm,
                    rotation: 0,
                },
            },
            InteriorBlockRecord {
                hex: Axial { q: -1, r: 0 },
                block: MachineBlock {
                    kind: MachineBlockType::Crate,
                    rotation: 0,
                },
            },
            InteriorBlockRecord {
                hex: Axial { q: 1, r: 0 },
                block: MachineBlock {
                    kind: MachineBlockType::Assembler,
                    rotation: 0,
                },
            },
        ],
        containers: vec![InteriorContainerRecord {
            hex: Axial { q: -1, r: 0 },
            items: vec![
                InteriorPartStack {
                    kind: InteriorPartKind::WheelAssembly,
                    amount: 1,
                },
                InteriorPartStack {
                    kind: InteriorPartKind::ChassisFrame,
                    amount: 1,
                },
            ],
        }],
        belt_items: Vec::new(),
        mechanic_arms: vec![MechanicArmState {
            hex: Axial { q: 0, r: 0 },
            held: None,
            cooldown: 0.0,
            output_hexes: vec![Axial { q: 1, r: 0 }],
            input_mode: MechanicArmInputMode::ExplicitInputs,
            input_hexes: vec![Axial { q: -1, r: 0 }],
            filters: Vec::new(),
            action_stage: MechanicArmActionStage::Idle,
            action_origin: None,
            action_target: None,
            action_progress: 0.0,
        }],
        assembly_nodes: vec![AssemblyNodeState {
            hex: Axial { q: 1, r: 0 },
            selected_recipe: AssemblerRecipeId::BuilderTender,
            inserted: Vec::new(),
            completed_outputs: Vec::new(),
        }],
        forklifts: Vec::new(),
        logistics_requests: Vec::new(),
        camera: default_interior_camera(),
    };

    tick_base_interior(&mut interior, 0.1);
    assert_eq!(
        interior.mechanic_arms[0].held,
        Some(InteriorPartKind::ChassisFrame)
    );
    assert_eq!(
        interior.containers[0]
            .items
            .iter()
            .find(|stack| stack.kind == InteriorPartKind::WheelAssembly)
            .map(|stack| stack.amount),
        Some(1)
    );
}

#[test]
fn mechanic_arm_transfers_directly_from_input_to_output() {
    let source_hex = Axial { q: -1, r: 0 };
    let output_hex = Axial { q: 1, r: 0 };
    let mut interior = BaseInteriorState {
        blocks: vec![
            InteriorBlockRecord {
                hex: Axial { q: 0, r: 0 },
                block: MachineBlock {
                    kind: MachineBlockType::MechanicArm,
                    rotation: 0,
                },
            },
            InteriorBlockRecord {
                hex: source_hex,
                block: MachineBlock {
                    kind: MachineBlockType::Crate,
                    rotation: 0,
                },
            },
            InteriorBlockRecord {
                hex: output_hex,
                block: MachineBlock {
                    kind: MachineBlockType::Assembler,
                    rotation: 0,
                },
            },
        ],
        containers: vec![InteriorContainerRecord {
            hex: source_hex,
            items: Vec::new(),
        }],
        belt_items: Vec::new(),
        mechanic_arms: vec![MechanicArmState {
            hex: Axial { q: 0, r: 0 },
            held: Some(InteriorPartKind::ChassisFrame),
            cooldown: 0.0,
            output_hexes: vec![output_hex],
            input_mode: MechanicArmInputMode::ExplicitInputs,
            input_hexes: vec![source_hex],
            filters: Vec::new(),
            action_stage: MechanicArmActionStage::Pickup,
            action_origin: None,
            action_target: Some(source_hex),
            action_progress: 0.9,
        }],
        assembly_nodes: vec![AssemblyNodeState {
            hex: output_hex,
            selected_recipe: AssemblerRecipeId::BuilderTender,
            inserted: Vec::new(),
            completed_outputs: Vec::new(),
        }],
        forklifts: Vec::new(),
        logistics_requests: Vec::new(),
        camera: default_interior_camera(),
    };

    tick_base_interior(&mut interior, 0.1);

    let arm = &interior.mechanic_arms[0];
    assert_eq!(arm.action_stage, MechanicArmActionStage::Deliver);
    assert_eq!(arm.action_origin, Some(source_hex));
    assert_eq!(arm.action_target, Some(output_hex));
    assert_eq!(arm.held, Some(InteriorPartKind::ChassisFrame));
}

#[test]
fn assembler_blocks_new_parts_while_output_waits() {
    let node = AssemblyNodeState {
        hex: Axial { q: 0, r: 0 },
        selected_recipe: AssemblerRecipeId::BuilderTender,
        inserted: Vec::new(),
        completed_outputs: vec![InteriorPartStack {
            kind: InteriorPartKind::BuilderTenderKit,
            amount: 1,
        }],
    };
    assert!(!assembly_can_accept(&node, InteriorPartKind::ChassisFrame));
}

#[test]
fn mechanic_arm_can_pick_completed_output_from_assembler() {
    let mut interior = BaseInteriorState {
        blocks: vec![
            InteriorBlockRecord {
                hex: Axial { q: 0, r: 0 },
                block: MachineBlock {
                    kind: MachineBlockType::MechanicArm,
                    rotation: 0,
                },
            },
            InteriorBlockRecord {
                hex: Axial { q: -1, r: 0 },
                block: MachineBlock {
                    kind: MachineBlockType::Assembler,
                    rotation: 0,
                },
            },
            InteriorBlockRecord {
                hex: Axial { q: 1, r: 0 },
                block: MachineBlock {
                    kind: MachineBlockType::Crate,
                    rotation: 0,
                },
            },
        ],
        containers: vec![InteriorContainerRecord {
            hex: Axial { q: 1, r: 0 },
            items: Vec::new(),
        }],
        belt_items: Vec::new(),
        mechanic_arms: vec![MechanicArmState {
            hex: Axial { q: 0, r: 0 },
            held: None,
            cooldown: 0.0,
            output_hexes: vec![Axial { q: 1, r: 0 }],
            input_mode: MechanicArmInputMode::ExplicitInputs,
            input_hexes: vec![Axial { q: -1, r: 0 }],
            filters: Vec::new(),
            action_stage: MechanicArmActionStage::Idle,
            action_origin: None,
            action_target: None,
            action_progress: 0.0,
        }],
        assembly_nodes: vec![AssemblyNodeState {
            hex: Axial { q: -1, r: 0 },
            selected_recipe: AssemblerRecipeId::BuilderTender,
            inserted: Vec::new(),
            completed_outputs: vec![InteriorPartStack {
                kind: InteriorPartKind::BuilderTenderKit,
                amount: 1,
            }],
        }],
        forklifts: Vec::new(),
        logistics_requests: Vec::new(),
        camera: default_interior_camera(),
    };

    tick_base_interior(&mut interior, 0.1);
    assert_eq!(
        interior.mechanic_arms[0].held,
        Some(InteriorPartKind::BuilderTenderKit)
    );
    assert!(interior.assembly_nodes[0].completed_outputs.is_empty());
}

#[test]
fn mechanic_arm_retracts_after_delivering_part() {
    let target_hex = Axial { q: 1, r: 0 };
    let mut interior = BaseInteriorState {
        blocks: vec![
            InteriorBlockRecord {
                hex: Axial { q: 0, r: 0 },
                block: MachineBlock {
                    kind: MachineBlockType::MechanicArm,
                    rotation: 0,
                },
            },
            InteriorBlockRecord {
                hex: target_hex,
                block: MachineBlock {
                    kind: MachineBlockType::Crate,
                    rotation: 0,
                },
            },
        ],
        containers: vec![InteriorContainerRecord {
            hex: target_hex,
            items: Vec::new(),
        }],
        belt_items: Vec::new(),
        mechanic_arms: vec![MechanicArmState {
            hex: Axial { q: 0, r: 0 },
            held: Some(InteriorPartKind::ChassisFrame),
            cooldown: 0.0,
            output_hexes: vec![target_hex],
            input_mode: MechanicArmInputMode::AnyNeighbor,
            input_hexes: Vec::new(),
            filters: Vec::new(),
            action_stage: MechanicArmActionStage::Deliver,
            action_origin: None,
            action_target: Some(target_hex),
            action_progress: 0.9,
        }],
        assembly_nodes: Vec::new(),
        forklifts: Vec::new(),
        logistics_requests: Vec::new(),
        camera: default_interior_camera(),
    };

    tick_base_interior(&mut interior, 0.1);
    assert_eq!(
        interior.mechanic_arms[0].action_stage,
        MechanicArmActionStage::Retract
    );
    assert_eq!(interior.mechanic_arms[0].action_target, Some(target_hex));
    assert_eq!(interior.mechanic_arms[0].held, None);
    assert_eq!(
        interior.containers[0].items,
        vec![InteriorPartStack {
            kind: InteriorPartKind::ChassisFrame,
            amount: 1,
        }]
    );

    tick_base_interior(&mut interior, 0.56);
    assert_eq!(
        interior.mechanic_arms[0].action_stage,
        MechanicArmActionStage::Idle
    );
    assert_eq!(interior.mechanic_arms[0].action_target, None);
}

#[test]
fn mechanic_arm_moves_to_next_ready_input_after_delivery() {
    let input_hex = Axial { q: -1, r: 0 };
    let output_hex = Axial { q: 1, r: 0 };
    let mut interior = BaseInteriorState {
        blocks: vec![
            InteriorBlockRecord {
                hex: Axial { q: 0, r: 0 },
                block: MachineBlock {
                    kind: MachineBlockType::MechanicArm,
                    rotation: 0,
                },
            },
            InteriorBlockRecord {
                hex: input_hex,
                block: MachineBlock {
                    kind: MachineBlockType::Crate,
                    rotation: 0,
                },
            },
            InteriorBlockRecord {
                hex: output_hex,
                block: MachineBlock {
                    kind: MachineBlockType::Crate,
                    rotation: 0,
                },
            },
        ],
        containers: vec![
            InteriorContainerRecord {
                hex: input_hex,
                items: vec![InteriorPartStack {
                    kind: InteriorPartKind::WheelAssembly,
                    amount: 1,
                }],
            },
            InteriorContainerRecord {
                hex: output_hex,
                items: Vec::new(),
            },
        ],
        belt_items: Vec::new(),
        mechanic_arms: vec![MechanicArmState {
            hex: Axial { q: 0, r: 0 },
            held: Some(InteriorPartKind::ChassisFrame),
            cooldown: 0.0,
            output_hexes: vec![output_hex],
            input_mode: MechanicArmInputMode::ExplicitInputs,
            input_hexes: vec![input_hex],
            filters: Vec::new(),
            action_stage: MechanicArmActionStage::Deliver,
            action_origin: Some(input_hex),
            action_target: Some(output_hex),
            action_progress: 0.9,
        }],
        assembly_nodes: Vec::new(),
        forklifts: Vec::new(),
        logistics_requests: Vec::new(),
        camera: default_interior_camera(),
    };

    tick_base_interior(&mut interior, 0.1);

    let arm = &interior.mechanic_arms[0];
    assert_eq!(arm.action_stage, MechanicArmActionStage::Pickup);
    assert_eq!(arm.action_origin, Some(output_hex));
    assert_eq!(arm.action_target, Some(input_hex));
    assert_eq!(arm.held, Some(InteriorPartKind::WheelAssembly));
    assert_eq!(interior.containers[0].items, Vec::new());
    assert_eq!(
        interior.containers[1].items,
        vec![InteriorPartStack {
            kind: InteriorPartKind::ChassisFrame,
            amount: 1,
        }]
    );
}

#[test]
fn forklift_request_moves_container_only_after_arrival() {
    let mut interior = default_base_interior();
    crate::core::build_units::request_container_delivery(
        &mut interior,
        Axial { q: 0, r: 0 },
        Axial { q: -1, r: 0 },
        None,
    );

    tick_base_interior(&mut interior, 2.0);
    assert!(interior.block_at(Axial { q: -3, r: 0 }).is_some());
    assert!(interior.block_at(Axial { q: -1, r: 0 }).is_none());
    assert!(
        interior
            .forklifts
            .iter()
            .all(|forklift| forklift.carried_block.is_none())
    );

    tick_base_interior(&mut interior, 2.0);
    assert!(interior.block_at(Axial { q: -3, r: 0 }).is_none());
    assert!(interior.block_at(Axial { q: -1, r: 0 }).is_none());
    assert!(
        interior
            .forklifts
            .iter()
            .any(|forklift| forklift.carried_block.is_some())
    );

    tick_base_interior(&mut interior, 2.0);
    tick_base_interior(&mut interior, 2.0);
    assert!(interior.block_at(Axial { q: -1, r: 0 }).is_some());
    assert!(interior.logistics_requests.is_empty());
}
