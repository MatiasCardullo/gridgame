use macroquad::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use crate::core::{FrameContext, Scene};
use crate::core::ui::ui_button;

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

fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut n = (x as i64) * 374761393
        + (y as i64) * 668265263
        + (z as i64) * 2147483647
        + seed as i64 * 69069;
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    let n = n ^ (n >> 16);
    (n as u32) as f32 / u32::MAX as f32
}

fn value_noise3(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let z0 = z.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let z1 = z0 + 1;
    let sx = x - x0 as f32;
    let sy = y - y0 as f32;
    let sz = z - z0 as f32;
    let u = sx * sx * (3.0 - 2.0 * sx);
    let v = sy * sy * (3.0 - 2.0 * sy);
    let w = sz * sz * (3.0 - 2.0 * sz);
    let n000 = hash3(x0, y0, z0, seed);
    let n100 = hash3(x1, y0, z0, seed);
    let n010 = hash3(x0, y1, z0, seed);
    let n110 = hash3(x1, y1, z0, seed);
    let n001 = hash3(x0, y0, z1, seed);
    let n101 = hash3(x1, y0, z1, seed);
    let n011 = hash3(x0, y1, z1, seed);
    let n111 = hash3(x1, y1, z1, seed);
    let nx00 = n000 + (n100 - n000) * u;
    let nx10 = n010 + (n110 - n010) * u;
    let nx01 = n001 + (n101 - n001) * u;
    let nx11 = n011 + (n111 - n011) * u;
    let nxy0 = nx00 + (nx10 - nx00) * v;
    let nxy1 = nx01 + (nx11 - nx01) * v;
    nxy0 + (nxy1 - nxy0) * w
}

