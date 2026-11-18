use std::collections::HashSet;

use macroquad::prelude::*;

use crate::core::object_assets::{
    EditableMesh, FaceGroupProgram, FaceMetadata, ObjectCatalog, ObjectCategory, ObjectId,
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

#[derive(Clone, Debug)]
struct EditSnapshot {
    mesh: EditableMesh,
    metadata: FaceMetadata,
    selected_faces: Vec<usize>,
    selected_group: Option<u64>,
    selected_vertices: Vec<usize>,
    selection_mode: SelectionMode,
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
    dragged_faces: Option<Vec<usize>>,
    rename_dialog: Option<RenameDialog>,
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
            dragged_faces: None,
            rename_dialog: None,
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

fn mesh_ray_hit(origin: Vec3, direction: Vec3, mesh: &EditableMesh) -> Option<usize> {
    let mut closest: Option<(f32, usize)> = None;
    for face in 0..mesh.face_count() {
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
        if distance > 0.0 && closest.map(|(best, _)| distance < best).unwrap_or(true) {
            closest = Some((distance, face));
        }
    }
    closest.map(|(_, face)| face)
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
            SelectionMode::Face => state.selected_faces.len(),
        };
        draw_text(
            &format!("Selected: {selected}"),
            panel.x + 12.0,
            y,
            ctx.font_sm,
            ctx.colors_rt.text_secondary,
        );
    }
    if let Some(error) = &state.error {
        draw_text(
            error,
            panel.x + 12.0,
            panel.y + panel.h - 18.0,
            ctx.font_sm,
            Color::from_rgba(255, 110, 110, 255),
        );
    }
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
    state.dragged_faces = None;
    state.rename_dialog = None;
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
    state.dragged_faces = None;
    state.rename_dialog = None;
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
    Ok(())
}

fn create_selected_group(
    catalog: &mut ObjectCatalog,
    state: &mut CadSceneState,
) -> Result<(), String> {
    let snapshot = capture_snapshot(catalog, state)?;
    let selected_faces = state.selected_faces.clone();
    let parent_id = selected_metadata(catalog, state).and_then(|metadata| {
        state.selected_group.or_else(|| {
            (state.selected == ObjectId::PlanetRoute)
                .then(|| metadata.radial_main_group_id())
                .flatten()
        })
    });
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
    let view_panel = Rect::new(screen_width() - 205.0, 18.0, 190.0, 155.0);
    if is_key_pressed(KeyCode::Escape) {
        if state.rename_dialog.is_some() {
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
    let modal_open = state.exit_confirmation_open;
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
        || view_panel.contains(ctx.mouse);
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
                SIDEBAR_WIDTH - 24.0 - indent,
                FACE_ROW_HEIGHT - 2.0,
            );
            if !modal_open && !rename_dialog_open && list_row_clicked(rect, ctx.mouse) {
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
                if state.selected == ObjectId::PlanetRoute {
                    match selected_metadata_mut(catalog, state)
                        .ok_or_else(|| "Selected face metadata is unavailable".to_string())
                        .and_then(FaceMetadata::ensure_radial_program)
                    {
                        Ok(changed) => state.dirty = changed,
                        Err(error) => state.error = Some(error),
                    }
                }
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
        state.selected_vertices.clear();
    }
    x += 94.0;
    if !modal_open
        && !rename_dialog_open
        && state.editing
        && button(ctx, Rect::new(x, 12.0, 86.0, 34.0), "Face")
    {
        state.selection_mode = SelectionMode::Face;
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
    }
    ui_capturing |= state.editing
        && Rect::new(SIDEBAR_WIDTH, action_y - 4.0, screen_width(), 40.0).contains(ctx.mouse);

    let camera = object_camera(state);
    let viewport_active =
        !modal_open && !rename_dialog_open && viewport.contains(ctx.mouse) && !ui_capturing;
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
    let camera = object_camera(state);

    let current_mesh = selected_mesh(catalog, state).cloned();
    let preview = current_mesh.as_ref().map(preview_mesh);
    set_camera(&camera);
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
        }
    }
    set_default_camera();

    if viewport_active && state.editing && is_mouse_button_pressed(MouseButton::Left) {
        if let (Some((origin, direction)), Some(mesh)) =
            (mouse_ray(&camera, ctx.mouse), preview.as_ref())
        {
            match state.selection_mode {
                SelectionMode::Vertex => {
                    if let Some(index) = pick_vertex(&camera, ctx.mouse, mesh) {
                        if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
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
                    state.selected_group = None;
                }
                SelectionMode::Face => {
                    if let Some(face) = mesh_ray_hit(origin, direction, mesh) {
                        let extend =
                            is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                        select_face(state, face, extend);
                    } else if !is_key_down(KeyCode::LeftShift) && !is_key_down(KeyCode::RightShift)
                    {
                        state.selected_faces.clear();
                        state.selected_group = None;
                    }
                }
            }
            state.error = None;
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
                let rect = Rect::new(
                    12.0 + indent,
                    y,
                    SIDEBAR_WIDTH - 24.0 - indent,
                    FACE_ROW_HEIGHT - 2.0,
                );
                draw_list_row(ctx, rect, &label, selected, color);
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
    if state.dirty {
        draw_text(
            "Unsaved mesh edits",
            screen_width() - 220.0,
            screen_height() - 18.0,
            ctx.font_sm,
            Color::from_rgba(255, 220, 100, 255),
        );
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
    fn radial_route_groups_appear_as_a_main_group_with_condition_children() {
        let mut metadata = FaceMetadata::default();
        metadata.ensure_radial_program().unwrap();

        let main_id = metadata.radial_main_group_id().unwrap();
        let items = sidebar_items(&metadata, 0);

        assert_eq!(items.first(), Some(&SidebarItem::Group(main_id)));
        let children = metadata
            .groups
            .iter()
            .filter(|group| group.parent_id == Some(main_id))
            .collect::<Vec<_>>();
        assert_eq!(children.len(), 3);
        let conditions = children
            .iter()
            .filter_map(|group| match group.program {
                FaceGroupProgram::IfNeighbor(condition) => Some(condition),
                _ => None,
            })
            .collect::<HashSet<_>>();
        assert_eq!(
            conditions,
            HashSet::from([
                crate::core::object_assets::RadialNeighborState::Empty,
                crate::core::object_assets::RadialNeighborState::Route,
                crate::core::object_assets::RadialNeighborState::OtherBuilding,
            ])
        );
        assert_eq!(
            items,
            std::iter::once(SidebarItem::Group(main_id))
                .chain(children.iter().map(|group| SidebarItem::Group(group.id)))
                .collect::<Vec<_>>()
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
