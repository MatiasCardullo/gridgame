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
}

#[derive(Clone, Copy, Debug)]
struct CadFigure {
    kind: CadPrimitiveKind,
    pos: Vec3,
    size: Vec3,
    rot: Vec3, // x/y/z radians
}

pub struct CadState {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    figures: Vec<CadFigure>,
    selected: Option<usize>,
    tool: CadTool,
    selected_handle: usize,
    selected_axis: usize,
    dragging_handle: bool,
    dragging_axis: bool,
    drag_last_mouse: Vec2,
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
            selected_handle: 0,
            selected_axis: 1,
            dragging_handle: false,
            dragging_axis: false,
            drag_last_mouse: Vec2::ZERO,
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

fn figure_basis(fig: &CadFigure) -> (Vec3, Vec3, Vec3) {
    let q = Quat::from_euler(EulerRot::XYZ, fig.rot.x, fig.rot.y, fig.rot.z);
    (q * Vec3::X, q * Vec3::Y, q * Vec3::Z)
}

fn local_to_world(fig: &CadFigure, local: Vec3) -> Vec3 {
    let (x, y, z) = figure_basis(fig);
    fig.pos + x * local.x + y * local.y + z * local.z
}

fn draw_oriented_box(fig: &CadFigure, fill: Color, wire: Color) {
    let (x_axis, y_axis, z_axis) = figure_basis(fig);
    let origin = fig.pos - x_axis * (fig.size.x * 0.5) - y_axis * (fig.size.y * 0.5) - z_axis * (fig.size.z * 0.5);
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

fn draw_figure(fig: &CadFigure, selected: bool) {
    let (fill, wire) = if selected {
        (Color::from_rgba(125, 210, 255, 255), Color::from_rgba(250, 255, 255, 255))
    } else {
        (Color::from_rgba(95, 145, 180, 215), Color::from_rgba(170, 200, 225, 255))
    };
    match fig.kind {
        CadPrimitiveKind::Square => draw_oriented_box(fig, fill, wire),
        CadPrimitiveKind::Cylinder => {
            // Prototype cylinder body; handles/resize/rotation use full local basis.
            let radius = (fig.size.x + fig.size.z) * 0.25;
            draw_cylinder_ex(
                vec3(fig.pos.x, fig.pos.y - fig.size.y * 0.5, fig.pos.z),
                radius,
                radius,
                fig.size.y,
                None,
                fill,
                DrawCylinderParams {
                    sides: 32,
                    draw_mode: DrawMode::Triangles,
                },
            );
            draw_cylinder_ex(
                vec3(fig.pos.x, fig.pos.y - fig.size.y * 0.5, fig.pos.z),
                radius,
                radius,
                fig.size.y,
                None,
                wire,
                DrawCylinderParams {
                    sides: 32,
                    draw_mode: DrawMode::Lines,
                },
            );
        }
    }
}

fn resize_handle_dirs() -> Vec<Vec3> {
    let mut dirs = Vec::new();
    for sx in [-1.0_f32, 0.0, 1.0] {
        for sy in [-1.0_f32, 0.0, 1.0] {
            for sz in [-1.0_f32, 0.0, 1.0] {
                if sx == 0.0 && sy == 0.0 && sz == 0.0 {
                    continue;
                }
                dirs.push(vec3(sx, sy, sz));
            }
        }
    }
    dirs
}

fn handle_world_positions(fig: &CadFigure) -> Vec<Vec3> {
    let dirs = resize_handle_dirs();
    let half = fig.size * 0.5;
    dirs.into_iter()
        .map(|d| local_to_world(fig, vec3(d.x * half.x, d.y * half.y, d.z * half.z)))
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

fn pick_closest_index(camera: &Camera3D, mouse: Vec2, points: &[Vec3], max_px: f32) -> Option<usize> {
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

fn apply_handle_local_delta(fig: &mut CadFigure, h: Vec3, local_delta: Vec3) {
    let min_size = 8.0;

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
    let (x, y, z) = figure_basis(fig);
    fig.pos += x * center_shift_local.x + y * center_shift_local.y + z * center_shift_local.z;
    fig.size = new_size;
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
    let panel = Rect::new(screen_width() - 290.0, 24.0, 266.0, 460.0);
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
                            state.selected_handle = handle_i;
                            state.dragging_handle = true;
                            state.drag_last_mouse = ctx.mouse;
                        }
                    }
                    CadTool::Rotate => {
                        let axes = axis_world_positions(fig);
                        if let Some(axis_i) =
                            pick_closest_index(&camera, ctx.mouse, &axes, 18.0)
                        {
                            state.selected_axis = axis_i;
                            state.dragging_axis = true;
                            state.drag_last_mouse = ctx.mouse;
                        }
                    }
                    CadTool::Move => {}
                }
            }
        }
        if !state.dragging_handle && !state.dragging_axis {
            let centers: Vec<Vec3> = state.figures.iter().map(|f| f.pos).collect();
            state.selected = pick_closest_index(&camera, ctx.mouse, &centers, 28.0);
        }
    }
    if is_mouse_button_released(MouseButton::Left) {
        state.dragging_handle = false;
        state.dragging_axis = false;
    }

    draw_rectangle(panel.x, panel.y, panel.w, panel.h, Color::from_rgba(21, 28, 36, 220));
    draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 1.5, colors.panel_border);
    draw_text("CAD Tools", panel.x + 10.0, panel.y + 24.0, ctx.font_sm, colors.text_primary);

    let mut y = panel.y + 36.0;
    let bw = panel.w - 20.0;
    let bh = 26.0;
    let (add_square, _) = ui_button(Rect::new(panel.x + 10.0, y, bw, bh), "Add Square", ctx.mouse, ctx.font_sm, ctx.button_colors);
    y += 30.0;
    let (add_cylinder, _) = ui_button(Rect::new(panel.x + 10.0, y, bw, bh), "Add Cylinder", ctx.mouse, ctx.font_sm, ctx.button_colors);
    y += 34.0;
    let (tool_move, _) = ui_button(Rect::new(panel.x + 10.0, y, bw, bh), "Move Figure", ctx.mouse, ctx.font_sm, ctx.button_colors);
    y += 30.0;
    let (tool_resize, _) = ui_button(Rect::new(panel.x + 10.0, y, bw, bh), "Resize Figure", ctx.mouse, ctx.font_sm, ctx.button_colors);
    y += 30.0;
    let (tool_rotate, _) = ui_button(Rect::new(panel.x + 10.0, y, bw, bh), "Rotate Figure", ctx.mouse, ctx.font_sm, ctx.button_colors);

    if add_square {
        let idx = state.figures.len() as f32;
        state.figures.push(CadFigure {
            kind: CadPrimitiveKind::Square,
            pos: vec3((idx % 4.0) * 22.0 - 33.0, 20.0, (idx / 4.0).floor() * 22.0 - 22.0),
            size: vec3(36.0, 36.0, 36.0),
            rot: vec3(0.0, 0.0, 0.0),
        });
        state.selected = Some(state.figures.len().saturating_sub(1));
    }
    if add_cylinder {
        let idx = state.figures.len() as f32;
        state.figures.push(CadFigure {
            kind: CadPrimitiveKind::Cylinder,
            pos: vec3((idx % 4.0) * 22.0 - 33.0, 20.0, (idx / 4.0).floor() * 22.0 - 22.0),
            size: vec3(30.0, 36.0, 30.0),
            rot: vec3(0.0, 0.0, 0.0),
        });
        state.selected = Some(state.figures.len().saturating_sub(1));
    }

    if state.figures.is_empty() {
        state.selected = None;
    } else if state.selected.is_none() {
        state.selected = Some(0);
    }

    if tool_move {
        state.tool = CadTool::Move;
    }
    if tool_resize {
        state.tool = CadTool::Resize;
    }
    if tool_rotate {
        state.tool = CadTool::Rotate;
    }

    if let Some(i) = state.selected {
        if let Some(fig) = state.figures.get_mut(i) {
            let dt = get_frame_time();
            match state.tool {
                CadTool::Move => apply_move_tool(fig, dt),
                CadTool::Resize => {
                    if state.dragging_handle && is_mouse_button_down(MouseButton::Left) {
                        let mouse_delta = ctx.mouse - state.drag_last_mouse;
                        state.drag_last_mouse = ctx.mouse;
                        let cam_fwd = (camera.target - camera.position).normalize_or_zero();
                        let cam_right = cam_fwd.cross(camera.up).normalize_or_zero();
                        let cam_up = camera.up.normalize_or_zero();
                        let world_move = cam_right * (mouse_delta.x * 0.12)
                            + cam_up * (-mouse_delta.y * 0.12)
                            + cam_fwd * (mouse_wheel().1 * 2.8);
                        let (x, y, z) = figure_basis(fig);
                        let local_delta = vec3(world_move.dot(x), world_move.dot(y), world_move.dot(z));
                        let dirs = resize_handle_dirs();
                        if !dirs.is_empty() {
                            let h = dirs[state.selected_handle % dirs.len()];
                            apply_handle_local_delta(fig, h, local_delta);
                        }
                    }
                }
                CadTool::Rotate => {
                    if state.dragging_axis && is_mouse_button_down(MouseButton::Left) {
                        let mouse_delta = ctx.mouse - state.drag_last_mouse;
                        state.drag_last_mouse = ctx.mouse;
                        let amount = (mouse_delta.x - mouse_delta.y) * 0.0085;
                        match state.selected_axis {
                            0 => fig.rot.x += amount,
                            1 => fig.rot.y += amount,
                            _ => fig.rot.z += amount,
                        }
                    }
                }
            }
        }
    }

    set_camera(&camera);
    for i in -12..=12 {
        let t = i as f32 * 12.0;
        draw_line_3d(vec3(t, 0.0, -144.0), vec3(t, 0.0, 144.0), Color::from_rgba(66, 77, 92, 255));
        draw_line_3d(vec3(-144.0, 0.0, t), vec3(144.0, 0.0, t), Color::from_rgba(66, 77, 92, 255));
    }
    draw_line_3d(vec3(-160.0, 0.0, 0.0), vec3(160.0, 0.0, 0.0), colors.hover);
    draw_line_3d(vec3(0.0, 0.0, -160.0), vec3(0.0, 0.0, 160.0), colors.text_secondary);
    for (i, fig) in state.figures.iter().enumerate() {
        draw_figure(fig, state.selected == Some(i));
    }
    if let Some(i) = state.selected {
        if let Some(fig) = state.figures.get(i) {
            if state.tool == CadTool::Resize {
                let handles = handle_world_positions(fig);
                for (h_i, pos) in handles.iter().enumerate() {
                    let color = if h_i == state.selected_handle {
                        colors.hover
                    } else {
                        Color::from_rgba(215, 230, 240, 255)
                    };
                    draw_cube(*pos, vec3(3.2, 3.2, 3.2), None, color);
                    draw_cube_wires(*pos, vec3(3.2, 3.2, 3.2), colors.text_primary);
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
                    let marker_color = if axis_i == state.selected_axis {
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
    };
    let selected_label = state.selected.map(|i| format!("{}", i + 1)).unwrap_or_else(|| "none".to_string());
    draw_text("CAD Prototype", 24.0, 36.0, ctx.font_md, colors.text_primary);
    draw_text(
        &format!("Figures: {} | Selected: {}", state.figures.len(), selected_label),
        24.0,
        62.0,
        ctx.font_sm,
        colors.text_secondary,
    );
    draw_text(
        &format!("Tool: {} | Handle: {} | Axis: {}", tool_label, state.selected_handle + 1, state.selected_axis + 1),
        24.0,
        84.0,
        ctx.font_sm,
        colors.text_secondary,
    );
    draw_text(
        "LMB select/drag gizmo | resize shows all handles | rotate shows xyz axes",
        24.0,
        106.0,
        ctx.font_sm,
        colors.text_secondary,
    );
}
