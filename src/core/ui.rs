use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

use crate::core::{
    block_color, draw_hex_filled, draw_hex_outline, AppColors, Axial, BlockType, ColorRgba,
    RuntimeColors,
};

#[derive(Clone, Copy, Debug)]
pub struct UiButtonColors {
    pub base: Color,
    pub hover: Color,
    pub border: Color,
    pub text: Color,
}

// Window frame styling options.
#[derive(Clone, Copy, Debug)]
pub struct WindowStyle {
    pub bg: Color,
    pub border: Color,
    pub title: Color,
    pub title_bg: Color,
}

// Simple window state for UI popups.
#[derive(Clone, Debug)]
pub struct WindowState {
    pub title: String,
    pub rect: Rect,
    pub open: bool,
    pub target: Option<Axial>,
}

// UI result from a panel template.
#[derive(Clone, Copy, Debug)]
pub struct PanelResult {
    pub toggled: bool,
    pub toggle_hovered: bool,
    pub selected_changed: bool,
}

// Result for the build panel content area.
#[derive(Clone, Debug)]
pub struct BuildPanelResult {
    pub toggled: bool,
    pub toggle_hovered: bool,
    pub clicked_option: Option<Option<BlockType>>,
    pub hovered_tip: Option<&'static str>,
}

// Draw a simple UI button and return (clicked, hovered).
pub fn ui_button(
    rect: Rect,
    label: &str,
    mouse: Vec2,
    font_size: f32,
    colors: UiButtonColors,
) -> (bool, bool) {
    let hover = rect.contains(mouse);
    let base = if hover { colors.hover } else { colors.base };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, base);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.5, colors.border);
    let text_dim = measure_text(label, None, font_size as u16, 1.0);
    draw_text(
        label,
        rect.x + (rect.w - text_dim.width) * 0.5,
        rect.y + (rect.h + text_dim.height) * 0.5 - 4.0,
        font_size,
        colors.text,
    );
    (hover && is_mouse_button_pressed(MouseButton::Left), hover)
}

// Friendly label for a color target in UI.
pub fn color_target_name(target: ColorTarget) -> &'static str {
    match target {
        ColorTarget::Background => "Fondo",
        ColorTarget::Grid => "Grilla",
        ColorTarget::Hover => "Hover",
        ColorTarget::TextPrimary => "Texto",
        ColorTarget::TextSecondary => "Texto secund",
        ColorTarget::ButtonBase => "Boton base",
        ColorTarget::ButtonHover => "Boton hover",
        ColorTarget::ButtonBorder => "Boton borde",
        ColorTarget::ButtonText => "Boton texto",
        ColorTarget::PanelBg => "Panel fondo",
        ColorTarget::PanelBorder => "Panel borde",
        ColorTarget::TooltipBg => "Tooltip fondo",
        ColorTarget::TooltipBorder => "Tooltip borde",
        ColorTarget::RouteLine => "Ruta linea",
        ColorTarget::PortIn => "Input",
        ColorTarget::PortOut => "Output",
        ColorTarget::BlockVivienda => "Vivienda",
        ColorTarget::BlockFabrica => "Fabrica",
        ColorTarget::BlockMina => "Mina",
        ColorTarget::BlockAlmacen => "Almacen",
        ColorTarget::BlockLogistica => "Logistica",
        ColorTarget::BlockRuta => "Ruta",
        ColorTarget::TilePiedra => "Piedra",
        ColorTarget::TileHierro => "Hierro",
        ColorTarget::TileCobre => "Cobre",
        ColorTarget::TileAgua => "Agua",
    }
}

