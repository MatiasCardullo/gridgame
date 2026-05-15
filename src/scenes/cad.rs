use macroquad::prelude::*;
use opengeometry::booleans::{
    OGBooleanOperation, OGBooleanOptions, boolean_subtraction, boolean_union,
};
use opengeometry::brep::Brep;
use opengeometry::export::stl::{StlExportConfig, export_breps_to_stl_file};
use opengeometry::primitives::cuboid::OGCuboid;
use opengeometry::primitives::cylinder::OGCylinder;
use opengeometry::primitives::sphere::OGSphere;
use opengeometry::primitives::wedge::OGWedge;
use openmaths::Vector3;
use rfd::FileDialog;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::core::ui::ui_button;
use crate::core::{FrameContext, RuntimeColors, Scene};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CadPrimitiveKind {
    Square,
    Cylinder,
    Sphere,
    Wedge,
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
enum AddShapeKind {
    Square,
    Cylinder,
    Sphere,
    Wedge,
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

#[derive(Clone)]
struct CadFigure {
    kind: CadPrimitiveKind,
    pos: Vec3,
    size: Vec3,
    rot: Vec3,
    mesh: CadMeshData,
    brep: Option<Brep>,
}

impl CadFigure {
    fn new(kind: CadPrimitiveKind, pos: Vec3, size: Vec3) -> Self {
        let mut fig = Self {
            kind,
            pos,
            size,
            rot: Vec3::ZERO,
            mesh: CadMeshData::new(),
            brep: None,
        };
        fig.rebuild();
        fig
    }

    fn from_brep(kind: CadPrimitiveKind, brep: Brep) -> Self {
        let mesh = mesh_from_brep(&brep);
        let (pos, size) = mesh_bounds(&mesh).unwrap_or((Vec3::ZERO, vec3(12.0, 12.0, 12.0)));
        Self {
            kind,
            pos,
            size,
            rot: Vec3::ZERO,
            mesh,
            brep: Some(brep),
        }
    }

    fn from_mesh(mesh: CadMeshData) -> Self {
        let (pos, size) = mesh_bounds(&mesh).unwrap_or((Vec3::ZERO, vec3(12.0, 12.0, 12.0)));
        Self {
            kind: CadPrimitiveKind::Mesh,
            pos,
            size,
            rot: Vec3::ZERO,
            mesh,
            brep: None,
        }
    }

    // Rebuilds OpenGeometry BRep, then refreshes render triangles.
    fn rebuild(&mut self) {
        match self.kind {
            CadPrimitiveKind::Square => {
                let mut cuboid = OGCuboid::new("gridgame-cuboid".to_string());
                if cuboid
                    .set_config(
                        og_vec(Vec3::ZERO),
                        self.size.x.max(1.0) as f64,
                        self.size.y.max(1.0) as f64,
                        self.size.z.max(1.0) as f64,
                    )
                    .is_ok()
                    && cuboid
                        .set_transform(og_vec(self.pos), og_vec(self.rot), og_vec(Vec3::ONE))
                        .is_ok()
                {
                    let brep = cuboid.world_brep();
                    self.mesh = mesh_from_brep(&brep);
                    self.brep = Some(brep);
                }
            }
            CadPrimitiveKind::Cylinder => {
                let radius = ((self.size.x + self.size.z) * 0.25).max(1.0);
                self.size.x = radius * 2.0;
                self.size.z = radius * 2.0;
                let mut cylinder = OGCylinder::new("gridgame-cylinder".to_string());
                if cylinder
                    .set_config(
                        og_vec(Vec3::ZERO),
                        radius as f64,
                        self.size.y.max(1.0) as f64,
                        std::f64::consts::TAU,
                        48,
                    )
                    .is_ok()
                    && cylinder
                        .set_transform(og_vec(self.pos), og_vec(self.rot), og_vec(Vec3::ONE))
                        .is_ok()
                {
                    let brep = cylinder.world_brep();
                    self.mesh = mesh_from_brep(&brep);
                    self.brep = Some(brep);
                }
            }
            CadPrimitiveKind::Sphere => {
                let radius = (self.size.x + self.size.y + self.size.z).max(3.0) / 6.0;
                self.size = Vec3::splat(radius * 2.0);
                let mut sphere = OGSphere::new("gridgame-sphere".to_string());
                if sphere
                    .set_config(og_vec(Vec3::ZERO), radius as f64, 32, 18)
                    .is_ok()
                    && sphere
                        .set_transform(og_vec(self.pos), og_vec(self.rot), og_vec(Vec3::ONE))
                        .is_ok()
                {
                    let brep = sphere.world_brep();
                    self.mesh = mesh_from_brep(&brep);
                    self.brep = Some(brep);
                }
            }
            CadPrimitiveKind::Wedge => {
                let mut wedge = OGWedge::new("gridgame-wedge".to_string());
                if wedge
                    .set_config(
                        og_vec(Vec3::ZERO),
                        self.size.x.max(1.0) as f64,
                        self.size.y.max(1.0) as f64,
                        self.size.z.max(1.0) as f64,
                    )
                    .is_ok()
                    && wedge
                        .set_transform(og_vec(self.pos), og_vec(self.rot), og_vec(Vec3::ONE))
                        .is_ok()
                {
                    let brep = wedge.world_brep();
                    self.mesh = mesh_from_brep(&brep);
                    self.brep = Some(brep);
                }
            }
            CadPrimitiveKind::Mesh => {}
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
    align_mode: AlignMode,
    align_face: usize,
    add_menu_open: bool,
    selected_vertices: Vec<usize>,
    status: String,
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
            align_mode: AlignMode::Center,
            align_face: 0,
            add_menu_open: false,
            selected_vertices: Vec::new(),
            status: "OpenGeometry CAD".to_string(),
        }
    }
}

fn og_vec(v: Vec3) -> Vector3 {
    Vector3::new(v.x as f64, v.y as f64, v.z as f64)
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

fn mesh_from_brep(brep: &Brep) -> CadMeshData {
    let mut mesh = CadMeshData::new();
    let buffer = brep.get_triangle_vertex_buffer();
    for tri in buffer.chunks_exact(9) {
        let base = mesh.vertices.len();
        mesh.vertices
            .push(vec3(tri[0] as f32, tri[1] as f32, tri[2] as f32));
        mesh.vertices
            .push(vec3(tri[3] as f32, tri[4] as f32, tri[5] as f32));
        mesh.vertices
            .push(vec3(tri[6] as f32, tri[7] as f32, tri[8] as f32));
        mesh.triangles.push([base, base + 1, base + 2]);
    }
    mesh
}

fn mesh_bounds(mesh: &CadMeshData) -> Option<(Vec3, Vec3)> {
    let first = *mesh.vertices.first()?;
    let mut min = first;
    let mut max = first;
    for v in &mesh.vertices[1..] {
        min = min.min(*v);
        max = max.max(*v);
    }
    Some(((min + max) * 0.5, (max - min).max(vec3(1.0, 1.0, 1.0))))
}

fn draw_cad_mesh(mesh: &CadMeshData, fill: Color, wire: Color) {
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
            if let Some(p) = mesh.vertices.get(*idx) {
                vertices.push(Vertex::new2(*p, vec2(0.0, 0.0), fill));
            }
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
        draw_line_3d(*a, *b, wire);
        draw_line_3d(*b, *c, wire);
        draw_line_3d(*c, *a, wire);
    }
}

fn draw_figure(fig: &CadFigure, selected: bool) {
    let (fill, wire) = if selected {
        (
            Color::from_rgba(67, 120, 155, 160),
            Color::from_rgba(165, 228, 255, 255),
        )
    } else {
        (
            Color::from_rgba(68, 84, 105, 130),
            Color::from_rgba(105, 132, 150, 255),
        )
    };
    draw_cad_mesh(&fig.mesh, fill, wire);
}

fn box_handles(fig: &CadFigure) -> Vec<ResizeHandle> {
    let h = fig.size * 0.5;
    let mut out = Vec::new();
    for sx in [-1, 1] {
        out.push(ResizeHandle {
            local_pos: vec3(h.x * sx as f32, 0.0, 0.0),
            kind: ResizeHandleKind::BoxDir(Vec3Sign { x: sx, y: 0, z: 0 }),
        });
    }
    for sy in [-1, 1] {
        out.push(ResizeHandle {
            local_pos: vec3(0.0, h.y * sy as f32, 0.0),
            kind: ResizeHandleKind::BoxDir(Vec3Sign { x: 0, y: sy, z: 0 }),
        });
    }
    for sz in [-1, 1] {
        out.push(ResizeHandle {
            local_pos: vec3(0.0, 0.0, h.z * sz as f32),
            kind: ResizeHandleKind::BoxDir(Vec3Sign { x: 0, y: 0, z: sz }),
        });
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
        CadPrimitiveKind::Cylinder => cylinder_handles(fig),
        CadPrimitiveKind::Square
        | CadPrimitiveKind::Sphere
        | CadPrimitiveKind::Wedge
        | CadPrimitiveKind::Mesh => box_handles(fig),
    }
}

fn axis_world_positions(fig: &CadFigure) -> [Vec3; 3] {
    let (x, y, z) = figure_basis(fig);
    let len = fig.size.max_element().max(18.0) * 0.7;
    [fig.pos + x * len, fig.pos + y * len, fig.pos + z * len]
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
        let Some(screen) = project_world_to_screen(camera, *p) else {
            continue;
        };
        let dist = screen.distance(mouse);
        if dist <= max_px && best.is_none_or(|(best_dist, _)| dist < best_dist) {
            best = Some((dist, i));
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

fn ray_triangle_t(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let eps = 0.00001;
    let edge1 = b - a;
    let edge2 = c - a;
    let h = dir.cross(edge2);
    let det = edge1.dot(h);
    if det.abs() < eps {
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
    (t > eps).then_some(t)
}

fn ray_mesh_t(origin: Vec3, dir: Vec3, mesh: &CadMeshData) -> Option<f32> {
    let mut best: Option<f32> = None;
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
        if let Some(t) = ray_triangle_t(origin, dir, *a, *b, *c) {
            if best.is_none_or(|best_t| t < best_t) {
                best = Some(t);
            }
        }
    }
    best
}

fn ray_aabb_t(origin: Vec3, dir: Vec3, center: Vec3, size: Vec3) -> Option<f32> {
    let half = size * 0.5 + Vec3::splat(1.5);
    let local = origin - center;
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;
    for (o, d, h) in [
        (local.x, dir.x, half.x),
        (local.y, dir.y, half.y),
        (local.z, dir.z, half.z),
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
    (t_max >= 0.0).then_some(t_min.max(0.0))
}

fn pick_figure_by_body(camera: &Camera3D, mouse: Vec2, figures: &[CadFigure]) -> Option<usize> {
    let (origin, dir) = mouse_world_ray(camera, mouse);
    let mut best: Option<(f32, usize)> = None;
    for (i, fig) in figures.iter().enumerate() {
        let hit = ray_mesh_t(origin, dir, &fig.mesh)
            .or_else(|| ray_aabb_t(origin, dir, fig.pos, fig.size));
        if let Some(t) = hit {
            if best.is_none_or(|(best_t, _)| t < best_t) {
                best = Some((t, i));
            }
        }
    }
    best.map(|(_, i)| i)
}

fn same_vertex(a: Vec3, b: Vec3) -> bool {
    a.distance_squared(b) <= 0.0001
}

fn unique_vertex_indices(mesh: &CadMeshData) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for (i, v) in mesh.vertices.iter().enumerate() {
        if !out.iter().any(|idx| same_vertex(mesh.vertices[*idx], *v)) {
            out.push(i);
        }
    }
    out
}

fn pick_vertex(camera: &Camera3D, mouse: Vec2, fig: &CadFigure) -> Option<usize> {
    let points: Vec<Vec3> = unique_vertex_indices(&fig.mesh)
        .into_iter()
        .filter_map(|idx| fig.mesh.vertices.get(idx).copied())
        .collect();
    pick_closest_index(camera, mouse, &points, 16.0).and_then(|hit| {
        let point = points.get(hit)?;
        fig.mesh
            .vertices
            .iter()
            .position(|v| same_vertex(*v, *point))
    })
}

fn add_vertex_selection(
    state: &mut CadState,
    figure_idx: usize,
    vertex_idx: usize,
    additive: bool,
) {
    if state.selected != Some(figure_idx) || !additive {
        state.selected_vertices.clear();
    }
    state.selected = Some(figure_idx);
    if !state.selected_vertices.contains(&vertex_idx) {
        state.selected_vertices.push(vertex_idx);
    }
}

fn apply_vertex_delta(fig: &mut CadFigure, selected_vertices: &[usize], delta: Vec3) {
    if selected_vertices.is_empty() || delta.length_squared() == 0.0 {
        return;
    }
    let anchors: Vec<Vec3> = selected_vertices
        .iter()
        .filter_map(|idx| fig.mesh.vertices.get(*idx).copied())
        .collect();
    for vertex in &mut fig.mesh.vertices {
        if anchors.iter().any(|anchor| same_vertex(*vertex, *anchor)) {
            *vertex += delta;
        }
    }
    fig.kind = CadPrimitiveKind::Mesh;
    fig.brep = None;
    if let Some((pos, size)) = mesh_bounds(&fig.mesh) {
        fig.pos = pos;
        fig.size = size;
    }
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
    fig.rebuild();
}

fn nudge_delta(dt: f32) -> Vec3 {
    let speed = if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
        28.0
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
    if is_key_down(KeyCode::Up) {
        out.z -= speed * dt;
    }
    if is_key_down(KeyCode::Down) {
        out.z += speed * dt;
    }
    if is_key_down(KeyCode::Q) {
        out.y += speed * dt;
    }
    if is_key_down(KeyCode::E) {
        out.y -= speed * dt;
    }
    out
}

fn apply_resize_with_handle(fig: &mut CadFigure, handle: ResizeHandleKind, local_delta: Vec3) {
    let min_size = 8.0;
    match handle {
        ResizeHandleKind::BoxDir(sign) => {
            let dir = sign.as_vec3();
            let mut size = fig.size;
            let delta = local_delta.dot(dir) * 0.5;
            if dir.x != 0.0 {
                size.x = (size.x + delta).max(min_size);
            }
            if dir.y != 0.0 {
                size.y = (size.y + delta).max(min_size);
            }
            if dir.z != 0.0 {
                size.z = (size.z + delta).max(min_size);
            }
            let shift = dir * ((size - fig.size) * 0.5);
            let (x, y, z) = figure_basis(fig);
            fig.pos += x * shift.x + y * shift.y + z * shift.z;
            fig.size = size;
        }
        ResizeHandleKind::CylinderTop => {
            let old = fig.size.y;
            fig.size.y = (fig.size.y + local_delta.y).max(min_size);
            fig.pos += figure_quat(fig) * Vec3::Y * ((fig.size.y - old) * 0.5);
        }
        ResizeHandleKind::CylinderBottom => {
            let old = fig.size.y;
            fig.size.y = (fig.size.y - local_delta.y).max(min_size);
            fig.pos -= figure_quat(fig) * Vec3::Y * ((fig.size.y - old) * 0.5);
        }
        ResizeHandleKind::CylinderRadial(sign) => {
            let dir = sign.as_vec3().normalize_or_zero();
            let delta = local_delta.dot(dir) * 0.5;
            let old_r = (fig.size.x + fig.size.z) * 0.25;
            let new_r = (old_r + delta).max(4.0);
            let shift = dir * (new_r - old_r);
            let (x, y, z) = figure_basis(fig);
            fig.pos += x * shift.x + y * shift.y + z * shift.z;
            fig.size.x = new_r * 2.0;
            fig.size.z = new_r * 2.0;
        }
    }
    fig.rebuild();
}

fn rotate_selected(fig: &mut CadFigure, axis_i: usize, amount: f32) {
    let q = figure_quat(fig);
    let axis = match axis_i % 3 {
        0 => Vec3::X,
        1 => Vec3::Y,
        _ => Vec3::Z,
    };
    let world_axis = (q * axis).normalize_or_zero();
    let new_q = (Quat::from_axis_angle(world_axis, amount) * q).normalize();
    let (rx, ry, rz) = new_q.to_euler(EulerRot::XYZ);
    fig.rot = vec3(rx, ry, rz);
    fig.rebuild();
}

fn apply_align(
    figures: &mut [CadFigure],
    source: usize,
    target: usize,
    mode: AlignMode,
    face: usize,
) {
    if source >= figures.len() || target >= figures.len() || source == target {
        return;
    }
    let target_fig = figures[target].clone();
    if let Some(source_fig) = figures.get_mut(source) {
        match mode {
            AlignMode::Center => source_fig.pos = target_fig.pos,
            AlignMode::Face => {
                let normals = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
                let n = figure_quat(&target_fig) * normals[face % normals.len()];
                source_fig.pos = target_fig.pos
                    + n * ((target_fig.size.dot(n.abs()) + source_fig.size.dot(n.abs())) * 0.5);
            }
        }
        source_fig.rebuild();
    }
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

fn apply_boolean(
    base: &CadFigure,
    tool: &CadFigure,
    op: OGBooleanOperation,
) -> Result<CadFigure, String> {
    let lhs = base
        .brep
        .as_ref()
        .ok_or_else(|| "Source has no OpenGeometry BRep".to_string())?;
    let rhs = tool
        .brep
        .as_ref()
        .ok_or_else(|| "Target has no OpenGeometry BRep".to_string())?;
    let out = match op {
        OGBooleanOperation::Union => boolean_union(lhs, rhs, OGBooleanOptions::default()),
        OGBooleanOperation::Subtraction => {
            boolean_subtraction(lhs, rhs, OGBooleanOptions::default())
        }
        OGBooleanOperation::Intersection => unreachable!(),
    }
    .map_err(|err| format!("OpenGeometry boolean failed: {}", err))?;
    Ok(CadFigure::from_brep(CadPrimitiveKind::Mesh, out.brep))
}

fn switch_tool(state: &mut CadState, tool: CadTool) {
    state.tool = tool;
    state.pending_source = None;
    state.pair_phase = PairPhase::PickSource;
}

fn is_pair_tool(tool: CadTool) -> bool {
    matches!(tool, CadTool::Align | CadTool::Join | CadTool::Subtract)
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
                    state.status = "Aligned".to_string();
                }
                CadTool::Join | CadTool::Subtract => {
                    let op = if state.tool == CadTool::Join {
                        OGBooleanOperation::Union
                    } else {
                        OGBooleanOperation::Subtraction
                    };
                    let result = apply_boolean(
                        &state.figures[source_idx],
                        &state.figures[picked_figure],
                        op,
                    );
                    match result {
                        Ok(fig) => {
                            state.selected = replace_pair_with_result(
                                &mut state.figures,
                                source_idx,
                                picked_figure,
                                Some(fig),
                            );
                            state.status = if state.tool == CadTool::Join {
                                "Joined with OpenGeometry"
                            } else {
                                "Subtracted with OpenGeometry"
                            }
                            .to_string();
                        }
                        Err(err) => state.status = err,
                    }
                }
                _ => {}
            }
            state.pending_source = None;
            state.pair_phase = PairPhase::PickSource;
        }
    }
}

fn export_obj(figures: &[CadFigure], path: &Path) -> Result<(), String> {
    let mut out = String::from("# GridGame OpenGeometry CAD OBJ\n");
    let mut offset = 1usize;
    for fig in figures {
        for v in &fig.mesh.vertices {
            out.push_str(&format!("v {} {} {}\n", v.x, v.y, v.z));
        }
        for tri in &fig.mesh.triangles {
            out.push_str(&format!(
                "f {} {} {}\n",
                tri[0] + offset,
                tri[1] + offset,
                tri[2] + offset
            ));
        }
        offset += fig.mesh.vertices.len();
    }
    fs::write(path, out).map_err(|err| format!("OBJ export failed: {}", err))
}

fn import_obj(path: &Path) -> Result<CadFigure, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("OBJ import failed: {}", err))?;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let x: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                let y: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                let z: f32 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
                vertices.push(vec3(x, y, z));
            }
            Some("f") => {
                let face: Vec<usize> = parts
                    .filter_map(|p| p.split('/').next()?.parse::<usize>().ok())
                    .filter_map(|i| i.checked_sub(1))
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
    if vertices.is_empty() || triangles.is_empty() {
        Err("OBJ import failed: no mesh data".to_string())
    } else {
        Ok(CadFigure::from_mesh(CadMeshData {
            vertices,
            triangles,
        }))
    }
}

