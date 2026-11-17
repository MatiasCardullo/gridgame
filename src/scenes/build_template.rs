use macroquad::prelude::*;

use crate::HEX_SIZE;
use crate::core::base_interior::{
    AssemblerRecipeId, BaseInteriorState, InteriorCameraState, InteriorPartKind, InteriorPartStack,
    MechanicArmActionStage, MechanicArmInputMode, all_interior_filter_parts, assembly_recipe_ids,
    assembly_recipe_parts, machine_block_label,
};
use crate::core::map_common::{
    ConfirmAction, MapOutline, build_panel_layout, confirm_label_for_target, draw_common_hud,
    draw_window_frame, in_bounds, is_ui_capturing, popup_rect_near_mouse, run_confirm_window,
};
use crate::core::ui::{WINDOW_TITLE_HEIGHT, WindowState, ui_button};
use crate::core::{
    AppConfig, Axial, FrameContext, MachineBlock, MachineBlockType, RuntimeColors, Scene,
    axial_neighbors, hex_to_pixel, pixel_to_hex,
};
use crate::scenes::planet::{InteriorPickMode, PlanetState};

const CELL_HEIGHT: f32 = 3.2;
const CELL_RADIUS_SCALE: f32 = 1.0;
const BLOCK_Y: f32 = CELL_HEIGHT + 3.0;
const ARM_BASE_HEIGHT: f32 = 16.0;
const FLOOR_DRAW_RADIUS: i32 = 22;
const BELT_HEIGHT: f32 = 3.6;
const BELT_HALF_LENGTH: f32 = 17.0;
const BELT_HALF_WIDTH: f32 = 9.0;
const BELT_WALL_EXTRA_HEIGHT: f32 = 2.0;
const BELT_WALL_THICKNESS: f32 = 1.5;

fn machine_block_color(kind: MachineBlockType) -> Color {
    match kind {
        MachineBlockType::ConveyorBelt => Color::from_rgba(12, 12, 14, 255),
        MachineBlockType::MechanicArm => Color::from_rgba(239, 137, 74, 255),
        MachineBlockType::Chest => Color::from_rgba(150, 150, 150, 255),
        MachineBlockType::Assembler => Color::from_rgba(78, 207, 211, 255),
        MachineBlockType::Furnace => Color::from_rgba(214, 88, 88, 255),
        MachineBlockType::Pallet => Color::from_rgba(151, 108, 66, 255),
        MachineBlockType::Crate => Color::from_rgba(119, 84, 54, 255),
    }
}

fn shade(color: Color, factor: f32) -> Color {
    Color::new(
        (color.r * factor).clamp(0.0, 1.0),
        (color.g * factor).clamp(0.0, 1.0),
        (color.b * factor).clamp(0.0, 1.0),
        color.a,
    )
}

