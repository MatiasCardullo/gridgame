use std::collections::{HashMap, HashSet};

use macroquad::prelude::*;

use crate::core::object_assets::{
    EditableMesh, FaceGroupProgram, FaceMetadata, MeshPrimitive, ObjectCatalog, ObjectCategory,
    ObjectId, RadialNeighborState,
};
use crate::core::ui::{fit_status_text, ui_button};
use crate::core::{FrameContext, Scene};

const SIDEBAR_WIDTH: f32 = 250.0;
const TOOLBAR_HEIGHT: f32 = 62.0;
const STATUS_BAR_HEIGHT: f32 = 28.0;
const CAMERA_MIN_DISTANCE: f32 = 1.5;
const CAMERA_MAX_DISTANCE: f32 = 100.0;
const HISTORY_LIMIT: usize = 100;
const FACE_LIST_TOP: f32 = 148.0;
const FACE_ROW_HEIGHT: f32 = 26.0;
const MAX_TRANSFORM_VALUE: f32 = 1_000_000.0;
const MAX_SCALE_FACTOR: f32 = 1_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionMode {
    Vertex,
    Edge,
    Face,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TrimPhase {
    #[default]
    Targets,
    Cutters,
}

#[derive(Clone, Copy)]
enum GroupFlag {
    Locked,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransformMode {
    Move,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransformAxis {
    All,
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ObjectSnapKind {
    Center,
    Tangent,
    Quadrant,
    Intersection,
    Endpoint,
    Midpoint,
    Perpendicular,
}

#[derive(Clone, Copy, Debug)]
struct ObjectSnapSettings {
    center: bool,
    tangent: bool,
    quadrant: bool,
    intersection: bool,
    endpoint: bool,
    midpoint: bool,
    perpendicular: bool,
}

impl Default for ObjectSnapSettings {
    fn default() -> Self {
        Self {
            center: true,
            tangent: true,
            quadrant: true,
            intersection: true,
            endpoint: true,
            midpoint: true,
            perpendicular: true,
        }
    }
}

impl ObjectSnapSettings {
    fn enabled(self, kind: ObjectSnapKind) -> bool {
        match kind {
            ObjectSnapKind::Center => self.center,
            ObjectSnapKind::Tangent => self.tangent,
            ObjectSnapKind::Quadrant => self.quadrant,
            ObjectSnapKind::Intersection => self.intersection,
            ObjectSnapKind::Endpoint => self.endpoint,
            ObjectSnapKind::Midpoint => self.midpoint,
            ObjectSnapKind::Perpendicular => self.perpendicular,
        }
    }

    fn set(&mut self, kind: ObjectSnapKind, enabled: bool) {
        match kind {
            ObjectSnapKind::Center => self.center = enabled,
            ObjectSnapKind::Tangent => self.tangent = enabled,
            ObjectSnapKind::Quadrant => self.quadrant = enabled,
            ObjectSnapKind::Intersection => self.intersection = enabled,
            ObjectSnapKind::Endpoint => self.endpoint = enabled,
            ObjectSnapKind::Midpoint => self.midpoint = enabled,
            ObjectSnapKind::Perpendicular => self.perpendicular = enabled,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ObjectSnapCandidate {
    kind: ObjectSnapKind,
    position: Vec3,
}

#[derive(Clone, Debug)]
struct CircularLoop {
    center: Vec3,
    normal: Vec3,
    u: Vec3,
    v: Vec3,
    radius: f32,
    vertices: Vec<Vec3>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TransformCommand {
    mode: TransformMode,
    pivot: Vec3,
    values: Vec3,
}

#[derive(Clone, Copy, Debug)]
struct GizmoDrag {
    mode: TransformMode,
    axis: TransformAxis,
    pivot_model: Vec3,
    pivot_display: Vec3,
    plane_normal: Vec3,
    handle_size: f32,
    start_parameter: f32,
    start_vector: Vec3,
}

#[derive(Clone, Copy, Debug)]
struct GizmoUpdate {
    command: TransformCommand,
    snap_target: Option<ObjectSnapCandidate>,
    polar_angle: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenameTarget {
    Face(usize),
    Group(u64),
}

#[derive(Clone, Debug)]
struct RenameDialog {
    target: RenameTarget,
    text: String,
}

#[derive(Clone, Copy, Debug)]
struct GroupConfigDialog {
    group_id: u64,
    program: FaceGroupProgram,
}

#[derive(Clone, Debug)]
struct EditSnapshot {
    mesh: EditableMesh,
    metadata: FaceMetadata,
    selected_faces: Vec<usize>,
    selected_group: Option<u64>,
    selected_vertices: Vec<usize>,
    selected_edges: Vec<(usize, usize)>,
    selection_mode: SelectionMode,
}

#[derive(Clone, Debug)]
struct PrimitivePlacement {
    primitive: MeshPrimitive,
    center: Option<Vec3>,
    dimensions: String,
    input_focused: bool,
    input_selected: bool,
    dimension_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SidebarItem {
    Group(u64),
    Face { index: usize, group: Option<u64> },
}

#[derive(Clone, Debug)]
pub struct CadSceneState {
    category: ObjectCategory,
    selected: ObjectId,
    editing: bool,
    dirty: bool,
    exit_confirmation_open: bool,
    selection_mode: SelectionMode,
    face_scroll: usize,
    selected_faces: Vec<usize>,
    selected_group: Option<u64>,
    selected_vertices: Vec<usize>,
    selected_edges: Vec<(usize, usize)>,
    dragged_faces: Option<Vec<usize>>,
    rename_dialog: Option<RenameDialog>,
    transform_mode: TransformMode,
    gizmo_drag: Option<GizmoDrag>,
    gizmo_preview: Option<TransformCommand>,
    gizmo_snap_target: Option<ObjectSnapCandidate>,
    gizmo_polar_angle: Option<f32>,
    numeric_transform_open: bool,
    numeric_transform_focused: bool,
    numeric_transform_selected: bool,
    numeric_transform_values: String,
    object_snaps: ObjectSnapSettings,
    snap_settings_open: bool,
    polar_tracking: bool,
    group_config_dialog: Option<GroupConfigDialog>,
    trim_active: bool,
    trim_phase: TrimPhase,
    trim_target_faces: Vec<usize>,
    trim_cutter_faces: Vec<usize>,
    trim_path_active: bool,
    trim_path_points: Vec<Vec3>,
    trim_path_face: Option<usize>,
    mirror_active: bool,
    mirror_points: Vec<Vec3>,
    fillet_active: bool,
    offset_active: bool,
    operation_value: String,
    operation_value_focused: bool,
    operation_value_selected: bool,
    paint_active: bool,
    paint_color: Color,
    paint_hex: String,
    paint_hex_focused: bool,
    paint_hex_selected: bool,
    primitive_placement: Option<PrimitivePlacement>,
    grid_snap: bool,
    grid_spacing: f32,
    interaction_preview: bool,
    placing_radial_center: bool,
    preview_seed: u64,
    preview_neighbors: [RadialNeighborState; 6],
    original_mesh: Option<EditableMesh>,
    original_metadata: Option<FaceMetadata>,
    undo_stack: Vec<EditSnapshot>,
    redo_stack: Vec<EditSnapshot>,
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: Vec3,
    last_mouse: Vec2,
    error: Option<String>,
}

impl Default for CadSceneState {
    fn default() -> Self {
        Self {
            category: ObjectCategory::Buildings,
            selected: ObjectId::PlanetBase,
            editing: false,
            dirty: false,
            exit_confirmation_open: false,
            selection_mode: SelectionMode::Vertex,
            face_scroll: 0,
            selected_faces: Vec::new(),
            selected_group: None,
            selected_vertices: Vec::new(),
            selected_edges: Vec::new(),
            dragged_faces: None,
            rename_dialog: None,
            transform_mode: TransformMode::Move,
            gizmo_drag: None,
            gizmo_preview: None,
            gizmo_snap_target: None,
            gizmo_polar_angle: None,
            numeric_transform_open: false,
            numeric_transform_focused: false,
            numeric_transform_selected: false,
            numeric_transform_values: "0, 0, 0".to_string(),
            object_snaps: ObjectSnapSettings::default(),
            snap_settings_open: false,
            polar_tracking: false,
            group_config_dialog: None,
            trim_active: false,
            trim_phase: TrimPhase::Targets,
            trim_target_faces: Vec::new(),
            trim_cutter_faces: Vec::new(),
            trim_path_active: false,
            trim_path_points: Vec::new(),
            trim_path_face: None,
            mirror_active: false,
            mirror_points: Vec::new(),
            fillet_active: false,
            offset_active: false,
            operation_value: "0.1".to_string(),
            operation_value_focused: false,
            operation_value_selected: false,
            paint_active: false,
            paint_color: Color::from_rgba(205, 190, 160, 255),
            paint_hex: "CDBEA0".to_string(),
            paint_hex_focused: false,
            paint_hex_selected: false,
            primitive_placement: None,
            grid_snap: false,
            grid_spacing: 0.5,
            interaction_preview: false,
            placing_radial_center: false,
            preview_seed: 1,
            preview_neighbors: [RadialNeighborState::Empty; 6],
            original_mesh: None,
            original_metadata: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            yaw: 0.7,
            pitch: 0.35,
            distance: 4.0,
            target: Vec3::ZERO,
            last_mouse: Vec2::ZERO,
            error: None,
        }
    }
}

fn object_camera(state: &CadSceneState) -> Camera3D {
    let direction = vec3(
        state.yaw.cos() * state.pitch.cos(),
        state.pitch.sin(),
        state.yaw.sin() * state.pitch.cos(),
    )
    .normalize_or_zero();
    Camera3D {
        position: state.target + direction * state.distance,
        target: state.target,
        up: Vec3::Y,
        fovy: 45.0,
        z_near: 0.05,
        z_far: 500.0,
        ..Default::default()
    }
}

fn zoom_distance(distance: f32, wheel: f32) -> f32 {
    (distance * (-wheel * 0.01).exp()).clamp(CAMERA_MIN_DISTANCE, CAMERA_MAX_DISTANCE)
}

fn vec3_is_finite(value: Vec3) -> bool {
    value.x.is_finite() && value.y.is_finite() && value.z.is_finite()
}

fn parse_vec3_input(text: &str, label: &str) -> Result<Vec3, String> {
    let parts = if text.contains(',') {
        text.split(',').collect::<Vec<_>>()
    } else {
        text.split_whitespace().collect::<Vec<_>>()
    };
    if parts.len() != 3 {
        return Err(format!("Enter {label} as three X, Y, Z values"));
    }
    let values = parts
        .into_iter()
        .map(|value| {
            let value = value.trim();
            if value.is_empty() {
                return Err(format!("Enter {label} as three X, Y, Z values"));
            }
            value
                .parse::<f32>()
                .map_err(|_| format!("Invalid {label} value '{value}'"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value = vec3(values[0], values[1], values[2]);
    if !vec3_is_finite(value) {
        return Err(format!("{label} values must be finite"));
    }
    Ok(value)
}

fn parse_transform_values(mode: TransformMode, text: &str) -> Result<Vec3, String> {
    let values = parse_vec3_input(text, "transform")?;
    match mode {
        TransformMode::Scale
            if [values.x, values.y, values.z]
                .iter()
                .any(|value| *value <= 0.0 || *value > MAX_SCALE_FACTOR) =>
        {
            Err(format!(
                "Scale factors must be greater than 0 and at most {MAX_SCALE_FACTOR}"
            ))
        }
        TransformMode::Move | TransformMode::Rotate
            if [values.x, values.y, values.z]
                .iter()
                .any(|value| value.abs() > MAX_TRANSFORM_VALUE) =>
        {
            Err(format!(
                "Transform values must be between -{MAX_TRANSFORM_VALUE} and {MAX_TRANSFORM_VALUE}"
            ))
        }
        _ => Ok(values),
    }
}

fn constrained_transform_values(mode: TransformMode, axis: TransformAxis, values: Vec3) -> Vec3 {
    let unchanged = if mode == TransformMode::Scale {
        1.0
    } else {
        0.0
    };
    match axis {
        TransformAxis::All => values,
        TransformAxis::X => vec3(values.x, unchanged, unchanged),
        TransformAxis::Y => vec3(unchanged, values.y, unchanged),
        TransformAxis::Z => vec3(unchanged, unchanged, values.z),
    }
}

fn transform_position(position: Vec3, command: TransformCommand) -> Vec3 {
    match command.mode {
        TransformMode::Move => position + command.values,
        TransformMode::Rotate => {
            let mut relative = position - command.pivot;
            let radians = [
                command.values.x.to_radians(),
                command.values.y.to_radians(),
                command.values.z.to_radians(),
            ];
            let (sin_x, cos_x) = radians[0].sin_cos();
            relative = vec3(
                relative.x,
                relative.y * cos_x - relative.z * sin_x,
                relative.y * sin_x + relative.z * cos_x,
            );
            let (sin_y, cos_y) = radians[1].sin_cos();
            relative = vec3(
                relative.x * cos_y + relative.z * sin_y,
                relative.y,
                -relative.x * sin_y + relative.z * cos_y,
            );
            let (sin_z, cos_z) = radians[2].sin_cos();
            relative = vec3(
                relative.x * cos_z - relative.y * sin_z,
                relative.x * sin_z + relative.y * cos_z,
                relative.z,
            );
            command.pivot + relative
        }
        TransformMode::Scale => {
            let relative = position - command.pivot;
            command.pivot
                + vec3(
                    relative.x * command.values.x,
                    relative.y * command.values.y,
                    relative.z * command.values.z,
                )
        }
    }
}

fn apply_transform_to_indices(
    mesh: &mut EditableMesh,
    indices: &[usize],
    command: TransformCommand,
) -> Result<(), String> {
    if indices.is_empty() {
        return Err("Select vertices, edges, or faces before transforming".to_string());
    }
    if !vec3_is_finite(command.pivot) || !vec3_is_finite(command.values) {
        return Err("Transform values must be finite".to_string());
    }
    if command.mode == TransformMode::Scale
        && [command.values.x, command.values.y, command.values.z]
            .iter()
            .any(|value| *value <= 0.0 || *value > MAX_SCALE_FACTOR)
    {
        return Err("Scale factors must be greater than 0 and at most 1000".to_string());
    }

    let transformed = indices
        .iter()
        .map(|&index| {
            let vertex = mesh
                .vertices
                .get(index)
                .ok_or_else(|| format!("Selected vertex {index} is out of range"))?;
            let position = transform_position(vertex.position, command);
            if !vec3_is_finite(position) {
                return Err("Transform would create non-finite vertex coordinates".to_string());
            }
            Ok((index, position))
        })
        .collect::<Result<Vec<_>, String>>()?;

    for (index, position) in transformed {
        mesh.vertices[index].position = position;
    }
    Ok(())
}

fn transform_has_effect(command: TransformCommand) -> bool {
    match command.mode {
        TransformMode::Move => command.values.length_squared() > 1e-8,
        TransformMode::Rotate => command.values.length_squared() > 1e-8,
        TransformMode::Scale => (command.values - Vec3::ONE).length_squared() > 1e-8,
    }
}

fn axis_direction(axis: TransformAxis) -> Option<Vec3> {
    match axis {
        TransformAxis::All => None,
        TransformAxis::X => Some(Vec3::X),
        TransformAxis::Y => Some(Vec3::Y),
        TransformAxis::Z => Some(Vec3::Z),
    }
}

fn axis_color(axis: TransformAxis) -> Color {
    match axis {
        TransformAxis::X => Color::from_rgba(245, 80, 80, 255),
        TransformAxis::Y => Color::from_rgba(90, 225, 125, 255),
        TransformAxis::Z => Color::from_rgba(85, 155, 255, 255),
        TransformAxis::All => WHITE,
    }
}

fn axis_plane_basis(axis: TransformAxis) -> Option<(Vec3, Vec3)> {
    match axis {
        TransformAxis::X => Some((Vec3::Y, Vec3::Z)),
        TransformAxis::Y => Some((Vec3::X, Vec3::Z)),
        TransformAxis::Z => Some((Vec3::X, Vec3::Y)),
        TransformAxis::All => None,
    }
}

fn axis_screen_parameter(
    camera: &Camera3D,
    mouse: Vec2,
    pivot: Vec3,
    axis: TransformAxis,
    handle_size: f32,
) -> Option<f32> {
    let direction = axis_direction(axis)?;
    let start = project_to_screen(camera, pivot)?;
    let end = project_to_screen(camera, pivot + direction * handle_size)?;
    let projected_axis = end - start;
    let projected_length = projected_axis.length();
    if projected_length < 2.0 {
        return None;
    }
    Some((mouse - start).dot(projected_axis / projected_length) * handle_size / projected_length)
}

fn pick_gizmo_axis(
    camera: &Camera3D,
    mouse: Vec2,
    pivot: Vec3,
    mode: TransformMode,
    handle_size: f32,
) -> Option<TransformAxis> {
    if mode == TransformMode::Move
        && project_to_screen(camera, pivot).is_some_and(|screen| screen.distance(mouse) < 12.0)
    {
        return Some(TransformAxis::All);
    }
    let mut closest = None;
    let mut closest_distance = f32::INFINITY;
    for axis in [TransformAxis::X, TransformAxis::Y, TransformAxis::Z] {
        let Some(direction) = axis_direction(axis) else {
            continue;
        };
        if mode == TransformMode::Rotate {
            let Some((u, v)) = axis_plane_basis(axis) else {
                continue;
            };
            let mut previous = None;
            for step in 0..=48 {
                let angle = std::f32::consts::TAU * step as f32 / 48.0;
                let point = pivot + (u * angle.cos() + v * angle.sin()) * handle_size * 0.78;
                let Some(screen) = project_to_screen(camera, point) else {
                    previous = None;
                    continue;
                };
                if let Some(start) = previous {
                    let distance = point_segment_distance(mouse, start, screen);
                    if distance < 10.0 && distance < closest_distance {
                        closest = Some(axis);
                        closest_distance = distance;
                    }
                }
                previous = Some(screen);
            }
        } else {
            let (Some(start), Some(end)) = (
                project_to_screen(camera, pivot),
                project_to_screen(camera, pivot + direction * handle_size),
            ) else {
                continue;
            };
            let distance = point_segment_distance(mouse, start, end);
            if mouse.distance(start) > 10.0 && distance < 10.0 && distance < closest_distance {
                closest = Some(axis);
                closest_distance = distance;
            }
        }
    }
    closest
}

fn ray_plane_hit(origin: Vec3, direction: Vec3, point: Vec3, normal: Vec3) -> Option<Vec3> {
    let denominator = direction.dot(normal);
    if denominator.abs() < 1e-6 {
        return None;
    }
    let distance = (point - origin).dot(normal) / denominator;
    (distance >= 0.0).then(|| origin + direction * distance)
}

fn begin_gizmo_drag(
    camera: &Camera3D,
    mouse: Vec2,
    mesh: &EditableMesh,
    state: &CadSceneState,
    mode: TransformMode,
    handle_size: f32,
) -> Option<GizmoDrag> {
    let indices = selected_indices(mesh, state);
    let pivot_model = selected_bounds_center(mesh, &indices).ok()?;
    let pivot_display = model_to_display_point(mesh, pivot_model);
    let axis = pick_gizmo_axis(camera, mouse, pivot_display, mode, handle_size)?;
    if mode == TransformMode::Move && axis == TransformAxis::All {
        let plane_normal = (camera.target - camera.position).normalize_or_zero();
        let (origin, direction) = mouse_ray(camera, mouse)?;
        let point = ray_plane_hit(origin, direction, pivot_display, plane_normal)?;
        Some(GizmoDrag {
            mode,
            axis,
            pivot_model,
            pivot_display,
            plane_normal,
            handle_size,
            start_parameter: 0.0,
            start_vector: point - pivot_display,
        })
    } else if mode == TransformMode::Rotate {
        let plane_normal = axis_direction(axis)?;
        let (origin, direction) = mouse_ray(camera, mouse)?;
        let point = ray_plane_hit(origin, direction, pivot_display, plane_normal)?;
        let start_vector = point - pivot_display;
        (start_vector.length_squared() > 1e-8).then_some(GizmoDrag {
            mode,
            axis,
            pivot_model,
            pivot_display,
            plane_normal,
            handle_size,
            start_parameter: 0.0,
            start_vector,
        })
    } else {
        let start_parameter =
            axis_screen_parameter(camera, mouse, pivot_display, axis, handle_size)?;
        Some(GizmoDrag {
            mode,
            axis,
            pivot_model,
            pivot_display,
            plane_normal: Vec3::ZERO,
            handle_size,
            start_parameter,
            start_vector: Vec3::ZERO,
        })
    }
}

fn update_gizmo_drag(
    camera: &Camera3D,
    mouse: Vec2,
    mesh: &EditableMesh,
    state: &CadSceneState,
    radial_grid_center: Option<Vec3>,
    drag: GizmoDrag,
) -> Option<GizmoUpdate> {
    let values = match drag.mode {
        TransformMode::Move => {
            let (_, display_scale) = mesh_center_and_scale(mesh);
            if drag.axis == TransformAxis::All {
                let (origin, ray_direction) = mouse_ray(camera, mouse)?;
                let point =
                    ray_plane_hit(origin, ray_direction, drag.pivot_display, drag.plane_normal)?;
                (point - drag.pivot_display - drag.start_vector) / display_scale
            } else {
                let direction = axis_direction(drag.axis)?;
                let current = axis_screen_parameter(
                    camera,
                    mouse,
                    drag.pivot_display,
                    drag.axis,
                    drag.handle_size,
                )?;
                direction * ((current - drag.start_parameter) / display_scale)
            }
        }
        TransformMode::Scale => {
            let direction = axis_direction(drag.axis)?;
            let current = axis_screen_parameter(
                camera,
                mouse,
                drag.pivot_display,
                drag.axis,
                drag.handle_size,
            )?;
            let factor = (1.0 + (current - drag.start_parameter) / drag.handle_size)
                .clamp(0.01, MAX_SCALE_FACTOR);
            Vec3::ONE + direction * (factor - 1.0)
        }
        TransformMode::Rotate => {
            let direction = axis_direction(drag.axis)?;
            let (origin, ray_direction) = mouse_ray(camera, mouse)?;
            let point = ray_plane_hit(origin, ray_direction, drag.pivot_display, direction)?;
            let current_vector = point - drag.pivot_display;
            if current_vector.length_squared() <= 1e-8 {
                return None;
            }
            let angle = direction
                .dot(drag.start_vector.cross(current_vector))
                .atan2(drag.start_vector.dot(current_vector))
                .to_degrees();
            direction * angle
        }
    };
    let mut command = TransformCommand {
        mode: drag.mode,
        pivot: drag.pivot_model,
        values,
    };
    let snap_target = if drag.mode == TransformMode::Move && drag.axis == TransformAxis::All {
        let display_mesh = preview_mesh(mesh);
        nearest_object_snap(
            camera,
            mouse,
            mesh,
            &display_mesh,
            state,
            state.object_snaps,
        )
    } else {
        None
    };
    let mut polar_angle = None;
    if let Some(target) = snap_target {
        command.values = target.position - drag.pivot_model;
    } else if drag.mode == TransformMode::Move {
        if state.grid_snap {
            let mut snapped = if let Some(center) = radial_grid_center {
                snap_transform_to_triangular_grid(
                    drag.pivot_model,
                    command.values,
                    drag.axis,
                    state.grid_spacing,
                    center,
                )
            } else {
                snap_transform_to_grid(
                    drag.pivot_model,
                    command.values,
                    drag.axis,
                    state.grid_spacing,
                )
            };
            if matches!(drag.axis, TransformAxis::All | TransformAxis::Y) {
                let floor_y = radial_grid_center
                    .map(|center| center.y)
                    .unwrap_or_else(|| mesh_center_and_scale(mesh).0.y);
                snapped = snap_point_to_grid_floor(snapped, floor_y, state.grid_spacing);
            }
            command.values = snapped - drag.pivot_model;
        } else if drag.axis == TransformAxis::All && state.polar_tracking {
            let forward = (camera.target - camera.position).normalize_or_zero();
            let right = forward.cross(camera.up).normalize_or_zero();
            let up = right.cross(forward).normalize_or_zero();
            let display_scale = mesh_center_and_scale(mesh).1;
            let (tracked, angle) =
                polar_track_delta(command.values * display_scale, right, up, 15.0);
            command.values = tracked / display_scale;
            polar_angle = Some(angle);
        }
    }
    Some(GizmoUpdate {
        command,
        snap_target,
        polar_angle,
    })
}

fn gizmo_center_and_size(mesh: &EditableMesh, state: &CadSceneState) -> Option<(Vec3, f32)> {
    let indices = selected_indices(mesh, state);
    let center = selected_bounds_center(mesh, &indices).ok()?;
    Some((
        model_to_display_point(mesh, center),
        (state.distance * 0.13).clamp(0.38, 0.85),
    ))
}

fn draw_transform_gizmo(center: Vec3, mode: TransformMode, handle_size: f32) {
    for axis in [TransformAxis::X, TransformAxis::Y, TransformAxis::Z] {
        let direction = axis_direction(axis).unwrap_or(Vec3::ZERO);
        let color = axis_color(axis);
        if mode == TransformMode::Rotate {
            let (u, v) = axis_plane_basis(axis).unwrap_or((Vec3::X, Vec3::Y));
            let radius = handle_size * 0.78;
            let mut previous = center + u * radius;
            for step in 1..=48 {
                let angle = std::f32::consts::TAU * step as f32 / 48.0;
                let point = center + (u * angle.cos() + v * angle.sin()) * radius;
                draw_line_3d(previous, point, color);
                previous = point;
            }
        } else {
            let endpoint = center + direction * handle_size;
            draw_line_3d(center, endpoint, color);
            let marker_size = handle_size * 0.09;
            if mode == TransformMode::Scale {
                draw_cube(
                    endpoint,
                    vec3(marker_size, marker_size, marker_size),
                    None,
                    color,
                );
            } else {
                draw_sphere(endpoint, marker_size * 0.55, None, color);
            }
        }
    }
    draw_sphere(
        center,
        handle_size
            * if mode == TransformMode::Move {
                0.09
            } else {
                0.045
            },
        None,
        Color::from_rgba(245, 245, 245, 255),
    );
}

fn mesh_center_and_scale(mesh: &EditableMesh) -> (Vec3, f32) {
    let mut min = vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for vertex in &mesh.vertices {
        min = min.min(vertex.position);
        max = max.max(vertex.position);
    }
    let center = (min + max) * 0.5;
    let extent = max - min;
    let scale = 2.4 / extent.x.max(extent.y).max(extent.z).max(0.001);
    (center, scale)
}

fn preview_mesh(mesh: &EditableMesh) -> EditableMesh {
    preview_mesh_with_reference(mesh, mesh)
}

fn preview_mesh_with_reference(mesh: &EditableMesh, reference: &EditableMesh) -> EditableMesh {
    preview_mesh_with_reference_at(mesh, reference, None)
}

fn preview_mesh_with_reference_at(
    mesh: &EditableMesh,
    reference: &EditableMesh,
    anchor: Option<Vec3>,
) -> EditableMesh {
    let (reference_center, scale) = mesh_center_and_scale(reference);
    let center = anchor.unwrap_or(reference_center);
    mesh.transformed(|position| (position - center) * scale)
}

fn construction_plane_hit(origin: Vec3, direction: Vec3) -> Option<Vec3> {
    if direction.y.abs() <= 1e-6 {
        return None;
    }
    let distance = -origin.y / direction.y;
    (distance >= 0.0).then(|| origin + direction * distance)
}

fn display_to_model_point(mesh: &EditableMesh, point: Vec3) -> Vec3 {
    let (center, scale) = mesh_center_and_scale(mesh);
    point / scale + center
}

fn model_to_display_point(mesh: &EditableMesh, point: Vec3) -> Vec3 {
    let (center, scale) = mesh_center_and_scale(mesh);
    (point - center) * scale
}

fn snap_model_point(point: Vec3, spacing: f32) -> Vec3 {
    let spacing = spacing.max(0.001);
    vec3(
        (point.x / spacing).round() * spacing,
        point.y,
        (point.z / spacing).round() * spacing,
    )
}

fn snap_model_point_to_triangular_grid(point: Vec3, center: Vec3, spacing: f32) -> Vec3 {
    let spacing = spacing.max(0.001);
    let relative = point - center;
    let root_three = 3.0_f32.sqrt();
    let axial_q = relative.x / spacing - relative.z / (root_three * spacing);
    let axial_r = 2.0 * relative.z / (root_three * spacing);
    let q_center = axial_q.round() as i32;
    let r_center = axial_r.round() as i32;
    let mut closest = Vec3::ZERO;
    let mut closest_distance = f32::INFINITY;
    for q in q_center - 1..=q_center + 1 {
        for r in r_center - 1..=r_center + 1 {
            let candidate = vec3(
                (q as f32 + r as f32 * 0.5) * spacing,
                relative.y,
                r as f32 * root_three * 0.5 * spacing,
            );
            let distance = (candidate.x - relative.x).powi(2) + (candidate.z - relative.z).powi(2);
            if distance < closest_distance {
                closest = candidate;
                closest_distance = distance;
            }
        }
    }
    center + closest
}

fn snap_point_to_grid_floor(point: Vec3, floor_y: f32, spacing: f32) -> Vec3 {
    let snap_distance = spacing.max(0.001) * 0.25;
    if (point.y - floor_y).abs() <= snap_distance {
        vec3(point.x, floor_y, point.z)
    } else {
        point
    }
}

fn grid_center_for_mesh(mesh: &EditableMesh, metadata: &FaceMetadata) -> (Vec3, bool) {
    if metadata.radial_main_group_id().is_some() {
        let center = metadata
            .radial_pivot
            .map(|[x, y, z]| vec3(x, y, z))
            .unwrap_or(Vec3::ZERO);
        (center, true)
    } else {
        let model_center = mesh_center_and_scale(mesh).0;
        (vec3(0.0, model_center.y, 0.0), false)
    }
}

fn snap_model_point_to_grid_layout(
    point: Vec3,
    center: Vec3,
    spacing: f32,
    triangular: bool,
) -> Vec3 {
    let snapped = if triangular {
        snap_model_point_to_triangular_grid(point, center, spacing)
    } else {
        snap_model_point(point, spacing)
    };
    snap_point_to_grid_floor(snapped, center.y, spacing)
}

fn primitive_dimension_count(primitive: MeshPrimitive) -> usize {
    match primitive {
        MeshPrimitive::Cube | MeshPrimitive::Sphere => 1,
        MeshPrimitive::Plane | MeshPrimitive::Cylinder => 2,
    }
}

fn primitive_default_dimensions(primitive: MeshPrimitive) -> &'static str {
    match primitive {
        MeshPrimitive::Cube => "1",
        MeshPrimitive::Plane => "1, 1",
        MeshPrimitive::Sphere => "0.5",
        MeshPrimitive::Cylinder => "0.5, 1",
    }
}

fn parse_dimension_values(primitive: MeshPrimitive, text: &str) -> Result<Vec<f32>, String> {
    let count = primitive_dimension_count(primitive);
    let values = text
        .split(|character: char| character == ',' || character.is_whitespace())
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<f32>()
                .map_err(|_| format!("Invalid dimension '{value}'"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != count {
        return Err(match primitive {
            MeshPrimitive::Cube => "Enter one cube edge length".to_string(),
            MeshPrimitive::Plane => {
                "Enter plane width and length, separated by a comma".to_string()
            }
            MeshPrimitive::Sphere => "Enter one sphere radius".to_string(),
            MeshPrimitive::Cylinder => {
                "Enter cylinder radius and height, separated by a comma".to_string()
            }
        });
    }
    if values
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0 || *value > 1000.0)
    {
        return Err("Dimensions must be finite, greater than 0, and at most 1000".to_string());
    }
    Ok(values)
}

fn parse_primitive_dimensions(primitive: MeshPrimitive, text: &str) -> Result<Vec3, String> {
    let values = parse_dimension_values(primitive, text)?;
    Ok(match primitive {
        MeshPrimitive::Cube => vec3(values[0], values[0], values[0]),
        MeshPrimitive::Plane => vec3(values[0], 1.0, values[1]),
        MeshPrimitive::Sphere => vec3(values[0] * 2.0, values[0] * 2.0, values[0] * 2.0),
        MeshPrimitive::Cylinder => vec3(values[0] * 2.0, values[1], values[0] * 2.0),
    })
}

fn adjust_placement_dimension(
    placement: &mut PrimitivePlacement,
    delta: f32,
) -> Result<(), String> {
    let mut values = parse_dimension_values(placement.primitive, &placement.dimensions)?;
    let mut tokens = placement
        .dimensions
        .split(|character: char| character == ',' || character.is_whitespace())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let index = placement.dimension_index.min(values.len() - 1);
    values[index] = (values[index] + delta).clamp(0.01, 1000.0);
    let adjusted = format!("{:.2}", values[index]);
    tokens[index] = adjusted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string();
    placement.dimensions = tokens.join(", ");
    placement.input_focused = true;
    placement.input_selected = false;
    Ok(())
}

fn select_placement_dimension(placement: &mut PrimitivePlacement, direction: i32) {
    let count = primitive_dimension_count(placement.primitive);
    if count > 1 {
        placement.dimension_index =
            (placement.dimension_index as i32 + direction).rem_euclid(count as i32) as usize;
    }
    placement.input_focused = true;
    placement.input_selected = false;
}

fn placement_dimension_label(placement: &PrimitivePlacement) -> &'static str {
    match (placement.primitive, placement.dimension_index) {
        (MeshPrimitive::Cube, _) => "edge length",
        (MeshPrimitive::Plane, 0) => "width",
        (MeshPrimitive::Plane, _) => "length",
        (MeshPrimitive::Sphere, _) => "radius",
        (MeshPrimitive::Cylinder, 0) => "radius",
        (MeshPrimitive::Cylinder, _) => "height",
    }
}

fn dimensions_from_click(primitive: MeshPrimitive, center: Vec3, point: Vec3) -> Vec3 {
    let diameter = (vec2(point.x - center.x, point.z - center.z).length() * 2.0).max(0.001);
    match primitive {
        MeshPrimitive::Cube | MeshPrimitive::Sphere | MeshPrimitive::Cylinder => {
            vec3(diameter, diameter, diameter)
        }
        MeshPrimitive::Plane => vec3(diameter, 1.0, diameter),
    }
}

fn mesh_ray_hit_excluding(
    origin: Vec3,
    direction: Vec3,
    mesh: &EditableMesh,
    excluded_faces: &HashSet<usize>,
) -> Option<usize> {
    mesh_ray_hit_point_filtered(origin, direction, mesh, None, Some(excluded_faces))
        .map(|(_, face, _)| face)
}

fn mesh_without_faces(mesh: &EditableMesh, excluded_faces: &HashSet<usize>) -> EditableMesh {
    let indices = mesh
        .indices
        .chunks_exact(3)
        .enumerate()
        .filter(|(face, _)| !excluded_faces.contains(face))
        .flat_map(|(_, triangle)| triangle.iter().copied())
        .collect();
    EditableMesh {
        vertices: mesh.vertices.clone(),
        indices,
    }
}

fn mesh_ray_hit_point(
    origin: Vec3,
    direction: Vec3,
    mesh: &EditableMesh,
    allowed_faces: Option<&[usize]>,
) -> Option<(f32, usize, Vec3)> {
    mesh_ray_hit_point_filtered(origin, direction, mesh, allowed_faces, None)
}

fn mesh_ray_hit_point_filtered(
    origin: Vec3,
    direction: Vec3,
    mesh: &EditableMesh,
    allowed_faces: Option<&[usize]>,
    excluded_faces: Option<&HashSet<usize>>,
) -> Option<(f32, usize, Vec3)> {
    let mut closest: Option<(f32, usize, Vec3)> = None;
    for face in 0..mesh.face_count() {
        if allowed_faces.is_some_and(|faces| !faces.contains(&face)) {
            continue;
        }
        if excluded_faces.is_some_and(|faces| faces.contains(&face)) {
            continue;
        }
        let start = face * 3;
        let [ia, ib, ic] = [
            mesh.indices[start] as usize,
            mesh.indices[start + 1] as usize,
            mesh.indices[start + 2] as usize,
        ];
        let [Some(a), Some(b), Some(c)] = [
            mesh.vertices.get(ia),
            mesh.vertices.get(ib),
            mesh.vertices.get(ic),
        ] else {
            continue;
        };
        let edge_1 = b.position - a.position;
        let edge_2 = c.position - a.position;
        let cross = direction.cross(edge_2);
        let determinant = edge_1.dot(cross);
        if determinant.abs() < 1e-6 {
            continue;
        }
        let inverse = 1.0 / determinant;
        let offset = origin - a.position;
        let u = offset.dot(cross) * inverse;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = offset.cross(edge_1);
        let v = direction.dot(q) * inverse;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let distance = edge_2.dot(q) * inverse;
        if distance > 0.0 && closest.map(|(best, _, _)| distance < best).unwrap_or(true) {
            closest = Some((distance, face, origin + direction * distance));
        }
    }
    closest
}

fn mouse_ray(camera: &Camera3D, mouse: Vec2) -> Option<(Vec3, Vec3)> {
    let x = mouse.x / screen_width() * 2.0 - 1.0;
    let y = 1.0 - mouse.y / screen_height() * 2.0;
    let inverse = camera.matrix().inverse();
    let near = inverse * vec4(x, y, -1.0, 1.0);
    let far = inverse * vec4(x, y, 1.0, 1.0);
    if near.w.abs() < 1e-6 || far.w.abs() < 1e-6 {
        return None;
    }
    let origin = vec3(near.x / near.w, near.y / near.w, near.z / near.w);
    let end = vec3(far.x / far.w, far.y / far.w, far.z / far.w);
    Some((origin, (end - origin).normalize_or_zero()))
}

fn project_to_screen(camera: &Camera3D, point: Vec3) -> Option<Vec2> {
    let projected = camera.matrix() * point.extend(1.0);
    if projected.w <= 1e-5 {
        return None;
    }
    let ndc = projected.truncate() / projected.w;
    Some(vec2(
        (ndc.x + 1.0) * 0.5 * screen_width(),
        (1.0 - ndc.y) * 0.5 * screen_height(),
    ))
}

fn vertex_has_available_face(
    mesh: &EditableMesh,
    vertex: usize,
    blocked_faces: &HashSet<usize>,
) -> bool {
    mesh.indices
        .chunks_exact(3)
        .enumerate()
        .any(|(face, triangle)| {
            !blocked_faces.contains(&face) && triangle.contains(&(vertex as u32))
        })
}

fn pick_vertex(
    camera: &Camera3D,
    mouse: Vec2,
    mesh: &EditableMesh,
    blocked_faces: &HashSet<usize>,
) -> Option<usize> {
    let mut closest: Option<(f32, usize)> = None;
    for (index, vertex) in mesh.vertices.iter().enumerate() {
        if !vertex_has_available_face(mesh, index, blocked_faces) {
            continue;
        }
        let Some(screen) = project_to_screen(camera, vertex.position) else {
            continue;
        };
        let distance = screen.distance(mouse);
        if distance < 18.0 && closest.map(|(best, _)| distance < best).unwrap_or(true) {
            closest = Some((distance, index));
        }
    }
    closest.map(|(_, index)| index)
}

fn point_segment_distance(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance(start);
    }
    let t = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * t)
}

fn project_screen_point_to_segment(point: Vec2, start: Vec2, end: Vec2) -> Option<(f32, f32)> {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= 1e-6 {
        return None;
    }
    let amount = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    Some((amount, point.distance(start + segment * amount)))
}

fn polar_track_delta(
    delta: Vec3,
    screen_right: Vec3,
    screen_up: Vec3,
    increment_degrees: f32,
) -> (Vec3, f32) {
    let horizontal = delta.dot(screen_right);
    let vertical = delta.dot(screen_up);
    let magnitude = (horizontal * horizontal + vertical * vertical).sqrt();
    if magnitude <= 1e-8 || increment_degrees <= 0.0 {
        return (delta, 0.0);
    }
    let increment = increment_degrees.to_radians();
    let angle = vertical.atan2(horizontal);
    let tracked_angle = (angle / increment).round() * increment;
    (
        screen_right * (tracked_angle.cos() * magnitude)
            + screen_up * (tracked_angle.sin() * magnitude),
        tracked_angle.to_degrees(),
    )
}

fn snap_transform_to_grid(pivot: Vec3, values: Vec3, axis: TransformAxis, spacing: f32) -> Vec3 {
    let target = pivot + values;
    let spacing = spacing.max(0.001);
    match axis {
        TransformAxis::All => snap_model_point(target, spacing),
        TransformAxis::X => vec3((target.x / spacing).round() * spacing, target.y, target.z),
        TransformAxis::Y => target,
        TransformAxis::Z => vec3(target.x, target.y, (target.z / spacing).round() * spacing),
    }
}

fn snap_transform_to_triangular_grid(
    pivot: Vec3,
    values: Vec3,
    axis: TransformAxis,
    spacing: f32,
    center: Vec3,
) -> Vec3 {
    let target = pivot + values;
    let snapped = snap_model_point_to_triangular_grid(target, center, spacing);
    match axis {
        TransformAxis::All => snapped,
        TransformAxis::X => vec3(snapped.x, target.y, target.z),
        TransformAxis::Y => target,
        TransformAxis::Z => vec3(target.x, target.y, snapped.z),
    }
}

fn object_snap_priority(kind: ObjectSnapKind) -> u8 {
    match kind {
        ObjectSnapKind::Endpoint => 0,
        ObjectSnapKind::Intersection => 1,
        ObjectSnapKind::Midpoint => 2,
        ObjectSnapKind::Center => 3,
        ObjectSnapKind::Quadrant => 4,
        ObjectSnapKind::Perpendicular => 5,
        ObjectSnapKind::Tangent => 6,
    }
}

fn unique_mesh_edges(mesh: &EditableMesh) -> Vec<(usize, usize)> {
    let mut edges = HashSet::new();
    for triangle in mesh.indices.chunks_exact(3) {
        for side in 0..3 {
            let a = triangle[side] as usize;
            let b = triangle[(side + 1) % 3] as usize;
            if a < mesh.vertices.len() && b < mesh.vertices.len() && a != b {
                edges.insert(if a < b { (a, b) } else { (b, a) });
            }
        }
    }
    let mut edges = edges.into_iter().collect::<Vec<_>>();
    edges.sort_unstable();
    edges
}

fn fit_circular_loop(points: &[Vec3]) -> Option<CircularLoop> {
    if points.len() < 6 {
        return None;
    }
    let center = points.iter().copied().sum::<Vec3>() / points.len() as f32;
    let first = (points[0] - center).normalize_or_zero();
    if first.length_squared() <= 1e-8 {
        return None;
    }
    let normal = points
        .iter()
        .skip(1)
        .map(|point| first.cross(*point - center))
        .find(|normal| normal.length_squared() > 1e-8)?
        .normalize();
    let u = (points[0] - center).normalize_or_zero();
    let v = normal.cross(u).normalize_or_zero();
    let radii = points
        .iter()
        .map(|point| {
            let relative = *point - center;
            if relative.dot(normal).abs() > 0.02 {
                return None;
            }
            Some(vec2(relative.dot(u), relative.dot(v)).length())
        })
        .collect::<Option<Vec<_>>>()?;
    let radius = radii.iter().sum::<f32>() / radii.len() as f32;
    if radius <= 1e-5
        || radii
            .iter()
            .any(|value| (*value - radius).abs() > (radius * 0.035).max(0.002))
    {
        return None;
    }
    Some(CircularLoop {
        center,
        normal,
        u,
        v,
        radius,
        vertices: points.to_vec(),
    })
}

fn circular_loop_snap_points(
    circle: &CircularLoop,
    cursor_on_plane: Option<Vec3>,
) -> Vec<ObjectSnapCandidate> {
    let mut candidates = vec![ObjectSnapCandidate {
        kind: ObjectSnapKind::Center,
        position: circle.center,
    }];
    for direction in [circle.u, -circle.u, circle.v, -circle.v] {
        candidates.push(ObjectSnapCandidate {
            kind: ObjectSnapKind::Quadrant,
            position: circle.center + direction * circle.radius,
        });
    }
    if let Some(cursor_point) = cursor_on_plane {
        let relative = cursor_point - circle.center;
        let distance_squared = relative.dot(circle.u).powi(2) + relative.dot(circle.v).powi(2);
        if distance_squared > circle.radius * circle.radius + 1e-6 {
            let base_angle = relative.dot(circle.v).atan2(relative.dot(circle.u));
            let offset = (circle.radius / distance_squared.sqrt()).acos();
            for angle in [base_angle + offset, base_angle - offset] {
                candidates.push(ObjectSnapCandidate {
                    kind: ObjectSnapKind::Tangent,
                    position: circle.center
                        + circle.radius * (circle.u * angle.cos() + circle.v * angle.sin()),
                });
            }
        }
    }
    candidates
}

fn circular_edge_loops(mesh: &EditableMesh) -> Vec<CircularLoop> {
    let mut edge_normals: HashMap<(usize, usize), Vec<Vec3>> = HashMap::new();
    for triangle in mesh.indices.chunks_exact(3) {
        let [Some(a_vertex), Some(b_vertex), Some(c_vertex)] = [
            mesh.vertices.get(triangle[0] as usize),
            mesh.vertices.get(triangle[1] as usize),
            mesh.vertices.get(triangle[2] as usize),
        ] else {
            continue;
        };
        let normal = (b_vertex.position - a_vertex.position)
            .cross(c_vertex.position - a_vertex.position)
            .normalize_or_zero();
        for side in 0..3 {
            let a = triangle[side] as usize;
            let b = triangle[(side + 1) % 3] as usize;
            if a < mesh.vertices.len() && b < mesh.vertices.len() && a != b {
                let edge = if a < b { (a, b) } else { (b, a) };
                edge_normals.entry(edge).or_default().push(normal);
            }
        }
    }
    let circular_feature_edges = edge_normals
        .into_iter()
        .filter_map(|(edge, normals)| {
            let is_loop_edge = match normals.as_slice() {
                [_] => true,
                [first, second] => first.dot(*second) <= 0.25,
                _ => false,
            };
            is_loop_edge.then_some(edge)
        })
        .collect::<Vec<_>>();
    let mut adjacency = vec![Vec::new(); mesh.vertices.len()];
    for &(a, b) in &circular_feature_edges {
        adjacency[a].push(b);
        adjacency[b].push(a);
    }
    let mut visited = HashSet::new();
    let mut loops = Vec::new();
    for &(start, next) in &circular_feature_edges {
        let edge = if start < next {
            (start, next)
        } else {
            (next, start)
        };
        if visited.contains(&edge) || adjacency[start].len() != 2 || adjacency[next].len() != 2 {
            continue;
        }
        let mut indices = vec![start];
        let mut previous = start;
        let mut current = next;
        visited.insert(edge);
        while current != start && indices.len() <= circular_feature_edges.len() {
            indices.push(current);
            let Some(&following) = adjacency[current]
                .iter()
                .find(|&&candidate| candidate != previous)
            else {
                break;
            };
            let edge = if current < following {
                (current, following)
            } else {
                (following, current)
            };
            if !visited.insert(edge) && following != start {
                break;
            }
            previous = current;
            current = following;
        }
        if current == start {
            let points = indices
                .iter()
                .filter_map(|&index| mesh.vertices.get(index).map(|vertex| vertex.position))
                .collect::<Vec<_>>();
            if let Some(circle) = fit_circular_loop(&points) {
                loops.push(circle);
            }
        }
    }
    loops
}

fn projected_segment_intersection(
    a0: Vec2,
    a1: Vec2,
    b0: Vec2,
    b1: Vec2,
) -> Option<(f32, f32, Vec2)> {
    let a = a1 - a0;
    let b = b1 - b0;
    let cross = a.perp_dot(b);
    if cross.abs() <= 1e-5 {
        return None;
    }
    let offset = b0 - a0;
    let t = offset.perp_dot(b) / cross;
    let u = offset.perp_dot(a) / cross;
    (0.01..=0.99)
        .contains(&t)
        .then_some(())
        .filter(|_| (0.01..=0.99).contains(&u))
        .map(|_| (t, u, a0 + a * t))
}

fn snap_candidate_is_visible(
    camera: &Camera3D,
    mesh: &EditableMesh,
    position: Vec3,
    screen: Vec2,
) -> bool {
    let Some((origin, direction)) = mouse_ray(camera, screen) else {
        return false;
    };
    let target_distance = (position - origin).dot(direction);
    if target_distance <= 0.0 {
        return false;
    }
    mesh_ray_hit_point(origin, direction, mesh, None)
        .is_none_or(|(hit_distance, _, _)| hit_distance + 0.035 >= target_distance)
}

fn nearest_object_snap(
    camera: &Camera3D,
    mouse: Vec2,
    source_mesh: &EditableMesh,
    display_mesh: &EditableMesh,
    state: &CadSceneState,
    settings: ObjectSnapSettings,
) -> Option<ObjectSnapCandidate> {
    let mut candidates = Vec::new();
    let selected_positions = selected_indices(source_mesh, state)
        .into_iter()
        .filter_map(|index| {
            source_mesh
                .vertices
                .get(index)
                .map(|vertex| model_to_display_point(source_mesh, vertex.position))
        })
        .collect::<Vec<_>>();
    let is_selected = |position: Vec3| {
        selected_positions
            .iter()
            .any(|selected| (*selected - position).length_squared() <= 1e-8)
    };
    let edges = unique_mesh_edges(display_mesh);
    let edge_screens = edges
        .iter()
        .filter_map(|&(a, b)| {
            let a_position = display_mesh.vertices.get(a)?.position;
            let b_position = display_mesh.vertices.get(b)?.position;
            let a_screen = project_to_screen(camera, a_position)?;
            let b_screen = project_to_screen(camera, b_position)?;
            let near_cursor = point_segment_distance(mouse, a_screen, b_screen) < 24.0;
            near_cursor.then_some((a, b, a_position, b_position, a_screen, b_screen))
        })
        .collect::<Vec<_>>();
    let mut consider = |kind: ObjectSnapKind, point: Vec3| {
        if !settings.enabled(kind) {
            return;
        }
        let Some(screen) = project_to_screen(camera, point) else {
            return;
        };
        let distance = mouse.distance(screen);
        if distance <= 14.0 && snap_candidate_is_visible(camera, display_mesh, point, screen) {
            candidates.push((distance, object_snap_priority(kind), kind, point));
        }
    };

    if settings.endpoint {
        for vertex in &display_mesh.vertices {
            if !is_selected(vertex.position) {
                consider(ObjectSnapKind::Endpoint, vertex.position);
            }
        }
    }
    if settings.midpoint {
        for &(_, _, a, b, _, _) in &edge_screens {
            if !is_selected(a) && !is_selected(b) {
                consider(ObjectSnapKind::Midpoint, (a + b) * 0.5);
            }
        }
    }
    if settings.intersection {
        for first in 0..edge_screens.len() {
            let (a_index, a_end, a0, a1, a0_screen, a1_screen) = edge_screens[first];
            for &(b_index, b_end, b0, b1, b0_screen, b1_screen) in &edge_screens[first + 1..] {
                if [a_index, a_end].contains(&b_index) || [a_index, a_end].contains(&b_end) {
                    continue;
                }
                if [a0, a1].iter().all(|point| is_selected(*point))
                    || [b0, b1].iter().all(|point| is_selected(*point))
                {
                    continue;
                }
                let Some((t, u, screen)) =
                    projected_segment_intersection(a0_screen, a1_screen, b0_screen, b1_screen)
                else {
                    continue;
                };
                let first_position = a0.lerp(a1, t);
                let second_position = b0.lerp(b1, u);
                if first_position.distance(second_position) <= 0.035 {
                    consider(
                        ObjectSnapKind::Intersection,
                        (first_position + second_position) * 0.5,
                    );
                } else {
                    let Some((origin, direction)) = mouse_ray(camera, screen) else {
                        continue;
                    };
                    let first_depth = (first_position - origin).dot(direction);
                    let second_depth = (second_position - origin).dot(direction);
                    if (first_depth - second_depth).abs() <= 0.035 {
                        consider(
                            ObjectSnapKind::Intersection,
                            (first_position + second_position) * 0.5,
                        );
                    }
                }
            }
        }
    }
    if settings.perpendicular {
        for &(_, _, a, b, a_screen, b_screen) in &edge_screens {
            if is_selected(a) && is_selected(b) {
                continue;
            }
            let segment = b_screen - a_screen;
            if segment.length_squared() <= 1e-6 {
                continue;
            }
            let t = ((mouse - a_screen).dot(segment) / segment.length_squared()).clamp(0.0, 1.0);
            if point_segment_distance(mouse, a_screen, b_screen) <= 14.0 {
                consider(ObjectSnapKind::Perpendicular, a.lerp(b, t));
            }
        }
    }

    let circles = circular_edge_loops(display_mesh);
    for circle in circles {
        if circle.vertices.iter().all(|point| is_selected(*point)) {
            continue;
        }
        let cursor_on_plane = mouse_ray(camera, mouse).and_then(|(origin, direction)| {
            ray_plane_hit(origin, direction, circle.center, circle.normal)
        });
        for candidate in circular_loop_snap_points(&circle, cursor_on_plane) {
            consider(candidate.kind, candidate.position);
        }
    }

    candidates.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    candidates
        .first()
        .map(|(_, _, kind, display_position)| ObjectSnapCandidate {
            kind: *kind,
            position: display_to_model_point(source_mesh, *display_position),
        })
}

fn nearest_trim_face_snap(
    camera: &Camera3D,
    mouse: Vec2,
    mesh: &EditableMesh,
    face: usize,
    settings: ObjectSnapSettings,
) -> Option<ObjectSnapCandidate> {
    let start = face.checked_mul(3)?;
    let indices = mesh.indices.get(start..start + 3)?;
    let points = indices
        .iter()
        .map(|&index| {
            mesh.vertices
                .get(index as usize)
                .map(|vertex| vertex.position)
        })
        .collect::<Option<Vec<_>>>()?;
    let mut candidates = Vec::new();
    let mut consider = |kind: ObjectSnapKind, position: Vec3| {
        if !settings.enabled(kind) {
            return;
        }
        let Some(screen) = project_to_screen(camera, position) else {
            return;
        };
        let distance = mouse.distance(screen);
        if distance <= 14.0 && snap_candidate_is_visible(camera, mesh, position, screen) {
            candidates.push((distance, object_snap_priority(kind), kind, position));
        }
    };

    if settings.endpoint {
        for &point in &points {
            consider(ObjectSnapKind::Endpoint, point);
        }
    }
    for edge in 0..3 {
        let start = points[edge];
        let end = points[(edge + 1) % 3];
        if settings.midpoint {
            consider(ObjectSnapKind::Midpoint, (start + end) * 0.5);
        }
        if settings.perpendicular
            && let (Some(start_screen), Some(end_screen)) = (
                project_to_screen(camera, start),
                project_to_screen(camera, end),
            )
            && let Some((amount, distance)) =
                project_screen_point_to_segment(mouse, start_screen, end_screen)
            && distance <= 14.0
        {
            consider(ObjectSnapKind::Perpendicular, start.lerp(end, amount));
        }
    }

    candidates.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    candidates
        .first()
        .map(|(_, _, kind, position)| ObjectSnapCandidate {
            kind: *kind,
            position: *position,
        })
}

fn trim_path_point_at_cursor(
    camera: &Camera3D,
    mouse: Vec2,
    mesh: &EditableMesh,
    target_faces: &[usize],
    path_face: Option<usize>,
    settings: ObjectSnapSettings,
) -> Option<(usize, Vec3)> {
    let eligible_faces = path_face
        .map(|face| vec![face])
        .unwrap_or_else(|| target_faces.to_vec());
    if eligible_faces.is_empty() {
        return None;
    }
    if let Some((_, face, point)) = mouse_ray(camera, mouse).and_then(|(origin, direction)| {
        mesh_ray_hit_point(origin, direction, mesh, Some(&eligible_faces))
    }) {
        let point = nearest_trim_face_snap(camera, mouse, mesh, face, settings)
            .map_or(point, |candidate| candidate.position);
        return Some((face, point));
    }
    eligible_faces
        .iter()
        .filter_map(|&face| {
            let candidate = nearest_trim_face_snap(camera, mouse, mesh, face, settings)?;
            let screen = project_to_screen(camera, candidate.position)?;
            Some((mouse.distance(screen), face, candidate.position))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, face, point)| (face, point))
}

fn snap_model_point_to_grid(
    point: Vec3,
    state: &CadSceneState,
    metadata: &FaceMetadata,
    mesh: &EditableMesh,
) -> Vec3 {
    let (center, triangular) = grid_center_for_mesh(mesh, metadata);
    snap_model_point_to_grid_layout(point, center, state.grid_spacing, triangular)
}

fn model_point_at_cursor(
    camera: &Camera3D,
    mouse: Vec2,
    source_mesh: &EditableMesh,
    display_mesh: &EditableMesh,
    state: &CadSceneState,
    metadata: &FaceMetadata,
) -> Option<Vec3> {
    if let Some(target) = nearest_object_snap(
        camera,
        mouse,
        source_mesh,
        display_mesh,
        state,
        state.object_snaps,
    ) {
        return Some(target.position);
    }
    let (origin, direction) = mouse_ray(camera, mouse)?;
    let display_point = mesh_ray_hit_point(origin, direction, display_mesh, None)
        .map(|(_, _, point)| point)
        .or_else(|| construction_plane_hit(origin, direction))?;
    let mut point = display_to_model_point(source_mesh, display_point);
    if state.grid_snap {
        point = snap_model_point_to_grid(point, state, metadata, source_mesh);
    }
    Some(point)
}

fn apply_trim(catalog: &mut ObjectCatalog, state: &mut CadSceneState) -> Result<(), String> {
    let target_faces = state.trim_target_faces.clone();
    let cutter_faces = state.trim_cutter_faces.clone();
    let mut affected_faces = target_faces.clone();
    affected_faces.extend(cutter_faces.iter().copied());
    ensure_faces_editable(
        selected_metadata(catalog, state)
            .ok_or_else(|| "Selected face metadata is unavailable".to_string())?,
        &affected_faces,
    )?;
    let multiline = state
        .trim_path_face
        .map(|face| (face, state.trim_path_points.clone()));
    if target_faces.is_empty() {
        return Err("Select at least one target face".to_string());
    }
    if cutter_faces.is_empty() && multiline.is_none() {
        return Err("Select cutter faces or draw a multiline path".to_string());
    }
    let snapshot = capture_snapshot(catalog, state)?;
    let face_map = selected_mesh_mut(catalog, state)
        .ok_or_else(|| "Selected object is unavailable".to_string())?
        .split_faces_by_intersections(
            &target_faces,
            &cutter_faces,
            multiline
                .as_ref()
                .map(|(face, points)| (*face, points.as_slice())),
        )?;
    let face_count = selected_mesh(catalog, state)
        .map(EditableMesh::face_count)
        .ok_or_else(|| "Selected object is unavailable".to_string())?;
    selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())?
        .remap_after_face_rewrite(&face_map, face_count);
    push_undo(state, snapshot);
    state.dirty = true;
    state.selected_faces = target_faces
        .iter()
        .flat_map(|&face| face_map[face].iter().copied())
        .collect();
    state.selected_group = None;
    state.selected_vertices.clear();
    state.selected_edges.clear();
    state.trim_active = false;
    state.trim_target_faces.clear();
    state.trim_cutter_faces.clear();
    state.trim_path_active = false;
    state.trim_path_points.clear();
    state.trim_path_face = None;
    state.error = None;
    Ok(())
}

fn clear_trim_state(state: &mut CadSceneState) {
    state.trim_active = false;
    state.trim_phase = TrimPhase::Targets;
    state.trim_target_faces.clear();
    state.trim_cutter_faces.clear();
    state.trim_path_active = false;
    state.trim_path_points.clear();
    state.trim_path_face = None;
}

fn finish_trim_path(state: &mut CadSceneState) -> Result<(), String> {
    if state.trim_path_points.len() < 2 || state.trim_path_face.is_none() {
        return Err("Draw a multiline path with at least two points".to_string());
    }
    state.trim_path_active = false;
    state.error = None;
    Ok(())
}

fn advance_trim(catalog: &mut ObjectCatalog, state: &mut CadSceneState) -> Result<(), String> {
    if state.trim_phase == TrimPhase::Targets {
        if state.trim_target_faces.is_empty() {
            return Err("Select at least one target face first".to_string());
        }
        state.trim_phase = TrimPhase::Cutters;
        state.error = None;
        return Ok(());
    }
    if state.trim_path_active {
        return finish_trim_path(state);
    }
    apply_trim(catalog, state)
}

fn copy_faces_with_operation(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
    mut operation: impl FnMut(&mut EditableMesh, &[usize]) -> Result<Vec<(usize, usize)>, String>,
) -> Result<(), String> {
    ensure_selection_unlocked(catalog, state)?;
    let snapshot = capture_snapshot(catalog, state)?;
    let copied = operation(
        selected_mesh_mut(catalog, state)
            .ok_or_else(|| "Selected object is unavailable".to_string())?,
        &state.selected_faces,
    )?;
    selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())?
        .inherit_copied_faces(&copied);
    push_undo(state, snapshot);
    state.dirty = true;
    state.selected_faces = copied.iter().map(|&(_, destination)| destination).collect();
    state.selected_edges.clear();
    state.selected_vertices.clear();
    state.selection_mode = SelectionMode::Face;
    state.selected_group = None;
    state.error = None;
    Ok(())
}

fn apply_mirror(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
    plane_point: Vec3,
    plane_normal: Vec3,
) -> Result<(), String> {
    copy_faces_with_operation(catalog, state, |mesh, faces| {
        mesh.copy_faces_reflected(faces, plane_point, plane_normal)
    })
}

fn parse_paint_hex(text: &str) -> Result<Color, String> {
    let text = text.trim().trim_start_matches('#');
    if text.len() != 6 && text.len() != 8 {
        return Err("Enter a 6-digit or 8-digit hex color".to_string());
    }
    let bytes = text
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).map_err(|_| "Invalid hex color".to_string())?;
            u8::from_str_radix(pair, 16).map_err(|_| "Enter a valid hex color".to_string())
        })
        .collect::<Result<Vec<_>, String>>()?;
    let alpha = if bytes.len() == 4 { bytes[3] } else { 255 };
    Ok(Color::from_rgba(bytes[0], bytes[1], bytes[2], alpha))
}

fn parse_operation_distance(text: &str, label: &str) -> Result<f32, String> {
    let value = text
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("Enter a numeric {label}"))?;
    if !value.is_finite() || value.abs() <= 1e-6 {
        return Err(format!("{label} must be finite and non-zero"));
    }
    Ok(value)
}

fn apply_offset(catalog: &mut ObjectCatalog, state: &mut CadSceneState) -> Result<(), String> {
    let distance = parse_operation_distance(&state.operation_value, "offset distance")?;
    copy_faces_with_operation(catalog, state, |mesh, faces| {
        mesh.copy_faces_offset(faces, distance)
    })
}

fn apply_paint_pick(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
    camera: &Camera3D,
    mouse: Vec2,
    mesh: &EditableMesh,
    blocked_faces: &HashSet<usize>,
) -> Result<(), String> {
    let vertex_indices = match state.selection_mode {
        SelectionMode::Vertex => {
            let index = pick_vertex(camera, mouse, mesh, blocked_faces)
                .ok_or_else(|| "Click a mesh vertex to paint".to_string())?;
            state.selected_vertices = vec![index];
            state.selected_edges.clear();
            state.selected_faces.clear();
            state.selected_group = None;
            vec![index]
        }
        SelectionMode::Edge => {
            let edge = pick_edge(camera, mouse, mesh, blocked_faces)
                .ok_or_else(|| "Click a mesh edge to paint".to_string())?;
            state.selected_edges = vec![edge];
            state.selected_vertices.clear();
            state.selected_faces.clear();
            state.selected_group = None;
            vec![edge.0, edge.1]
        }
        SelectionMode::Face => {
            let (origin, direction) = mouse_ray(camera, mouse)
                .ok_or_else(|| "Could not create a mouse ray".to_string())?;
            let face = mesh_ray_hit_excluding(origin, direction, mesh, blocked_faces)
                .ok_or_else(|| "Click a mesh face to paint".to_string())?;
            ensure_faces_editable(
                selected_metadata(catalog, state)
                    .ok_or_else(|| "Selected face metadata is unavailable".to_string())?,
                &[face],
            )?;
            state.selected_faces = vec![face];
            state.selected_vertices.clear();
            state.selected_edges.clear();
            state.selected_group = None;
            let start = face
                .checked_mul(3)
                .ok_or_else(|| "Invalid face index".to_string())?;
            let triangle = mesh
                .indices
                .get(start..start + 3)
                .ok_or_else(|| "Selected face is outside the mesh".to_string())?;
            triangle.iter().map(|&index| index as usize).collect()
        }
    };
    if vertex_indices.is_empty() {
        return Err("Selected paint target is empty".to_string());
    }
    let snapshot = capture_snapshot(catalog, state)?;
    let mesh = selected_mesh_mut(catalog, state)
        .ok_or_else(|| "Selected object is unavailable".to_string())?;
    let mut unique_indices = vertex_indices;
    unique_indices.sort_unstable();
    unique_indices.dedup();
    let result = mesh.paint_vertices(&unique_indices, state.paint_color);
    if result.is_err() {
        return result;
    }
    push_undo(state, snapshot);
    state.dirty = true;
    state.error = None;
    Ok(())
}

fn apply_fillet(catalog: &mut ObjectCatalog, state: &mut CadSceneState) -> Result<(), String> {
    ensure_selection_unlocked(catalog, state)?;
    let radius = parse_operation_distance(&state.operation_value, "fillet radius")?;
    let snapshot = capture_snapshot(catalog, state)?;
    let face_map = selected_mesh_mut(catalog, state)
        .ok_or_else(|| "Selected object is unavailable".to_string())?
        .fillet_edges(&state.selected_edges, radius, 4)?;
    let face_count = selected_mesh(catalog, state)
        .map(EditableMesh::face_count)
        .ok_or_else(|| "Selected object is unavailable".to_string())?;
    selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())?
        .remap_after_face_rewrite(&face_map, face_count);
    push_undo(state, snapshot);
    state.dirty = true;
    state.selected_faces = face_map.into_iter().flatten().collect();
    state.selected_edges.clear();
    state.selection_mode = SelectionMode::Face;
    state.error = None;
    Ok(())
}

fn pick_edge(
    camera: &Camera3D,
    mouse: Vec2,
    mesh: &EditableMesh,
    blocked_faces: &HashSet<usize>,
) -> Option<(usize, usize)> {
    let mut closest: Option<(f32, (usize, usize))> = None;
    let mut visited = HashSet::new();
    for (face, triangle) in mesh.indices.chunks_exact(3).enumerate() {
        if blocked_faces.contains(&face) {
            continue;
        }
        for side in 0..3 {
            let a = triangle[side] as usize;
            let b = triangle[(side + 1) % 3] as usize;
            let key = if a < b { (a, b) } else { (b, a) };
            if !visited.insert(key) {
                continue;
            }
            let [Some(a_vertex), Some(b_vertex)] =
                [mesh.vertices.get(key.0), mesh.vertices.get(key.1)]
            else {
                continue;
            };
            let [Some(a_screen), Some(b_screen)] = [
                project_to_screen(camera, a_vertex.position),
                project_to_screen(camera, b_vertex.position),
            ] else {
                continue;
            };
            let distance = point_segment_distance(mouse, a_screen, b_screen);
            if distance < 12.0 && closest.map(|(best, _)| distance < best).unwrap_or(true) {
                closest = Some((distance, key));
            }
        }
    }
    closest.map(|(_, edge)| edge)
}

fn draw_paint_panel(ctx: &FrameContext, panel: Rect, state: &mut CadSceneState) {
    if !state.editing {
        return;
    }
    let paint_panel = panel;
    draw_rectangle(
        paint_panel.x,
        paint_panel.y,
        paint_panel.w,
        paint_panel.h,
        ctx.colors_rt.panel_bg,
    );
    draw_rectangle_lines(
        paint_panel.x,
        paint_panel.y,
        paint_panel.w,
        paint_panel.h,
        1.5,
        ctx.colors_rt.panel_border,
    );
    draw_text(
        "Paint Color",
        paint_panel.x + 12.0,
        paint_panel.y + 20.0,
        ctx.font_sm,
        ctx.colors_rt.text_primary,
    );
    let swatches = [
        Color::from_rgba(205, 190, 160, 255),
        Color::from_rgba(220, 70, 70, 255),
        Color::from_rgba(220, 150, 60, 255),
        Color::from_rgba(210, 210, 70, 255),
        Color::from_rgba(70, 190, 120, 255),
        Color::from_rgba(70, 150, 220, 255),
        Color::from_rgba(150, 90, 210, 255),
        Color::from_rgba(220, 220, 220, 255),
    ];
    for (index, &color) in swatches.iter().enumerate() {
        let column = index % 4;
        let row = index / 4;
        let rect = Rect::new(
            paint_panel.x + 12.0 + column as f32 * 41.0,
            paint_panel.y + 34.0 + row as f32 * 34.0,
            32.0,
            26.0,
        );
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, color);
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            1.5,
            ctx.colors_rt.panel_border,
        );
        if is_mouse_button_pressed(MouseButton::Left) && rect.contains(ctx.mouse) {
            state.paint_active = true;
            state.paint_color = color;
            state.paint_hex = format!(
                "{:02X}{:02X}{:02X}",
                (color.r * 255.0).round() as u8,
                (color.g * 255.0).round() as u8,
                (color.b * 255.0).round() as u8
            );
            state.paint_hex_focused = false;
            state.paint_hex_selected = false;
        }
    }
    let input = Rect::new(paint_panel.x + 12.0, paint_panel.y + 104.0, 166.0, 28.0);
    draw_rectangle(
        input.x,
        input.y,
        input.w,
        input.h,
        ctx.colors_rt.button_base,
    );
    draw_rectangle_lines(
        input.x,
        input.y,
        input.w,
        input.h,
        1.5,
        ctx.colors_rt.panel_border,
    );
    draw_text(
        &format!("#{}", state.paint_hex),
        input.x + 8.0,
        input.y + 19.0,
        ctx.font_sm,
        ctx.colors_rt.text_primary,
    );
    if is_mouse_button_pressed(MouseButton::Left) && input.contains(ctx.mouse) {
        state.paint_active = true;
        state.paint_hex_focused = true;
        state.paint_hex_selected = true;
    }
}

fn draw_view_controls(
    ctx: &FrameContext,
    state: &CadSceneState,
    panel: Rect,
    has_radial_group: bool,
    radial_pivot: Option<[f32; 3]>,
) {
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, ctx.colors_rt.panel_bg);
    draw_rectangle_lines(
        panel.x,
        panel.y,
        panel.w,
        panel.h,
        1.0,
        ctx.colors_rt.panel_border,
    );
    let mut y = panel.y + 22.0;
    let mode = if state.editing {
        "Editing mesh"
    } else {
        "View mode"
    };
    draw_text(
        mode,
        panel.x + 12.0,
        y,
        ctx.font_md,
        ctx.colors_rt.text_primary,
    );
    y += 24.0;
    draw_text(
        "Right drag: orbit",
        panel.x + 12.0,
        y,
        ctx.font_sm,
        ctx.colors_rt.text_secondary,
    );
    y += 19.0;
    draw_text(
        "Middle drag: pan",
        panel.x + 12.0,
        y,
        ctx.font_sm,
        ctx.colors_rt.text_secondary,
    );
    y += 19.0;
    draw_text(
        "Wheel: zoom",
        panel.x + 12.0,
        y,
        ctx.font_sm,
        ctx.colors_rt.text_secondary,
    );
    if state.editing {
        y += 24.0;
        let selection = match state.selection_mode {
            SelectionMode::Vertex => "Vertex selection",
            SelectionMode::Edge => "Edge selection",
            SelectionMode::Face => "Face selection",
        };
        draw_text(
            selection,
            panel.x + 12.0,
            y,
            ctx.font_sm,
            ctx.colors_rt.button_hover,
        );
        y += 21.0;
        let selected = match state.selection_mode {
            SelectionMode::Vertex => state.selected_vertices.len(),
            SelectionMode::Edge => state.selected_edges.len(),
            SelectionMode::Face => state.selected_faces.len(),
        };
        draw_text(
            &format!("Selected: {selected}"),
            panel.x + 12.0,
            y,
            ctx.font_sm,
            ctx.colors_rt.text_secondary,
        );
        let snap_rect = object_snap_rect(panel);
        let spacing_minus = grid_spacing_minus_rect(panel);
        let spacing_plus = grid_spacing_plus_rect(panel);
        let preview_rect = interaction_preview_rect(panel);
        let _ = button(ctx, snap_rect, "Object Snaps...");
        let _ = button(
            ctx,
            spacing_minus,
            &format!("Grid: {:.2} (-)", state.grid_spacing),
        );
        let _ = button(ctx, spacing_plus, "+");
        let _ = button(
            ctx,
            preview_rect,
            if state.interaction_preview {
                "Mesh View"
            } else {
                "Interaction Preview"
            },
        );
        if state.interaction_preview {
            let _ = button(ctx, preview_shuffle_rect(panel), "Shuffle neighbors");
            let neighbors = state
                .preview_neighbors
                .iter()
                .map(|state| match state {
                    RadialNeighborState::Empty => "Empty",
                    RadialNeighborState::Route => "Route",
                    RadialNeighborState::OtherBuilding => "Building",
                })
                .collect::<Vec<_>>()
                .join(", ");
            draw_text(
                &neighbors,
                panel.x + 12.0,
                panel.y + panel.h - 7.0,
                ctx.font_sm,
                ctx.colors_rt.text_secondary,
            );
        }
        if has_radial_group {
            let label = if state.placing_radial_center {
                "Cancel UCS placement"
            } else if radial_pivot.is_some() {
                "Move UCS origin"
            } else {
                "Place UCS origin"
            };
            let _ = button(ctx, radial_center_rect(panel), label);
        }
    }
}

fn object_snap_rect(panel: Rect) -> Rect {
    Rect::new(panel.x + 12.0, panel.y + 166.0, panel.w - 24.0, 27.0)
}

fn grid_spacing_minus_rect(panel: Rect) -> Rect {
    Rect::new(panel.x + 12.0, panel.y + 196.0, panel.w - 56.0, 27.0)
}

fn grid_spacing_plus_rect(panel: Rect) -> Rect {
    Rect::new(panel.x + panel.w - 38.0, panel.y + 196.0, 26.0, 27.0)
}

fn interaction_preview_rect(panel: Rect) -> Rect {
    Rect::new(panel.x + 12.0, panel.y + 226.0, panel.w - 24.0, 27.0)
}

fn preview_shuffle_rect(panel: Rect) -> Rect {
    Rect::new(panel.x + 12.0, panel.y + 256.0, panel.w - 24.0, 27.0)
}

fn radial_center_rect(panel: Rect) -> Rect {
    Rect::new(panel.x + 12.0, panel.y + 286.0, panel.w - 24.0, 27.0)
}

fn object_snap_panel_rect(panel: Rect) -> Rect {
    Rect::new(
        (panel.x - 232.0).max(SIDEBAR_WIDTH + 8.0),
        panel.y + 165.0,
        220.0,
        218.0,
    )
}

fn object_snap_tracking_button_rect(panel: Rect, index: usize) -> Rect {
    let popup = object_snap_panel_rect(panel);
    Rect::new(
        popup.x + 8.0 + index as f32 * 104.0,
        popup.y + 183.0,
        98.0,
        27.0,
    )
}

fn object_snap_button_rect(panel: Rect, index: usize) -> Rect {
    let popup = object_snap_panel_rect(panel);
    let column = index % 2;
    let row = index / 2;
    Rect::new(
        popup.x + 8.0 + column as f32 * 104.0,
        popup.y + 28.0 + row as f32 * 31.0,
        98.0,
        27.0,
    )
}

fn placement_panel_rect(viewport: Rect) -> Rect {
    Rect::new(viewport.x + 12.0, viewport.y + 12.0, 330.0, 150.0)
}

fn placement_input_rect(viewport: Rect) -> Rect {
    let panel = placement_panel_rect(viewport);
    Rect::new(panel.x + 12.0, panel.y + 86.0, panel.w - 24.0, 26.0)
}

fn placement_create_rect(viewport: Rect) -> Rect {
    let panel = placement_panel_rect(viewport);
    Rect::new(panel.x + 12.0, panel.y + 117.0, 145.0, 26.0)
}

fn placement_cancel_rect(viewport: Rect) -> Rect {
    let panel = placement_panel_rect(viewport);
    Rect::new(panel.x + 173.0, panel.y + 117.0, 145.0, 26.0)
}

fn button(ctx: &FrameContext, rect: Rect, label: &str) -> bool {
    ui_button(rect, label, ctx.mouse, ctx.font_sm, ctx.button_colors).0
}

fn list_row_clicked(rect: Rect, mouse: Vec2) -> bool {
    rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Left)
}

