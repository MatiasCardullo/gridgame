use macroquad::prelude::*;

use crate::core::base_interior::{
    AssemblerRecipeId, InteriorPartKind, InteriorPartStack, MechanicArmActionStage,
    MechanicArmInputMode, all_interior_filter_parts, assembly_recipe_ids, assembly_recipe_parts,
    machine_block_label,
};
use crate::core::map_common::{
    ConfirmAction, MapOutline, build_panel_layout, confirm_label_for_target, draw_common_hud,
    draw_window_frame, handle_camera_drag, handle_cursor_zoom, hex_screen_center,
    hover_hex_from_mouse, in_bounds, is_ui_capturing, popup_rect_near_mouse, run_confirm_window,
};
use crate::core::ui::{WINDOW_TITLE_HEIGHT, WindowState, ui_button};
use crate::core::{
    AppConfig, Axial, FrameContext, MachineBlock, MachineBlockType, RuntimeColors, Scene,
    draw_hex_filled, draw_hex_outline, hex_to_pixel,
};
use crate::scenes::planet::{InteriorPickMode, PlanetState};
use crate::{HEX_RADIUS, HEX_SIZE};

fn machine_block_color(kind: MachineBlockType) -> Color {
    match kind {
        MachineBlockType::ConveyorBelt => Color::from_rgba(232, 206, 92, 255),
        MachineBlockType::MechanicArm => Color::from_rgba(239, 137, 74, 255),
        MachineBlockType::Chest => Color::from_rgba(150, 150, 150, 255),
        MachineBlockType::Assembler => Color::from_rgba(78, 207, 211, 255),
        MachineBlockType::Furnace => Color::from_rgba(214, 88, 88, 255),
        MachineBlockType::Pallet => Color::from_rgba(151, 108, 66, 255),
        MachineBlockType::Crate => Color::from_rgba(119, 84, 54, 255),
    }
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

fn draw_belt_chevrons(center: Vec2, rotation: u8, zoom: f32, color: Color) {
    let angle = rotation as f32 * std::f32::consts::PI / 3.0;
    let forward = vec2(angle.cos(), angle.sin());
    let side = vec2(-forward.y, forward.x);
    for offset in [-5.0_f32, 3.0] {
        let mid = center + forward * offset * zoom;
        let a = mid - forward * 4.0 * zoom + side * 3.0 * zoom;
        let b = mid;
        let c = mid - forward * 4.0 * zoom - side * 3.0 * zoom;
        draw_line(a.x, a.y, b.x, b.y, 1.8, color);
        draw_line(c.x, c.y, b.x, b.y, 1.8, color);
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

fn arm_hand_center(
    ctx: &FrameContext,
    cam_offset: Vec2,
    cam_zoom: f32,
    arm_hex: Axial,
    target: Option<Axial>,
    progress: f32,
) -> Vec2 {
    let center = hex_screen_center(ctx, cam_offset, cam_zoom, arm_hex);
    let Some(target_hex) = target else {
        return center;
    };
    let target_center = hex_screen_center(ctx, cam_offset, cam_zoom, target_hex);
    let reach = if progress < 0.5 {
        progress * 2.0
    } else {
        (1.0 - progress) * 2.0
    };
    center.lerp(target_center, reach.clamp(0.0, 1.0))
}

fn forklift_screen_center(
    ctx: &FrameContext,
    cam_offset: Vec2,
    cam_zoom: f32,
    from: Axial,
    to: Option<Axial>,
    progress: f32,
) -> Vec2 {
    let start = hex_screen_center(ctx, cam_offset, cam_zoom, from);
    let Some(target_hex) = to else {
        return start;
    };
    let end = hex_screen_center(ctx, cam_offset, cam_zoom, target_hex);
    start.lerp(end, progress.clamp(0.0, 1.0))
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
                    .map(|hex| format!("({}, {})", hex.q, hex.r))
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
    cam_offset: &mut Vec2,
    cam_zoom: &mut f32,
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
    handle_cursor_zoom(ctx, cam_offset, cam_zoom, config.zoom_speed);
    handle_camera_drag(ctx, cam_offset, dragging, last_mouse);

    let hover_hex = hover_hex_from_mouse(ctx, *cam_offset, *cam_zoom);
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
    let mut tooltip: Option<String> = None;
    let hovered_summary = hover_summary(state, hover_hex);
    let hover_occupied = state
        .active_base_interior()
        .and_then(|interior| interior.block_at(hover_hex))
        .is_some();
    let pick_mode = state.interior_pick_mode();

    if is_mouse_button_pressed(MouseButton::Left) && !ui_capturing && in_bounds(hover_hex, outline)
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
                                if *block_type == MachineBlockType::MechanicArm {
                                    state.begin_arm_output_pick(hover_hex);
                                }
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

    if is_mouse_button_pressed(MouseButton::Right) && !ui_capturing {
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
            if let Some(block) = interior.block_at_mut(hover_hex) {
                block.rotation = (block.rotation + 1) % 6;
                changed = true;
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
            Some(InteriorPickMode::ArmOutputs(arm_hex)) => {
                let mut remove_arm = false;
                if let Some(interior) = state.active_base_interior() {
                    remove_arm = interior
                        .mechanic_arm(arm_hex)
                        .map(|arm| arm.output_hexes.is_empty())
                        .unwrap_or(false);
                }
                if remove_arm {
                    if let Some(interior) = state.active_base_interior_mut() {
                        interior.remove_block(arm_hex);
                        state.mark_buildings_dirty();
                    }
                }
                state.clear_interior_pick_mode();
            }
            Some(InteriorPickMode::ArmInputs(_)) => {
                state.clear_interior_pick_mode();
            }
            None => {
                state.clear_base_interior_edit();
                *scene = Scene::Planet;
                return;
            }
        }
    }

    if is_key_pressed(KeyCode::Enter) {
        if let Some(InteriorPickMode::ArmOutputs(arm_hex)) = state.interior_pick_mode() {
            let can_finish = state
                .active_base_interior()
                .and_then(|interior| interior.mechanic_arm(arm_hex))
                .map(|arm| !arm.output_hexes.is_empty())
                .unwrap_or(false);
            if can_finish {
                state.clear_interior_pick_mode();
            }
        } else if matches!(
            state.interior_pick_mode(),
            Some(InteriorPickMode::ArmInputs(_))
        ) {
            state.clear_interior_pick_mode();
        }
    }

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

    if let Some(interior) = state.active_base_interior() {
        for r in -HEX_RADIUS..=HEX_RADIUS {
            for q in -HEX_RADIUS..=HEX_RADIUS {
                let hex = Axial { q, r };
                if !in_bounds(hex, outline) {
                    continue;
                }
                let world_center = hex_to_pixel(hex, HEX_SIZE, Vec2::ZERO);
                let center = ctx.screen_center + *cam_offset + world_center * *cam_zoom;
                let size = HEX_SIZE * *cam_zoom;
                if center.x < -size
                    || center.y < -size
                    || center.x > screen_width() + size
                    || center.y > screen_height() + size
                {
                    continue;
                }

                if let Some(block) = interior.block_at(hex) {
                    draw_hex_filled(
                        center,
                        (HEX_SIZE - 2.5) * *cam_zoom,
                        machine_block_color(block.kind),
                    );
                    if block.kind == MachineBlockType::ConveyorBelt {
                        draw_belt_chevrons(center, block.rotation, *cam_zoom, colors.text_primary);
                    }
                }
                if *cam_zoom > 1.1 {
                    draw_hex_outline(center, size, colors.grid, config.line_thickness);
                }
            }
        }
        for belt_item in &interior.belt_items {
            let center = hex_screen_center(ctx, *cam_offset, *cam_zoom, belt_item.hex);
            draw_circle(
                center.x,
                center.y,
                4.0 * *cam_zoom,
                Color::from_rgba(255, 250, 220, 255),
            );
        }

        for arm in &interior.mechanic_arms {
            let center = hex_screen_center(ctx, *cam_offset, *cam_zoom, arm.hex);
            if arm.action_stage != MechanicArmActionStage::Idle {
                let hand_center = arm_hand_center(
                    ctx,
                    *cam_offset,
                    *cam_zoom,
                    arm.hex,
                    arm.action_target,
                    arm.action_progress,
                );
                draw_line(
                    center.x,
                    center.y,
                    hand_center.x,
                    hand_center.y,
                    (config.line_thickness * 2.2).max(2.0),
                    colors.hover,
                );
                draw_circle(hand_center.x, hand_center.y, 4.0 * *cam_zoom, colors.hover);
                let show_item = match arm.action_stage {
                    MechanicArmActionStage::Pickup => arm.action_progress >= 0.5,
                    MechanicArmActionStage::Deliver => true,
                    MechanicArmActionStage::Idle => arm.held.is_some(),
                };
                if show_item {
                    if let Some(kind) = arm.held {
                        draw_circle(
                            hand_center.x,
                            hand_center.y - 6.0 * *cam_zoom,
                            3.4 * *cam_zoom,
                            part_draw_color(kind),
                        );
                    }
                }
            } else if let Some(kind) = arm.held {
                draw_circle_lines(center.x, center.y, 7.0 * *cam_zoom, 2.0, colors.hover);
                draw_circle(
                    center.x,
                    center.y - 6.0 * *cam_zoom,
                    3.4 * *cam_zoom,
                    part_draw_color(kind),
                );
            }
        }

        for node in &interior.assembly_nodes {
            let center = hex_screen_center(ctx, *cam_offset, *cam_zoom, node.hex);
            if !node.inserted.is_empty() {
                draw_circle(
                    center.x,
                    center.y,
                    6.0 * *cam_zoom,
                    Color::from_rgba(140, 255, 190, 180),
                );
            }
            let ready_total: u32 = node
                .completed_outputs
                .iter()
                .map(|stack| stack.amount)
                .sum();
            if ready_total > 0 {
                draw_text(
                    &format!("K{}", ready_total),
                    center.x - 8.0,
                    center.y - 10.0,
                    ctx.font_sm,
                    colors.text_primary,
                );
            }
        }

        for forklift in &interior.forklifts {
            let center = forklift_screen_center(
                ctx,
                *cam_offset,
                *cam_zoom,
                forklift.hex,
                forklift.target,
                forklift.move_progress,
            );
            if let Some(block_kind) = forklift.carried_block {
                let mut cargo_color = machine_block_color(block_kind);
                cargo_color.a = 220.0 / 255.0;
                draw_hex_filled(
                    center + vec2(0.0, -11.0 * *cam_zoom),
                    (HEX_SIZE * 0.46) * *cam_zoom,
                    cargo_color,
                );
                draw_hex_outline(
                    center + vec2(0.0, -11.0 * *cam_zoom),
                    (HEX_SIZE * 0.46) * *cam_zoom,
                    colors.text_primary,
                    config.line_thickness.max(1.0),
                );
            }
            draw_rectangle(
                center.x - 7.0 * *cam_zoom,
                center.y - 4.0 * *cam_zoom,
                14.0 * *cam_zoom,
                8.0 * *cam_zoom,
                Color::from_rgba(120, 214, 255, 255),
            );
            draw_text(
                &format!("{}", forklift.id),
                center.x - 4.0,
                center.y - 8.0,
                ctx.font_sm,
                colors.text_primary,
            );
        }
    }

    if let Some(kind) = *selected {
        if let Some(interior) = state.active_base_interior() {
            if in_bounds(hover_hex, outline) {
                let hover_center = hex_screen_center(ctx, *cam_offset, *cam_zoom, hover_hex);
                let mut ghost = machine_block_color(kind);
                ghost.a = if interior.block_at(hover_hex).is_none() {
                    0.3
                } else {
                    0.15
                };
                draw_hex_filled(hover_center, (HEX_SIZE - 2.5) * *cam_zoom, ghost);
                if interior.block_at(hover_hex).is_some() {
                    draw_circle_lines(
                        hover_center.x,
                        hover_center.y,
                        9.0 * *cam_zoom,
                        2.0,
                        Color::from_rgba(255, 110, 110, 255),
                    );
                }
            }
        }
    }

    if in_bounds(hover_hex, outline) {
        let hover_center = hex_screen_center(ctx, *cam_offset, *cam_zoom, hover_hex);
        draw_hex_outline(
            hover_center,
            HEX_SIZE * *cam_zoom,
            colors.hover,
            config.line_thickness * 2.0,
        );
    }

    draw_common_hud(ctx, colors, *placement_rotation, hover_hex);

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
    if panel_result.toggled {
        *panel_collapsed = !*panel_collapsed;
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
