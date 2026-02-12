use macroquad::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::core::{FrameContext, Scene};
use crate::core::ui::ui_button;

const PLANET_DATA_DIR: &str = "planet_data";
const HEIGHTMAP_META_PATH: &str = "planet_data/planet_heightmap_meta.json";
const HEIGHTMAP_DATA_PATH: &str = "planet_data/planet_heightmap.rgba";
const MESH_POINTS_PATH: &str = "planet_data/planet_mesh_points.csv";
const PLANET_NOISE_PATH: &str = "planet_data/planet_noise.json";
const HEIGHTMAP_SIZE: u16 = 1024;

// Wraps an angle to the [-PI, PI] range for smooth camera interpolation.
fn wrap_angle(mut angle: f32) -> f32 {
    let two_pi = std::f32::consts::PI * 2.0;
    angle = (angle + std::f32::consts::PI) % two_pi;
    if angle < 0.0 {
        angle += two_pi;
    }
    angle - std::f32::consts::PI
}

// Rotates a vector around an axis using Rodrigues' rotation formula.
fn rotate_vec3(v: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let axis = axis.normalize();
    let cos_theta = angle.cos();
    let sin_theta = angle.sin();
    v * cos_theta + axis.cross(v) * sin_theta + axis * (axis.dot(v) * (1.0 - cos_theta))
}

// Builds the icosahedron edge list used for arcs and face construction.
fn icosahedron_edges() -> Vec<(usize, usize)> {
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for i in 2..6 {
        edges.push((0,i));
        edges.push((11,4+i));
    }
    for i in 1..5 {
        edges.push((i,i+5));
        edges.push((i,i+6));
    }
    edges.push((1,5));
    edges.push((5,10));
    edges.push((10,6));
    for i in 0..11 {
        edges.push((i,i+1));
    }
    edges
}

// Returns the 12 vertices of a unit icosahedron.
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

// Hashes a 3D integer coordinate to a deterministic [0,1] value.
fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut n = (x as i64) * 374761393
        + (y as i64) * 668265263
        + (z as i64) * 2147483647
        + seed as i64 * 69069;
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    let n = n ^ (n >> 16);
    (n as u32) as f32 / u32::MAX as f32
}

// Produces smooth value noise for a 3D position.
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

// Combines multiple octaves of value noise into fBm.
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
    #[serde(skip_serializing, default = "default_subdivisions")]
    pub subdivisions: u8,
}

