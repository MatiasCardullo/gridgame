use macroquad::prelude::*;
use std::sync::OnceLock;

use crate::core::Scene;

fn wrap_angle(mut angle: f32) -> f32 {
    let two_pi = std::f32::consts::PI * 2.0;
    angle = (angle + std::f32::consts::PI) % two_pi;
    if angle < 0.0 {
        angle += two_pi;
    }
    angle - std::f32::consts::PI
}

fn rotate_vec3(v: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let axis = axis.normalize();
    let cos_theta = angle.cos();
    let sin_theta = angle.sin();
    v * cos_theta + axis.cross(v) * sin_theta + axis * (axis.dot(v) * (1.0 - cos_theta))
}

fn icosahedron_edges() -> Vec<(usize, usize)> {
    vec![
        (0, 1), (0, 2), (0, 3), (0, 4), (0, 5),
        (5, 1), (1, 2), (2, 3), (3, 4), (4, 5),
        (5, 6), (1, 6), (1, 7), (2, 7), (2, 8),
        (3, 8), (3, 9), (4, 9), (4, 10), (5, 10),
        (10, 6), (6, 7), (7, 8), (8, 9), (9, 10),
        (11, 6), (11, 7), (11, 8), (11, 9), (11, 10),
    ]
}

fn icosahedron_vertices() -> [Vec3; 12] {
    let phi = (1.0 + 5.0_f32.sqrt()) * 0.5;
    [
        vec3(-1.0,  phi, 0.0),
        vec3(0.0,  1.0,  phi),
        vec3( 1.0,  phi, 0.0),
        vec3(0.0,  1.0, -phi),
        vec3(-phi, 0.0, -1.0),
        vec3(-phi, 0.0,  1.0),
        vec3(0.0, -1.0,  phi),
        vec3( phi, 0.0,  1.0),
        vec3( phi, 0.0, -1.0),
        vec3(0.0, -1.0, -phi),
        vec3(-1.0, -phi, 0.0),
        vec3( 1.0, -phi, 0.0),
    ]
}

fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut n = (x as i64) * 374761393 + (y as i64) * 668265263 + seed as i64 * 69069;
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    let n = n ^ (n >> 16);
    (n as u32) as f32 / u32::MAX as f32
}

fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let sx = x - x0 as f32;
    let sy = y - y0 as f32;
    let u = sx * sx * (3.0 - 2.0 * sx);
    let v = sy * sy * (3.0 - 2.0 * sy);
    let n00 = hash2(x0, y0, seed);
    let n10 = hash2(x1, y0, seed);
    let n01 = hash2(x0, y1, seed);
    let n11 = hash2(x1, y1, seed);
    let nx0 = n00 + (n10 - n00) * u;
    let nx1 = n01 + (n11 - n01) * u;
    nx0 + (nx1 - nx0) * v
}

fn fbm(x: f32, y: f32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    for i in 0..5 {
        sum += value_noise(x * freq, y * freq, seed + i) * amp;
        freq *= 2.0;
        amp *= 0.5;
    }
    sum.clamp(0.0, 1.0)
}

fn build_planet_texture(size: u16) -> Texture2D {
    let mut img = Image::gen_image_color(size, size / 2, Color::new(0.0, 0.0, 0.0, 1.0));
    let seed = 1337;
    let width = img.width() as f32;
    let height = img.height() as f32;
    for y in 0..img.height() {
        for x in 0..img.width() {
            let u = x as f32 / width;
            let v = y as f32 / height;
            let lat = (v - 0.5) * std::f32::consts::PI;
            let lon = (u - 0.5) * std::f32::consts::PI * 2.0;
            let nx = lon.cos() * lat.cos();
            let ny = lat.sin();
            let nz = lon.sin() * lat.cos();
            let noise = fbm(nx * 2.2 + 3.7, nz * 2.2 - 1.9, seed);
            let height_val = (noise * 1.1 - 0.25 + ny * 0.15).clamp(0.0, 1.0);
            let ice = (ny.abs() - 0.65).clamp(0.0, 1.0);
            let (mut r, mut g, mut b) = if height_val < 0.45 {
                (0.08, 0.18, 0.42)
            } else if height_val < 0.6 {
                (0.12, 0.32, 0.24)
            } else if height_val < 0.78 {
                (0.22, 0.44, 0.26)
            } else {
                (0.48, 0.46, 0.40)
            };
            if ice > 0.0 {
                let t = ice * ice;
                r = r * (1.0 - t) + 0.85 * t;
                g = g * (1.0 - t) + 0.9 * t;
                b = b * (1.0 - t) + 0.95 * t;
            }
            img.set_pixel(x as u32, y as u32, Color::new(r, g, b, 1.0));
        }
    }
    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Linear);
    tex
}

