use std::collections::HashMap;

use macroquad::prelude::*;

use crate::core::{
    AppConfig, Axial, BlockType, FrameContext, PlacedBlock, RuntimeColors, Scene, TileData,
};
use crate::core::ui::WindowState;
use crate::scenes::planet_sector::{run as run_planet_sector, MapOutline};

// Build Template scene shares the planet sector logic but keeps the hex outline.
pub fn run(
    ctx: &FrameContext,
    blocks: &mut HashMap<Axial, PlacedBlock>,
    tiles: &mut HashMap<Axial, TileData>,
    cam_offset: &mut Vec2,
    cam_zoom: &mut f32,
    dragging: &mut bool,
    last_mouse: &mut Vec2,
    selected: &mut Option<BlockType>,
    placement_rotation: &mut u8,
    panel_collapsed: &mut bool,
    window: &mut WindowState,
    confirm_window: &mut WindowState,
    units: &mut Vec<crate::core::Unit>,
    station_in: &mut Option<Axial>,
    station_out: &mut Option<Axial>,
    station_pick: &mut Option<crate::core::StationPick>,
    dirty: &mut bool,
    scene: &mut Scene,
    map_path: &str,
    config: &AppConfig,
    colors: &RuntimeColors,
) {
    run_planet_sector(
        ctx,
        blocks,
        tiles,
        cam_offset,
        cam_zoom,
        dragging,
        last_mouse,
        selected,
        placement_rotation,
        panel_collapsed,
        window,
        confirm_window,
        units,
        station_in,
        station_out,
        station_pick,
        dirty,
        scene,
        map_path,
        config,
        colors,
        MapOutline::Hexagon,
    );
}