fn import_stl(path: &Path) -> Result<CadFigure, String> {
    let file = File::open(path).map_err(|err| format!("STL import failed: {}", err))?;
    let mut reader = BufReader::new(file);
    let stl = stl_io::read_stl(&mut reader).map_err(|err| format!("STL import failed: {}", err))?;
    let vertices = stl
        .vertices
        .iter()
        .map(|v| vec3(v[0], v[1], v[2]))
        .collect();
    let triangles = stl
        .faces
        .iter()
        .map(|tri| [tri.vertices[0], tri.vertices[1], tri.vertices[2]])
        .collect();
    Ok(CadFigure::from_mesh(CadMeshData {
        vertices,
        triangles,
    }))
}

fn export_mesh_stl(figures: &[CadFigure], path: &Path) -> Result<(), String> {
    let mut triangles = Vec::new();
    for fig in figures {
        for tri in &fig.mesh.triangles {
            let Some(a) = fig.mesh.vertices.get(tri[0]) else {
                continue;
            };
            let Some(b) = fig.mesh.vertices.get(tri[1]) else {
                continue;
            };
            let Some(c) = fig.mesh.vertices.get(tri[2]) else {
                continue;
            };
            triangles.push(stl_io::Triangle {
                normal: stl_io::Normal::new([0.0, 0.0, 0.0]),
                vertices: [
                    stl_io::Vertex::new([a.x, a.y, a.z]),
                    stl_io::Vertex::new([b.x, b.y, b.z]),
                    stl_io::Vertex::new([c.x, c.y, c.z]),
                ],
            });
        }
    }
    let mut file = File::create(path).map_err(|err| format!("STL export failed: {}", err))?;
    stl_io::write_stl(&mut file, triangles.iter())
        .map_err(|err| format!("STL export failed: {}", err))
}