fn planet_texture() -> Texture2D {
    static TEX: OnceLock<Texture2D> = OnceLock::new();
    TEX.get_or_init(|| build_planet_texture(512)).clone()
}

fn draw_arc_on_sphere(start: Vec3, end: Vec3, radius: f32, segments: usize, color: Color) {
    let start = start.normalize();
    let end = end.normalize();
    let dot = start.dot(end).clamp(-1.0, 1.0);
    let theta = dot.acos();
    let sin_theta = theta.sin();
    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        let p0 = if sin_theta.abs() < 1e-5 {
            (start + (end - start) * t0).normalize()
        } else {
            let w0 = ((1.0 - t0) * theta).sin() / sin_theta;
            let w1 = (t0 * theta).sin() / sin_theta;
            (start * w0 + end * w1).normalize()
        } * radius;
        let p1 = if sin_theta.abs() < 1e-5 {
            (start + (end - start) * t1).normalize()
        } else {
            let w0 = ((1.0 - t1) * theta).sin() / sin_theta;
            let w1 = (t1 * theta).sin() / sin_theta;
            (start * w0 + end * w1).normalize()
        } * radius;
        draw_line_3d(p0, p1, color);
    }
}

fn build_faces(vertices: &[Vec3; 12], edges: &[(usize, usize)]) -> Vec<[usize; 3]> {
    let mut connected = [[false; 12]; 12];
    for (a, b) in edges {
        connected[*a][*b] = true;
        connected[*b][*a] = true;
    }

    let mut faces = Vec::new();
    for i in 0..12 {
        for j in (i + 1)..12 {
            if !connected[i][j] {
                continue;
            }
            for k in (j + 1)..12 {
                if !(connected[i][k] && connected[j][k]) {
                    continue;
                }
                let a = vertices[i];
                let b = vertices[j];
                let c = vertices[k];
                let mut normal = (b - a).cross(c - a);
                if normal.length() < 1e-5 {
                    continue;
                }
                normal = normal.normalize();
                if normal.dot(a) < 0.0 {
                    normal = -normal;
                }
                let mut is_face = true;
                for (idx, v) in vertices.iter().enumerate() {
                    if idx == i || idx == j || idx == k {
                        continue;
                    }
                    if (*v - a).dot(normal) > 1e-4 {
                        is_face = false;
                        break;
                    }
                }
                if is_face {
                    faces.push([i, j, k]);
                }
            }
        }
    }
    faces
}

fn ray_from_mouse(camera: &Camera3D, mouse: Vec2) -> Option<(Vec3, Vec3)> {
    let x = (mouse.x / screen_width()) * 2.0 - 1.0;
    let y = 1.0 - (mouse.y / screen_height()) * 2.0;
    let inv = camera.matrix().inverse();
    let near = inv * vec4(x, y, -1.0, 1.0);
    let far = inv * vec4(x, y, 1.0, 1.0);
    if near.w.abs() < 1e-5 || far.w.abs() < 1e-5 {
        return None;
    }
    let p0 = vec3(near.x / near.w, near.y / near.w, near.z / near.w);
    let p1 = vec3(far.x / far.w, far.y / far.w, far.z / far.w);
    let dir = (p1 - p0).normalize();
    Some((p0, dir))
}