// Get a mutable color entry by target.
pub fn color_target_mut(target: ColorTarget, colors: &mut AppColors) -> &mut ColorRgba {
    match target {
        ColorTarget::Background => &mut colors.background,
        ColorTarget::Grid => &mut colors.grid,
        ColorTarget::Hover => &mut colors.hover,
        ColorTarget::TextPrimary => &mut colors.text_primary,
        ColorTarget::TextSecondary => &mut colors.text_secondary,
        ColorTarget::ButtonBase => &mut colors.button_base,
        ColorTarget::ButtonHover => &mut colors.button_hover,
        ColorTarget::ButtonBorder => &mut colors.button_border,
        ColorTarget::ButtonText => &mut colors.button_text,
        ColorTarget::PanelBg => &mut colors.panel_bg,
        ColorTarget::PanelBorder => &mut colors.panel_border,
        ColorTarget::TooltipBg => &mut colors.tooltip_bg,
        ColorTarget::TooltipBorder => &mut colors.tooltip_border,
        ColorTarget::RouteLine => &mut colors.route_line,
        ColorTarget::PortIn => &mut colors.port_in,
        ColorTarget::PortOut => &mut colors.port_out,
        ColorTarget::BlockVivienda => &mut colors.block_vivienda,
        ColorTarget::BlockFabrica => &mut colors.block_fabrica,
        ColorTarget::BlockMina => &mut colors.block_mina,
        ColorTarget::BlockAlmacen => &mut colors.block_almacen,
        ColorTarget::BlockLogistica => &mut colors.block_logistica,
        ColorTarget::BlockRuta => &mut colors.block_ruta,
        ColorTarget::TilePiedra => &mut colors.tile_piedra,
        ColorTarget::TileHierro => &mut colors.tile_hierro,
        ColorTarget::TileCobre => &mut colors.tile_cobre,
        ColorTarget::TileAgua => &mut colors.tile_agua,
    }
}

