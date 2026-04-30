use macroquad::prelude::*;

use crate::core::ui::ui_button;
use crate::core::{FrameContext, RuntimeColors, Scene};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CadPrimitiveKind {
    Square,
    Cylinder,
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

#[derive(Clone, Copy, Debug)]
struct CadPart {
    kind: CadPrimitiveKind,
    pos: Vec3,
    size: Vec3,
    rot: Vec3,
}

#[derive(Clone, Debug)]
struct CadFigure {
    kind: CadPrimitiveKind,
    pos: Vec3,
    size: Vec3,
    rot: Vec3, // x/y/z radians in local form
    parts: Vec<CadPart>,
}

impl CadFigure {
    fn new(kind: CadPrimitiveKind, pos: Vec3, size: Vec3, rot: Vec3) -> Self {
        Self {
            kind,
            pos,
            size,
            rot,
            parts: Vec::new(),
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
    CadFigure::new(
        part.kind,
        local_to_world(parent, part.pos),
        part.size,
        vec3(rx, ry, rz),
    )
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

fn draw_figure_body(fig: &CadFigure, fill: Color, wire: Color) {
    match fig.kind {
        CadPrimitiveKind::Square => draw_oriented_box(fig, fill, wire),
        CadPrimitiveKind::Cylinder => draw_oriented_cylinder(fig, fill, wire),
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
        CadPrimitiveKind::Square => box_handles(fig),
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

fn figure_local_corners(fig: &CadFigure) -> [Vec3; 8] {
    let hx = fig.size.x * 0.5;
    let hy = fig.size.y * 0.5;
    let hz = fig.size.z * 0.5;
    [
        vec3(-hx, -hy, -hz),
        vec3(hx, -hy, -hz),
        vec3(-hx, hy, -hz),
        vec3(hx, hy, -hz),
        vec3(-hx, -hy, hz),
        vec3(hx, -hy, hz),
        vec3(-hx, hy, hz),
        vec3(hx, hy, hz),
    ]
}

fn approximate_aabb_in_space(fig: &CadFigure, space: &CadFigure) -> (Vec3, Vec3) {
    // Approximate fig by AABB expressed in space figure local basis.
    let mut min_v = vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max_v = vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for corner in figure_local_corners(fig) {
        let world = local_to_world(fig, corner);
        let in_space = world_to_local(space, world);
        min_v = min_v.min(in_space);
        max_v = max_v.max(in_space);
    }
    (min_v, max_v)
}

fn base_local_aabb(base: &CadFigure) -> (Vec3, Vec3) {
    let half = base.size * 0.5;
    (-half, half)
}

fn figure_from_local_aabb(min_v: Vec3, max_v: Vec3, space: &CadFigure) -> CadFigure {
    let center_local = (min_v + max_v) * 0.5;
    CadFigure::new(
        CadPrimitiveKind::Square,
        local_to_world(space, center_local),
        (max_v - min_v).max(vec3(0.0, 0.0, 0.0)),
        space.rot,
    )
}

fn part_from_world_figure(fig: &CadFigure, root: &CadFigure) -> CadPart {
    let q = (figure_quat(root).conjugate() * figure_quat(fig)).normalize();
    let (rx, ry, rz) = q.to_euler(EulerRot::XYZ);
    CadPart {
        kind: fig.kind,
        pos: world_to_local(root, fig.pos),
        size: fig.size,
        rot: vec3(rx, ry, rz),
    }
}

fn build_composite_from_world_parts(parts: Vec<CadFigure>, space: &CadFigure) -> Option<CadFigure> {
    // Result keeps real pieces; root only supplies transform, selection, and gizmo bounds.
    if parts.is_empty() {
        return None;
    }
    if parts.len() == 1 {
        return parts.into_iter().next();
    }

    let mut min_v = vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max_v = vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for part in &parts {
        let (part_min, part_max) = approximate_aabb_in_space(part, space);
        min_v = min_v.min(part_min);
        max_v = max_v.max(part_max);
    }

    let center = (min_v + max_v) * 0.5;
    let mut root = CadFigure::new(
        CadPrimitiveKind::Square,
        local_to_world(space, center),
        (max_v - min_v).max(vec3(0.1, 0.1, 0.1)),
        space.rot,
    );
    root.parts = parts
        .iter()
        .map(|part| part_from_world_figure(part, &root))
        .collect();
    Some(root)
}

fn apply_join(base: CadFigure, tool: CadFigure) -> Option<CadFigure> {
    let mut parts = figure_leaf_world_figures(&base);
    parts.extend(figure_leaf_world_figures(&tool));
    build_composite_from_world_parts(parts, &base)
}

fn overlap_aabb(a_min: Vec3, a_max: Vec3, b_min: Vec3, b_max: Vec3) -> Option<(Vec3, Vec3)> {
    let min_v = a_min.max(b_min);
    let max_v = a_max.min(b_max);
    if min_v.x < max_v.x && min_v.y < max_v.y && min_v.z < max_v.z {
        Some((min_v, max_v))
    } else {
        None
    }
}

fn push_aabb_piece(out: &mut Vec<(Vec3, Vec3)>, min_v: Vec3, max_v: Vec3) {
    const EPS: f32 = 0.05;
    if max_v.x - min_v.x > EPS && max_v.y - min_v.y > EPS && max_v.z - min_v.z > EPS {
        out.push((min_v, max_v));
    }
}

fn split_aabb_difference(a_min: Vec3, a_max: Vec3, o_min: Vec3, o_max: Vec3) -> Vec<(Vec3, Vec3)> {
    // Six-piece box difference: remove overlap, keep every remaining slab.
    let mut out = Vec::new();

    push_aabb_piece(
        &mut out,
        vec3(a_min.x, a_min.y, a_min.z),
        vec3(o_min.x, a_max.y, a_max.z),
    );
    push_aabb_piece(
        &mut out,
        vec3(o_max.x, a_min.y, a_min.z),
        vec3(a_max.x, a_max.y, a_max.z),
    );

    let mid_x_min = o_min.x.max(a_min.x);
    let mid_x_max = o_max.x.min(a_max.x);
    push_aabb_piece(
        &mut out,
        vec3(mid_x_min, a_min.y, a_min.z),
        vec3(mid_x_max, o_min.y, a_max.z),
    );
    push_aabb_piece(
        &mut out,
        vec3(mid_x_min, o_max.y, a_min.z),
        vec3(mid_x_max, a_max.y, a_max.z),
    );

    let mid_y_min = o_min.y.max(a_min.y);
    let mid_y_max = o_max.y.min(a_max.y);
    push_aabb_piece(
        &mut out,
        vec3(mid_x_min, mid_y_min, a_min.z),
        vec3(mid_x_max, mid_y_max, o_min.z),
    );
    push_aabb_piece(
        &mut out,
        vec3(mid_x_min, mid_y_min, o_max.z),
        vec3(mid_x_max, mid_y_max, a_max.z),
    );

    out
}

fn overlap_consumes_aabb(a_min: Vec3, a_max: Vec3, o_min: Vec3, o_max: Vec3) -> bool {
    const EPS: f32 = 0.05;
    o_min.x <= a_min.x + EPS
        && o_min.y <= a_min.y + EPS
        && o_min.z <= a_min.z + EPS
        && o_max.x >= a_max.x - EPS
        && o_max.y >= a_max.y - EPS
        && o_max.z >= a_max.z - EPS
}

fn subtract_leaf_by_tool(base_leaf: CadFigure, tool_leaf: &CadFigure) -> Vec<CadFigure> {
    let (a_min, a_max) = base_local_aabb(&base_leaf);
    let (b_min, b_max) = approximate_aabb_in_space(tool_leaf, &base_leaf);
    let Some((o_min, o_max)) = overlap_aabb(a_min, a_max, b_min, b_max) else {
        return vec![base_leaf];
    };

    if overlap_consumes_aabb(a_min, a_max, o_min, o_max) {
        return Vec::new();
    }

    split_aabb_difference(a_min, a_max, o_min, o_max)
        .into_iter()
        .map(|(min_v, max_v)| figure_from_local_aabb(min_v, max_v, &base_leaf))
        .collect()
}

fn apply_subtract(base: CadFigure, tool: CadFigure) -> Option<CadFigure> {
    let tool_leaves = figure_leaf_world_figures(&tool);
    let mut result_leaves = figure_leaf_world_figures(&base);

    for tool_leaf in &tool_leaves {
        let mut next = Vec::new();
        for base_leaf in result_leaves {
            next.extend(subtract_leaf_by_tool(base_leaf, tool_leaf));
        }
        if next.is_empty() {
            return None;
        }
        result_leaves = next;
    }

    build_composite_from_world_parts(result_leaves, &base)
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
    let panel = Rect::new(screen_width() - 290.0, 24.0, 266.0, 560.0);
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
}