fn draw_list_row(
    ctx: &FrameContext,
    rect: Rect,
    label: &str,
    selected: bool,
    selected_color: Color,
) {
    let hovered = rect.contains(ctx.mouse);
    let fill = if selected {
        selected_color
    } else if hovered {
        ctx.colors_rt.button_hover
    } else {
        ctx.colors_rt.button_base
    };
    let border = if selected {
        selected_color
    } else {
        ctx.colors_rt.button_border
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, fill);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, border);
    draw_text(
        label,
        rect.x + 10.0,
        rect.y + rect.h * 0.5 + ctx.font_sm * 0.35,
        ctx.font_sm,
        WHITE,
    );
}

fn draw_group_action_button(ctx: &FrameContext, rect: Rect, label: &str, active: bool) {
    let hovered = rect.contains(ctx.mouse);
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        if active {
            Color::from_rgba(27, 119, 105, 255)
        } else if hovered {
            ctx.colors_rt.button_hover
        } else {
            ctx.colors_rt.button_base
        },
    );
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        1.0,
        ctx.colors_rt.button_border,
    );
    let text_width = measure_text(label, None, ctx.font_sm as u16, 1.0).width;
    draw_text(
        label,
        rect.x + (rect.w - text_width) * 0.5,
        rect.y + rect.h * 0.5 + ctx.font_sm * 0.35,
        ctx.font_sm,
        WHITE,
    );
}

