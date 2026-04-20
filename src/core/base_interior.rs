use serde::{Deserialize, Serialize};
use crate::core::common::{axial_neighbors, Axial, MachineBlock, MachineBlockType};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InteriorPartKind {
    ChassisFrame,
    WheelAssembly,
    EngineCore,
    ControlModule,
    ForkCarriage,
    MastSegment,
    HydraulicSet,
    SensorPack,
    FastenerBundle,
    BuilderTenderKit,
    CargoHaulerKit,
    SiteShuttleKit,
}

impl InteriorPartKind {
    pub fn label(self) -> &'static str {
        match self {
            InteriorPartKind::ChassisFrame => "Chassis Frame",
            InteriorPartKind::WheelAssembly => "Wheel Assembly",
            InteriorPartKind::EngineCore => "Engine Core",
            InteriorPartKind::ControlModule => "Control Module",
            InteriorPartKind::ForkCarriage => "Fork Carriage",
            InteriorPartKind::MastSegment => "Mast Segment",
            InteriorPartKind::HydraulicSet => "Hydraulic Set",
            InteriorPartKind::SensorPack => "Sensor Pack",
            InteriorPartKind::FastenerBundle => "Fastener Bundle",
            InteriorPartKind::BuilderTenderKit => "Builder Tender Kit",
            InteriorPartKind::CargoHaulerKit => "Cargo Hauler Kit",
            InteriorPartKind::SiteShuttleKit => "Site Shuttle Kit",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteriorPartStack {
    pub kind: InteriorPartKind,
    pub amount: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InteriorBlockRecord {
    pub hex: Axial,
    pub block: MachineBlock,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InteriorContainerRecord {
    pub hex: Axial,
    #[serde(default)]
    pub items: Vec<InteriorPartStack>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InteriorBeltItem {
    pub hex: Axial,
    pub kind: InteriorPartKind,
    #[serde(default)]
    pub progress: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum MechanicArmInputMode {
    #[default]
    AnyNeighbor,
    ExplicitInputs,
}

impl MechanicArmInputMode {
    pub fn label(self) -> &'static str {
        match self {
            MechanicArmInputMode::AnyNeighbor => "Any neighbor",
            MechanicArmInputMode::ExplicitInputs => "Explicit inputs",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MechanicArmActionStage {
    #[default]
    Idle,
    Pickup,
    Deliver,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MechanicArmState {
    pub hex: Axial,
    #[serde(default)]
    pub held: Option<InteriorPartKind>,
    #[serde(default)]
    pub cooldown: f32,
    #[serde(default)]
    pub output_hexes: Vec<Axial>,
    #[serde(default)]
    pub input_mode: MechanicArmInputMode,
    #[serde(default)]
    pub input_hexes: Vec<Axial>,
    #[serde(default)]
    pub filters: Vec<InteriorPartKind>,
    #[serde(default)]
    pub action_stage: MechanicArmActionStage,
    #[serde(default)]
    pub action_target: Option<Axial>,
    #[serde(default)]
    pub action_progress: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InteriorForkliftState {
    pub id: u32,
    pub hex: Axial,
    pub home: Axial,
    #[serde(default)]
    pub target: Option<Axial>,
    #[serde(default)]
    pub carried: Option<InteriorPartKind>,
    #[serde(default)]
    pub move_progress: f32,
    #[serde(default)]
    pub request_target: Option<Axial>,
    #[serde(default)]
    pub source_hex: Option<Axial>,
    #[serde(default)]
    pub carried_block: Option<MachineBlockType>,
    #[serde(default)]
    pub carried_items: Vec<InteriorPartStack>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum AssemblerRecipeId {
    #[default]
    BuilderTender,
    CargoHauler,
    SiteShuttle,
}

impl AssemblerRecipeId {
    pub fn label(self) -> &'static str {
        match self {
            AssemblerRecipeId::BuilderTender => "Builder Tender",
            AssemblerRecipeId::CargoHauler => "Cargo Hauler",
            AssemblerRecipeId::SiteShuttle => "Site Shuttle",
        }
    }

    pub fn output_kind(self) -> InteriorPartKind {
        match self {
            AssemblerRecipeId::BuilderTender => InteriorPartKind::BuilderTenderKit,
            AssemblerRecipeId::CargoHauler => InteriorPartKind::CargoHaulerKit,
            AssemblerRecipeId::SiteShuttle => InteriorPartKind::SiteShuttleKit,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssemblyNodeState {
    pub hex: Axial,
    #[serde(default)]
    pub selected_recipe: AssemblerRecipeId,
    #[serde(default)]
    pub inserted: Vec<InteriorPartKind>,
    #[serde(default)]
    pub completed_outputs: Vec<InteriorPartStack>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InteriorLogisticsRequest {
    pub requester_hex: Axial,
    pub target_hex: Axial,
    #[serde(default)]
    pub required_filter: Option<InteriorPartKind>,
    #[serde(default)]
    pub assigned_forklift: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BaseInteriorState {
    #[serde(default)]
    pub blocks: Vec<InteriorBlockRecord>,
    #[serde(default)]
    pub containers: Vec<InteriorContainerRecord>,
    #[serde(default)]
    pub belt_items: Vec<InteriorBeltItem>,
    #[serde(default)]
    pub mechanic_arms: Vec<MechanicArmState>,
    #[serde(default)]
    pub assembly_nodes: Vec<AssemblyNodeState>,
    #[serde(default)]
    pub forklifts: Vec<InteriorForkliftState>,
    #[serde(default)]
    pub logistics_requests: Vec<InteriorLogisticsRequest>,
}

impl Default for BaseInteriorState {
    fn default() -> Self {
        default_base_interior()
    }
}

impl BaseInteriorState {
    pub fn block_at(&self, hex: Axial) -> Option<&MachineBlock> {
        self.blocks
            .iter()
            .find(|record| record.hex == hex)
            .map(|record| &record.block)
    }

    pub fn block_at_mut(&mut self, hex: Axial) -> Option<&mut MachineBlock> {
        self.blocks
            .iter_mut()
            .find(|record| record.hex == hex)
            .map(|record| &mut record.block)
    }

    pub fn upsert_block(&mut self, hex: Axial, block: MachineBlock) {
        if let Some(existing) = self.blocks.iter_mut().find(|record| record.hex == hex) {
            existing.block = block;
            self.sync_runtime_for_block(hex);
            return;
        }
        self.blocks.push(InteriorBlockRecord { hex, block });
        self.sync_runtime_for_block(hex);
    }

    pub fn remove_block(&mut self, hex: Axial) {
        self.blocks.retain(|record| record.hex != hex);
        self.containers.retain(|record| record.hex != hex);
        self.mechanic_arms.retain(|record| record.hex != hex);
        self.assembly_nodes.retain(|record| record.hex != hex);
        self.belt_items.retain(|record| record.hex != hex);
        self.logistics_requests
            .retain(|request| request.target_hex != hex);
        for arm in self.mechanic_arms.iter_mut() {
            arm.output_hexes.retain(|target| *target != hex);
            arm.input_hexes.retain(|target| *target != hex);
        }
    }

    pub fn sync_runtime_for_block(&mut self, hex: Axial) {
        let Some(block) = self.block_at(hex).cloned() else {
            return;
        };
        match block.kind {
            MachineBlockType::Pallet | MachineBlockType::Crate => {
                if self.containers.iter().all(|record| record.hex != hex) {
                    self.containers.push(InteriorContainerRecord {
                        hex,
                        items: Vec::new(),
                    });
                }
            }
            MachineBlockType::MechanicArm => {
                if self.mechanic_arms.iter().all(|record| record.hex != hex) {
                    self.mechanic_arms.push(MechanicArmState {
                        hex,
                        held: None,
                        cooldown: 0.0,
                        output_hexes: Vec::new(),
                        input_mode: MechanicArmInputMode::AnyNeighbor,
                        input_hexes: Vec::new(),
                        filters: Vec::new(),
                        action_stage: MechanicArmActionStage::Idle,
                        action_target: None,
                        action_progress: 0.0,
                    });
                }
            }
            MachineBlockType::Assembler => {
                if self.assembly_nodes.iter().all(|record| record.hex != hex) {
                    self.assembly_nodes.push(AssemblyNodeState {
                        hex,
                        selected_recipe: AssemblerRecipeId::BuilderTender,
                        inserted: Vec::new(),
                        completed_outputs: Vec::new(),
                    });
                }
            }
            MachineBlockType::ConveyorBelt
            | MachineBlockType::Chest
            | MachineBlockType::Furnace => {}
        }
    }

    pub fn ensure_runtime_state(&mut self) {
        let block_hexes: Vec<Axial> = self.blocks.iter().map(|record| record.hex).collect();
        for hex in block_hexes {
            self.sync_runtime_for_block(hex);
        }
    }

    pub fn builder_tender_kits(&self) -> u32 {
        self.assembly_nodes
            .iter()
            .flat_map(|node| node.completed_outputs.iter())
            .filter(|stack| stack.kind == InteriorPartKind::BuilderTenderKit)
            .map(|stack| stack.amount)
            .sum()
    }

    pub fn consume_builder_tender_kit(&mut self) -> bool {
        for node in self.assembly_nodes.iter_mut() {
            if take_part(
                &mut node.completed_outputs,
                InteriorPartKind::BuilderTenderKit,
            ) {
                return true;
            }
        }
        false
    }

    pub fn mechanic_arm(&self, hex: Axial) -> Option<&MechanicArmState> {
        self.mechanic_arms.iter().find(|arm| arm.hex == hex)
    }

    pub fn mechanic_arm_mut(&mut self, hex: Axial) -> Option<&mut MechanicArmState> {
        self.mechanic_arms.iter_mut().find(|arm| arm.hex == hex)
    }

    pub fn assembly_node(&self, hex: Axial) -> Option<&AssemblyNodeState> {
        self.assembly_nodes.iter().find(|node| node.hex == hex)
    }

    pub fn assembly_node_mut(&mut self, hex: Axial) -> Option<&mut AssemblyNodeState> {
        self.assembly_nodes.iter_mut().find(|node| node.hex == hex)
    }

    pub fn set_arm_input_mode(&mut self, arm_hex: Axial, mode: MechanicArmInputMode) {
        if let Some(arm) = self.mechanic_arm_mut(arm_hex) {
            arm.input_mode = mode;
            if mode == MechanicArmInputMode::AnyNeighbor {
                arm.input_hexes.clear();
            }
        }
    }

    pub fn toggle_arm_output_hex(&mut self, arm_hex: Axial, target_hex: Axial) {
        if let Some(arm) = self.mechanic_arm_mut(arm_hex) {
            if let Some(index) = arm.output_hexes.iter().position(|hex| *hex == target_hex) {
                arm.output_hexes.remove(index);
            } else if axial_neighbors(arm_hex).contains(&target_hex) {
                arm.output_hexes.push(target_hex);
            }
        }
    }

    pub fn toggle_arm_input_hex(&mut self, arm_hex: Axial, target_hex: Axial) {
        if let Some(arm) = self.mechanic_arm_mut(arm_hex) {
            if let Some(index) = arm.input_hexes.iter().position(|hex| *hex == target_hex) {
                arm.input_hexes.remove(index);
            } else if axial_neighbors(arm_hex).contains(&target_hex) {
                arm.input_hexes.push(target_hex);
            }
        }
    }

    pub fn toggle_arm_filter(&mut self, arm_hex: Axial, part_kind: InteriorPartKind) {
        if let Some(arm) = self.mechanic_arm_mut(arm_hex) {
            if let Some(index) = arm.filters.iter().position(|kind| *kind == part_kind) {
                arm.filters.remove(index);
            } else {
                arm.filters.push(part_kind);
            }
        }
    }

    pub fn set_assembler_recipe(&mut self, hex: Axial, recipe: AssemblerRecipeId) -> bool {
        let Some(node) = self.assembly_node_mut(hex) else {
            return false;
        };
        if !node.inserted.is_empty() || !node.completed_outputs.is_empty() {
            return false;
        }
        node.selected_recipe = recipe;
        true
    }
}

pub fn machine_block_label(kind: MachineBlockType) -> &'static str {
    match kind {
        MachineBlockType::ConveyorBelt => "Conveyor Belt",
        MachineBlockType::MechanicArm => "Mechanic Arm",
        MachineBlockType::Chest => "Chest",
        MachineBlockType::Assembler => "Assembler",
        MachineBlockType::Furnace => "Furnace",
        MachineBlockType::Pallet => "Pallet",
        MachineBlockType::Crate => "Crate",
    }
}

pub fn assembly_recipe_ids() -> &'static [AssemblerRecipeId] {
    &[
        AssemblerRecipeId::BuilderTender,
        AssemblerRecipeId::CargoHauler,
        AssemblerRecipeId::SiteShuttle,
    ]
}

pub fn assembly_recipe_parts(recipe: AssemblerRecipeId) -> &'static [InteriorPartKind] {
    match recipe {
        AssemblerRecipeId::BuilderTender => &[
            InteriorPartKind::ChassisFrame,
            InteriorPartKind::WheelAssembly,
            InteriorPartKind::EngineCore,
            InteriorPartKind::ControlModule,
            InteriorPartKind::ForkCarriage,
            InteriorPartKind::MastSegment,
            InteriorPartKind::HydraulicSet,
            InteriorPartKind::SensorPack,
            InteriorPartKind::FastenerBundle,
        ],
        AssemblerRecipeId::CargoHauler => &[
            InteriorPartKind::ChassisFrame,
            InteriorPartKind::WheelAssembly,
            InteriorPartKind::WheelAssembly,
            InteriorPartKind::EngineCore,
            InteriorPartKind::ControlModule,
            InteriorPartKind::FastenerBundle,
        ],
        AssemblerRecipeId::SiteShuttle => &[
            InteriorPartKind::ChassisFrame,
            InteriorPartKind::EngineCore,
            InteriorPartKind::ControlModule,
            InteriorPartKind::SensorPack,
            InteriorPartKind::FastenerBundle,
        ],
    }
}

pub fn all_interior_filter_parts() -> &'static [InteriorPartKind] {
    &[
        InteriorPartKind::ChassisFrame,
        InteriorPartKind::WheelAssembly,
        InteriorPartKind::EngineCore,
        InteriorPartKind::ControlModule,
        InteriorPartKind::ForkCarriage,
        InteriorPartKind::MastSegment,
        InteriorPartKind::HydraulicSet,
        InteriorPartKind::SensorPack,
        InteriorPartKind::FastenerBundle,
    ]
}

pub fn default_base_interior() -> BaseInteriorState {
    let mut state = BaseInteriorState {
        blocks: vec![
            InteriorBlockRecord {
                hex: Axial { q: -3, r: 0 },
                block: MachineBlock {
                    kind: MachineBlockType::Pallet,
                    rotation: 0,
                },
            },
            InteriorBlockRecord {
                hex: Axial { q: -3, r: 1 },
                block: MachineBlock {
                    kind: MachineBlockType::Crate,
                    rotation: 0,
                },
            },
        ],
        containers: vec![
            InteriorContainerRecord {
                hex: Axial { q: -3, r: 0 },
                items: vec![
                    InteriorPartStack {
                        kind: InteriorPartKind::ChassisFrame,
                        amount: 2,
                    },
                    InteriorPartStack {
                        kind: InteriorPartKind::WheelAssembly,
                        amount: 4,
                    },
                    InteriorPartStack {
                        kind: InteriorPartKind::ForkCarriage,
                        amount: 2,
                    },
                    InteriorPartStack {
                        kind: InteriorPartKind::MastSegment,
                        amount: 2,
                    },
                ],
            },
            InteriorContainerRecord {
                hex: Axial { q: -3, r: 1 },
                items: vec![
                    InteriorPartStack {
                        kind: InteriorPartKind::EngineCore,
                        amount: 2,
                    },
                    InteriorPartStack {
                        kind: InteriorPartKind::ControlModule,
                        amount: 2,
                    },
                    InteriorPartStack {
                        kind: InteriorPartKind::HydraulicSet,
                        amount: 2,
                    },
                    InteriorPartStack {
                        kind: InteriorPartKind::SensorPack,
                        amount: 2,
                    },
                    InteriorPartStack {
                        kind: InteriorPartKind::FastenerBundle,
                        amount: 4,
                    },
                ],
            },
        ],
        belt_items: Vec::new(),
        mechanic_arms: Vec::new(),
        assembly_nodes: Vec::new(),
        forklifts: vec![
            InteriorForkliftState {
                id: 1,
                hex: Axial { q: -4, r: 0 },
                home: Axial { q: -4, r: 0 },
                target: None,
                carried: None,
                move_progress: 0.0,
                request_target: None,
                source_hex: None,
                carried_block: None,
                carried_items: Vec::new(),
            },
            InteriorForkliftState {
                id: 2,
                hex: Axial { q: -4, r: 1 },
                home: Axial { q: -4, r: 1 },
                target: None,
                carried: None,
                move_progress: 0.0,
                request_target: None,
                source_hex: None,
                carried_block: None,
                carried_items: Vec::new(),
            },
        ],
        logistics_requests: Vec::new(),
    };
    state.ensure_runtime_state();
    state
}

fn direction_hex(hex: Axial, rotation: u8) -> Axial {
    axial_neighbors(hex)[rotation as usize % 6]
}

fn container_at_mut(
    interior: &mut BaseInteriorState,
    hex: Axial,
) -> Option<&mut InteriorContainerRecord> {
    interior
        .containers
        .iter_mut()
        .find(|record| record.hex == hex)
}

fn take_part(items: &mut Vec<InteriorPartStack>, kind: InteriorPartKind) -> bool {
    if let Some(stack) = items
        .iter_mut()
        .find(|stack| stack.kind == kind && stack.amount > 0)
    {
        stack.amount -= 1;
        if stack.amount == 0 {
            items.retain(|entry| entry.amount > 0);
        }
        return true;
    }
    false
}

fn push_part(items: &mut Vec<InteriorPartStack>, kind: InteriorPartKind) {
    if let Some(stack) = items.iter_mut().find(|stack| stack.kind == kind) {
        stack.amount += 1;
    } else {
        items.push(InteriorPartStack { kind, amount: 1 });
    }
}

fn belt_has_item(interior: &BaseInteriorState, hex: Axial) -> bool {
    interior.belt_items.iter().any(|item| item.hex == hex)
}

fn assembly_can_accept(node: &AssemblyNodeState, part: InteriorPartKind) -> bool {
    if !node.completed_outputs.is_empty() {
        return false;
    }
    let recipe = assembly_recipe_parts(node.selected_recipe);
    recipe.get(node.inserted.len()).copied() == Some(part)
}

fn part_passes_filter(arm: &MechanicArmState, part: InteriorPartKind) -> bool {
    arm.filters.is_empty() || arm.filters.contains(&part)
}

fn push_unique_part(parts: &mut Vec<InteriorPartKind>, kind: InteriorPartKind) {
    if !parts.contains(&kind) {
        parts.push(kind);
    }
}

fn arm_output_demands(
    interior: &BaseInteriorState,
    arm: &MechanicArmState,
) -> Vec<InteriorPartKind> {
    let mut demanded = Vec::new();
    for output_hex in &arm.output_hexes {
        if let Some(node) = interior.assembly_node(*output_hex) {
            if let Some(next_part) = assembly_recipe_parts(node.selected_recipe)
                .get(node.inserted.len())
                .copied()
            {
                push_unique_part(&mut demanded, next_part);
            }
        }
    }
    demanded
}

fn output_accepts_part(
    interior: &BaseInteriorState,
    output_hex: Axial,
    part: InteriorPartKind,
) -> bool {
    if interior
        .block_at(output_hex)
        .map(|block| block.kind == MachineBlockType::ConveyorBelt)
        .unwrap_or(false)
        && !belt_has_item(interior, output_hex)
    {
        return true;
    }
    if let Some(node) = interior.assembly_node(output_hex) {
        return assembly_can_accept(node, part);
    }
    interior
        .containers
        .iter()
        .any(|container| container.hex == output_hex)
}

fn arm_can_deliver_part(
    interior: &BaseInteriorState,
    arm: &MechanicArmState,
    part: InteriorPartKind,
) -> bool {
    arm.output_hexes
        .iter()
        .any(|output_hex| output_accepts_part(interior, *output_hex, part))
}

fn select_stack_part_for_arm(
    interior: &BaseInteriorState,
    arm: &MechanicArmState,
    items: &[InteriorPartStack],
) -> Option<InteriorPartKind> {
    let demanded = arm_output_demands(interior, arm);
    let demand_is_strict = !demanded.is_empty();
    for stack in items {
        if stack.amount == 0 {
            continue;
        }
        if !part_passes_filter(arm, stack.kind) {
            continue;
        }
        if demand_is_strict && !demanded.contains(&stack.kind) {
            continue;
        }
        if arm_can_deliver_part(interior, arm, stack.kind) {
            return Some(stack.kind);
        }
    }
    None
}

fn pickup_for_arm(
    interior: &mut BaseInteriorState,
    arm_hex: Axial,
) -> Option<(Axial, InteriorPartKind)> {
    let arm = interior.mechanic_arm(arm_hex)?.clone();
    if arm.output_hexes.is_empty() {
        return None;
    }
    let candidates = match arm.input_mode {
        MechanicArmInputMode::AnyNeighbor => axial_neighbors(arm_hex).to_vec(),
        MechanicArmInputMode::ExplicitInputs => arm.input_hexes.clone(),
    };
    let demanded = arm_output_demands(interior, &arm);
    let demand_is_strict = !demanded.is_empty();
    for candidate in candidates {
        let selected_from_container = interior
            .containers
            .iter()
            .find(|container| container.hex == candidate)
            .and_then(|container| select_stack_part_for_arm(interior, &arm, &container.items));
        if let Some(kind) = selected_from_container {
            if let Some(container) = container_at_mut(interior, candidate) {
                if take_part(&mut container.items, kind) {
                    return Some((candidate, kind));
                }
            }
        }
        let selected_from_assembler = interior
            .assembly_nodes
            .iter()
            .find(|node| node.hex == candidate)
            .and_then(|node| select_stack_part_for_arm(interior, &arm, &node.completed_outputs));
        if let Some(kind) = selected_from_assembler {
            if let Some(node) = interior.assembly_node_mut(candidate) {
                if take_part(&mut node.completed_outputs, kind) {
                    return Some((candidate, kind));
                }
            }
        }
        let belt_kind = interior
            .belt_items
            .iter()
            .find(|item| item.hex == candidate)
            .filter(|item| item.progress >= 0.5)
            .map(|item| item.kind);
        if let Some(kind) = belt_kind {
            if part_passes_filter(&arm, kind)
                && (!demand_is_strict || demanded.contains(&kind))
                && arm_can_deliver_part(interior, &arm, kind)
            {
                interior
                    .belt_items
                    .retain(|belt_item| belt_item.hex != candidate);
                return Some((candidate, kind));
            }
        }
        if arm.input_mode == MechanicArmInputMode::ExplicitInputs
            && interior.block_at(candidate).is_none()
        {
            let demanded = arm_output_demands(interior, &arm);
            crate::core::build_units::request_container_delivery(
                interior,
                arm_hex,
                candidate,
                demanded
                    .first()
                    .copied()
                    .or_else(|| arm.filters.first().copied()),
            );
        }
    }
    None
}

fn delivery_target_for_arm(
    interior: &BaseInteriorState,
    arm_hex: Axial,
    held_kind: InteriorPartKind,
) -> Option<Axial> {
    let arm = interior.mechanic_arm(arm_hex)?;
    arm.output_hexes
        .iter()
        .copied()
        .find(|output_hex| output_accepts_part(interior, *output_hex, held_kind))
}

fn complete_delivery_to_target(
    interior: &mut BaseInteriorState,
    target_hex: Axial,
    held_kind: InteriorPartKind,
) -> bool {
    if interior
        .block_at(target_hex)
        .map(|block| block.kind == MachineBlockType::ConveyorBelt)
        .unwrap_or(false)
        && !belt_has_item(interior, target_hex)
    {
        interior.belt_items.push(InteriorBeltItem {
            hex: target_hex,
            kind: held_kind,
            progress: 0.0,
        });
        return true;
    }
    if let Some(node) = interior.assembly_node_mut(target_hex) {
        if assembly_can_accept(node, held_kind) {
            node.inserted.push(held_kind);
            if node.inserted.len() == assembly_recipe_parts(node.selected_recipe).len() {
                push_part(
                    &mut node.completed_outputs,
                    node.selected_recipe.output_kind(),
                );
                node.inserted.clear();
            }
            return true;
        }
    }
    if let Some(container) = container_at_mut(interior, target_hex) {
        push_part(&mut container.items, held_kind);
        return true;
    }
    false
}

fn tick_belts(interior: &mut BaseInteriorState, dt: f32) {
    let mut moves: Vec<(usize, Axial)> = Vec::new();
    let snapshot = interior.belt_items.clone();
    for (index, snapshot_item) in snapshot.iter().enumerate() {
        let new_progress = snapshot_item.progress + dt * 0.65;
        if let Some(item) = interior.belt_items.get_mut(index) {
            item.progress = new_progress;
        }
        if new_progress < 1.0 {
            continue;
        }
        let Some(block) = interior.block_at(snapshot_item.hex) else {
            continue;
        };
        if block.kind != MachineBlockType::ConveyorBelt {
            continue;
        }
        let next_hex = direction_hex(snapshot_item.hex, block.rotation);
        if interior
            .block_at(next_hex)
            .map(|next| next.kind == MachineBlockType::ConveyorBelt)
            .unwrap_or(false)
            && !belt_has_item(interior, next_hex)
        {
            moves.push((index, next_hex));
        }
    }
    for (index, next_hex) in moves.into_iter().rev() {
        if let Some(item) = interior.belt_items.get_mut(index) {
            item.hex = next_hex;
            item.progress = 0.0;
        }
    }
}

fn tick_mechanic_arms(interior: &mut BaseInteriorState, dt: f32) {
    let arm_hexes: Vec<Axial> = interior.mechanic_arms.iter().map(|arm| arm.hex).collect();
    for arm_hex in arm_hexes {
        let Some(index) = interior
            .mechanic_arms
            .iter()
            .position(|arm| arm.hex == arm_hex)
        else {
            continue;
        };
        if let Some(arm) = interior.mechanic_arms.get_mut(index) {
            if arm.action_stage != MechanicArmActionStage::Idle {
                arm.action_progress += dt * 1.8;
                if arm.action_progress < 1.0 {
                    continue;
                }
                arm.action_progress = 0.0;
                match arm.action_stage {
                    // Pickup is already reserved; completing the animation just returns the claw home.
                    MechanicArmActionStage::Pickup => {
                        arm.action_stage = MechanicArmActionStage::Idle;
                        arm.action_target = None;
                        arm.cooldown = 0.08;
                    }
                    MechanicArmActionStage::Deliver => {
                        let target = arm.action_target;
                        let held = arm.held;
                        arm.action_stage = MechanicArmActionStage::Idle;
                        arm.action_target = None;
                        let _ = arm;
                        if let (Some(target_hex), Some(held_kind)) = (target, held) {
                            if complete_delivery_to_target(interior, target_hex, held_kind) {
                                if let Some(arm) = interior.mechanic_arms.get_mut(index) {
                                    arm.held = None;
                                    arm.cooldown = 0.12;
                                }
                            }
                        }
                        continue;
                    }
                    MechanicArmActionStage::Idle => {}
                }
            }
            if arm.cooldown > 0.0 {
                arm.cooldown = (arm.cooldown - dt).max(0.0);
                continue;
            }
        }
        let held = interior.mechanic_arms[index].held;
        if held.is_none() {
            if let Some((source_hex, kind)) = pickup_for_arm(interior, arm_hex) {
                if let Some(arm) = interior.mechanic_arms.get_mut(index) {
                    arm.held = Some(kind);
                    arm.action_stage = MechanicArmActionStage::Pickup;
                    arm.action_target = Some(source_hex);
                    arm.action_progress = 0.0;
                }
            }
            continue;
        }
        if let Some(target_hex) = delivery_target_for_arm(interior, arm_hex, held.unwrap()) {
            if let Some(arm) = interior.mechanic_arms.get_mut(index) {
                arm.action_stage = MechanicArmActionStage::Deliver;
                arm.action_target = Some(target_hex);
                arm.action_progress = 0.0;
            }
        }
    }
}

pub fn tick_base_interior(interior: &mut BaseInteriorState, dt: f32) {
    interior.ensure_runtime_state();
    tick_belts(interior, dt);
    tick_mechanic_arms(interior, dt);
    crate::core::build_units::tick_interior_forklifts(interior, dt);
}

#[cfg(test)]
mod tests {
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
        };

        tick_base_interior(&mut interior, 0.1);
        assert_eq!(
            interior.mechanic_arms[0].held,
            Some(InteriorPartKind::BuilderTenderKit)
        );
        assert!(interior.assembly_nodes[0].completed_outputs.is_empty());
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
}
