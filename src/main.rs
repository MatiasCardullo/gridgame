use std::collections::HashMap;
use std::fs;

use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

const HEX_SIZE: f32 = 16.0;
const GRID_RADIUS: i32 = 64;
const SQRT_3: f32 = 1.732_050_8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Axial {
    q: i32,
    r: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum BlockType {
    Vivienda,
    Fabrica,
    Mina,
    Almacen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Block {
    hex: Axial,
    kind: BlockType,
}

#[derive(Serialize, Deserialize)]
struct MapData {
    blocks: Vec<Block>,
}

fn hex_to_pixel(hex: Axial, size: f32, origin: Vec2) -> Vec2 {
    let q = hex.q as f32;
    let r = hex.r as f32;
    let x = size * (SQRT_3 * q + (SQRT_3 / 2.0) * r);
    let y = size * (1.5 * r);
    origin + vec2(x, y)
}

fn pixel_to_hex(pos: Vec2, size: f32, origin: Vec2) -> Axial {
    let p = pos - origin;
    let q = (SQRT_3 / 3.0 * p.x - 1.0 / 3.0 * p.y) / size;
    let r = (2.0 / 3.0 * p.y) / size;
    axial_round(q, r)
}

fn axial_round(q: f32, r: f32) -> Axial {
    let mut x = q.round();
    let mut z = r.round();
    let mut y = (-q - r).round();

    let x_diff = (x - q).abs();
    let y_diff = (y + q + r).abs();
    let z_diff = (z - r).abs();

    if x_diff > y_diff && x_diff > z_diff {
        x = -y - z;
    } else if y_diff > z_diff {
        y = -x - z;
    } else {
        z = -x - y;
    }

    Axial {
        q: x as i32,
        r: z as i32,
    }
}

fn hex_corners(center: Vec2, size: f32) -> [Vec2; 6] {
    let mut points = [Vec2::ZERO; 6];
    for i in 0..6 {
        let angle = (60.0 * i as f32 - 30.0).to_radians();
        points[i] = center + vec2(size * angle.cos(), size * angle.sin());
    }
    points
}

fn draw_hex_outline(center: Vec2, size: f32, color: Color, thickness: f32) {
    let points = hex_corners(center, size);
    for i in 0..6 {
        let a = points[i];
        let b = points[(i + 1) % 6];
        draw_line(a.x, a.y, b.x, b.y, thickness, color);
    }
}

fn draw_hex_filled(center: Vec2, size: f32, color: Color) {
    let points = hex_corners(center, size);
    for i in 0..6 {
        let a = points[i];
        let b = points[(i + 1) % 6];
        draw_triangle(center, a, b, color);
    }
}

fn hex_distance(a: Axial, b: Axial) -> i32 {
    let dq = (a.q - b.q).abs();
    let dr = (a.r - b.r).abs();
    let ds = (a.q + a.r - b.q - b.r).abs();
    (dq + dr + ds) / 2
}

fn save_map(path: &str, blocks: &HashMap<Axial, BlockType>) {
    let data = MapData {
        blocks: blocks
            .iter()
            .map(|(hex, kind)| Block { hex: *hex, kind: *kind })
            .collect(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&data) {
        let _ = fs::write(path, json);
    }
}

fn load_map(path: &str) -> HashMap<Axial, BlockType> {
    if let Ok(json) = fs::read_to_string(path) {
        if let Ok(data) = serde_json::from_str::<MapData>(&json) {
            return data
                .blocks
                .into_iter()
                .map(|b| (b.hex, b.kind))
                .collect();
        }
    }
    HashMap::new()
}

fn block_color(kind: BlockType) -> Color {
    match kind {
        BlockType::Vivienda => Color::from_rgba(120, 200, 120, 255),
        BlockType::Fabrica => Color::from_rgba(220, 140, 80, 255),
        BlockType::Mina => Color::from_rgba(110, 150, 200, 255),
        BlockType::Almacen => Color::from_rgba(210, 190, 90, 255),
    }
}

#[macroquad::main("GridGame")]
async fn main() {
    let mut blocks: HashMap<Axial, BlockType> = HashMap::new();
    let mut cam_offset = Vec2::ZERO;
    let mut cam_zoom: f32 = 1.0;
    let mut dragging = false;
    let mut last_mouse = Vec2::ZERO;
    let map_path = "map.json";
    let mut selected: Option<BlockType> = Some(BlockType::Vivienda);
    let mut dirty = false;

    loop {
        clear_background(Color::from_rgba(18, 22, 26, 255));

        let mouse = vec2(mouse_position().0, mouse_position().1);
        let screen_center = vec2(screen_width() * 0.5, screen_height() * 0.5);

        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.01 {
            let factor = 1.0 + wheel * 0.001;
            cam_zoom = (cam_zoom * factor).clamp(0.3, 3.0);
        }

        if is_mouse_button_pressed(MouseButton::Middle) {
            dragging = true;
            last_mouse = mouse;
        }
        if is_mouse_button_down(MouseButton::Middle) && dragging {
            let delta = mouse - last_mouse;
            cam_offset += delta;
            last_mouse = mouse;
        }
        if is_mouse_button_released(MouseButton::Middle) {
            dragging = false;
        }

        let world_mouse = (mouse - screen_center - cam_offset) / cam_zoom;
        let hover_hex = pixel_to_hex(world_mouse, HEX_SIZE, Vec2::ZERO);

        if is_mouse_button_pressed(MouseButton::Left) {
            if hex_distance(hover_hex, Axial { q: 0, r: 0 }) <= GRID_RADIUS {
                if let Some(kind) = selected {
                    blocks.insert(hover_hex, kind);
                    dirty = true;
                } else if blocks.remove(&hover_hex).is_some() {
                    dirty = true;
                }
            }
        }

        if is_key_pressed(KeyCode::S) {
            save_map(map_path, &blocks);
            dirty = false;
        }

        if is_key_pressed(KeyCode::L) {
            blocks = load_map(map_path);
            dirty = false;
        }

        if dirty {
            save_map(map_path, &blocks);
            dirty = false;
        }

        if is_key_pressed(KeyCode::Escape) {
            save_map(map_path, &blocks);
            break;
        }

        for r in -GRID_RADIUS..=GRID_RADIUS {
            for q in -GRID_RADIUS..=GRID_RADIUS {
                let hex = Axial { q, r };
                if hex_distance(hex, Axial { q: 0, r: 0 }) > GRID_RADIUS {
                    continue;
                }
                let world_center = hex_to_pixel(hex, HEX_SIZE, Vec2::ZERO);
                let center = screen_center + cam_offset + world_center * cam_zoom;

                let size = HEX_SIZE * cam_zoom;
                if center.x < -size
                    || center.y < -size
                    || center.x > screen_width() + size
                    || center.y > screen_height() + size
                {
                    continue;
                }

                if let Some(kind) = blocks.get(&hex) {
                    draw_hex_filled(
                        center,
                        (HEX_SIZE - 2.5) * cam_zoom,
                        block_color(*kind),
                    );
                }

                draw_hex_outline(
                    center,
                    size,
                    Color::from_rgba(70, 78, 86, 255),
                    1.0,
                );
            }
        }

        let hover_center =
            screen_center + cam_offset + hex_to_pixel(hover_hex, HEX_SIZE, Vec2::ZERO) * cam_zoom;
        draw_hex_outline(
            hover_center,
            HEX_SIZE * cam_zoom,
            Color::from_rgba(255, 210, 90, 255),
            2.0,
        );

        draw_text(
            "Click: colocar  |  Rueda: zoom  |  Boton medio: mover  |  S: guardar  |  L: cargar  |  Esc: salir",
            16.0,
            28.0,
            22.0,
            Color::from_rgba(220, 220, 220, 255),
        );

        let coord_text = format!("Hex: q={} r={}", hover_hex.q, hover_hex.r);
        draw_text(
            &coord_text,
            16.0,
            52.0,
            20.0,
            Color::from_rgba(190, 200, 210, 255),
        );

        let panel_pos = vec2(16.0, screen_height() - 110.0);
        let panel_size = vec2(320.0, 94.0);
        draw_rectangle(
            panel_pos.x,
            panel_pos.y,
            panel_size.x,
            panel_size.y,
            Color::from_rgba(28, 34, 40, 230),
        );
        draw_rectangle_lines(
            panel_pos.x,
            panel_pos.y,
            panel_size.x,
            panel_size.y,
            1.5,
            Color::from_rgba(80, 90, 100, 255),
        );

        let button_size = 52.0;
        let gap = 8.0;
        let mut bx = panel_pos.x + 12.0;
        let by = panel_pos.y + 16.0;

        let buttons: [Option<BlockType>; 5] = [
            None,
            Some(BlockType::Vivienda),
            Some(BlockType::Fabrica),
            Some(BlockType::Mina),
            Some(BlockType::Almacen),
        ];

        for option in buttons {
            let rect = Rect::new(bx, by, button_size, button_size);
            let hover = rect.contains(mouse);
            let selected_now = selected == option;

            let base_color = if selected_now {
                Color::from_rgba(70, 100, 120, 255)
            } else {
                Color::from_rgba(45, 55, 65, 255)
            };
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, base_color);
            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, Color::from_rgba(90, 110, 130, 255));

            if hover && is_mouse_button_pressed(MouseButton::Left) {
                selected = option;
            }

            if let Some(kind) = option {
                let icon_center = vec2(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
                draw_hex_filled(icon_center, 16.0, block_color(kind));
                draw_hex_outline(icon_center, 16.0, Color::from_rgba(20, 25, 30, 255), 1.0);
            } else {
                let pad = 10.0;
                draw_line(
                    rect.x + pad,
                    rect.y + pad,
                    rect.x + rect.w - pad,
                    rect.y + rect.h - pad,
                    3.0,
                    Color::from_rgba(220, 90, 90, 255),
                );
                draw_line(
                    rect.x + rect.w - pad,
                    rect.y + pad,
                    rect.x + pad,
                    rect.y + rect.h - pad,
                    3.0,
                    Color::from_rgba(220, 90, 90, 255),
                );
            }

            if hover {
                draw_rectangle_lines(rect.x - 1.0, rect.y - 1.0, rect.w + 2.0, rect.h + 2.0, 1.0, Color::from_rgba(180, 200, 220, 255));
            }

            bx += button_size + gap;
        }

        next_frame().await;
    }
}