impl Default for PlanetNoiseConfig {
    // Provides a baseline noise configuration for the planet.
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

fn default_subdivisions() -> u8 {
    4
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlanetNoiseSnapshot {
    config: PlanetNoiseConfig,
    sector_values: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HeightmapCacheMeta {
    config: PlanetNoiseConfig,
    texture_yaw: f32,
    texture_pitch: f32,
    size: u16,
}

pub struct PlanetBuildData {
    config: PlanetNoiseConfig,
    base_vertices: Vec<Vec3>,
    sector_vertices: Vec<Vec3>,
    sector_faces: Vec<[usize; 3]>,
    mesh_data: Vec<MeshData>,
    base_texture_mesh_data: Vec<MeshData>,
    face_normals: Vec<Vec3>,
    sector_values: Vec<f32>,
    heightmap_pixels: Vec<u8>,
    texture_yaw: f32,
    texture_pitch: f32,
}

fn ensure_planet_data_dir() {
    let _ = fs::create_dir_all(PLANET_DATA_DIR);
    let files = [
        ("planet_heightmap_meta.json", HEIGHTMAP_META_PATH),
        ("planet_heightmap.rgba", HEIGHTMAP_DATA_PATH),
        ("planet_mesh_points.csv", MESH_POINTS_PATH),
        ("planet_noise.json", PLANET_NOISE_PATH),
    ];
    for (old_name, new_path) in files.iter() {
        let old_path = std::path::Path::new(old_name);
        let new_path = std::path::Path::new(new_path);
        if old_path.exists() && !new_path.exists() {
            let _ = fs::rename(old_path, new_path);
        }
    }
}

enum PlanetLoadEvent {
    StepStart(String),
    StepDone { name: String, elapsed_ms: u128 },
    Progress { done: u32, total: u32 },
    LogLine(String),
    Done(PlanetBuildData),
    Error(String),
}

enum PlanetLoadStatus {
    Loading,
    Done,
    Error,
}

pub struct PlanetLoader {
    status: PlanetLoadStatus,
    progress: f32,
    current_step: Option<String>,
    log_entries: Vec<String>,
    rx: Option<Receiver<PlanetLoadEvent>>,
    started_at: Instant,
    pending_data: Option<PlanetBuildData>,
    error: Option<String>,
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
    pub relief_meshes: Vec<Mesh>,
    pub base_texture_meshes: Vec<Mesh>,
    pub face_normals: Vec<Vec3>,
    pub face_centers: Vec<Vec3>,
    pub base_vertices: Vec<Vec3>,
    pub sector_vertices: Vec<Vec3>,
    pub sector_faces: Vec<[usize; 3]>,
    pub sector_values: Vec<f32>,
    pub printed_midpoints: bool,
    pub show_relief: bool,
    pub heightmap_texture: Option<Texture2D>,
    pub texture_yaw: f32,
    pub texture_pitch: f32,
    pub regen_rx: Option<Receiver<RegenMessage>>,
    pub regen_in_progress: bool,
    pub regen_pending: bool,
    pub regen_version: u64,
    pub regen_expected_chunks: usize,
    pub regen_received_chunks: usize,
}

impl PlanetState {
    // Creates a fresh planet state with default noise and meshes.
    pub fn new() -> Self {
        let texture_yaw = 90.0_f32.to_radians();
        let texture_pitch = 32.0_f32.to_radians();
        let data = build_planet_data(
            PlanetNoiseConfig::default(),
            texture_yaw,
            texture_pitch,
            HEIGHTMAP_SIZE,
        );
        Self::from_build_data(data)
    }

    pub fn from_build_data(data: PlanetBuildData) -> Self {
        let mut heightmap_texture =
            Texture2D::from_rgba8(HEIGHTMAP_SIZE, HEIGHTMAP_SIZE, &data.heightmap_pixels);
        heightmap_texture.set_filter(FilterMode::Linear);
        let relief_meshes = data
            .mesh_data
            .into_iter()
            .map(mesh_data_to_mesh)
            .collect::<Vec<_>>();
        let mut base_texture_meshes = data
            .base_texture_mesh_data
            .into_iter()
            .map(mesh_data_to_mesh)
            .collect::<Vec<_>>();
        let mut value = Some(heightmap_texture.clone());
        for mesh in base_texture_meshes.iter_mut() {
            mesh.texture = value.clone();
        }
        let face_centers = build_face_centers(&data.sector_vertices, &data.sector_faces);
        Self {
            yaw: 0.0,
            pitch: 0.3,
            distance: 6.0,
            target_distance: 6.0,
            target_yaw: 0.0,
            target_pitch: 0.3,
            dragging: false,
            last_mouse: Vec2::ZERO,
            config: data.config,
            relief_meshes,
            base_texture_meshes,
            face_normals: data.face_normals,
            face_centers,
            base_vertices: data.base_vertices,
            sector_vertices: data.sector_vertices,
            sector_faces: data.sector_faces,
            sector_values: data.sector_values,
            printed_midpoints: false,
            show_relief: true,
            heightmap_texture: Some(heightmap_texture),
            texture_yaw: data.texture_yaw,
            texture_pitch: data.texture_pitch,
            regen_rx: None,
            regen_in_progress: false,
            regen_pending: false,
            regen_version: 0,
            regen_expected_chunks: 0,
            regen_received_chunks: 0,
        }
    }

    // Rebuilds planet meshes from current noise settings.
    fn regenerate(&mut self, size: u16) {
        self.request_regen();
        let _ = size;
    }

    // Persists the current height snapshot to a JSON file.
    fn save_heightmap(&self, size: u16, path: &str) {
        let snapshot = PlanetNoiseSnapshot {
            config: self.config,
            sector_values: self.sector_values.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
            ensure_planet_data_dir();
            let _ = fs::write(path, json);
        }
        let _ = size;
    }

    // Loads a height snapshot from JSON and regenerates meshes.
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

    // Requests a background regeneration of meshes.
    fn request_regen(&mut self) {
        if self.regen_in_progress {
            self.regen_pending = true;
            return;
        }
        self.regen_version = self.regen_version.wrapping_add(1);
        self.start_regen(self.regen_version);
    }

    fn start_regen(&mut self, version: u64) {
        let (tx, rx) = mpsc::channel();
        let base_vertices = Arc::new(self.sector_vertices.clone());
        let faces = Arc::new(self.sector_faces.clone());
        let config = self.config;
        let subdivisions = self.config.subdivisions as usize;
        let texture_size = HEIGHTMAP_SIZE;
        let align = alignment_axis_angle(&icosahedron_vertices());
        let texture_yaw = self.texture_yaw;
        let texture_pitch = self.texture_pitch;
        let face_count = faces.len();
        let workers = std::thread::available_parallelism()
            .map(|v| v.get())
            .unwrap_or(1)
            .min(face_count.max(1));
        let chunk_size = (face_count + workers - 1) / workers;

        self.regen_expected_chunks = workers;
        self.regen_received_chunks = 0;

        let tx_texture = tx.clone();
        std::thread::spawn(move || {
            let pixels = load_or_build_heightmap(
                &config,
                texture_size,
                align,
                texture_yaw,
                texture_pitch,
            );
            let _ = tx_texture.send(RegenMessage::Texture {
                version,
                pixels,
                size: texture_size,
            });
        });

        for worker in 0..workers {
            let start = worker * chunk_size;
            if start >= face_count {
                break;
            }
            let end = (start + chunk_size).min(face_count);
            let base_vertices = Arc::clone(&base_vertices);
            let faces = Arc::clone(&faces);
            let tx = tx.clone();
            let config = config;
            std::thread::spawn(move || {
                let slice = &faces[start..end];
                let (mesh_data, face_normals, sector_values) = build_planet_chunk_data_for_faces(
                    &base_vertices,
                    slice,
                    subdivisions,
                    1.6,
                    &config,
                    true,
                    false,
                    align,
                    texture_yaw,
                    texture_pitch,
                );
                let _ = tx.send(RegenMessage::MeshChunk {
                    version,
                    start,
                    mesh_data,
                    face_normals,
                    sector_values,
                });
            });
        }
        self.regen_rx = Some(rx);
        self.regen_in_progress = true;
        self.regen_pending = false;
    }

    fn poll_regen(&mut self) {
        let Some(rx) = &self.regen_rx else {
            return;
        };
        let mut received_any = false;
        while let Ok(message) = rx.try_recv() {
            received_any = true;
            match message {
                RegenMessage::Texture { version, pixels, size } => {
                    if version != self.regen_version {
                        continue;
                    }
                    let tex = Texture2D::from_rgba8(size, size, &pixels);
                    tex.set_filter(FilterMode::Linear);
                    self.heightmap_texture = Some(tex);
                    for mesh in self.base_texture_meshes.iter_mut() {
                        mesh.texture = self.heightmap_texture.clone();
                    }
                }
                RegenMessage::MeshChunk {
                    version,
                    start,
                    mesh_data,
                    face_normals,
                    sector_values,
                } => {
                    if version != self.regen_version {
                        continue;
                    }
                    for (offset, data) in mesh_data.into_iter().enumerate() {
                        let mesh = mesh_data_to_mesh(data);
                        if start + offset < self.relief_meshes.len() {
                            self.relief_meshes[start + offset] = mesh;
                        }
                    }
                    for (offset, normal) in face_normals.into_iter().enumerate() {
                        if start + offset < self.face_normals.len() {
                            self.face_normals[start + offset] = normal;
                        }
                    }
                    for (offset, value) in sector_values.into_iter().enumerate() {
                        if start + offset < self.sector_values.len() {
                            self.sector_values[start + offset] = value;
                        }
                    }
                    self.regen_received_chunks += 1;
                }
            }
        }

        if received_any
            && self.regen_expected_chunks > 0
            && self.regen_received_chunks >= self.regen_expected_chunks
        {
            save_mesh_points_from_meshes(&self.relief_meshes);
            self.regen_rx = None;
            self.regen_in_progress = false;
            if self.regen_pending {
                self.regen_pending = false;
                self.regen_version = self.regen_version.wrapping_add(1);
                self.start_regen(self.regen_version);
            }
        }
    }
}

impl PlanetLoader {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut log_file = File::create("loading.log")
                .ok()
                .map(BufWriter::new);
            let total_steps: u32 = 4;
            let mut done_steps: u32 = 0;

            fn send_log(
                tx: &mpsc::Sender<PlanetLoadEvent>,
                log_file: &mut Option<BufWriter<File>>,
                line: String,
            ) {
                if let Some(file) = log_file.as_mut() {
                    let _ = writeln!(file, "{line}");
                    let _ = file.flush();
                }
                let _ = tx.send(PlanetLoadEvent::LogLine(line));
            }

            fn step_start(
                tx: &mpsc::Sender<PlanetLoadEvent>,
                log_file: &mut Option<BufWriter<File>>,
                name: &str,
            ) -> Instant {
                let line = format!(
                    "{} Step start: {}",
                    format_log_timestamp(SystemTime::now()),
                    name
                );
                send_log(tx, log_file, line);
                let _ = tx.send(PlanetLoadEvent::StepStart(name.to_string()));
                Instant::now()
            }

            fn step_done(
                tx: &mpsc::Sender<PlanetLoadEvent>,
                log_file: &mut Option<BufWriter<File>>,
                name: &str,
                start: Instant,
                done_steps: &mut u32,
                total_steps: u32,
            ) {
                let elapsed_ms = start.elapsed().as_millis();
                let line = format!(
                    "{} Step done: {} ({} ms)",
                    format_log_timestamp(SystemTime::now()),
                    name,
                    elapsed_ms
                );
                send_log(tx, log_file, line);
                let _ = tx.send(PlanetLoadEvent::StepDone {
                    name: name.to_string(),
                    elapsed_ms,
                });
                *done_steps += 1;
                let _ = tx.send(PlanetLoadEvent::Progress {
                    done: *done_steps,
                    total: total_steps,
                });
            }

            let texture_yaw = 90.0_f32.to_radians();
            let texture_pitch = 32.0_f32.to_radians();
            let mut config = PlanetNoiseConfig::default();
            let mut loaded_sector_values: Option<Vec<f32>> = None;
            if let Ok(contents) = fs::read_to_string(PLANET_NOISE_PATH) {
                if let Ok(snapshot) = serde_json::from_str::<PlanetNoiseSnapshot>(&contents) {
                    config = snapshot.config;
                    loaded_sector_values = Some(snapshot.sector_values);
                }
            }

            let geometry_start = step_start(&tx, &mut log_file, "Build geometry");
            let base_vertices = align_vertices_to_poles(&icosahedron_vertices());
            let base_faces = build_faces(&base_vertices, &icosahedron_edges());
            let (sector_vertices, sector_faces) =
                build_geodesic_sphere(&base_vertices, &base_faces, 4, 0, 1.0);
            let align = alignment_axis_angle(&icosahedron_vertices());
            step_done(
                &tx,
                &mut log_file,
                "Build geometry",
                geometry_start,
                &mut done_steps,
                total_steps,
            );

            let heightmap_start = step_start(&tx, &mut log_file, "Load/build heightmap");
            ensure_planet_data_dir();
            let heightmap_pixels =
                load_or_build_heightmap(&config, HEIGHTMAP_SIZE, align, texture_yaw, texture_pitch);
            step_done(
                &tx,
                &mut log_file,
                "Load/build heightmap",
                heightmap_start,
                &mut done_steps,
                total_steps,
            );

            let mesh_start = step_start(&tx, &mut log_file, "Build mesh data");
            let (mesh_data, face_normals, sector_values) = build_planet_chunk_data(
                &sector_vertices,
                &sector_faces,
                config.subdivisions as usize,
                1.6,
                &config,
                true,
                false,
                align,
                0.0,
                0.0,
            );
            let (base_texture_mesh_data, _, _) = build_planet_chunk_data(
                &sector_vertices,
                &sector_faces,
                2,
                1.6,
                &config,
                false,
                true,
                align,
                texture_yaw,
                texture_pitch,
            );
            let sector_values = loaded_sector_values.unwrap_or(sector_values);
            step_done(
                &tx,
                &mut log_file,
                "Build mesh data",
                mesh_start,
                &mut done_steps,
                total_steps,
            );

            let save_start = step_start(&tx, &mut log_file, "Save mesh points");
            save_mesh_points(&mesh_data);
            step_done(
                &tx,
                &mut log_file,
                "Save mesh points",
                save_start,
                &mut done_steps,
                total_steps,
            );

            let data = PlanetBuildData {
                config,
                base_vertices,
                sector_vertices,
                sector_faces,
                mesh_data,
                base_texture_mesh_data,
                face_normals,
                sector_values,
                heightmap_pixels,
                texture_yaw,
                texture_pitch,
            };

            let _ = tx.send(PlanetLoadEvent::Done(data));
        });

        Self {
            status: PlanetLoadStatus::Loading,
            progress: 0.0,
            current_step: None,
            log_entries: Vec::new(),
            rx: Some(rx),
            started_at: Instant::now(),
            pending_data: None,
            error: None,
        }
    }

