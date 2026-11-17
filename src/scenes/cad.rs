use macroquad::prelude::*;

use crate::core::object_assets::{EditableMesh, ObjectCatalog, ObjectCategory, ObjectId};
use crate::core::ui::ui_button;
use crate::core::{FrameContext, Scene};

const SIDEBAR_WIDTH: f32 = 250.0;
const TOOLBAR_HEIGHT: f32 = 62.0;
const CAMERA_MIN_DISTANCE: f32 = 1.5;
const CAMERA_MAX_DISTANCE: f32 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionMode {
    Vertex,
    Face,
}

#[derive(Clone, Debug)]
pub struct CadSceneState {
    category: ObjectCategory,
    selected: ObjectId,
    selected_state: Option<String>,
    editing: bool,
    dirty: bool,
    selection_mode: SelectionMode,
    selected_vertices: Vec<usize>,
    selected_face: Option<usize>,
    original_mesh: Option<EditableMesh>,
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
            selected_state: None,
            editing: false,
            dirty: false,
            selection_mode: SelectionMode::Vertex,
            selected_vertices: Vec::new(),
            selected_face: None,
            original_mesh: None,
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
            SelectionMode::Face => usize::from(state.selected_face.is_some()),
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
    state
        .selected_state
        .as_deref()
        .and_then(|key| catalog.get_state(state.selected, key))
        .or_else(|| {
            state
                .selected_state
                .is_none()
                .then(|| catalog.get(state.selected))
                .flatten()
        })
}

fn selected_mesh_mut<'a>(
    catalog: &'a mut ObjectCatalog,
    state: &CadSceneState,
) -> Option<&'a mut EditableMesh> {
    if let Some(key) = state.selected_state.as_deref() {
        catalog.get_state_mut(state.selected, key)
    } else {
        catalog.get_mut(state.selected)
    }
}

fn state_index(entry: &crate::core::object_assets::CatalogEntry, state: &CadSceneState) -> usize {
    state
        .selected_state
        .as_deref()
        .and_then(|key| entry.states.iter().position(|variant| variant.key == key))
        .map(|index| index + 1)
        .unwrap_or(0)
}

fn state_key_at(entry: &crate::core::object_assets::CatalogEntry, index: usize) -> Option<String> {
    index
        .checked_sub(1)
        .and_then(|variant_index| entry.states.get(variant_index))
        .map(|variant| variant.key.clone())
}

fn switch_to_object(state: &mut CadSceneState, id: ObjectId) {
    state.selected = id;
    state.selected_state = None;
    state.editing = false;
    state.dirty = false;
    state.original_mesh = None;
    state.selected_vertices.clear();
    state.selected_face = None;
    state.error = None;
    state.target = Vec3::ZERO;
    state.distance = 4.0;
}

