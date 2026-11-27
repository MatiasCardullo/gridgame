use super::*;
use crate::core::ui::{
    UiButtonColors, draw_editor_action_button, draw_editor_list_row, draw_panel_frame,
    fit_status_text, ui_button,
};

pub(super) fn button(ctx: &FrameContext, rect: Rect, label: &str) -> bool {
    ui_button(rect, label, ctx.mouse, ctx.font_sm, ctx.button_colors).0
}

pub(super) fn list_row_clicked(rect: Rect, mouse: Vec2) -> bool {
    rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Left)
}

pub(super) fn draw_list_row(
    ctx: &FrameContext,
    rect: Rect,
    label: &str,
    selected: bool,
    selected_color: Color,
) {
    let _ = draw_editor_list_row(
        rect,
        label,
        ctx.mouse,
        ctx.font_sm,
        selected,
        selected_color,
        UiButtonColors {
            base: ctx.colors_rt.button_base,
            hover: ctx.colors_rt.button_hover,
            border: ctx.colors_rt.button_border,
            text: WHITE,
        },
    );
}

pub(super) fn draw_group_action_button(ctx: &FrameContext, rect: Rect, label: &str, active: bool) {
    let _ = draw_editor_action_button(
        rect,
        label,
        ctx.mouse,
        ctx.font_sm,
        active,
        Color::from_rgba(27, 119, 105, 255),
        UiButtonColors {
            base: ctx.colors_rt.button_base,
            hover: ctx.colors_rt.button_hover,
            border: ctx.colors_rt.button_border,
            text: WHITE,
        },
    );
}

pub(super) fn numeric_transform_panel_rect() -> Rect {
    Rect::new(SIDEBAR_WIDTH + 12.0, TOOLBAR_HEIGHT + 48.0, 260.0, 108.0)
}

pub(super) fn draw_numeric_transform_panel(
    ctx: &FrameContext,
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
) {
    if !state.numeric_transform_open {
        return;
    }
    let panel = numeric_transform_panel_rect();
    draw_panel_frame(
        panel,
        1.0,
        ctx.colors_rt.panel_bg,
        ctx.colors_rt.panel_border,
    );
    let mode_label = match state.transform_mode {
        TransformMode::Move => "Move by X, Y, Z",
        TransformMode::Rotate => "Rotate by degrees X, Y, Z",
        TransformMode::Scale => "Scale by X, Y, Z",
    };
    draw_text(
        mode_label,
        panel.x + 10.0,
        panel.y + 21.0,
        ctx.font_sm,
        ctx.colors_rt.text_primary,
    );
    let input = Rect::new(panel.x + 10.0, panel.y + 28.0, panel.w - 20.0, 28.0);
    draw_rectangle(
        input.x,
        input.y,
        input.w,
        input.h,
        ctx.colors_rt.button_base,
    );
    draw_rectangle_lines(
        input.x,
        input.y,
        input.w,
        input.h,
        1.0,
        if state.numeric_transform_focused {
            Color::from_rgba(27, 119, 105, 255)
        } else {
            ctx.colors_rt.button_border
        },
    );
    if state.numeric_transform_selected {
        draw_rectangle(
            input.x + 6.0,
            input.y + 5.0,
            (state.numeric_transform_values.chars().count() as f32 * 9.0).min(input.w - 12.0),
            input.h - 10.0,
            Color::from_rgba(27, 119, 105, 90),
        );
    }
    draw_text(
        &state.numeric_transform_values,
        input.x + 9.0,
        input.y + 20.0,
        ctx.font_sm,
        ctx.colors_rt.text_primary,
    );
    if input.contains(ctx.mouse) && is_mouse_button_pressed(MouseButton::Left) {
        state.numeric_transform_focused = true;
        state.numeric_transform_selected = true;
        state.error = None;
    }
    let button_y = panel.y + 66.0;
    let apply_clicked = button(
        ctx,
        Rect::new(panel.x + 10.0, button_y, 110.0, 30.0),
        "Apply",
    );
    let close_clicked = button(
        ctx,
        Rect::new(panel.x + 140.0, button_y, 110.0, 30.0),
        "Close",
    );
    if apply_clicked {
        match apply_numeric_transform(catalog, state) {
            Ok(()) => {
                state.error = None;
            }
            Err(error) => state.error = Some(error),
        }
    } else if close_clicked {
        state.numeric_transform_open = false;
        state.numeric_transform_focused = false;
        state.numeric_transform_selected = false;
        state.error = None;
    }
}