    pub fn poll(&mut self) -> Option<PlanetBuildData> {
        let Some(rx) = self.rx.as_ref() else {
            return self.pending_data.take();
        };
        let mut pending_logs: Vec<String> = Vec::new();
        while let Ok(message) = rx.try_recv() {
            match message {
                PlanetLoadEvent::StepStart(name) => {
                    self.current_step = Some(name);
                }
                PlanetLoadEvent::StepDone { .. } => {}
                PlanetLoadEvent::Progress { done, total } => {
                    let total = total.max(1);
                    self.progress = (done as f32 / total as f32).clamp(0.0, 1.0);
                }
                PlanetLoadEvent::LogLine(line) => {
                    pending_logs.push(line);
                }
                PlanetLoadEvent::Done(data) => {
                    self.status = PlanetLoadStatus::Done;
                    self.progress = 1.0;
                    self.pending_data = Some(data);
                }
                PlanetLoadEvent::Error(message) => {
                    self.status = PlanetLoadStatus::Error;
                    self.error = Some(message);
                }
            }
        }
        for line in pending_logs {
            self.push_log(line);
        }
        self.pending_data.take()
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.status, PlanetLoadStatus::Done)
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.status, PlanetLoadStatus::Loading)
    }

    pub fn progress(&self) -> f32 {
        self.progress
    }

    pub fn log_entries(&self) -> &[String] {
        self.log_entries.as_slice()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn push_log(&mut self, line: String) {
        const MAX_LINES: usize = 12;
        self.log_entries.push(line);
        if self.log_entries.len() > MAX_LINES {
            let excess = self.log_entries.len() - MAX_LINES;
            self.log_entries.drain(0..excess);
        }
    }
}

