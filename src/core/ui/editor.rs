use macroquad::prelude::*;

use super::{UiButtonColors, ui_button};

#[derive(Clone, Copy, Debug)]
pub struct UiValueStepperLayout {
    pub label_position: Vec2,
    pub decrement_rect: Rect,
    pub increment_rect: Rect,
    pub value_position: Vec2,
}

#[derive(Clone, Copy, Debug)]
pub struct UiValueStepperStyle {
    pub font_size: f32,
    pub text_color: Color,
    pub button_colors: UiButtonColors,
}

pub fn draw_value_stepper_row(
    label: &str,
    value: &str,
    layout: UiValueStepperLayout,
    mouse: Vec2,
    style: UiValueStepperStyle,
) -> (bool, bool) {
    draw_text(
        label,
        layout.label_position.x,
        layout.label_position.y,
        style.font_size,
        style.text_color,
    );
    let (decremented, _) = ui_button(
        layout.decrement_rect,
        "-",
        mouse,
        style.font_size,
        style.button_colors,
    );
    let (incremented, _) = ui_button(
        layout.increment_rect,
        "+",
        mouse,
        style.font_size,
        style.button_colors,
    );
    draw_text(
        value,
        layout.value_position.x,
        layout.value_position.y,
        style.font_size,
        style.text_color,
    );
    (decremented, incremented)
}

pub fn draw_editor_list_row(
    rect: Rect,
    label: &str,
    mouse: Vec2,
    font_size: f32,
    selected: bool,
    selected_color: Color,
    colors: UiButtonColors,
) -> (bool, bool) {
    let hovered = rect.contains(mouse);
    let fill = if selected {
        selected_color
    } else if hovered {
        colors.hover
    } else {
        colors.base
    };
    let border = if selected {
        selected_color
    } else {
        colors.border
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, border);
    draw_text(
        label,
        rect.x + 10.0,
        rect.y + rect.h * 0.5 + font_size * 0.35,
        font_size,
        colors.text,
    );
    (
        hovered && is_mouse_button_pressed(MouseButton::Left),
        hovered,
    )
}

pub fn draw_editor_action_button(
    rect: Rect,
    label: &str,
    mouse: Vec2,
    font_size: f32,
    active: bool,
    active_color: Color,
    colors: UiButtonColors,
) -> (bool, bool) {
    let hovered = rect.contains(mouse);
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        if active {
            active_color
        } else if hovered {
            colors.hover
        } else {
            colors.base
        },
    );
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, colors.border);
    let text_width = measure_text(label, None, font_size as u16, 1.0).width;
    draw_text(
        label,
        rect.x + (rect.w - text_width) * 0.5,
        rect.y + rect.h * 0.5 + font_size * 0.35,
        font_size,
        colors.text,
    );
    (
        hovered && is_mouse_button_pressed(MouseButton::Left),
        hovered,
    )
}