fn selected_entry<'a>(
    catalog: &'a ObjectCatalog,
    id: ObjectId,
) -> Option<&'a crate::core::object_assets::CatalogEntry> {
    catalog
        .entries()
        .iter()
        .find(|entry| entry.definition.id == id)
}

fn selected_mesh<'a>(
    catalog: &'a ObjectCatalog,
    state: &CadSceneState,
) -> Option<&'a EditableMesh> {
    catalog.get(state.selected)
}

fn selected_mesh_mut<'a>(
    catalog: &'a mut ObjectCatalog,
    state: &CadSceneState,
) -> Option<&'a mut EditableMesh> {
    catalog.get_mut(state.selected)
}

fn selected_metadata<'a>(
    catalog: &'a ObjectCatalog,
    state: &CadSceneState,
) -> Option<&'a FaceMetadata> {
    catalog.metadata(state.selected, None)
}

fn selected_metadata_mut<'a>(
    catalog: &'a mut ObjectCatalog,
    state: &CadSceneState,
) -> Option<&'a mut FaceMetadata> {
    catalog.metadata_mut(state.selected, None)
}

fn capture_snapshot(
    catalog: &ObjectCatalog,
    state: &CadSceneState,
) -> Result<EditSnapshot, String> {
    Ok(EditSnapshot {
        mesh: selected_mesh(catalog, state)
            .cloned()
            .ok_or_else(|| "Selected object is unavailable".to_string())?,
        metadata: selected_metadata(catalog, state)
            .cloned()
            .ok_or_else(|| "Selected face metadata is unavailable".to_string())?,
        selected_faces: state.selected_faces.clone(),
        selected_group: state.selected_group,
        selected_vertices: state.selected_vertices.clone(),
        selected_edges: state.selected_edges.clone(),
        selection_mode: state.selection_mode,
    })
}

fn restore_snapshot(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
    snapshot: EditSnapshot,
) -> Result<(), String> {
    *selected_mesh_mut(catalog, state)
        .ok_or_else(|| "Selected object is unavailable".to_string())? = snapshot.mesh;
    *selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())? = snapshot.metadata;
    state.selected_faces = snapshot.selected_faces;
    state.selected_group = snapshot.selected_group;
    state.selected_vertices = snapshot.selected_vertices;
    state.selected_edges = snapshot.selected_edges;
    state.selection_mode = snapshot.selection_mode;
    Ok(())
}

fn push_undo(state: &mut CadSceneState, snapshot: EditSnapshot) {
    if state.undo_stack.len() == HISTORY_LIMIT {
        state.undo_stack.remove(0);
    }
    state.undo_stack.push(snapshot);
    state.redo_stack.clear();
}

fn undo(catalog: &mut ObjectCatalog, state: &mut CadSceneState) -> Result<bool, String> {
    let Some(previous) = state.undo_stack.pop() else {
        return Ok(false);
    };
    let current = match capture_snapshot(catalog, state) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            state.undo_stack.push(previous);
            return Err(error);
        }
    };
    state.redo_stack.push(current);
    restore_snapshot(catalog, state, previous)?;
    state.dirty = true;
    Ok(true)
}

fn redo(catalog: &mut ObjectCatalog, state: &mut CadSceneState) -> Result<bool, String> {
    let Some(next) = state.redo_stack.pop() else {
        return Ok(false);
    };
    let current = match capture_snapshot(catalog, state) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            state.redo_stack.push(next);
            return Err(error);
        }
    };
    state.undo_stack.push(current);
    restore_snapshot(catalog, state, next)?;
    state.dirty = true;
    Ok(true)
}

fn save_selected_mesh(catalog: &mut ObjectCatalog, state: &CadSceneState) -> Result<(), String> {
    let mesh = selected_mesh(catalog, state)
        .cloned()
        .ok_or_else(|| "Selected object is unavailable".to_string())?;
    catalog.save(state.selected, mesh)
}

fn restore_original_mesh(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
) -> Result<(), String> {
    if selected_mesh(catalog, state).is_none() {
        return Err("Selected object is unavailable".to_string());
    }
    if selected_metadata(catalog, state).is_none() {
        return Err("Selected face metadata is unavailable".to_string());
    }
    let original = state
        .original_mesh
        .take()
        .ok_or_else(|| "Original mesh is unavailable".to_string())?;
    let original_metadata = state
        .original_metadata
        .take()
        .ok_or_else(|| "Original face metadata is unavailable".to_string())?;
    *selected_mesh_mut(catalog, state)
        .ok_or_else(|| "Selected object is unavailable".to_string())? = original;
    *selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())? = original_metadata;
    close_editor(state);
    Ok(())
}

fn close_editor(state: &mut CadSceneState) {
    state.editing = false;
    state.dirty = false;
    state.paint_active = false;
    state.paint_hex_focused = false;
    state.paint_hex_selected = false;
    state.original_mesh = None;
    state.original_metadata = None;
    state.selection_mode = SelectionMode::Vertex;
    state.selected_group = None;
    state.selected_faces.clear();
    state.selected_vertices.clear();
    state.selected_edges.clear();
    state.dragged_faces = None;
    state.rename_dialog = None;
    state.gizmo_drag = None;
    state.gizmo_preview = None;
    state.gizmo_snap_target = None;
    state.gizmo_polar_angle = None;
    state.numeric_transform_open = false;
    state.numeric_transform_focused = false;
    state.numeric_transform_selected = false;
    state.group_config_dialog = None;
    state.trim_active = false;
    state.trim_phase = TrimPhase::Targets;
    state.trim_target_faces.clear();
    state.trim_cutter_faces.clear();
    state.trim_path_active = false;
    state.trim_path_points.clear();
    state.trim_path_face = None;
    state.mirror_active = false;
    state.mirror_points.clear();
    state.fillet_active = false;
    state.offset_active = false;
    state.operation_value_focused = false;
    state.operation_value_selected = false;
    state.primitive_placement = None;
    state.interaction_preview = false;
    state.placing_radial_center = false;
    state.face_scroll = 0;
    state.undo_stack.clear();
    state.redo_stack.clear();
}

fn switch_to_object(state: &mut CadSceneState, id: ObjectId) {
    state.selected = id;
    state.editing = false;
    state.dirty = false;
    state.original_mesh = None;
    state.original_metadata = None;
    state.selection_mode = SelectionMode::Vertex;
    state.selected_faces.clear();
    state.selected_group = None;
    state.selected_vertices.clear();
    state.selected_edges.clear();
    state.dragged_faces = None;
    state.rename_dialog = None;
    state.gizmo_drag = None;
    state.gizmo_preview = None;
    state.gizmo_snap_target = None;
    state.gizmo_polar_angle = None;
    state.numeric_transform_open = false;
    state.numeric_transform_focused = false;
    state.numeric_transform_selected = false;
    state.group_config_dialog = None;
    state.trim_active = false;
    state.trim_phase = TrimPhase::Targets;
    state.trim_target_faces.clear();
    state.trim_cutter_faces.clear();
    state.trim_path_active = false;
    state.trim_path_points.clear();
    state.trim_path_face = None;
    state.mirror_active = false;
    state.mirror_points.clear();
    state.fillet_active = false;
    state.offset_active = false;
    state.operation_value_focused = false;
    state.operation_value_selected = false;
    state.primitive_placement = None;
    state.interaction_preview = false;
    state.placing_radial_center = false;
    state.undo_stack.clear();
    state.redo_stack.clear();
    state.face_scroll = 0;
    state.error = None;
    state.target = Vec3::ZERO;
    state.distance = 4.0;
}

fn selected_indices(mesh: &EditableMesh, state: &CadSceneState) -> Vec<usize> {
    match state.selection_mode {
        SelectionMode::Vertex => {
            let positions = state
                .selected_vertices
                .iter()
                .filter_map(|&index| mesh.vertices.get(index).map(|vertex| vertex.position))
                .collect::<Vec<_>>();
            mesh.vertices
                .iter()
                .enumerate()
                .filter_map(|(index, vertex)| {
                    positions
                        .iter()
                        .any(|position| (*position - vertex.position).length_squared() <= 1e-8)
                        .then_some(index)
                })
                .collect()
        }
        SelectionMode::Edge => {
            let positions = state
                .selected_edges
                .iter()
                .flat_map(|&(a, b)| [a, b])
                .filter_map(|index| mesh.vertices.get(index).map(|vertex| vertex.position))
                .collect::<Vec<_>>();
            mesh.vertices
                .iter()
                .enumerate()
                .filter_map(|(index, vertex)| {
                    positions
                        .iter()
                        .any(|position| (*position - vertex.position).length_squared() <= 1e-8)
                        .then_some(index)
                })
                .collect()
        }
        SelectionMode::Face => {
            let mut indices = state
                .selected_faces
                .iter()
                .filter_map(|&face| {
                    let start = face.checked_mul(3)?;
                    mesh.indices.get(start..start + 3)
                })
                .flatten()
                .map(|&index| index as usize)
                .collect::<Vec<_>>();
            indices.sort_unstable();
            indices.dedup();
            indices
        }
    }
}

fn selected_bounds_center(mesh: &EditableMesh, indices: &[usize]) -> Result<Vec3, String> {
    let mut min = vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut found = false;
    for &index in indices {
        let vertex = mesh
            .vertices
            .get(index)
            .ok_or_else(|| format!("Selected vertex {index} is out of range"))?;
        min = min.min(vertex.position);
        max = max.max(vertex.position);
        found = true;
    }
    if !found {
        return Err("Select vertices, edges, or faces before transforming".to_string());
    }
    Ok((min + max) * 0.5)
}

fn numeric_transform_command(
    catalog: &ObjectCatalog,
    state: &CadSceneState,
) -> Result<TransformCommand, String> {
    let mesh = selected_mesh(catalog, state)
        .ok_or_else(|| "Selected object is unavailable".to_string())?;
    let indices = selected_indices(mesh, state);
    let pivot = selected_bounds_center(mesh, &indices)?;
    let values = constrained_transform_values(
        state.transform_mode,
        TransformAxis::All,
        parse_transform_values(state.transform_mode, &state.numeric_transform_values)?,
    );
    Ok(TransformCommand {
        mode: state.transform_mode,
        pivot,
        values,
    })
}

fn apply_transform_to_selection(
    mesh: &mut EditableMesh,
    state: &CadSceneState,
    command: TransformCommand,
) -> Result<(), String> {
    let indices = selected_indices(mesh, state);
    apply_transform_to_indices(mesh, &indices, command)
}

fn commit_transform_command(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
    command: TransformCommand,
) -> Result<(), String> {
    ensure_selection_unlocked(catalog, state)?;
    if !transform_has_effect(command) {
        state.gizmo_preview = None;
        state.gizmo_drag = None;
        state.gizmo_snap_target = None;
        state.gizmo_polar_angle = None;
        return Ok(());
    }
    let snapshot = capture_snapshot(catalog, state)?;
    let indices = selected_indices(&snapshot.mesh, state);
    let mesh = selected_mesh_mut(catalog, state)
        .ok_or_else(|| "Selected object is unavailable".to_string())?;
    apply_transform_to_indices(mesh, &indices, command)?;
    push_undo(state, snapshot);
    state.dirty = true;
    state.gizmo_preview = None;
    state.gizmo_drag = None;
    state.gizmo_snap_target = None;
    state.gizmo_polar_angle = None;
    Ok(())
}

fn apply_numeric_transform(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
) -> Result<(), String> {
    let command = numeric_transform_command(catalog, state)?;
    commit_transform_command(catalog, state, command)?;
    state.numeric_transform_open = false;
    state.numeric_transform_focused = false;
    state.numeric_transform_selected = false;
    Ok(())
}

fn sidebar_items(metadata: &FaceMetadata, face_count: usize) -> Vec<SidebarItem> {
    fn append_group(
        metadata: &FaceMetadata,
        group_id: u64,
        face_count: usize,
        grouped_faces: &mut HashSet<usize>,
        visited: &mut HashSet<u64>,
        items: &mut Vec<SidebarItem>,
    ) {
        if !visited.insert(group_id) {
            return;
        }
        let Some(group) = metadata.group(group_id) else {
            return;
        };
        items.push(SidebarItem::Group(group_id));
        for &face in &group.faces {
            if face < face_count {
                grouped_faces.insert(face);
                items.push(SidebarItem::Face {
                    index: face,
                    group: Some(group_id),
                });
            }
        }
        for child in metadata
            .groups
            .iter()
            .filter(|child| child.parent_id == Some(group_id))
        {
            append_group(
                metadata,
                child.id,
                face_count,
                grouped_faces,
                visited,
                items,
            );
        }
    }

    let mut items = Vec::new();
    let mut grouped_faces = HashSet::new();
    let mut visited = HashSet::new();
    for group in metadata
        .groups
        .iter()
        .filter(|group| group.parent_id.is_none())
    {
        append_group(
            metadata,
            group.id,
            face_count,
            &mut grouped_faces,
            &mut visited,
            &mut items,
        );
    }
    for face in 0..face_count {
        if !grouped_faces.contains(&face) {
            items.push(SidebarItem::Face {
                index: face,
                group: None,
            });
        }
    }
    items
}