fn build_planet_data(
    mut config: PlanetNoiseConfig,
    texture_yaw: f32,
    texture_pitch: f32,
    texture_size: u16,
) -> PlanetBuildData {
    ensure_planet_data_dir();
    let mut loaded_sector_values: Option<Vec<f32>> = None;
    if let Ok(contents) = fs::read_to_string(PLANET_NOISE_PATH) {
        if let Ok(snapshot) = serde_json::from_str::<PlanetNoiseSnapshot>(&contents) {
            config = snapshot.config;
            loaded_sector_values = Some(snapshot.sector_values);
        }
    }
    let base_vertices = align_vertices_to_poles(&icosahedron_vertices());
    let base_faces = build_faces(&base_vertices, &icosahedron_edges());
    let (sector_vertices, sector_faces) =
        build_geodesic_sphere(&base_vertices, &base_faces, 4, 0, 1.0);
    let align = alignment_axis_angle(&icosahedron_vertices());
    let heightmap_pixels =
        load_or_build_heightmap(&config, texture_size, align, texture_yaw, texture_pitch);
    let (mesh_data, face_normals, sector_values) = build_planet_chunk_data(
        &sector_vertices,
        &sector_faces,
        config.subdivisions as usize,
        1.6,
        &config,
        true,
        false,
        align,
        0.0,
        0.0,
    );
    let (base_texture_mesh_data, _, _) = build_planet_chunk_data(
        &sector_vertices,
        &sector_faces,
        2,
        1.6,
        &config,
        false,
        true,
        align,
        texture_yaw,
        texture_pitch,
    );
    let sector_values = loaded_sector_values.unwrap_or(sector_values);
    save_mesh_points(&mesh_data);
    PlanetBuildData {
        config,
        base_vertices,
        sector_vertices,
        sector_faces,
        mesh_data,
        base_texture_mesh_data,
        face_normals,
        sector_values,
        heightmap_pixels,
        texture_yaw,
        texture_pitch,
    }
}

fn format_log_timestamp(now: SystemTime) -> String {
    let since_epoch = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = since_epoch % 86_400;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("[{:02}:{:02}:{:02}]", h, m, s)
}

// Calculates a height value for a given surface normal.
fn height_value(normal: Vec3, config: &PlanetNoiseConfig) -> f32 {
    let noise = fbm3(
        normal.x * config.noise_scale,
        normal.y * config.noise_scale,
        normal.z * config.noise_scale,
        1337,
    );
    (noise * config.height_amp + config.height_bias + normal.y * config.lat_bias).clamp(0.0, 1.0)
}

// Maps height and latitude to a terrain color.
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

// Subdivides triangle faces by inserting normalized midpoints.
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

// Subdivides the base icosahedron faces into 4 using shared midpoints.
fn subdivide_base_faces(
    vertices: &[Vec3],
    faces: &[[usize; 3]],
) -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let mut out_vertices: Vec<Vec3> = vertices.iter().map(|v| v.normalize()).collect();
    let mut mid_cache: HashMap<(usize, usize), usize> = HashMap::new();
    let mut new_faces = Vec::with_capacity(faces.len() * 4);

    let mut midpoint = |a: usize, b: usize, verts: &mut Vec<Vec3>| -> usize {
        let key = if a < b { (a, b) } else { (b, a) };
        if let Some(&idx) = mid_cache.get(&key) {
            return idx;
        }
        let mid = (verts[a] + verts[b]) * 0.5;
        let idx = verts.len();
        verts.push(mid.normalize());
        mid_cache.insert(key, idx);
        idx
    };

    for face in faces {
        let a = face[0];
        let b = face[1];
        let c = face[2];
        let ab: usize = midpoint(a, b, &mut out_vertices);
        let bc = midpoint(b, c, &mut out_vertices);
        let ca = midpoint(c, a, &mut out_vertices);
        new_faces.push([a, ab, ca]);
        new_faces.push([b, bc, ab]);
        new_faces.push([c, ca, bc]);
        new_faces.push([ab, bc, ca]);
    }

    (out_vertices, new_faces)
}

// Builds a geodesic sphere by subdividing and relaxing vertices on the unit sphere.
fn build_geodesic_sphere(
    base_vertices: &[Vec3],
    base_faces: &[[usize; 3]],
    subdivisions: usize,
    relax_iterations: usize,
    relax_strength: f32,
) -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let mut vertices: Vec<Vec3> = base_vertices.iter().map(|v| v.normalize()).collect();
    let mut faces: Vec<[usize; 3]> = base_faces.to_vec();

    for _ in 0..subdivisions {
        let (new_vertices, new_faces) = subdivide_base_faces(&vertices, &faces);
        vertices = new_vertices;
        faces = new_faces;
    }

    if relax_iterations > 0 {
        //relax_sphere(&mut vertices, &faces, relax_iterations, relax_strength);
    }

    (vertices, faces)
}

// Relaxes vertex positions along the sphere to reduce edge length variance.
fn relax_sphere(
    vertices: &mut [Vec3],
    faces: &[[usize; 3]],
    iterations: usize,
    strength: f32,
) {
    let neighbors = build_vertex_neighbors(faces, vertices.len());
    let strength = strength.clamp(0.0, 1.0);

    for _ in 0..iterations {
        let mut next = vertices.to_vec();
        for (index, pos) in vertices.iter().enumerate() {
            let neighbor_list = &neighbors[index];
            if neighbor_list.is_empty() {
                continue;
            }
            let mut avg = Vec3::ZERO;
            for &neighbor in neighbor_list.iter() {
                avg += vertices[neighbor];
            }
            avg /= neighbor_list.len() as f32;
            let target = avg.normalize();
            let moved = *pos + (target - *pos) * strength;
            next[index] = moved.normalize();
        }
        vertices.copy_from_slice(&next);
    }
}