fn ray_sphere_intersection(origin: Vec3, dir: Vec3, radius: f32) -> Option<Vec3> {
    let b = 2.0 * origin.dot(dir);
    let c = origin.dot(origin) - radius * radius;
    let disc = b * b - 4.0 * c;
    if disc < 0.0 {
        return None;
    }
    let sqrt_disc = disc.sqrt();
    let t0 = (-b - sqrt_disc) * 0.5;
    let t1 = (-b + sqrt_disc) * 0.5;
    let t = if t0 > 0.0 { t0 } else { t1 };
    if t <= 0.0 {
        return None;
    }
    Some(origin + dir * t)
}

fn hovered_face(
    hit_point: Vec3,
    faces: &[[usize; 3]],
    vertices: &[Vec3; 12],
) -> Option<usize> {
    let dir = hit_point.normalize();
    let mut best: Option<(f32, usize)> = None;
    for (index, face) in faces.iter().enumerate() {
        let a = vertices[face[0]];
        let b = vertices[face[1]];
        let c = vertices[face[2]];
        let mut normal = (b - a).cross(c - a);
        if normal.length() < 1e-5 {
            continue;
        }
        normal = normal.normalize();
        if normal.dot(a) < 0.0 {
            normal = -normal;
        }
        let score = normal.dot(dir);
        match best {
            None => best = Some((score, index)),
            Some((best_score, _)) if score > best_score => best = Some((score, index)),
            _ => {}
        }
    }
    best.map(|(_, idx)| idx)
}

fn face_normal(vertices: &[Vec3; 12], face: [usize; 3]) -> Vec3 {
    let a = vertices[face[0]];
    let b = vertices[face[1]];
    let c = vertices[face[2]];
    let mut normal = (b - a).cross(c - a);
    if normal.length() < 1e-5 {
        return vec3(0.0, 1.0, 0.0);
    }
    normal = normal.normalize();
    if normal.dot(a) < 0.0 {
        normal = -normal;
    }
    normal
}

fn align_vertices_to_poles(vertices: [Vec3; 12]) -> [Vec3; 12] {
    let mut max_index = 0usize;
    let mut max_y = vertices[0].y;
    for (index, v) in vertices.iter().enumerate() {
        if v.y > max_y {
            max_y = v.y;
            max_index = index;
        }
    }
    let north = vertices[max_index].normalize();
    let up = vec3(0.0, 1.0, 0.0);
    let dot = north.dot(up).clamp(-1.0, 1.0);
    let angle = dot.acos();
    let axis = north.cross(up);
    if axis.length() < 1e-5 || angle.abs() < 1e-5 {
        return vertices;
    }
    let mut out = [Vec3::ZERO; 12];
    for (index, v) in vertices.iter().enumerate() {
        out[index] = rotate_vec3(*v, axis, angle);
    }
    out
}