fn export_stl(figures: &[CadFigure], path: &Path) -> Result<(), String> {
    let breps: Vec<&Brep> = figures.iter().filter_map(|fig| fig.brep.as_ref()).collect();
    if breps.len() == figures.len() {
        return export_breps_to_stl_file(
            breps,
            path.to_string_lossy().as_ref(),
            &StlExportConfig::default(),
        )
        .map(|_| ())
        .map_err(|err| format!("STL export failed: {}", err));
    }
    export_mesh_stl(figures, path)
}

fn export_with_dialog(figures: &[CadFigure]) -> Result<String, String> {
    let Some(path) = FileDialog::new()
        .add_filter("OBJ", &["obj"])
        .add_filter("STL", &["stl"])
        .set_file_name("cad_scene.obj")
        .save_file()
    else {
        return Ok("Export canceled".to_string());
    };
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
    {
        Some(ext) if ext == "obj" => {
            export_obj(figures, &path).map(|_| format!("Exported {}", path.display()))
        }
        Some(ext) if ext == "stl" => {
            export_stl(figures, &path).map(|_| format!("Exported {}", path.display()))
        }
        _ => Err("Export failed: choose .obj or .stl".to_string()),
    }
}

fn import_with_dialog() -> Result<(CadFigure, String), String> {
    let Some(path) = FileDialog::new()
        .add_filter("CAD mesh", &["obj", "stl"])
        .pick_file()
    else {
        return Err("Import canceled".to_string());
    };
    let fig = match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
    {
        Some(ext) if ext == "obj" => import_obj(&path)?,
        Some(ext) if ext == "stl" => import_stl(&path)?,
        _ => return Err("Import failed: choose .obj or .stl".to_string()),
    };
    Ok((fig, format!("Imported {}", path.display())))
}