// Builds adjacency lists for each vertex from face indices.
fn build_vertex_neighbors(faces: &[[usize; 3]], vertex_count: usize) -> Vec<Vec<usize>> {
    let mut neighbors: Vec<HashSet<usize>> = (0..vertex_count).map(|_| HashSet::new()).collect();

    for face in faces {
        let a = face[0];
        let b = face[1];
        let c = face[2];
        neighbors[a].insert(b);
        neighbors[a].insert(c);
        neighbors[b].insert(a);
        neighbors[b].insert(c);
        neighbors[c].insert(a);
        neighbors[c].insert(b);
    }

    neighbors
        .into_iter()
        .map(|set| set.into_iter().collect())
        .collect()
}

#[derive(Debug)]
struct MeshData {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}

#[derive(Debug)]
enum RegenMessage {
    MeshChunk {
        version: u64,
        start: usize,
        mesh_data: Vec<MeshData>,
        face_normals: Vec<Vec3>,
        sector_values: Vec<f32>,
    },
    Texture {
        version: u64,
        pixels: Vec<u8>,
        size: u16,
    },
}

fn mesh_data_to_mesh(data: MeshData) -> Mesh {
    Mesh {
        vertices: data.vertices,
        indices: data.indices,
        texture: None,
    }
}

// Builds mesh data for a set of triangle faces.
fn build_planet_chunk_data_for_faces(
    base_vertices: &[Vec3],
    faces_in: &[[usize; 3]],
    subdivisions: usize,
    radius: f32,
    config: &PlanetNoiseConfig,
    use_relief: bool,
    use_texture: bool,
    align: Option<(Vec3, f32)>,
    texture_yaw: f32,
    texture_pitch: f32,
) -> (Vec<MeshData>, Vec<Vec3>, Vec<f32>) {
    let mut meshes: Vec<MeshData> = Vec::with_capacity(faces_in.len());
    let mut face_normals: Vec<Vec3> = Vec::with_capacity(faces_in.len());

    for face in faces_in.iter().copied() {
        let mut vertices = vec![
            base_vertices[face[0]],
            base_vertices[face[1]],
            base_vertices[face[2]],
        ];
        let mut faces = vec![[0usize, 1usize, 2usize]];
        for _ in 0..subdivisions {
            faces = subdivide_faces(&mut vertices, &faces);
        }

        let mut positions: Vec<Vec3> = Vec::with_capacity(vertices.len());
        let mut normals: Vec<Vec3> = Vec::with_capacity(vertices.len());
        let mut heights: Vec<f32> = Vec::with_capacity(vertices.len());
    for v in vertices.iter() {
        let normal = v.normalize();
        let height = height_value(normal, config);
        let sea_level = config.sea_level;
        let elevation = if use_relief {
            if height < sea_level {
                0.0
            } else {
                (height - sea_level) * 0.28
            }
        } else {
            0.0
        };
        positions.push(normal * (radius + elevation));
        normals.push(normal);
        heights.push(height);
    }

    let mut mesh_vertices: Vec<Vertex> = Vec::with_capacity(faces.len() * 3);
    let mut indices: Vec<u16> = Vec::with_capacity(faces.len() * 3);
    for f in faces {
        let idxs = [f[0], f[1], f[2]];
        if use_relief {
            let avg_height = (heights[idxs[0]] + heights[idxs[1]] + heights[idxs[2]]) / 3.0;
            if avg_height < config.sea_level {
                continue;
            }
        }
        let mut uvs = [
            uv_from_normal_oriented(normals[idxs[0]], align, texture_yaw, texture_pitch),
            uv_from_normal_oriented(normals[idxs[1]], align, texture_yaw, texture_pitch),
            uv_from_normal_oriented(normals[idxs[2]], align, texture_yaw, texture_pitch),
            ];
            if use_texture {
                let min_u = uvs[0].x.min(uvs[1].x.min(uvs[2].x));
                let max_u = uvs[0].x.max(uvs[1].x.max(uvs[2].x));
                if (max_u - min_u) > 0.5 {
                    for uv in uvs.iter_mut() {
                        if uv.x < 0.5 {
                            uv.x += 1.0;
                        }
                    }
                }
            }

            for i in 0..3 {
                let normal = normals[idxs[i]];
                let height = heights[idxs[i]];
                let color = if use_texture {
                    Color::new(1.0, 1.0, 1.0, 1.0)
                } else {
                    color_from_height(height, normal, config)
                };
                let uv = if use_texture { uvs[i] } else { vec2(0.0, 0.0) };
                mesh_vertices.push(Vertex::new2(positions[idxs[i]], uv, color));
                indices.push((mesh_vertices.len() - 1) as u16);
            }
        }

        meshes.push(MeshData {
            vertices: mesh_vertices,
            indices,
        });
        face_normals.push(face_normal(base_vertices, face).normalize());
    }

    let sector_values = faces_in
        .iter()
        .map(|face| {
            let normal = face_normal(base_vertices, *face).normalize();
            height_value(normal, config)
        })
        .collect();

    (meshes, face_normals, sector_values)
}

// Builds mesh data for each base triangle face of the icosahedron.
fn build_planet_chunk_data(
    base_vertices: &[Vec3],
    sector_faces: &[[usize; 3]],
    subdivisions: usize,
    radius: f32,
    config: &PlanetNoiseConfig,
    use_relief: bool,
    use_texture: bool,
    align: Option<(Vec3, f32)>,
    texture_yaw: f32,
    texture_pitch: f32,
) -> (Vec<MeshData>, Vec<Vec3>, Vec<f32>) {
    build_planet_chunk_data_for_faces(
        base_vertices,
        sector_faces,
        subdivisions,
        radius,
        config,
        use_relief,
        use_texture,
        align,
        texture_yaw,
        texture_pitch,
    )
}