fn switch_to_state(state: &mut CadSceneState, key: Option<String>) {
    state.selected_state = key;
    state.editing = false;
    state.dirty = false;
    state.original_mesh = None;
    state.selected_vertices.clear();
    state.selected_face = None;
    state.error = None;
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
        SelectionMode::Face => state
            .selected_face
            .and_then(|face| {
                let start = face.checked_mul(3)?;
                Some(
                    mesh.indices
                        .get(start..start + 3)?
                        .iter()
                        .map(|&index| index as usize)
                        .collect(),
                )
            })
            .unwrap_or_default(),
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
    let mut ui_capturing = sidebar.contains(ctx.mouse)
        || toolbar.contains(ctx.mouse)
        || view_panel.contains(ctx.mouse);
    let mut category_y = 68.0;
    for category in [
        ObjectCategory::Buildings,
        ObjectCategory::Units,
        ObjectCategory::MachineBlocks,
    ] {
        let rect = Rect::new(12.0, category_y, SIDEBAR_WIDTH - 24.0, 34.0);
        if button(ctx, rect, category.label()) {
            if state.dirty {
                state.error = Some("Save or discard edits before changing categories".to_string());
            } else {
                state.category = category;
                if let Some(first) = catalog
                    .entries()
                    .iter()
                    .find(|entry| entry.definition.category == category)
                {
                    switch_to_object(state, first.definition.id);
                }
            }
        }

        category_y += 40.0;
    }

    let mut object_y = category_y + 8.0;
    let category = state.category;
    for entry in catalog
        .entries()
        .iter()
        .filter(|entry| entry.definition.category == category)
    {
        let rect = Rect::new(12.0, object_y, SIDEBAR_WIDTH - 24.0, 32.0);
        if button(ctx, rect, entry.definition.label) {
            if !state.editing || !state.dirty {
                switch_to_object(state, entry.definition.id);
            } else {
                state.error = Some("Save or discard edits before switching objects".to_string());
            }
        }
        object_y += 36.0;
    }

    let mut x = SIDEBAR_WIDTH + 12.0;
    let top = Rect::new(x, 12.0, 86.0, 34.0);
    if button(ctx, top, "Back") {
        if state.dirty {
            state.error = Some("Save or discard edits before leaving CAD".to_string());
        } else {
            *scene = Scene::MainMenu;
        }
    }
    x += 94.0;
    let editing_label = if state.editing { "Editing" } else { "Edit" };
    if button(ctx, Rect::new(x, 12.0, 86.0, 34.0), editing_label) {
        if !state.editing {
            if let Some(mesh) = selected_mesh(catalog, state) {
                state.original_mesh = Some(mesh.clone());
                state.editing = true;
                state.dirty = false;
                state.error = None;
            } else {
                state.error = selected_entry(catalog, state.selected)
                    .and_then(|entry| {
                        state
                            .selected_state
                            .as_ref()
                            .and_then(|key| entry.states.iter().find(|variant| &variant.key == key))
                            .and_then(|variant| variant.error.clone())
                            .or_else(|| entry.error.clone())
                    })
                    .or_else(|| Some("Selected object is unavailable".to_string()));
            }
        }
    }
    x += 94.0;
    if state.editing && button(ctx, Rect::new(x, 12.0, 86.0, 34.0), "Vertex") {
        state.selection_mode = SelectionMode::Vertex;
        state.selected_face = None;
    }
    x += 94.0;
    if state.editing && button(ctx, Rect::new(x, 12.0, 86.0, 34.0), "Face") {
        state.selection_mode = SelectionMode::Face;
        state.selected_vertices.clear();
    }
    x += 94.0;
    if state.editing && button(ctx, Rect::new(x, 12.0, 86.0, 34.0), "Save") {
        let result = selected_mesh(catalog, state)
            .cloned()
            .ok_or_else(|| "Selected object is unavailable".to_string())
            .and_then(|mesh| match state.selected_state.as_deref() {
                Some(key) => catalog.save_state(state.selected, key, mesh),
                None => catalog.save(state.selected, mesh),
            });
        match result {
            Ok(()) => {
                state.dirty = false;
                state.original_mesh = selected_mesh(catalog, state).cloned();
                state.error = None;
            }
            Err(error) => state.error = Some(error),
        }
    }
    x += 94.0;
    if state.editing && button(ctx, Rect::new(x, 12.0, 86.0, 34.0), "Discard") {
        if let Some(original) = state.original_mesh.take() {
            if let Some(mesh) = selected_mesh_mut(catalog, state) {
                *mesh = original;
            }
        }
        state.editing = false;
        state.dirty = false;
        state.selected_vertices.clear();
        state.selected_face = None;
        state.error = None;
    }
    if let Some(entry) = selected_entry(catalog, state.selected)
        && !entry.states.is_empty()
    {
        x += 94.0;
        if button(ctx, Rect::new(x, 12.0, 42.0, 34.0), "<") {
            if !state.dirty {
                let count = entry.states.len() + 1;
                let previous = (state_index(entry, state) + count - 1) % count;
                switch_to_state(state, state_key_at(entry, previous));
            } else {
                state.error = Some("Save or discard edits before changing states".to_string());
            }
        }
        x += 50.0;
        if button(ctx, Rect::new(x, 12.0, 42.0, 34.0), ">") {
            if !state.dirty {
                let count = entry.states.len() + 1;
                let next = (state_index(entry, state) + 1) % count;
                switch_to_state(state, state_key_at(entry, next));
            } else {
                state.error = Some("Save or discard edits before changing states".to_string());
            }
        }
        x += 50.0;
        let current = state.selected_state.as_deref().unwrap_or("Base mesh");
        draw_text(current, x, 35.0, ctx.font_sm, ctx.colors_rt.text_secondary);
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
            if button(ctx, Rect::new(action_x, action_y, 46.0, 30.0), label) {
                if let Some(mesh) = selected_mesh_mut(catalog, state) {
                    if move_selected(mesh, state, offset) {
                        state.dirty = true;
                    } else {
                        state.error = Some("Select a vertex or face first".to_string());
                    }
                }
            }
            action_x += 50.0;
        }
        action_x += 8.0;
        if button(ctx, Rect::new(action_x, action_y, 72.0, 30.0), "Scale +") {
            if let Some(mesh) = selected_mesh_mut(catalog, state) {
                if scale_selected(mesh, state, 1.1) {
                    state.dirty = true;
                } else {
                    state.error = Some("Select a vertex or face first".to_string());
                }
            }
        }
        action_x += 76.0;
        if button(ctx, Rect::new(action_x, action_y, 72.0, 30.0), "Scale -") {
            if let Some(mesh) = selected_mesh_mut(catalog, state) {
                if scale_selected(mesh, state, 0.9) {
                    state.dirty = true;
                } else {
                    state.error = Some("Select a vertex or face first".to_string());
                }
            }
        }
        action_x += 76.0;
        if button(ctx, Rect::new(action_x, action_y, 84.0, 30.0), "Extrude") {
            if let Some(face) = state.selected_face {
                if let Some(mesh) = selected_mesh_mut(catalog, state) {
                    match mesh.extrude_face(face, 0.2) {
                        Ok(()) => {
                            state.dirty = true;
                            state.selected_face = Some(mesh.face_count().saturating_sub(7));
                        }
                        Err(error) => state.error = Some(error),
                    }
                }
            } else {
                state.error = Some("Select a face to extrude".to_string());
            }
        }
    }
    ui_capturing |= state.editing
        && Rect::new(SIDEBAR_WIDTH, action_y - 4.0, screen_width(), 40.0).contains(ctx.mouse);

    let camera = object_camera(state);
    let viewport_active = viewport.contains(ctx.mouse) && !ui_capturing;
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
            state.distance = (state.distance * (1.0 - wheel * 0.12))
                .clamp(CAMERA_MIN_DISTANCE, CAMERA_MAX_DISTANCE);
        }
    }
    state.last_mouse = ctx.mouse;
    let camera = object_camera(state);

    let selected_mesh = selected_mesh(catalog, state).cloned();
    let preview = selected_mesh.as_ref().map(preview_mesh);
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
            if let Some(face) = state.selected_face {
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
                    state.selected_face = None;
                }
                SelectionMode::Face => {
                    state.selected_face = mesh_ray_hit(origin, direction, mesh);
                    state.selected_vertices.clear();
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
        .map(|entry| match state.selected_state.as_deref() {
            Some(key) => format!("{} / {key}", entry.definition.label),
            None => entry.definition.label.to_string(),
        })
        .unwrap_or_else(|| "Object".to_string());
    draw_text(&title, 12.0, 36.0, ctx.font_lg, ctx.colors_rt.text_primary);
    if let Some(entry) = selected_entry(catalog, state.selected)
        && let Some(error) = state
            .selected_state
            .as_ref()
            .and_then(|key| entry.states.iter().find(|variant| &variant.key == key))
            .and_then(|variant| variant.error.as_ref())
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