pub(super) fn draw_object_snap_panel(ctx: &FrameContext, panel: Rect, state: &mut CadSceneState) {
    if !state.snap_settings_open {
        return;
    }
    let popup = object_snap_panel_rect(panel);
    draw_panel_frame(
        popup,
        1.0,
        ctx.colors_rt.panel_bg,
        ctx.colors_rt.panel_border,
    );
    draw_text(
        "Object snap modes",
        popup.x + 8.0,
        popup.y + 19.0,
        ctx.font_sm,
        ctx.colors_rt.text_primary,
    );
    for (index, (kind, label)) in [
        (ObjectSnapKind::Endpoint, "End"),
        (ObjectSnapKind::Midpoint, "Mid"),
        (ObjectSnapKind::Center, "Center"),
        (ObjectSnapKind::Intersection, "Intersect"),
        (ObjectSnapKind::Quadrant, "Quad"),
        (ObjectSnapKind::Perpendicular, "Perp"),
        (ObjectSnapKind::Tangent, "Tangent"),
    ]
    .into_iter()
    .enumerate()
    {
        let rect = object_snap_button_rect(panel, index);
        let enabled = state.object_snaps.enabled(kind);
        if button(ctx, rect, label) {
            state.object_snaps.set(kind, !enabled);
        }
        if enabled {
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                2.0,
                Color::from_rgba(27, 119, 105, 255),
            );
        }
    }
    for (index, label, enabled) in [
        (0, "Polar", state.polar_tracking),
        (1, "Grid Snap", state.grid_snap),
    ] {
        let rect = object_snap_tracking_button_rect(panel, index);
        if button(ctx, rect, label) {
            if index == 0 {
                state.polar_tracking = !state.polar_tracking;
            } else {
                state.grid_snap = !state.grid_snap;
            }
        }
        if enabled {
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                2.0,
                Color::from_rgba(27, 119, 105, 255),
            );
        }
    }
}

pub(super) fn status_bar_message(
    state: &CadSceneState,
    asset_error: Option<&str>,
) -> (String, bool) {
    if let Some(error) = &state.error {
        return (error.clone(), true);
    }
    if let Some(error) = asset_error {
        return (error.to_string(), true);
    }
    let message = if state.placing_radial_center {
        "Click the grid to place the UCS origin; Esc cancels."
    } else if let Some(placement) = &state.primitive_placement {
        if placement.center.is_some() {
            "Primitive placement: click the viewport to set its size; use Create to finish."
        } else {
            "Click the grid to place the primitive center; Esc cancels."
        }
    } else if state.trim_active && state.trim_phase == TrimPhase::Targets {
        "Trim 1/2: select target faces; Shift toggles; Enter for cutters."
    } else if state.trim_active && state.trim_path_active {
        "Trim path: click the target face; edge and object snaps are active; Enter finishes."
    } else if state.trim_active && !state.trim_path_points.is_empty() {
        "Trim 2/2: path ready; Enter to split or Draw Path to replace it."
    } else if state.trim_active {
        "Trim 2/2: select cutter faces or Draw Path; Enter to split."
    } else if state.mirror_active {
        "Mirror: click three plane points; Backspace removes the last; Enter applies."
    } else if state.fillet_active {
        "Fillet: select mesh edges, then set the radius and apply."
    } else if state.offset_active {
        "Offset: select faces, then set the distance and apply."
    } else if state.numeric_transform_open {
        "Enter X, Y, Z values; Enter applies the transform; Esc closes."
    } else if state.gizmo_drag.is_some() {
        "Drag to transform; hold Grid Snap to align the move."
    } else if state.editing {
        "Ready."
    } else {
        "Select an object to edit."
    };
    (message.to_string(), false)
}

pub(super) fn draw_status_bar(ctx: &FrameContext, catalog: &ObjectCatalog, state: &CadSceneState) {
    let height = STATUS_BAR_HEIGHT.min(screen_height());
    let y = screen_height() - height;
    draw_panel_frame(
        Rect::new(0.0, y, screen_width(), height),
        1.0,
        ctx.colors_rt.panel_bg,
        ctx.colors_rt.panel_border,
    );

    let asset_error = selected_entry(catalog, state.selected).and_then(|entry| {
        entry
            .metadata_error
            .as_deref()
            .or_else(|| {
                entry
                    .states
                    .iter()
                    .find_map(|state| state.metadata_error.as_deref())
            })
            .or_else(|| entry.states.iter().find_map(|state| state.error.as_deref()))
            .or(entry.variant_config_error.as_deref())
            .or(entry.states_error.as_deref())
            .or(entry.error.as_deref())
    });
    let (message, is_error) = status_bar_message(state, asset_error);
    let baseline = y + height * 0.68;
    let font_size = ctx.font_sm as u16;
    let object_label = selected_entry(catalog, state.selected)
        .map(|entry| entry.definition.label)
        .unwrap_or("Object");
    let details = if state.editing {
        format!(
            "{} | Grid Snap: {}{}",
            object_label,
            if state.grid_snap { "On" } else { "Off" },
            if state.dirty { " | Unsaved edits" } else { "" }
        )
    } else {
        object_label.to_string()
    };
    let details = fit_status_text(&details, screen_width() - 24.0, font_size);
    let details_width = measure_text(&details, None, font_size, 1.0).width;
    let message = fit_status_text(&message, screen_width() - details_width - 36.0, font_size);
    draw_text(
        &message,
        12.0,
        baseline,
        ctx.font_sm,
        if is_error {
            Color::from_rgba(255, 110, 110, 255)
        } else {
            ctx.colors_rt.text_primary
        },
    );
    draw_text(
        &details,
        (screen_width() - details_width - 12.0).max(12.0),
        baseline,
        ctx.font_sm,
        if state.dirty {
            Color::from_rgba(255, 220, 100, 255)
        } else {
            ctx.colors_rt.text_secondary
        },
    );
}