fn build_planet_chunk_data_parallel(
    base_vertices: Arc<Vec<Vec3>>,
    faces: Arc<Vec<[usize; 3]>>,
    subdivisions: usize,
    radius: f32,
    config: PlanetNoiseConfig,
    use_relief: bool,
    use_texture: bool,
    align: Option<(Vec3, f32)>,
    texture_yaw: f32,
    texture_pitch: f32,
) -> (Vec<MeshData>, Vec<Vec3>, Vec<f32>) {
    let face_count = faces.len();
    if face_count == 0 {
        return (Vec::new(), Vec::new(), Vec::new());
    }

    let workers = std::thread::available_parallelism()
        .map(|v| v.get())
        .unwrap_or(1)
        .min(face_count);
    let chunk_size = (face_count + workers - 1) / workers;

    let mut handles = Vec::with_capacity(workers);
    for worker in 0..workers {
        let start = worker * chunk_size;
        if start >= face_count {
            break;
        }
        let end = (start + chunk_size).min(face_count);
        let base_vertices = Arc::clone(&base_vertices);
        let faces = Arc::clone(&faces);
        let handle = std::thread::spawn(move || {
            let slice = &faces[start..end];
            let (meshes, normals, values) = build_planet_chunk_data_for_faces(
                &base_vertices,
                slice,
                subdivisions,
                radius,
                &config,
                use_relief,
                use_texture,
                align,
                texture_yaw,
                texture_pitch,
            );
            (start, meshes, normals, values)
        });
        handles.push(handle);
    }

    let mut meshes: Vec<Option<MeshData>> = (0..face_count).map(|_| None).collect();
    let mut face_normals: Vec<Option<Vec3>> = vec![None; face_count];
    let mut sector_values: Vec<Option<f32>> = vec![None; face_count];

    for handle in handles {
        let (start, chunk_meshes, chunk_normals, chunk_values) =
            handle.join().unwrap_or((0, Vec::new(), Vec::new(), Vec::new()));
        for (offset, mesh) in chunk_meshes.into_iter().enumerate() {
            meshes[start + offset] = Some(mesh);
        }
        for (offset, normal) in chunk_normals.into_iter().enumerate() {
            face_normals[start + offset] = Some(normal);
        }
        for (offset, value) in chunk_values.into_iter().enumerate() {
            sector_values[start + offset] = Some(value);
        }
    }

    let meshes = meshes.into_iter().map(|m| m.unwrap()).collect();
    let face_normals = face_normals.into_iter().map(|n| n.unwrap()).collect();
    let sector_values = sector_values.into_iter().map(|v| v.unwrap()).collect();
    (meshes, face_normals, sector_values)
}

// Computes a point along the great-circle arc between two directions.
fn great_circle_point(start: Vec3, end: Vec3, t: f32) -> Vec3 {
    let start = start.normalize();
    let end = end.normalize();
    let dot = start.dot(end).clamp(-1.0, 1.0);
    let theta = dot.acos();
    let sin_theta = theta.sin();
    if sin_theta.abs() < 1e-5 {
        (start + (end - start) * t).normalize()
    } else {
        let w0 = ((1.0 - t) * theta).sin() / sin_theta;
        let w1 = (t * theta).sin() / sin_theta;
        (start * w0 + end * w1).normalize()
    }
}

// Draws a polyline polygon on the sphere surface around a point.
fn draw_polygon_on_sphere(center: Vec3, radius: f32, sides: usize, size: f32, color: Color) {
    if sides < 3 {
        return;
    }
    let normal = center.normalize();
    let mut tangent = normal.cross(vec3(0.0, 1.0, 0.0));
    if tangent.length() < 1e-4 {
        tangent = normal.cross(vec3(1.0, 0.0, 0.0));
    }
    tangent = tangent.normalize();
    let bitangent = normal.cross(tangent).normalize();

    let mut prev = Vec3::ZERO;
    for i in 0..=sides {
        let angle = (i as f32 / sides as f32) * std::f32::consts::TAU;
        let offset = tangent * angle.sin() + bitangent * angle.cos();
        let point = (normal * radius + offset * size);
        if i > 0 {
            draw_line_3d(prev, point, color);
        }
        prev = point;
    }
}

// Draws a great-circle arc between two points on the sphere.
fn draw_arc_on_sphere(start: Vec3, end: Vec3, radius: f32, segments: usize, color: Color) {
    let start = start.normalize();
    let end = end.normalize();
    let dot = start.dot(end).clamp(-1.0, 1.0);
    let theta = dot.acos();
    let sin_theta = theta.sin();
    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        let p0 = great_circle_point(start, end, t0) * radius;
        let p1 = great_circle_point(start, end, t1) * radius;
        draw_line_3d(p0, p1, color);
    }
}