fn project_to_screen(camera: &Camera3D, point: Vec3) -> Option<Vec2> {
    let mat = camera.matrix();
    let clip = mat * vec4(point.x, point.y, point.z, 1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = vec3(clip.x, clip.y, clip.z) / clip.w;
    if ndc.x.abs() > 1.2 || ndc.y.abs() > 1.2 {
        return None;
    }
    Some(vec2(
        (ndc.x * 0.5 + 0.5) * screen_width(),
        (0.5 - ndc.y * 0.5) * screen_height(),
    ))
}

pub fn run(
    yaw: &mut f32,
    pitch: &mut f32,
    distance: &mut f32,
    target_distance: &mut f32,
    target_yaw: &mut f32,
    target_pitch: &mut f32,
    dragging: &mut bool,
    last_mouse: &mut Vec2,
    scene: &mut Scene,
) {
    if is_key_pressed(KeyCode::Escape) {
        *scene = Scene::MainMenu;
        return;
    }

    let mouse = vec2(mouse_position().0, mouse_position().1);
    if is_mouse_button_down(MouseButton::Middle) {
        if !*dragging {
            *dragging = true;
            *last_mouse = mouse;
        } else {
            let delta = mouse - *last_mouse;
            *yaw += delta.x * 0.01;
            *pitch += delta.y * 0.01;
            *pitch = pitch.clamp(-1.3, 1.3);
            *target_yaw = *yaw;
            *target_pitch = *pitch;
            *last_mouse = mouse;
        }
    } else {
        *dragging = false;
    }

    let (_wx, wy) = mouse_wheel();
    if wy.abs() > 0.01 {
        *target_distance = (*target_distance - wy * 0.01).clamp(2.0, 20.0);
    }
    *distance += (*target_distance - *distance) * 0.06;
    let yaw_delta = wrap_angle(*target_yaw - *yaw);
    *yaw += yaw_delta * 0.12;
    *pitch += (*target_pitch - *pitch) * 0.12;

    let camera_pos = vec3(
        *distance * yaw.cos() * pitch.cos(),
        *distance * pitch.sin(),
        *distance * yaw.sin() * pitch.cos(),
    );
    let camera = Camera3D {
        position: camera_pos,
        target: vec3(0.0, 0.0, 0.0),
        up: vec3(0.0, 1.0, 0.0),
        fovy: 45.0,
        ..Default::default()
    };
    set_camera(&camera);

    let radius = 1.6;
    let tex = planet_texture();
    draw_sphere(vec3(0.0, 0.0, 0.0), radius, Some(&tex), WHITE);
    draw_sphere_wires(vec3(0.0, 0.0, 0.0), radius, None, Color::from_rgba(120, 150, 180, 180));

    let vertices = align_vertices_to_poles(icosahedron_vertices());
    let edges = icosahedron_edges();
    let faces = build_faces(&vertices, &edges);
    let point_color = Color::from_rgba(240, 240, 255, 255);
    let edge_color = Color::from_rgba(200, 220, 250, 255);
    let hover_color = Color::from_rgba(255, 200, 120, 255);

    let mut projected: [Vec3; 12] = [Vec3::ZERO; 12];
    let mut screen_points: [Option<Vec2>; 12] = [None; 12];
    for (index, v) in vertices.iter().enumerate() {
        projected[index] = v.normalize() * radius;
        draw_sphere(projected[index], 0.06, None, point_color);
        screen_points[index] = project_to_screen(&camera, projected[index]);
    }

    let segments = 24;
    for (a, b) in edges.iter().copied() {
        draw_arc_on_sphere(projected[a], projected[b], radius, segments, edge_color);
    }

    let mut hovered: Option<usize> = None;
    if let Some((origin, dir)) = ray_from_mouse(&camera, mouse) {
        if let Some(hit) = ray_sphere_intersection(origin, dir, radius) {
            hovered = hovered_face(hit, &faces, &vertices);
        }
    }
    if let Some(face_index) = hovered {
        let face = faces[face_index];
        let a = projected[face[0]];
        let b = projected[face[1]];
        let c = projected[face[2]];
        draw_arc_on_sphere(a, b, radius, segments, hover_color);
        draw_arc_on_sphere(b, c, radius, segments, hover_color);
        draw_arc_on_sphere(c, a, radius, segments, hover_color);
    }
    if let Some(face_index) = hovered {
        if is_mouse_button_pressed(MouseButton::Left) {
            let face = faces[face_index];
            let normal = face_normal(&vertices, face);
            *target_yaw = normal.z.atan2(normal.x);
            *target_pitch = normal.y.asin();
        }
    }

    set_default_camera();
    for (index, pos) in screen_points.iter().enumerate() {
        if let Some(pos) = pos {
            draw_text(
                &format!("{}", index),
                pos.x + 6.0,
                pos.y - 6.0,
                18.0,
                Color::from_rgba(240, 230, 120, 255),
            );
        }
    }
}