fn fbm3(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    for i in 0..5 {
        sum += value_noise3(x * freq, y * freq, z * freq, seed + i) * amp;
        freq *= 2.0;
        amp *= 0.5;
    }
    sum.clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PlanetNoiseConfig {
    pub noise_scale: f32,
    pub height_amp: f32,
    pub height_bias: f32,
    pub lat_bias: f32,
    pub sea_level: f32,
    pub ice_start: f32,
    pub ice_strength: f32,
    pub subdivisions: u8,
}

impl Default for PlanetNoiseConfig {
    fn default() -> Self {
        Self {
            noise_scale: 2.0,
            height_amp: 1.2,
            height_bias: -0.3,
            lat_bias: 0.1,
            sea_level: 0.62,
            ice_start: 0.55,
            ice_strength: 1.6,
            subdivisions: 4,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlanetNoiseSnapshot {
    config: PlanetNoiseConfig,
    sector_values: Vec<f32>,
}

pub struct PlanetState {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target_distance: f32,
    pub target_yaw: f32,
    pub target_pitch: f32,
    pub dragging: bool,
    pub last_mouse: Vec2,
    pub config: PlanetNoiseConfig,
    pub meshes: Vec<Mesh>,
    pub face_normals: Vec<Vec3>,
    pub sector_vertices: [Vec3; 12],
    pub sector_faces: Vec<[usize; 3]>,
    pub sector_values: Vec<f32>,
}

impl PlanetState {
    pub fn new() -> Self {
        let config = PlanetNoiseConfig::default();
        let sector_vertices = align_vertices_to_poles(icosahedron_vertices());
        let sector_faces = build_faces(&sector_vertices, &icosahedron_edges());
        let (meshes, face_normals, sector_values) = build_planet_chunks(
            &sector_vertices,
            &sector_faces,
            config.subdivisions as usize,
            1.6,
            &config,
        );
        Self {
            yaw: 0.0,
            pitch: 0.3,
            distance: 6.0,
            target_distance: 6.0,
            target_yaw: 0.0,
            target_pitch: 0.3,
            dragging: false,
            last_mouse: Vec2::ZERO,
            config,
            meshes,
            face_normals,
            sector_vertices,
            sector_faces,
            sector_values,
        }
    }

    fn regenerate(&mut self, size: u16) {
        let (meshes, face_normals, sector_values) = build_planet_chunks(
            &self.sector_vertices,
            &self.sector_faces,
            self.config.subdivisions as usize,
            1.6,
            &self.config,
        );
        self.meshes = meshes;
        self.face_normals = face_normals;
        self.sector_values = sector_values;
        let _ = size;
    }

    fn save_heightmap(&self, size: u16, path: &str) {
        let snapshot = PlanetNoiseSnapshot {
            config: self.config,
            sector_values: self.sector_values.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
            let _ = fs::write(path, json);
        }
        let _ = size;
    }

    fn load_heightmap(&mut self, path: &str) {
        let Ok(contents) = fs::read_to_string(path) else {
            return;
        };
        let Ok(snapshot) = serde_json::from_str::<PlanetNoiseSnapshot>(&contents) else {
            return;
        };
        self.config = snapshot.config;
        self.sector_values = snapshot.sector_values;
        self.regenerate(512);
    }
}

fn height_value(normal: Vec3, config: &PlanetNoiseConfig) -> f32 {
    let noise = fbm3(
        normal.x * config.noise_scale,
        normal.y * config.noise_scale,
        normal.z * config.noise_scale,
        1337,
    );
    (noise * config.height_amp + config.height_bias + normal.y * config.lat_bias).clamp(0.0, 1.0)
}

fn color_from_height(height: f32, normal: Vec3, config: &PlanetNoiseConfig) -> Color {
    let ice = ((normal.y.abs() - config.ice_start) * config.ice_strength).clamp(0.0, 1.0);
    let (mut r, mut g, mut b) = if height < config.sea_level {
        (0.08, 0.18, 0.42)
    } else if height < config.sea_level + 0.08 {
        (0.12, 0.32, 0.24)
    } else if height < config.sea_level + 0.22 {
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
    Color::new(r, g, b, 1.0)
}

fn subdivide_faces(vertices: &mut Vec<Vec3>, faces: &[[usize; 3]]) -> Vec<[usize; 3]> {
    let mut mid_cache: HashMap<(usize, usize), usize> = HashMap::new();
    let mut new_faces = Vec::with_capacity(faces.len() * 4);

    let mut midpoint = |a: usize, b: usize, vertices: &mut Vec<Vec3>| -> usize {
        let key = if a < b { (a, b) } else { (b, a) };
        if let Some(&idx) = mid_cache.get(&key) {
            return idx;
        }
        let mid = (vertices[a] + vertices[b]) * 0.5;
        let idx = vertices.len();
        vertices.push(mid.normalize());
        mid_cache.insert(key, idx);
        idx
    };

    for face in faces {
        let a = face[0];
        let b = face[1];
        let c = face[2];
        let ab = midpoint(a, b, vertices);
        let bc = midpoint(b, c, vertices);
        let ca = midpoint(c, a, vertices);
        new_faces.push([a, ab, ca]);
        new_faces.push([b, bc, ab]);
        new_faces.push([c, ca, bc]);
        new_faces.push([ab, bc, ca]);
    }

    new_faces
}

fn build_planet_chunks(
    base_vertices: &[Vec3; 12],
    sector_faces: &[[usize; 3]],
    subdivisions: usize,
    radius: f32,
    config: &PlanetNoiseConfig,
) -> (Vec<Mesh>, Vec<Vec3>, Vec<f32>) {
    let mut meshes: Vec<Mesh> = Vec::with_capacity(sector_faces.len());
    let mut face_normals: Vec<Vec3> = Vec::with_capacity(sector_faces.len());

    for face in sector_faces.iter().copied() {
        let mut vertices = vec![
            base_vertices[face[0]],
            base_vertices[face[1]],
            base_vertices[face[2]],
        ];
        let mut faces = vec![[0usize, 1usize, 2usize]];
        for _ in 0..subdivisions {
            faces = subdivide_faces(&mut vertices, &faces);
        }

        let mut mesh_vertices: Vec<Vertex> = Vec::with_capacity(vertices.len());
        for v in vertices.iter() {
            let normal = v.normalize();
            let height = height_value(normal, config);
            let sea_level = config.sea_level;
            let elevation = if height < sea_level {
                (height - sea_level) * 0.12
            } else {
                (height - sea_level) * 0.28
            };
            let pos = normal * (radius + elevation);
            let color = color_from_height(height, normal, config);
            mesh_vertices.push(Vertex::new2(pos, vec2(0.0, 0.0), color));
        }

        let mut indices: Vec<u16> = Vec::with_capacity(faces.len() * 3);
        for f in faces {
            indices.push(f[0] as u16);
            indices.push(f[1] as u16);
            indices.push(f[2] as u16);
        }

        meshes.push(Mesh {
            vertices: mesh_vertices,
            indices,
            texture: None,
        });
        face_normals.push(face_normal(base_vertices, face).normalize());
    }

    let sector_values = sector_faces
        .iter()
        .map(|face| {
            let normal = face_normal(base_vertices, *face).normalize();
            height_value(normal, config)
        })
        .collect();

    (meshes, face_normals, sector_values)
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

pub fn run(ctx: &FrameContext, state: &mut PlanetState, scene: &mut Scene) {
    if is_key_pressed(KeyCode::Escape) {
        *scene = Scene::MainMenu;
        return;
    }

    let mouse = vec2(mouse_position().0, mouse_position().1);
    if is_mouse_button_down(MouseButton::Middle) {
        if !state.dragging {
            state.dragging = true;
            state.last_mouse = mouse;
        } else {
            let delta = mouse - state.last_mouse;
            state.yaw += delta.x * 0.01;
            state.pitch += delta.y * 0.01;
            state.pitch = state.pitch.clamp(-1.3, 1.3);
            state.target_yaw = state.yaw;
            state.target_pitch = state.pitch;
            state.last_mouse = mouse;
        }
    } else {
        state.dragging = false;
    }

    let (_wx, wy) = mouse_wheel();
    if wy.abs() > 0.01 {
        state.target_distance = (state.target_distance - wy * 0.01).clamp(2.0, 20.0);
    }
    state.distance += (state.target_distance - state.distance) * 0.06;
    let yaw_delta = wrap_angle(state.target_yaw - state.yaw);
    state.yaw += yaw_delta * 0.12;
    state.pitch += (state.target_pitch - state.pitch) * 0.12;

    let camera_pos = vec3(
        state.distance * state.yaw.cos() * state.pitch.cos(),
        state.distance * state.pitch.sin(),
        state.distance * state.yaw.sin() * state.pitch.cos(),
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
    let camera_dir = camera_pos.normalize();
    for (index, mesh) in state.meshes.iter().enumerate() {
        if let Some(normal) = state.face_normals.get(index) {
            if normal.dot(camera_dir) <= 0.0 {
                continue;
            }
        }
        draw_mesh(mesh);
    }
    draw_sphere_wires(vec3(0.0, 0.0, 0.0), radius, None, Color::from_rgba(120, 150, 180, 180));

    let vertices = state.sector_vertices;
    let edges = icosahedron_edges();
    let faces = state.sector_faces.clone();
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
            state.target_yaw = normal.z.atan2(normal.x);
            state.target_pitch = normal.y.asin();
        }
    }

    set_default_camera();
    draw_planet_controls(ctx, state);
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

fn draw_planet_controls(ctx: &FrameContext, state: &mut PlanetState) {
    let panel = Rect::new(18.0, 18.0, 260.0, 260.0);
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, ctx.colors_rt.panel_bg);
    draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 1.5, ctx.colors_rt.panel_border);
    draw_text(
        "Planet Terrain",
        panel.x + 12.0,
        panel.y + 24.0,
        ctx.font_md,
        ctx.colors_rt.text_primary,
    );

    let mut y = panel.y + 54.0;
    let x = panel.x + 12.0;
    let value_x = panel.x + panel.w - 90.0;
    let step = 26.0;

    let mut changed = false;
    changed |= draw_adjust_row(
        ctx,
        "Noise",
        &mut state.config.noise_scale,
        0.2,
        0.5,
        5.0,
        x,
        value_x,
        y,
    );
    y += step;
    changed |= draw_adjust_row(
        ctx,
        "Amp",
        &mut state.config.height_amp,
        0.1,
        0.5,
        2.5,
        x,
        value_x,
        y,
    );
    y += step;
    changed |= draw_adjust_row(
        ctx,
        "Bias",
        &mut state.config.height_bias,
        0.05,
        -1.0,
        1.0,
        x,
        value_x,
        y,
    );
    y += step;
    changed |= draw_adjust_row(
        ctx,
        "Lat",
        &mut state.config.lat_bias,
        0.05,
        -0.5,
        0.5,
        x,
        value_x,
        y,
    );
    y += step;
    changed |= draw_adjust_row(
        ctx,
        "Sea",
        &mut state.config.sea_level,
        0.02,
        0.2,
        0.9,
        x,
        value_x,
        y,
    );
    y += step;
    changed |= draw_adjust_row(
        ctx,
        "Ice",
        &mut state.config.ice_start,
        0.02,
        0.2,
        0.9,
        x,
        value_x,
        y,
    );
    y += step;
    changed |= draw_adjust_row(
        ctx,
        "IcePow",
        &mut state.config.ice_strength,
        0.1,
        0.5,
        3.0,
        x,
        value_x,
        y,
    );
    y += step + 6.0;

    let mut detail_changed = false;
    let (clicked_minus, _) = ui_button(
        Rect::new(value_x, y - 16.0, 22.0, 20.0),
        "-",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    let (clicked_plus, _) = ui_button(
        Rect::new(value_x + 54.0, y - 16.0, 22.0, 20.0),
        "+",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    draw_text("Detail", x, y, ctx.font_sm, ctx.colors_rt.text_secondary);
    if clicked_minus {
        state.config.subdivisions = state.config.subdivisions.saturating_sub(1);
        detail_changed = true;
    }
    if clicked_plus {
        state.config.subdivisions = (state.config.subdivisions + 1).min(5);
        detail_changed = true;
    }
    draw_text(
        &format!("{}", state.config.subdivisions),
        value_x + 28.0,
        y,
        ctx.font_sm,
        ctx.colors_rt.text_secondary,
    );
    y += step + 6.0;

    let rect_load = Rect::new(x, y, 110.0, 26.0);
    let (clicked_load, _) = ui_button(
        rect_load,
        "Load JSON",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    let rect_save = Rect::new(x + 120.0, y, 110.0, 26.0);
    let (clicked_save, _) = ui_button(
        rect_save,
        "Save JSON",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );

    if changed || detail_changed {
        state.regenerate(512);
    }
    if clicked_load {
        state.load_heightmap("planet_noise.json");
    }
    if clicked_save {
        state.save_heightmap(512, "planet_noise.json");
    }
}

fn draw_adjust_row(
    ctx: &FrameContext,
    label: &str,
    value: &mut f32,
    step: f32,
    min: f32,
    max: f32,
    x: f32,
    value_x: f32,
    y: f32,
) -> bool {
    draw_text(label, x, y, ctx.font_sm, ctx.colors_rt.text_secondary);
    let rect_minus = Rect::new(value_x, y - 16.0, 22.0, 20.0);
    let rect_plus = Rect::new(value_x + 54.0, y - 16.0, 22.0, 20.0);
    let (clicked_minus, _) = ui_button(
        rect_minus,
        "-",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    let (clicked_plus, _) = ui_button(
        rect_plus,
        "+",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    if clicked_minus {
        *value = (*value - step).max(min);
    }
    if clicked_plus {
        *value = (*value + step).min(max);
    }
    draw_text(
        &format!("{:.2}", *value),
        value_x + 26.0,
        y,
        ctx.font_sm,
        ctx.colors_rt.text_secondary,
    );
    clicked_minus || clicked_plus
}