// Builds triangle faces from the icosahedron edges via planar checks.
fn build_faces(vertices: &[Vec3], edges: &[(usize, usize)]) -> Vec<[usize; 3]> {
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

// Builds a ray from screen-space mouse coordinates in world space.
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

// Intersects a ray with a sphere, returning the nearest hit point.
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

// Finds the face whose normal most aligns with the hit direction.
fn hovered_face(
    hit_point: Vec3,
    faces: &[[usize; 3]],
    vertices: &[Vec3],
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

// Computes a consistent outward normal for a face.
fn face_normal(vertices: &[Vec3], face: [usize; 3]) -> Vec3 {
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

// Computes unit face centers for frustum checks and highlighting.
fn build_face_centers(vertices: &[Vec3], faces: &[[usize; 3]]) -> Vec<Vec3> {
    faces
        .iter()
        .map(|face| {
            let a = vertices[face[0]];
            let b = vertices[face[1]];
            let c = vertices[face[2]];
            (a + b + c).normalize()
        })
        .collect()
}

// Rotates vertices so one vertex aligns with the +Y axis.
fn alignment_axis_angle(vertices: &[Vec3; 12]) -> Option<(Vec3, f32)> {
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
        return None;
    }
    Some((axis.normalize(), angle))
}

fn align_vertices_to_poles(vertices: &[Vec3; 12]) -> Vec<Vec3> {
    let Some((axis, angle)) = alignment_axis_angle(vertices) else {
        return vertices.to_vec();
    };
    let mut out = vec![Vec3::ZERO; vertices.len()];
    for (index, v) in vertices.iter().enumerate() {
        out[index] = rotate_vec3(*v, axis, angle);
    }
    out
}

// Projects a 3D point into screen space and clips off-screen points.
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

// Checks whether a point lies within the camera frustum (with optional padding).
fn point_in_frustum(camera: &Camera3D, point: Vec3, pad: f32) -> bool {
    let mat = camera.matrix();
    let clip = mat * vec4(point.x, point.y, point.z, 1.0);
    if clip.w <= 0.0 {
        return false;
    }
    let ndc = vec3(clip.x, clip.y, clip.z) / clip.w;
    let limit = 1.0 + pad;
    ndc.x.abs() <= limit && ndc.y.abs() <= limit && ndc.z >= -limit && ndc.z <= limit
}

// Converts a normal direction to equirectangular UVs.
fn uv_from_normal(normal: Vec3) -> Vec2 {
    let n = normal.normalize();
    let u = 0.5 + n.z.atan2(n.x) / std::f32::consts::TAU;
    let v = 0.5 - n.y.asin() / std::f32::consts::PI;
    vec2(u, v)
}

fn apply_align(normal: Vec3, align: Option<(Vec3, f32)>) -> Vec3 {
    if let Some((axis, angle)) = align {
        rotate_vec3(normal, axis, angle)
    } else {
        normal
    }
}

fn uv_from_normal_oriented(
    normal: Vec3,
    align: Option<(Vec3, f32)>,
    texture_yaw: f32,
    texture_pitch: f32,
) -> Vec2 {
    let n = apply_align(normal.normalize(), align);
    let pole_axis = rotate_vec3(vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), texture_pitch);
    let n = rotate_vec3(n, vec3(1.0, 0.0, 0.0), texture_pitch);
    let n = rotate_vec3(n, pole_axis, texture_yaw);
    uv_from_normal(n)
}

// Builds an RGBA heightmap texture using equirectangular sampling.
fn build_heightmap_pixels(
    config: &PlanetNoiseConfig,
    size: u16,
    align: Option<(Vec3, f32)>,
    texture_yaw: f32,
    texture_pitch: f32,
) -> Vec<u8> {
    let size = size as usize;
    let mut pixels = vec![0u8; size * size * 4];
    for y in 0..size {
        let v = y as f32 / (size - 1) as f32;
        let phi = v * std::f32::consts::PI;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        for x in 0..size {
            let u = x as f32 / (size - 1) as f32;
            let theta = u * std::f32::consts::TAU;
            let mut normal = vec3(sin_phi * theta.cos(), cos_phi, sin_phi * theta.sin());
            if let Some((axis, angle)) = align {
                normal = rotate_vec3(normal, axis, angle);
            }
            let pole_axis = rotate_vec3(vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), texture_pitch);
            normal = rotate_vec3(normal, vec3(1.0, 0.0, 0.0), texture_pitch);
            normal = rotate_vec3(normal, pole_axis, texture_yaw);
            let height = height_value(normal, config);
            let color = color_from_height(height, normal, config);
            let idx = (y * size + x) * 4;
            pixels[idx] = (color.r * 255.0) as u8;
            pixels[idx + 1] = (color.g * 255.0) as u8;
            pixels[idx + 2] = (color.b * 255.0) as u8;
            pixels[idx + 3] = (color.a * 255.0) as u8;
        }
    }
    pixels
}

fn config_matches(a: &PlanetNoiseConfig, b: &PlanetNoiseConfig) -> bool {
    let eps = 1e-4;
    (a.noise_scale - b.noise_scale).abs() < eps
        && (a.height_amp - b.height_amp).abs() < eps
        && (a.height_bias - b.height_bias).abs() < eps
        && (a.lat_bias - b.lat_bias).abs() < eps
        && (a.sea_level - b.sea_level).abs() < eps
        && (a.ice_start - b.ice_start).abs() < eps
        && (a.ice_strength - b.ice_strength).abs() < eps
        && a.subdivisions == b.subdivisions
}

fn load_heightmap_cache(
    config: &PlanetNoiseConfig,
    size: u16,
    texture_yaw: f32,
    texture_pitch: f32,
) -> Option<Vec<u8>> {
    ensure_planet_data_dir();
    let Ok(meta_raw) = fs::read_to_string(HEIGHTMAP_META_PATH) else {
        return None;
    };
    let Ok(meta) = serde_json::from_str::<HeightmapCacheMeta>(&meta_raw) else {
        return None;
    };
    if meta.size != size
        || !config_matches(&meta.config, config)
        || (meta.texture_yaw - texture_yaw).abs() > 1e-4
        || (meta.texture_pitch - texture_pitch).abs() > 1e-4
    {
        return None;
    }
    let Ok(pixels) = fs::read(HEIGHTMAP_DATA_PATH) else {
        return None;
    };
    if pixels.len() != size as usize * size as usize * 4 {
        return None;
    }
    Some(pixels)
}

fn save_heightmap_cache(
    config: &PlanetNoiseConfig,
    size: u16,
    texture_yaw: f32,
    texture_pitch: f32,
    pixels: &[u8],
) {
    ensure_planet_data_dir();
    let meta = HeightmapCacheMeta {
        config: *config,
        texture_yaw,
        texture_pitch,
        size,
    };
    if let Ok(json) = serde_json::to_string_pretty(&meta) {
        let _ = fs::write(HEIGHTMAP_META_PATH, json);
    }
    let _ = fs::write(HEIGHTMAP_DATA_PATH, pixels);
}

fn load_or_build_heightmap(
    config: &PlanetNoiseConfig,
    size: u16,
    align: Option<(Vec3, f32)>,
    texture_yaw: f32,
    texture_pitch: f32,
) -> Vec<u8> {
    if let Some(pixels) = load_heightmap_cache(config, size, texture_yaw, texture_pitch) {
        return pixels;
    }
    let pixels = build_heightmap_pixels(config, size, align, texture_yaw, texture_pitch);
    save_heightmap_cache(config, size, texture_yaw, texture_pitch, &pixels);
    pixels
}

fn save_mesh_points(meshes: &[MeshData]) {
    ensure_planet_data_dir();
    let Ok(file) = File::create(MESH_POINTS_PATH) else {
        return;
    };
    let mut writer = BufWriter::new(file);
    let _ = writeln!(writer, "x,y,z");
    for mesh in meshes {
        for v in mesh.vertices.iter() {
            let _ = writeln!(writer, "{},{},{}", v.position.x, v.position.y, v.position.z);
        }
    }
}

