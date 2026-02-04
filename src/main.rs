use std::collections::HashSet;

use macroquad::prelude::*;

const HEX_SIZE: f32 = 16.0;
const GRID_RADIUS: i32 = 64;
const SQRT_3: f32 = 1.732_050_8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Axial {
    q: i32,
    r: i32,
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

#[macroquad::main("GridGame")]
async fn main() {
    let mut filled: HashSet<Axial> = HashSet::new();
    let mut cam_offset = Vec2::ZERO;
    let mut cam_zoom: f32 = 1.0;
    let mut dragging = false;
    let mut last_mouse = Vec2::ZERO;

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
            if filled.contains(&hover_hex) {
                filled.remove(&hover_hex);
            } else {
                filled.insert(hover_hex);
            }
        }

        for r in -GRID_RADIUS..=GRID_RADIUS {
            for q in -GRID_RADIUS..=GRID_RADIUS {
                let hex = Axial { q, r };
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

                if filled.contains(&hex) {
                    draw_hex_filled(
                        center,
                        (HEX_SIZE - 2.5) * cam_zoom,
                        Color::from_rgba(80, 170, 255, 255),
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
            "Click: poner/quitar bloque  |  Rueda: zoom  |  Boton medio: mover",
            16.0,
            28.0,
            22.0,
            Color::from_rgba(220, 220, 220, 255),
        );

        next_frame().await;
    }
}