fn add_shape(state: &mut CadState, shape: AddShapeKind) {
    let idx = state.figures.len() as f32;
    let pos = vec3(
        (idx % 4.0) * 24.0 - 36.0,
        20.0,
        (idx / 4.0).floor() * 24.0 - 24.0,
    );
    let (kind, size) = match shape {
        AddShapeKind::Square => (CadPrimitiveKind::Square, vec3(36.0, 36.0, 36.0)),
        AddShapeKind::Cylinder => (CadPrimitiveKind::Cylinder, vec3(30.0, 36.0, 30.0)),
        AddShapeKind::Sphere => (CadPrimitiveKind::Sphere, vec3(34.0, 34.0, 34.0)),
        AddShapeKind::Wedge => (CadPrimitiveKind::Wedge, vec3(38.0, 34.0, 34.0)),
    };
    state.figures.push(CadFigure::new(kind, pos, size));
    state.selected = Some(state.figures.len().saturating_sub(1));
    state.selected_vertices.clear();
}

fn draw_tools_panel(ctx: &FrameContext, state: &mut CadState, colors: &RuntimeColors) {
    let panel = Rect::new(18.0, 54.0, 190.0, 430.0);
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, colors.tooltip_bg);
    draw_rectangle_lines(
        panel.x,
        panel.y,
        panel.w,
        panel.h,
        1.0,
        colors.tooltip_border,
    );
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
    let (add_clicked, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Add",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    if add_clicked {
        state.add_menu_open = !state.add_menu_open;
    }
    y += 30.0;
    if state.add_menu_open {
        for (label, shape) in [
            ("Square", AddShapeKind::Square),
            ("Cylinder", AddShapeKind::Cylinder),
            ("Sphere", AddShapeKind::Sphere),
            ("Wedge", AddShapeKind::Wedge),
        ] {
            let (clicked, _) = ui_button(
                Rect::new(panel.x + 24.0, y, bw - 14.0, bh),
                label,
                ctx.mouse,
                ctx.font_sm,
                ctx.button_colors,
            );
            if clicked {
                add_shape(state, shape);
                state.add_menu_open = false;
            }
            y += 30.0;
        }
        y += 4.0;
    }

    for (label, tool) in [
        ("Move Figure", CadTool::Move),
        ("Resize Vertices", CadTool::Resize),
        ("Rotate Figure", CadTool::Rotate),
        ("Align", CadTool::Align),
        ("Join", CadTool::Join),
        ("Subtract", CadTool::Subtract),
    ] {
        let (clicked, _) = ui_button(
            Rect::new(panel.x + 10.0, y, bw, bh),
            label,
            ctx.mouse,
            ctx.font_sm,
            ctx.button_colors,
        );
        if clicked {
            switch_tool(state, tool);
        }
        y += if matches!(tool, CadTool::Subtract) {
            34.0
        } else {
            30.0
        };
    }

    let (export_clicked, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Export",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    if export_clicked {
        state.status = match export_with_dialog(&state.figures) {
            Ok(message) => message,
            Err(err) => err,
        };
    }
    y += 30.0;
    let (import_clicked, _) = ui_button(
        Rect::new(panel.x + 10.0, y, bw, bh),
        "Import",
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    if import_clicked {
        state.status = match import_with_dialog() {
            Ok((fig, message)) => {
                state.figures.push(fig);
                state.selected = Some(state.figures.len().saturating_sub(1));
                state.selected_vertices.clear();
                message
            }
            Err(err) => err,
        };
    }
    y += 34.0;

    if state.tool == CadTool::Align {
        let label = match state.align_mode {
            AlignMode::Center => "Align Mode: Center",
            AlignMode::Face => "Align Mode: Face",
        };
        let (toggle, _) = ui_button(
            Rect::new(panel.x + 10.0, y, bw, bh),
            label,
            ctx.mouse,
            ctx.font_sm,
            ctx.button_colors,
        );
        if toggle {
            state.align_mode = match state.align_mode {
                AlignMode::Center => AlignMode::Face,
                AlignMode::Face => AlignMode::Center,
            };
        }
        y += 30.0;
        let (face, _) = ui_button(
            Rect::new(panel.x + 10.0, y, bw, bh),
            &format!("Face {}", state.align_face + 1),
            ctx.mouse,
            ctx.font_sm,
            ctx.button_colors,
        );
        if face {
            state.align_face = (state.align_face + 1) % 6;
        }
    }
}