fn save_mesh_points_from_meshes(meshes: &[Mesh]) {
    let Ok(file) = File::create(MESH_POINTS_PATH) else {
        return;
    };
    let mut writer = BufWriter::new(file);
    let _ = writeln!(writer, "x,y,z");
    for mesh in meshes {
        for v in mesh.vertices.iter() {
            let _ = writeln!(writer, "{},{},{}", v.position.x, v.position.y, v.position.z);
        }
    }
}

// Maps camera distance to mesh subdivision detail levels.
fn zoom_to_subdivisions(distance: f32) -> u8 {
    if distance < 4.0 {
        5
    } else if distance < 5.0 {
        4
    } else if distance < 7.0 {
        3
    } else if distance < 9.0 {
        2
    } else {
        1
    }
}

// Runs the planet scene frame update and rendering.
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
    let desired_subdivisions = zoom_to_subdivisions(state.distance);
    if desired_subdivisions != state.config.subdivisions {
        state.config.subdivisions = desired_subdivisions;
        state.regenerate(512);
    }
    state.poll_regen();
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
    let line_radius = radius * 1.04;
    let zoom_t = ((state.distance - 2.0) / 18.0).clamp(0.0, 1.0);
    let segments = ((32.0 - 16.0 * zoom_t).round() as i32).clamp(10, 32) as usize;
    let pent_size = 0.02 * (1.0 - 0.6 * zoom_t);
    let hex_size = 0.018 * (1.0 - 0.6 * zoom_t);
    let show_labels = false;
    let show_hexes = zoom_t < 0.75;
    let camera_dir = camera_pos.normalize();
    for mesh in state.base_texture_meshes.iter() {
        if mesh.vertices.is_empty() {
            continue;
        }
        draw_mesh(mesh);
    }
    if state.show_relief {
        for (index, mesh) in state.relief_meshes.iter().enumerate() {
            if mesh.vertices.is_empty() {
                continue;
            }
        if let Some(center) = state.face_centers.get(index) {
            if !point_in_frustum(&camera, *center * radius, 0.6) {
                continue;
            }
        }
        if let Some(normal) = state.face_normals.get(index) {
            if normal.dot(camera_dir) <= 0.0 {
                continue;
            }
        }
        draw_mesh(mesh);
        }
    }
    
    let vertices = &state.sector_vertices;
    let base_vertices = &state.base_vertices;
    let edges = icosahedron_edges();
    let faces = state.sector_faces.clone();
    let point_color = Color::from_rgba(240, 240, 255, 255);
    let edge_color = Color::from_rgba(200, 220, 250, 255);
    let hover_color = Color::from_rgba(255, 200, 120, 255);
    let mid_color = Color::from_rgba(180, 255, 220, 255);

    let mut projected_base: Vec<Vec3> = vec![Vec3::ZERO; base_vertices.len()];
    let mut projected_sector: Vec<Vec3> = vec![Vec3::ZERO; vertices.len()];
    let mut screen_points: Vec<Option<Vec2>> = vec![None; base_vertices.len()];
    for (index, v) in base_vertices.iter().enumerate() {
        projected_base[index] = v.normalize() * line_radius;
        draw_polygon_on_sphere(projected_base[index], line_radius, 5, pent_size, point_color);
        screen_points[index] = project_to_screen(&camera, projected_base[index]);
        if !state.printed_midpoints {
            println!(
                "Vertex pent: ({:.3}, {:.3}, {:.3})",
                projected_base[index].x, projected_base[index].y, projected_base[index].z
            );
        }   
    }
    for (index, v) in vertices.iter().enumerate() {
        projected_sector[index] = v.normalize() * line_radius;
    }

    for (a, b) in edges.iter().copied() {
        draw_arc_on_sphere(projected_base[a], projected_base[b], line_radius, segments, edge_color);
        let mid_dir = great_circle_point(projected_base[a], projected_base[b], 0.5);
        let mid_point = mid_dir * line_radius;
        if show_hexes {
            draw_polygon_on_sphere(mid_point, line_radius, 6, hex_size, mid_color);
        }
        if !state.printed_midpoints {
            println!(
                "Midpoint hex: ({:.3}, {:.3}, {:.3})",
                mid_point.x, mid_point.y, mid_point.z
            );
        }
    }
    state.printed_midpoints = true;

    let mut hovered: Option<usize> = None;
    if let Some((origin, dir)) = ray_from_mouse(&camera, mouse) {
        if let Some(hit) = ray_sphere_intersection(origin, dir, radius) {
            hovered = hovered_face(hit, &faces, vertices);
        }
    }
    if let Some(face_index) = hovered {
        let face = faces[face_index];
        let a = projected_sector[face[0]];
        let b = projected_sector[face[1]];
        let c = projected_sector[face[2]];
        draw_arc_on_sphere(a, b, line_radius, segments, hover_color);
        draw_arc_on_sphere(b, c, line_radius, segments, hover_color);
        draw_arc_on_sphere(c, a, line_radius, segments, hover_color);
    }
    if let Some(face_index) = hovered {
        if is_mouse_button_pressed(MouseButton::Left) {
            let face = faces[face_index];
            let normal = face_normal(vertices, face);
            state.target_yaw = normal.z.atan2(normal.x);
            state.target_pitch = normal.y.asin();
        }
    }

    set_default_camera();
    draw_planet_controls(ctx, state);
    if show_labels {
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
}

// Draws the planet configuration UI panel.
fn draw_planet_controls(ctx: &FrameContext, state: &mut PlanetState) {
    let panel = Rect::new(18.0, 18.0, 260.0, 254.0);
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

    let rect_toggle = Rect::new(x, y, 230.0, 26.0);
    let (clicked_toggle, _) = ui_button(
        rect_toggle,
        if state.show_relief { "Relief: On" } else { "Relief: Off" },
        ctx.mouse,
        ctx.font_sm,
        ctx.button_colors,
    );
    if clicked_toggle {
        state.show_relief = !state.show_relief;
    }
    y += step;


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

    if changed {
        state.regenerate(512);
    }
    if clicked_load {
        state.load_heightmap(PLANET_NOISE_PATH);
    }
    if clicked_save {
        state.save_heightmap(512, PLANET_NOISE_PATH);
    }
}

// Draws a labeled +/- row for numeric configuration tweaks.
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