fn stack_summary(items: &[InteriorPartStack]) -> String {
    if items.is_empty() {
        return "empty".to_string();
    }
    items
        .iter()
        .map(|stack| format!("{} x{}", stack.kind.label(), stack.amount))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn selected_filter_summary(filters: &[InteriorPartKind]) -> String {
    if filters.is_empty() {
        "all".to_string()
    } else {
        filters
            .iter()
            .map(|kind| kind.label())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

fn part_draw_color(kind: InteriorPartKind) -> Color {
    match kind {
        InteriorPartKind::ChassisFrame => Color::from_rgba(196, 204, 216, 255),
        InteriorPartKind::WheelAssembly => Color::from_rgba(110, 110, 110, 255),
        InteriorPartKind::EngineCore => Color::from_rgba(255, 142, 92, 255),
        InteriorPartKind::ControlModule => Color::from_rgba(111, 208, 255, 255),
        InteriorPartKind::ForkCarriage => Color::from_rgba(244, 199, 78, 255),
        InteriorPartKind::MastSegment => Color::from_rgba(168, 181, 193, 255),
        InteriorPartKind::HydraulicSet => Color::from_rgba(131, 222, 193, 255),
        InteriorPartKind::SensorPack => Color::from_rgba(170, 146, 255, 255),
        InteriorPartKind::FastenerBundle => Color::from_rgba(230, 230, 230, 255),
        InteriorPartKind::BuilderTenderKit => Color::from_rgba(255, 226, 126, 255),
        InteriorPartKind::CargoHaulerKit => Color::from_rgba(154, 222, 255, 255),
        InteriorPartKind::SiteShuttleKit => Color::from_rgba(202, 255, 168, 255),
    }
}

fn hex_world_center(hex: Axial, y: f32) -> Vec3 {
    let center = hex_to_pixel(hex, HEX_SIZE, Vec2::ZERO);
    vec3(center.x, y, center.y)
}

fn rotation_dir(rotation: u8) -> Vec3 {
    let neighbor = axial_neighbors(Axial { q: 0, r: 0 })[rotation as usize % 6];
    hex_world_center(neighbor, 0.0).normalize_or_zero()
}

fn axial_distance(a: Axial, b: Axial) -> i32 {
    let dq = a.q - b.q;
    let dr = a.r - b.r;
    let ds = (a.q + a.r) - (b.q + b.r);
    (dq.abs() + dr.abs() + ds.abs()) / 2
}

fn draw_box(center: Vec3, size: Vec3, fill: Color, outline: Color, wire_only: bool) {
    if wire_only {
        draw_cube_wires(center, size, outline);
        return;
    }
    draw_cube(center, size, None, fill);
    draw_cube_wires(center, size, outline);
}

fn draw_hex_tile(hex: Axial, y: f32, fill: Color, outline: Color, wire_only: bool) {
    let center = hex_world_center(hex, y);
    let radius = HEX_SIZE * CELL_RADIUS_SCALE;
    let height = 0.24;
    let params = DrawCylinderParams {
        sides: 6,
        draw_mode: if wire_only {
            DrawMode::Lines
        } else {
            DrawMode::Triangles
        },
    };
    draw_cylinder_ex(
        vec3(center.x, y - height, center.z),
        radius,
        radius,
        height,
        None,
        fill,
        params,
    );
    if !wire_only {
        let outline_params = DrawCylinderParams {
            sides: 6,
            draw_mode: DrawMode::Lines,
        };
        draw_cylinder_ex(
            vec3(center.x, y - height, center.z),
            radius,
            radius,
            height,
            None,
            outline,
            outline_params,
        );
    }
}

fn draw_hex_outline(hex: Axial, y: f32, color: Color) {
    let center = hex_world_center(hex, y);
    let radius = HEX_SIZE * CELL_RADIUS_SCALE;
    let params = DrawCylinderParams {
        sides: 6,
        draw_mode: DrawMode::Lines,
    };
    draw_cylinder_ex(
        vec3(center.x, y - 0.02, center.z),
        radius,
        radius,
        0.04,
        None,
        color,
        params,
    );
}

fn arm_home_hand(arm_hex: Axial, rest_rotation: u8) -> Vec3 {
    hex_world_center(arm_hex, ARM_BASE_HEIGHT + 8.0)
        + rotation_dir(rest_rotation) * (HEX_SIZE * 1.65)
}

fn arm_target_hand(interior: &BaseInteriorState, target_hex: Axial) -> Vec3 {
    let target_height = match interior.block_at(target_hex).map(|block| block.kind) {
        Some(MachineBlockType::ConveyorBelt) => BLOCK_Y + 10.4,
        Some(MachineBlockType::Crate | MachineBlockType::Chest | MachineBlockType::Pallet) => {
            BLOCK_Y + 16.5
        }
        Some(MachineBlockType::Assembler) => BLOCK_Y + 16.0,
        _ => BLOCK_Y + 12.0,
    };
    hex_world_center(target_hex, target_height)
}

fn ease_arm_motion(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    progress * progress * (3.0 - 2.0 * progress)
}

fn arm_hand_world(
    interior: &BaseInteriorState,
    arm_hex: Axial,
    rest_rotation: u8,
    action_origin: Option<Axial>,
    target: Option<Axial>,
    progress: f32,
    stage: MechanicArmActionStage,
) -> Vec3 {
    let home = arm_home_hand(arm_hex, rest_rotation);
    let Some(target_hex) = target else {
        return home;
    };
    let target_hand = arm_target_hand(interior, target_hex);
    let (from, to, phase) = match stage {
        MechanicArmActionStage::Pickup => (
            action_origin
                .map(|origin| arm_target_hand(interior, origin))
                .unwrap_or(home),
            target_hand,
            progress,
        ),
        MechanicArmActionStage::Deliver => (
            action_origin
                .map(|origin| arm_target_hand(interior, origin))
                .unwrap_or(home),
            target_hand,
            progress,
        ),
        MechanicArmActionStage::Retract => (target_hand, home, progress),
        MechanicArmActionStage::Idle => return home,
    };
    let mut hand = from.lerp(to, ease_arm_motion(phase));
    hand.y += (phase.clamp(0.0, 1.0) * std::f32::consts::PI).sin() * 5.0;
    hand
}

fn draw_arm_link(start: Vec3, end: Vec3, width: f32, depth: f32, fill: Color, outline: Color) {
    let delta = end - start;
    let length = delta.length();
    if length <= f32::EPSILON {
        return;
    }
    let axis = delta / length;
    let mut side = axis.cross(vec3(0.0, 1.0, 0.0)).normalize_or_zero();
    if side.length_squared() <= f32::EPSILON {
        side = vec3(1.0, 0.0, 0.0);
    }
    let up = side.cross(axis).normalize_or_zero();
    let x = axis * length;
    let y = up * depth;
    let z = side * width;
    let origin = (start + end) * 0.5 - x * 0.5 - y * 0.5 - z * 0.5;
    draw_affine_parallelepiped(origin, x, y, z, None, fill);
    let p000 = origin;
    let p100 = origin + x;
    let p010 = origin + y;
    let p110 = origin + x + y;
    let p001 = origin + z;
    let p101 = origin + x + z;
    let p011 = origin + y + z;
    let p111 = origin + x + y + z;
    for (a, b) in [
        (p000, p100),
        (p000, p010),
        (p000, p001),
        (p100, p110),
        (p100, p101),
        (p010, p110),
        (p010, p011),
        (p001, p101),
        (p001, p011),
        (p110, p111),
        (p101, p111),
        (p011, p111),
    ] {
        draw_line_3d(a, b, outline);
    }
}

fn forklift_world_center(from: Axial, to: Option<Axial>, progress: f32) -> Vec3 {
    let start = hex_world_center(from, BLOCK_Y + 2.0);
    let Some(target_hex) = to else {
        return start;
    };
    let end = hex_world_center(target_hex, BLOCK_Y + 2.0);
    start.lerp(end, progress.clamp(0.0, 1.0))
}

fn forklift_forward_dir(from: Axial, to: Option<Axial>) -> Vec3 {
    let Some(target_hex) = to else {
        return vec3(1.0, 0.0, 0.0);
    };
    let from_center = hex_world_center(from, BLOCK_Y + 2.0);
    let to_center = hex_world_center(target_hex, BLOCK_Y + 2.0);
    let flat = vec3(
        to_center.x - from_center.x,
        0.0,
        to_center.z - from_center.z,
    );
    if flat.length_squared() <= f32::EPSILON {
        vec3(1.0, 0.0, 0.0)
    } else {
        flat.normalize()
    }
}

fn belt_item_world_center(interior: &BaseInteriorState, hex: Axial, progress: f32) -> Vec3 {
    let from = hex_world_center(hex, BLOCK_Y + 5.4);
    let Some(block) = interior.block_at(hex) else {
        return from;
    };
    if block.kind != MachineBlockType::ConveyorBelt {
        return from;
    }
    let next_hex = axial_neighbors(hex)[block.rotation as usize % 6];
    let next_is_belt = interior
        .block_at(next_hex)
        .map(|next| next.kind == MachineBlockType::ConveyorBelt)
        .unwrap_or(false);
    if !next_is_belt {
        return from;
    }
    let next_has_item = interior.belt_items.iter().any(|item| item.hex == next_hex);
    if next_has_item {
        let stop_progress = progress.clamp(0.0, 0.94);
        let to = hex_world_center(next_hex, BLOCK_Y + 5.4);
        return from.lerp(to, stop_progress);
    }
    let to = hex_world_center(next_hex, BLOCK_Y + 5.4);
    from.lerp(to, progress.clamp(0.0, 1.0))
}

fn belt_end_side_lengths(
    direction: Vec3,
    connected_direction: Vec3,
    center_distance: f32,
    front: bool,
) -> (f32, f32) {
    belt_end_side_lengths_for_width(
        direction,
        connected_direction,
        center_distance,
        front,
        BELT_HALF_WIDTH,
    )
}

fn belt_end_side_lengths_for_width(
    direction: Vec3,
    connected_direction: Vec3,
    center_distance: f32,
    front: bool,
    half_width: f32,
) -> (f32, f32) {
    let axis = if front { direction } else { -direction };
    let side = vec3(-axis.z, 0.0, axis.x);
    let connected_side = vec3(-connected_direction.z, 0.0, connected_direction.x);
    let current_to_connected = axis * center_distance;
    let side_offset = (-current_to_connected).dot(connected_side);
    let across_axis = axis.dot(connected_side);
    if across_axis.abs() < 1e-4 || side_offset.abs() < 1e-4 {
        let half_distance = center_distance * 0.5;
        return (half_distance, half_distance);
    }

    let connected_edge = side_offset.signum() * half_width;
    let negative_side =
        (connected_edge - side_offset + half_width * side.dot(connected_side)) / across_axis;
    let positive_side =
        (connected_edge - side_offset - half_width * side.dot(connected_side)) / across_axis;
    let lengths = (negative_side.max(0.0), positive_side.max(0.0));
    if front {
        lengths
    } else {
        (lengths.1, lengths.0)
    }
}

fn belt_connection(
    interior: &BaseInteriorState,
    hex: Axial,
    neighbor_index: usize,
) -> Option<(Vec3, f32)> {
    let neighbor_hex = axial_neighbors(hex)[neighbor_index];
    let block = interior.block_at(neighbor_hex)?;
    if block.kind != MachineBlockType::ConveyorBelt {
        return None;
    }
    let center = hex_world_center(hex, 0.0);
    let neighbor_center = hex_world_center(neighbor_hex, 0.0);
    Some((
        rotation_dir(block.rotation),
        center.distance(neighbor_center),
    ))
}

fn belt_side_wall_gaps(
    interior: &BaseInteriorState,
    hex: Axial,
    rotation: usize,
) -> [Vec<(f32, f32)>; 2] {
    let rotation = rotation % 6;
    let front_index = rotation;
    let back_index = (rotation + 3) % 6;
    let direction = rotation_dir(rotation as u8);
    let side = vec3(-direction.z, 0.0, direction.x);
    let center = hex_world_center(hex, 0.0);
    let neighbors = axial_neighbors(hex);
    let mut gaps = [Vec::new(), Vec::new()];

    for (neighbor_index, neighbor_hex) in neighbors.into_iter().enumerate() {
        if neighbor_index == front_index || neighbor_index == back_index {
            continue;
        }
        let Some(block) = interior.block_at(neighbor_hex) else {
            continue;
        };
        let neighbor_rotation = block.rotation as usize % 6;
        if block.kind != MachineBlockType::ConveyorBelt
            || (neighbor_rotation != neighbor_index
                && neighbor_rotation != (neighbor_index + 3) % 6)
        {
            continue;
        }
        let relative = hex_world_center(neighbor_hex, 0.0) - center;
        let side_index = usize::from(relative.dot(side) >= 0.0);
        let neighbor_direction = rotation_dir(block.rotation);
        let neighbor_front = belt_connection(interior, neighbor_hex, block.rotation as usize % 6);
        let neighbor_back =
            belt_connection(interior, neighbor_hex, (block.rotation as usize + 3) % 6);
        let neighbor_front_lengths = neighbor_front
            .map(|(connected_direction, distance)| {
                belt_end_side_lengths(neighbor_direction, connected_direction, distance, true)
            })
            .unwrap_or((BELT_HALF_LENGTH, BELT_HALF_LENGTH));
        let neighbor_back_lengths = neighbor_back
            .map(|(connected_direction, distance)| {
                belt_end_side_lengths(neighbor_direction, connected_direction, distance, false)
            })
            .unwrap_or((BELT_HALF_LENGTH, BELT_HALF_LENGTH));
        let neighbor_side = vec3(-neighbor_direction.z, 0.0, neighbor_direction.x);
        let neighbor_center = hex_world_center(neighbor_hex, 0.0);
        let neighbor_polygon = [
            neighbor_center + neighbor_direction * neighbor_front_lengths.0
                - neighbor_side * BELT_HALF_WIDTH,
            neighbor_center
                + neighbor_direction * neighbor_front_lengths.1
                + neighbor_side * BELT_HALF_WIDTH,
            neighbor_center - neighbor_direction * neighbor_back_lengths.1
                + neighbor_side * BELT_HALF_WIDTH,
            neighbor_center
                - neighbor_direction * neighbor_back_lengths.0
                - neighbor_side * BELT_HALF_WIDTH,
        ];
        let wall_line_origin = center
            + side
                * (if side_index == 0 {
                    -BELT_HALF_WIDTH
                } else {
                    BELT_HALF_WIDTH
                });
        let mut intersections = Vec::new();
        for edge_index in 0..neighbor_polygon.len() {
            let start = neighbor_polygon[edge_index];
            let end = neighbor_polygon[(edge_index + 1) % neighbor_polygon.len()];
            let edge = end - start;
            let offset = start - wall_line_origin;
            let denominator = direction.x * edge.z - direction.z * edge.x;
            if denominator.abs() < 1e-4 {
                continue;
            }
            let distance = (offset.x * edge.z - offset.z * edge.x) / denominator;
            let edge_fraction = (offset.x * direction.z - offset.z * direction.x) / denominator;
            if (-1e-4..=1.0001).contains(&edge_fraction) {
                intersections.push(distance);
            }
        }
        intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        intersections.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
        if let (Some(first), Some(last)) = (intersections.first(), intersections.last()) {
            if last - first > 1e-3 {
                gaps[side_index].push((*first, *last));
            }
        }
    }

    gaps
}

fn belt_wall_sections(start: f32, end: f32, gaps: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut gaps = gaps.to_vec();
    gaps.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut sections = Vec::with_capacity(gaps.len() + 1);
    let mut cursor = start;
    for (gap_start, gap_end) in gaps {
        let gap_start = gap_start.max(start).min(end);
        let gap_end = gap_end.max(start).min(end);
        if gap_start > cursor {
            sections.push((cursor, gap_start));
        }
        cursor = cursor.max(gap_end);
    }
    if cursor < end {
        sections.push((cursor, end));
    }
    sections
}

fn belt_has_angled_front_connection(
    interior: &BaseInteriorState,
    hex: Axial,
    rotation: u8,
) -> bool {
    let direction = rotation_dir(rotation);
    belt_connection(interior, hex, rotation as usize % 6)
        .map(|(connected_direction, _)| direction.dot(connected_direction) < 0.999)
        .unwrap_or(false)
}

fn push_belt_quad(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    points: [Vec3; 4],
    color: Color,
) {
    let first = vertices.len() as u16;
    vertices.extend(
        points
            .into_iter()
            .map(|point| Vertex::new(point.x, point.y, point.z, 0.0, 0.0, color)),
    );
    indices.extend([first, first + 1, first + 2, first + 1, first + 3, first + 2]);
}

fn draw_belt_wall(
    center: Vec3,
    direction: Vec3,
    side_sign: f32,
    start_fraction: f32,
    end_fraction: f32,
    inner_back_length: f32,
    inner_front_length: f32,
    outer_back_length: f32,
    outer_front_length: f32,
    fill: Color,
    outline: Color,
    wire_only: bool,
) {
    let side = vec3(-direction.z, 0.0, direction.x);
    let inner_offset = side * (BELT_HALF_WIDTH * side_sign);
    let outer_offset = side * ((BELT_HALF_WIDTH + BELT_WALL_THICKNESS) * side_sign);
    let bottom = vec3(0.0, -center.y, 0.0);
    let top = vec3(0.0, BELT_HEIGHT + BELT_WALL_EXTRA_HEIGHT, 0.0);
    let inner_start_length =
        -inner_back_length + (inner_back_length + inner_front_length) * start_fraction;
    let inner_end_length =
        -inner_back_length + (inner_back_length + inner_front_length) * end_fraction;
    let outer_start_length =
        -outer_back_length + (outer_back_length + outer_front_length) * start_fraction;
    let outer_end_length =
        -outer_back_length + (outer_back_length + outer_front_length) * end_fraction;
    let inner_start = center + direction * inner_start_length + inner_offset;
    let inner_end = center + direction * inner_end_length + inner_offset;
    let outer_start = center + direction * outer_start_length + outer_offset;
    let outer_end = center + direction * outer_end_length + outer_offset;
    let corners = [
        inner_start + bottom,
        inner_end + bottom,
        outer_start + bottom,
        outer_end + bottom,
        inner_start + top,
        inner_end + top,
        outer_start + top,
        outer_end + top,
    ];

    if wire_only {
        for (start, end) in [
            (0, 1),
            (2, 3),
            (4, 5),
            (6, 7),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
            (0, 2),
            (1, 3),
            (4, 6),
            (5, 7),
        ] {
            draw_line_3d(corners[start], corners[end], outline);
        }
        return;
    }

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    push_belt_quad(
        &mut vertices,
        &mut indices,
        [corners[2], corners[3], corners[6], corners[7]],
        fill,
    );
    push_belt_quad(
        &mut vertices,
        &mut indices,
        [corners[0], corners[4], corners[1], corners[5]],
        shade(fill, 0.8),
    );
    push_belt_quad(
        &mut vertices,
        &mut indices,
        [corners[4], corners[6], corners[5], corners[7]],
        shade(fill, 1.2),
    );
    push_belt_quad(
        &mut vertices,
        &mut indices,
        [corners[0], corners[2], corners[4], corners[6]],
        shade(fill, 0.9),
    );
    push_belt_quad(
        &mut vertices,
        &mut indices,
        [corners[1], corners[5], corners[3], corners[7]],
        shade(fill, 0.9),
    );
    draw_mesh(&Mesh {
        vertices,
        indices,
        texture: None,
    });
}

fn draw_belt_walls(
    center: Vec3,
    direction: Vec3,
    front_lengths: (f32, f32),
    back_lengths: (f32, f32),
    front_outer_lengths: (f32, f32),
    back_outer_lengths: (f32, f32),
    side_wall_gaps: [Vec<(f32, f32)>; 2],
    wire_only: bool,
) {
    let wall_color = Color::from_rgba(115, 115, 120, 255);
    let wall_outline = shade(wall_color, 0.55);
    for (side_index, side_sign) in [-1.0_f32, 1.0].into_iter().enumerate() {
        let front_length = [front_lengths.0, front_lengths.1][side_index];
        let back_length = [back_lengths.0, back_lengths.1][side_index];
        let front_outer_length = [front_outer_lengths.0, front_outer_lengths.1][side_index];
        let back_outer_length = [back_outer_lengths.0, back_outer_lengths.1][side_index];
        let total_inner_length = back_length + front_length;
        if total_inner_length <= f32::EPSILON {
            continue;
        }
        for (start, end) in
            belt_wall_sections(-back_length, front_length, &side_wall_gaps[side_index])
        {
            draw_belt_wall(
                center,
                direction,
                side_sign,
                (start + back_length) / total_inner_length,
                (end + back_length) / total_inner_length,
                back_length,
                front_length,
                back_outer_length,
                front_outer_length,
                wall_color,
                wall_outline,
                wire_only,
            );
        }
    }
}

fn draw_belt_prism(
    center: Vec3,
    direction: Vec3,
    front_lengths: (f32, f32),
    back_lengths: (f32, f32),
    front_connected: bool,
    back_connected: bool,
    fill: Color,
    outline: Color,
    wire_only: bool,
) {
    let side = vec3(-direction.z, 0.0, direction.x);
    let top_y = center.y + BELT_HEIGHT;
    let front_negative =
        vec3(center.x, top_y, center.z) + direction * front_lengths.0 - side * BELT_HALF_WIDTH;
    let front_positive =
        vec3(center.x, top_y, center.z) + direction * front_lengths.1 + side * BELT_HALF_WIDTH;
    let back_negative =
        vec3(center.x, top_y, center.z) - direction * back_lengths.0 - side * BELT_HALF_WIDTH;
    let back_positive =
        vec3(center.x, top_y, center.z) - direction * back_lengths.1 + side * BELT_HALF_WIDTH;
    let bottom_offset = vec3(0.0, -BELT_HEIGHT, 0.0);
    let front_negative_bottom = front_negative + bottom_offset;
    let front_positive_bottom = front_positive + bottom_offset;
    let back_negative_bottom = back_negative + bottom_offset;
    let back_positive_bottom = back_positive + bottom_offset;

    if wire_only {
        let mut edges = Vec::new();
        if !front_connected {
            edges.extend([
                (front_negative, front_positive),
                (front_negative_bottom, front_positive_bottom),
            ]);
        }
        if !back_connected {
            edges.extend([
                (back_negative, back_positive),
                (back_negative_bottom, back_positive_bottom),
            ]);
        }
        for (start, end) in edges {
            draw_line_3d(start, end, outline);
        }
    } else {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        push_belt_quad(
            &mut vertices,
            &mut indices,
            [front_negative, front_positive, back_negative, back_positive],
            fill,
        );
        if !front_connected {
            push_belt_quad(
                &mut vertices,
                &mut indices,
                [
                    front_negative_bottom,
                    front_positive_bottom,
                    front_negative,
                    front_positive,
                ],
                Color::from_rgba(19, 19, 22, 255),
            );
        }
        if !back_connected {
            push_belt_quad(
                &mut vertices,
                &mut indices,
                [
                    back_negative_bottom,
                    back_positive_bottom,
                    back_negative,
                    back_positive,
                ],
                Color::from_rgba(19, 19, 22, 255),
            );
        }
        draw_mesh(&Mesh {
            vertices,
            indices,
            texture: None,
        });
    }
}

fn build_template_camera(state: &InteriorCameraState) -> Camera3D {
    let position = vec3(state.focus_x, state.distance, state.focus_z);
    let forward = vec3(
        state.yaw.cos() * state.pitch.cos(),
        state.pitch.sin(),
        state.yaw.sin() * state.pitch.cos(),
    )
    .normalize_or_zero();
    Camera3D {
        position,
        target: position + forward,
        up: vec3(0.0, 1.0, 0.0),
        fovy: 45.0,
        z_near: 1.0,
        z_far: 4_000.0,
        ..Default::default()
    }
}

fn update_build_template_camera(
    camera: &mut InteriorCameraState,
    mouse: Vec2,
    ui_capturing: bool,
    dragging: &mut bool,
    last_mouse: &mut Vec2,
) {
    let camera_active = !ui_capturing && is_mouse_button_down(MouseButton::Right);
    if camera_active {
        let delta = mouse - *last_mouse;
        camera.yaw -= delta.x * 0.0055;
        camera.pitch = (camera.pitch - delta.y * 0.0045).clamp(-1.25, 1.25);
        *dragging = true;
    } else {
        *dragging = false;
    }

    if !ui_capturing {
        let dt = get_frame_time();
        let speed = if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
            320.0
        } else {
            180.0
        };
        let forward = vec3(camera.yaw.cos(), 0.0, camera.yaw.sin());
        let right = vec3(-camera.yaw.sin(), 0.0, camera.yaw.cos());
        let mut move_dir = Vec3::ZERO;
        if is_key_down(KeyCode::W) {
            move_dir += forward;
        }
        if is_key_down(KeyCode::S) {
            move_dir -= forward;
        }
        if is_key_down(KeyCode::D) {
            move_dir += right;
        }
        if is_key_down(KeyCode::A) {
            move_dir -= right;
        }
        if is_key_down(KeyCode::Space) {
            move_dir.y += 1.0;
        }
        if is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl) {
            move_dir.y -= 1.0;
        }
        if move_dir.length_squared() > f32::EPSILON {
            let step = move_dir.normalize() * (speed * dt);
            camera.focus_x += step.x;
            camera.distance = (camera.distance + step.y).clamp(4.0, 500.0);
            camera.focus_z += step.z;
        }
    }
    *last_mouse = mouse;
}

fn ray_from_mouse(camera: &Camera3D, mouse: Vec2) -> Option<(Vec3, Vec3)> {
    let x = (mouse.x / screen_width()) * 2.0 - 1.0;
    let y = 1.0 - (mouse.y / screen_height()) * 2.0;
    let inv = camera.matrix().inverse();
    let near = inv * vec4(x, y, -1.0, 1.0);
    let far = inv * vec4(x, y, 1.0, 1.0);
    if near.w.abs() < 1e-5 || far.w.abs() < 1e-5 {
        return None;
    }
    let p0 = vec3(near.x / near.w, near.y / near.w, near.z / near.w);
    let p1 = vec3(far.x / far.w, far.y / far.w, far.z / far.w);
    Some((p0, (p1 - p0).normalize_or_zero()))
}

fn ray_ground_intersection(origin: Vec3, dir: Vec3, y: f32) -> Option<Vec3> {
    if dir.y.abs() < 1e-5 {
        return None;
    }
    let t = (y - origin.y) / dir.y;
    if t <= 0.0 {
        return None;
    }
    Some(origin + dir * t)
}

fn hover_hex_from_mouse_3d(camera: &Camera3D, mouse: Vec2) -> Option<Axial> {
    let (origin, dir) = ray_from_mouse(camera, mouse)?;
    let hit = ray_ground_intersection(origin, dir, 0.0)?;
    Some(pixel_to_hex(vec2(hit.x, hit.z), HEX_SIZE, Vec2::ZERO))
}

fn draw_machine_block(
    interior: &BaseInteriorState,
    hex: Axial,
    kind: MachineBlockType,
    rotation: u8,
    center: Vec3,
    wire_only: bool,
) {
    let fill = machine_block_color(kind);
    let outline = shade(fill, 0.55);
    match kind {
        MachineBlockType::ConveyorBelt => {
            let dir = rotation_dir(rotation);
            let front_connection = belt_connection(interior, hex, rotation as usize % 6);
            let back_connection = belt_connection(interior, hex, (rotation as usize + 3) % 6);
            let front_lengths = front_connection
                .map(|(connected_dir, distance)| {
                    belt_end_side_lengths(dir, connected_dir, distance, true)
                })
                .unwrap_or((BELT_HALF_LENGTH, BELT_HALF_LENGTH));
            let back_lengths = back_connection
                .map(|(connected_dir, distance)| {
                    belt_end_side_lengths(dir, connected_dir, distance, false)
                })
                .unwrap_or((BELT_HALF_LENGTH, BELT_HALF_LENGTH));
            let wall_half_width = BELT_HALF_WIDTH + BELT_WALL_THICKNESS;
            let front_outer_lengths = front_connection
                .map(|(connected_dir, distance)| {
                    belt_end_side_lengths_for_width(
                        dir,
                        connected_dir,
                        distance,
                        true,
                        wall_half_width,
                    )
                })
                .unwrap_or((BELT_HALF_LENGTH, BELT_HALF_LENGTH));
            let back_outer_lengths = back_connection
                .map(|(connected_dir, distance)| {
                    belt_end_side_lengths_for_width(
                        dir,
                        connected_dir,
                        distance,
                        false,
                        wall_half_width,
                    )
                })
                .unwrap_or((BELT_HALF_LENGTH, BELT_HALF_LENGTH));
            let side_wall_gaps = belt_side_wall_gaps(interior, hex, rotation as usize % 6);
            draw_belt_prism(
                center,
                dir,
                front_lengths,
                back_lengths,
                front_connection.is_some(),
                back_connection.is_some(),
                fill,
                outline,
                wire_only,
            );
            draw_belt_walls(
                center,
                dir,
                front_lengths,
                back_lengths,
                front_outer_lengths,
                back_outer_lengths,
                side_wall_gaps,
                wire_only,
            );
            let side = vec3(-dir.z, 0.0, dir.x);
            let top = center + vec3(0.0, 4.6, 0.0);
            let arrow_color = Color::from_rgba(255, 220, 0, 255);
            let mut arrow_offsets = vec![-7.0_f32, 6.0];
            if belt_has_angled_front_connection(interior, hex, rotation) {
                arrow_offsets.push(19.0);
            }
            for offset in arrow_offsets {
                let mid = top + dir * offset;
                let a = mid - dir * 5.0 + side * 3.5;
                let b = mid;
                let c = mid - dir * 5.0 - side * 3.5;
                draw_line_3d(a, b, arrow_color);
                draw_line_3d(c, b, arrow_color);
            }
        }
        MachineBlockType::MechanicArm => {
            draw_box(
                center + vec3(0.0, 2.0, 0.0),
                vec3(28.0, 4.0, 28.0),
                fill,
                outline,
                wire_only,
            );
        }
        MachineBlockType::Chest => {
            draw_box(
                center + vec3(0.0, 8.0, 0.0),
                vec3(24.0, 16.0, 24.0),
                fill,
                outline,
                wire_only,
            );
        }
        MachineBlockType::Assembler => {
            draw_box(
                center + vec3(0.0, 12.0, 0.0),
                vec3(30.0, 24.0, 30.0),
                fill,
                outline,
                wire_only,
            );
            draw_box(
                center + vec3(0.0, 24.0, 0.0),
                vec3(18.0, 6.0, 18.0),
                shade(fill, 1.15),
                outline,
                wire_only,
            );
        }
        MachineBlockType::Furnace => {
            draw_box(
                center + vec3(0.0, 10.0, 0.0),
                vec3(28.0, 20.0, 28.0),
                fill,
                outline,
                wire_only,
            );
            draw_box(
                center + vec3(0.0, 23.0, 0.0),
                vec3(14.0, 8.0, 14.0),
                shade(fill, 0.82),
                outline,
                wire_only,
            );
        }
        MachineBlockType::Pallet => {
            draw_box(
                center + vec3(0.0, 2.2, 0.0),
                vec3(26.0, 4.4, 26.0),
                fill,
                outline,
                wire_only,
            );
            for offset in [-7.0_f32, 0.0, 7.0] {
                draw_box(
                    center + vec3(0.0, 4.7, offset),
                    vec3(28.0, 1.2, 3.2),
                    Color::from_rgba(207, 171, 118, 255),
                    outline,
                    wire_only,
                );
            }
        }
        MachineBlockType::Crate => {
            draw_box(
                center + vec3(0.0, 7.0, 0.0),
                vec3(24.0, 14.0, 24.0),
                fill,
                outline,
                wire_only,
            );
        }
    }
}

fn draw_world(
    interior: &BaseInteriorState,
    hover_hex: Option<Axial>,
    preview: Option<(Axial, MachineBlockType, u8)>,
    camera_center_hex: Axial,
    config: &AppConfig,
    colors: &RuntimeColors,
) {
    for r in (camera_center_hex.r - FLOOR_DRAW_RADIUS)..=(camera_center_hex.r + FLOOR_DRAW_RADIUS) {
        for q in
            (camera_center_hex.q - FLOOR_DRAW_RADIUS)..=(camera_center_hex.q + FLOOR_DRAW_RADIUS)
        {
            let hex = Axial { q, r };
            if !in_bounds(hex, MapOutline::Hexagon) {
                continue;
            }
            if axial_distance(hex, camera_center_hex) > FLOOR_DRAW_RADIUS {
                continue;
            }
            let occupied = interior.block_at(hex).is_some();
            let hovered = hover_hex == Some(hex);
            let base = if occupied {
                Color::from_rgba(58, 70, 82, 255)
            } else {
                Color::from_rgba(34, 44, 56, 255)
            };
            let fill = if hovered {
                Color::from_rgba(92, 132, 170, 255)
            } else {
                base
            };
            let outline = if hovered { colors.hover } else { colors.grid };
            draw_hex_tile(hex, 0.0, fill, outline, false);
        }
    }

    for belt_item in &interior.belt_items {
        let color = part_draw_color(belt_item.kind);
        draw_box(
            belt_item_world_center(interior, belt_item.hex, belt_item.progress),
            vec3(6.0, 4.0, 6.0),
            color,
            shade(color, 0.55),
            false,
        );
    }

    for block_record in &interior.blocks {
        let hex = block_record.hex;
        let block = &block_record.block;
        let center = hex_world_center(hex, BLOCK_Y);
        draw_machine_block(interior, hex, block.kind, block.rotation, center, false);
    }

    for container in &interior.containers {
        for (index, stack) in container.items.iter().enumerate() {
            if index >= 4 || stack.amount == 0 {
                continue;
            }
            let offset_x = -8.0 + (index as f32 % 2.0) * 16.0;
            let offset_z = -8.0 + (index as f32 / 2.0).floor() * 16.0;
            let color = part_draw_color(stack.kind);
            draw_box(
                hex_world_center(container.hex, BLOCK_Y + 8.0)
                    + vec3(
                        offset_x,
                        3.5 + (stack.amount.min(3) as f32 - 1.0) * 1.3,
                        offset_z,
                    ),
                vec3(7.0, 7.0, 7.0),
                color,
                shade(color, 0.55),
                false,
            );
        }
    }

    for arm in &interior.mechanic_arms {
        let rest_rotation = interior
            .block_at(arm.hex)
            .map(|block| block.rotation)
            .unwrap_or(0);
        let steel = Color::from_rgba(91, 107, 125, 255);
        let dark_steel = Color::from_rgba(42, 52, 66, 255);
        let accent = Color::from_rgba(239, 151, 59, 255);
        draw_box(
            hex_world_center(arm.hex, ARM_BASE_HEIGHT - 3.5),
            vec3(18.0, 5.0, 18.0),
            steel,
            dark_steel,
            false,
        );
        draw_box(
            hex_world_center(arm.hex, ARM_BASE_HEIGHT + 3.0),
            vec3(11.0, 7.0, 11.0),
            dark_steel,
            colors.text_primary,
            false,
        );
        draw_box(
            hex_world_center(arm.hex, ARM_BASE_HEIGHT + 7.0),
            vec3(8.0, 3.0, 8.0),
            accent,
            shade(accent, 0.58),
            false,
        );
        let hand = if arm.action_stage == MechanicArmActionStage::Idle {
            arm_home_hand(arm.hex, rest_rotation)
        } else {
            arm_hand_world(
                interior,
                arm.hex,
                rest_rotation,
                arm.action_origin,
                arm.action_target,
                arm.action_progress,
                arm.action_stage,
            )
        };
        let shoulder = hex_world_center(arm.hex, ARM_BASE_HEIGHT + 8.0);
        let elbow = shoulder.lerp(hand, 0.52) + vec3(0.0, 7.0, 0.0);
        draw_arm_link(shoulder, elbow, 11.0, 9.0, steel, dark_steel);
        draw_arm_link(elbow, hand, 9.0, 8.0, steel, dark_steel);
        draw_box(
            elbow,
            vec3(12.0, 10.0, 12.0),
            accent,
            shade(accent, 0.55),
            false,
        );
        draw_box(hand, vec3(10.0, 7.0, 10.0), dark_steel, steel, false);
        let forward = vec3(hand.x - shoulder.x, 0.0, hand.z - shoulder.z).normalize_or_zero();
        let side = vec3(-forward.z, 0.0, forward.x);
        for offset in [-3.5_f32, 3.5] {
            let finger_start = hand + side * offset;
            draw_arm_link(
                finger_start,
                finger_start + forward * 9.0,
                2.5,
                2.5,
                accent,
                shade(accent, 0.55),
            );
        }
        let show_item = match arm.action_stage {
            MechanicArmActionStage::Pickup => arm.action_progress >= 0.95 && arm.held.is_some(),
            MechanicArmActionStage::Deliver | MechanicArmActionStage::Retract => arm.held.is_some(),
            MechanicArmActionStage::Idle => arm.held.is_some(),
        };
        if show_item {
            if let Some(kind) = arm.held {
                let color = part_draw_color(kind);
                draw_box(
                    hand + vec3(0.0, -5.0, 0.0) + forward * 3.0,
                    vec3(3.6, 3.6, 3.6),
                    color,
                    shade(color, 0.55),
                    false,
                );
            }
        }
        for output_hex in &arm.output_hexes {
            draw_line_3d(
                hex_world_center(arm.hex, ARM_BASE_HEIGHT - 3.0),
                hex_world_center(*output_hex, CELL_HEIGHT + 1.4),
                Color::from_rgba(125, 213, 255, 190),
            );
        }
        if arm.input_mode == MechanicArmInputMode::ExplicitInputs {
            for input_hex in &arm.input_hexes {
                draw_line_3d(
                    hex_world_center(*input_hex, CELL_HEIGHT + 0.8),
                    hex_world_center(arm.hex, ARM_BASE_HEIGHT - 5.0),
                    Color::from_rgba(255, 185, 110, 180),
                );
            }
        }
    }

    for node in &interior.assembly_nodes {
        let ready_total: u32 = node
            .completed_outputs
            .iter()
            .map(|stack| stack.amount)
            .sum();
        if !node.inserted.is_empty() {
            draw_box(
                hex_world_center(node.hex, BLOCK_Y + 19.0),
                vec3(8.0, 5.0, 8.0),
                Color::from_rgba(140, 255, 190, 180),
                Color::from_rgba(200, 255, 220, 220),
                false,
            );
        }
        if ready_total > 0 {
            for offset in 0..ready_total.min(3) {
                draw_box(
                    hex_world_center(node.hex, BLOCK_Y + 8.0 + offset as f32 * 4.8)
                        + vec3(9.0, 0.0, -9.0),
                    vec3(7.0, 4.0, 7.0),
                    Color::from_rgba(255, 226, 126, 255),
                    Color::from_rgba(255, 245, 185, 255),
                    false,
                );
            }
        }
    }

    for forklift in &interior.forklifts {
        let center = forklift_world_center(forklift.hex, forklift.target, forklift.move_progress);
        let forward = forklift_forward_dir(forklift.hex, forklift.target);
        let side = vec3(-forward.z, 0.0, forward.x);
        draw_box(
            center,
            vec3(14.0, 9.0, 12.0),
            Color::from_rgba(120, 214, 255, 255),
            Color::from_rgba(200, 235, 255, 255),
            false,
        );
        draw_box(
            center + forward * -3.0 + vec3(0.0, 7.0, 0.0),
            vec3(4.0, 7.0, 10.0),
            Color::from_rgba(92, 120, 140, 255),
            Color::from_rgba(180, 200, 215, 255),
            false,
        );
        draw_line_3d(
            center + forward * 7.0 + side * -3.0 + vec3(0.0, -2.0, 0.0),
            center + forward * 16.0 + side * -3.0 + vec3(0.0, -2.0, 0.0),
            Color::from_rgba(230, 230, 230, 255),
        );
        draw_line_3d(
            center + forward * 7.0 + side * 3.0 + vec3(0.0, -2.0, 0.0),
            center + forward * 16.0 + side * 3.0 + vec3(0.0, -2.0, 0.0),
            Color::from_rgba(230, 230, 230, 255),
        );
        if let Some(block_kind) = forklift.carried_block {
            draw_box(
                center + forward * 11.0 + vec3(0.0, 7.5, 0.0),
                vec3(10.0, 8.0, 10.0),
                machine_block_color(block_kind),
                colors.text_primary,
                false,
            );
        }
    }

    if let Some((hex, kind, rotation)) = preview {
        let center = hex_world_center(hex, BLOCK_Y);
        draw_machine_block(interior, hex, kind, rotation, center, true);
        draw_hex_tile(
            hex,
            0.04,
            Color::from_rgba(160, 220, 255, 24),
            colors.hover,
            true,
        );
    }

    if let Some(hover_hex) = hover_hex {
        draw_hex_outline(hover_hex, 0.08, colors.hover);
    }

    let border_color = shade(colors.hover, 0.72);
    for r in (camera_center_hex.r - FLOOR_DRAW_RADIUS)..=(camera_center_hex.r + FLOOR_DRAW_RADIUS) {
        for q in
            (camera_center_hex.q - FLOOR_DRAW_RADIUS)..=(camera_center_hex.q + FLOOR_DRAW_RADIUS)
        {
            let hex = Axial { q, r };
            if !in_bounds(hex, MapOutline::Hexagon) {
                continue;
            }
            if axial_distance(hex, camera_center_hex) > FLOOR_DRAW_RADIUS {
                continue;
            }
            if axial_neighbors(hex)
                .iter()
                .any(|neighbor| !in_bounds(*neighbor, MapOutline::Hexagon))
            {
                draw_hex_outline(hex, 0.04, border_color);
            }
        }
    }

    let _ = config;
}

fn hover_summary(state: &PlanetState, hex: Axial) -> Option<String> {
    let interior = state.active_base_interior()?;
    let block = interior.block_at(hex)?;
    let mut lines = vec![format!(
        "{} | rot {}",
        machine_block_label(block.kind),
        block.rotation
    )];

    if let Some(container) = interior.containers.iter().find(|record| record.hex == hex) {
        lines.push(format!("Stored: {}", stack_summary(&container.items)));
    }
    if let Some(arm) = interior
        .mechanic_arms
        .iter()
        .find(|record| record.hex == hex)
    {
        let held = arm
            .held
            .map(|part| part.label().to_string())
            .unwrap_or_else(|| "none".to_string());
        lines.push(format!("Held: {}", held));
        lines.push(format!("Inputs: {}", arm.input_mode.label()));
        if !arm.output_hexes.is_empty() {
            lines.push(format!(
                "Outputs: {}",
                arm.output_hexes
                    .iter()
                    .map(|entry| format!("({}, {})", entry.q, entry.r))
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        }
    }
    if let Some(node) = interior
        .assembly_nodes
        .iter()
        .find(|record| record.hex == hex)
    {
        lines.push(format!("Recipe: {}", node.selected_recipe.label()));
        lines.push(format!("Ready: {}", stack_summary(&node.completed_outputs)));
        if !node.inserted.is_empty() {
            lines.push(format!(
                "Assembly: {}",
                node.inserted
                    .iter()
                    .map(|part| part.label())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ));
        }
    }
    if let Some(forklift) = interior.forklifts.iter().find(|record| record.hex == hex) {
        lines.push(format!("Forklift {}", forklift.id));
    }
    if interior.belt_items.iter().any(|item| item.hex == hex) {
        let items = interior
            .belt_items
            .iter()
            .filter(|item| item.hex == hex)
            .map(|item| item.kind.label())
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(format!("Belt item: {}", items));
    }

    Some(lines.join("\n"))
}

pub fn run(
    ctx: &FrameContext,
    state: &mut PlanetState,
    _cam_offset: &mut Vec2,
    _cam_zoom: &mut f32,
    dragging: &mut bool,
    last_mouse: &mut Vec2,
    selected: &mut Option<MachineBlockType>,
    placement_rotation: &mut u8,
    panel_collapsed: &mut bool,
    window: &mut WindowState,
    confirm_window: &mut WindowState,
    scene: &mut Scene,
    config: &AppConfig,
    colors: &RuntimeColors,
) {
    if state.active_base_interior().is_none() {
        *scene = Scene::Planet;
        return;
    }
    state.tick_active_base_interior(get_frame_time());

    let outline = MapOutline::Hexagon;
    let buttons: [(Option<MachineBlockType>, &str); 8] = [
        (None, "Delete"),
        (Some(MachineBlockType::ConveyorBelt), "Belt"),
        (Some(MachineBlockType::MechanicArm), "Mechanic Arm"),
        (Some(MachineBlockType::Pallet), "Pallet"),
        (Some(MachineBlockType::Crate), "Crate"),
        (Some(MachineBlockType::Chest), "Chest"),
        (Some(MachineBlockType::Assembler), "Assembler"),
        (Some(MachineBlockType::Furnace), "Furnace"),
    ];

    let panel_layout = build_panel_layout(buttons.len(), *panel_collapsed);
    let panel_rect = panel_layout.rect;
    let ui_capturing = is_ui_capturing(panel_rect, window, confirm_window, ctx.mouse);

    if let Some(interior) = state.active_base_interior_mut() {
        update_build_template_camera(
            &mut interior.camera,
            ctx.mouse,
            ui_capturing,
            dragging,
            last_mouse,
        );
    }

    let camera = state
        .active_base_interior()
        .map(|interior| build_template_camera(&interior.camera))
        .unwrap_or_default();
    let hover_hex =
        hover_hex_from_mouse_3d(&camera, ctx.mouse).filter(|hex| in_bounds(*hex, outline));
    let hover_hex_fallback = hover_hex.unwrap_or(Axial { q: 0, r: 0 });
    let hovered_summary = hover_hex.and_then(|hex| hover_summary(state, hex));
    let hover_occupied = hover_hex
        .and_then(|hex| {
            state
                .active_base_interior()
                .and_then(|interior| interior.block_at(hex))
                .map(|_| true)
        })
        .unwrap_or(false);
    let preview = selected.and_then(|kind| {
        hover_hex.and_then(|hex| {
            if hover_occupied {
                None
            } else {
                Some((hex, kind, *placement_rotation))
            }
        })
    });
    let pick_mode = state.interior_pick_mode();

    if is_mouse_button_pressed(MouseButton::Left)
        && !ui_capturing
        && let Some(hover_hex) = hover_hex
        && in_bounds(hover_hex, outline)
    {
        let mut changed = false;
        match pick_mode {
            Some(InteriorPickMode::ArmOutputs(arm_hex)) => {
                if let Some(interior) = state.active_base_interior_mut() {
                    interior.toggle_arm_output_hex(arm_hex, hover_hex);
                    changed = true;
                }
            }
            Some(InteriorPickMode::ArmInputs(arm_hex)) => {
                if let Some(interior) = state.active_base_interior_mut() {
                    interior.toggle_arm_input_hex(arm_hex, hover_hex);
                    changed = true;
                }
            }
            None => {
                if let Some(interior) = state.active_base_interior_mut() {
                    match selected {
                        Some(block_type) => {
                            if interior.block_at(hover_hex).is_none() {
                                interior.upsert_block(
                                    hover_hex,
                                    MachineBlock {
                                        kind: *block_type,
                                        rotation: *placement_rotation,
                                    },
                                );
                                changed = true;
                            }
                        }
                        None => {
                            if interior.block_at(hover_hex).is_some() {
                                confirm_window.title = "Confirm".to_string();
                                confirm_window.open = true;
                                confirm_window.target = Some(hover_hex);
                                confirm_window.dragging = false;
                            }
                        }
                    }
                }
            }
        }
        if changed {
            state.mark_buildings_dirty();
        }
    }

    if is_mouse_button_pressed(MouseButton::Right)
        && !ui_capturing
        && let Some(hover_hex) = hover_hex
    {
        if let Some(interior) = state.active_base_interior() {
            if let Some(block) = interior.block_at(hover_hex) {
                let win_h = match block.kind {
                    MachineBlockType::MechanicArm => 470.0,
                    MachineBlockType::Assembler => 360.0,
                    _ => 220.0,
                };
                let rect = popup_rect_near_mouse(ctx.mouse, 320.0, win_h);
                window.title = machine_block_label(block.kind).to_string();
                window.rect = rect;
                window.open = true;
                window.target = Some(hover_hex);
                window.dragging = false;
            }
        }
    }

    if is_key_pressed(KeyCode::R) {
        let mut changed = false;
        if let Some(interior) = state.active_base_interior_mut() {
            if let Some(hover_hex) = hover_hex {
                if let Some(block) = interior.block_at_mut(hover_hex) {
                    block.rotation = (block.rotation + 1) % 6;
                    changed = true;
                } else {
                    *placement_rotation = (*placement_rotation + 1) % 6;
                }
            } else {
                *placement_rotation = (*placement_rotation + 1) % 6;
            }
        }
        if changed {
            state.mark_buildings_dirty();
        }
    }

    if is_key_pressed(KeyCode::Escape) {
        match state.interior_pick_mode() {
            Some(InteriorPickMode::ArmOutputs(_)) => {
                state.clear_interior_pick_mode();
            }
            Some(InteriorPickMode::ArmInputs(_)) => {
                state.clear_interior_pick_mode();
            }
            None => {
                if selected.is_some() {
                    *selected = None;
                } else {
                    state.clear_base_interior_edit();
                    *scene = Scene::Planet;
                    return;
                }
            }
        }
    }

    if is_key_pressed(KeyCode::Enter) {
        if state.interior_pick_mode().is_some() {
            state.clear_interior_pick_mode();
        }
    }

    let camera_center_hex =
        pixel_to_hex(vec2(camera.target.x, camera.target.z), HEX_SIZE, Vec2::ZERO);
    set_camera(&camera);
    draw_world(
        state.active_base_interior().unwrap(),
        hover_hex,
        preview,
        camera_center_hex,
        config,
        colors,
    );
    set_default_camera();

    let interior_label = state
        .active_base_cell()
        .map(|cell| format!("Base Interior {}", cell))
        .unwrap_or_else(|| "Base Interior".to_string());
    draw_text(
        &interior_label,
        24.0,
        36.0,
        ctx.font_md,
        colors.text_primary,
    );
    draw_text(
        "3D FPS: RMB look | WASD move | Shift sprint | Space/Ctrl up/down",
        24.0,
        60.0,
        ctx.font_sm,
        colors.text_secondary,
    );

    draw_common_hud(ctx, colors, *placement_rotation, hover_hex_fallback);

    if let Some(mode) = state.interior_pick_mode() {
        let message = match mode {
            InteriorPickMode::ArmOutputs(arm_hex) => format!(
                "Select mechanic arm outputs for ({}, {}) | left click neighbor hexes | Enter finish | Esc cancel",
                arm_hex.q, arm_hex.r
            ),
            InteriorPickMode::ArmInputs(arm_hex) => format!(
                "Select mechanic arm inputs for ({}, {}) | left click neighbor hexes | Enter finish | Esc cancel",
                arm_hex.q, arm_hex.r
            ),
        };
        draw_text(&message, 16.0, 98.0, ctx.font_sm, colors.text_primary);
    }

    let panel_result = crate::core::ui::draw_build_panel(
        panel_layout.pos,
        panel_layout.size,
        *panel_collapsed,
        ctx.mouse,
        config.line_thickness,
        colors,
        &buttons,
        *selected,
        machine_block_color,
    );
    let mut tooltip: Option<String> = None;
    if panel_result.toggled {
        *panel_collapsed = !*panel_collapsed;
        if *panel_collapsed {
            *selected = None;
        }
    }
    if let Some(tip) = panel_result.hovered_tip {
        tooltip = Some(tip.to_string());
    }
    if let Some(option) = panel_result.clicked_option {
        *selected = option;
    }

    let hover_tip = hovered_summary.or_else(|| {
        if hover_occupied && selected.is_some() {
            Some("Occupied".to_string())
        } else {
            None
        }
    });
    if tooltip.is_none() {
        tooltip = hover_tip;
    }

    if let Some(tip) = tooltip {
        let pad = 6.0;
        let lines: Vec<&str> = tip.lines().collect();
        let mut max_width: f32 = 0.0;
        let mut total_height: f32 = 0.0;
        for line in &lines {
            let dim = measure_text(line, None, ctx.font_sm as u16, 1.0);
            max_width = max_width.max(dim.width);
            total_height += dim.height.max(ctx.font_sm);
        }
        let x = (ctx.mouse.x + 14.0).min(screen_width() - max_width - 2.0 * pad);
        let y = (ctx.mouse.y + 16.0).min(screen_height() - total_height - 2.0 * pad);
        draw_rectangle(
            x,
            y,
            max_width + 2.0 * pad,
            total_height + 2.0 * pad,
            colors.tooltip_bg,
        );
        draw_rectangle_lines(
            x,
            y,
            max_width + 2.0 * pad,
            total_height + 2.0 * pad,
            config.line_thickness.max(1.0),
            colors.tooltip_border,
        );
        let mut line_y = y + pad + ctx.font_sm - 2.0;
        for line in lines {
            draw_text(line, x + pad, line_y, ctx.font_sm, colors.text_primary);
            line_y += ctx.font_sm;
        }
    }

    if window.open {
        let closed = draw_window_frame(window, ctx, config, colors);
        if !closed {
            if let Some(target) = window.target {
                let block = state
                    .active_base_interior()
                    .and_then(|interior| interior.block_at(target).cloned());
                let container = state.active_base_interior().and_then(|interior| {
                    interior
                        .containers
                        .iter()
                        .find(|record| record.hex == target)
                        .cloned()
                });
                let arm_state = state.active_base_interior().and_then(|interior| {
                    interior
                        .mechanic_arms
                        .iter()
                        .find(|record| record.hex == target)
                        .cloned()
                });
                let node_state = state.active_base_interior().and_then(|interior| {
                    interior
                        .assembly_nodes
                        .iter()
                        .find(|record| record.hex == target)
                        .cloned()
                });
                if let Some(block) = block {
                    let content_x = window.rect.x + 10.0;
                    let mut content_y = window.rect.y + WINDOW_TITLE_HEIGHT + 20.0;
                    draw_text(
                        &format!("Type: {}", machine_block_label(block.kind)),
                        content_x,
                        content_y,
                        ctx.font_sm,
                        colors.text_secondary,
                    );
                    content_y += 24.0;
                    draw_text(
                        &format!("Rotation: {}", block.rotation),
                        content_x,
                        content_y,
                        ctx.font_sm,
                        colors.text_secondary,
                    );
                    content_y += 24.0;

                    if let Some(container) = container.as_ref() {
                        draw_text(
                            &format!("Stored: {}", stack_summary(&container.items)),
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 24.0;
                    }
                    if let Some(arm) = arm_state.as_ref() {
                        let held = arm
                            .held
                            .map(|part| part.label().to_string())
                            .unwrap_or_else(|| "none".to_string());
                        draw_text(
                            &format!("Held in air: {}", held),
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 24.0;
                        draw_text(
                            &format!("Input mode: {}", arm.input_mode.label()),
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 20.0;
                        draw_text(
                            &format!(
                                "Outputs: {}",
                                if arm.output_hexes.is_empty() {
                                    "(none)".to_string()
                                } else {
                                    arm.output_hexes
                                        .iter()
                                        .map(|hex| format!("({}, {})", hex.q, hex.r))
                                        .collect::<Vec<_>>()
                                        .join(" | ")
                                }
                            ),
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 20.0;
                        if arm.input_mode == MechanicArmInputMode::ExplicitInputs {
                            draw_text(
                                &format!(
                                    "Inputs: {}",
                                    if arm.input_hexes.is_empty() {
                                        "(none)".to_string()
                                    } else {
                                        arm.input_hexes
                                            .iter()
                                            .map(|hex| format!("({}, {})", hex.q, hex.r))
                                            .collect::<Vec<_>>()
                                            .join(" | ")
                                    }
                                ),
                                content_x,
                                content_y,
                                ctx.font_sm,
                                colors.text_secondary,
                            );
                            content_y += 20.0;
                        }
                        draw_text(
                            &format!("Filters: {}", selected_filter_summary(&arm.filters)),
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 24.0;
                    }
                    if let Some(node) = node_state.as_ref() {
                        draw_text(
                            &format!("Recipe: {}", node.selected_recipe.label()),
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 24.0;
                        let inserted = if node.inserted.is_empty() {
                            "empty".to_string()
                        } else {
                            node.inserted
                                .iter()
                                .map(|part| part.label())
                                .collect::<Vec<_>>()
                                .join(" -> ")
                        };
                        draw_text(
                            &format!("Assembly: {}", inserted),
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 24.0;
                        draw_text(
                            &format!("Ready outputs: {}", stack_summary(&node.completed_outputs)),
                            content_x,
                            content_y,
                            ctx.font_sm,
                            colors.text_secondary,
                        );
                        content_y += 24.0;
                    }

                    if block.kind == MachineBlockType::MechanicArm {
                        let rect_outputs = Rect::new(content_x, content_y, 120.0, 24.0);
                        let rect_inputs = Rect::new(content_x + 130.0, content_y, 120.0, 24.0);
                        let (clicked_outputs, _) = ui_button(
                            rect_outputs,
                            "Pick Outputs",
                            ctx.mouse,
                            ctx.font_sm,
                            ctx.button_colors,
                        );
                        let (clicked_inputs, _) = ui_button(
                            rect_inputs,
                            "Pick Inputs",
                            ctx.mouse,
                            ctx.font_sm,
                            ctx.button_colors,
                        );
                        if clicked_outputs {
                            state.begin_arm_output_pick(target);
                        }
                        if clicked_inputs {
                            state.begin_arm_input_pick(target);
                            if let Some(interior_mut) = state.active_base_interior_mut() {
                                interior_mut.set_arm_input_mode(
                                    target,
                                    MechanicArmInputMode::ExplicitInputs,
                                );
                            }
                            state.mark_buildings_dirty();
                        }
                        content_y += 30.0;

                        let rect_any = Rect::new(content_x, content_y, 160.0, 24.0);
                        let label = if let Some(arm) = arm_state.as_ref() {
                            if arm.input_mode == MechanicArmInputMode::AnyNeighbor {
                                "Use explicit inputs"
                            } else {
                                "Use any neighbor"
                            }
                        } else {
                            "Use any neighbor"
                        };
                        let (clicked_mode, _) =
                            ui_button(rect_any, label, ctx.mouse, ctx.font_sm, ctx.button_colors);
                        if clicked_mode {
                            let next_mode = if let Some(arm) = arm_state.as_ref() {
                                if arm.input_mode == MechanicArmInputMode::AnyNeighbor {
                                    MechanicArmInputMode::ExplicitInputs
                                } else {
                                    MechanicArmInputMode::AnyNeighbor
                                }
                            } else {
                                MechanicArmInputMode::AnyNeighbor
                            };
                            if let Some(interior_mut) = state.active_base_interior_mut() {
                                interior_mut.set_arm_input_mode(target, next_mode);
                            }
                            state.mark_buildings_dirty();
                        }
                        content_y += 30.0;

                        for filter_kind in all_interior_filter_parts() {
                            let rect_filter = Rect::new(content_x, content_y, 170.0, 22.0);
                            let label = format!("Filter {}", filter_kind.label());
                            let (clicked_filter, _) = ui_button(
                                rect_filter,
                                &label,
                                ctx.mouse,
                                ctx.font_sm,
                                ctx.button_colors,
                            );
                            if clicked_filter {
                                if let Some(interior_mut) = state.active_base_interior_mut() {
                                    interior_mut.toggle_arm_filter(target, *filter_kind);
                                }
                                state.mark_buildings_dirty();
                            }
                            content_y += 24.0;
                        }
                    }

                    if block.kind == MachineBlockType::Assembler {
                        for recipe in assembly_recipe_ids() {
                            let rect_recipe = Rect::new(content_x, content_y, 180.0, 24.0);
                            let suffix = match recipe {
                                AssemblerRecipeId::BuilderTender => "",
                                _ => " (planned)",
                            };
                            let label = format!("Build {}{}", recipe.label(), suffix);
                            let (clicked_recipe, _) = ui_button(
                                rect_recipe,
                                &label,
                                ctx.mouse,
                                ctx.font_sm,
                                ctx.button_colors,
                            );
                            if clicked_recipe {
                                let mut changed = false;
                                if let Some(interior_mut) = state.active_base_interior_mut() {
                                    changed = interior_mut.set_assembler_recipe(target, *recipe);
                                }
                                if changed {
                                    state.mark_buildings_dirty();
                                }
                            }
                            content_y += 26.0;
                        }

                        if let Some(node) = node_state.as_ref() {
                            let next_recipe = assembly_recipe_parts(node.selected_recipe)
                                .iter()
                                .map(|part| part.label())
                                .collect::<Vec<_>>()
                                .join(" -> ");
                            draw_text(
                                &format!("Recipe parts: {}", next_recipe),
                                content_x,
                                content_y,
                                ctx.font_sm,
                                colors.text_secondary,
                            );
                            content_y += 24.0;
                        }
                    }

                    let rect_close = Rect::new(content_x, content_y, 90.0, 26.0);
                    let (clicked_close, _) = ui_button(
                        rect_close,
                        "Close",
                        ctx.mouse,
                        ctx.font_sm,
                        ctx.button_colors,
                    );
                    if clicked_close {
                        window.open = false;
                        window.target = None;
                    }
                }
            }
        }
    }

    let confirm_target = confirm_window.target;
    let confirm_label = confirm_label_for_target(confirm_target, |target| {
        state
            .active_base_interior()
            .and_then(|interior| interior.block_at(target))
            .map(|block| machine_block_label(block.kind).to_string())
    });
    if run_confirm_window(confirm_window, ctx, config, colors, &confirm_label)
        == ConfirmAction::Confirm
    {
        if let Some(target) = confirm_target {
            let mut changed = false;
            if let Some(interior) = state.active_base_interior_mut() {
                interior.remove_block(target);
                changed = true;
            }
            if changed {
                state.mark_buildings_dirty();
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
                    (relative_to_neighbor.dot(connected_side) - facing_side * BELT_HALF_WIDTH)
                        .abs()
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
}