pub fn run(
    ctx: &FrameContext,
    scene: &mut Scene,
    state: &mut CadState,
    last_mouse: &mut Vec2,
    colors: &RuntimeColors,
) {
    let dt = get_frame_time();
    let camera = cad_camera(state);

    if is_key_pressed(KeyCode::Escape) {
        *scene = Scene::MainMenu;
        return;
    }

    if is_mouse_button_down(MouseButton::Right) {
        let delta = ctx.mouse - *last_mouse;
        state.yaw -= delta.x * 0.008;
        state.pitch = (state.pitch + delta.y * 0.006).clamp(-1.2, 1.2);
    }
    let wheel = mouse_wheel().1;
    if wheel.abs() > 0.0 {
        state.distance = (state.distance - wheel * 8.0).clamp(40.0, 460.0);
    }

    if let Some(i) = state.selected {
        if let Some(fig) = state.figures.get_mut(i) {
            match state.tool {
                CadTool::Move => apply_move_tool(fig, dt),
                CadTool::Resize => {
                    let delta = nudge_delta(dt);
                    if !state.selected_vertices.is_empty() {
                        apply_vertex_delta(fig, &state.selected_vertices, delta);
                    } else {
                        let handles = resize_handles(fig);
                        if is_key_pressed(KeyCode::Tab) {
                            state.selected_resize_handle =
                                (state.selected_resize_handle + 1) % handles.len();
                        }
                        if delta.length_squared() > 0.0 {
                            let local_delta = figure_quat(fig).conjugate() * delta;
                            let handle = handles[state.selected_resize_handle % handles.len()].kind;
                            apply_resize_with_handle(fig, handle, local_delta);
                        }
                    }
                }
                CadTool::Rotate => {
                    if is_key_pressed(KeyCode::Tab) {
                        state.selected_rotate_axis = (state.selected_rotate_axis + 1) % 3;
                    }
                    let amount = (if is_key_down(KeyCode::Left) {
                        -1.0
                    } else {
                        0.0
                    } + if is_key_down(KeyCode::Right) {
                        1.0
                    } else {
                        0.0
                    } + if is_key_down(KeyCode::Q) { -1.0 } else { 0.0 }
                        + if is_key_down(KeyCode::E) { 1.0 } else { 0.0 })
                        * dt;
                    if amount != 0.0 {
                        rotate_selected(fig, state.selected_rotate_axis, amount);
                    }
                }
                CadTool::Align | CadTool::Join | CadTool::Subtract => {}
            }
        }
    }

    if is_mouse_button_pressed(MouseButton::Left) && ctx.mouse.x > 220.0 {
        if let Some(i) = state.selected {
            if state.tool == CadTool::Resize {
                if let Some(fig) = state.figures.get(i) {
                    if let Some(vertex_idx) = pick_vertex(&camera, ctx.mouse, fig) {
                        let additive =
                            is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                        add_vertex_selection(state, i, vertex_idx, additive);
                    } else {
                        let points: Vec<Vec3> = resize_handles(fig)
                            .iter()
                            .map(|h| local_to_world(fig, h.local_pos))
                            .collect();
                        if let Some(handle) = pick_closest_index(&camera, ctx.mouse, &points, 14.0)
                        {
                            state.selected_resize_handle = handle;
                            state.selected_vertices.clear();
                        }
                    }
                }
            } else if state.tool == CadTool::Rotate {
                if let Some(fig) = state.figures.get(i) {
                    if let Some(axis) =
                        pick_closest_index(&camera, ctx.mouse, &axis_world_positions(fig), 16.0)
                    {
                        state.selected_rotate_axis = axis;
                    }
                }
            }
        }
        if let Some(picked) = pick_figure_by_body(&camera, ctx.mouse, &state.figures) {
            if is_pair_tool(state.tool) {
                pair_selection_click(state, picked);
            } else {
                if state.selected != Some(picked) {
                    state.selected_vertices.clear();
                }
                state.selected = Some(picked);
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
                for vertex_idx in unique_vertex_indices(&fig.mesh) {
                    let Some(pos) = fig.mesh.vertices.get(vertex_idx) else {
                        continue;
                    };
                    let selected = state.selected_vertices.iter().any(|idx| {
                        fig.mesh
                            .vertices
                            .get(*idx)
                            .is_some_and(|anchor| same_vertex(*anchor, *pos))
                    });
                    let color = if selected {
                        colors.hover
                    } else {
                        Color::from_rgba(215, 230, 240, 255)
                    };
                    draw_cube(*pos, vec3(2.8, 2.8, 2.8), None, color);
                }
            } else if state.tool == CadTool::Rotate {
                let axis = axis_world_positions(fig);
                for (i, tip) in axis.iter().enumerate() {
                    let color = if i == state.selected_rotate_axis {
                        colors.hover
                    } else {
                        Color::from_rgba(220, 225, 235, 255)
                    };
                    draw_line_3d(fig.pos, *tip, color);
                    draw_sphere(*tip, 3.0, None, color);
                }
            }
        }
    }
    set_default_camera();

    draw_text(
        "CAD Prototype",
        24.0,
        36.0,
        ctx.font_md,
        colors.text_primary,
    );
    draw_tools_panel(ctx, state, colors);
    let tool = match state.tool {
        CadTool::Move => "Move",
        CadTool::Resize => "Resize",
        CadTool::Rotate => "Rotate",
        CadTool::Align => "Align",
        CadTool::Join => "Join",
        CadTool::Subtract => "Subtract",
    };
    let phase = if is_pair_tool(state.tool) {
        match state.pair_phase {
            PairPhase::PickSource => "pick source",
            PairPhase::PickTarget => "pick target",
        }
    } else {
        "body click selects"
    };
    draw_text(
        &format!("Tool: {} | {}", tool, phase),
        230.0,
        34.0,
        ctx.font_sm,
        colors.text_secondary,
    );
    draw_text(
        &state.status,
        230.0,
        58.0,
        ctx.font_sm,
        colors.text_secondary,
    );
    *last_mouse = ctx.mouse;
}
