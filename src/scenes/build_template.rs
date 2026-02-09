use std::collections::HashMap;
use macroquad::prelude::*;

use crate::core::{
    hex_to_pixel, AppConfig, Axial, FrameContext, RuntimeColors, Scene,
    MachineBlock, MachineBlockType, draw_hex_filled, draw_hex_outline,
};
use crate::core::ui::{ui_button, WindowState, WINDOW_TITLE_HEIGHT};
use crate::scenes::map_common::{
    build_panel_layout,
    confirm_label_for_target,
    draw_window_frame,
    is_ui_capturing,
    popup_rect_near_mouse,
    run_confirm_window,
    ConfirmAction,
    MapOutline,
    draw_common_hud,
    handle_camera_drag,
    handle_cursor_zoom,
    hex_screen_center,
    hover_hex_from_mouse,
    in_bounds,
};
use crate::{HEX_SIZE, HEX_RADIUS};

// Helper function to get friendly label for machine block type
fn machine_block_label(kind: MachineBlockType) -> &'static str {
    match kind {
        MachineBlockType::ConveyorBelt => "Conveyor Belt",
        MachineBlockType::Inserter => "Inserter",
        MachineBlockType::Chest => "Chest",
        MachineBlockType::Assembler => "Assembler",
        MachineBlockType::Furnace => "Furnace",
    }
}

// Helper function to get color for machine block type
fn machine_block_color(kind: MachineBlockType) -> Color {
    match kind {
        MachineBlockType::ConveyorBelt => Color::new(1.0, 1.0, 0.0, 1.0),
        MachineBlockType::Inserter => Color::new(1.0, 0.5, 0.0, 1.0),
        MachineBlockType::Chest => Color::new(0.5, 0.5, 0.5, 1.0),
        MachineBlockType::Assembler => Color::new(0.0, 1.0, 1.0, 1.0),
        MachineBlockType::Furnace => Color::new(1.0, 0.0, 0.0, 1.0),
    }
}

