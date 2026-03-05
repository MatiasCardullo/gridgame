use macroquad::prelude::*;
use crate::core::{hex_distance, hex_to_pixel, pixel_to_hex, AppConfig, Axial, FrameContext, RuntimeColors};
use crate::core::ui::{
    draw_window, ui_button, window_close_rect, window_title_rect, WindowState, WindowStyle,
    WINDOW_TITLE_HEIGHT,
};
use crate::{TRI_LENGHT, HEX_RADIUS, HEX_SIZE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapOutline {
    Hexagon,
    Triangle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConfirmAction {
    None,
    Cancel,
    Confirm,
}

#[derive(Clone, Copy, Debug)]
pub struct PanelLayout {
    pub pos: Vec2,
    pub size: Vec2,
    pub rect: Rect,
}

pub fn in_bounds(hex: Axial, outline: MapOutline) -> bool {
    match outline {
        MapOutline::Hexagon => hex_distance(hex, Axial { q: 0, r: 0 }) <= HEX_RADIUS,
        MapOutline::Triangle => {
            let q = hex.q;
            let r = hex.r;
            q >= 0 && r >= 0 && q + r <= TRI_LENGHT
        }
    }
}

fn outline_centroid(outline: MapOutline) -> Vec2 {
    match outline {
        MapOutline::Hexagon => Vec2::ZERO,
        MapOutline::Triangle => {
            let a = hex_to_pixel(Axial { q: 0, r: 0 }, HEX_SIZE, Vec2::ZERO);
            let b = hex_to_pixel(Axial { q: TRI_LENGHT, r: 0 }, HEX_SIZE, Vec2::ZERO);
            let c = hex_to_pixel(Axial { q: 0, r: TRI_LENGHT }, HEX_SIZE, Vec2::ZERO);
            (a + b + c) / 3.0
        }
    }
}

pub fn outline_start_offset(outline: MapOutline, zoom: f32) -> Vec2 {
    -outline_centroid(outline) * zoom
}

pub fn hover_hex_from_mouse(ctx: &FrameContext, cam_offset: Vec2, cam_zoom: f32) -> Axial {
    let world_mouse = (ctx.mouse - ctx.screen_center - cam_offset) / cam_zoom;
    pixel_to_hex(world_mouse, HEX_SIZE, Vec2::ZERO)
}

pub fn hex_screen_center(ctx: &FrameContext, cam_offset: Vec2, cam_zoom: f32, hex: Axial) -> Vec2 {
    ctx.screen_center + cam_offset + hex_to_pixel(hex, HEX_SIZE, Vec2::ZERO) * cam_zoom
}

pub fn draw_common_hud(
    ctx: &FrameContext,
    colors: &RuntimeColors,
    placement_rotation: u8,
    hover_hex: Axial,
) {
    draw_text(
        "Right click: place  |  Wheel: zoom  |  Middle mouse: pan  |  R: rotate  |  Esc: menu",
        16.0,
        28.0,
        ctx.font_md,
        colors.text_primary,
    );

    let coord_text = format!("Hex: q={} r={}", hover_hex.q, hover_hex.r);
    draw_text(
        &coord_text,
        16.0,
        52.0,
        ctx.font_sm,
        colors.text_secondary,
    );

    let rot_text = format!("Rotation: {}", placement_rotation);
    draw_text(&rot_text, 16.0, 74.0, ctx.font_sm, colors.text_secondary);
}

pub fn build_panel_layout(button_count: usize, panel_collapsed: bool) -> PanelLayout {
    let button_size = 52.0;
    let gap = 8.0;
    let panel_pad_x = 12.0;
    let panel_pad_y = 16.0;
    let toggle_space = 36.0;
    let button_count = button_count as f32;
    let panel_width = panel_pad_x * 2.0
        + (button_size * button_count)
        + (gap * (button_count - 1.0))
        + toggle_space;
    let panel_height = panel_pad_y * 2.0 + button_size;
    let panel_size = if panel_collapsed {
        vec2(170.0, 36.0)
    } else {
        vec2(panel_width, panel_height)
    };
    let panel_pos = vec2(16.0, screen_height() - panel_size.y - 16.0);
    let panel_rect = Rect::new(panel_pos.x, panel_pos.y, panel_size.x, panel_size.y);
    PanelLayout {
        pos: panel_pos,
        size: panel_size,
        rect: panel_rect,
    }
}

pub fn is_ui_capturing(
    panel_rect: Rect,
    window: &WindowState,
    confirm_window: &WindowState,
    mouse: Vec2,
) -> bool {
    let mut capturing = panel_rect.contains(mouse);
    if window.open && (window.rect.contains(mouse) || window.dragging) {
        capturing = true;
    }
    if confirm_window.open && (confirm_window.rect.contains(mouse) || confirm_window.dragging) {
        capturing = true;
    }
    capturing
}

pub fn draw_window_frame(
    window: &mut WindowState,
    ctx: &FrameContext,
    config: &AppConfig,
    colors: &RuntimeColors,
) -> bool {
    handle_window_drag(window, ctx.mouse);
    let style = WindowStyle {
        bg: colors.panel_bg,
        border: colors.panel_border,
        title: colors.text_primary,
        title_bg: colors.button_base,
    };
    if draw_window(window, style, ctx.font_sm, config.line_thickness, ctx.mouse) {
        window.open = false;
        window.target = None;
        window.dragging = false;
        return true;
    }
    false
}

pub fn run_confirm_window(
    confirm_window: &mut WindowState,
    ctx: &FrameContext,
    config: &AppConfig,
    colors: &RuntimeColors,
    label: &str,
) -> ConfirmAction {
    if !confirm_window.open {
        return ConfirmAction::None;
    }

    if draw_window_frame(confirm_window, ctx, config, colors) {
        return ConfirmAction::Cancel;
    }

    let content_x = confirm_window.rect.x + 10.0;
    let mut content_y = confirm_window.rect.y + WINDOW_TITLE_HEIGHT + 20.0;
    draw_text(label, content_x, content_y, ctx.font_sm, colors.text_secondary);
    content_y += 28.0;

    let rect_cancel = Rect::new(content_x, content_y, 90.0, 26.0);
    let rect_ok = Rect::new(content_x + 100.0, content_y, 90.0, 26.0);
    let (clicked_cancel, _) =
        ui_button(rect_cancel, "Cancel", ctx.mouse, ctx.font_sm, ctx.button_colors);
    let (clicked_ok, _) =
        ui_button(rect_ok, "Delete", ctx.mouse, ctx.font_sm, ctx.button_colors);

    if clicked_cancel {
        confirm_window.open = false;
        confirm_window.target = None;
        return ConfirmAction::Cancel;
    }
    if clicked_ok {
        confirm_window.open = false;
        confirm_window.target = None;
        return ConfirmAction::Confirm;
    }

    ConfirmAction::None
}

pub fn confirm_label_for_target<F>(target: Option<Axial>, label_fn: F) -> String
where
    F: FnOnce(Axial) -> Option<String>,
{
    if let Some(target) = target {
        if let Some(label) = label_fn(target) {
            format!("Demolish {}?", label)
        } else {
            "Block not found".to_string()
        }
    } else {
        "Block not found".to_string()
    }
}

pub fn popup_rect_near_mouse(mouse: Vec2, win_w: f32, win_h: f32) -> Rect {
    let mut x = mouse.x + 12.0;
    let mut y = mouse.y + 12.0;
    if x + win_w > screen_width() {
        x = screen_width() - win_w - 8.0;
    }
    if y + win_h > screen_height() {
        y = screen_height() - win_h - 8.0;
    }
    Rect::new(x.max(8.0), y.max(8.0), win_w, win_h)
}

pub fn handle_cursor_zoom(
    ctx: &FrameContext,
    cam_offset: &mut Vec2,
    cam_zoom: &mut f32,
    zoom_speed: f32,
) {
    let wheel = mouse_wheel().1;
    if wheel.abs() > 0.01 {
        let factor = 1.0 + wheel * zoom_speed * 0.05;
        let new_zoom = (*cam_zoom * factor).clamp(0.3, 3.0);
        let world_pos = (ctx.mouse - ctx.screen_center - *cam_offset) / *cam_zoom;
        *cam_zoom = new_zoom;
        *cam_offset = ctx.mouse - ctx.screen_center - world_pos * *cam_zoom;
    }
}

pub fn handle_camera_drag(
    ctx: &FrameContext,
    cam_offset: &mut Vec2,
    dragging: &mut bool,
    last_mouse: &mut Vec2,
) {
    if is_mouse_button_pressed(MouseButton::Middle) {
        *dragging = true;
        *last_mouse = ctx.mouse;
    }
    if is_mouse_button_down(MouseButton::Middle) && *dragging {
        let delta = ctx.mouse - *last_mouse;
        *cam_offset += delta;
        *last_mouse = ctx.mouse;
    }
    if is_mouse_button_released(MouseButton::Middle) {
        *dragging = false;
    }
}

pub fn handle_window_drag(window: &mut WindowState, mouse: Vec2) {
    let title_rect = window_title_rect(window);
    let close_rect = window_close_rect(window);
    if is_mouse_button_pressed(MouseButton::Left)
        && title_rect.contains(mouse)
        && !close_rect.contains(mouse)
    {
        window.dragging = true;
        window.drag_offset = mouse - vec2(window.rect.x, window.rect.y);
    }
    if window.dragging && is_mouse_button_down(MouseButton::Left) {
        let mut x = mouse.x - window.drag_offset.x;
        let mut y = mouse.y - window.drag_offset.y;
        if x + window.rect.w > screen_width() {
            x = screen_width() - window.rect.w;
        }
        if y + window.rect.h > screen_height() {
            y = screen_height() - window.rect.h;
        }
        if x < 0.0 {
            x = 0.0;
        }
        if y < 0.0 {
            y = 0.0;
        }
        window.rect.x = x;
        window.rect.y = y;
    }
    if is_mouse_button_released(MouseButton::Left) {
        window.dragging = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_label_missing_target() {
        let label = confirm_label_for_target(None, |_target| Some("X".to_string()));
        assert_eq!(label, "Block not found");
    }

    #[test]
    fn confirm_label_with_target() {
        let target = Axial { q: 1, r: -2 };
        let label = confirm_label_for_target(Some(target), |_target| Some("Test".to_string()));
        assert_eq!(label, "Demolish Test?");
    }
}