fn group_depth(metadata: &FaceMetadata, group_id: u64) -> usize {
    let mut depth = 0;
    let mut current = metadata.group(group_id).and_then(|group| group.parent_id);
    let mut visited = HashSet::new();
    while let Some(parent_id) = current {
        if !visited.insert(parent_id) {
            break;
        }
        depth += 1;
        current = metadata.group(parent_id).and_then(|group| group.parent_id);
    }
    depth
}

fn select_face(state: &mut CadSceneState, face: usize, extend: bool) {
    state.selection_mode = SelectionMode::Face;
    state.selected_group = None;
    state.selected_vertices.clear();
    state.selected_edges.clear();
    if extend {
        if let Some(position) = state
            .selected_faces
            .iter()
            .position(|&selected| selected == face)
        {
            state.selected_faces.remove(position);
        } else {
            state.selected_faces.push(face);
            state.selected_faces.sort_unstable();
        }
    } else {
        state.selected_faces = vec![face];
    }
}

fn toggle_index(selection: &mut Vec<usize>, index: usize) {
    if let Some(position) = selection.iter().position(|&selected| selected == index) {
        selection.remove(position);
    } else {
        selection.push(index);
        selection.sort_unstable();
    }
}

fn draw_face_outline(mesh: &EditableMesh, face: usize, color: Color) {
    let start = face * 3;
    let Some(indices) = mesh.indices.get(start..start + 3) else {
        return;
    };
    let Some(points) = indices
        .iter()
        .map(|&index| {
            mesh.vertices
                .get(index as usize)
                .map(|vertex| vertex.position)
        })
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    for edge in 0..3 {
        draw_line_3d(points[edge], points[(edge + 1) % 3], color);
    }
}

fn select_edge(state: &mut CadSceneState, edge: (usize, usize), extend: bool) {
    state.selection_mode = SelectionMode::Edge;
    state.selected_group = None;
    state.selected_faces.clear();
    state.selected_vertices.clear();
    if extend {
        if let Some(position) = state
            .selected_edges
            .iter()
            .position(|&selected| selected == edge)
        {
            state.selected_edges.remove(position);
        } else {
            state.selected_edges.push(edge);
        }
    } else {
        state.selected_edges = vec![edge];
    }
}

fn select_group(
    state: &mut CadSceneState,
    metadata: &FaceMetadata,
    group_id: u64,
) -> Result<(), String> {
    let faces = metadata
        .faces_for_group(group_id)
        .ok_or_else(|| format!("face group {group_id} does not exist"))?;
    state.selection_mode = SelectionMode::Face;
    state.selected_group = Some(group_id);
    state.selected_faces = faces;
    state.selected_vertices.clear();
    state.selected_edges.clear();
    Ok(())
}

fn ensure_faces_editable(metadata: &FaceMetadata, faces: &[usize]) -> Result<(), String> {
    let locked_faces = metadata.locked_faces();
    if let Some(face) = faces.iter().find(|face| locked_faces.contains(face)) {
        return Err(format!("Unlock the group before modifying face {face}"));
    }
    let hidden_faces = metadata.hidden_faces();
    if let Some(face) = faces.iter().find(|face| hidden_faces.contains(face)) {
        return Err(format!("Show the group before modifying face {face}"));
    }
    Ok(())
}

fn ensure_selection_unlocked(catalog: &ObjectCatalog, state: &CadSceneState) -> Result<(), String> {
    let mesh = selected_mesh(catalog, state)
        .ok_or_else(|| "Selected object is unavailable".to_string())?;
    let metadata = selected_metadata(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())?;
    if let Some(group_id) = state.selected_group {
        if metadata.group_is_locked(group_id) {
            return Err("Unlock the group before modifying its geometry".to_string());
        }
        if metadata.group_is_hidden(group_id) {
            return Err("Show the group before modifying its geometry".to_string());
        }
    }
    ensure_faces_editable(metadata, &state.selected_faces)?;
    let locked_faces = metadata.locked_faces();
    let mut blocked_faces = metadata.hidden_faces();
    blocked_faces.extend(locked_faces.iter().copied());
    let selected_vertices = match state.selection_mode {
        SelectionMode::Vertex => state.selected_vertices.clone(),
        SelectionMode::Edge => state
            .selected_edges
            .iter()
            .flat_map(|&(a, b)| [a, b])
            .collect(),
        SelectionMode::Face => Vec::new(),
    };
    for face in blocked_faces {
        let start = face * 3;
        if mesh.indices.get(start..start + 3).is_some_and(|triangle| {
            triangle
                .iter()
                .any(|index| selected_vertices.contains(&(*index as usize)))
        }) {
            return Err("Show or unlock the group before modifying its geometry".to_string());
        }
    }
    Ok(())
}

fn toggle_group_flag(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
    group_id: u64,
    flag: GroupFlag,
) -> Result<(), String> {
    let metadata = selected_metadata(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())?;
    let group = metadata
        .group(group_id)
        .ok_or_else(|| format!("face group {group_id} does not exist"))?;
    let (inherited, current) = match flag {
        GroupFlag::Locked => (metadata.group_has_locked_ancestor(group_id), group.locked),
        GroupFlag::Hidden => (metadata.group_has_hidden_ancestor(group_id), group.hidden),
    };
    if inherited {
        return Err("Change the parent group state first".to_string());
    }

    let snapshot = capture_snapshot(catalog, state)?;
    let metadata = selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())?;
    match flag {
        GroupFlag::Locked => metadata.set_group_locked(group_id, !current)?,
        GroupFlag::Hidden => metadata.set_group_hidden(group_id, !current)?,
    }
    push_undo(state, snapshot);
    state.dirty = true;
    state.selected_faces.clear();
    state.selected_vertices.clear();
    state.selected_edges.clear();
    state.gizmo_drag = None;
    state.gizmo_preview = None;
    state.gizmo_snap_target = None;
    state.gizmo_polar_angle = None;
    clear_trim_state(state);
    state.mirror_active = false;
    state.mirror_points.clear();
    state.fillet_active = false;
    state.offset_active = false;
    state.operation_value_focused = false;
    state.error = None;
    Ok(())
}

fn create_selected_group(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
) -> Result<(), String> {
    ensure_selection_unlocked(catalog, state)?;
    let snapshot = capture_snapshot(catalog, state)?;
    let selected_faces = state.selected_faces.clone();
    let parent_id = state.selected_group;
    let group_id = selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())?
        .create_group_under(&selected_faces, parent_id)?;
    let group_faces = selected_metadata(catalog, state)
        .and_then(|metadata| metadata.group(group_id))
        .map(|group| group.faces.clone())
        .ok_or_else(|| format!("new face group {group_id} is unavailable"))?;
    state.selected_group = Some(group_id);
    state.selected_faces = group_faces;
    state.selection_mode = SelectionMode::Face;
    push_undo(state, snapshot);
    state.dirty = true;
    Ok(())
}

fn add_primitive(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
    primitive: MeshPrimitive,
    center: Vec3,
    dimensions: Vec3,
) -> Result<(), String> {
    let snapshot = capture_snapshot(catalog, state)?;
    let shape = EditableMesh::primitive(primitive).transformed(|position| {
        center
            + vec3(
                position.x * dimensions.x,
                position.y * dimensions.y,
                position.z * dimensions.z,
            )
    });
    let first_face = snapshot.mesh.face_count();
    let face_count = shape.face_count();
    let mut mesh = snapshot.mesh.clone();
    mesh.append(&shape)?;
    let mut metadata = snapshot.metadata.clone();
    metadata.face_names.resize(first_face + face_count, None);
    let parent_id = state
        .selected_group
        .or_else(|| metadata.radial_main_group_id());
    if let Some(group_id) = parent_id {
        if metadata.group_is_locked(group_id) {
            return Err("Unlock the group before adding geometry".to_string());
        }
        if metadata.group_is_hidden(group_id) {
            return Err("Show the group before adding geometry".to_string());
        }
    }
    let faces = (first_face..first_face + face_count).collect::<Vec<_>>();
    let group_id = metadata.create_group_under(&faces, parent_id)?;
    metadata.rename_group(group_id, primitive_group_name(primitive).to_string())?;
    *selected_mesh_mut(catalog, state)
        .ok_or_else(|| "Selected object is unavailable".to_string())? = mesh;
    *selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())? = metadata;
    state.selection_mode = SelectionMode::Face;
    state.selected_faces = faces;
    state.selected_vertices.clear();
    state.selected_edges.clear();
    state.selected_group = None;
    push_undo(state, snapshot);
    state.dirty = true;
    state.error = None;
    Ok(())
}

fn primitive_group_name(primitive: MeshPrimitive) -> &'static str {
    match primitive {
        MeshPrimitive::Cube => "Cube",
        MeshPrimitive::Plane => "Plane",
        MeshPrimitive::Sphere => "Sphere",
        MeshPrimitive::Cylinder => "Cylinder",
    }
}

fn start_primitive_placement(state: &mut CadSceneState, primitive: MeshPrimitive) {
    state.paint_active = false;
    state.paint_hex_focused = false;
    state.paint_hex_selected = false;
    state.primitive_placement = Some(PrimitivePlacement {
        primitive,
        center: None,
        dimensions: primitive_default_dimensions(primitive).to_string(),
        input_focused: false,
        input_selected: false,
        dimension_index: 0,
    });
    state.error = None;
}

fn confirm_primitive_placement(catalog: &mut ObjectCatalog, state: &mut CadSceneState) {
    let Some(placement) = state.primitive_placement.clone() else {
        return;
    };
    let Some(center) = placement.center else {
        state.error = Some("Click the grid to place the primitive center first".to_string());
        return;
    };
    let dimensions = match parse_primitive_dimensions(placement.primitive, &placement.dimensions) {
        Ok(dimensions) => dimensions,
        Err(error) => {
            state.error = Some(error);
            return;
        }
    };
    match add_primitive(catalog, state, placement.primitive, center, dimensions) {
        Ok(()) => state.primitive_placement = None,
        Err(error) => state.error = Some(error),
    }
}

fn shuffle_preview_neighbors(state: &mut CadSceneState, catalog: &ObjectCatalog) {
    state.preview_seed = state.preview_seed.wrapping_add(1).max(1);
    state.preview_neighbors = selected_metadata(catalog, state)
        .map(|metadata| random_preview_neighbors(metadata, state.preview_seed))
        .unwrap_or_else(|| random_preview_neighbors(&FaceMetadata::default(), state.preview_seed));
}

fn random_preview_neighbors(metadata: &FaceMetadata, seed: u64) -> [RadialNeighborState; 6] {
    let mut choices = metadata
        .groups
        .iter()
        .filter_map(|group| match group.program {
            FaceGroupProgram::IfNeighbor(condition) => Some(condition),
            _ => None,
        })
        .collect::<Vec<_>>();
    if choices.is_empty() {
        choices.extend([
            RadialNeighborState::Empty,
            RadialNeighborState::Route,
            RadialNeighborState::OtherBuilding,
        ]);
    }
    let mut random = seed.max(1);
    std::array::from_fn(|_| {
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        choices[(random as usize) % choices.len()]
    })
}

fn rotate_mesh_y_about(mesh: &EditableMesh, angle: f32, pivot: Vec3) -> EditableMesh {
    let (sin, cos) = angle.sin_cos();
    mesh.transformed(|point| {
        let relative = point - pivot;
        vec3(
            relative.x * cos - relative.z * sin,
            relative.y,
            relative.x * sin + relative.z * cos,
        ) + pivot
    })
}

fn interaction_preview_mesh(
    catalog: &ObjectCatalog,
    state: &CadSceneState,
) -> Result<EditableMesh, String> {
    let base_mesh = selected_mesh(catalog, state)
        .ok_or_else(|| "Selected object is unavailable".to_string())?;
    let Some(metadata) = selected_metadata(catalog, state) else {
        return Ok(base_mesh.clone());
    };
    let pivot = metadata
        .radial_pivot
        .map(|[x, y, z]| vec3(x, y, z))
        .unwrap_or(Vec3::ZERO);
    compose_conditioned_preview(base_mesh, metadata, &state.preview_neighbors, pivot)
}

fn compose_conditioned_preview(
    base_mesh: &EditableMesh,
    metadata: &FaceMetadata,
    neighbors: &[RadialNeighborState; 6],
    pivot: Vec3,
) -> Result<EditableMesh, String> {
    if metadata.radial_main_group_id().is_none() {
        return Ok(base_mesh.clone());
    }
    let mut composite = EditableMesh {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    for (sector, condition) in neighbors.iter().copied().enumerate() {
        let sector_mesh = metadata
            .radial_sector_mesh(base_mesh, condition)?
            .ok_or_else(|| "Configured radial group could not be previewed".to_string())?;
        composite.append(&rotate_mesh_y_about(
            &sector_mesh,
            std::f32::consts::TAU * sector as f32 / 6.0,
            pivot,
        ))?;
    }
    Ok(composite)
}

fn route_neighbor_preview_mesh(
    route_mesh: &EditableMesh,
    metadata: &FaceMetadata,
    neighbor_index: usize,
) -> Result<Option<EditableMesh>, String> {
    let Some(sector_mesh) = metadata.radial_sector_mesh(route_mesh, RadialNeighborState::Route)?
    else {
        return Ok(None);
    };
    let pivot = metadata
        .radial_pivot
        .map(|[x, y, z]| vec3(x, y, z))
        .unwrap_or(Vec3::ZERO);
    let sector = (neighbor_index + 3) % 6;
    Ok(Some(rotate_mesh_y_about(
        &sector_mesh,
        std::f32::consts::TAU * sector as f32 / 6.0,
        pivot,
    )))
}

fn preview_neighbor_object(
    catalog: &ObjectCatalog,
    selected: ObjectId,
    seed: u64,
    neighbor_index: usize,
) -> Option<ObjectId> {
    let candidates = catalog
        .entries()
        .iter()
        .filter(|entry| {
            entry.definition.category == ObjectCategory::Buildings
                && entry.definition.id != selected
                && entry.definition.id != ObjectId::PlanetRoute
                && entry.mesh.is_some()
        })
        .map(|entry| entry.definition.id)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    let index = seed.wrapping_add((neighbor_index as u64).wrapping_mul(0x9E37_79B9)) as usize
        % candidates.len();
    Some(candidates[index])
}

fn hex_cell_center(index: usize, radius: f32, origin: Vec3) -> Vec3 {
    let angle = std::f32::consts::TAU * index as f32 / 6.0 + std::f32::consts::FRAC_PI_6;
    origin
        + vec3(
            angle.cos() * radius * 3.0_f32.sqrt(),
            0.0,
            angle.sin() * radius * 3.0_f32.sqrt(),
        )
}

fn draw_hex_outline(center: Vec3, radius: f32, color: Color) {
    for corner in 0..6 {
        let angle_a = std::f32::consts::TAU * corner as f32 / 6.0;
        let angle_b = std::f32::consts::TAU * (corner + 1) as f32 / 6.0;
        let a = center + vec3(angle_a.cos() * radius, 0.0, angle_a.sin() * radius);
        let b = center + vec3(angle_b.cos() * radius, 0.0, angle_b.sin() * radius);
        draw_line_3d(a, b, color);
    }
}

fn draw_preview_object(
    mesh: &EditableMesh,
    reference: &EditableMesh,
    anchor: Option<Vec3>,
    center: Vec3,
    scale: f32,
) -> Result<(), String> {
    let normalized = preview_mesh_with_reference_at(mesh, reference, anchor);
    let min_y = normalized
        .vertices
        .iter()
        .map(|vertex| vertex.position.y)
        .reduce(f32::min)
        .unwrap_or(0.0);
    normalized
        .transformed(|position| position * scale + center + vec3(0.0, -min_y * scale + 0.02, 0.0))
        .render()
}

fn draw_triangular_grid(center: Vec3, model_scale: f32, spacing: f32) {
    let color = Color::from_rgba(150, 175, 190, 75);
    for (start, end) in triangular_grid_segments(center, spacing * model_scale, 4.0) {
        draw_line_3d(start, end, color);
    }
}

fn triangular_grid_segments(center: Vec3, spacing: f32, side_length: f32) -> Vec<(Vec3, Vec3)> {
    let spacing = spacing.max(0.001);
    let steps = (side_length.max(spacing) / spacing).ceil().clamp(1.0, 60.0) as usize;
    let x_axis = Vec3::X;
    let z_axis = vec3(0.5, 0.0, 3.0_f32.sqrt() * 0.5);
    let mut segments = Vec::with_capacity(steps * 3);

    for step in 0..=steps {
        let distance = step as f32 * spacing;
        let remaining = (steps - step) as f32 * spacing;
        let x_offset = x_axis * distance;
        let z_offset = z_axis * distance;
        let diagonal_start = center + x_offset;
        let diagonal_end = center + z_offset;

        if remaining > 0.0 {
            segments.push((center + x_offset, center + x_offset + z_axis * remaining));
            segments.push((center + z_offset, center + z_offset + x_axis * remaining));
        }
        if step > 0 {
            segments.push((diagonal_start, diagonal_end));
        }
    }

    segments
}

fn draw_ucs_icon(origin: Vec3, length: f32) {
    draw_line_3d(
        origin,
        origin + Vec3::X * length,
        Color::from_rgba(240, 65, 65, 255),
    );
    draw_line_3d(
        origin,
        origin + Vec3::Y * length,
        Color::from_rgba(80, 225, 100, 255),
    );
    draw_line_3d(
        origin,
        origin + vec3(0.5, 0.0, 3.0_f32.sqrt() * 0.5) * length,
        Color::from_rgba(80, 145, 255, 255),
    );
    draw_sphere(origin, 0.035, None, WHITE);
}

fn preview_camera(state: &CadSceneState) -> Camera3D {
    let mut camera = object_camera(state);
    let direction = (camera.position - camera.target).normalize_or_zero();
    let distance = (state.distance * 1.8).max(5.5);
    camera.position = camera.target + direction * distance;
    camera
}

fn draw_interaction_preview(catalog: &ObjectCatalog, state: &CadSceneState) -> Result<(), String> {
    let camera = preview_camera(state);
    set_camera(&camera);
    let result = (|| {
        let origin = state.target;
        const CELL_RADIUS: f32 = 0.9;
        draw_hex_outline(origin, CELL_RADIUS, Color::from_rgba(255, 225, 90, 255));
        for index in 0..6 {
            let center = hex_cell_center(index, CELL_RADIUS, origin);
            draw_hex_outline(center, CELL_RADIUS, Color::from_rgba(115, 145, 165, 255));
        }

        let center_mesh = interaction_preview_mesh(catalog, state)?;
        let reference_mesh = selected_mesh(catalog, state)
            .ok_or_else(|| "Selected object is unavailable".to_string())?;
        let metadata = selected_metadata(catalog, state);
        let radial_anchor = metadata
            .filter(|metadata| metadata.radial_main_group_id().is_some())
            .map(|metadata| {
                metadata
                    .radial_pivot
                    .map(|[x, y, z]| vec3(x, y, z))
                    .unwrap_or(Vec3::ZERO)
            });
        draw_preview_object(&center_mesh, reference_mesh, radial_anchor, origin, 0.28)?;
        for (index, condition) in state.preview_neighbors.iter().copied().enumerate() {
            let object_id = match condition {
                RadialNeighborState::Empty => None,
                RadialNeighborState::Route => Some(ObjectId::PlanetRoute),
                RadialNeighborState::OtherBuilding => {
                    preview_neighbor_object(catalog, state.selected, state.preview_seed, index)
                }
            };
            let Some(mesh) = object_id.and_then(|id| catalog.get(id)) else {
                continue;
            };
            if condition == RadialNeighborState::Route {
                if let Some(metadata) = catalog.metadata(ObjectId::PlanetRoute, None)
                    && metadata.radial_main_group_id().is_some()
                {
                    let Some(neighbor_mesh) = route_neighbor_preview_mesh(mesh, metadata, index)?
                    else {
                        continue;
                    };
                    if neighbor_mesh.vertices.is_empty() {
                        continue;
                    }
                    let pivot = metadata
                        .radial_pivot
                        .map(|[x, y, z]| vec3(x, y, z))
                        .unwrap_or(Vec3::ZERO);
                    draw_preview_object(
                        &neighbor_mesh,
                        mesh,
                        Some(pivot),
                        hex_cell_center(index, CELL_RADIUS, origin),
                        0.24,
                    )?;
                    continue;
                }
            }
            draw_preview_object(
                mesh,
                mesh,
                None,
                hex_cell_center(index, CELL_RADIUS, origin),
                0.24,
            )?;
        }
        Ok(())
    })();
    set_default_camera();
    result
}

fn draw_square_grid(model_center: Vec3, scale: f32, spacing: f32) {
    let spacing = spacing.max(0.001);
    let half_count = (4.0 / (spacing * scale)).ceil().clamp(1.0, 60.0) as i32;
    let extent = half_count as f32 * spacing;
    let color = Color::from_rgba(150, 175, 190, 75);
    for line in -half_count..=half_count {
        let coordinate = line as f32 * spacing;
        let x = (coordinate - model_center.x) * scale;
        let z = (coordinate - model_center.z) * scale;
        let min_x = (-extent - model_center.x) * scale;
        let max_x = (extent - model_center.x) * scale;
        let min_z = (-extent - model_center.z) * scale;
        let max_z = (extent - model_center.z) * scale;
        draw_line_3d(vec3(x, 0.0, min_z), vec3(x, 0.0, max_z), color);
        draw_line_3d(vec3(min_x, 0.0, z), vec3(max_x, 0.0, z), color);
    }
}

fn assign_dragged_faces(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
    faces: &[usize],
    group_id: u64,
) -> Result<(), String> {
    ensure_faces_editable(
        selected_metadata(catalog, state)
            .ok_or_else(|| "Selected face metadata is unavailable".to_string())?,
        faces,
    )?;
    if let Some(metadata) = selected_metadata(catalog, state) {
        if metadata.group_is_locked(group_id) {
            return Err("Unlock the group before moving faces into it".to_string());
        }
        if metadata.group_is_hidden(group_id) {
            return Err("Show the group before moving faces into it".to_string());
        }
    }
    let snapshot = capture_snapshot(catalog, state)?;
    selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())?
        .assign_faces_to_group(faces, group_id)?;
    select_group(
        state,
        selected_metadata(catalog, state)
            .ok_or_else(|| "Selected face metadata is unavailable".to_string())?,
        group_id,
    )?;
    push_undo(state, snapshot);
    state.dirty = true;
    Ok(())
}

fn delete_selected(catalog: &mut ObjectCatalog, state: &mut CadSceneState) -> Result<(), String> {
    ensure_selection_unlocked(catalog, state)?;
    let snapshot = capture_snapshot(catalog, state)?;
    let mut mesh = snapshot.mesh.clone();
    let mut metadata = snapshot.metadata.clone();
    let faces = match state.selected_group {
        Some(group_id) => metadata.remove_group(group_id)?,
        None => state.selected_faces.clone(),
    };
    if faces.is_empty() && state.selected_group.is_none() {
        return Err("select one or more faces or a group to delete".to_string());
    }
    let face_remap = if faces.is_empty() {
        (0..mesh.face_count()).map(Some).collect()
    } else {
        mesh.remove_faces(&faces)?
    };
    metadata.remap_after_face_removal(&face_remap);
    *selected_mesh_mut(catalog, state)
        .ok_or_else(|| "Selected object is unavailable".to_string())? = mesh;
    *selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())? = metadata;
    state.selected_faces.clear();
    state.selected_group = None;
    state.selected_vertices.clear();
    push_undo(state, snapshot);
    state.dirty = true;
    Ok(())
}

fn set_radial_center(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
    center: Vec3,
) -> Result<(), String> {
    let metadata = selected_metadata(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())?;
    if metadata.radial_main_group_id().is_none() {
        return Err("Create a Radial Repeat group before setting its center".to_string());
    }
    let center = [center.x, center.y, center.z];
    if metadata.radial_pivot == Some(center) {
        return Ok(());
    }
    let snapshot = capture_snapshot(catalog, state)?;
    selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())?
        .radial_pivot = Some(center);
    push_undo(state, snapshot);
    state.dirty = true;
    Ok(())
}

fn rename_selected(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
    target: RenameTarget,
    name: String,
) -> Result<(), String> {
    let snapshot = capture_snapshot(catalog, state)?;
    let metadata = selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())?;
    match target {
        RenameTarget::Face(face) => metadata.rename_face(face, name)?,
        RenameTarget::Group(group) => metadata.rename_group(group, name)?,
    }
    push_undo(state, snapshot);
    state.dirty = true;
    Ok(())
}

fn apply_rename_dialog(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
) -> Result<(), String> {
    let dialog = state
        .rename_dialog
        .clone()
        .ok_or_else(|| "No rename is in progress".to_string())?;
    rename_selected(catalog, state, dialog.target, dialog.text)?;
    state.rename_dialog = None;
    Ok(())
}

fn apply_group_config_dialog(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
) -> Result<(), String> {
    let dialog = state
        .group_config_dialog
        .ok_or_else(|| "No group configuration is open".to_string())?;
    let snapshot = capture_snapshot(catalog, state)?;
    selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())?
        .set_group_program(dialog.group_id, dialog.program)?;
    push_undo(state, snapshot);
    state.dirty = true;
    state.group_config_dialog = None;
    Ok(())
}

fn draw_group_config_dialog(
    ctx: &FrameContext,
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
) {
    let Some(mut dialog) = state.group_config_dialog else {
        return;
    };
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::from_rgba(0, 0, 0, 190),
    );
    let modal = Rect::new(
        (screen_width() - 460.0) * 0.5,
        (screen_height() - 330.0) * 0.5,
        460.0,
        330.0,
    );
    draw_rectangle(modal.x, modal.y, modal.w, modal.h, ctx.colors_rt.panel_bg);
    draw_rectangle_lines(
        modal.x,
        modal.y,
        modal.w,
        modal.h,
        1.5,
        ctx.colors_rt.panel_border,
    );
    let group_name = selected_metadata(catalog, state)
        .and_then(|metadata| metadata.group(dialog.group_id))
        .map(|group| group.name.as_str())
        .unwrap_or("Group");
    draw_text(
        &format!("Configure group: {group_name}"),
        modal.x + 20.0,
        modal.y + 38.0,
        ctx.font_md,
        ctx.colors_rt.text_primary,
    );
    draw_text(
        "Program",
        modal.x + 20.0,
        modal.y + 76.0,
        ctx.font_sm,
        ctx.colors_rt.text_secondary,
    );

    for (index, (program, label)) in [
        (FaceGroupProgram::Geometry, "Plain geometry"),
        (FaceGroupProgram::RadialRepeat, "Radial Repeat"),
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect::new(
            modal.x + 20.0 + index as f32 * 205.0,
            modal.y + 88.0,
            195.0,
            36.0,
        );
        if button(ctx, rect, label) {
            dialog.program = program;
            state.error = None;
        }
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            2.0,
            if dialog.program == program {
                Color::from_rgba(27, 119, 105, 255)
            } else {
                ctx.colors_rt.button_border
            },
        );
    }
    draw_text(
        "If Neighbor",
        modal.x + 20.0,
        modal.y + 160.0,
        ctx.font_sm,
        ctx.colors_rt.text_secondary,
    );
    for (index, (condition, label)) in [
        (RadialNeighborState::Empty, "Empty"),
        (RadialNeighborState::Route, "Same building"),
        (RadialNeighborState::OtherBuilding, "Other building"),
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect::new(
            modal.x + 20.0 + index as f32 * 140.0,
            modal.y + 172.0,
            132.0,
            36.0,
        );
        let program = FaceGroupProgram::IfNeighbor(condition);
        if button(ctx, rect, label) {
            dialog.program = program;
            state.error = None;
        }
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            2.0,
            if dialog.program == program {
                Color::from_rgba(27, 119, 105, 255)
            } else {
                ctx.colors_rt.button_border
            },
        );
    }
    state.group_config_dialog = Some(dialog);
    let apply = button(
        ctx,
        Rect::new(modal.x + 240.0, modal.y + 265.0, 98.0, 36.0),
        "Apply",
    );
    let cancel = button(
        ctx,
        Rect::new(modal.x + 350.0, modal.y + 265.0, 90.0, 36.0),
        "Cancel",
    );
    if apply {
        match apply_group_config_dialog(catalog, state) {
            Ok(()) => state.error = None,
            Err(error) => state.error = Some(error),
        }
    } else if cancel {
        state.group_config_dialog = None;
        state.error = None;
    }
    if let Some(error) = &state.error {
        draw_text(
            error,
            modal.x + 20.0,
            modal.y + modal.h - 12.0,
            ctx.font_sm,
            Color::from_rgba(255, 110, 110, 255),
        );
    }
}

fn numeric_transform_panel_rect() -> Rect {
    Rect::new(SIDEBAR_WIDTH + 12.0, TOOLBAR_HEIGHT + 48.0, 260.0, 108.0)
}

fn operation_value_panel_rect(viewport: Rect) -> Rect {
    Rect::new(viewport.x + 12.0, viewport.y + 112.0, 310.0, 86.0)
}

fn operation_value_input_rect(viewport: Rect) -> Rect {
    let panel = operation_value_panel_rect(viewport);
    Rect::new(panel.x + 10.0, panel.y + 32.0, panel.w - 20.0, 28.0)
}