// Render and handle input for the build template scene.
pub fn run(
    ctx: &FrameContext,
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
    blocks: &mut HashMap<Axial, MachineBlock>,
) {
    let outline = MapOutline::Hexagon;

    handle_cursor_zoom(ctx, cam_offset, cam_zoom, config.zoom_speed);
    handle_camera_drag(ctx, cam_offset, dragging, last_mouse);

    let hover_hex = hover_hex_from_mouse(ctx, *cam_offset, *cam_zoom);

    // Template block types for the panel
    let buttons: [(Option<MachineBlockType>, &str); 6] = [
        (None, "Delete"),
        (Some(MachineBlockType::ConveyorBelt), "Belt"),
        (Some(MachineBlockType::Inserter), "Inserter"),
        (Some(MachineBlockType::Chest), "Chest"),
        (Some(MachineBlockType::Assembler), "Assembler"),
        (Some(MachineBlockType::Furnace), "Furnace"),
    ];

    let panel_layout = build_panel_layout(buttons.len(), *panel_collapsed);
    let panel_pos = panel_layout.pos;
    let panel_size = panel_layout.size;
    let panel_rect = panel_layout.rect;

    let mut tooltip: Option<&str> = None;
    let ui_capturing = is_ui_capturing(panel_rect, window, confirm_window, ctx.mouse);

    // Handle block placement/deletion
    if is_mouse_button_pressed(MouseButton::Left) && !ui_capturing {
        if in_bounds(hover_hex, outline) {
            match selected {
                Some(block_type) => {
                    if !blocks.contains_key(&hover_hex) {
                        blocks.insert(hover_hex, MachineBlock {
                            kind: *block_type,
                            rotation: *placement_rotation,
                        });
                    }
                }
                None => {
                    blocks.remove(&hover_hex);
                }
            }
        }
    }

    // Click on existing block to open window
    if is_mouse_button_pressed(MouseButton::Right) && !ui_capturing {
        if let Some(block) = blocks.get(&hover_hex) {
            let win_w = 220.0;
            let win_h = 120.0;
            let rect = popup_rect_near_mouse(ctx.mouse, win_w, win_h);
            window.title = machine_block_label(block.kind).to_string();
            window.rect = rect;
            window.open = true;
            window.target = Some(hover_hex);
            window.dragging = false;
        }
    }

    // Handle rotation
    if is_key_pressed(KeyCode::R) {
        if let Some(block) = blocks.get_mut(&hover_hex) {
            block.rotation = (block.rotation + 1) % 6;
        } else {
            *placement_rotation = (*placement_rotation + 1) % 6;
        }
    }

    // ESC to return to menu
    if is_key_pressed(KeyCode::Escape) {
        *scene = Scene::MainMenu;
    }

    // Draw hexagonal grid and blocks
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

            // Draw placed block
            if let Some(block) = blocks.get(&hex) {
                draw_hex_filled(center, (HEX_SIZE - 2.5) * *cam_zoom, machine_block_color(block.kind));
            }
            if *cam_zoom > 1.2 {
                draw_hex_outline(center, size, colors.grid, config.line_thickness);
            }
        }
    }

    // Draw ghost preview for placement
    if let Some(kind) = *selected {
        if in_bounds(hover_hex, outline) && !blocks.contains_key(&hover_hex) {
            let hover_center = hex_screen_center(ctx, *cam_offset, *cam_zoom, hover_hex);
            let mut ghost = machine_block_color(kind);
            ghost.a = 0.3;
            draw_hex_filled(
                hover_center,
                (HEX_SIZE - 2.5) * *cam_zoom,
                ghost,
            );
        }
    }

    // Draw hover highlight
    if in_bounds(hover_hex, outline) {
        let hover_center = hex_screen_center(ctx, *cam_offset, *cam_zoom, hover_hex);
        draw_hex_outline(
            hover_center,
            HEX_SIZE * *cam_zoom,
            colors.hover,
            config.line_thickness * 2.0,
        );
    }

    // Draw info text
    draw_common_hud(ctx, colors, *placement_rotation, hover_hex);

    let panel_result = crate::core::ui::draw_build_panel(
        panel_pos,
        panel_size,
        *panel_collapsed,
        ctx.mouse,
        config.line_thickness,
        colors,
        &buttons,
        *selected,
        |kind| machine_block_color(kind),
    );
    if panel_result.toggled {
        *panel_collapsed = !*panel_collapsed;
    }
    if let Some(tip) = panel_result.hovered_tip {
        tooltip = Some(tip);
    }
    if let Some(option) = panel_result.clicked_option {
        *selected = option;
    }

    if let Some(tip) = tooltip {
        let pad = 6.0;
        let font_size = ctx.font_sm;
        let dim = measure_text(tip, None, font_size as u16, 1.0);
        let x = (ctx.mouse.x + 14.0).min(screen_width() - dim.width - 2.0 * pad);
        let y = (ctx.mouse.y + 16.0).min(screen_height() - dim.height - 2.0 * pad);
        draw_rectangle(
            x,
            y,
            dim.width + 2.0 * pad,
            dim.height + 2.0 * pad,
            colors.tooltip_bg,
        );
        draw_rectangle_lines(
            x,
            y,
            dim.width + 2.0 * pad,
            dim.height + 2.0 * pad,
            config.line_thickness.max(1.0),
            colors.tooltip_border,
        );
        draw_text(
            tip,
            x + pad,
            y + dim.height + pad - 2.0,
            font_size,
            colors.text_primary,
        );
    }

    // Handle window interactions
    if window.open {
        let closed = draw_window_frame(window, ctx, config, colors);
        if !closed {
            if let Some(target) = window.target {
                if let Some(block) = blocks.get(&target) {
                    let content_x = window.rect.x + 10.0;
                    let mut content_y = window.rect.y + WINDOW_TITLE_HEIGHT + 20.0;

                    draw_text(
                        &format!("Tipo: {}", machine_block_label(block.kind)),
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
                    content_y += 28.0;

                    let rect_close = Rect::new(content_x, content_y, 90.0, 26.0);
                    let (clicked_close, _) =
                        ui_button(rect_close, "Cerrar", ctx.mouse, ctx.font_sm, ctx.button_colors);
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
        blocks
            .get(&target)
            .map(|block| machine_block_label(block.kind).to_string())
    });
    if run_confirm_window(confirm_window, ctx, config, colors, &confirm_label)
        == ConfirmAction::Confirm
    {
        if let Some(target) = confirm_target {
            if blocks.remove(&target).is_some() {
                // Block deleted
            }
        }
    }
}


