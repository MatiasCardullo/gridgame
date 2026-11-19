use std::collections::HashSet;

use macroquad::prelude::*;

use crate::core::object_assets::{
    EditableMesh, FaceGroupProgram, FaceMetadata, MeshPrimitive, ObjectCatalog, ObjectCategory,
    ObjectId, RadialNeighborState,
};
use crate::core::ui::ui_button;
use crate::core::{FrameContext, Scene};

const SIDEBAR_WIDTH: f32 = 250.0;
const TOOLBAR_HEIGHT: f32 = 62.0;
const CAMERA_MIN_DISTANCE: f32 = 1.5;
const CAMERA_MAX_DISTANCE: f32 = 100.0;
const HISTORY_LIMIT: usize = 100;
const FACE_LIST_TOP: f32 = 148.0;
const FACE_ROW_HEIGHT: f32 = 26.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionMode {
    Vertex,
    Edge,
    Face,
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

#[derive(Clone, Copy, Debug)]
struct KnifeStart {
    point: Vec3,
    face: usize,
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
    group_config_dialog: Option<GroupConfigDialog>,
    knife_active: bool,
    knife_start: Option<KnifeStart>,
    primitive_placement: Option<PrimitivePlacement>,
    grid_snap: bool,
    grid_spacing: f32,
    interaction_preview: bool,
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
            group_config_dialog: None,
            knife_active: false,
            knife_start: None,
            primitive_placement: None,
            grid_snap: false,
            grid_spacing: 0.5,
            interaction_preview: false,
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
    let (center, scale) = mesh_center_and_scale(mesh);
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

fn mesh_ray_hit(origin: Vec3, direction: Vec3, mesh: &EditableMesh) -> Option<usize> {
    mesh_ray_hit_point(origin, direction, mesh, None).map(|(_, face, _)| face)
}

fn mesh_ray_hit_point(
    origin: Vec3,
    direction: Vec3,
    mesh: &EditableMesh,
    allowed_faces: Option<&[usize]>,
) -> Option<(f32, usize, Vec3)> {
    let mut closest: Option<(f32, usize, Vec3)> = None;
    for face in 0..mesh.face_count() {
        if allowed_faces.is_some_and(|faces| !faces.contains(&face)) {
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

fn pick_vertex(camera: &Camera3D, mouse: Vec2, mesh: &EditableMesh) -> Option<usize> {
    let mut closest: Option<(f32, usize)> = None;
    for (index, vertex) in mesh.vertices.iter().enumerate() {
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

fn pick_edge(camera: &Camera3D, mouse: Vec2, mesh: &EditableMesh) -> Option<(usize, usize)> {
    let mut closest: Option<(f32, (usize, usize))> = None;
    let mut visited = HashSet::new();
    for triangle in mesh.indices.chunks_exact(3) {
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

fn draw_view_controls(ctx: &FrameContext, state: &CadSceneState, panel: Rect) {
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
        let snap_rect = grid_snap_rect(panel);
        let spacing_minus = grid_spacing_minus_rect(panel);
        let spacing_plus = grid_spacing_plus_rect(panel);
        let preview_rect = interaction_preview_rect(panel);
        let _ = button(
            ctx,
            snap_rect,
            if state.grid_snap {
                "Snap: On"
            } else {
                "Snap: Off"
            },
        );
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
    }
}

fn grid_snap_rect(panel: Rect) -> Rect {
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
    state.original_mesh = None;
    state.original_metadata = None;
    state.selection_mode = SelectionMode::Vertex;
    state.selected_group = None;
    state.selected_faces.clear();
    state.selected_vertices.clear();
    state.selected_edges.clear();
    state.dragged_faces = None;
    state.rename_dialog = None;
    state.group_config_dialog = None;
    state.knife_active = false;
    state.knife_start = None;
    state.primitive_placement = None;
    state.interaction_preview = false;
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
    state.group_config_dialog = None;
    state.knife_active = false;
    state.knife_start = None;
    state.primitive_placement = None;
    state.interaction_preview = false;
    state.undo_stack.clear();
    state.redo_stack.clear();
    state.face_scroll = 0;
    state.error = None;
    state.target = Vec3::ZERO;
    state.distance = 4.0;
}

fn move_selected(mesh: &mut EditableMesh, state: &CadSceneState, offset: Vec3) -> bool {
    let indices = selected_indices(mesh, state);
    if indices.is_empty() {
        return false;
    }
    for index in indices {
        if let Some(vertex) = mesh.vertices.get_mut(index) {
            vertex.position += offset;
        }
    }
    true
}

fn scale_selected(mesh: &mut EditableMesh, state: &CadSceneState, factor: f32) -> bool {
    let indices = selected_indices(mesh, state);
    if indices.is_empty() {
        return false;
    }
    let points = indices
        .iter()
        .filter_map(|&index| mesh.vertices.get(index).map(|vertex| vertex.position))
        .collect::<Vec<_>>();
    if points.is_empty() {
        return false;
    }
    let center = points
        .iter()
        .copied()
        .fold(Vec3::ZERO, |sum, point| sum + point)
        / points.len() as f32;
    for index in indices {
        if let Some(vertex) = mesh.vertices.get_mut(index) {
            vertex.position = center + (vertex.position - center) * factor;
        }
    }
    true
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

fn create_selected_group(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
) -> Result<(), String> {
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

fn rotate_mesh_y(mesh: &EditableMesh, angle: f32) -> EditableMesh {
    let (sin, cos) = angle.sin_cos();
    mesh.transformed(|point| {
        vec3(
            point.x * cos - point.z * sin,
            point.y,
            point.x * sin + point.z * cos,
        )
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
    compose_conditioned_preview(base_mesh, metadata, &state.preview_neighbors)
}

fn compose_conditioned_preview(
    base_mesh: &EditableMesh,
    metadata: &FaceMetadata,
    neighbors: &[RadialNeighborState; 6],
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
        composite.append(&rotate_mesh_y(
            &sector_mesh,
            std::f32::consts::TAU * sector as f32 / 6.0,
        ))?;
    }
    Ok(composite)
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
            entry.definition.category != ObjectCategory::Units
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

fn draw_preview_object(mesh: &EditableMesh, center: Vec3, scale: f32) -> Result<(), String> {
    let normalized = preview_mesh(mesh);
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
        draw_preview_object(&center_mesh, origin, 0.28)?;
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
            draw_preview_object(mesh, hex_cell_center(index, CELL_RADIUS, origin), 0.24)?;
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

fn perform_knife_cut(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
    preview: &EditableMesh,
    start: KnifeStart,
    end: Vec3,
) -> Result<(), String> {
    let snapshot = capture_snapshot(catalog, state)?;
    let triangle_start = start
        .face
        .checked_mul(3)
        .ok_or_else(|| "face index overflow".to_string())?;
    let indices = preview
        .indices
        .get(triangle_start..triangle_start + 3)
        .ok_or_else(|| "selected face is unavailable".to_string())?;
    let [a, b, c] = [
        preview.vertices.get(indices[0] as usize),
        preview.vertices.get(indices[1] as usize),
        preview.vertices.get(indices[2] as usize),
    ];
    let [Some(a), Some(b), Some(c)] = [a, b, c] else {
        return Err("selected face has invalid mesh indices".to_string());
    };
    let face_normal = (b.position - a.position)
        .cross(c.position - a.position)
        .normalize_or_zero();
    let plane_normal = face_normal.cross(end - start.point).normalize_or_zero();
    if plane_normal.length_squared() <= f32::EPSILON {
        return Err("choose two distinct points on the selected face".to_string());
    }
    let (center, scale) = mesh_center_and_scale(&snapshot.mesh);
    if scale <= f32::EPSILON {
        return Err("model has no usable geometry".to_string());
    }
    let to_model_space = |point: Vec3| point / scale + center;
    let plane_point = to_model_space(start.point);
    let mut mesh = snapshot.mesh.clone();
    let face_map = mesh.cut_faces_by_plane(&state.selected_faces, plane_point, plane_normal)?;
    let mut metadata = snapshot.metadata.clone();
    metadata.remap_after_face_split(&face_map);
    let selected_faces = state
        .selected_faces
        .iter()
        .filter_map(|&face| face_map.get(face))
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    *selected_mesh_mut(catalog, state)
        .ok_or_else(|| "Selected object is unavailable".to_string())? = mesh;
    *selected_metadata_mut(catalog, state)
        .ok_or_else(|| "Selected face metadata is unavailable".to_string())? = metadata;
    state.selected_faces = selected_faces;
    state.selected_group = None;
    state.selected_vertices.clear();
    state.selected_edges.clear();
    push_undo(state, snapshot);
    state.dirty = true;
    state.error = None;
    Ok(())
}

fn assign_dragged_faces(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
    faces: &[usize],
    group_id: u64,
) -> Result<(), String> {
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
    let snapshot = capture_snapshot(catalog, state)?;
    let mut mesh = snapshot.mesh.clone();
    let mut metadata = snapshot.metadata.clone();
    let faces = match state.selected_group {
        Some(group_id) => metadata.remove_group(group_id)?,
        None => state.selected_faces.clone(),
    };
    if faces.is_empty() {
        return Err("select one or more faces or a group to delete".to_string());
    }
    let face_remap = mesh.remove_faces(&faces)?;
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

pub fn run(
    ctx: &FrameContext,
    state: &mut CadSceneState,
    catalog: &mut ObjectCatalog,
    scene: &mut Scene,
) {
    let sidebar = Rect::new(0.0, 0.0, SIDEBAR_WIDTH, screen_height());
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
        screen_height() - TOOLBAR_HEIGHT,
    );
    let view_panel = Rect::new(
        screen_width() - 205.0,
        18.0,
        190.0,
        if state.editing {
            if state.interaction_preview {
                318.0
            } else {
                288.0
            }
        } else {
            155.0
        },
    );
    if is_key_pressed(KeyCode::Escape) {
        if state.primitive_placement.is_some() {
            state.primitive_placement = None;
            state.error = None;
        } else if state.knife_active {
            state.knife_active = false;
            state.knife_start = None;
            state.error = None;
        } else if state.group_config_dialog.is_some() {
            state.group_config_dialog = None;
        } else if state.rename_dialog.is_some() {
            state.rename_dialog = None;
        } else if state.exit_confirmation_open {
            state.exit_confirmation_open = false;
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
    let mut modal_open = state.exit_confirmation_open
        || state.group_config_dialog.is_some()
        || state.primitive_placement.is_some();
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
        || state
            .primitive_placement
            .as_ref()
            .is_some_and(|placement| placement.center.is_some())
            && placement_panel_rect(viewport).contains(ctx.mouse);
    if !modal_open && !rename_dialog_open && state.editing {
        if button(
            ctx,
            grid_snap_rect(view_panel),
            if state.grid_snap {
                "Snap: On"
            } else {
                "Snap: Off"
            },
        ) {
            state.grid_snap = !state.grid_snap;
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
                    - usize::from(matches!(item, SidebarItem::Group(_))) as f32 * 42.0,
                FACE_ROW_HEIGHT - 2.0,
            );
            let configure_rect = Rect::new(SIDEBAR_WIDTH - 48.0, y, 36.0, FACE_ROW_HEIGHT - 2.0);
            if !modal_open
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
        for (label, offset) in [
            ("-X", vec3(-0.1, 0.0, 0.0)),
            ("+X", vec3(0.1, 0.0, 0.0)),
            ("-Y", vec3(0.0, -0.1, 0.0)),
            ("+Y", vec3(0.0, 0.1, 0.0)),
            ("-Z", vec3(0.0, 0.0, -0.1)),
            ("+Z", vec3(0.0, 0.0, 0.1)),
        ] {
            if !modal_open
                && !rename_dialog_open
                && button(ctx, Rect::new(action_x, action_y, 46.0, 30.0), label)
            {
                match capture_snapshot(catalog, state) {
                    Ok(snapshot) => {
                        if let Some(mesh) = selected_mesh_mut(catalog, state) {
                            if move_selected(mesh, state, offset) {
                                push_undo(state, snapshot);
                                state.dirty = true;
                            } else {
                                state.error = Some("Select a vertex or face first".to_string());
                            }
                        }
                    }
                    Err(error) => state.error = Some(error),
                }
            }
            action_x += 50.0;
        }
        action_x += 8.0;
        if !modal_open
            && !rename_dialog_open
            && button(ctx, Rect::new(action_x, action_y, 72.0, 30.0), "Scale +")
        {
            match capture_snapshot(catalog, state) {
                Ok(snapshot) => {
                    if let Some(mesh) = selected_mesh_mut(catalog, state) {
                        if scale_selected(mesh, state, 1.1) {
                            push_undo(state, snapshot);
                            state.dirty = true;
                        } else {
                            state.error = Some("Select a vertex or face first".to_string());
                        }
                    }
                }
                Err(error) => state.error = Some(error),
            }
        }
        action_x += 76.0;
        if !modal_open
            && !rename_dialog_open
            && button(ctx, Rect::new(action_x, action_y, 72.0, 30.0), "Scale -")
        {
            match capture_snapshot(catalog, state) {
                Ok(snapshot) => {
                    if let Some(mesh) = selected_mesh_mut(catalog, state) {
                        if scale_selected(mesh, state, 0.9) {
                            push_undo(state, snapshot);
                            state.dirty = true;
                        } else {
                            state.error = Some("Select a vertex or face first".to_string());
                        }
                    }
                }
                Err(error) => state.error = Some(error),
            }
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
                            metadata.inherit_group_for_extrusion(
                                face,
                                old_face_count,
                                new_face_count.saturating_sub(old_face_count),
                            );
                        }
                        state.selected_faces = vec![old_face_count];
                        push_undo(state, before);
                        state.dirty = true;
                        state.error = None;
                    }
                    Err(error) => state.error = Some(error),
                }
            }
        }
        action_x += 88.0;
        if !modal_open
            && !rename_dialog_open
            && button(
                ctx,
                Rect::new(action_x, action_y, 84.0, 30.0),
                if state.knife_active {
                    "Cancel Cut"
                } else {
                    "Knife"
                },
            )
        {
            if state.knife_active {
                state.knife_active = false;
                state.knife_start = None;
                state.error = None;
            } else if state.selected_faces.is_empty() {
                state.error = Some("Select one or more faces before using Knife".to_string());
            } else {
                state.knife_active = true;
                state.knife_start = None;
                state.selection_mode = SelectionMode::Face;
                state.selected_edges.clear();
                state.error = None;
            }
        }
    }
    ui_capturing |= state.editing
        && Rect::new(SIDEBAR_WIDTH, action_y - 4.0, screen_width(), 40.0).contains(ctx.mouse);

    let camera = object_camera(state);
    let editor_modal_open =
        state.exit_confirmation_open || state.group_config_dialog.is_some() || rename_dialog_open;
    let viewport_active = !editor_modal_open && viewport.contains(ctx.mouse) && !ui_capturing;
    if viewport_active && is_mouse_button_down(MouseButton::Right) {
        let delta = ctx.mouse - state.last_mouse;
        state.yaw -= delta.x * 0.008;
        state.pitch = (state.pitch - delta.y * 0.006).clamp(-1.45, 1.45);
    } else if viewport_active && is_mouse_button_down(MouseButton::Middle) {
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
    let preview = current_mesh.as_ref().map(preview_mesh);
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
            draw_square_grid(model_center, scale, state.grid_spacing);
        }
        if let Some(mesh) = preview.as_ref() {
            if let Err(error) = mesh.render() {
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
                        if let Some(start) = state.knife_start {
                            draw_sphere(
                                start.point,
                                0.035,
                                None,
                                Color::from_rgba(255, 90, 90, 255),
                            );
                        }
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

    if viewport_active
        && state.editing
        && !state.interaction_preview
        && is_mouse_button_pressed(MouseButton::Left)
    {
        if let (Some((origin, direction)), Some(mesh)) =
            (mouse_ray(&camera, ctx.mouse), preview.as_ref())
        {
            if let Some(placement) = state.primitive_placement.clone() {
                if let Some(display_point) = construction_plane_hit(origin, direction) {
                    let mut model_point = display_to_model_point(
                        current_mesh.as_ref().unwrap_or(mesh),
                        display_point,
                    );
                    if state.grid_snap {
                        model_point = snap_model_point(model_point, state.grid_spacing);
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
            } else if state.knife_active {
                let hit = mesh_ray_hit_point(origin, direction, mesh, Some(&state.selected_faces));
                match (state.knife_start, hit) {
                    (None, Some((_, face, point))) => {
                        state.knife_start = Some(KnifeStart { point, face });
                        state.error = None;
                    }
                    (Some(start), Some((_, face, point))) if face == start.face => {
                        match perform_knife_cut(catalog, state, mesh, start, point) {
                            Ok(()) => {
                                state.knife_active = false;
                                state.knife_start = None;
                            }
                            Err(error) => state.error = Some(error),
                        }
                    }
                    (Some(_), Some(_)) => {
                        state.error =
                            Some("Both knife points must lie on the same selected face".into());
                    }
                    (_, None) => {
                        state.error = Some("Knife points must land on the selected face(s)".into());
                    }
                }
            } else {
                let extend = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                match state.selection_mode {
                    SelectionMode::Vertex => {
                        if let Some(index) = pick_vertex(&camera, ctx.mouse, mesh) {
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
                        if let Some(edge) = pick_edge(&camera, ctx.mouse, mesh) {
                            select_edge(state, edge, extend);
                        } else if !extend {
                            state.selected_edges.clear();
                        }
                    }
                    SelectionMode::Face => {
                        if let Some(face) = mesh_ray_hit(origin, direction, mesh) {
                            select_face(state, face, extend);
                        } else if !extend {
                            state.selected_faces.clear();
                            state.selected_group = None;
                        }
                    }
                }
            }
            if !state.knife_active {
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
                let configure_width = if is_group { 42.0 } else { 0.0 };
                let rect = Rect::new(
                    12.0 + indent,
                    y,
                    SIDEBAR_WIDTH - 24.0 - indent - configure_width,
                    FACE_ROW_HEIGHT - 2.0,
                );
                draw_list_row(ctx, rect, &label, selected, color);
                if is_group {
                    let configure =
                        Rect::new(rect.x + rect.w + 4.0, y, 38.0, FACE_ROW_HEIGHT - 2.0);
                    let hovered = configure.contains(ctx.mouse);
                    draw_rectangle(
                        configure.x,
                        configure.y,
                        configure.w,
                        configure.h,
                        if hovered {
                            ctx.colors_rt.button_hover
                        } else {
                            ctx.colors_rt.button_base
                        },
                    );
                    draw_rectangle_lines(
                        configure.x,
                        configure.y,
                        configure.w,
                        configure.h,
                        1.0,
                        ctx.colors_rt.button_border,
                    );
                    draw_text(
                        "Set",
                        configure.x + 7.0,
                        configure.y + configure.h * 0.5 + ctx.font_sm * 0.35,
                        ctx.font_sm,
                        WHITE,
                    );
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
    if let Some(entry) = selected_entry(catalog, state.selected)
        && let Some(error) = entry
            .metadata_error
            .as_ref()
            .or_else(|| {
                entry
                    .states
                    .iter()
                    .find_map(|state| state.metadata_error.as_ref())
            })
            .or_else(|| entry.states.iter().find_map(|state| state.error.as_ref()))
            .or(entry.variant_config_error.as_ref())
            .or(entry.states_error.as_ref())
            .or(entry.error.as_ref())
    {
        draw_text(
            error,
            12.0,
            screen_height() - 24.0,
            ctx.font_sm,
            Color::from_rgba(255, 110, 110, 255),
        );
    }
    draw_view_controls(ctx, state, view_panel);
    if let Some(error) = &state.error {
        draw_text(
            error,
            SIDEBAR_WIDTH + 12.0,
            screen_height() - 18.0,
            ctx.font_sm,
            Color::from_rgba(255, 110, 110, 255),
        );
    }
    if state.dirty {
        draw_text(
            "Unsaved mesh edits",
            screen_width() - 220.0,
            screen_height() - 18.0,
            ctx.font_sm,
            Color::from_rgba(255, 220, 100, 255),
        );
    }

    draw_group_config_dialog(ctx, catalog, state);

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
        } else {
            draw_text(
                "Click the square grid to place the primitive center. Esc cancels.",
                SIDEBAR_WIDTH + 18.0,
                screen_height() - 24.0,
                ctx.font_sm,
                ctx.colors_rt.text_primary,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            dimensions_from_click(MeshPrimitive::Sphere, Vec3::ZERO, Vec3::X),
            vec3(2.0, 2.0, 2.0)
        );
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
        )
        .unwrap();
        assert_eq!(preview.face_count(), 10);
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

        assert_eq!(selected_indices(&mesh, &state), vec![0, 1]);
        assert!(move_selected(&mut mesh, &state, Vec3::Y));
        assert_eq!(mesh.vertices[0].position, Vec3::Y);
        assert_eq!(mesh.vertices[1].position, Vec3::Y);
        assert_eq!(mesh.vertices[2].position, Vec3::X);
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
        assert!(move_selected(
            selected_mesh_mut(&mut catalog, &state).unwrap(),
            &state,
            Vec3::X
        ));
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
}