// Adjust a single RGBA channel by a delta in UI.
pub fn adjust_color_channel(color: &mut ColorRgba, channel: usize, delta: i32) {
    let apply = |value: u8, delta: i32| -> u8 {
        let next = value as i32 + delta;
        next.clamp(0, 255) as u8
    };
    match channel {
        0 => color.r = apply(color.r, delta),
        1 => color.g = apply(color.g, delta),
        2 => color.b = apply(color.b, delta),
        _ => color.a = apply(color.a, delta),
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ColorTarget {
    Background,
    Grid,
    Hover,
    TextPrimary,
    TextSecondary,
    ButtonBase,
    ButtonHover,
    ButtonBorder,
    ButtonText,
    PanelBg,
    PanelBorder,
    TooltipBg,
    TooltipBorder,
    RouteLine,
    PortIn,
    PortOut,
    BlockVivienda,
    BlockFabrica,
    BlockMina,
    BlockAlmacen,
    BlockLogistica,
    BlockRuta,
    TilePiedra,
    TileHierro,
    TileCobre,
    TileAgua,
}

// Ordered list of selectable color targets.
pub fn color_target_list() -> [ColorTarget; 26] {
    [
        ColorTarget::Background,
        ColorTarget::Grid,
        ColorTarget::Hover,
        ColorTarget::TextPrimary,
        ColorTarget::TextSecondary,
        ColorTarget::ButtonBase,
        ColorTarget::ButtonHover,
        ColorTarget::ButtonBorder,
        ColorTarget::ButtonText,
        ColorTarget::PanelBg,
        ColorTarget::PanelBorder,
        ColorTarget::TooltipBg,
        ColorTarget::TooltipBorder,
        ColorTarget::RouteLine,
        ColorTarget::PortIn,
        ColorTarget::PortOut,
        ColorTarget::BlockVivienda,
        ColorTarget::BlockFabrica,
        ColorTarget::BlockMina,
        ColorTarget::BlockAlmacen,
        ColorTarget::BlockLogistica,
        ColorTarget::BlockRuta,
        ColorTarget::TilePiedra,
        ColorTarget::TileHierro,
        ColorTarget::TileCobre,
        ColorTarget::TileAgua,
    ]
}

// Template for the game panel (toggle + container). Returns toggle info.
pub fn draw_game_panel(
    panel_pos: Vec2,
    panel_size: Vec2,
    mouse: Vec2,
    font_sm: f32,
    line_thickness: f32,
    colors: &RuntimeColors,
) -> PanelResult {
    let panel_rect = Rect::new(panel_pos.x, panel_pos.y, panel_size.x, panel_size.y);
    let mut toggled = false;
    let mut toggle_hovered = false;

    draw_rectangle(panel_pos.x, panel_pos.y, panel_size.x, panel_size.y, colors.panel_bg);
    draw_rectangle_lines(
        panel_pos.x,
        panel_pos.y,
        panel_size.x,
        panel_size.y,
        line_thickness.max(1.0),
        colors.panel_border,
    );

    let toggle_rect = Rect::new(panel_pos.x + panel_size.x - 32.0, panel_pos.y + 6.0, 26.0, 24.0);
    toggle_hovered = toggle_rect.contains(mouse);
    let toggle_color = if toggle_hovered { colors.button_hover } else { colors.button_base };
    draw_rectangle(toggle_rect.x, toggle_rect.y, toggle_rect.w, toggle_rect.h, toggle_color);
    draw_rectangle_lines(
        toggle_rect.x,
        toggle_rect.y,
        toggle_rect.w,
        toggle_rect.h,
        line_thickness.max(1.0),
        colors.button_border,
    );

    let toggle_label = if panel_size.y < 60.0 { ">>" } else { "<<" };
    let toggle_dim = measure_text(toggle_label, None, font_sm as u16, 1.0);
    draw_text(
        toggle_label,
        toggle_rect.x + (toggle_rect.w - toggle_dim.width) * 0.5,
        toggle_rect.y + (toggle_rect.h + toggle_dim.height) * 0.5 - 2.0,
        font_sm,
        colors.button_text,
    );

    if toggle_hovered && is_mouse_button_pressed(MouseButton::Left) {
        toggled = true;
    }

    PanelResult {
        toggled,
        toggle_hovered,
        selected_changed: false,
    }
}

// Draw build panel buttons and return click/hover info.
pub fn draw_build_panel(
    panel_pos: Vec2,
    panel_size: Vec2,
    collapsed: bool,
    mouse: Vec2,
    line_thickness: f32,
    colors: &RuntimeColors,
    buttons: &[(Option<BlockType>, &'static str)],
    selected: Option<BlockType>,
) -> BuildPanelResult {
    let panel_result = draw_game_panel(panel_pos, panel_size, mouse, 18.0, line_thickness, colors);
    let mut hovered_tip: Option<&'static str> = None;
    let mut clicked_option: Option<Option<BlockType>> = None;

    if !collapsed {
        let button_size = 52.0;
        let gap = 8.0;
        let mut bx = panel_pos.x + 12.0;
        let by = panel_pos.y + 16.0;

        for (option, tip) in buttons {
            let rect = Rect::new(bx, by, button_size, button_size);
            let hover = rect.contains(mouse);
            if hover {
                hovered_tip = Some(*tip);
            }
            let selected_now = selected == *option;

            let base_color = if selected_now { colors.button_hover } else { colors.button_base };
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, base_color);
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                line_thickness.max(1.0),
                colors.button_border,
            );

            if hover && is_mouse_button_pressed(MouseButton::Left) {
                clicked_option = Some(*option);
            }

            if let Some(kind) = *option {
                let icon_center = vec2(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
                draw_hex_filled(icon_center, 16.0, block_color(kind, colors));
                draw_hex_outline(
                    icon_center,
                    16.0,
                    colors.panel_border,
                    line_thickness.max(1.0),
                );
            } else {
                let pad = 10.0;
                draw_line(
                    rect.x + pad,
                    rect.y + pad,
                    rect.x + rect.w - pad,
                    rect.y + rect.h - pad,
                    3.0,
                    colors.port_out,
                );
                draw_line(
                    rect.x + rect.w - pad,
                    rect.y + pad,
                    rect.x + pad,
                    rect.y + rect.h - pad,
                    3.0,
                    colors.port_out,
                );
            }

            if hover {
                draw_rectangle_lines(
                    rect.x - 1.0,
                    rect.y - 1.0,
                    rect.w + 2.0,
                    rect.h + 2.0,
                    line_thickness.max(1.0),
                    colors.hover,
                );
            }

            bx += button_size + gap;
        }
    }

    BuildPanelResult {
        toggled: panel_result.toggled,
        toggle_hovered: panel_result.toggle_hovered,
        clicked_option,
        hovered_tip,
    }
}

// Draw a basic window frame. Returns true if close clicked.
pub fn draw_window(
    state: &WindowState,
    style: WindowStyle,
    font_title: f32,
    line_thickness: f32,
    mouse: Vec2,
) -> bool {
    if !state.open {
        return false;
    }
    draw_rectangle(state.rect.x, state.rect.y, state.rect.w, state.rect.h, style.bg);
    draw_rectangle_lines(
        state.rect.x,
        state.rect.y,
        state.rect.w,
        state.rect.h,
        line_thickness.max(1.0),
        style.border,
    );

    let title_h = 28.0;
    draw_rectangle(state.rect.x, state.rect.y, state.rect.w, title_h, style.title_bg);
    draw_text(
        &state.title,
        state.rect.x + 8.0,
        state.rect.y + title_h - 8.0,
        font_title,
        style.title,
    );

    let close_rect = Rect::new(state.rect.x + state.rect.w - 24.0, state.rect.y + 4.0, 18.0, 18.0);
    draw_rectangle(close_rect.x, close_rect.y, close_rect.w, close_rect.h, style.border);
    draw_text("x", close_rect.x + 5.0, close_rect.y + 14.0, font_title, style.title);
    close_rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Left)
}