fn draw_numeric_transform_panel(
    ctx: &FrameContext,
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
) {
    if !state.numeric_transform_open {
        return;
    }
    let panel = numeric_transform_panel_rect();
    draw_rectangle(panel.x, panel.y, panel.w, panel.h, ctx.colors_rt.panel_bg);
    draw_rectangle_lines(
        panel.x,
        panel.y,
        panel.w,
        panel.h,
        1.0,
        ctx.colors_rt.panel_border,
    );
    let mode_label = match state.transform_mode {
        TransformMode::Move => "Move by X, Y, Z",
        TransformMode::Rotate => "Rotate by degrees X, Y, Z",
        TransformMode::Scale => "Scale by X, Y, Z",
    };
    draw_text(
        mode_label,
        panel.x + 10.0,
        panel.y + 21.0,
        ctx.font_sm,
        ctx.colors_rt.text_primary,
    );
    let input = Rect::new(panel.x + 10.0, panel.y + 28.0, panel.w - 20.0, 28.0);
    draw_rectangle(
        input.x,
        input.y,
        input.w,
        input.h,
        ctx.colors_rt.button_base,
    );
    draw_rectangle_lines(
        input.x,
        input.y,
        input.w,
        input.h,
        1.0,
        if state.numeric_transform_focused {
            Color::from_rgba(27, 119, 105, 255)
        } else {
            ctx.colors_rt.button_border
        },
    );
    if state.numeric_transform_selected {
        draw_rectangle(
            input.x + 6.0,
            input.y + 5.0,
            (state.numeric_transform_values.chars().count() as f32 * 9.0).min(input.w - 12.0),
            input.h - 10.0,
            Color::from_rgba(27, 119, 105, 90),
        );
    }
    draw_text(
        &state.numeric_transform_values,
        input.x + 9.0,
        input.y + 20.0,
        ctx.font_sm,
        ctx.colors_rt.text_primary,
    );
    if input.contains(ctx.mouse) && is_mouse_button_pressed(MouseButton::Left) {
        state.numeric_transform_focused = true;
        state.numeric_transform_selected = true;
        state.error = None;
    }
    let button_y = panel.y + 66.0;
    let apply_clicked = button(
        ctx,
        Rect::new(panel.x + 10.0, button_y, 110.0, 30.0),
        "Apply",
    );
    let close_clicked = button(
        ctx,
        Rect::new(panel.x + 140.0, button_y, 110.0, 30.0),
        "Close",
    );
    if apply_clicked {
        match apply_numeric_transform(catalog, state) {
            Ok(()) => {
                state.error = None;
            }
            Err(error) => state.error = Some(error),
        }
    } else if close_clicked {
        state.numeric_transform_open = false;
        state.numeric_transform_focused = false;
        state.numeric_transform_selected = false;
        state.error = None;
    }
}

fn draw_object_snap_panel(ctx: &FrameContext, panel: Rect, state: &mut CadSceneState) {
    if !state.snap_settings_open {
        return;
    }
    let popup = object_snap_panel_rect(panel);
    draw_rectangle(popup.x, popup.y, popup.w, popup.h, ctx.colors_rt.panel_bg);
    draw_rectangle_lines(
        popup.x,
        popup.y,
        popup.w,
        popup.h,
        1.0,
        ctx.colors_rt.panel_border,
    );
    draw_text(
        "Object snap modes",
        popup.x + 8.0,
        popup.y + 19.0,
        ctx.font_sm,
        ctx.colors_rt.text_primary,
    );
    for (index, (kind, label)) in [
        (ObjectSnapKind::Endpoint, "End"),
        (ObjectSnapKind::Midpoint, "Mid"),
        (ObjectSnapKind::Center, "Center"),
        (ObjectSnapKind::Intersection, "Intersect"),
        (ObjectSnapKind::Quadrant, "Quad"),
        (ObjectSnapKind::Perpendicular, "Perp"),
        (ObjectSnapKind::Tangent, "Tangent"),
    ]
    .into_iter()
    .enumerate()
    {
        let rect = object_snap_button_rect(panel, index);
        let enabled = state.object_snaps.enabled(kind);
        if button(ctx, rect, label) {
            state.object_snaps.set(kind, !enabled);
        }
        if enabled {
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                2.0,
                Color::from_rgba(27, 119, 105, 255),
            );
        }
    }
    for (index, label, enabled) in [
        (0, "Polar", state.polar_tracking),
        (1, "Grid Snap", state.grid_snap),
    ] {
        let rect = object_snap_tracking_button_rect(panel, index);
        if button(ctx, rect, label) {
            if index == 0 {
                state.polar_tracking = !state.polar_tracking;
            } else {
                state.grid_snap = !state.grid_snap;
            }
        }
        if enabled {
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                2.0,
                Color::from_rgba(27, 119, 105, 255),
            );
        }
    }
}

fn undo_or_redo(catalog: &mut ObjectCatalog, state: &mut CadSceneState, is_undo: bool) {
    let result = if is_undo {
        undo(catalog, state)
    } else {
        redo(catalog, state)
    };
    match result {
        Ok(true) => state.error = None,
        Ok(false) => {}
        Err(error) => state.error = Some(error),
    }
}

fn status_bar_message(state: &CadSceneState, asset_error: Option<&str>) -> (String, bool) {
    if let Some(error) = &state.error {
        return (error.clone(), true);
    }
    if let Some(error) = asset_error {
        return (error.to_string(), true);
    }
    let message = if state.placing_radial_center {
        "Click the grid to place the UCS origin; Esc cancels."
    } else if let Some(placement) = &state.primitive_placement {
        if placement.center.is_some() {
            "Primitive placement: click the viewport to set its size; use Create to finish."
        } else {
            "Click the grid to place the primitive center; Esc cancels."
        }
    } else if state.trim_active && state.trim_phase == TrimPhase::Targets {
        "Trim 1/2: select target faces; Shift toggles; Enter for cutters."
    } else if state.trim_active && state.trim_path_active {
        "Trim path: click the target face; edge and object snaps are active; Enter finishes."
    } else if state.trim_active && !state.trim_path_points.is_empty() {
        "Trim 2/2: path ready; Enter to split or Draw Path to replace it."
    } else if state.trim_active {
        "Trim 2/2: select cutter faces or Draw Path; Enter to split."
    } else if state.mirror_active {
        "Mirror: click three plane points; Backspace removes the last; Enter applies."
    } else if state.fillet_active {
        "Fillet: select mesh edges, then set the radius and apply."
    } else if state.offset_active {
        "Offset: select faces, then set the distance and apply."
    } else if state.numeric_transform_open {
        "Enter X, Y, Z values; Enter applies the transform; Esc closes."
    } else if state.gizmo_drag.is_some() {
        "Drag to transform; hold Grid Snap to align the move."
    } else if state.editing {
        "Ready."
    } else {
        "Select an object to edit."
    };
    (message.to_string(), false)
}

fn draw_status_bar(ctx: &FrameContext, catalog: &ObjectCatalog, state: &CadSceneState) {
    let height = STATUS_BAR_HEIGHT.min(screen_height());
    let y = screen_height() - height;
    draw_rectangle(0.0, y, screen_width(), height, ctx.colors_rt.panel_bg);
    draw_rectangle_lines(
        0.0,
        y,
        screen_width(),
        height,
        1.0,
        ctx.colors_rt.panel_border,
    );

    let asset_error = selected_entry(catalog, state.selected).and_then(|entry| {
        entry
            .metadata_error
            .as_deref()
            .or_else(|| {
                entry
                    .states
                    .iter()
                    .find_map(|state| state.metadata_error.as_deref())
            })
            .or_else(|| entry.states.iter().find_map(|state| state.error.as_deref()))
            .or(entry.variant_config_error.as_deref())
            .or(entry.states_error.as_deref())
            .or(entry.error.as_deref())
    });
    let (message, is_error) = status_bar_message(state, asset_error);
    let baseline = y + height * 0.68;
    let font_size = ctx.font_sm as u16;
    let object_label = selected_entry(catalog, state.selected)
        .map(|entry| entry.definition.label)
        .unwrap_or("Object");
    let details = if state.editing {
        format!(
            "{} | Grid Snap: {}{}",
            object_label,
            if state.grid_snap { "On" } else { "Off" },
            if state.dirty { " | Unsaved edits" } else { "" }
        )
    } else {
        object_label.to_string()
    };
    let details = fit_status_text(&details, screen_width() - 24.0, font_size);
    let details_width = measure_text(&details, None, font_size, 1.0).width;
    let message = fit_status_text(&message, screen_width() - details_width - 36.0, font_size);
    draw_text(
        &message,
        12.0,
        baseline,
        ctx.font_sm,
        if is_error {
            Color::from_rgba(255, 110, 110, 255)
        } else {
            ctx.colors_rt.text_primary
        },
    );
    draw_text(
        &details,
        (screen_width() - details_width - 12.0).max(12.0),
        baseline,
        ctx.font_sm,
        if state.dirty {
            Color::from_rgba(255, 220, 100, 255)
        } else {
            ctx.colors_rt.text_secondary
        },
    );
}

