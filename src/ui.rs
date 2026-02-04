use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub struct UiButtonColors {
    pub base: Color,
    pub hover: Color,
    pub border: Color,
    pub text: Color,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ColorRgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorRgba {
    pub fn to_color(self) -> Color {
        Color::from_rgba(self.r, self.g, self.b, self.a)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub zoom_speed: f32,
    pub text_scale: f32,
    pub tile_clusters: usize,
    pub tile_cluster_min: usize,
    pub tile_cluster_max: usize,
    pub tile_neighbor_chance: f32,
    pub tile_center_bonus: f32,
    pub weight_piedra: f32,
    pub weight_hierro: f32,
    pub weight_cobre: f32,
    pub weight_agua: f32,
    pub line_thickness: f32,
    pub arrow_scale: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            zoom_speed: 0.1,
            text_scale: 1.0,
            tile_clusters: 80,
            tile_cluster_min: 8,
            tile_cluster_max: 20,
            tile_neighbor_chance: 0.45,
            tile_center_bonus: 0.35,
            weight_piedra: 0.35,
            weight_hierro: 0.2,
            weight_cobre: 0.2,
            weight_agua: 0.25,
            line_thickness: 1.0,
            arrow_scale: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeColors {
    pub background: Color,
    pub grid: Color,
    pub hover: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub button_base: Color,
    pub button_hover: Color,
    pub button_border: Color,
    pub button_text: Color,
    pub panel_bg: Color,
    pub panel_border: Color,
    pub tooltip_bg: Color,
    pub tooltip_border: Color,
    pub route_line: Color,
    pub port_in: Color,
    pub port_out: Color,
    pub block_vivienda: Color,
    pub block_fabrica: Color,
    pub block_mina: Color,
    pub block_almacen: Color,
    pub block_logistica: Color,
    pub block_ruta: Color,
    pub tile_piedra: Color,
    pub tile_hierro: Color,
    pub tile_cobre: Color,
    pub tile_agua: Color,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AppColors {
    pub background: ColorRgba,
    pub grid: ColorRgba,
    pub hover: ColorRgba,
    pub text_primary: ColorRgba,
    pub text_secondary: ColorRgba,
    pub button_base: ColorRgba,
    pub button_hover: ColorRgba,
    pub button_border: ColorRgba,
    pub button_text: ColorRgba,
    pub panel_bg: ColorRgba,
    pub panel_border: ColorRgba,
    pub tooltip_bg: ColorRgba,
    pub tooltip_border: ColorRgba,
    pub route_line: ColorRgba,
    pub port_in: ColorRgba,
    pub port_out: ColorRgba,
    pub block_vivienda: ColorRgba,
    pub block_fabrica: ColorRgba,
    pub block_mina: ColorRgba,
    pub block_almacen: ColorRgba,
    pub block_logistica: ColorRgba,
    pub block_ruta: ColorRgba,
    pub tile_piedra: ColorRgba,
    pub tile_hierro: ColorRgba,
    pub tile_cobre: ColorRgba,
    pub tile_agua: ColorRgba,
}

impl Default for AppColors {
    fn default() -> Self {
        Self {
            background: ColorRgba { r: 18, g: 22, b: 26, a: 255 },
            grid: ColorRgba { r: 70, g: 78, b: 86, a: 255 },
            hover: ColorRgba { r: 255, g: 210, b: 90, a: 255 },
            text_primary: ColorRgba { r: 220, g: 220, b: 220, a: 255 },
            text_secondary: ColorRgba { r: 190, g: 200, b: 210, a: 255 },
            button_base: ColorRgba { r: 50, g: 62, b: 75, a: 255 },
            button_hover: ColorRgba { r: 70, g: 85, b: 100, a: 255 },
            button_border: ColorRgba { r: 110, g: 130, b: 150, a: 255 },
            button_text: ColorRgba { r: 220, g: 230, b: 240, a: 255 },
            panel_bg: ColorRgba { r: 28, g: 34, b: 40, a: 230 },
            panel_border: ColorRgba { r: 80, g: 90, b: 100, a: 255 },
            tooltip_bg: ColorRgba { r: 30, g: 36, b: 44, a: 240 },
            tooltip_border: ColorRgba { r: 100, g: 120, b: 140, a: 255 },
            route_line: ColorRgba { r: 120, g: 140, b: 160, a: 200 },
            port_in: ColorRgba { r: 80, g: 160, b: 220, a: 255 },
            port_out: ColorRgba { r: 220, g: 170, b: 90, a: 255 },
            block_vivienda: ColorRgba { r: 120, g: 200, b: 120, a: 255 },
            block_fabrica: ColorRgba { r: 220, g: 140, b: 80, a: 255 },
            block_mina: ColorRgba { r: 110, g: 150, b: 200, a: 255 },
            block_almacen: ColorRgba { r: 210, g: 190, b: 90, a: 255 },
            block_logistica: ColorRgba { r: 120, g: 140, b: 160, a: 255 },
            block_ruta: ColorRgba { r: 90, g: 100, b: 115, a: 255 },
            tile_piedra: ColorRgba { r: 110, g: 110, b: 120, a: 255 },
            tile_hierro: ColorRgba { r: 120, g: 95, b: 85, a: 255 },
            tile_cobre: ColorRgba { r: 150, g: 95, b: 70, a: 255 },
            tile_agua: ColorRgba { r: 60, g: 110, b: 160, a: 255 },
        }
    }
}

impl AppColors {
    pub fn runtime(&self) -> RuntimeColors {
        RuntimeColors {
            background: self.background.to_color(),
            grid: self.grid.to_color(),
            hover: self.hover.to_color(),
            text_primary: self.text_primary.to_color(),
            text_secondary: self.text_secondary.to_color(),
            button_base: self.button_base.to_color(),
            button_hover: self.button_hover.to_color(),
            button_border: self.button_border.to_color(),
            button_text: self.button_text.to_color(),
            panel_bg: self.panel_bg.to_color(),
            panel_border: self.panel_border.to_color(),
            tooltip_bg: self.tooltip_bg.to_color(),
            tooltip_border: self.tooltip_border.to_color(),
            route_line: self.route_line.to_color(),
            port_in: self.port_in.to_color(),
            port_out: self.port_out.to_color(),
            block_vivienda: self.block_vivienda.to_color(),
            block_fabrica: self.block_fabrica.to_color(),
            block_mina: self.block_mina.to_color(),
            block_almacen: self.block_almacen.to_color(),
            block_logistica: self.block_logistica.to_color(),
            block_ruta: self.block_ruta.to_color(),
            tile_piedra: self.tile_piedra.to_color(),
            tile_hierro: self.tile_hierro.to_color(),
            tile_cobre: self.tile_cobre.to_color(),
            tile_agua: self.tile_agua.to_color(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
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
