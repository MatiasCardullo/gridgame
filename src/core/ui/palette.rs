use serde::{Deserialize, Serialize};

use crate::core::{AppColors, ColorRgba};

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
        ColorTarget::RouteLine => "Route linea",
        ColorTarget::PortIn => "Input",
        ColorTarget::PortOut => "Output",
        ColorTarget::BlockBase => "Base",
        ColorTarget::BlockBuilder => "Builder",
        ColorTarget::BlockHousing => "Housing",
        ColorTarget::BlockFactory => "Factory",
        ColorTarget::BlockMine => "Mine",
        ColorTarget::BlockWarehouse => "Warehouse",
        ColorTarget::BlockLogistics => "Logistics",
        ColorTarget::BlockRoute => "Route",
        ColorTarget::TileIron => "Iron",
        ColorTarget::TileCopper => "Copper",
        ColorTarget::TileGold => "Gold",
        ColorTarget::TileZinc => "Zinc",
        ColorTarget::TileLead => "Lead",
        ColorTarget::TileAluminum => "Aluminum",
        ColorTarget::TileWater => "Water",
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
        ColorTarget::BlockBase => &mut colors.block_base,
        ColorTarget::BlockBuilder => &mut colors.block_builder,
        ColorTarget::BlockHousing => &mut colors.block_housing,
        ColorTarget::BlockFactory => &mut colors.block_factory,
        ColorTarget::BlockMine => &mut colors.block_mine,
        ColorTarget::BlockWarehouse => &mut colors.block_warehouse,
        ColorTarget::BlockLogistics => &mut colors.block_logistics,
        ColorTarget::BlockRoute => &mut colors.block_route,
        ColorTarget::TileIron => &mut colors.tile_iron,
        ColorTarget::TileCopper => &mut colors.tile_copper,
        ColorTarget::TileGold => &mut colors.tile_gold,
        ColorTarget::TileZinc => &mut colors.tile_zinc,
        ColorTarget::TileLead => &mut colors.tile_lead,
        ColorTarget::TileAluminum => &mut colors.tile_aluminum,
        ColorTarget::TileWater => &mut colors.tile_water,
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
    BlockBase,
    BlockBuilder,
    BlockHousing,
    BlockFactory,
    BlockMine,
    BlockWarehouse,
    BlockLogistics,
    BlockRoute,
    TileIron,
    TileCopper,
    TileGold,
    TileZinc,
    TileLead,
    TileAluminum,
    TileWater,
}

// Ordered list of selectable color targets.
pub fn color_target_list() -> [ColorTarget; 31] {
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
        ColorTarget::BlockBase,
        ColorTarget::BlockBuilder,
        ColorTarget::BlockHousing,
        ColorTarget::BlockFactory,
        ColorTarget::BlockMine,
        ColorTarget::BlockWarehouse,
        ColorTarget::BlockLogistics,
        ColorTarget::BlockRoute,
        ColorTarget::TileIron,
        ColorTarget::TileCopper,
        ColorTarget::TileGold,
        ColorTarget::TileZinc,
        ColorTarget::TileLead,
        ColorTarget::TileAluminum,
        ColorTarget::TileWater,
    ]
}