pub fn run(
    ctx: &FrameContext,
    state: &mut CadSceneState,
    catalog: &mut ObjectCatalog,
    scene: &mut Scene,
) {
    let content_height = (screen_height() - STATUS_BAR_HEIGHT).max(0.0);
    let sidebar = Rect::new(0.0, 0.0, SIDEBAR_WIDTH, content_height);
    let toolbar = Rect::new(
        SIDEBAR_WIDTH,
        0.0,
        screen_width() - SIDEBAR_WIDTH,
        TOOLBAR_HEIGHT,
    );
    let viewport = Rect::new(
        SIDEBAR_WIDTH,
        TOOLBAR_HEIGHT,
        screen_width() - SIDEBAR_WIDTH,
        (content_height - TOOLBAR_HEIGHT).max(0.0),
    );
    let view_panel = Rect::new(
        screen_width() - 205.0,
        18.0,
        190.0,
        if state.editing { 390.0 } else { 155.0 },
    );
    let paint_panel = Rect::new(
        view_panel.x,
        view_panel.y + view_panel.h + 8.0,
        view_panel.w,
        142.0,
    );
    if is_key_pressed(KeyCode::Escape) {
        if state.exit_confirmation_open {
            state.exit_confirmation_open = false;
        } else if state.group_config_dialog.is_some() {
            state.group_config_dialog = None;
        } else if state.rename_dialog.is_some() {
            state.rename_dialog = None;
        } else if state.numeric_transform_open {
            state.numeric_transform_open = false;
            state.numeric_transform_focused = false;
            state.numeric_transform_selected = false;
            state.error = None;
        } else if state.gizmo_drag.is_some() {
            state.gizmo_drag = None;
            state.gizmo_preview = None;
            state.gizmo_snap_target = None;
            state.gizmo_polar_angle = None;
            state.error = None;
        } else if state.primitive_placement.is_some() {
            state.primitive_placement = None;
            state.error = None;
        } else if state.placing_radial_center {
            state.placing_radial_center = false;
            state.error = None;
        } else if state.trim_active {
            state.trim_active = false;
            state.trim_phase = TrimPhase::Targets;
            state.trim_target_faces.clear();
            state.trim_cutter_faces.clear();
            state.trim_path_active = false;
            state.trim_path_points.clear();
            state.trim_path_face = None;
            state.error = None;
        } else if state.mirror_active {
            state.mirror_active = false;
            state.mirror_points.clear();
            state.error = None;
        } else if state.fillet_active || state.offset_active {
            state.fillet_active = false;
            state.offset_active = false;
            state.operation_value_focused = false;
            state.operation_value_selected = false;
            state.error = None;
        } else if state.paint_active {
            state.paint_active = false;
            state.paint_hex_focused = false;
            state.paint_hex_selected = false;
            state.error = None;
        } else if state.editing {
            if state.dirty {
                state.exit_confirmation_open = true;
            } else {
                close_editor(state);
            }
        } else {
            *scene = Scene::MainMenu;
            return;
        }
    }
    if state.mirror_active && is_key_pressed(KeyCode::Backspace) && !state.mirror_points.is_empty()
    {
        state.mirror_points.pop();
        state.error = None;
    }
    if state.editing && is_key_pressed(KeyCode::Enter) {
        if state.trim_active {
            match advance_trim(catalog, state) {
                Ok(()) => state.error = None,
                Err(error) => state.error = Some(error),
            }
        } else if state.mirror_active {
            if state.mirror_points.len() == 3 {
                let [origin, second, third] = [
                    state.mirror_points[0],
                    state.mirror_points[1],
                    state.mirror_points[2],
                ];
                match apply_mirror(
                    catalog,
                    state,
                    origin,
                    (second - origin).cross(third - origin),
                ) {
                    Ok(()) => {
                        state.mirror_active = false;
                        state.mirror_points.clear();
                    }
                    Err(error) => state.error = Some(error),
                }
            } else {
                state.error = Some("Choose three points to define the mirror plane".to_string());
            }
        } else if state.fillet_active {
            match apply_fillet(catalog, state) {
                Ok(()) => state.fillet_active = false,
                Err(error) => state.error = Some(error),
            }
        } else if state.offset_active {
            match apply_offset(catalog, state) {
                Ok(()) => state.offset_active = false,
                Err(error) => state.error = Some(error),
            }
        }
    }
    let mut modal_open = state.exit_confirmation_open
        || state.group_config_dialog.is_some()
        || state.primitive_placement.is_some();
    if !modal_open && state.numeric_transform_open && state.numeric_transform_focused {
        let target = &mut state.numeric_transform_values;
        let mut changed = false;
        while let Some(character) = get_char_pressed() {
            let allowed = character.is_ascii_digit() || matches!(character, '.' | ',' | ' ' | '-');
            if allowed && (state.numeric_transform_selected || target.len() < 64) {
                if state.numeric_transform_selected {
                    target.clear();
                }
                target.push(character);
                state.numeric_transform_selected = false;
                changed = true;
            }
        }
        if is_key_pressed(KeyCode::Backspace) && !target.is_empty() {
            if state.numeric_transform_selected {
                target.clear();
            } else {
                target.pop();
            }
            state.numeric_transform_selected = false;
            changed = true;
        }
        if changed {
            state.error = None;
        }
    }
    if !modal_open && (state.fillet_active || state.offset_active) && state.operation_value_focused
    {
        while let Some(character) = get_char_pressed() {
            if character.is_ascii_digit() || matches!(character, '.' | '-' | '+') {
                if state.operation_value_selected {
                    state.operation_value.clear();
                    state.operation_value_selected = false;
                }
                if state.operation_value.len() < 24 {
                    state.operation_value.push(character);
                }
                state.error = None;
            }
        }
        if is_key_pressed(KeyCode::Backspace) && !state.operation_value.is_empty() {
            if state.operation_value_selected {
                state.operation_value.clear();
                state.operation_value_selected = false;
            } else {
                state.operation_value.pop();
            }
            state.error = None;
        }
    }
    if !modal_open && state.paint_active && state.paint_hex_focused {
        let mut changed = false;
        while let Some(character) = get_char_pressed() {
            if character.is_ascii_hexdigit() {
                if state.paint_hex_selected {
                    state.paint_hex.clear();
                    state.paint_hex_selected = false;
                }
                if state.paint_hex.chars().count() < 8 {
                    state.paint_hex.push(character.to_ascii_uppercase());
                    changed = true;
                }
            }
        }
        if is_key_pressed(KeyCode::Backspace) && !state.paint_hex.is_empty() {
            if state.paint_hex_selected {
                state.paint_hex.clear();
                state.paint_hex_selected = false;
            } else {
                state.paint_hex.pop();
            }
            changed = true;
        }
        if is_key_pressed(KeyCode::Enter) {
            match parse_paint_hex(&state.paint_hex) {
                Ok(color) => {
                    state.paint_active = true;
                    state.paint_color = color;
                    state.paint_hex = format!(
                        "{:02X}{:02X}{:02X}",
                        (color.r * 255.0).round() as u8,
                        (color.g * 255.0).round() as u8,
                        (color.b * 255.0).round() as u8,
                    );
                    state.paint_hex_focused = false;
                    state.paint_hex_selected = false;
                    state.error = None;
                }
                Err(error) => state.error = Some(error),
            }
        }
        if changed {
            match parse_paint_hex(&state.paint_hex) {
                Ok(color) => {
                    state.paint_color = color;
                    state.error = None;
                }
                Err(_) => {}
            }
        }
    }
    if !modal_open
        && state.numeric_transform_open
        && state.numeric_transform_focused
        && is_key_pressed(KeyCode::Enter)
    {
        match apply_numeric_transform(catalog, state) {
            Ok(()) => state.error = None,
            Err(error) => state.error = Some(error),
        }
    }
    if !state.exit_confirmation_open
        && state.group_config_dialog.is_none()
        && state.rename_dialog.is_none()
        && let Some(placement) = state
            .primitive_placement
            .as_mut()
            .filter(|placement| placement.center.is_some())
    {
        if placement.input_focused {
            while let Some(character) = get_char_pressed() {
                if character.is_ascii_digit() || matches!(character, '.' | ',' | ' ') {
                    if placement.input_selected {
                        placement.dimensions.clear();
                        placement.input_selected = false;
                    }
                    if placement.dimensions.len() < 32 {
                        placement.dimensions.push(character);
                    }
                }
            }
            if is_key_pressed(KeyCode::Backspace) {
                if placement.input_selected {
                    placement.dimensions.clear();
                    placement.input_selected = false;
                } else {
                    placement.dimensions.pop();
                }
            }
            if is_key_pressed(KeyCode::Left) {
                select_placement_dimension(placement, -1);
            } else if is_key_pressed(KeyCode::Right) {
                select_placement_dimension(placement, 1);
            }
            let step = if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
                0.01
            } else {
                0.1
            };
            let adjustment = if is_key_pressed(KeyCode::Up) {
                Some(step)
            } else if is_key_pressed(KeyCode::Down) {
                Some(-step)
            } else {
                None
            };
            if let Some(delta) = adjustment {
                match adjust_placement_dimension(placement, delta) {
                    Ok(()) => state.error = None,
                    Err(error) => state.error = Some(error),
                }
            }
        }
        if is_key_pressed(KeyCode::Enter) {
            confirm_primitive_placement(catalog, state);
        }
    }
    if !modal_open && let Some(dialog) = state.rename_dialog.as_mut() {
        while let Some(character) = get_char_pressed() {
            if !character.is_control() && dialog.text.chars().count() < 64 {
                dialog.text.push(character);
            }
        }
        if is_key_pressed(KeyCode::Backspace) {
            dialog.text.pop();
        }
    }
    if !modal_open && is_key_pressed(KeyCode::Enter) {
        if state.rename_dialog.is_some() {
            match apply_rename_dialog(catalog, state) {
                Ok(()) => state.error = None,
                Err(error) => state.error = Some(error),
            }
        }
    }
    let rename_dialog_open = state.rename_dialog.is_some();
    if state.editing && !modal_open && !rename_dialog_open {
        let control_down = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        if control_down && is_key_pressed(KeyCode::Z) {
            undo_or_redo(catalog, state, true);
        } else if control_down && is_key_pressed(KeyCode::Y) {
            undo_or_redo(catalog, state, false);
        } else if is_key_pressed(KeyCode::Delete)
            && (!state.selected_faces.is_empty() || state.selected_group.is_some())
        {
            match delete_selected(catalog, state) {
                Ok(()) => state.error = None,
                Err(error) => state.error = Some(error),
            }
        }
    }
    let mut ui_capturing = sidebar.contains(ctx.mouse)
        || toolbar.contains(ctx.mouse)
        || view_panel.contains(ctx.mouse)
        || paint_panel.contains(ctx.mouse)
        || state
            .primitive_placement
            .as_ref()
            .is_some_and(|placement| placement.center.is_some())
            && placement_panel_rect(viewport).contains(ctx.mouse);
    if !modal_open && !rename_dialog_open && state.editing {
        if button(ctx, object_snap_rect(view_panel), "Object Snaps...") {
            state.snap_settings_open = !state.snap_settings_open;
        }
        if button(
            ctx,
            grid_spacing_minus_rect(view_panel),
            &format!("Grid: {:.2} (-)", state.grid_spacing),
        ) {
            state.grid_spacing = (state.grid_spacing - 0.1).max(0.1);
        }
        if button(ctx, grid_spacing_plus_rect(view_panel), "+") {
            state.grid_spacing = (state.grid_spacing + 0.1).min(10.0);
        }
        if button(
            ctx,
            interaction_preview_rect(view_panel),
            if state.interaction_preview {
                "Mesh View"
            } else {
                "Interaction Preview"
            },
        ) {
            state.interaction_preview = !state.interaction_preview;
            if state.interaction_preview {
                shuffle_preview_neighbors(state, catalog);
            }
        }
        if state.interaction_preview
            && button(ctx, preview_shuffle_rect(view_panel), "Shuffle neighbors")
        {
            shuffle_preview_neighbors(state, catalog);
        }
        let radial_metadata = selected_metadata(catalog, state)
            .filter(|metadata| metadata.radial_main_group_id().is_some());
        let radial_pivot = radial_metadata.and_then(|metadata| metadata.radial_pivot);
        let radial_center_label = if state.placing_radial_center {
            "Cancel UCS placement"
        } else if radial_pivot.is_some() {
            "Move UCS origin"
        } else {
            "Place UCS origin"
        };
        if radial_metadata.is_some()
            && button(ctx, radial_center_rect(view_panel), radial_center_label)
        {
            state.placing_radial_center = !state.placing_radial_center;
            state.error = None;
        }
        if state.grid_spacing != 0.5 {
            state.grid_spacing = (state.grid_spacing * 100.0).round() / 100.0;
        }
    }
    let mut category_y = 68.0;
    if !state.editing {
        for category in [
            ObjectCategory::Buildings,
            ObjectCategory::Units,
            ObjectCategory::MachineBlocks,
        ] {
            let rect = Rect::new(12.0, category_y, SIDEBAR_WIDTH - 24.0, 34.0);
            if !modal_open && !rename_dialog_open && list_row_clicked(rect, ctx.mouse) {
                state.category = category;
                if let Some(first) = catalog
                    .entries()
                    .iter()
                    .find(|entry| entry.definition.category == category)
                {
                    switch_to_object(state, first.definition.id);
                }
            }

            category_y += 40.0;
        }
    }

    let mut object_y = category_y + 8.0;
    let category = state.category;
    if !state.editing {
        for entry in catalog
            .entries()
            .iter()
            .filter(|entry| entry.definition.category == category)
        {
            let rect = Rect::new(12.0, object_y, SIDEBAR_WIDTH - 24.0, 32.0);
            if !modal_open && !rename_dialog_open && list_row_clicked(rect, ctx.mouse) {
                switch_to_object(state, entry.definition.id);
            }
            object_y += 36.0;
        }
    } else {
        let face_count = selected_mesh(catalog, state)
            .map(EditableMesh::face_count)
            .unwrap_or(0);
        let items = selected_metadata(catalog, state)
            .map(|metadata| sidebar_items(metadata, face_count))
            .unwrap_or_default();
        let visible_rows = ((screen_height() - FACE_LIST_TOP - 46.0) / FACE_ROW_HEIGHT)
            .floor()
            .max(1.0) as usize;
        if !modal_open && !rename_dialog_open && sidebar.contains(ctx.mouse) {
            let (_, wheel) = mouse_wheel();
            if wheel > 0.0 {
                state.face_scroll = state.face_scroll.saturating_sub(wheel.ceil() as usize);
            } else if wheel < 0.0 {
                state.face_scroll = state.face_scroll.saturating_add((-wheel).ceil() as usize);
            }
        }
        state.face_scroll = state
            .face_scroll
            .min(items.len().saturating_sub(visible_rows));
        let mut drop_target = None;
        for row in state.face_scroll..(state.face_scroll + visible_rows).min(items.len()) {
            let item = items[row];
            let y = FACE_LIST_TOP + (row - state.face_scroll) as f32 * FACE_ROW_HEIGHT;
            let show_lock_hide = matches!(item, SidebarItem::Group(group_id)
                if selected_metadata(catalog, state)
                    .and_then(|metadata| metadata.faces_for_group(group_id))
                    .is_some_and(|faces| !faces.is_empty()));
            let (depth, nested_face) = selected_metadata(catalog, state)
                .map(|metadata| match item {
                    SidebarItem::Group(group_id) => (group_depth(metadata, group_id), false),
                    SidebarItem::Face {
                        group: Some(group_id),
                        ..
                    } => (group_depth(metadata, group_id), true),
                    SidebarItem::Face { group: None, .. } => (0, false),
                })
                .unwrap_or((0, false));
            let indent = depth as f32 * 10.0 + if nested_face { 10.0 } else { 0.0 };
            let rect = Rect::new(
                12.0 + indent,
                y,
                SIDEBAR_WIDTH
                    - 24.0
                    - indent
                    - usize::from(matches!(item, SidebarItem::Group(_))) as f32 * 100.0,
                FACE_ROW_HEIGHT - 2.0,
            );
            let configure_rect = Rect::new(SIDEBAR_WIDTH - 108.0, y, 34.0, FACE_ROW_HEIGHT - 2.0);
            let lock_rect = Rect::new(SIDEBAR_WIDTH - 72.0, y, 28.0, FACE_ROW_HEIGHT - 2.0);
            let hide_rect = Rect::new(SIDEBAR_WIDTH - 42.0, y, 28.0, FACE_ROW_HEIGHT - 2.0);
            if !modal_open
                && !rename_dialog_open
                && matches!(item, SidebarItem::Group(_))
                && show_lock_hide
                && list_row_clicked(lock_rect, ctx.mouse)
            {
                if let SidebarItem::Group(group_id) = item
                    && let Err(error) =
                        toggle_group_flag(catalog, state, group_id, GroupFlag::Locked)
                {
                    state.error = Some(error);
                }
            } else if !modal_open
                && !rename_dialog_open
                && matches!(item, SidebarItem::Group(_))
                && show_lock_hide
                && list_row_clicked(hide_rect, ctx.mouse)
            {
                if let SidebarItem::Group(group_id) = item
                    && let Err(error) =
                        toggle_group_flag(catalog, state, group_id, GroupFlag::Hidden)
                {
                    state.error = Some(error);
                }
            } else if !modal_open
                && !rename_dialog_open
                && matches!(item, SidebarItem::Group(_))
                && list_row_clicked(configure_rect, ctx.mouse)
            {
                if let SidebarItem::Group(group_id) = item
                    && let Some(group) = selected_metadata(catalog, state)
                        .and_then(|metadata| metadata.group(group_id))
                {
                    state.group_config_dialog = Some(GroupConfigDialog {
                        group_id,
                        program: group.program,
                    });
                    modal_open = true;
                    state.error = None;
                }
            } else if !modal_open && !rename_dialog_open && list_row_clicked(rect, ctx.mouse) {
                match item {
                    SidebarItem::Group(group_id) => {
                        if let Some(metadata) = selected_metadata(catalog, state)
                            && let Err(error) = select_group(state, metadata, group_id)
                        {
                            state.error = Some(error);
                        }
                    }
                    SidebarItem::Face { index, .. } => {
                        let extend =
                            is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                        if state.trim_active && state.trim_phase == TrimPhase::Targets {
                            toggle_index(&mut state.trim_target_faces, index);
                            state.selected_faces = state.trim_target_faces.clone();
                            state.selected_group = None;
                            state.selection_mode = SelectionMode::Face;
                            state.dragged_faces = None;
                        } else if state.trim_active && state.trim_phase == TrimPhase::Cutters {
                            toggle_index(&mut state.trim_cutter_faces, index);
                            state.selected_faces = state.trim_cutter_faces.clone();
                            state.selected_group = None;
                            state.selection_mode = SelectionMode::Face;
                            state.dragged_faces = None;
                        } else {
                            select_face(state, index, extend);
                            let payload = if state.selected_faces.contains(&index) {
                                state.selected_faces.clone()
                            } else {
                                vec![index]
                            };
                            state.dragged_faces = Some(payload);
                        }
                    }
                }
            }
            if !modal_open
                && !rename_dialog_open
                && is_mouse_button_released(MouseButton::Left)
                && let SidebarItem::Group(group_id) = item
                && state.dragged_faces.is_some()
                && rect.contains(ctx.mouse)
            {
                drop_target = Some(group_id);
            }
        }
        if is_mouse_button_released(MouseButton::Left) {
            if let (Some(faces), Some(group_id)) = (state.dragged_faces.take(), drop_target) {
                match assign_dragged_faces(catalog, state, &faces, group_id) {
                    Ok(()) => state.error = None,
                    Err(error) => state.error = Some(error),
                }
            } else {
                state.dragged_faces = None;
            }
        }
    }

    let mut x = SIDEBAR_WIDTH + 12.0;
    let editing_label = if state.editing { "Editing" } else { "Edit" };
    if !modal_open
        && !rename_dialog_open
        && button(ctx, Rect::new(x, 12.0, 86.0, 34.0), editing_label)
    {
        if !state.editing {
            if let (Some(mesh), Some(metadata)) = (
                selected_mesh(catalog, state),
                selected_metadata(catalog, state),
            ) {
                state.original_mesh = Some(mesh.clone());
                state.original_metadata = Some(metadata.clone());
                state.editing = true;
                state.dirty = false;
                state.face_scroll = 0;
                state.selected_faces.clear();
                state.selected_group = None;
                state.undo_stack.clear();
                state.redo_stack.clear();
                state.error = None;
            } else {
                state.error = selected_entry(catalog, state.selected)
                    .and_then(|entry| entry.metadata_error.clone().or_else(|| entry.error.clone()))
                    .or_else(|| Some("Selected object is unavailable".to_string()));
            }
        }
    }
    x += 94.0;
    if !modal_open
        && !rename_dialog_open
        && state.editing
        && button(ctx, Rect::new(x, 12.0, 86.0, 34.0), "Vertex")
    {
        state.selection_mode = SelectionMode::Vertex;
        state.face_scroll = 0;
        state.selected_faces.clear();
        state.selected_group = None;
        state.selected_edges.clear();
        state.selected_vertices.clear();
    }
    x += 94.0;
    if !modal_open
        && !rename_dialog_open
        && state.editing
        && button(ctx, Rect::new(x, 12.0, 86.0, 34.0), "Edge")
    {
        state.selection_mode = SelectionMode::Edge;
        state.selected_faces.clear();
        state.selected_group = None;
        state.selected_vertices.clear();
    }
    x += 94.0;
    if !modal_open
        && !rename_dialog_open
        && state.editing
        && button(ctx, Rect::new(x, 12.0, 86.0, 34.0), "Face")
    {
        state.selection_mode = SelectionMode::Face;
        state.selected_edges.clear();
        state.selected_vertices.clear();
    }
    x += 94.0;
    if !modal_open
        && !rename_dialog_open
        && state.editing
        && button(ctx, Rect::new(x, 12.0, 86.0, 34.0), "Save")
    {
        match save_selected_mesh(catalog, state) {
            Ok(()) => {
                state.dirty = false;
                state.original_mesh = selected_mesh(catalog, state).cloned();
                state.original_metadata = selected_metadata(catalog, state).cloned();
                state.error = None;
            }
            Err(error) => state.error = Some(error),
        }
    }
    x += 94.0;
    if state.editing {
        for (label, primitive) in [
            ("Cube", MeshPrimitive::Cube),
            ("Plane", MeshPrimitive::Plane),
            ("Sphere", MeshPrimitive::Sphere),
            ("Cylinder", MeshPrimitive::Cylinder),
        ] {
            if !modal_open
                && !rename_dialog_open
                && button(ctx, Rect::new(x, 12.0, 72.0, 34.0), label)
            {
                state.numeric_transform_open = false;
                state.numeric_transform_focused = false;
                state.numeric_transform_selected = false;
                start_primitive_placement(state, primitive);
                modal_open = true;
            }
            x += 76.0;
        }
    }
    if let Some(placement) = state.primitive_placement.as_ref()
        && placement.center.is_some()
    {
        if placement_input_rect(viewport).contains(ctx.mouse)
            && is_mouse_button_pressed(MouseButton::Left)
            && let Some(placement) = state.primitive_placement.as_mut()
        {
            placement.input_focused = true;
            placement.input_selected = true;
        }
        if button(ctx, placement_create_rect(viewport), "Create") {
            confirm_primitive_placement(catalog, state);
        }
        if button(ctx, placement_cancel_rect(viewport), "Cancel") {
            state.primitive_placement = None;
            state.error = None;
        }
        ui_capturing |= placement_panel_rect(viewport).contains(ctx.mouse);
    }
    if state.editing {
        let add_group = Rect::new(12.0, 66.0, 108.0, 30.0);
        let rename = Rect::new(130.0, 66.0, 108.0, 30.0);
        let undo_button = Rect::new(12.0, 102.0, 108.0, 30.0);
        let redo_button = Rect::new(130.0, 102.0, 108.0, 30.0);
        if !modal_open && !rename_dialog_open && button(ctx, add_group, "Add Group") {
            match create_selected_group(catalog, state) {
                Ok(()) => state.error = None,
                Err(error) => state.error = Some(error),
            }
        }
        if !modal_open && !rename_dialog_open && button(ctx, rename, "Rename") {
            let target = if let Some(group_id) = state.selected_group {
                selected_metadata(catalog, state)
                    .and_then(|metadata| metadata.group(group_id))
                    .map(|group| (RenameTarget::Group(group_id), group.name.clone()))
            } else if state.selected_faces.len() == 1 {
                let face = state.selected_faces[0];
                selected_metadata(catalog, state).map(|metadata| {
                    (
                        RenameTarget::Face(face),
                        metadata.face_names[face]
                            .clone()
                            .unwrap_or_else(|| format!("Face {}", face + 1)),
                    )
                })
            } else {
                None
            };
            if let Some((target, text)) = target {
                state.rename_dialog = Some(RenameDialog { target, text });
                state.error = None;
            } else {
                state.error = Some("Select one face or one group to rename".to_string());
            }
        }
        if !modal_open && !rename_dialog_open && button(ctx, undo_button, "Undo") {
            undo_or_redo(catalog, state, true);
        }
        if !modal_open && !rename_dialog_open && button(ctx, redo_button, "Redo") {
            undo_or_redo(catalog, state, false);
        }
    }
    let mut action_x = SIDEBAR_WIDTH + 12.0;
    let action_y = 76.0;
    if state.editing {
        for (mode, label) in [
            (TransformMode::Move, "Move"),
            (TransformMode::Rotate, "Rotate"),
            (TransformMode::Scale, "Scale"),
        ] {
            let rect = Rect::new(action_x, action_y, 72.0, 30.0);
            if !modal_open && !rename_dialog_open && button(ctx, rect, label) {
                state.transform_mode = mode;
                state.numeric_transform_values = match mode {
                    TransformMode::Scale => "1, 1, 1",
                    TransformMode::Move | TransformMode::Rotate => "0, 0, 0",
                }
                .to_string();
                state.numeric_transform_selected = state.numeric_transform_open;
                state.error = None;
            }
            draw_rectangle_lines(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                2.0,
                if state.transform_mode == mode {
                    Color::from_rgba(27, 119, 105, 255)
                } else {
                    ctx.colors_rt.button_border
                },
            );
            action_x += 76.0;
        }
        if !modal_open
            && !rename_dialog_open
            && button(ctx, Rect::new(action_x, action_y, 72.0, 30.0), "Value")
        {
            state.numeric_transform_open = !state.numeric_transform_open;
            state.numeric_transform_focused = state.numeric_transform_open;
            state.numeric_transform_selected = state.numeric_transform_open;
            state.numeric_transform_values = match state.transform_mode {
                TransformMode::Scale => "1, 1, 1",
                TransformMode::Move | TransformMode::Rotate => "0, 0, 0",
            }
            .to_string();
            state.error = None;
        }
        action_x += 76.0;
        if !modal_open
            && !rename_dialog_open
            && button(ctx, Rect::new(action_x, action_y, 84.0, 30.0), "Extrude")
        {
            if state.selected_faces.len() != 1 {
                state.error = Some("Select exactly one face to extrude".to_string());
            } else {
                let face = state.selected_faces[0];
                if let Err(error) = selected_metadata(catalog, state)
                    .ok_or_else(|| "Selected face metadata is unavailable".to_string())
                    .and_then(|metadata| ensure_faces_editable(metadata, &[face]))
                {
                    state.error = Some(error);
                    return;
                }
                let before = match capture_snapshot(catalog, state) {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        state.error = Some(error);
                        return;
                    }
                };
                let old_face_count = before.mesh.face_count();
                let result = selected_mesh_mut(catalog, state)
                    .ok_or_else(|| "Selected object is unavailable".to_string())
                    .and_then(|mesh| mesh.extrude_face(face, 0.2));
                match result {
                    Ok(()) => {
                        let new_face_count = selected_mesh(catalog, state)
                            .map(EditableMesh::face_count)
                            .unwrap_or(old_face_count);
                        if let Some(metadata) = selected_metadata_mut(catalog, state) {
                            match metadata.inherit_group_for_extrusion(
                                face,
                                old_face_count,
                                new_face_count.saturating_sub(old_face_count),
                            ) {
                                Ok(group_id) => {
                                    state.selected_group = Some(group_id);
                                }
                                Err(error) => {
                                    state.error = Some(error);
                                    return;
                                }
                            }
                        }
                        state.selected_faces = (old_face_count..new_face_count).collect();
                        state.selected_faces.push(face);
                        push_undo(state, before);
                        state.dirty = true;
                        state.error = None;
                    }
                    Err(error) => state.error = Some(error),
                }
            }
        }
        let modify_y = action_y + 38.0;
        let mut modify_x = SIDEBAR_WIDTH + 12.0;
        let trim_label = if state.trim_active {
            "Cancel Trim"
        } else {
            "Trim"
        };
        if !modal_open
            && !rename_dialog_open
            && button(ctx, Rect::new(modify_x, modify_y, 92.0, 30.0), trim_label)
        {
            if state.trim_active {
                clear_trim_state(state);
            } else {
                state.trim_active = true;
                state.trim_phase = TrimPhase::Targets;
                state.trim_target_faces.clear();
                state.trim_cutter_faces.clear();
                state.trim_path_active = false;
                state.trim_path_points.clear();
                state.trim_path_face = None;
                state.selected_faces.clear();
                state.selected_group = None;
                state.selected_edges.clear();
                state.selection_mode = SelectionMode::Face;
                state.mirror_active = false;
                state.mirror_points.clear();
                state.fillet_active = false;
                state.offset_active = false;
                state.operation_value_focused = false;
                state.numeric_transform_open = false;
                state.numeric_transform_focused = false;
                state.error = None;
            }
        }
        modify_x += 98.0;
        if state.trim_active && state.trim_phase == TrimPhase::Cutters {
            let path_label = if state.trim_path_active {
                "Finish Path"
            } else if state.trim_path_points.is_empty() {
                "Draw Path"
            } else {
                "Redraw Path"
            };
            if !modal_open
                && !rename_dialog_open
                && button(ctx, Rect::new(modify_x, modify_y, 94.0, 30.0), path_label)
            {
                if state.trim_path_active {
                    if let Err(error) = finish_trim_path(state) {
                        state.error = Some(error);
                    }
                } else {
                    state.trim_path_active = true;
                    state.trim_path_points.clear();
                    state.trim_path_face = None;
                    state.error = None;
                }
            }
            modify_x += 100.0;
        }
        for (label, active, width) in [
            (
                if state.mirror_active {
                    "Cancel Mirror"
                } else {
                    "Mirror"
                },
                state.mirror_active,
                92.0,
            ),
            (
                if state.fillet_active {
                    "Cancel Fillet"
                } else {
                    "Fillet"
                },
                state.fillet_active,
                88.0,
            ),
            (
                if state.offset_active {
                    "Cancel Offset"
                } else {
                    "Offset"
                },
                state.offset_active,
                88.0,
            ),
        ] {
            let rect = Rect::new(modify_x, modify_y, width, 30.0);
            if !modal_open && !rename_dialog_open && button(ctx, rect, label) {
                if active {
                    clear_trim_state(state);
                    state.mirror_active = false;
                    state.mirror_points.clear();
                    state.fillet_active = false;
                    state.offset_active = false;
                    state.operation_value_focused = false;
                    state.error = None;
                } else if label == "Mirror" && state.selected_faces.is_empty() {
                    state.error = Some("Select faces to mirror before starting the tool".into());
                } else if label == "Offset" && state.selected_faces.is_empty() {
                    state.error = Some("Select faces to offset before starting the tool".into());
                } else if label == "Fillet" && state.selected_edges.is_empty() {
                    state.error =
                        Some("Select mesh edges to fillet before starting the tool".into());
                } else {
                    clear_trim_state(state);
                    state.paint_active = false;
                    state.paint_hex_focused = false;
                    state.paint_hex_selected = false;
                    state.mirror_active = label == "Mirror";
                    state.mirror_points.clear();
                    state.fillet_active = label == "Fillet";
                    state.offset_active = label == "Offset";
                    state.operation_value_focused = state.fillet_active || state.offset_active;
                    state.operation_value_selected = state.operation_value_focused;
                    if state.trim_active {
                        state.selected_faces.clear();
                        state.selected_group = None;
                    } else if state.mirror_active || state.offset_active {
                        state.selection_mode = SelectionMode::Face;
                    } else if state.fillet_active {
                        state.selection_mode = SelectionMode::Edge;
                    }
                    state.numeric_transform_open = false;
                    state.numeric_transform_focused = false;
                    state.error = None;
                }
            }
            modify_x += width + 6.0;
        }
    }
    ui_capturing |= state.editing
        && Rect::new(SIDEBAR_WIDTH, action_y - 4.0, screen_width(), 40.0).contains(ctx.mouse);
    ui_capturing |= state.editing
        && Rect::new(SIDEBAR_WIDTH, action_y + 34.0, screen_width(), 38.0).contains(ctx.mouse);
    ui_capturing |=
        state.numeric_transform_open && numeric_transform_panel_rect().contains(ctx.mouse);
    ui_capturing |=
        state.snap_settings_open && object_snap_panel_rect(view_panel).contains(ctx.mouse);
    ui_capturing |=
        state.snap_settings_open && object_snap_panel_rect(view_panel).contains(ctx.mouse);
    let operation_panel_open = state.fillet_active || state.offset_active;
    let operation_panel = operation_value_panel_rect(viewport);
    ui_capturing |= operation_panel_open && operation_panel.contains(ctx.mouse);
    if operation_panel_open && is_mouse_button_pressed(MouseButton::Left) {
        state.operation_value_focused = operation_value_input_rect(viewport).contains(ctx.mouse);
        state.operation_value_selected = state.operation_value_focused;
    }
    if state.numeric_transform_open
        && is_mouse_button_pressed(MouseButton::Left)
        && !numeric_transform_panel_rect().contains(ctx.mouse)
        && !Rect::new(SIDEBAR_WIDTH, action_y - 4.0, screen_width(), 40.0).contains(ctx.mouse)
    {
        state.numeric_transform_focused = false;
        state.numeric_transform_selected = false;
    }

    let camera = object_camera(state);
    let editor_modal_open =
        state.exit_confirmation_open || state.group_config_dialog.is_some() || rename_dialog_open;
    let viewport_active = state.gizmo_drag.is_some()
        || (!editor_modal_open && viewport.contains(ctx.mouse) && !ui_capturing);
    if viewport_active && state.gizmo_drag.is_none() && is_mouse_button_down(MouseButton::Right) {
        let delta = ctx.mouse - state.last_mouse;
        state.yaw -= delta.x * 0.008;
        state.pitch = (state.pitch - delta.y * 0.006).clamp(-1.45, 1.45);
    } else if viewport_active
        && state.gizmo_drag.is_none()
        && is_mouse_button_down(MouseButton::Middle)
    {
        let delta = ctx.mouse - state.last_mouse;
        let position = camera.position;
        let forward = (camera.target - position).normalize_or_zero();
        let right = forward.cross(camera.up).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let units_per_pixel = state.distance * 0.0018;
        state.target += right * (-delta.x * units_per_pixel) + up * (delta.y * units_per_pixel);
    }
    if viewport_active {
        let (_, wheel) = mouse_wheel();
        if wheel.abs() > 0.001 {
            state.distance = zoom_distance(state.distance, wheel);
        }
    }
    state.last_mouse = ctx.mouse;
    let camera = if state.interaction_preview {
        preview_camera(state)
    } else {
        object_camera(state)
    };

    let current_mesh = selected_mesh(catalog, state).cloned();
    let hidden_faces = selected_metadata(catalog, state)
        .map(FaceMetadata::hidden_faces)
        .unwrap_or_default();
    let mut blocked_faces = selected_metadata(catalog, state)
        .map(FaceMetadata::locked_faces)
        .unwrap_or_default();
    blocked_faces.extend(hidden_faces.iter().copied());
    if let (Some(drag), Some(mesh)) = (state.gizmo_drag, current_mesh.as_ref()) {
        if is_mouse_button_down(MouseButton::Left) || is_mouse_button_released(MouseButton::Left) {
            let radial_grid_center = selected_metadata(catalog, state).and_then(|metadata| {
                metadata.radial_main_group_id().map(|_| {
                    metadata
                        .radial_pivot
                        .map(|[x, y, z]| vec3(x, y, z))
                        .unwrap_or(Vec3::ZERO)
                })
            });
            let update =
                update_gizmo_drag(&camera, ctx.mouse, mesh, state, radial_grid_center, drag);
            state.gizmo_preview = update.map(|update| update.command);
            state.gizmo_snap_target = update.and_then(|update| update.snap_target);
            state.gizmo_polar_angle = update.and_then(|update| update.polar_angle);
        }
        if is_mouse_button_released(MouseButton::Left) {
            if let Some(command) = state.gizmo_preview {
                match commit_transform_command(catalog, state, command) {
                    Ok(()) => state.error = None,
                    Err(error) => {
                        state.gizmo_drag = None;
                        state.gizmo_preview = None;
                        state.gizmo_snap_target = None;
                        state.gizmo_polar_angle = None;
                        state.error = Some(error);
                    }
                }
            } else {
                state.gizmo_drag = None;
                state.gizmo_snap_target = None;
                state.gizmo_polar_angle = None;
            }
        }
    }
    let preview = current_mesh.as_ref().map(|mesh| {
        let mut candidate = mesh.clone();
        if let Some(command) = state.gizmo_preview
            && let Err(error) = apply_transform_to_selection(&mut candidate, state, command)
        {
            state.error = Some(error);
        }
        preview_mesh_with_reference(&candidate, mesh)
    });
    let trim_path_cursor = if state.trim_active
        && state.trim_phase == TrimPhase::Cutters
        && state.trim_path_active
        && viewport.contains(ctx.mouse)
        && !ui_capturing
    {
        current_mesh
            .as_ref()
            .zip(preview.as_ref())
            .and_then(|(source_mesh, display_mesh)| {
                let (_, point) = trim_path_point_at_cursor(
                    &camera,
                    ctx.mouse,
                    display_mesh,
                    &state.trim_target_faces,
                    state.trim_path_face,
                    state.object_snaps,
                )?;
                Some(display_to_model_point(source_mesh, point))
            })
    } else {
        None
    };
    let mirror_cursor_point =
        if state.mirror_active && viewport.contains(ctx.mouse) && !ui_capturing {
            current_mesh
                .as_ref()
                .zip(preview.as_ref())
                .and_then(|(source_mesh, display_mesh)| {
                    selected_metadata(catalog, state).and_then(|metadata| {
                        model_point_at_cursor(
                            &camera,
                            ctx.mouse,
                            source_mesh,
                            display_mesh,
                            state,
                            metadata,
                        )
                    })
                })
        } else {
            None
        };
    let operation_preview = current_mesh.as_ref().and_then(|mesh| {
        let mut candidate = mesh.clone();
        let preview_result = if state.fillet_active {
            parse_operation_distance(&state.operation_value, "fillet radius").and_then(|radius| {
                candidate
                    .fillet_edges(&state.selected_edges, radius, 4)
                    .map(|_| ())
            })
        } else if state.offset_active {
            parse_operation_distance(&state.operation_value, "offset distance").and_then(
                |distance| {
                    candidate
                        .copy_faces_offset(&state.selected_faces, distance)
                        .map(|_| ())
                },
            )
        } else if state.mirror_active && state.mirror_points.len() == 3 {
            let [origin, second, third] = [
                state.mirror_points[0],
                state.mirror_points[1],
                state.mirror_points[2],
            ];
            candidate
                .copy_faces_reflected(
                    &state.selected_faces,
                    origin,
                    (second - origin).cross(third - origin),
                )
                .map(|_| ())
        } else {
            Err("No preview operation".to_string())
        };
        preview_result
            .ok()
            .map(|()| preview_mesh_with_reference(&candidate, mesh))
    });
    if state.interaction_preview {
        if let Err(error) = draw_interaction_preview(catalog, state) {
            state.error = Some(error);
        }
    } else {
        set_camera(&camera);
        if state.editing
            && let Some(mesh) = current_mesh.as_ref()
        {
            let (model_center, scale) = mesh_center_and_scale(mesh);
            let metadata = selected_metadata(catalog, state);
            if let Some(metadata) = metadata
                && metadata.radial_main_group_id().is_some()
            {
                let pivot = metadata
                    .radial_pivot
                    .map(|[x, y, z]| vec3(x, y, z))
                    .unwrap_or(Vec3::ZERO);
                let center = (pivot - model_center) * scale;
                draw_triangular_grid(center, scale, state.grid_spacing);
                draw_ucs_icon(center, 0.32);
            } else {
                draw_square_grid(model_center, scale, state.grid_spacing);
            }
        }
        if let Some(mesh) = operation_preview.as_ref().or(preview.as_ref()) {
            let render_mesh = if !hidden_faces.is_empty()
                && current_mesh
                    .as_ref()
                    .is_some_and(|source| source.face_count() == mesh.face_count())
            {
                mesh_without_faces(mesh, &hidden_faces)
            } else {
                mesh.clone()
            };
            if let Err(error) = render_mesh.render() {
                state.error = Some(error);
            }
            if state.editing {
                for &index in &state.selected_vertices {
                    if let Some(vertex) = mesh.vertices.get(index) {
                        draw_cube(
                            vertex.position,
                            vec3(0.07, 0.07, 0.07),
                            None,
                            Color::from_rgba(255, 235, 60, 255),
                        );
                    }
                }
                if let Some(mesh) = current_mesh.as_ref()
                    && let Some([x, y, z]) =
                        selected_metadata(catalog, state).and_then(|metadata| metadata.radial_pivot)
                {
                    let (center, scale) = mesh_center_and_scale(mesh);
                    draw_sphere(
                        (vec3(x, y, z) - center) * scale,
                        0.06,
                        None,
                        Color::from_rgba(70, 235, 120, 255),
                    );
                }
                for &(a, b) in &state.selected_edges {
                    if let (Some(a), Some(b)) = (mesh.vertices.get(a), mesh.vertices.get(b)) {
                        draw_line_3d(a.position, b.position, Color::from_rgba(80, 235, 240, 255));
                    }
                }
                for &face in &state.selected_faces {
                    let start = face * 3;
                    if let Some(indices) = mesh.indices.get(start..start + 3) {
                        let points = indices
                            .iter()
                            .filter_map(|&index| mesh.vertices.get(index as usize))
                            .map(|vertex| vertex.position)
                            .collect::<Vec<_>>();
                        if points.len() == 3 {
                            for edge in 0..3 {
                                draw_line_3d(
                                    points[edge],
                                    points[(edge + 1) % 3],
                                    Color::from_rgba(255, 235, 60, 255),
                                );
                            }
                        }
                    }
                }
                for &face in &state.trim_target_faces {
                    draw_face_outline(mesh, face, Color::from_rgba(255, 190, 65, 255));
                }
                for &face in &state.trim_cutter_faces {
                    draw_face_outline(mesh, face, Color::from_rgba(180, 130, 255, 255));
                }
                if let Some(source_mesh) = current_mesh.as_ref() {
                    let path = state
                        .trim_path_points
                        .iter()
                        .map(|&point| model_to_display_point(source_mesh, point))
                        .collect::<Vec<_>>();
                    for pair in path.windows(2) {
                        draw_line_3d(pair[0], pair[1], Color::from_rgba(255, 190, 65, 255));
                    }
                    for point in &path {
                        draw_sphere(*point, 0.035, None, Color::from_rgba(255, 190, 65, 255));
                    }
                    if let Some(point) = trim_path_cursor {
                        let cursor = model_to_display_point(source_mesh, point);
                        if let Some(&last) = path.last() {
                            draw_line_3d(last, cursor, Color::from_rgba(255, 190, 65, 210));
                        }
                        draw_sphere(cursor, 0.045, None, Color::from_rgba(255, 190, 65, 255));
                    }
                    let mirror_points = state
                        .mirror_points
                        .iter()
                        .map(|&point| model_to_display_point(source_mesh, point))
                        .collect::<Vec<_>>();
                    for pair in mirror_points.windows(2) {
                        draw_line_3d(pair[0], pair[1], Color::from_rgba(200, 125, 255, 255));
                    }
                    if mirror_points.len() == 3 {
                        draw_line_3d(
                            mirror_points[2],
                            mirror_points[0],
                            Color::from_rgba(200, 125, 255, 255),
                        );
                    }
                    for point in &mirror_points {
                        draw_sphere(*point, 0.045, None, Color::from_rgba(200, 125, 255, 255));
                    }
                    if let Some(cursor_point) = mirror_cursor_point {
                        let cursor = model_to_display_point(source_mesh, cursor_point);
                        if let Some(&last) = mirror_points.last() {
                            draw_line_3d(last, cursor, Color::from_rgba(200, 125, 255, 210));
                        }
                        draw_sphere(cursor, 0.04, None, Color::from_rgba(200, 125, 255, 255));
                    }
                }
                if let Some(source_mesh) = current_mesh.as_ref()
                    && let Some((start_center, handle_size)) =
                        gizmo_center_and_size(source_mesh, state)
                {
                    let mut center = start_center;
                    if let Some(command) = state.gizmo_preview {
                        let indices = selected_indices(source_mesh, state);
                        if let Ok(pivot) = selected_bounds_center(source_mesh, &indices) {
                            center = model_to_display_point(
                                source_mesh,
                                transform_position(pivot, command),
                            );
                        }
                    }
                    draw_transform_gizmo(center, state.transform_mode, handle_size);
                    if let Some(target) = state.gizmo_snap_target {
                        let snap_point = model_to_display_point(source_mesh, target.position);
                        draw_line_3d(
                            start_center,
                            snap_point,
                            Color::from_rgba(255, 220, 75, 220),
                        );
                        draw_sphere(
                            snap_point,
                            handle_size * 0.075,
                            None,
                            Color::from_rgba(255, 220, 75, 255),
                        );
                    }
                    if state.gizmo_polar_angle.is_some() {
                        let pivot_model = selected_bounds_center(
                            source_mesh,
                            &selected_indices(source_mesh, state),
                        )
                        .unwrap_or(Vec3::ZERO);
                        draw_line_3d(
                            start_center,
                            model_to_display_point(
                                source_mesh,
                                pivot_model
                                    + state
                                        .gizmo_preview
                                        .map(|command| command.values)
                                        .unwrap_or(Vec3::ZERO),
                            ),
                            Color::from_rgba(255, 220, 75, 220),
                        );
                    }
                }
            }
            if let (Some(placement), Some(base_mesh)) =
                (state.primitive_placement.as_ref(), current_mesh.as_ref())
                && let Some(center) = placement.center
                && let Ok(dimensions) =
                    parse_primitive_dimensions(placement.primitive, &placement.dimensions)
            {
                let ghost = EditableMesh::primitive(placement.primitive).transformed(|position| {
                    let model_position = center
                        + vec3(
                            position.x * dimensions.x,
                            position.y * dimensions.y,
                            position.z * dimensions.z,
                        );
                    model_to_display_point(base_mesh, model_position)
                });
                if let Err(error) = ghost.render() {
                    state.error = Some(error);
                }
            }
        }
        set_default_camera();
    }
    if let Some(target) = state.gizmo_snap_target {
        draw_text(
            &format!("{:?}", target.kind),
            ctx.mouse.x + 12.0,
            ctx.mouse.y - 10.0,
            ctx.font_sm,
            Color::from_rgba(255, 220, 75, 255),
        );
    } else if let Some(angle) = state.gizmo_polar_angle {
        draw_text(
            &format!("{angle:.0} deg"),
            ctx.mouse.x + 12.0,
            ctx.mouse.y + 16.0,
            ctx.font_sm,
            Color::from_rgba(255, 220, 75, 255),
        );
    }
    if viewport_active
        && state.editing
        && !state.interaction_preview
        && is_mouse_button_pressed(MouseButton::Left)
    {
        if let (Some((origin, direction)), Some(mesh)) =
            (mouse_ray(&camera, ctx.mouse), preview.as_ref())
        {
            if state.placing_radial_center {
                if let Some(display_point) = construction_plane_hit(origin, direction) {
                    let model_point = display_to_model_point(
                        current_mesh.as_ref().unwrap_or(mesh),
                        display_point,
                    );
                    match set_radial_center(catalog, state, model_point) {
                        Ok(()) => {
                            state.placing_radial_center = false;
                            state.error = None;
                        }
                        Err(error) => state.error = Some(error),
                    }
                } else {
                    state.error = Some("The click did not intersect the construction plane".into());
                }
            } else if let Some(placement) = state.primitive_placement.clone() {
                if let Some(display_point) = construction_plane_hit(origin, direction) {
                    let mut model_point = display_to_model_point(
                        current_mesh.as_ref().unwrap_or(mesh),
                        display_point,
                    );
                    if state.grid_snap {
                        if let Some(metadata) = selected_metadata(catalog, state) {
                            model_point = snap_model_point_to_grid(
                                model_point,
                                state,
                                metadata,
                                current_mesh.as_ref().unwrap_or(mesh),
                            );
                        } else {
                            let floor_y =
                                mesh_center_and_scale(current_mesh.as_ref().unwrap_or(mesh))
                                    .0
                                    .y;
                            model_point = snap_model_point_to_grid_layout(
                                model_point,
                                vec3(0.0, floor_y, 0.0),
                                state.grid_spacing,
                                false,
                            );
                        }
                    }
                    if let Some(center) = placement.center {
                        let dimensions =
                            dimensions_from_click(placement.primitive, center, model_point);
                        match add_primitive(catalog, state, placement.primitive, center, dimensions)
                        {
                            Ok(()) => state.primitive_placement = None,
                            Err(error) => state.error = Some(error),
                        }
                    } else if let Some(active) = state.primitive_placement.as_mut() {
                        active.center = Some(model_point);
                        active.input_focused = true;
                        active.input_selected = true;
                        state.error = None;
                    }
                } else {
                    state.error = Some("The click did not intersect the construction plane".into());
                }
            } else if state.trim_active {
                let extend = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                if state.trim_phase == TrimPhase::Targets {
                    if let Some(face) =
                        mesh_ray_hit_excluding(origin, direction, mesh, &blocked_faces)
                    {
                        if !extend {
                            state.trim_target_faces.clear();
                        }
                        toggle_index(&mut state.trim_target_faces, face);
                        state.selected_faces = state.trim_target_faces.clone();
                        state.selected_group = None;
                        state.error = None;
                    } else {
                        state.error = Some("Click a target face to split".into());
                    }
                } else if state.trim_path_active {
                    if let Some((face, point)) = current_mesh.as_ref().and_then(|source_mesh| {
                        trim_path_point_at_cursor(
                            &camera,
                            ctx.mouse,
                            mesh,
                            &state.trim_target_faces,
                            state.trim_path_face,
                            state.object_snaps,
                        )
                        .map(|(face, point)| (face, display_to_model_point(source_mesh, point)))
                    }) {
                        if state.trim_path_face.is_some_and(|active| active != face) {
                            state.error =
                                Some("Draw one multiline path on a single target face".into());
                        } else {
                            if state
                                .trim_path_points
                                .last()
                                .is_none_or(|last| last.distance_squared(point) > 1e-10)
                            {
                                state.trim_path_points.push(point);
                                state.trim_path_face = Some(face);
                            }
                            state.error = None;
                        }
                    } else {
                        state.error =
                            Some("Draw the multiline on one of the selected target faces".into());
                    }
                } else if let Some(face) =
                    mesh_ray_hit_excluding(origin, direction, mesh, &blocked_faces)
                {
                    if !extend {
                        state.trim_cutter_faces.clear();
                    }
                    toggle_index(&mut state.trim_cutter_faces, face);
                    state.selected_faces = state.trim_cutter_faces.clone();
                    state.selected_group = None;
                    state.error = None;
                } else {
                    state.error = Some("Click a mesh face to use as a cutter".into());
                }
            } else if state.mirror_active {
                if let Some(point) = mirror_cursor_point {
                    if state.mirror_points.len() < 3 {
                        state.mirror_points.push(point);
                        state.error = None;
                    }
                } else {
                    state.error =
                        Some("Click the mesh or construction plane to set the mirror plane".into());
                }
            } else if state.paint_active {
                if let Some(source_mesh) = current_mesh.as_ref() {
                    match apply_paint_pick(
                        catalog,
                        state,
                        &camera,
                        ctx.mouse,
                        source_mesh,
                        &blocked_faces,
                    ) {
                        Ok(()) => state.error = None,
                        Err(error) => state.error = Some(error),
                    }
                } else {
                    state.error = Some("Selected mesh is unavailable".to_string());
                }
            } else if ensure_selection_unlocked(catalog, state).is_ok()
                && let Some(source_mesh) = current_mesh.as_ref()
                && let Some((_, handle_size)) = gizmo_center_and_size(source_mesh, state)
                && let Some(drag) = begin_gizmo_drag(
                    &camera,
                    ctx.mouse,
                    source_mesh,
                    state,
                    state.transform_mode,
                    handle_size,
                )
            {
                state.gizmo_drag = Some(drag);
                state.gizmo_preview = None;
                state.gizmo_snap_target = None;
                state.gizmo_polar_angle = None;
                state.error = None;
            } else {
                let extend = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                match state.selection_mode {
                    SelectionMode::Vertex => {
                        if let Some(index) = pick_vertex(&camera, ctx.mouse, mesh, &blocked_faces) {
                            if extend {
                                if let Some(existing) = state
                                    .selected_vertices
                                    .iter()
                                    .position(|&selected| selected == index)
                                {
                                    state.selected_vertices.remove(existing);
                                } else {
                                    state.selected_vertices.push(index);
                                }
                            } else {
                                state.selected_vertices = vec![index];
                            }
                        }
                        state.selected_faces.clear();
                        state.selected_edges.clear();
                        state.selected_group = None;
                    }
                    SelectionMode::Edge => {
                        if let Some(edge) = pick_edge(&camera, ctx.mouse, mesh, &blocked_faces) {
                            select_edge(state, edge, extend);
                        } else if !extend {
                            state.selected_edges.clear();
                        }
                    }
                    SelectionMode::Face => {
                        if let Some(face) =
                            mesh_ray_hit_excluding(origin, direction, mesh, &blocked_faces)
                        {
                            select_face(state, face, extend);
                        } else if !extend {
                            state.selected_faces.clear();
                            state.selected_group = None;
                        }
                    }
                }
            }
            if !state.trim_active {
                state.error = None;
            }
        }
    }

    draw_rectangle(
        sidebar.x,
        sidebar.y,
        sidebar.w,
        sidebar.h,
        ctx.colors_rt.panel_bg,
    );
    draw_rectangle_lines(
        sidebar.x,
        sidebar.y,
        sidebar.w,
        sidebar.h,
        1.0,
        ctx.colors_rt.panel_border,
    );
    let title = selected_entry(catalog, state.selected)
        .map(|entry| entry.definition.label.to_string())
        .unwrap_or_else(|| "Object".to_string());
    draw_text(&title, 12.0, 36.0, ctx.font_lg, ctx.colors_rt.text_primary);
    if state.editing {
        draw_text(
            if state.selected == ObjectId::PlanetRoute {
                "Radial route program"
            } else {
                "Groups and faces"
            },
            12.0,
            140.0,
            ctx.font_md,
            ctx.colors_rt.text_secondary,
        );
        if let (Some(mesh), Some(metadata)) = (
            selected_mesh(catalog, state),
            selected_metadata(catalog, state),
        ) {
            let items = sidebar_items(metadata, mesh.face_count());
            let visible_rows = ((screen_height() - FACE_LIST_TOP - 46.0) / FACE_ROW_HEIGHT)
                .floor()
                .max(1.0) as usize;
            let add_group = Rect::new(12.0, 66.0, 108.0, 30.0);
            let rename = Rect::new(130.0, 66.0, 108.0, 30.0);
            let undo_button = Rect::new(12.0, 102.0, 108.0, 30.0);
            let redo_button = Rect::new(130.0, 102.0, 108.0, 30.0);
            let _ = button(ctx, add_group, "Add Group");
            let _ = button(ctx, rename, "Rename");
            let _ = button(ctx, undo_button, "Undo");
            let _ = button(ctx, redo_button, "Redo");
            for row in state.face_scroll..(state.face_scroll + visible_rows).min(items.len()) {
                let item = items[row];
                let y = FACE_LIST_TOP + (row - state.face_scroll) as f32 * FACE_ROW_HEIGHT;
                let (label, selected, depth, is_face, color) = match item {
                    SidebarItem::Group(group_id) => {
                        let group = metadata.group(group_id).expect("listed group must exist");
                        let face_count = metadata
                            .faces_for_group(group_id)
                            .map_or(group.faces.len(), |faces| faces.len());
                        let label = match group.program {
                            FaceGroupProgram::RadialRepeat => {
                                format!("{} [Radial] ({face_count})", group.name)
                            }
                            _ => format!("{} ({face_count})", group.name),
                        };
                        let dragging_over = state.dragged_faces.is_some()
                            && Rect::new(
                                12.0 + group_depth(metadata, group_id) as f32 * 10.0,
                                y,
                                SIDEBAR_WIDTH
                                    - 24.0
                                    - group_depth(metadata, group_id) as f32 * 10.0,
                                FACE_ROW_HEIGHT - 2.0,
                            )
                            .contains(ctx.mouse);
                        (
                            label,
                            state.selected_group == Some(group_id) || dragging_over,
                            group_depth(metadata, group_id),
                            false,
                            Color::from_rgba(55, 91, 156, 255),
                        )
                    }
                    SidebarItem::Face { index, group } => {
                        let name = metadata
                            .face_names
                            .get(index)
                            .and_then(Option::as_deref)
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("Face {}", index + 1));
                        (
                            name,
                            state.selected_faces.contains(&index),
                            group
                                .map(|id| group_depth(metadata, id))
                                .unwrap_or_default(),
                            group.is_some(),
                            Color::from_rgba(27, 119, 105, 255),
                        )
                    }
                };
                let indent = depth as f32 * 10.0 + if is_face && depth > 0 { 10.0 } else { 0.0 };
                let is_group = matches!(item, SidebarItem::Group(_));
                let configure_width = if is_group { 100.0 } else { 0.0 };
                let rect = Rect::new(
                    12.0 + indent,
                    y,
                    SIDEBAR_WIDTH - 24.0 - indent - configure_width,
                    FACE_ROW_HEIGHT - 2.0,
                );
                draw_list_row(ctx, rect, &label, selected, color);
                if is_group {
                    if let SidebarItem::Group(group_id) = item {
                        let configure =
                            Rect::new(rect.x + rect.w + 4.0, y, 34.0, FACE_ROW_HEIGHT - 2.0);
                        let lock = Rect::new(configure.x + 36.0, y, 28.0, FACE_ROW_HEIGHT - 2.0);
                        let hide = Rect::new(configure.x + 66.0, y, 28.0, FACE_ROW_HEIGHT - 2.0);
                        draw_group_action_button(ctx, configure, "Set", false);
                        let nonempty = metadata
                            .faces_for_group(group_id)
                            .is_some_and(|faces| !faces.is_empty());
                        if nonempty {
                            draw_group_action_button(
                                ctx,
                                lock,
                                "L",
                                metadata.group_is_locked(group_id),
                            );
                            draw_group_action_button(
                                ctx,
                                hide,
                                "H",
                                metadata.group_is_hidden(group_id),
                            );
                        }
                    }
                }
            }
        }
    } else {
        let mut y = 68.0;
        for category in [
            ObjectCategory::Buildings,
            ObjectCategory::Units,
            ObjectCategory::MachineBlocks,
        ] {
            draw_list_row(
                ctx,
                Rect::new(12.0, y, SIDEBAR_WIDTH - 24.0, 34.0),
                category.label(),
                state.category == category,
                Color::from_rgba(55, 91, 156, 255),
            );
            y += 40.0;
        }
        y += 8.0;
        for entry in catalog
            .entries()
            .iter()
            .filter(|entry| entry.definition.category == state.category)
        {
            draw_list_row(
                ctx,
                Rect::new(12.0, y, SIDEBAR_WIDTH - 24.0, 32.0),
                entry.definition.label,
                state.selected == entry.definition.id,
                Color::from_rgba(27, 119, 105, 255),
            );
            y += 36.0;
        }
    }
    let metadata = selected_metadata(catalog, state);
    draw_view_controls(
        ctx,
        state,
        view_panel,
        metadata.is_some_and(|metadata| metadata.radial_main_group_id().is_some()),
        metadata.and_then(|metadata| metadata.radial_pivot),
    );
    draw_object_snap_panel(ctx, view_panel, state);
    draw_paint_panel(ctx, paint_panel, state);

    draw_group_config_dialog(ctx, catalog, state);
    draw_numeric_transform_panel(ctx, catalog, state);
    if operation_panel_open {
        let panel = operation_value_panel_rect(viewport);
        draw_rectangle(panel.x, panel.y, panel.w, panel.h, ctx.colors_rt.panel_bg);
        draw_rectangle_lines(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            1.0,
            ctx.colors_rt.panel_border,
        );
        draw_text(
            if state.fillet_active {
                "Fillet radius (mesh units)"
            } else {
                "Offset distance (mesh units)"
            },
            panel.x + 10.0,
            panel.y + 21.0,
            ctx.font_sm,
            ctx.colors_rt.text_primary,
        );
        let input = operation_value_input_rect(viewport);
        draw_rectangle(
            input.x,
            input.y,
            input.w,
            input.h,
            ctx.colors_rt.button_base,
        );
        draw_rectangle_lines(
            input.x,
            input.y,
            input.w,
            input.h,
            1.0,
            if state.operation_value_focused {
                ctx.colors_rt.text_primary
            } else {
                ctx.colors_rt.button_border
            },
        );
        draw_text(
            &state.operation_value,
            input.x + 8.0,
            input.y + input.h * 0.5 + ctx.font_sm * 0.35,
            ctx.font_sm,
            ctx.colors_rt.text_primary,
        );
        let apply_rect = Rect::new(panel.x + panel.w - 88.0, panel.y + 64.0, 78.0, 24.0);
        if button(ctx, apply_rect, "Apply") {
            let result = if state.fillet_active {
                apply_fillet(catalog, state)
            } else {
                apply_offset(catalog, state)
            };
            match result {
                Ok(()) => {
                    state.fillet_active = false;
                    state.offset_active = false;
                    state.operation_value_focused = false;
                }
                Err(error) => state.error = Some(error),
            }
        }
    }

    if let Some(dialog) = &state.rename_dialog {
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::from_rgba(0, 0, 0, 190),
        );
        let modal = Rect::new(
            (screen_width() - 420.0) * 0.5,
            (screen_height() - 190.0) * 0.5,
            420.0,
            190.0,
        );
        draw_rectangle(modal.x, modal.y, modal.w, modal.h, ctx.colors_rt.panel_bg);
        draw_rectangle_lines(
            modal.x,
            modal.y,
            modal.w,
            modal.h,
            1.5,
            ctx.colors_rt.panel_border,
        );
        draw_text(
            "Rename face or group",
            modal.x + 22.0,
            modal.y + 42.0,
            ctx.font_md,
            ctx.colors_rt.text_primary,
        );
        let input = Rect::new(modal.x + 20.0, modal.y + 58.0, modal.w - 40.0, 36.0);
        draw_rectangle(
            input.x,
            input.y,
            input.w,
            input.h,
            ctx.colors_rt.button_base,
        );
        draw_rectangle_lines(
            input.x,
            input.y,
            input.w,
            input.h,
            1.0,
            ctx.colors_rt.button_border,
        );
        draw_text(
            &dialog.text,
            input.x + 10.0,
            input.y + input.h * 0.5 + ctx.font_sm * 0.35,
            ctx.font_sm,
            WHITE,
        );
        let button_y = modal.y + 120.0;
        let apply_clicked = button(
            ctx,
            Rect::new(modal.x + 86.0, button_y, 112.0, 36.0),
            "Apply",
        );
        let cancel_clicked = button(
            ctx,
            Rect::new(modal.x + 222.0, button_y, 112.0, 36.0),
            "Cancel",
        );
        if apply_clicked {
            match apply_rename_dialog(catalog, state) {
                Ok(()) => state.error = None,
                Err(error) => state.error = Some(error),
            }
        } else if cancel_clicked {
            state.rename_dialog = None;
        }
        if let Some(error) = &state.error {
            draw_text(
                error,
                modal.x + 20.0,
                modal.y + modal.h - 8.0,
                ctx.font_sm,
                Color::from_rgba(255, 110, 110, 255),
            );
        }
    }
    if state.exit_confirmation_open {
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            Color::from_rgba(0, 0, 0, 190),
        );
        let modal = Rect::new(
            (screen_width() - 390.0) * 0.5,
            (screen_height() - 184.0) * 0.5,
            390.0,
            184.0,
        );
        draw_rectangle(modal.x, modal.y, modal.w, modal.h, ctx.colors_rt.panel_bg);
        draw_rectangle_lines(
            modal.x,
            modal.y,
            modal.w,
            modal.h,
            1.5,
            ctx.colors_rt.panel_border,
        );
        draw_text(
            "Save changes before closing the editor?",
            modal.x + 24.0,
            modal.y + 48.0,
            ctx.font_md,
            ctx.colors_rt.text_primary,
        );
        let button_y = modal.y + 96.0;
        let save_clicked = button(
            ctx,
            Rect::new(modal.x + 20.0, button_y, 106.0, 38.0),
            "Save",
        );
        let discard_clicked = button(
            ctx,
            Rect::new(modal.x + 142.0, button_y, 106.0, 38.0),
            "Discard",
        );
        let cancel_clicked = button(
            ctx,
            Rect::new(modal.x + 264.0, button_y, 106.0, 38.0),
            "Cancel",
        );
        if save_clicked {
            match save_selected_mesh(catalog, state) {
                Ok(()) => {
                    state.error = None;
                    state.exit_confirmation_open = false;
                    close_editor(state);
                }
                Err(error) => state.error = Some(error),
            }
        } else if discard_clicked {
            match restore_original_mesh(catalog, state) {
                Ok(()) => {
                    state.error = None;
                    state.exit_confirmation_open = false;
                    *scene = Scene::MainMenu;
                }
                Err(error) => state.error = Some(error),
            }
        } else if cancel_clicked {
            state.exit_confirmation_open = false;
        }
        if let Some(error) = &state.error {
            draw_text(
                error,
                modal.x + 20.0,
                modal.y + modal.h - 12.0,
                ctx.font_sm,
                Color::from_rgba(255, 110, 110, 255),
            );
        }
    }
    if let Some(placement) = state.primitive_placement.as_ref() {
        if placement.center.is_some() {
            let panel = placement_panel_rect(viewport);
            draw_rectangle(panel.x, panel.y, panel.w, panel.h, ctx.colors_rt.panel_bg);
            draw_rectangle_lines(
                panel.x,
                panel.y,
                panel.w,
                panel.h,
                1.0,
                ctx.colors_rt.panel_border,
            );
            let name = match placement.primitive {
                MeshPrimitive::Cube => "Cube",
                MeshPrimitive::Plane => "Plane",
                MeshPrimitive::Sphere => "Sphere",
                MeshPrimitive::Cylinder => "Cylinder",
            };
            draw_text(
                "Click viewport for size, or type values",
                panel.x + 12.0,
                panel.y + 20.0,
                ctx.font_sm,
                ctx.colors_rt.text_primary,
            );
            draw_text(
                &format!(
                    "{name}: {} active (model units)",
                    placement_dimension_label(placement)
                ),
                panel.x + 12.0,
                panel.y + 42.0,
                ctx.font_sm,
                ctx.colors_rt.text_secondary,
            );
            draw_text(
                "Up/Down: +/-0.1; Shift: +/-0.01",
                panel.x + 12.0,
                panel.y + 62.0,
                ctx.font_sm,
                ctx.colors_rt.text_secondary,
            );
            draw_text(
                "Left/Right: select dimension",
                panel.x + 12.0,
                panel.y + 80.0,
                ctx.font_sm,
                ctx.colors_rt.text_secondary,
            );
            let input = placement_input_rect(viewport);
            draw_rectangle(
                input.x,
                input.y,
                input.w,
                input.h,
                if placement.input_selected {
                    Color::from_rgba(55, 91, 156, 255)
                } else {
                    ctx.colors_rt.button_base
                },
            );
            draw_rectangle_lines(
                input.x,
                input.y,
                input.w,
                input.h,
                1.0,
                ctx.colors_rt.button_border,
            );
            draw_text(
                &placement.dimensions,
                input.x + 8.0,
                input.y + input.h * 0.5 + ctx.font_sm * 0.35,
                ctx.font_sm,
                WHITE,
            );
            let _ = button(ctx, placement_create_rect(viewport), "Create");
            let _ = button(ctx, placement_cancel_rect(viewport), "Cancel");
        }
    }
    draw_status_bar(ctx, catalog, state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_hex_accepts_rgb_and_rgba() {
        let rgb = parse_paint_hex("#12ab34").unwrap();
        assert_eq!((rgb.r * 255.0).round() as u8, 18);
        assert_eq!((rgb.g * 255.0).round() as u8, 171);
        assert_eq!((rgb.b * 255.0).round() as u8, 52);
        assert_eq!((rgb.a * 255.0).round() as u8, 255);

        let rgba = parse_paint_hex("12ab34cd").unwrap();
        assert_eq!((rgba.a * 255.0).round() as u8, 205);
        assert!(parse_paint_hex("xyz").is_err());
    }

    #[test]
    fn status_bar_reports_trim_instructions_and_prioritizes_errors() {
        let mut state = CadSceneState {
            trim_active: true,
            trim_phase: TrimPhase::Cutters,
            trim_path_active: true,
            ..CadSceneState::default()
        };
        let (message, is_error) = status_bar_message(&state, None);
        assert!(message.contains("Trim path"));
        assert!(message.contains("snaps are active"));
        assert!(!is_error);

        state.error = Some("Trim failed".to_string());
        assert_eq!(
            status_bar_message(&state, None),
            ("Trim failed".to_string(), true)
        );
    }

    #[test]
    fn wheel_zoom_uses_small_relative_steps_and_respects_limits() {
        let zoomed_in = zoom_distance(10.0, 1.0);
        assert!(zoomed_in < 10.0);
        assert!(zoomed_in > 9.0);
        assert_eq!(
            zoom_distance(CAMERA_MIN_DISTANCE, 100.0),
            CAMERA_MIN_DISTANCE
        );
        assert_eq!(
            zoom_distance(CAMERA_MAX_DISTANCE, -100.0),
            CAMERA_MAX_DISTANCE
        );
    }

    #[test]
    fn placement_dimensions_parse_local_primitive_measurements() {
        assert_eq!(
            parse_primitive_dimensions(MeshPrimitive::Cube, "2.5").unwrap(),
            vec3(2.5, 2.5, 2.5)
        );
        assert_eq!(
            parse_primitive_dimensions(MeshPrimitive::Plane, "3, 4").unwrap(),
            vec3(3.0, 1.0, 4.0)
        );
        assert_eq!(
            parse_primitive_dimensions(MeshPrimitive::Sphere, "0.75").unwrap(),
            vec3(1.5, 1.5, 1.5)
        );
        assert_eq!(
            parse_primitive_dimensions(MeshPrimitive::Cylinder, "0.5, 2").unwrap(),
            vec3(1.0, 2.0, 1.0)
        );
        assert!(parse_primitive_dimensions(MeshPrimitive::Cylinder, "0.5").is_err());
        assert!(parse_primitive_dimensions(MeshPrimitive::Cube, "NaN").is_err());
    }

    #[test]
    fn arrow_adjustment_uses_requested_steps_and_selects_multi_value_fields() {
        let mut placement = PrimitivePlacement {
            primitive: MeshPrimitive::Cylinder,
            center: Some(Vec3::ZERO),
            dimensions: "0.5, 1".to_string(),
            input_focused: true,
            input_selected: false,
            dimension_index: 0,
        };
        adjust_placement_dimension(&mut placement, 0.1).unwrap();
        assert_eq!(placement.dimensions, "0.6, 1");
        adjust_placement_dimension(&mut placement, -0.01).unwrap();
        assert_eq!(placement.dimensions, "0.59, 1");
        select_placement_dimension(&mut placement, 1);
        assert_eq!(placement_dimension_label(&placement), "height");
        adjust_placement_dimension(&mut placement, 0.01).unwrap();
        assert_eq!(placement.dimensions, "0.59, 1.01");
        select_placement_dimension(&mut placement, -1);
        assert_eq!(placement_dimension_label(&placement), "radius");
    }

    #[test]
    fn grid_snapping_and_construction_plane_intersection_are_stable() {
        assert_eq!(
            snap_model_point(vec3(0.26, 3.0, -0.74), 0.5),
            vec3(0.5, 3.0, -0.5)
        );
        let mesh = EditableMesh::primitive(MeshPrimitive::Cube)
            .transformed(|point| point + vec3(3.0, 2.0, -4.0));
        let model_point = vec3(3.25, 2.4, -3.5);
        let displayed = model_to_display_point(&mesh, model_point);
        assert!((display_to_model_point(&mesh, displayed) - model_point).length() < 1e-5);
        assert_eq!(
            construction_plane_hit(vec3(0.0, 2.0, 0.0), vec3(0.0, -1.0, 0.0)),
            Some(Vec3::ZERO)
        );
        assert_eq!(construction_plane_hit(vec3(0.0, 2.0, 0.0), Vec3::X), None);
    }

    #[test]
    fn object_snap_modes_are_independent_and_enabled_by_default() {
        let mut settings = ObjectSnapSettings::default();
        for kind in [
            ObjectSnapKind::Center,
            ObjectSnapKind::Tangent,
            ObjectSnapKind::Quadrant,
            ObjectSnapKind::Intersection,
            ObjectSnapKind::Endpoint,
            ObjectSnapKind::Midpoint,
            ObjectSnapKind::Perpendicular,
        ] {
            assert!(settings.enabled(kind));
        }

        settings.set(ObjectSnapKind::Endpoint, false);
        assert!(!settings.enabled(ObjectSnapKind::Endpoint));
        assert!(settings.enabled(ObjectSnapKind::Midpoint));
        assert!(settings.enabled(ObjectSnapKind::Center));
    }

    #[test]
    fn polar_tracking_uses_nearest_fifteen_degree_direction() {
        let (tracked, angle) = polar_track_delta(vec3(1.0, 0.0, 1.0), Vec3::X, Vec3::Z, 15.0);
        assert!((angle - 45.0).abs() < 1e-4);
        assert!((tracked - vec3(1.0, 0.0, 1.0)).length() < 1e-4);

        let (_, angle) = polar_track_delta(vec3(1.0, 0.0, 0.4), Vec3::X, Vec3::Z, 15.0);
        assert!((angle - 15.0).abs() < 1e-4);
    }

    #[test]
    fn grid_snapping_respects_transform_axis_constraints() {
        assert_eq!(
            snap_transform_to_grid(
                vec3(0.2, 0.4, 0.3),
                vec3(0.5, 0.0, 0.4),
                TransformAxis::All,
                1.0
            ),
            vec3(1.0, 0.4, 1.0)
        );
        let x_snapped = snap_transform_to_grid(
            vec3(0.2, 0.4, 0.3),
            vec3(0.5, 0.0, 0.4),
            TransformAxis::X,
            1.0,
        );
        assert!((x_snapped - vec3(1.0, 0.4, 0.7)).length() < 1e-5);
    }

    #[test]
    fn recognized_circular_loops_fit_and_non_circular_loops_are_rejected() {
        let points = (0..12)
            .map(|step| {
                let angle = std::f32::consts::TAU * step as f32 / 12.0;
                vec3(angle.cos() * 2.0, 0.0, angle.sin() * 2.0)
            })
            .collect::<Vec<_>>();
        let circle = fit_circular_loop(&points).unwrap();
        assert!(circle.center.length() < 1e-5);
        assert!((circle.radius - 2.0).abs() < 1e-5);
        let snap_points = circular_loop_snap_points(&circle, Some(vec3(4.0, 0.0, 0.0)));
        assert_eq!(
            snap_points
                .iter()
                .filter(|candidate| candidate.kind == ObjectSnapKind::Tangent)
                .count(),
            2
        );
        assert_eq!(
            snap_points
                .iter()
                .filter(|candidate| candidate.kind == ObjectSnapKind::Quadrant)
                .count(),
            4
        );
        assert_eq!(
            snap_points
                .iter()
                .find(|candidate| candidate.kind == ObjectSnapKind::Center)
                .unwrap()
                .position,
            circle.center
        );
        assert!(
            fit_circular_loop(&[
                vec3(-1.0, 0.0, -1.0),
                vec3(1.0, 0.0, -1.0),
                vec3(1.0, 0.0, 1.0),
                vec3(-1.0, 0.0, 1.0),
            ])
            .is_none()
        );
    }

    #[test]
    fn circular_snap_detection_requires_a_closed_mesh_edge_loop() {
        let mut vertices = vec![Vertex::new2(Vec3::ZERO, Vec2::ZERO, WHITE)];
        for step in 0..8 {
            let angle = std::f32::consts::TAU * step as f32 / 8.0;
            vertices.push(Vertex::new2(
                vec3(angle.cos(), 0.0, angle.sin()),
                Vec2::ZERO,
                WHITE,
            ));
        }
        let mut indices = Vec::new();
        for step in 0..8 {
            indices.extend_from_slice(&[0, (step + 1) as u32, ((step + 1) % 8 + 1) as u32]);
        }
        let mesh = EditableMesh { vertices, indices };
        assert_eq!(circular_edge_loops(&mesh).len(), 1);
        let triangle = EditableMesh {
            vertices: vec![
                Vertex::new2(Vec3::ZERO, Vec2::ZERO, WHITE),
                Vertex::new2(Vec3::X, Vec2::ZERO, WHITE),
                Vertex::new2(Vec3::Y, Vec2::ZERO, WHITE),
            ],
            indices: vec![0, 1, 2],
        };
        assert!(circular_edge_loops(&triangle).is_empty());
    }

    #[test]
    fn circular_snap_detection_recognizes_capped_cylinder_rims() {
        let segments = 8;
        let mut vertices = Vec::new();
        for y in [1.0, -1.0] {
            for step in 0..segments {
                let angle = std::f32::consts::TAU * step as f32 / segments as f32;
                vertices.push(Vertex::new2(
                    vec3(angle.cos(), y, angle.sin()),
                    Vec2::ZERO,
                    WHITE,
                ));
            }
        }
        vertices.push(Vertex::new2(Vec3::Y, Vec2::ZERO, WHITE));
        vertices.push(Vertex::new2(-Vec3::Y, Vec2::ZERO, WHITE));
        let mut indices = Vec::new();
        for step in 0..segments {
            let next = (step + 1) % segments;
            let top = step as u32;
            let top_next = next as u32;
            let bottom = (step + segments) as u32;
            let bottom_next = (next + segments) as u32;
            indices.extend_from_slice(&[top, bottom, bottom_next, top, bottom_next, top_next]);
            indices.extend_from_slice(&[16, top_next, top, 17, bottom, bottom_next]);
        }
        let mesh = EditableMesh { vertices, indices };
        let circles = circular_edge_loops(&mesh);
        assert_eq!(circles.len(), 2);
        assert!(
            circles
                .iter()
                .all(|circle| (circle.radius - 1.0).abs() < 1e-5)
        );
    }

    #[test]
    fn projected_intersection_snap_ignores_parallel_and_endpoint_contacts() {
        let crossing = projected_segment_intersection(
            vec2(-1.0, 0.0),
            vec2(1.0, 0.0),
            vec2(0.0, -1.0),
            vec2(0.0, 1.0),
        )
        .unwrap();
        assert!((crossing.0 - 0.5).abs() < 1e-5);
        assert!((crossing.1 - 0.5).abs() < 1e-5);
        assert_eq!(crossing.2, Vec2::ZERO);
        assert!(
            projected_segment_intersection(Vec2::ZERO, Vec2::X, vec2(0.0, 1.0), vec2(1.0, 1.0))
                .is_none()
        );
        assert_eq!(
            dimensions_from_click(MeshPrimitive::Sphere, Vec3::ZERO, Vec3::X),
            vec3(2.0, 2.0, 2.0)
        );
    }

    #[test]
    fn trim_snap_projects_nearby_cursor_positions_to_face_edges() {
        let (amount, distance) =
            project_screen_point_to_segment(vec2(5.0, 3.0), vec2(0.0, 0.0), vec2(10.0, 0.0))
                .unwrap();
        assert!((amount - 0.5).abs() < 1e-6);
        assert!((distance - 3.0).abs() < 1e-6);

        let (amount, distance) =
            project_screen_point_to_segment(vec2(-2.0, 1.0), vec2(0.0, 0.0), vec2(10.0, 0.0))
                .unwrap();
        assert_eq!(amount, 0.0);
        assert!((distance - 5.0_f32.sqrt()).abs() < 1e-6);
        assert!(project_screen_point_to_segment(Vec2::ZERO, Vec2::ZERO, Vec2::ZERO).is_none());
    }

    #[test]
    fn triangular_grid_snapping_is_centered_on_the_radial_pivot() {
        let center = vec3(2.0, 4.0, -3.0);
        let snapped = snap_model_point_to_triangular_grid(vec3(2.74, 5.0, -2.2), center, 1.0);

        assert!((snapped - vec3(2.5, 5.0, -3.0 + 3.0_f32.sqrt() * 0.5)).length() < 1e-5);
    }

    #[test]
    fn grid_snapping_pulls_nearby_points_to_square_and_triangular_floors() {
        let floor = vec3(0.0, 2.0, 0.0);
        let near_floor = vec3(0.26, 2.2, 0.24);
        assert_eq!(
            snap_model_point_to_grid_layout(near_floor, floor, 1.0, false),
            vec3(0.0, 2.0, 0.0)
        );
        assert_eq!(
            snap_model_point_to_grid_layout(near_floor, floor, 1.0, true),
            vec3(0.0, 2.0, 0.0)
        );

        let above_threshold = vec3(0.26, 2.26, 0.24);
        assert_eq!(
            snap_model_point_to_grid_layout(above_threshold, floor, 1.0, false).y,
            above_threshold.y
        );
        assert_eq!(
            snap_model_point_to_grid_layout(above_threshold, floor, 1.0, true).y,
            above_threshold.y
        );
    }

    #[test]
    fn radial_gizmo_snapping_uses_the_triangular_grid_and_preserves_axis_constraints() {
        let center = vec3(2.0, 4.0, -3.0);
        let pivot = vec3(2.74, 5.0, -2.2);
        let expected = vec3(2.5, 5.0, -3.0 + 3.0_f32.sqrt() * 0.5);

        assert!(
            (snap_transform_to_triangular_grid(pivot, Vec3::ZERO, TransformAxis::All, 1.0, center)
                - expected)
                .length()
                < 1e-5
        );
        assert_eq!(
            snap_transform_to_triangular_grid(pivot, Vec3::ZERO, TransformAxis::X, 1.0, center),
            vec3(expected.x, pivot.y, pivot.z)
        );
        assert_eq!(
            snap_transform_to_triangular_grid(pivot, Vec3::ZERO, TransformAxis::Y, 1.0, center),
            pivot
        );
    }

    #[test]
    fn radial_ucs_and_grid_are_limited_to_a_sixty_degree_wedge() {
        let origin = vec3(2.0, 1.0, -4.0);
        let x_axis = Vec3::X;
        let z_axis = vec3(0.5, 0.0, 3.0_f32.sqrt() * 0.5);
        assert!((x_axis.dot(z_axis) - 0.5).abs() < 1e-6);

        let segments = triangular_grid_segments(origin, 1.0, 4.0);
        assert_eq!(segments.len(), 12);
        for (start, end) in segments {
            for point in [start, end] {
                let relative = point - origin;
                let r = 2.0 * relative.z / 3.0_f32.sqrt();
                let q = relative.x - r * 0.5;
                assert!(q >= -1e-5);
                assert!(r >= -1e-5);
                assert!(q + r <= 4.0 + 1e-5);
            }
        }
    }

    #[test]
    fn radial_rotation_uses_the_ucs_origin_as_its_pivot() {
        let mesh = EditableMesh {
            vertices: vec![
                Vertex::new2(Vec3::X, Vec2::ZERO, WHITE),
                Vertex::new2(vec3(2.0, 0.0, 0.0), Vec2::ZERO, WHITE),
            ],
            indices: vec![],
        };

        let rotated = rotate_mesh_y_about(&mesh, std::f32::consts::FRAC_PI_2, Vec3::X);

        assert!((rotated.vertices[0].position - Vec3::X).length() < 1e-5);
        assert!((rotated.vertices[1].position - vec3(1.0, 0.0, 1.0)).length() < 1e-5);
    }

    #[test]
    fn interaction_preview_randomizes_only_configured_neighbor_conditions() {
        let mut metadata = FaceMetadata::for_face_count(3);
        let main = metadata.create_group(&[0]).unwrap();
        metadata
            .set_group_program(main, FaceGroupProgram::RadialRepeat)
            .unwrap();
        let empty = metadata.create_group_under(&[1], Some(main)).unwrap();
        metadata
            .set_group_program(
                empty,
                FaceGroupProgram::IfNeighbor(RadialNeighborState::Empty),
            )
            .unwrap();
        let route = metadata.create_group_under(&[2], Some(main)).unwrap();
        metadata
            .set_group_program(
                route,
                FaceGroupProgram::IfNeighbor(RadialNeighborState::Route),
            )
            .unwrap();

        for seed in 1..16 {
            assert!(
                random_preview_neighbors(&metadata, seed)
                    .iter()
                    .all(|state| matches!(
                        state,
                        RadialNeighborState::Empty | RadialNeighborState::Route
                    ))
            );
        }
    }

    #[test]
    fn interaction_preview_neighbor_buildings_exclude_machine_blocks() {
        let catalog = ObjectCatalog::load();
        let selected = ObjectId::PlanetBase;
        for neighbor_index in 0..6 {
            let neighbor = preview_neighbor_object(&catalog, selected, 1, neighbor_index)
                .expect("catalog should contain another building");
            let entry = catalog
                .entries()
                .iter()
                .find(|entry| entry.definition.id == neighbor)
                .unwrap();
            assert_eq!(entry.definition.category, ObjectCategory::Buildings);
        }
    }

    #[test]
    fn interaction_preview_center_scale_uses_all_configured_geometry_bounds() {
        let color = Color::from_rgba(255, 255, 255, 255);
        let reference = EditableMesh {
            vertices: vec![
                Vertex::new2(vec3(-1.0, 0.0, -1.0), Vec2::ZERO, color),
                Vertex::new2(vec3(1.0, 0.0, -1.0), Vec2::ZERO, color),
                Vertex::new2(vec3(0.0, 1.0, 1.0), Vec2::ZERO, color),
            ],
            indices: vec![0, 1, 2],
        };
        let small_variant = reference.clone();
        let mut large_variant = reference.clone();
        large_variant
            .append(&EditableMesh {
                vertices: vec![
                    Vertex::new2(vec3(8.0, 0.0, 0.0), Vec2::ZERO, color),
                    Vertex::new2(vec3(9.0, 0.0, 0.0), Vec2::ZERO, color),
                    Vertex::new2(vec3(8.0, 1.0, 0.0), Vec2::ZERO, color),
                ],
                indices: vec![0, 1, 2],
            })
            .unwrap();

        let small_preview = preview_mesh_with_reference(&small_variant, &reference);
        let large_preview = preview_mesh_with_reference(&large_variant, &reference);
        assert_eq!(
            small_preview.vertices[0].position,
            large_preview.vertices[0].position
        );
        assert_eq!(
            small_preview.vertices[1].position,
            large_preview.vertices[1].position
        );
    }

    #[test]
    fn pending_primitive_is_added_only_after_explicit_confirmation() {
        let mut catalog = ObjectCatalog::load();
        let mut state = CadSceneState::default();
        let original_faces = catalog.get(state.selected).unwrap().face_count();
        start_primitive_placement(&mut state, MeshPrimitive::Cube);
        assert_eq!(
            catalog.get(state.selected).unwrap().face_count(),
            original_faces
        );
        confirm_primitive_placement(&mut catalog, &mut state);
        assert_eq!(
            catalog.get(state.selected).unwrap().face_count(),
            original_faces
        );

        let placement = state.primitive_placement.as_mut().unwrap();
        placement.center = Some(vec3(2.0, 0.0, -1.0));
        placement.dimensions = "2".to_string();
        confirm_primitive_placement(&mut catalog, &mut state);

        assert!(state.primitive_placement.is_none());
        assert_eq!(
            catalog.get(state.selected).unwrap().face_count(),
            original_faces + 12
        );
        let inserted_faces = (original_faces..original_faces + 12).collect::<Vec<_>>();
        let primitive_group = catalog
            .metadata(state.selected, None)
            .unwrap()
            .groups
            .iter()
            .find(|group| group.faces == inserted_faces)
            .unwrap();
        assert_eq!(primitive_group.name, "Cube");
        assert!(state.dirty);
        assert!(undo(&mut catalog, &mut state).unwrap());
        assert_eq!(
            catalog.get(state.selected).unwrap().face_count(),
            original_faces
        );
        assert!(
            catalog
                .metadata(state.selected, None)
                .unwrap()
                .groups
                .iter()
                .all(|group| group.faces != inserted_faces)
        );
    }

    #[test]
    fn interaction_preview_composes_the_neighbor_branch_for_each_sector() {
        let color = Color::from_rgba(255, 255, 255, 255);
        let mut vertices = Vec::new();
        for offset in [0.0, 2.0, 4.0] {
            vertices.extend([
                Vertex::new2(vec3(offset, 0.0, 0.0), Vec2::ZERO, color),
                Vertex::new2(vec3(offset + 1.0, 0.0, 0.0), Vec2::ZERO, color),
                Vertex::new2(vec3(offset, 1.0, 0.0), Vec2::ZERO, color),
            ]);
        }
        let mesh = EditableMesh {
            vertices,
            indices: vec![0, 1, 2, 3, 4, 5, 6, 7, 8],
        };
        let mut metadata = FaceMetadata::for_face_count(3);
        let main = metadata.create_group(&[0]).unwrap();
        metadata
            .set_group_program(main, FaceGroupProgram::RadialRepeat)
            .unwrap();
        let empty = metadata.create_group_under(&[1], Some(main)).unwrap();
        metadata
            .set_group_program(
                empty,
                FaceGroupProgram::IfNeighbor(RadialNeighborState::Empty),
            )
            .unwrap();
        let route = metadata.create_group_under(&[2], Some(main)).unwrap();
        metadata
            .set_group_program(
                route,
                FaceGroupProgram::IfNeighbor(RadialNeighborState::Route),
            )
            .unwrap();

        let preview = compose_conditioned_preview(
            &mesh,
            &metadata,
            &[
                RadialNeighborState::Empty,
                RadialNeighborState::Route,
                RadialNeighborState::OtherBuilding,
                RadialNeighborState::Empty,
                RadialNeighborState::Route,
                RadialNeighborState::OtherBuilding,
            ],
            Vec3::ZERO,
        )
        .unwrap();
        assert_eq!(preview.face_count(), 10);
    }

    #[test]
    fn route_neighbor_preview_faces_back_toward_the_selected_cell() {
        let color = Color::from_rgba(255, 255, 255, 255);
        let mesh = EditableMesh {
            vertices: vec![
                Vertex::new2(-Vec3::X, Vec2::ZERO, color),
                Vertex::new2(Vec3::X, Vec2::ZERO, color),
                Vertex::new2(Vec3::ZERO, Vec2::ZERO, color),
                Vertex::new2(Vec3::ZERO, Vec2::ZERO, color),
                Vertex::new2(Vec3::X, Vec2::ZERO, color),
                Vertex::new2(vec3(0.5, 0.0, 3.0_f32.sqrt() * 0.5), Vec2::ZERO, color),
            ],
            indices: vec![0, 1, 2, 3, 4, 5],
        };
        let mut metadata = FaceMetadata::for_face_count(2);
        let main = metadata.create_group(&[0]).unwrap();
        metadata
            .set_group_program(main, FaceGroupProgram::RadialRepeat)
            .unwrap();
        let route = metadata.create_group_under(&[1], Some(main)).unwrap();
        metadata
            .set_group_program(
                route,
                FaceGroupProgram::IfNeighbor(RadialNeighborState::Route),
            )
            .unwrap();

        let preview = route_neighbor_preview_mesh(&mesh, &metadata, 0)
            .unwrap()
            .unwrap();
        let centroid = preview
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .sum::<Vec3>()
            / preview.vertices.len() as f32;
        let expected = 7.0 * std::f32::consts::PI / 6.0;

        assert!((centroid.normalize() - vec3(expected.cos(), 0.0, expected.sin())).length() < 1e-5);
    }

    #[test]
    fn radial_route_groups_appear_as_a_main_group_with_condition_children() {
        let mut metadata = FaceMetadata::for_face_count(3);
        let main_id = metadata.create_group(&[0]).unwrap();
        assert_eq!(
            metadata.group(main_id).unwrap().program,
            FaceGroupProgram::Geometry
        );
        metadata
            .set_group_program(main_id, FaceGroupProgram::RadialRepeat)
            .unwrap();
        let route_group = metadata.create_group_under(&[1], Some(main_id)).unwrap();
        metadata
            .set_group_program(
                route_group,
                FaceGroupProgram::IfNeighbor(RadialNeighborState::Route),
            )
            .unwrap();

        let items = sidebar_items(&metadata, 3);
        assert_eq!(items.first(), Some(&SidebarItem::Group(main_id)));
        assert_eq!(
            items,
            [
                SidebarItem::Group(main_id),
                SidebarItem::Face {
                    index: 0,
                    group: Some(main_id),
                },
                SidebarItem::Group(route_group),
                SidebarItem::Face {
                    index: 1,
                    group: Some(route_group),
                },
                SidebarItem::Face {
                    index: 2,
                    group: None,
                },
            ]
        );
    }

    #[test]
    fn discarding_dirty_mesh_restores_original_and_leaves_edit_mode() {
        let mut catalog = ObjectCatalog::load();
        let original = catalog.get(ObjectId::PlanetBase).unwrap().clone();
        let original_metadata = catalog
            .metadata(ObjectId::PlanetBase, None)
            .unwrap()
            .clone();
        catalog.get_mut(ObjectId::PlanetBase).unwrap().vertices[0].position += Vec3::X;
        let mut state = CadSceneState {
            editing: true,
            dirty: true,
            original_mesh: Some(original.clone()),
            original_metadata: Some(original_metadata),
            ..CadSceneState::default()
        };

        restore_original_mesh(&mut catalog, &mut state).unwrap();

        assert_eq!(
            catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position,
            original.vertices[0].position
        );
        assert!(!state.editing);
        assert!(!state.dirty);
    }

    #[test]
    fn closing_clean_editor_keeps_mesh_and_clears_edit_state() {
        let mut catalog = ObjectCatalog::load();
        let position = {
            let mesh = catalog.get_mut(ObjectId::PlanetBase).unwrap();
            mesh.vertices[0].position += Vec3::X;
            mesh.vertices[0].position
        };
        let mut state = CadSceneState {
            editing: true,
            selected_faces: vec![0],
            selected_group: Some(1),
            selected_vertices: vec![0],
            dirty: false,
            interaction_preview: true,
            primitive_placement: Some(PrimitivePlacement {
                primitive: MeshPrimitive::Cube,
                center: Some(Vec3::ZERO),
                dimensions: "1".to_string(),
                input_focused: true,
                input_selected: false,
                dimension_index: 0,
            }),
            ..CadSceneState::default()
        };

        close_editor(&mut state);

        assert_eq!(
            catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position,
            position
        );
        assert!(!state.editing);
        assert!(state.selected_faces.is_empty());
        assert!(state.selected_group.is_none());
        assert!(state.selected_vertices.is_empty());
        assert!(state.undo_stack.is_empty());
        assert!(!state.interaction_preview);
        assert!(state.primitive_placement.is_none());
    }

    #[test]
    fn vertex_edits_include_duplicate_face_corners() {
        let mut mesh = EditableMesh {
            vertices: vec![
                Vertex::new2(Vec3::ZERO, Vec2::ZERO, WHITE),
                Vertex::new2(Vec3::ZERO, Vec2::ZERO, WHITE),
                Vertex::new2(Vec3::X, Vec2::ZERO, WHITE),
            ],
            indices: vec![0, 1, 2],
        };
        let state = CadSceneState {
            selected_vertices: vec![0],
            ..CadSceneState::default()
        };
        let command = TransformCommand {
            mode: TransformMode::Move,
            pivot: Vec3::ZERO,
            values: Vec3::Y,
        };

        assert_eq!(selected_indices(&mesh, &state), vec![0, 1]);
        apply_transform_to_selection(&mut mesh, &state, command).unwrap();
        assert_eq!(mesh.vertices[0].position, Vec3::Y);
        assert_eq!(mesh.vertices[1].position, Vec3::Y);
        assert_eq!(mesh.vertices[2].position, Vec3::X);
    }

    #[test]
    fn transform_input_requires_three_finite_values() {
        assert_eq!(
            parse_transform_values(TransformMode::Move, "-1.5, 2, 0").unwrap(),
            vec3(-1.5, 2.0, 0.0)
        );
        assert!(parse_transform_values(TransformMode::Move, "1, 2").is_err());
        assert!(parse_transform_values(TransformMode::Move, "1,,2,3").is_err());
        assert!(parse_transform_values(TransformMode::Move, "1, NaN, 0").is_err());
        assert!(parse_transform_values(TransformMode::Scale, "1, 0, 1").is_err());
    }

    #[test]
    fn transform_axis_constraint_leaves_other_axes_unchanged() {
        assert_eq!(
            constrained_transform_values(
                TransformMode::Move,
                TransformAxis::Y,
                vec3(3.0, 4.0, 5.0)
            ),
            vec3(0.0, 4.0, 0.0)
        );
        assert_eq!(
            constrained_transform_values(
                TransformMode::Scale,
                TransformAxis::X,
                vec3(2.0, 3.0, 4.0)
            ),
            vec3(2.0, 1.0, 1.0)
        );
    }

    #[test]
    fn rotation_and_scale_use_pivot_and_only_affect_selected_vertices() {
        let mut mesh = EditableMesh {
            vertices: vec![
                Vertex::new2(Vec3::X, Vec2::ZERO, WHITE),
                Vertex::new2(Vec3::ZERO, Vec2::ZERO, WHITE),
            ],
            indices: vec![],
        };
        let state = CadSceneState {
            selected_vertices: vec![0],
            ..CadSceneState::default()
        };

        apply_transform_to_selection(
            &mut mesh,
            &state,
            TransformCommand {
                mode: TransformMode::Rotate,
                pivot: Vec3::ZERO,
                values: vec3(0.0, 0.0, 90.0),
            },
        )
        .unwrap();
        assert!((mesh.vertices[0].position - Vec3::Y).length() < 1e-5);
        assert_eq!(mesh.vertices[1].position, Vec3::ZERO);

        apply_transform_to_selection(
            &mut mesh,
            &state,
            TransformCommand {
                mode: TransformMode::Scale,
                pivot: vec3(-1.0, 1.0, 0.0),
                values: vec3(2.0, 1.0, 1.0),
            },
        )
        .unwrap();
        assert!((mesh.vertices[0].position - vec3(1.0, 1.0, 0.0)).length() < 1e-5);
        assert_eq!(mesh.vertices[1].position, Vec3::ZERO);
    }

    #[test]
    fn gizmo_preview_is_non_destructive_and_commit_is_undoable() {
        let mut catalog = ObjectCatalog::load();
        let original = catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position;
        let mut state = CadSceneState {
            selected_vertices: vec![0],
            ..CadSceneState::default()
        };
        let source = selected_mesh(&catalog, &state).unwrap().clone();
        let center = selected_bounds_center(&source, &selected_indices(&source, &state)).unwrap();
        let command = TransformCommand {
            mode: TransformMode::Move,
            pivot: center,
            values: Vec3::X,
        };
        let mut preview = source.clone();
        apply_transform_to_selection(&mut preview, &state, command).unwrap();
        state.gizmo_preview = Some(command);

        assert_eq!(
            catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position,
            original
        );

        commit_transform_command(&mut catalog, &mut state, command).unwrap();
        assert!(state.gizmo_preview.is_none());
        assert_eq!(state.undo_stack.len(), 1);
        let transformed = catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position;
        assert_ne!(transformed, original);
        assert!(undo(&mut catalog, &mut state).unwrap());
        assert_eq!(
            catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position,
            original
        );
    }

    #[test]
    fn grouping_and_group_deletion_are_undoable() {
        let mut catalog = ObjectCatalog::load();
        let face_count = catalog.get(ObjectId::PlanetBase).unwrap().face_count();
        assert!(face_count > 2);
        let mut state = CadSceneState {
            editing: true,
            selection_mode: SelectionMode::Face,
            selected_faces: vec![0, 1],
            ..CadSceneState::default()
        };

        create_selected_group(&mut catalog, &mut state).unwrap();
        let group_id = state.selected_group.unwrap();
        assert_eq!(
            selected_metadata(&catalog, &state)
                .unwrap()
                .group(group_id)
                .unwrap()
                .faces,
            vec![0, 1]
        );
        assert!(undo(&mut catalog, &mut state).unwrap());
        assert!(
            selected_metadata(&catalog, &state)
                .unwrap()
                .group(group_id)
                .is_none()
        );
        assert!(redo(&mut catalog, &mut state).unwrap());
        assert_eq!(
            selected_metadata(&catalog, &state)
                .unwrap()
                .group(group_id)
                .unwrap()
                .faces,
            vec![0, 1]
        );

        delete_selected(&mut catalog, &mut state).unwrap();
        assert_eq!(
            selected_mesh(&catalog, &state).unwrap().face_count(),
            face_count - 2
        );
        assert!(
            selected_metadata(&catalog, &state)
                .unwrap()
                .group(group_id)
                .is_none()
        );
        assert!(undo(&mut catalog, &mut state).unwrap());
        assert_eq!(
            selected_mesh(&catalog, &state).unwrap().face_count(),
            face_count
        );
        assert_eq!(
            selected_metadata(&catalog, &state)
                .unwrap()
                .group(group_id)
                .unwrap()
                .faces,
            vec![0, 1]
        );
    }

    #[test]
    fn geometry_history_is_undoable_and_capped() {
        let mut catalog = ObjectCatalog::load();
        let original_position = catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position;
        let mut state = CadSceneState {
            editing: true,
            selected_vertices: vec![0],
            ..CadSceneState::default()
        };
        let snapshot = capture_snapshot(&catalog, &state).unwrap();
        apply_transform_to_selection(
            selected_mesh_mut(&mut catalog, &state).unwrap(),
            &state,
            TransformCommand {
                mode: TransformMode::Move,
                pivot: Vec3::ZERO,
                values: Vec3::X,
            },
        )
        .unwrap();
        push_undo(&mut state, snapshot.clone());

        assert!(undo(&mut catalog, &mut state).unwrap());
        assert_eq!(
            catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position,
            original_position
        );
        assert!(redo(&mut catalog, &mut state).unwrap());
        assert_eq!(
            catalog.get(ObjectId::PlanetBase).unwrap().vertices[0].position,
            original_position + Vec3::X
        );

        for _ in 0..=HISTORY_LIMIT {
            push_undo(&mut state, snapshot.clone());
        }
        assert_eq!(state.undo_stack.len(), HISTORY_LIMIT);
    }

    fn trim_test_mesh() -> EditableMesh {
        let vertex = |position| Vertex::new2(position, Vec2::ZERO, WHITE);
        EditableMesh {
            vertices: vec![
                vertex(vec3(0.0, 0.0, 0.0)),
                vertex(vec3(2.0, 0.0, 0.0)),
                vertex(vec3(0.0, 2.0, 0.0)),
                vertex(vec3(0.5, -1.0, -1.0)),
                vertex(vec3(0.5, 3.0, -1.0)),
                vertex(vec3(0.5, 1.0, 1.0)),
            ],
            indices: vec![0, 1, 2, 3, 4, 5],
        }
    }

    #[test]
    fn trim_splits_targets_and_keeps_both_sides_with_undo() {
        let mut catalog = ObjectCatalog::load();
        let mut state = CadSceneState {
            editing: true,
            trim_active: true,
            trim_phase: TrimPhase::Cutters,
            trim_target_faces: vec![0],
            trim_cutter_faces: vec![1],
            ..CadSceneState::default()
        };
        *selected_mesh_mut(&mut catalog, &state).unwrap() = trim_test_mesh();
        let metadata = selected_metadata_mut(&mut catalog, &state).unwrap();
        *metadata = FaceMetadata::for_face_count(2);
        metadata.face_names[0] = Some("Target".to_string());
        let group = metadata.create_group(&[0]).unwrap();
        let original = selected_mesh(&catalog, &state).unwrap().clone();

        apply_trim(&mut catalog, &mut state).unwrap();
        assert_eq!(selected_mesh(&catalog, &state).unwrap().face_count(), 4);
        let metadata = selected_metadata(&catalog, &state).unwrap();
        assert_eq!(metadata.face_names.len(), 4);
        assert_eq!(
            metadata
                .face_names
                .iter()
                .filter(|name| name.is_some())
                .count(),
            3
        );
        assert!(
            metadata
                .group(group)
                .unwrap()
                .faces
                .iter()
                .all(|&face| face < 4)
        );
        let target_centroids = state
            .selected_faces
            .iter()
            .map(|&face| {
                let start = face * 3;
                (0..3)
                    .map(|slot| {
                        selected_mesh(&catalog, &state).unwrap().vertices[selected_mesh(
                            &catalog, &state,
                        )
                        .unwrap()
                        .indices[start + slot]
                            as usize]
                            .position
                            .x
                    })
                    .sum::<f32>()
                    / 3.0
            })
            .collect::<Vec<_>>();
        assert!(target_centroids.iter().any(|&x| x < 0.5));
        assert!(target_centroids.iter().any(|&x| x > 0.5));
        assert!(undo(&mut catalog, &mut state).unwrap());
        assert_eq!(selected_mesh(&catalog, &state).unwrap().face_count(), 2);
        assert_eq!(
            selected_mesh(&catalog, &state).unwrap().indices,
            original.indices
        );
        assert_eq!(
            selected_metadata(&catalog, &state).unwrap().face_names[0].as_deref(),
            Some("Target")
        );
    }

    #[test]
    fn deleting_a_radial_group_removes_its_program_descendants() {
        let mut catalog = ObjectCatalog::load();
        let mut state = CadSceneState {
            editing: true,
            ..CadSceneState::default()
        };
        let mesh = EditableMesh::primitive(MeshPrimitive::Cube);
        let face_count = mesh.face_count();
        *selected_mesh_mut(&mut catalog, &state).unwrap() = mesh;

        let mut metadata = FaceMetadata::for_face_count(face_count);
        let radial_group = metadata.create_group(&[0]).unwrap();
        metadata
            .set_group_program(radial_group, FaceGroupProgram::RadialRepeat)
            .unwrap();
        let condition_group = metadata
            .create_group_under(&[1], Some(radial_group))
            .unwrap();
        metadata
            .set_group_program(
                condition_group,
                FaceGroupProgram::IfNeighbor(RadialNeighborState::Route),
            )
            .unwrap();
        metadata.radial_pivot = Some([1.0, 2.0, 3.0]);
        *selected_metadata_mut(&mut catalog, &state).unwrap() = metadata;
        state.selected_group = Some(radial_group);

        delete_selected(&mut catalog, &mut state).unwrap();
        assert_eq!(
            selected_mesh(&catalog, &state).unwrap().face_count(),
            face_count - 2
        );
        let metadata = selected_metadata(&catalog, &state).unwrap();
        assert!(metadata.groups.is_empty());
        assert_eq!(metadata.radial_pivot, None);

        assert!(undo(&mut catalog, &mut state).unwrap());
        assert_eq!(
            selected_mesh(&catalog, &state).unwrap().face_count(),
            face_count
        );
        assert_eq!(
            selected_metadata(&catalog, &state)
                .unwrap()
                .radial_main_group_id(),
            Some(radial_group)
        );
    }

    #[test]
    fn trim_requires_intersection_and_leaves_mesh_unchanged_on_failure() {
        let mut catalog = ObjectCatalog::load();
        let mut state = CadSceneState {
            editing: true,
            trim_active: true,
            trim_phase: TrimPhase::Cutters,
            trim_target_faces: vec![0],
            trim_cutter_faces: vec![1],
            ..CadSceneState::default()
        };
        let mut mesh = trim_test_mesh();
        for index in 3..6 {
            mesh.vertices[index].position.x += 5.0;
        }
        *selected_mesh_mut(&mut catalog, &state).unwrap() = mesh;
        let original = selected_mesh(&catalog, &state).unwrap().clone();

        assert!(apply_trim(&mut catalog, &mut state).is_err());
        assert_eq!(
            selected_mesh(&catalog, &state).unwrap().indices,
            original.indices
        );
        assert!(state.undo_stack.is_empty());
        assert!(state.trim_active);
    }

    #[test]
    fn trim_path_splits_selected_face_and_is_undoable() {
        let mut catalog = ObjectCatalog::load();
        let mut state = CadSceneState {
            editing: true,
            trim_active: true,
            trim_phase: TrimPhase::Cutters,
            trim_target_faces: vec![0],
            trim_path_face: Some(0),
            trim_path_points: vec![vec3(0.5, 0.0, 0.0), vec3(0.0, 0.0, -0.5)],
            ..CadSceneState::default()
        };
        let mut mesh = EditableMesh::primitive(MeshPrimitive::Plane);
        *selected_mesh_mut(&mut catalog, &state).unwrap() = mesh.clone();

        apply_trim(&mut catalog, &mut state).unwrap();
        assert_eq!(selected_mesh(&catalog, &state).unwrap().face_count(), 4);
        assert!(undo(&mut catalog, &mut state).unwrap());
        mesh = selected_mesh(&catalog, &state).unwrap().clone();
        assert_eq!(mesh.face_count(), 2);
    }

    #[test]
    fn offset_copy_commits_and_is_undoable() {
        let mut catalog = ObjectCatalog::load();
        let mut state = CadSceneState {
            editing: true,
            selected_faces: vec![0],
            operation_value: "0.1".to_string(),
            ..CadSceneState::default()
        };
        let original_face_count = selected_mesh(&catalog, &state).unwrap().face_count();
        apply_offset(&mut catalog, &mut state).unwrap();
        assert_eq!(
            selected_mesh(&catalog, &state).unwrap().face_count(),
            original_face_count + 1
        );
        assert_eq!(state.selected_faces, vec![original_face_count]);
        assert!(undo(&mut catalog, &mut state).unwrap());
        assert_eq!(
            selected_mesh(&catalog, &state).unwrap().face_count(),
            original_face_count
        );
    }

    #[test]
    fn mirror_copy_inherits_face_metadata_and_is_undoable() {
        let mut catalog = ObjectCatalog::load();
        let mut state = CadSceneState {
            editing: true,
            selected_faces: vec![0],
            ..CadSceneState::default()
        };
        let original_face_count = selected_mesh(&catalog, &state).unwrap().face_count();
        let group = {
            let metadata = selected_metadata_mut(&mut catalog, &state).unwrap();
            metadata.face_names[0] = Some("Mirrored face".to_string());
            metadata.create_group(&[0]).unwrap()
        };

        apply_mirror(&mut catalog, &mut state, Vec3::ZERO, Vec3::X).unwrap();
        let metadata = selected_metadata(&catalog, &state).unwrap();
        assert_eq!(
            metadata.face_names[original_face_count].as_deref(),
            Some("Mirrored face")
        );
        assert!(
            metadata
                .group(group)
                .unwrap()
                .faces
                .contains(&original_face_count)
        );
        assert!(undo(&mut catalog, &mut state).unwrap());
        assert_eq!(
            selected_mesh(&catalog, &state).unwrap().face_count(),
            original_face_count
        );
        assert_eq!(
            selected_metadata(&catalog, &state).unwrap().face_names[0].as_deref(),
            Some("Mirrored face")
        );
    }

    #[test]
    fn group_lock_blocks_edits_and_hidden_faces_are_removed_from_render_mesh() {
        let mut catalog = ObjectCatalog::load();
        let mut state = CadSceneState {
            editing: true,
            ..CadSceneState::default()
        };
        let group_id = selected_metadata_mut(&mut catalog, &state)
            .unwrap()
            .create_group(&[0])
            .unwrap();
        let metadata = selected_metadata(&catalog, &state).unwrap();
        select_group(&mut state, metadata, group_id).unwrap();

        toggle_group_flag(&mut catalog, &mut state, group_id, GroupFlag::Locked).unwrap();
        assert!(ensure_selection_unlocked(&catalog, &state).is_err());
        let before = selected_mesh(&catalog, &state).unwrap().clone();
        assert!(
            commit_transform_command(
                &mut catalog,
                &mut state,
                TransformCommand {
                    mode: TransformMode::Move,
                    pivot: Vec3::ZERO,
                    values: Vec3::X,
                },
            )
            .is_err()
        );
        assert_eq!(
            selected_mesh(&catalog, &state).unwrap().indices,
            before.indices
        );
        toggle_group_flag(&mut catalog, &mut state, group_id, GroupFlag::Locked).unwrap();
        assert!(ensure_selection_unlocked(&catalog, &state).is_ok());

        toggle_group_flag(&mut catalog, &mut state, group_id, GroupFlag::Hidden).unwrap();
        assert!(ensure_selection_unlocked(&catalog, &state).is_err());
        let hidden_faces = selected_metadata(&catalog, &state).unwrap().hidden_faces();
        assert!(hidden_faces.contains(&0));
        let mesh = selected_mesh(&catalog, &state).unwrap();
        let visible_mesh = mesh_without_faces(mesh, &hidden_faces);
        assert_eq!(visible_mesh.face_count(), mesh.face_count() - 1);
        assert_eq!(&visible_mesh.indices[..3], &mesh.indices[3..6]);
    }
}
