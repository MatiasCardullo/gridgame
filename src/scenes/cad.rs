use macroquad::prelude::*;
use std::fs;

use crate::core::ui::ui_button;
use crate::core::{FrameContext, RuntimeColors, Scene};

const CAD_OBJ_PATH: &str = "cad_scene.obj";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CadPrimitiveKind {
    Square,
    Cylinder,
    Mesh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CadTool {
    Move,
    Resize,
    Rotate,
    Align,
    Join,
    Subtract,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PairPhase {
    PickSource,
    PickTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AlignMode {
    Center,
    Face,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResizeHandleKind {
    BoxDir(Vec3Sign),
    CylinderTop,
    CylinderBottom,
    CylinderRadial(Vec3Sign),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Vec3Sign {
    x: i8,
    y: i8,
    z: i8,
}

impl Vec3Sign {
    fn as_vec3(self) -> Vec3 {
        vec3(self.x as f32, self.y as f32, self.z as f32)
    }
}

#[derive(Clone, Copy, Debug)]
struct ResizeHandle {
    local_pos: Vec3,
    kind: ResizeHandleKind,
}

#[derive(Clone, Debug)]
struct CadMeshData {
    vertices: Vec<Vec3>,
    triangles: Vec<[usize; 3]>,
}

impl CadMeshData {
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
            triangles: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct CadPart {
    kind: CadPrimitiveKind,
    pos: Vec3,
    size: Vec3,
    rot: Vec3,
    mesh: Option<CadMeshData>,
}

#[derive(Clone, Debug)]
struct CadFigure {
    kind: CadPrimitiveKind,
    pos: Vec3,
    size: Vec3,
    rot: Vec3, // x/y/z radians in local form
    parts: Vec<CadPart>,
    mesh: Option<CadMeshData>,
}

impl CadFigure {
    fn new(kind: CadPrimitiveKind, pos: Vec3, size: Vec3, rot: Vec3) -> Self {
        Self {
            kind,
            pos,
            size,
            rot,
            parts: Vec::new(),
            mesh: None,
        }
    }
}

pub struct CadState {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    figures: Vec<CadFigure>,
    selected: Option<usize>,
    tool: CadTool,
    pair_phase: PairPhase,
    pending_source: Option<usize>,
    selected_resize_handle: usize,
    selected_rotate_axis: usize,
    dragging_handle: bool,
    dragging_axis: bool,
    drag_last_mouse: Vec2,
    align_mode: AlignMode,
    align_face: usize,
    obj_status: String,
}

impl Default for CadState {
    fn default() -> Self {
        Self {
            yaw: 0.95,
            pitch: 0.45,
            distance: 180.0,
            figures: Vec::new(),
            selected: None,
            tool: CadTool::Move,
            pair_phase: PairPhase::PickSource,
            pending_source: None,
            selected_resize_handle: 0,
            selected_rotate_axis: 1,
            dragging_handle: false,
            dragging_axis: false,
            drag_last_mouse: Vec2::ZERO,
            align_mode: AlignMode::Center,
            align_face: 0,
            obj_status: "OBJ: cad_scene.obj".to_string(),
        }
    }
}

fn cad_camera(state: &CadState) -> Camera3D {
    let eye = vec3(
        state.distance * state.yaw.cos() * state.pitch.cos(),
        state.distance * state.pitch.sin(),
        state.distance * state.yaw.sin() * state.pitch.cos(),
    );
    Camera3D {
        position: eye,
        target: vec3(0.0, 20.0, 0.0),
        up: vec3(0.0, 1.0, 0.0),
        fovy: 45.0,
        z_near: 1.0,
        z_far: 2000.0,
        ..Default::default()
    }
}

fn figure_quat(fig: &CadFigure) -> Quat {
    Quat::from_euler(EulerRot::XYZ, fig.rot.x, fig.rot.y, fig.rot.z)
}

fn figure_basis(fig: &CadFigure) -> (Vec3, Vec3, Vec3) {
    let q = figure_quat(fig);
    (q * Vec3::X, q * Vec3::Y, q * Vec3::Z)
}

fn local_to_world(fig: &CadFigure, local: Vec3) -> Vec3 {
    let (x, y, z) = figure_basis(fig);
    fig.pos + x * local.x + y * local.y + z * local.z
}

fn part_quat(part: &CadPart) -> Quat {
    Quat::from_euler(EulerRot::XYZ, part.rot.x, part.rot.y, part.rot.z)
}

fn part_as_world_figure(parent: &CadFigure, part: &CadPart) -> CadFigure {
    let q = (figure_quat(parent) * part_quat(part)).normalize();
    let (rx, ry, rz) = q.to_euler(EulerRot::XYZ);
    let mut fig = CadFigure::new(
        part.kind,
        local_to_world(parent, part.pos),
        part.size,
        vec3(rx, ry, rz),
    );
    fig.mesh = part.mesh.clone();
    fig
}

fn figure_leaf_world_figures(fig: &CadFigure) -> Vec<CadFigure> {
    if fig.parts.is_empty() {
        return vec![CadFigure::new(fig.kind, fig.pos, fig.size, fig.rot)];
    }
    fig.parts
        .iter()
        .map(|part| part_as_world_figure(fig, part))
        .collect()
}

fn draw_oriented_box(fig: &CadFigure, fill: Color, wire: Color) {
    let (x_axis, y_axis, z_axis) = figure_basis(fig);
    let origin = fig.pos
        - x_axis * (fig.size.x * 0.5)
        - y_axis * (fig.size.y * 0.5)
        - z_axis * (fig.size.z * 0.5);
    draw_affine_parallelepiped(
        origin,
        x_axis * fig.size.x,
        y_axis * fig.size.y,
        z_axis * fig.size.z,
        None,
        fill,
    );

    let hx = fig.size.x * 0.5;
    let hy = fig.size.y * 0.5;
    let hz = fig.size.z * 0.5;
    let p000 = local_to_world(fig, vec3(-hx, -hy, -hz));
    let p100 = local_to_world(fig, vec3(hx, -hy, -hz));
    let p010 = local_to_world(fig, vec3(-hx, hy, -hz));
    let p110 = local_to_world(fig, vec3(hx, hy, -hz));
    let p001 = local_to_world(fig, vec3(-hx, -hy, hz));
    let p101 = local_to_world(fig, vec3(hx, -hy, hz));
    let p011 = local_to_world(fig, vec3(-hx, hy, hz));
    let p111 = local_to_world(fig, vec3(hx, hy, hz));
    let edges = [
        (p000, p100),
        (p000, p010),
        (p000, p001),
        (p100, p110),
        (p100, p101),
        (p010, p110),
        (p010, p011),
        (p001, p101),
        (p001, p011),
        (p110, p111),
        (p101, p111),
        (p011, p111),
    ];
    for (a, b) in edges {
        draw_line_3d(a, b, wire);
    }
}

fn draw_oriented_cylinder(fig: &CadFigure, fill: Color, wire: Color) {
    let sides: usize = 24;
    let hy = fig.size.y * 0.5;
    let rx = fig.size.x * 0.5;
    let rz = fig.size.z * 0.5;
    let mut ring_bottom: Vec<Vec3> = Vec::with_capacity(sides);
    let mut ring_top: Vec<Vec3> = Vec::with_capacity(sides);

    for i in 0..sides {
        let a = (i as f32 / sides as f32) * std::f32::consts::TAU;
        let local = vec3(a.sin() * rx, 0.0, a.cos() * rz);
        ring_bottom.push(local_to_world(fig, vec3(local.x, -hy, local.z)));
        ring_top.push(local_to_world(fig, vec3(local.x, hy, local.z)));
    }

    let mut vertices: Vec<Vertex> = Vec::with_capacity(sides * 2 + 2);
    for i in 0..sides {
        vertices.push(Vertex::new2(ring_bottom[i], vec2(0.0, 0.0), fill));
        vertices.push(Vertex::new2(ring_top[i], vec2(0.0, 1.0), fill));
    }
    let bottom_center_idx = vertices.len() as u16;
    vertices.push(Vertex::new2(
        local_to_world(fig, vec3(0.0, -hy, 0.0)),
        vec2(0.5, 0.5),
        fill,
    ));
    let top_center_idx = vertices.len() as u16;
    vertices.push(Vertex::new2(
        local_to_world(fig, vec3(0.0, hy, 0.0)),
        vec2(0.5, 0.5),
        fill,
    ));

    let mut indices: Vec<u16> = Vec::with_capacity(sides * 12);
    for i in 0..sides {
        let ni = (i + 1) % sides;
        let bi = (i * 2) as u16;
        let ti = bi + 1;
        let bni = (ni * 2) as u16;
        let tni = bni + 1;

        indices.extend_from_slice(&[bi, bni, tni, bi, tni, ti]);
        indices.extend_from_slice(&[bottom_center_idx, bni, bi]);
        indices.extend_from_slice(&[top_center_idx, ti, tni]);
    }

    draw_mesh(&Mesh {
        vertices,
        indices,
        texture: None,
    });

    for i in 0..sides {
        let ni = (i + 1) % sides;
        draw_line_3d(ring_bottom[i], ring_bottom[ni], wire);
        draw_line_3d(ring_top[i], ring_top[ni], wire);
        if i % 6 == 0 {
            draw_line_3d(ring_bottom[i], ring_top[i], wire);
        }
    }

    // seam line shows local yaw rotation on symmetric body.
    draw_line_3d(
        local_to_world(fig, vec3(rx, -hy, 0.0)),
        local_to_world(fig, vec3(rx, hy, 0.0)),
        wire,
    );
}

fn draw_cad_mesh(fig: &CadFigure, mesh: &CadMeshData, fill: Color, wire: Color) {
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    for tri in &mesh.triangles {
        if vertices.len() + 3 >= u16::MAX as usize {
            draw_mesh(&Mesh {
                vertices,
                indices,
                texture: None,
            });
            vertices = Vec::new();
            indices = Vec::new();
        }
        let base = vertices.len() as u16;
        for idx in tri {
            let Some(local) = mesh.vertices.get(*idx) else {
                continue;
            };
            vertices.push(Vertex::new2(
                local_to_world(fig, *local),
                vec2(0.0, 0.0),
                fill,
            ));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    if !vertices.is_empty() {
        draw_mesh(&Mesh {
            vertices,
            indices,
            texture: None,
        });
    }

    for tri in &mesh.triangles {
        let Some(a) = mesh.vertices.get(tri[0]) else {
            continue;
        };
        let Some(b) = mesh.vertices.get(tri[1]) else {
            continue;
        };
        let Some(c) = mesh.vertices.get(tri[2]) else {
            continue;
        };
        let aw = local_to_world(fig, *a);
        let bw = local_to_world(fig, *b);
        let cw = local_to_world(fig, *c);
        draw_line_3d(aw, bw, wire);
        draw_line_3d(bw, cw, wire);
        draw_line_3d(cw, aw, wire);
    }
}

fn draw_figure_body(fig: &CadFigure, fill: Color, wire: Color) {
    match fig.kind {
        CadPrimitiveKind::Square => draw_oriented_box(fig, fill, wire),
        CadPrimitiveKind::Cylinder => draw_oriented_cylinder(fig, fill, wire),
        CadPrimitiveKind::Mesh => {
            if let Some(mesh) = &fig.mesh {
                draw_cad_mesh(fig, mesh, fill, wire);
            }
        }
    }
}

fn draw_figure(fig: &CadFigure, selected: bool) {
    let (fill, wire) = if selected {
        (
            Color::from_rgba(125, 210, 255, 255),
            Color::from_rgba(250, 255, 255, 255),
        )
    } else {
        (
            Color::from_rgba(95, 145, 180, 215),
            Color::from_rgba(170, 200, 225, 255),
        )
    };
    if fig.parts.is_empty() {
        draw_figure_body(fig, fill, wire);
        return;
    }
    for part in &fig.parts {
        let part_fig = part_as_world_figure(fig, part);
        draw_figure_body(&part_fig, fill, wire);
    }
}

fn box_handles(fig: &CadFigure) -> Vec<ResizeHandle> {
    let mut out = Vec::new();
    let half = fig.size * 0.5;
    for sx in [-1_i8, 0, 1] {
        for sy in [-1_i8, 0, 1] {
            for sz in [-1_i8, 0, 1] {
                if sx == 0 && sy == 0 && sz == 0 {
                    continue;
                }
                let sign = Vec3Sign {
                    x: sx,
                    y: sy,
                    z: sz,
                };
                out.push(ResizeHandle {
                    local_pos: vec3(half.x * sx as f32, half.y * sy as f32, half.z * sz as f32),
                    kind: ResizeHandleKind::BoxDir(sign),
                });
            }
        }
    }
    out
}

fn cylinder_handles(fig: &CadFigure) -> Vec<ResizeHandle> {
    let hy = fig.size.y * 0.5;
    let r = (fig.size.x + fig.size.z) * 0.25;
    vec![
        ResizeHandle {
            local_pos: vec3(0.0, hy, 0.0),
            kind: ResizeHandleKind::CylinderTop,
        },
        ResizeHandle {
            local_pos: vec3(0.0, -hy, 0.0),
            kind: ResizeHandleKind::CylinderBottom,
        },
        ResizeHandle {
            local_pos: vec3(r, 0.0, 0.0),
            kind: ResizeHandleKind::CylinderRadial(Vec3Sign { x: 1, y: 0, z: 0 }),
        },
        ResizeHandle {
            local_pos: vec3(-r, 0.0, 0.0),
            kind: ResizeHandleKind::CylinderRadial(Vec3Sign { x: -1, y: 0, z: 0 }),
        },
        ResizeHandle {
            local_pos: vec3(0.0, 0.0, r),
            kind: ResizeHandleKind::CylinderRadial(Vec3Sign { x: 0, y: 0, z: 1 }),
        },
        ResizeHandle {
            local_pos: vec3(0.0, 0.0, -r),
            kind: ResizeHandleKind::CylinderRadial(Vec3Sign { x: 0, y: 0, z: -1 }),
        },
    ]
}

fn resize_handles(fig: &CadFigure) -> Vec<ResizeHandle> {
    match fig.kind {
        CadPrimitiveKind::Square | CadPrimitiveKind::Mesh => box_handles(fig),
        CadPrimitiveKind::Cylinder => cylinder_handles(fig),
    }
}

fn handle_world_positions(fig: &CadFigure) -> Vec<Vec3> {
    resize_handles(fig)
        .into_iter()
        .map(|h| local_to_world(fig, h.local_pos))
        .collect()
}

fn axis_world_positions(fig: &CadFigure) -> [Vec3; 3] {
    let (x, y, z) = figure_basis(fig);
    let arm = fig.size.max_element() * 0.65 + 12.0;
    [fig.pos + x * arm, fig.pos + y * arm, fig.pos + z * arm]
}

fn project_world_to_screen(camera: &Camera3D, point: Vec3) -> Option<Vec2> {
    let clip = camera.matrix() * vec4(point.x, point.y, point.z, 1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = vec3(clip.x / clip.w, clip.y / clip.w, clip.z / clip.w);
    Some(vec2(
        (ndc.x * 0.5 + 0.5) * screen_width(),
        (0.5 - ndc.y * 0.5) * screen_height(),
    ))
}

fn pick_closest_index(
    camera: &Camera3D,
    mouse: Vec2,
    points: &[Vec3],
    max_px: f32,
) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for (i, p) in points.iter().enumerate() {
        let Some(s) = project_world_to_screen(camera, *p) else {
            continue;
        };
        let d = s.distance(mouse);
        if d > max_px {
            continue;
        }
        match best {
            Some((best_d, _)) if d >= best_d => {}
            _ => best = Some((d, i)),
        }
    }
    best.map(|(_, i)| i)
}

fn mouse_world_ray(camera: &Camera3D, mouse: Vec2) -> (Vec3, Vec3) {
    let forward = (camera.target - camera.position).normalize_or_zero();
    let right = forward.cross(camera.up).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    let aspect = screen_width() / screen_height().max(1.0);
    let fovy = camera.fovy.to_radians();
    let sx = mouse.x / screen_width().max(1.0) * 2.0 - 1.0;
    let sy = 1.0 - mouse.y / screen_height().max(1.0) * 2.0;
    let tan_half = (fovy * 0.5).tan();
    let dir =
        (forward + right * (sx * tan_half * aspect) + up * (sy * tan_half)).normalize_or_zero();
    (camera.position, dir)
}

fn ray_box_t(origin: Vec3, dir: Vec3, half: Vec3) -> Option<f32> {
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;
    for (o, d, h) in [
        (origin.x, dir.x, half.x),
        (origin.y, dir.y, half.y),
        (origin.z, dir.z, half.z),
    ] {
        if d.abs() < 0.00001 {
            if o < -h || o > h {
                return None;
            }
            continue;
        }
        let mut t1 = (-h - o) / d;
        let mut t2 = (h - o) / d;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_min > t_max {
            return None;
        }
    }
    if t_max < 0.0 {
        None
    } else {
        Some(t_min.max(0.0))
    }
}

fn ray_cylinder_t(origin: Vec3, dir: Vec3, size: Vec3) -> Option<f32> {
    let rx = (size.x * 0.5).max(0.001);
    let rz = (size.z * 0.5).max(0.001);
    let hy = size.y * 0.5;
    let mut best: Option<f32> = None;

    let a = (dir.x * dir.x) / (rx * rx) + (dir.z * dir.z) / (rz * rz);
    let b = 2.0 * ((origin.x * dir.x) / (rx * rx) + (origin.z * dir.z) / (rz * rz));
    let c = (origin.x * origin.x) / (rx * rx) + (origin.z * origin.z) / (rz * rz) - 1.0;
    let disc = b * b - 4.0 * a * c;
    if a.abs() > 0.00001 && disc >= 0.0 {
        let root = disc.sqrt();
        for t in [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)] {
            let y = origin.y + dir.y * t;
            if t >= 0.0 && y >= -hy && y <= hy {
                best = Some(best.map_or(t, |old| old.min(t)));
            }
        }
    }

    if dir.y.abs() > 0.00001 {
        for y in [-hy, hy] {
            let t = (y - origin.y) / dir.y;
            let x = origin.x + dir.x * t;
            let z = origin.z + dir.z * t;
            if t >= 0.0 && (x / rx).powi(2) + (z / rz).powi(2) <= 1.0 {
                best = Some(best.map_or(t, |old| old.min(t)));
            }
        }
    }

    best
}

fn ray_mesh_t(origin: Vec3, dir: Vec3, mesh: &CadMeshData) -> Option<f32> {
    let mut best: Option<f32> = None;
    for tri in &mesh.triangles {
        let Some((a, b, c)) = triangle_points(mesh, *tri) else {
            continue;
        };
        if let Some(t) = ray_triangle_t(origin, dir, a, b, c) {
            best = Some(best.map_or(t, |old| old.min(t)));
        }
    }
    best
}

fn ray_leaf_t(fig: &CadFigure, ray_origin: Vec3, ray_dir: Vec3) -> Option<f32> {
    let local_origin = world_to_local(fig, ray_origin);
    let local_dir = figure_quat(fig).conjugate() * ray_dir;
    match fig.kind {
        CadPrimitiveKind::Square => ray_box_t(local_origin, local_dir, fig.size * 0.5),
        CadPrimitiveKind::Cylinder => ray_cylinder_t(local_origin, local_dir, fig.size),
        CadPrimitiveKind::Mesh => fig
            .mesh
            .as_ref()
            .and_then(|mesh| ray_mesh_t(local_origin, local_dir, mesh)),
    }
}

fn ray_figure_t(fig: &CadFigure, ray_origin: Vec3, ray_dir: Vec3) -> Option<f32> {
    if fig.parts.is_empty() {
        return ray_leaf_t(fig, ray_origin, ray_dir);
    }
    figure_leaf_world_figures(fig)
        .iter()
        .filter_map(|leaf| ray_leaf_t(leaf, ray_origin, ray_dir))
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
}

fn pick_figure_by_body(camera: &Camera3D, mouse: Vec2, figures: &[CadFigure]) -> Option<usize> {
    let (origin, dir) = mouse_world_ray(camera, mouse);
    figures
        .iter()
        .enumerate()
        .filter_map(|(i, fig)| ray_figure_t(fig, origin, dir).map(|t| (i, t)))
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
}

fn apply_move_tool(fig: &mut CadFigure, dt: f32) {
    let speed = if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
        52.0
    } else {
        28.0
    };
    if is_key_down(KeyCode::Left) {
        fig.pos.x -= speed * dt;
    }
    if is_key_down(KeyCode::Right) {
        fig.pos.x += speed * dt;
    }
    if is_key_down(KeyCode::Up) {
        fig.pos.z -= speed * dt;
    }
    if is_key_down(KeyCode::Down) {
        fig.pos.z += speed * dt;
    }
    if is_key_down(KeyCode::Q) {
        fig.pos.y += speed * dt;
    }
    if is_key_down(KeyCode::E) {
        fig.pos.y -= speed * dt;
    }
}

fn apply_box_handle_local_delta(fig: &mut CadFigure, h: Vec3, local_delta: Vec3) {
    let min_size = 8.0;
    let old_size = fig.size;
    let old_min = -old_size * 0.5;

    let mut min_v = -fig.size * 0.5;
    let mut max_v = fig.size * 0.5;

    let apply_axis = |h_axis: f32, delta_axis: f32, min_axis: &mut f32, max_axis: &mut f32| {
        if h_axis > 0.0 {
            *max_axis += delta_axis;
            if *max_axis - *min_axis < min_size {
                *max_axis = *min_axis + min_size;
            }
        } else if h_axis < 0.0 {
            *min_axis += delta_axis;
            if *max_axis - *min_axis < min_size {
                *min_axis = *max_axis - min_size;
            }
        }
    };

    apply_axis(h.x, local_delta.x, &mut min_v.x, &mut max_v.x);
    apply_axis(h.y, local_delta.y, &mut min_v.y, &mut max_v.y);
    apply_axis(h.z, local_delta.z, &mut min_v.z, &mut max_v.z);

    let new_size = max_v - min_v;
    let center_shift_local = (max_v + min_v) * 0.5;
    if !fig.parts.is_empty() {
        let sx = if old_size.x.abs() > 0.001 {
            new_size.x / old_size.x
        } else {
            1.0
        };
        let sy = if old_size.y.abs() > 0.001 {
            new_size.y / old_size.y
        } else {
            1.0
        };
        let sz = if old_size.z.abs() > 0.001 {
            new_size.z / old_size.z
        } else {
            1.0
        };
        for part in &mut fig.parts {
            part.pos = vec3(
                (part.pos.x - old_min.x) * sx - new_size.x * 0.5,
                (part.pos.y - old_min.y) * sy - new_size.y * 0.5,
                (part.pos.z - old_min.z) * sz - new_size.z * 0.5,
            );
            part.size = vec3(
                (part.size.x * sx.abs()).max(0.1),
                (part.size.y * sy.abs()).max(0.1),
                (part.size.z * sz.abs()).max(0.1),
            );
        }
    }
    let (x, y, z) = figure_basis(fig);
    fig.pos += x * center_shift_local.x + y * center_shift_local.y + z * center_shift_local.z;
    fig.size = new_size;
}

fn apply_cylinder_height(fig: &mut CadFigure, top: bool, local_delta: f32) {
    let min_h = 8.0;
    let mut min_y = -fig.size.y * 0.5;
    let mut max_y = fig.size.y * 0.5;
    if top {
        max_y += local_delta;
        if max_y - min_y < min_h {
            max_y = min_y + min_h;
        }
    } else {
        min_y += local_delta;
        if max_y - min_y < min_h {
            min_y = max_y - min_h;
        }
    }
    let shift_local = (max_y + min_y) * 0.5;
    let (_, y_axis, _) = figure_basis(fig);
    fig.pos += y_axis * shift_local;
    fig.size.y = max_y - min_y;
}

fn apply_cylinder_radial(fig: &mut CadFigure, dir_sign: Vec3Sign, local_delta: Vec3) {
    let min_r = 4.0;
    let dir = dir_sign.as_vec3().normalize_or_zero();
    let mut radius = (fig.size.x + fig.size.z) * 0.25;
    let delta = local_delta.dot(dir) * 0.5;
    let new_radius = (radius + delta).max(min_r);
    let center_shift_local = dir * (new_radius - radius);
    let (x, y, z) = figure_basis(fig);
    fig.pos += x * center_shift_local.x + y * center_shift_local.y + z * center_shift_local.z;
    radius = new_radius;
    fig.size.x = radius * 2.0;
    fig.size.z = radius * 2.0;
}

fn apply_resize_with_handle(fig: &mut CadFigure, handle: ResizeHandleKind, local_delta: Vec3) {
    match handle {
        ResizeHandleKind::BoxDir(sign) => {
            apply_box_handle_local_delta(fig, sign.as_vec3(), local_delta)
        }
        ResizeHandleKind::CylinderTop => apply_cylinder_height(fig, true, local_delta.y),
        ResizeHandleKind::CylinderBottom => apply_cylinder_height(fig, false, local_delta.y),
        ResizeHandleKind::CylinderRadial(sign) => apply_cylinder_radial(fig, sign, local_delta),
    }
}

fn resize_keyboard_delta(dt: f32) -> Vec3 {
    let speed = if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
        24.0
    } else {
        14.0
    };
    let mut out = Vec3::ZERO;
    if is_key_down(KeyCode::Left) {
        out.x -= speed * dt;
    }
    if is_key_down(KeyCode::Right) {
        out.x += speed * dt;
    }
    if is_key_down(KeyCode::Q) {
        out.y += speed * dt;
    }
    if is_key_down(KeyCode::E) {
        out.y -= speed * dt;
    }
    if is_key_down(KeyCode::Up) {
        out.z -= speed * dt;
    }
    if is_key_down(KeyCode::Down) {
        out.z += speed * dt;
    }
    out
}

fn figure_extent_along_world_normal(fig: &CadFigure, normal: Vec3) -> f32 {
    let (x, y, z) = figure_basis(fig);
    0.5 * (fig.size.x * normal.dot(x).abs()
        + fig.size.y * normal.dot(y).abs()
        + fig.size.z * normal.dot(z).abs())
}

fn align_face_normal(target: &CadFigure, face_idx: usize) -> Vec3 {
    let (x, y, z) = figure_basis(target);
    match face_idx % 6 {
        0 => x,
        1 => -x,
        2 => y,
        3 => -y,
        4 => z,
        _ => -z,
    }
}

fn apply_align(
    figures: &mut [CadFigure],
    source_idx: usize,
    target_idx: usize,
    mode: AlignMode,
    face_idx: usize,
) {
    if source_idx >= figures.len() || target_idx >= figures.len() || source_idx == target_idx {
        return;
    }
    let target = figures[target_idx].clone();
    let source = &mut figures[source_idx];
    match mode {
        AlignMode::Center => {
            source.pos = target.pos;
        }
        AlignMode::Face => {
            let normal = align_face_normal(&target, face_idx).normalize_or_zero();
            let target_extent = figure_extent_along_world_normal(&target, normal);
            let source_extent = figure_extent_along_world_normal(source, normal);
            source.pos = target.pos + normal * (target_extent + source_extent);
        }
    }
}

fn world_to_local(fig: &CadFigure, world: Vec3) -> Vec3 {
    let q = figure_quat(fig);
    q.conjugate() * (world - fig.pos)
}

fn push_mesh_triangle(mesh: &mut CadMeshData, a: Vec3, b: Vec3, c: Vec3) {
    let base = mesh.vertices.len();
    mesh.vertices.extend_from_slice(&[a, b, c]);
    mesh.triangles.push([base, base + 1, base + 2]);
}

fn push_mesh_quad(mesh: &mut CadMeshData, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
    push_mesh_triangle(mesh, a, b, c);
    push_mesh_triangle(mesh, a, c, d);
}

fn add_grid_face(mesh: &mut CadMeshData, corner: Vec3, u: Vec3, v: Vec3, steps: usize) {
    let steps = steps.max(1);
    for i in 0..steps {
        for j in 0..steps {
            let u0 = i as f32 / steps as f32;
            let u1 = (i + 1) as f32 / steps as f32;
            let v0 = j as f32 / steps as f32;
            let v1 = (j + 1) as f32 / steps as f32;
            let a = corner + u * u0 + v * v0;
            let b = corner + u * u1 + v * v0;
            let c = corner + u * u1 + v * v1;
            let d = corner + u * u0 + v * v1;
            push_mesh_quad(mesh, a, b, c, d);
        }
    }
}

fn box_mesh_local(size: Vec3, steps: usize) -> CadMeshData {
    let mut mesh = CadMeshData::new();
    let hx = size.x * 0.5;
    let hy = size.y * 0.5;
    let hz = size.z * 0.5;
    add_grid_face(
        &mut mesh,
        vec3(hx, -hy, -hz),
        vec3(0.0, 0.0, size.z),
        vec3(0.0, size.y, 0.0),
        steps,
    );
    add_grid_face(
        &mut mesh,
        vec3(-hx, -hy, hz),
        vec3(0.0, 0.0, -size.z),
        vec3(0.0, size.y, 0.0),
        steps,
    );
    add_grid_face(
        &mut mesh,
        vec3(-hx, hy, -hz),
        vec3(size.x, 0.0, 0.0),
        vec3(0.0, 0.0, size.z),
        steps,
    );
    add_grid_face(
        &mut mesh,
        vec3(-hx, -hy, hz),
        vec3(size.x, 0.0, 0.0),
        vec3(0.0, 0.0, -size.z),
        steps,
    );
    add_grid_face(
        &mut mesh,
        vec3(-hx, -hy, hz),
        vec3(size.x, 0.0, 0.0),
        vec3(0.0, size.y, 0.0),
        steps,
    );
    add_grid_face(
        &mut mesh,
        vec3(hx, -hy, -hz),
        vec3(-size.x, 0.0, 0.0),
        vec3(0.0, size.y, 0.0),
        steps,
    );
    mesh
}

fn cylinder_mesh_local(size: Vec3, sides: usize, y_steps: usize) -> CadMeshData {
    let mut mesh = CadMeshData::new();
    let sides = sides.max(12);
    let y_steps = y_steps.max(1);
    let rx = size.x * 0.5;
    let rz = size.z * 0.5;
    let hy = size.y * 0.5;

    for yi in 0..y_steps {
        let y0 = -hy + size.y * yi as f32 / y_steps as f32;
        let y1 = -hy + size.y * (yi + 1) as f32 / y_steps as f32;
        for i in 0..sides {
            let a0 = i as f32 / sides as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / sides as f32 * std::f32::consts::TAU;
            let p00 = vec3(a0.sin() * rx, y0, a0.cos() * rz);
            let p10 = vec3(a1.sin() * rx, y0, a1.cos() * rz);
            let p11 = vec3(a1.sin() * rx, y1, a1.cos() * rz);
            let p01 = vec3(a0.sin() * rx, y1, a0.cos() * rz);
            push_mesh_quad(&mut mesh, p00, p10, p11, p01);
        }
    }

    let bottom = vec3(0.0, -hy, 0.0);
    let top = vec3(0.0, hy, 0.0);
    for i in 0..sides {
        let a0 = i as f32 / sides as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / sides as f32 * std::f32::consts::TAU;
        let b0 = vec3(a0.sin() * rx, -hy, a0.cos() * rz);
        let b1 = vec3(a1.sin() * rx, -hy, a1.cos() * rz);
        let t0 = vec3(a0.sin() * rx, hy, a0.cos() * rz);
        let t1 = vec3(a1.sin() * rx, hy, a1.cos() * rz);
        push_mesh_triangle(&mut mesh, bottom, b1, b0);
        push_mesh_triangle(&mut mesh, top, t0, t1);
    }
    mesh
}

fn local_mesh_for_figure(fig: &CadFigure, csg: bool) -> Option<CadMeshData> {
    match fig.kind {
        CadPrimitiveKind::Square => Some(box_mesh_local(fig.size, if csg { 24 } else { 1 })),
        CadPrimitiveKind::Cylinder => Some(cylinder_mesh_local(
            fig.size,
            if csg { 48 } else { 32 },
            if csg { 12 } else { 1 },
        )),
        CadPrimitiveKind::Mesh => fig.mesh.clone(),
    }
}

fn world_mesh_for_leaf(fig: &CadFigure, csg: bool) -> Option<CadMeshData> {
    let mut mesh = local_mesh_for_figure(fig, csg)?;
    for v in &mut mesh.vertices {
        *v = local_to_world(fig, *v);
    }
    Some(mesh)
}

fn figure_world_meshes(fig: &CadFigure, csg: bool) -> Vec<CadMeshData> {
    figure_leaf_world_figures(fig)
        .iter()
        .filter_map(|leaf| world_mesh_for_leaf(leaf, csg))
        .collect()
}

fn triangle_points(mesh: &CadMeshData, tri: [usize; 3]) -> Option<(Vec3, Vec3, Vec3)> {
    Some((
        *mesh.vertices.get(tri[0])?,
        *mesh.vertices.get(tri[1])?,
        *mesh.vertices.get(tri[2])?,
    ))
}

fn triangle_centroid(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    (a + b + c) / 3.0
}

fn append_world_triangle(out: &mut CadMeshData, a: Vec3, b: Vec3, c: Vec3, flip: bool) {
    if flip {
        push_mesh_triangle(out, c, b, a);
    } else {
        push_mesh_triangle(out, a, b, c);
    }
}

fn figure_from_world_mesh(world_mesh: CadMeshData) -> Option<CadFigure> {
    if world_mesh.vertices.is_empty() || world_mesh.triangles.is_empty() {
        return None;
    }

    let mut min_v = vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max_v = vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for v in &world_mesh.vertices {
        min_v = min_v.min(*v);
        max_v = max_v.max(*v);
    }

    let center = (min_v + max_v) * 0.5;
    let mut local_mesh = world_mesh;
    for v in &mut local_mesh.vertices {
        *v -= center;
    }

    let mut fig = CadFigure::new(
        CadPrimitiveKind::Mesh,
        center,
        (max_v - min_v).max(vec3(0.1, 0.1, 0.1)),
        Vec3::ZERO,
    );
    fig.mesh = Some(local_mesh);
    Some(fig)
}

fn ray_triangle_t(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let edge1 = b - a;
    let edge2 = c - a;
    let h = dir.cross(edge2);
    let det = edge1.dot(h);
    if det.abs() < 0.00001 {
        return None;
    }
    let inv_det = 1.0 / det;
    let s = origin - a;
    let u = inv_det * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(edge1);
    let v = inv_det * dir.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = inv_det * edge2.dot(q);
    if t > 0.0001 {
        Some(t)
    } else {
        None
    }
}

fn point_inside_mesh(mesh: &CadMeshData, local_point: Vec3) -> bool {
    let dir = vec3(1.0, 0.137, 0.061).normalize();
    let mut hits = Vec::new();
    for tri in &mesh.triangles {
        let Some((a, b, c)) = triangle_points(mesh, *tri) else {
            continue;
        };
        if let Some(t) = ray_triangle_t(local_point, dir, a, b, c) {
            hits.push(t);
        }
    }
    hits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    hits.dedup_by(|a, b| (*a - *b).abs() < 0.001);
    hits.len() % 2 == 1
}

fn point_inside_leaf(fig: &CadFigure, world_point: Vec3) -> bool {
    let p = world_to_local(fig, world_point);
    match fig.kind {
        CadPrimitiveKind::Square => {
            let h = fig.size * 0.5 + vec3(0.001, 0.001, 0.001);
            p.x.abs() <= h.x && p.y.abs() <= h.y && p.z.abs() <= h.z
        }
        CadPrimitiveKind::Cylinder => {
            let hy = fig.size.y * 0.5 + 0.001;
            let rx = (fig.size.x * 0.5).max(0.001);
            let rz = (fig.size.z * 0.5).max(0.001);
            p.y.abs() <= hy && (p.x / rx).powi(2) + (p.z / rz).powi(2) <= 1.001
        }
        CadPrimitiveKind::Mesh => fig
            .mesh
            .as_ref()
            .is_some_and(|mesh| point_inside_mesh(mesh, p)),
    }
}

fn point_inside_figure(fig: &CadFigure, world_point: Vec3) -> bool {
    if fig.parts.is_empty() {
        return point_inside_leaf(fig, world_point);
    }
    figure_leaf_world_figures(fig)
        .iter()
        .any(|leaf| point_inside_leaf(leaf, world_point))
}

fn point_strict_inside_leaf(fig: &CadFigure, world_point: Vec3, margin: f32) -> bool {
    let p = world_to_local(fig, world_point);
    match fig.kind {
        CadPrimitiveKind::Square => {
            let h = fig.size * 0.5 - vec3(margin, margin, margin);
            h.x > 0.0
                && h.y > 0.0
                && h.z > 0.0
                && p.x.abs() < h.x
                && p.y.abs() < h.y
                && p.z.abs() < h.z
        }
        CadPrimitiveKind::Cylinder => {
            let hy = fig.size.y * 0.5 - margin;
            let rx = (fig.size.x * 0.5 - margin).max(0.001);
            let rz = (fig.size.z * 0.5 - margin).max(0.001);
            hy > 0.0 && p.y.abs() < hy && (p.x / rx).powi(2) + (p.z / rz).powi(2) < 1.0
        }
        CadPrimitiveKind::Mesh => point_inside_leaf(fig, world_point),
    }
}

fn point_strict_inside_figure(fig: &CadFigure, world_point: Vec3, margin: f32) -> bool {
    if fig.parts.is_empty() {
        return point_strict_inside_leaf(fig, world_point, margin);
    }
    figure_leaf_world_figures(fig)
        .iter()
        .any(|leaf| point_strict_inside_leaf(leaf, world_point, margin))
}

fn csg_join_mesh(base: &CadFigure, tool: &CadFigure) -> Option<CadMeshData> {
    let mut out = CadMeshData::new();
    for mesh in figure_world_meshes(base, true) {
        for tri in &mesh.triangles {
            let Some((a, b, c)) = triangle_points(&mesh, *tri) else {
                continue;
            };
            if !point_inside_figure(tool, triangle_centroid(a, b, c)) {
                append_world_triangle(&mut out, a, b, c, false);
            }
        }
    }
    for mesh in figure_world_meshes(tool, true) {
        for tri in &mesh.triangles {
            let Some((a, b, c)) = triangle_points(&mesh, *tri) else {
                continue;
            };
            if !point_inside_figure(base, triangle_centroid(a, b, c)) {
                append_world_triangle(&mut out, a, b, c, false);
            }
        }
    }
    if out.triangles.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn csg_subtract_mesh(base: &CadFigure, tool: &CadFigure) -> Option<CadMeshData> {
    let mut out = CadMeshData::new();
    for mesh in figure_world_meshes(base, true) {
        for tri in &mesh.triangles {
            let Some((a, b, c)) = triangle_points(&mesh, *tri) else {
                continue;
            };
            if !point_inside_figure(tool, triangle_centroid(a, b, c)) {
                append_world_triangle(&mut out, a, b, c, false);
            }
        }
    }
    for mesh in figure_world_meshes(tool, true) {
        for tri in &mesh.triangles {
            let Some((a, b, c)) = triangle_points(&mesh, *tri) else {
                continue;
            };
            if point_strict_inside_figure(base, triangle_centroid(a, b, c), 0.05) {
                append_world_triangle(&mut out, a, b, c, true);
            }
        }
    }
    if out.triangles.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn apply_join(base: CadFigure, tool: CadFigure) -> Option<CadFigure> {
    csg_join_mesh(&base, &tool).and_then(figure_from_world_mesh)
}

fn apply_subtract(base: CadFigure, tool: CadFigure) -> Option<CadFigure> {
    csg_subtract_mesh(&base, &tool).and_then(figure_from_world_mesh)
}

fn replace_pair_with_result(
    figures: &mut Vec<CadFigure>,
    a: usize,
    b: usize,
    result: Option<CadFigure>,
) -> Option<usize> {
    if a >= figures.len() || b >= figures.len() || a == b {
        return None;
    }
    let hi = a.max(b);
    let lo = a.min(b);
    figures.remove(hi);
    figures.remove(lo);
    if let Some(fig) = result {
        figures.push(fig);
        Some(figures.len().saturating_sub(1))
    } else {
        None
    }
}

fn switch_tool(state: &mut CadState, tool: CadTool) {
    state.tool = tool;
    state.dragging_axis = false;
    state.dragging_handle = false;
    state.pending_source = None;
    state.pair_phase = PairPhase::PickSource;
}

fn is_pair_tool(tool: CadTool) -> bool {
    matches!(tool, CadTool::Align | CadTool::Join | CadTool::Subtract)
}

fn rotation_drag_amount(
    camera: &Camera3D,
    fig: &CadFigure,
    axis_i: usize,
    mouse_delta: Vec2,
) -> f32 {
    let axes = axis_world_positions(fig);
    let Some(center_s) = project_world_to_screen(camera, fig.pos) else {
        return (mouse_delta.x - mouse_delta.y) * 0.0085;
    };
    let Some(tip_s) = project_world_to_screen(camera, axes[axis_i % 3]) else {
        return (mouse_delta.x - mouse_delta.y) * 0.0085;
    };
    let arm = tip_s - center_s;
    if arm.length_squared() < 1.0 {
        return (mouse_delta.x - mouse_delta.y) * 0.0085;
    }
    let radial = arm.normalize();
    let tangent = vec2(-radial.y, radial.x);
    mouse_delta.dot(tangent) * 0.012
}

fn apply_local_axis_rotation(fig: &mut CadFigure, axis_i: usize, angle: f32) {
    let q = figure_quat(fig);
    let local_axis = match axis_i % 3 {
        0 => Vec3::X,
        1 => Vec3::Y,
        _ => Vec3::Z,
    };
    let world_axis = (q * local_axis).normalize_or_zero();
    let dq = Quat::from_axis_angle(world_axis, angle);
    let new_q = (dq * q).normalize();
    let (rx, ry, rz) = new_q.to_euler(EulerRot::XYZ);
    fig.rot = vec3(rx, ry, rz);
}

fn pair_selection_click(state: &mut CadState, picked_figure: usize) {
    match state.pair_phase {
        PairPhase::PickSource => {
            state.pending_source = Some(picked_figure);
            state.selected = Some(picked_figure);
            state.pair_phase = PairPhase::PickTarget;
        }
        PairPhase::PickTarget => {
            let Some(source_idx) = state.pending_source else {
                state.pair_phase = PairPhase::PickSource;
                return;
            };
            if source_idx == picked_figure {
                state.selected = Some(picked_figure);
                return;
            }
            match state.tool {
                CadTool::Align => {
                    apply_align(
                        &mut state.figures,
                        source_idx,
                        picked_figure,
                        state.align_mode,
                        state.align_face,
                    );
                    state.selected = Some(source_idx);
                }
                CadTool::Join => {
                    let a = state.figures[source_idx].clone();
                    let b = state.figures[picked_figure].clone();
                    let result = apply_join(a, b);
                    state.selected = replace_pair_with_result(
                        &mut state.figures,
                        source_idx,
                        picked_figure,
                        result,
                    );
                }
                CadTool::Subtract => {
                    let a = state.figures[source_idx].clone();
                    let b = state.figures[picked_figure].clone();
                    let result = apply_subtract(a, b);
                    state.selected = replace_pair_with_result(
                        &mut state.figures,
                        source_idx,
                        picked_figure,
                        result,
                    );
                }
                _ => {}
            }
            state.pending_source = None;
            state.pair_phase = PairPhase::PickSource;
        }
    }
}

fn align_face_label(i: usize) -> &'static str {
    match i % 6 {
        0 => "+X",
        1 => "-X",
        2 => "+Y",
        3 => "-Y",
        4 => "+Z",
        _ => "-Z",
    }
}

fn export_obj(figures: &[CadFigure], path: &str) -> Result<(), String> {
    let mut out = String::from("# GridGame CAD OBJ\n");
    let mut vertex_offset = 1usize;

    for (fig_i, fig) in figures.iter().enumerate() {
        out.push_str(&format!("o figure_{}\n", fig_i + 1));
        for mesh in figure_world_meshes(fig, false) {
            for v in &mesh.vertices {
                out.push_str(&format!("v {:.6} {:.6} {:.6}\n", v.x, v.y, v.z));
            }
            for tri in &mesh.triangles {
                out.push_str(&format!(
                    "f {} {} {}\n",
                    tri[0] + vertex_offset,
                    tri[1] + vertex_offset,
                    tri[2] + vertex_offset
                ));
            }
            vertex_offset += mesh.vertices.len();
        }
    }

    fs::write(path, out).map_err(|err| format!("Export failed: {}", err))
}

fn parse_obj_index(raw: &str, vertex_count: usize) -> Option<usize> {
    let first = raw.split('/').next()?;
    let idx = first.parse::<isize>().ok()?;
    if idx > 0 {
        Some((idx as usize).saturating_sub(1))
    } else if idx < 0 {
        Some((vertex_count as isize + idx) as usize)
    } else {
        None
    }
}

fn import_obj(path: &str) -> Result<CadFigure, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("Import failed: {}", err))?;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();

    for line in text.lines() {
        let clean = line.trim();
        if clean.is_empty() || clean.starts_with('#') {
            continue;
        }
        let mut parts = clean.split_whitespace();
        match parts.next() {
            Some("v") => {
                let x = parts
                    .next()
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0);
                let y = parts
                    .next()
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0);
                let z = parts
                    .next()
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(0.0);
                vertices.push(vec3(x, y, z));
            }
            Some("f") => {
                let face: Vec<usize> = parts
                    .filter_map(|p| parse_obj_index(p, vertices.len()))
                    .filter(|idx| *idx < vertices.len())
                    .collect();
                if face.len() >= 3 {
                    for i in 1..face.len() - 1 {
                        triangles.push([face[0], face[i], face[i + 1]]);
                    }
                }
            }
            _ => {}
        }
    }

    figure_from_world_mesh(CadMeshData {
        vertices,
        triangles,
    })
    .ok_or_else(|| "Import failed: OBJ has no mesh faces".to_string())
}

pub fn run(
    ctx: &FrameContext,
    scene: &mut Scene,
    state: &mut CadState,
    last_mouse: &mut Vec2,
    colors: &RuntimeColors,
) {
    if is_key_pressed(KeyCode::Escape) {
        *scene = Scene::MainMenu;
        return;
    }

    let camera = cad_camera(state);
    let panel = Rect::new(screen_width() - 290.0, 24.0, 266.0, 640.0);
    let ui_capturing = panel.contains(ctx.mouse);

    if is_mouse_button_down(MouseButton::Right) {
        let delta = ctx.mouse - *last_mouse;
        state.yaw -= delta.x * 0.006;
        state.pitch = (state.pitch - delta.y * 0.0045).clamp(-1.2, 1.2);
    }
    state.distance = (state.distance - mouse_wheel().1 * 8.0).clamp(60.0, 420.0);
    *last_mouse = ctx.mouse;

    if is_mouse_button_pressed(MouseButton::Left) && !ui_capturing {
        state.dragging_handle = false;
        state.dragging_axis = false;

        if let Some(sel) = state.selected {
            if let Some(fig) = state.figures.get(sel) {
                match state.tool {
                    CadTool::Resize => {
                        let handles = handle_world_positions(fig);
                        if let Some(handle_i) =
                            pick_closest_index(&camera, ctx.mouse, &handles, 14.0)
                        {
                            state.selected_resize_handle = handle_i;
                            state.dragging_handle = true;
                            state.drag_last_mouse = ctx.mouse;
                        }
                    }
                    CadTool::Rotate => {
                        let axes = axis_world_positions(fig);
                        if let Some(axis_i) = pick_closest_index(&camera, ctx.mouse, &axes, 18.0) {
                            state.selected_rotate_axis = axis_i;
                            state.dragging_axis = true;
                            state.drag_last_mouse = ctx.mouse;
                        }
                    }
                    _ => {}
                }
            }
        }

        if !state.dragging_handle && !state.dragging_axis {
            if let Some(picked) = pick_figure_by_body(&camera, ctx.mouse, &state.figures) {
                if is_pair_tool(state.tool) {
                    pair_selection_click(state, picked);
                } else {
                    state.selected = Some(picked);
                }
            } else {
                let centers: Vec<Vec3> = state.figures.iter().map(|f| f.pos).collect();
                if let Some(picked) = pick_closest_index(&camera, ctx.mouse, &centers, 28.0) {
                    if is_pair_tool(state.tool) {
                        pair_selection_click(state, picked);
                    } else {
                        state.selected = Some(picked);
                    }
                }
            }
        }
    }
    if is_mouse_button_released(MouseButton::Left) {
        state.dragging_handle = false;
        state.dragging_axis = false;
    }

    draw_rectangle(
        panel.x,
        panel.y,
        panel.w,
        panel.h,
        Color::from_rgba(21, 28, 36, 220),
    );
    draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 1.5, colors.panel_border);
    draw_text(
        "CAD Tools",
        panel.x + 10.0,
        panel.y + 24.0,
        ctx.font_sm,
        colors.text_primary,
    );

    let mut y = panel.y + 36.0;
    let bw = panel.w - 20.0;
    let bh = 26.0;
    let (add_square, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Add Square",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    y += 30.0;
    let (add_cylinder, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Add Cylinder",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    y += 34.0;
    let (tool_move, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Move Figure",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    y += 30.0;
    let (tool_resize, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Resize Figure",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    y += 30.0;
    let (tool_rotate, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Rotate Figure",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    y += 30.0;
    let (tool_align, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Align",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    y += 30.0;
    let (tool_join, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Join",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    y += 30.0;
    let (tool_subtract, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Subtract",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    y += 34.0;
    let (export_obj_clicked, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Export OBJ",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    y += 30.0;
    let (import_obj_clicked, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Import OBJ",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    y += 34.0;

    if state.tool == CadTool::Align {
        let align_mode_label = match state.align_mode {
            AlignMode::Center => "Align Mode: Center",
            AlignMode::Face => "Align Mode: Face",
        };
        let (toggle_mode, _) = ui_button(
            Rect::new(panel.x + 10.0, y, bw, bh),
            align_mode_label,
            ctx.mouse,
            ctx.font_sm,
            ctx.button_colors,
        );
        y += 30.0;
        if toggle_mode {
            state.align_mode = match state.align_mode {
                AlignMode::Center => AlignMode::Face,
                AlignMode::Face => AlignMode::Center,
            };
        }
        if state.align_mode == AlignMode::Face {
            let face_label = format!("Target Face: {}", align_face_label(state.align_face));
            let (next_face, _) = ui_button(
                Rect::new(panel.x + 10.0, y, bw, bh),
                &face_label,
                ctx.mouse,
                ctx.font_sm,
                ctx.button_colors,
            );
            if next_face {
                state.align_face = (state.align_face + 1) % 6;
            }
        }
    }

    if add_square {
        let idx = state.figures.len() as f32;
        state.figures.push(CadFigure::new(
            CadPrimitiveKind::Square,
            vec3(
                (idx % 4.0) * 22.0 - 33.0,
                20.0,
                (idx / 4.0).floor() * 22.0 - 22.0,
            ),
            vec3(36.0, 36.0, 36.0),
            vec3(0.0, 0.0, 0.0),
        ));
        state.selected = Some(state.figures.len().saturating_sub(1));
    }
    if add_cylinder {
        let idx = state.figures.len() as f32;
        state.figures.push(CadFigure::new(
            CadPrimitiveKind::Cylinder,
            vec3(
                (idx % 4.0) * 22.0 - 33.0,
                20.0,
                (idx / 4.0).floor() * 22.0 - 22.0,
            ),
            vec3(30.0, 36.0, 30.0),
            vec3(0.0, 0.0, 0.0),
        ));
        state.selected = Some(state.figures.len().saturating_sub(1));
    }

    if state.figures.is_empty() {
        state.selected = None;
        state.pending_source = None;
        state.pair_phase = PairPhase::PickSource;
    } else if state.selected.is_none() {
        state.selected = Some(0);
    }

    if tool_move {
        switch_tool(state, CadTool::Move);
    }
    if tool_resize {
        switch_tool(state, CadTool::Resize);
    }
    if tool_rotate {
        switch_tool(state, CadTool::Rotate);
    }
    if tool_align {
        switch_tool(state, CadTool::Align);
    }
    if tool_join {
        switch_tool(state, CadTool::Join);
    }
    if tool_subtract {
        switch_tool(state, CadTool::Subtract);
    }
    if export_obj_clicked {
        state.obj_status = match export_obj(&state.figures, CAD_OBJ_PATH) {
            Ok(()) => format!("Exported {}", CAD_OBJ_PATH),
            Err(err) => err,
        };
    }
    if import_obj_clicked {
        match import_obj(CAD_OBJ_PATH) {
            Ok(fig) => {
                state.figures.push(fig);
                state.selected = Some(state.figures.len().saturating_sub(1));
                state.obj_status = format!("Imported {}", CAD_OBJ_PATH);
            }
            Err(err) => state.obj_status = err,
        }
    }

    if let Some(i) = state.selected {
        if let Some(fig) = state.figures.get_mut(i) {
            let dt = get_frame_time();
            match state.tool {
                CadTool::Move => apply_move_tool(fig, dt),
                CadTool::Resize => {
                    let handles = resize_handles(fig);
                    if !handles.is_empty() {
                        let sel = state.selected_resize_handle % handles.len();
                        state.selected_resize_handle = sel;
                        if state.dragging_handle && is_mouse_button_down(MouseButton::Left) {
                            let mouse_delta = ctx.mouse - state.drag_last_mouse;
                            state.drag_last_mouse = ctx.mouse;
                            let cam_fwd = (camera.target - camera.position).normalize_or_zero();
                            let cam_right = cam_fwd.cross(camera.up).normalize_or_zero();
                            let cam_up = camera.up.normalize_or_zero();
                            let world_move = cam_right * (mouse_delta.x * 0.12)
                                + cam_up * (-mouse_delta.y * 0.12);
                            let (x, y, z) = figure_basis(fig);
                            let local_delta =
                                vec3(world_move.dot(x), world_move.dot(y), world_move.dot(z));
                            apply_resize_with_handle(fig, handles[sel].kind, local_delta);
                        }
                        let key_delta = resize_keyboard_delta(dt);
                        if key_delta.length_squared() > 0.0 {
                            apply_resize_with_handle(fig, handles[sel].kind, key_delta);
                        }
                    }
                }
                CadTool::Rotate => {
                    if state.dragging_axis && is_mouse_button_down(MouseButton::Left) {
                        let mouse_delta = ctx.mouse - state.drag_last_mouse;
                        state.drag_last_mouse = ctx.mouse;
                        let amount = rotation_drag_amount(
                            &camera,
                            fig,
                            state.selected_rotate_axis,
                            mouse_delta,
                        );
                        apply_local_axis_rotation(fig, state.selected_rotate_axis, amount);
                    }
                }
                CadTool::Align | CadTool::Join | CadTool::Subtract => {}
            }
        }
    }

    set_camera(&camera);
    for i in -12..=12 {
        let t = i as f32 * 12.0;
        draw_line_3d(
            vec3(t, 0.0, -144.0),
            vec3(t, 0.0, 144.0),
            Color::from_rgba(66, 77, 92, 255),
        );
        draw_line_3d(
            vec3(-144.0, 0.0, t),
            vec3(144.0, 0.0, t),
            Color::from_rgba(66, 77, 92, 255),
        );
    }
    draw_line_3d(vec3(-160.0, 0.0, 0.0), vec3(160.0, 0.0, 0.0), colors.hover);
    draw_line_3d(
        vec3(0.0, 0.0, -160.0),
        vec3(0.0, 0.0, 160.0),
        colors.text_secondary,
    );
    for (i, fig) in state.figures.iter().enumerate() {
        draw_figure(fig, state.selected == Some(i));
    }

    if let Some(i) = state.selected {
        if let Some(fig) = state.figures.get(i) {
            if state.tool == CadTool::Resize {
                let handles = resize_handles(fig);
                let sel = if handles.is_empty() {
                    0
                } else {
                    state.selected_resize_handle % handles.len()
                };
                for (h_i, h) in handles.iter().enumerate() {
                    let pos = local_to_world(fig, h.local_pos);
                    let color = if h_i == sel {
                        colors.hover
                    } else {
                        Color::from_rgba(215, 230, 240, 255)
                    };
                    draw_cube(pos, vec3(3.2, 3.2, 3.2), None, color);
                    draw_cube_wires(pos, vec3(3.2, 3.2, 3.2), colors.text_primary);
                }
            } else if state.tool == CadTool::Rotate {
                let (x, y, z) = figure_basis(fig);
                let axis_len = fig.size.max_element() * 0.8 + 14.0;
                let axis_data = [
                    (x, Color::from_rgba(255, 120, 120, 255)),
                    (y, Color::from_rgba(120, 255, 120, 255)),
                    (z, Color::from_rgba(120, 170, 255, 255)),
                ];
                for (axis_i, (dir, col)) in axis_data.iter().enumerate() {
                    let tip = fig.pos + *dir * axis_len;
                    draw_line_3d(fig.pos, tip, *col);
                    let marker_color = if axis_i == state.selected_rotate_axis {
                        colors.hover
                    } else {
                        *col
                    };
                    draw_cube(tip, vec3(3.6, 3.6, 3.6), None, marker_color);
                    draw_cube_wires(tip, vec3(3.6, 3.6, 3.6), colors.text_primary);
                }
            }
        }
    }
    set_default_camera();

    let tool_label = match state.tool {
        CadTool::Move => "Move",
        CadTool::Resize => "Resize",
        CadTool::Rotate => "Rotate",
        CadTool::Align => "Align",
        CadTool::Join => "Join",
        CadTool::Subtract => "Subtract",
    };
    let selected_label = state
        .selected
        .map(|i| format!("{}", i + 1))
        .unwrap_or_else(|| "none".to_string());
    let pending_label = state
        .pending_source
        .map(|i| format!("{}", i + 1))
        .unwrap_or_else(|| "none".to_string());
    draw_text(
        "CAD Prototype",
        24.0,
        36.0,
        ctx.font_md,
        colors.text_primary,
    );
    draw_text(
        &format!(
            "Figures: {} | Selected: {}",
            state.figures.len(),
            selected_label
        ),
        24.0,
        62.0,
        ctx.font_sm,
        colors.text_secondary,
    );
    draw_text(
        &format!(
            "Tool: {} | Handle: {} | Axis: {}",
            tool_label,
            state.selected_resize_handle + 1,
            state.selected_rotate_axis + 1
        ),
        24.0,
        84.0,
        ctx.font_sm,
        colors.text_secondary,
    );
    if is_pair_tool(state.tool) {
        let phase = match state.pair_phase {
            PairPhase::PickSource => "Pick Source",
            PairPhase::PickTarget => "Pick Target",
        };
        draw_text(
            &format!("Pair: {} | Pending: {}", phase, pending_label),
            24.0,
            106.0,
            ctx.font_sm,
            colors.text_secondary,
        );
    } else {
        draw_text(
            "LMB select/drag gizmo | resize handles local | rotate local axes",
            24.0,
            106.0,
            ctx.font_sm,
            colors.text_secondary,
        );
    }
    draw_text(
        &state.obj_status,
        24.0,
        128.0,
        ctx.font_sm,
        colors.text_secondary,
    );
}
