use macroquad::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::mpsc::{self, Receiver};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use crate::core::{FrameContext, Scene};
use crate::SQRT_3;
use crate::core::debug::{
    draw_planet_controls, format_log_timestamp, planet_perf_log, PLANET_DEBUG_UI_ENABLED_DEFAULT,
    SAVE_MESH_POINTS_RUNTIME,
};

const PLANET_DATA_DIR: &str = "planet_data";
const MESH_POINTS_PATH: &str = "planet_data/planet_mesh_points.csv";
const PLANET_NOISE_PATH: &str = "planet_data/planet_noise.json";
const HEIGHTMAP_IMAGE_PREFIX: &str = "planet_data/planet_heightmap";
const HEIGHTMAP_SIZE: u16 = 2048;
const TEXTURE_BASE_SUBDIVISIONS: usize = 2;
const TEXTURE_LAYER_OFFSET: f32 = 0.004;
const SUBDIVISION_HYSTERESIS: f32 = 0.25;
const FALLBACK_RELIEF_SUBDIVISIONS: usize = 1;
const REGEN_FACE_GRAIN: usize = 12;
const ICOSAHEDRON_FACE_COUNT: usize = 20;
const SUBFACE_HOVER_MAX_DISTANCE: f32 = 5.6;
const NEAR_GRID_DISTANCE: f32 = 2.4;
const HOVER_GRID_HEXES_ACROSS: i32 = 20;
const HOVER_GRID_CELL_BASE_LIFT: f32 = 0.006;
const HOVER_GRID_CELL_MID_EXTRA_LIFT: f32 = 0.003;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlanetNoiseSnapshot {
    config: PlanetNoiseConfig,
    sector_values: Vec<f32>,
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
}

pub struct PlanetTextureData {
    config: PlanetNoiseConfig,
    base_vertices: Vec<Vec3>,
    sector_vertices: Vec<Vec3>,
    sector_faces: Vec<[usize; 3]>,
    base_texture_mesh_data: Vec<MeshData>,
    heightmap_pixels: Vec<u8>,
}

// Ensures the planet data folder exists and migrates legacy files into it.
fn ensure_planet_data_dir() {
    let _ = fs::create_dir_all(PLANET_DATA_DIR);
    let files = [
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
    StepDone,
    Progress { done: u32, total: u32 },
    LogLine(String),
    TextureReady(PlanetTextureData),
    Done(PlanetBuildData),
}

enum PlanetLoadStatus {
    Loading,
    Done,
}

pub struct PlanetLoader {
    status: PlanetLoadStatus,
    progress: f32,
    current_step: Option<String>,
    log_entries: Vec<String>,
    rx: Option<Receiver<PlanetLoadEvent>>,
    pending_texture_data: Option<PlanetTextureData>,
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
    pub fallback_relief_meshes: Vec<Mesh>,
    pub base_texture_meshes: Vec<Mesh>,
    pub face_normals: Vec<Vec3>,
    pub face_centers: Vec<Vec3>,
    pub base_vertices: Vec<Vec3>,
    pub sector_vertices: Vec<Vec3>,
    pub sector_faces: Vec<[usize; 3]>,
    pub sector_values: Vec<f32>,
    pub show_relief: bool,
    pub relief_available: bool,
    pub debug_enabled: bool,
    pub heightmap_texture: Option<Texture2D>,
    pub texture_config: PlanetNoiseConfig,
    relief_cache: HashMap<ReliefCacheKey, ReliefCacheEntry>,
    pending_relief_key: Option<ReliefCacheKey>,
    regen_rx: Option<Receiver<RegenMessage>>,
    pub regen_in_progress: bool,
    pub regen_pending: bool,
    pub regen_version: u64,
    pub regen_expected_chunks: usize,
    pub regen_received_chunks: usize,
    last_hitch_log_at: f64,
    regen_generation: Arc<AtomicU64>,
}

impl PlanetState {
    pub fn from_build_data(data: PlanetBuildData) -> Self {
        let PlanetBuildData {
            config,
            base_vertices,
            sector_vertices,
            sector_faces,
            mesh_data,
            base_texture_mesh_data,
            face_normals,
            sector_values,
            heightmap_pixels,
        } = data;
        let texture_data = PlanetTextureData {
            config,
            base_vertices,
            sector_vertices,
            sector_faces,
            base_texture_mesh_data,
            heightmap_pixels,
        };
        let mut state = Self::from_texture_data(texture_data);
        state.apply_relief_payload(config, mesh_data, face_normals, sector_values);
        state
    }

    pub fn from_texture_data(data: PlanetTextureData) -> Self {
        let PlanetTextureData {
            config,
            base_vertices,
            sector_vertices,
            sector_faces,
            base_texture_mesh_data,
            heightmap_pixels,
        } = data;
        let heightmap_texture =
            Texture2D::from_rgba8(HEIGHTMAP_SIZE, HEIGHTMAP_SIZE, &heightmap_pixels);
        heightmap_texture.set_filter(FilterMode::Linear);
        let mut base_texture_meshes = base_texture_mesh_data
            .into_iter()
            .map(mesh_data_to_mesh)
            .collect::<Vec<_>>();
        let value = Some(heightmap_texture.clone());
        for mesh in base_texture_meshes.iter_mut() {
            mesh.texture = value.clone();
        }
        let face_normals = sector_faces
            .iter()
            .copied()
            .map(|face| face_normal(&sector_vertices, face))
            .collect::<Vec<_>>();
        let face_centers = build_face_centers(&sector_vertices, &sector_faces);
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
            relief_meshes: Vec::new(),
            fallback_relief_meshes: Vec::new(),
            base_texture_meshes,
            face_normals,
            face_centers,
            base_vertices,
            sector_vertices,
            sector_faces,
            sector_values: Vec::new(),
            show_relief: false,
            relief_available: false,
            debug_enabled: PLANET_DEBUG_UI_ENABLED_DEFAULT,
            heightmap_texture: Some(heightmap_texture),
            texture_config: config,
            relief_cache: HashMap::new(),
            pending_relief_key: None,
            regen_rx: None,
            regen_in_progress: false,
            regen_pending: false,
            regen_version: 0,
            regen_expected_chunks: 0,
            regen_received_chunks: 0,
            last_hitch_log_at: 0.0,
            regen_generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn apply_relief_build_data(&mut self, data: PlanetBuildData) {
        let PlanetBuildData {
            config,
            mesh_data,
            face_normals,
            sector_values,
            ..
        } = data;
        self.apply_relief_payload(config, mesh_data, face_normals, sector_values);
    }

    fn apply_relief_payload(
        &mut self,
        config: PlanetNoiseConfig,
        mesh_data: Vec<MeshData>,
        face_normals: Vec<Vec3>,
        sector_values: Vec<f32>,
    ) {
        self.config = config;
        self.texture_config = config;
        self.relief_meshes = mesh_data.into_iter().map(mesh_data_to_mesh).collect();
        self.face_normals = face_normals;
        self.sector_values = sector_values;
        self.rebuild_fallback_relief_meshes();
        self.relief_cache.clear();
        self.relief_cache.insert(
            ReliefCacheKey::from_config(&self.config, self.config.subdivisions),
            ReliefCacheEntry {
                meshes: clone_meshes(&self.relief_meshes),
                face_normals: self.face_normals.clone(),
                sector_values: self.sector_values.clone(),
            },
        );
        self.relief_available = true;
    }

    // Rebuilds planet meshes from current noise settings.
    fn regenerate(&mut self, size: u16) {
        self.request_regen();
        let _ = size;
    }

    fn apply_relief_cache_entry(&mut self, key: ReliefCacheKey) -> bool {
        let t0 = Instant::now();
        let Some(mut entry) = self.relief_cache.remove(&key) else {
            return false;
        };
        std::mem::swap(&mut self.relief_meshes, &mut entry.meshes);
        std::mem::swap(&mut self.face_normals, &mut entry.face_normals);
        std::mem::swap(&mut self.sector_values, &mut entry.sector_values);
        self.relief_cache.insert(key, entry);
        planet_perf_log(&format!(
            "apply cache entry subdiv={} meshes={} took_ms={}",
            self.config.subdivisions,
            self.relief_meshes.len(),
            t0.elapsed().as_millis()
        ));
        true
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
        self.relief_cache.clear();
        self.pending_relief_key = None;
        self.rebuild_fallback_relief_meshes();
        self.regenerate(512);
    }

    pub(crate) fn debug_on_controls_changed(&mut self) {
        self.relief_cache.clear();
        self.pending_relief_key = None;
        self.rebuild_fallback_relief_meshes();
        self.regenerate(512);
    }

    pub(crate) fn debug_load_heightmap(&mut self, path: &str) {
        self.load_heightmap(path);
    }

    pub(crate) fn debug_save_heightmap(&self, path: &str) {
        self.save_heightmap(512, path);
    }

    // Rebuilds the low-detail fallback relief mesh and reapplies current texture.
    fn rebuild_fallback_relief_meshes(&mut self) {
        let (fallback_mesh_data, _, _) = build_planet_chunk_data(
            &self.sector_vertices,
            &self.sector_faces,
            FALLBACK_RELIEF_SUBDIVISIONS,
            1.6,
            &self.config,
            true,
            true,
        );
        self.fallback_relief_meshes = fallback_mesh_data
            .into_iter()
            .map(mesh_data_to_mesh)
            .collect();
        for mesh in self.fallback_relief_meshes.iter_mut() {
            mesh.texture = self.heightmap_texture.clone();
        }
    }

    // Requests a background regeneration of meshes.
    fn request_regen(&mut self) {
        if self.regen_in_progress {
            planet_perf_log(&format!(
                "regen preempt old_version={} new_subdiv={}",
                self.regen_version, self.config.subdivisions
            ));
        }
        self.regen_version = self.regen_version.wrapping_add(1);
        self.regen_generation
            .store(self.regen_version, Ordering::Relaxed);
        planet_perf_log(&format!(
            "regen start version={} subdiv={}",
            self.regen_version, self.config.subdivisions
        ));
        self.start_regen(self.regen_version);
    }

    // Starts async regeneration jobs for relief chunks and (when needed) texture.
    fn start_regen(&mut self, version: u64) {
        let config = self.config;
        let subdivisions = self.config.subdivisions as usize;
        let relief_key = ReliefCacheKey::from_config(&config, self.config.subdivisions);
        let texture_size = HEIGHTMAP_SIZE;
        let regen_generation = Arc::clone(&self.regen_generation);
        let refresh_texture =
            !texture_params_match(&self.texture_config, &config) || self.heightmap_texture.is_none();

        if self.relief_cache.contains_key(&relief_key) {
            planet_perf_log(&format!(
                "regen cache hit version={} subdiv={} refresh_texture={}",
                version, self.config.subdivisions, refresh_texture
            ));
            let _ = self.apply_relief_cache_entry(relief_key);
            if !refresh_texture {
                self.regen_rx = None;
                self.regen_in_progress = false;
                self.regen_pending = false;
                self.regen_expected_chunks = 0;
                self.regen_received_chunks = 0;
                return;
            }
            let (tx, rx) = mpsc::channel();
            self.regen_expected_chunks = 1;
            self.regen_received_chunks = 0;
            self.pending_relief_key = None;
            let tx_texture = tx.clone();
            std::thread::spawn(move || {
                let pixels = load_or_build_heightmap(&config, texture_size);
                let _ = tx_texture.send(RegenMessage::Texture {
                    version,
                    pixels,
                    size: texture_size,
                });
            });
            self.regen_rx = Some(rx);
            self.regen_in_progress = true;
            self.regen_pending = false;
            return;
        }

        let (tx, rx) = mpsc::channel();
        let base_vertices = Arc::new(self.sector_vertices.clone());
        let faces = Arc::new(self.sector_faces.clone());
        let face_count = faces.len();
        let workers = std::thread::available_parallelism()
            .map(|v| v.get())
            .unwrap_or(1)
            .min(face_count.max(1));
        let chunk_size = face_count.div_ceil(workers);
        let mut mesh_chunks = 0usize;
        for worker in 0..workers {
            let start = worker * chunk_size;
            if start >= face_count {
                break;
            }
            let end = (start + chunk_size).min(face_count);
            mesh_chunks += (end - start).div_ceil(REGEN_FACE_GRAIN);
        }

        self.regen_expected_chunks = mesh_chunks + if refresh_texture { 1 } else { 0 };
        self.regen_received_chunks = 0;
        self.pending_relief_key = Some(relief_key);
        planet_perf_log(&format!(
            "regen cache miss version={} subdiv={} workers={} chunks={} refresh_texture={}",
            version, self.config.subdivisions, workers, self.regen_expected_chunks, refresh_texture
        ));

        if refresh_texture {
            let tx_texture = tx.clone();
            let regen_generation = Arc::clone(&regen_generation);
            std::thread::spawn(move || {
                if regen_generation.load(Ordering::Relaxed) != version {
                    return;
                }
                let pixels = load_or_build_heightmap(&config, texture_size);
                if regen_generation.load(Ordering::Relaxed) != version {
                    return;
                }
                let _ = tx_texture.send(RegenMessage::Texture {
                    version,
                    pixels,
                    size: texture_size,
                });
            });
        }

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
            let regen_generation = Arc::clone(&regen_generation);
            std::thread::spawn(move || {
                let mut local_start = start;
                while local_start < end {
                    if regen_generation.load(Ordering::Relaxed) != version {
                        return;
                    }
                    let local_end = (local_start + REGEN_FACE_GRAIN).min(end);
                    let slice = &faces[local_start..local_end];
                    let (mesh_data, face_normals, sector_values) = build_planet_chunk_data_for_faces(
                        &base_vertices,
                        slice,
                        subdivisions,
                        1.6,
                        &config,
                        true,
                        false,
                    );
                    if regen_generation.load(Ordering::Relaxed) != version {
                        return;
                    }
                    let _ = tx.send(RegenMessage::MeshChunk {
                        version,
                        start: local_start,
                        mesh_data,
                        face_normals,
                        sector_values,
                    });
                    local_start = local_end;
                }
            });
        }
        self.regen_rx = Some(rx);
        self.regen_in_progress = true;
        self.regen_pending = false;
    }

    // Polls finished async jobs and swaps generated data into active meshes/texture.
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
                    for mesh in self.fallback_relief_meshes.iter_mut() {
                        mesh.texture = self.heightmap_texture.clone();
                    }
                    self.texture_config = self.config;
                    self.regen_received_chunks += 1;
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
                        let mesh_index = start + offset;
                        let mesh = mesh_data_to_mesh(data);
                        if mesh_index < self.relief_meshes.len() {
                            self.relief_meshes[mesh_index] = mesh;
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
            let finalize_t0 = Instant::now();
            if let Some(cache_key) = self.pending_relief_key.take() {
                self.relief_cache.insert(
                    cache_key,
                    ReliefCacheEntry {
                        meshes: clone_meshes(&self.relief_meshes),
                        face_normals: self.face_normals.clone(),
                        sector_values: self.sector_values.clone(),
                    },
                );
            }
            if SAVE_MESH_POINTS_RUNTIME {
                let save_t0 = Instant::now();
                save_mesh_points_from_meshes(&self.relief_meshes);
                planet_perf_log(&format!(
                    "save mesh points on regen took_ms={}",
                    save_t0.elapsed().as_millis()
                ));
            }
            planet_perf_log(&format!(
                "regen completed version={} subdiv={} took_finalize_ms={} cache_size={}",
                self.regen_version,
                self.config.subdivisions,
                finalize_t0.elapsed().as_millis(),
                self.relief_cache.len()
            ));
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
            let total_steps: u32 = 5;
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
                let _ = tx.send(PlanetLoadEvent::StepDone);
                *done_steps += 1;
                let _ = tx.send(PlanetLoadEvent::Progress {
                    done: *done_steps,
                    total: total_steps,
                });
            }

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
                build_geodesic_sphere(&base_vertices, &base_faces, 4, 10, 1.0);
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
            let heightmap_pixels = load_or_build_heightmap(&config, HEIGHTMAP_SIZE);
            step_done(
                &tx,
                &mut log_file,
                "Load/build heightmap",
                heightmap_start,
                &mut done_steps,
                total_steps,
            );

            let texture_mesh_start = step_start(&tx, &mut log_file, "Build texture mesh data");
            let (base_texture_mesh_data, _, _) = build_planet_chunk_data(
                &sector_vertices,
                &sector_faces,
                TEXTURE_BASE_SUBDIVISIONS,
                1.6 + TEXTURE_LAYER_OFFSET,
                &config,
                false,
                true,
            );
            step_done(
                &tx,
                &mut log_file,
                "Build texture mesh data",
                texture_mesh_start,
                &mut done_steps,
                total_steps,
            );

            let texture_data = PlanetTextureData {
                config,
                base_vertices: base_vertices.clone(),
                sector_vertices: sector_vertices.clone(),
                sector_faces: sector_faces.clone(),
                base_texture_mesh_data: base_texture_mesh_data.clone(),
                heightmap_pixels: heightmap_pixels.clone(),
            };
            let _ = tx.send(PlanetLoadEvent::TextureReady(texture_data));

            let relief_start = step_start(&tx, &mut log_file, "Build relief mesh data");
            let (mesh_data, face_normals, sector_values_raw) = build_planet_chunk_data(
                &sector_vertices,
                &sector_faces,
                config.subdivisions as usize,
                1.6,
                &config,
                true,
                false,
            );
            let sector_values = loaded_sector_values.unwrap_or(sector_values_raw);
            step_done(
                &tx,
                &mut log_file,
                "Build relief mesh data",
                relief_start,
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
            };

            let _ = tx.send(PlanetLoadEvent::Done(data));
        });

        Self {
            status: PlanetLoadStatus::Loading,
            progress: 0.0,
            current_step: None,
            log_entries: Vec::new(),
            rx: Some(rx),
            pending_texture_data: None,
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
                PlanetLoadEvent::StepDone => {}
                PlanetLoadEvent::Progress { done, total } => {
                    let total = total.max(1);
                    self.progress = (done as f32 / total as f32).clamp(0.0, 1.0);
                }
                PlanetLoadEvent::LogLine(line) => {
                    pending_logs.push(line);
                }
                PlanetLoadEvent::TextureReady(data) => {
                    self.pending_texture_data = Some(data);
                }
                PlanetLoadEvent::Done(data) => {
                    self.status = PlanetLoadStatus::Done;
                    self.progress = 1.0;
                    self.pending_data = Some(data);
                }
            }
        }
        for line in pending_logs {
            self.push_log(line);
        }
        self.pending_data.take()
    }

    pub fn take_texture_data(&mut self) -> Option<PlanetTextureData> {
        self.pending_texture_data.take()
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
        relax_sphere(&mut vertices, &faces, relax_iterations, relax_strength);
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

#[derive(Debug, Clone)]
struct MeshData {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}

struct ReliefCacheEntry {
    meshes: Vec<Mesh>,
    face_normals: Vec<Vec3>,
    sector_values: Vec<f32>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct ReliefCacheKey {
    subdivisions: u8,
    noise_scale: u32,
    height_amp: u32,
    height_bias: u32,
    lat_bias: u32,
    sea_level: u32,
    ice_start: u32,
    ice_strength: u32,
}

impl ReliefCacheKey {
    fn from_config(config: &PlanetNoiseConfig, subdivisions: u8) -> Self {
        Self {
            subdivisions,
            noise_scale: config.noise_scale.to_bits(),
            height_amp: config.height_amp.to_bits(),
            height_bias: config.height_bias.to_bits(),
            lat_bias: config.lat_bias.to_bits(),
            sea_level: config.sea_level.to_bits(),
            ice_start: config.ice_start.to_bits(),
            ice_strength: config.ice_strength.to_bits(),
        }
    }
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

fn clone_mesh(mesh: &Mesh) -> Mesh {
    Mesh {
        vertices: mesh.vertices.clone(),
        indices: mesh.indices.clone(),
        texture: mesh.texture.clone(),
    }
}

fn clone_meshes(meshes: &[Mesh]) -> Vec<Mesh> {
    meshes.iter().map(clone_mesh).collect()
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
                uv_from_normal(normals[idxs[0]]),
                uv_from_normal(normals[idxs[1]]),
                uv_from_normal(normals[idxs[2]]),
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
) -> (Vec<MeshData>, Vec<Vec3>, Vec<f32>) {
    build_planet_chunk_data_for_faces(
        base_vertices,
        sector_faces,
        subdivisions,
        radius,
        config,
        use_relief,
        use_texture,
    )
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

// Converts axial hex coordinates to 2D plane coordinates.
fn axial_to_plane(q: i32, r: i32, size: f32) -> Vec2 {
    let q = q as f32;
    let r = r as f32;
    let x = size * (SQRT_3 * q + (SQRT_3 / 2.0) * r);
    let y = size * (1.5 * r);
    vec2(x, y)
}

fn point_in_triangle_2d(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let eps = 1e-5;
    let s1 = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    let s2 = (c.x - b.x) * (p.y - b.y) - (c.y - b.y) * (p.x - b.x);
    let s3 = (a.x - c.x) * (p.y - c.y) - (a.y - c.y) * (p.x - c.x);
    let has_neg = s1 < -eps || s2 < -eps || s3 < -eps;
    let has_pos = s1 > eps || s2 > eps || s3 > eps;
    !(has_neg && has_pos)
}

fn barycentric_coords_2d(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> Option<(f32, f32, f32)> {
    let denom = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
    if denom.abs() < 1e-6 {
        return None;
    }
    let wa = ((b.y - c.y) * (p.x - c.x) + (c.x - b.x) * (p.y - c.y)) / denom;
    let wb = ((c.y - a.y) * (p.x - c.x) + (a.x - c.x) * (p.y - c.y)) / denom;
    let wc = 1.0 - wa - wb;
    Some((wa, wb, wc))
}

// Draws a flat hex grid on the hovered face using planet-relative axes.
fn draw_flat_hex_grid_on_face(
    face_vertices: [Vec3; 3],
    global_vertices: &[Vec3],
    camera: &Camera3D,
    color: Color,
    line_thickness: f32,
    hexes_across: i32,
) {
    if hexes_across < 2 || global_vertices.is_empty() {
        return;
    }
    // Reorder so AB is the shortest side; this becomes the density baseline.
    let [a, b, c] = {
        let [v0, v1, v2] = face_vertices;
        let l01 = (v1 - v0).length();
        let l12 = (v2 - v1).length();
        let l20 = (v0 - v2).length();
        if l01 <= l12 && l01 <= l20 {
            [v0, v1, v2]
        } else if l12 <= l01 && l12 <= l20 {
            [v1, v2, v0]
        } else {
            [v2, v0, v1]
        }
    };
    let mut normal = (b - a).cross(c - a);
    if normal.length() < 1e-5 {
        return;
    }
    normal = normal.normalize();
    let center = (a + b + c) / 3.0;
    if normal.dot(center) < 0.0 {
        normal = -normal;
    }
    // Planet-relative axes: u from shortest face edge, v from face normal.
    let mut u = b - a;
    if u.length() < 1e-5 {
        return;
    }
    u = u.normalize();
    let v = normal.cross(u).normalize();
    if v.length() < 1e-5 {
        return;
    }
    let radius = center.length();

    let a2 = vec2((a - center).dot(u), (a - center).dot(v));
    let b2 = vec2((b - center).dot(u), (b - center).dot(v));
    let c2 = vec2((c - center).dot(u), (c - center).dot(v));

    let shortest_side = (b2 - a2).length();
    if shortest_side <= 1e-5 {
        return;
    }
    let side = shortest_side;
    let ref_a = vec2(0.0, 0.0);
    let ref_b = vec2(side, 0.0);
    let ref_c = vec2(side * 0.5, side * 0.866_025_4);
    let size = side / (SQRT_3 * hexes_across as f32);
    if size <= 1e-5 {
        return;
    }

    let mut centers_ref: Vec<Vec2> = Vec::new();
    // Ensure a hex center at each triangle tip.
    centers_ref.push(ref_a);
    centers_ref.push(ref_b);
    centers_ref.push(ref_c);

    let ref_height = side * 0.866_025_4;
    let max_q = ((side / (SQRT_3 * size)).ceil() as i32) + 3;
    let max_r = ((ref_height / (1.5 * size)).ceil() as i32) + 3;
    for r in -max_r..=max_r {
        for q in -max_q..=max_q {
            let p2 = axial_to_plane(q, r, size);
            if !point_in_triangle_2d(p2, ref_a, ref_b, ref_c) {
                continue;
            }
            centers_ref.push(p2);
        }
    }

    let global_edges = icosahedron_edges();
    let pent_step = std::f32::consts::TAU / 5.0;
    let mut dedupe: HashSet<(i32, i32)> = HashSet::new();
    for center_ref in centers_ref {
        let key_scale = (size * 0.2).max(1e-5);
        let key = (
            (center_ref.x / key_scale).round() as i32,
            (center_ref.y / key_scale).round() as i32,
        );
        if !dedupe.insert(key) {
            continue;
        }

        let Some((wa, wb, wc)) = barycentric_coords_2d(center_ref, ref_a, ref_b, ref_c) else {
            continue;
        };
        let center_dst = a2 * wa + b2 * wb + c2 * wc;
        let center_surface =
            (center + u * center_dst.x + v * center_dst.y).normalize() * radius;
        let normal = center_surface.normalize();
        let mut tangent_x = u - normal * normal.dot(u);
        if tangent_x.length() < 1e-5 {
            tangent_x = vec3(0.0, 1.0, 0.0) - normal * normal.dot(vec3(0.0, 1.0, 0.0));
        }
        if tangent_x.length() < 1e-5 {
            tangent_x = vec3(1.0, 0.0, 0.0) - normal * normal.dot(vec3(1.0, 0.0, 0.0));
        }
        if tangent_x.length() < 1e-5 {
            continue;
        }
        tangent_x = tangent_x.normalize();
        let tangent_y = normal.cross(tangent_x).normalize();
        if tangent_y.length() < 1e-5 {
            continue;
        }

        let center_factor = (wa.min(wb).min(wc) / (1.0 / 3.0)).clamp(0.0, 1.0);
        let cell_lift =
            radius * (HOVER_GRID_CELL_BASE_LIFT + HOVER_GRID_CELL_MID_EXTRA_LIFT * center_factor);
        let center_world = center_surface + normal * cell_lift;

        let Some((u_wa, u_wb, u_wc)) =
            barycentric_coords_2d(center_ref + vec2(size, 0.0), ref_a, ref_b, ref_c)
        else {
            continue;
        };
        let sample_u_dst = a2 * u_wa + b2 * u_wb + c2 * u_wc;
        let sample_u_surface =
            (center + u * sample_u_dst.x + v * sample_u_dst.y).normalize() * radius;

        let Some((v_wa, v_wb, v_wc)) =
            barycentric_coords_2d(center_ref + vec2(0.0, size), ref_a, ref_b, ref_c)
        else {
            continue;
        };
        let sample_v_dst = a2 * v_wa + b2 * v_wb + c2 * v_wc;
        let sample_v_surface =
            (center + u * sample_v_dst.x + v * sample_v_dst.y).normalize() * radius;

        let cell_step_world = ((sample_u_surface - center_surface).length()
            + (sample_v_surface - center_surface).length())
            * 0.5;
        let vertex_snap_threshold = (cell_step_world * 0.35).max(1e-4);

        let mut matched_global_vertex: Option<usize> = None;
        let mut best_dist = f32::MAX;
        for (idx, vertex) in global_vertices.iter().enumerate() {
            let dist = (*vertex - center_surface).length();
            if dist < best_dist {
                best_dist = dist;
                matched_global_vertex = Some(idx);
            }
        }
        let use_pentagon = best_dist <= vertex_snap_threshold;
        let sides = if use_pentagon { 5usize } else { 6usize };
        let draw_radius = size * 0.95;

        let mut angle_offset = -std::f32::consts::PI / 6.0;
        if use_pentagon {
            angle_offset = -std::f32::consts::FRAC_PI_2;
            if let Some(vertex_index) = matched_global_vertex {
                let vertex_pos = global_vertices[vertex_index];
                let mut neighbor_angles: Vec<f32> = Vec::new();
                for (ea, eb) in global_edges.iter().copied() {
                    let neighbor_index = if ea == vertex_index {
                        Some(eb)
                    } else if eb == vertex_index {
                        Some(ea)
                    } else {
                        None
                    };
                    let Some(neighbor_index) = neighbor_index else {
                        continue;
                    };
                    let dir_world = (global_vertices[neighbor_index] - vertex_pos).normalize();
                    let tangent_dir = dir_world - normal * normal.dot(dir_world);
                    if tangent_dir.length() < 1e-5 {
                        continue;
                    }
                    let tangent_dir = tangent_dir.normalize();
                    let x = tangent_dir.dot(tangent_x);
                    let y = tangent_dir.dot(tangent_y);
                    neighbor_angles.push(y.atan2(x));
                }
                if !neighbor_angles.is_empty() {
                    neighbor_angles
                        .sort_by(|lhs, rhs| lhs.partial_cmp(rhs).unwrap_or(std::cmp::Ordering::Equal));
                    let mut best_error = f32::MAX;
                    let mut best_offset = angle_offset;
                    for start in 0..neighbor_angles.len() {
                        let candidate = neighbor_angles[start] - 0.5 * pent_step;
                        let mut error = 0.0;
                        for i in 0..neighbor_angles.len() {
                            let idx = (start + i) % neighbor_angles.len();
                            let expected = candidate + pent_step * (i as f32 + 0.5);
                            error += wrap_angle(neighbor_angles[idx] - expected).abs();
                        }
                        if error < best_error {
                            best_error = error;
                            best_offset = candidate;
                        }
                    }
                    angle_offset = best_offset;
                }
            }
        }

        let mut corners_screen: Vec<Vec2> = Vec::with_capacity(sides);
        let mut all_visible = true;
        for i in 0..sides {
            let angle = angle_offset + (i as f32 / sides as f32) * std::f32::consts::TAU;
            let corner_ref =
                center_ref + vec2(draw_radius * angle.cos(), draw_radius * angle.sin());
            let Some((cwa, cwb, cwc)) = barycentric_coords_2d(corner_ref, ref_a, ref_b, ref_c)
            else {
                all_visible = false;
                break;
            };
            let corner_dst = a2 * cwa + b2 * cwb + c2 * cwc;
            let corner_delta = corner_dst - center_dst;
            let world_corner = center_world + tangent_x * corner_delta.x + tangent_y * corner_delta.y;
            let Some(screen_corner) = project_to_screen(camera, world_corner) else {
                all_visible = false;
                break;
            };
            corners_screen.push(screen_corner);
        }
        if !all_visible {
            continue;
        }
        for i in 0..sides {
            let p0 = corners_screen[i];
            let p1 = corners_screen[(i + 1) % sides];
            draw_line(p0.x, p0.y, p1.x, p1.y, line_thickness, color);
        }
    }
}

// Draws a great-circle arc between two points on the sphere.
fn draw_arc_on_sphere(start: Vec3, end: Vec3, radius: f32, segments: usize, color: Color) {
    let start = start.normalize();
    let end = end.normalize();
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

fn greek_face_letter(face_index: usize) -> char {
    match face_index {
        0 => 'α',
        1 => 'β',
        2 => 'γ',
        3 => 'δ',
        4 => 'ε',
        5 => 'ζ',
        6 => 'η',
        7 => 'θ',
        8 => 'ι',
        9 => 'κ',
        10 => 'λ',
        11 => 'μ',
        12 => 'ν',
        13 => 'ξ',
        14 => 'ο',
        15 => 'π',
        16 => 'ς',
        17 => 'σ',
        18 => 'τ',
        19 => 'υ',
        20 => 'φ',
        21 => 'ψ',
        _ => 'Ω'
    }
}

fn face_group_info(face_index: usize, total_faces: usize) -> (usize, usize) {
    if total_faces == 0 {
        return (0, 0);
    }
    let base_face_count = ICOSAHEDRON_FACE_COUNT.min(total_faces);
    let subfaces_per_base = (total_faces / base_face_count).max(1);
    let base_index = (face_index / subfaces_per_base).min(base_face_count.saturating_sub(1)) + 1;
    let subface_number = face_index % subfaces_per_base;
    (base_index, subface_number)
}

// For base-face mode this returns only the Greek letter.
// For close mode this returns "Letter+subface".
fn face_name(face_index: usize, total_faces: usize, include_subface: bool) -> String {
    let (base_index, subface_number) = face_group_info(face_index, total_faces);
    let letter = greek_face_letter(base_index);
    if include_subface {
        format!("{}{:02x}", letter, subface_number)
    } else {
        letter.to_string()
    }
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

// Builds an RGBA heightmap texture using equirectangular sampling.
fn build_heightmap_pixels(
    config: &PlanetNoiseConfig,
    size: u16,
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
            // Match uv_from_normal(): u = 0.5 + atan2(z, x) / TAU
            let theta = (u - 0.5) * std::f32::consts::TAU;
            let normal = vec3(sin_phi * theta.cos(), cos_phi, sin_phi * theta.sin());
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

// Checks if texture-relevant noise parameters changed.
fn texture_params_match(a: &PlanetNoiseConfig, b: &PlanetNoiseConfig) -> bool {
    let eps = 1e-4;
    (a.noise_scale - b.noise_scale).abs() < eps
        && (a.height_amp - b.height_amp).abs() < eps
        && (a.height_bias - b.height_bias).abs() < eps
        && (a.lat_bias - b.lat_bias).abs() < eps
        && (a.sea_level - b.sea_level).abs() < eps
        && (a.ice_start - b.ice_start).abs() < eps
        && (a.ice_strength - b.ice_strength).abs() < eps
}

// Builds or loads a cached heightmap PNG for current noise parameters.
fn load_or_build_heightmap(config: &PlanetNoiseConfig, size: u16) -> Vec<u8> {
    if let Some(pixels) = load_heightmap_png(config, size) {
        return pixels;
    }
    let pixels = build_heightmap_pixels(config, size);
    save_heightmap_png(config, size, &pixels);
    pixels
}

fn heightmap_cache_key(config: &PlanetNoiseConfig, size: u16) -> u64 {
    // Texture generation does not depend on LOD subdivisions.
    let words = [
        config.noise_scale.to_bits() as u64,
        config.height_amp.to_bits() as u64,
        config.height_bias.to_bits() as u64,
        config.lat_bias.to_bits() as u64,
        config.sea_level.to_bits() as u64,
        config.ice_start.to_bits() as u64,
        config.ice_strength.to_bits() as u64,
        size as u64,
    ];
    let mut hash: u64 = 1469598103934665603;
    for word in words {
        hash ^= word;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

fn heightmap_cache_path(config: &PlanetNoiseConfig, size: u16) -> String {
    format!(
        "{}_{:016x}_{}x{}.png",
        HEIGHTMAP_IMAGE_PREFIX,
        heightmap_cache_key(config, size),
        size,
        size
    )
}

fn load_heightmap_png(config: &PlanetNoiseConfig, size: u16) -> Option<Vec<u8>> {
    let path = heightmap_cache_path(config, size);
    let file_bytes = fs::read(path).ok()?;
    let mut image = Image::from_file_with_format(&file_bytes, None).ok()?;
    if image.width != size || image.height != size {
        return None;
    }
    flip_rgba_vertical(&mut image.bytes, image.width as usize, image.height as usize);
    Some(image.bytes)
}

fn save_heightmap_png(config: &PlanetNoiseConfig, size: u16, pixels: &[u8]) {
    if pixels.len() != size as usize * size as usize * 4 {
        return;
    }
    let path = heightmap_cache_path(config, size);
    let image = Image {
        bytes: pixels.to_vec(),
        width: size,
        height: size,
    };
    image.export_png(&path);
}

fn flip_rgba_vertical(bytes: &mut [u8], width: usize, height: usize) {
    let stride = width * 4;
    for y in 0..(height / 2) {
        let top = y * stride;
        let bottom = (height - 1 - y) * stride;
        for x in 0..stride {
            bytes.swap(top + x, bottom + x);
        }
    }
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

fn zoom_to_subdivisions_hysteresis(distance: f32, current: u8) -> u8 {
    let h = SUBDIVISION_HYSTERESIS;
    match current {
        5 => {
            if distance > 4.0 + h { 4 } else { 5 }
        }
        4 => {
            if distance < 4.0 - h {
                5
            } else if distance > 5.0 + h {
                3
            } else {
                4
            }
        }
        3 => {
            if distance < 5.0 - h {
                4
            } else if distance > 7.0 + h {
                2
            } else {
                3
            }
        }
        2 => {
            if distance < 7.0 - h {
                3
            } else if distance > 9.0 + h {
                1
            } else {
                2
            }
        }
        1 => {
            if distance < 9.0 - h { 2 } else { 1 }
        }
        _ => zoom_to_subdivisions(distance),
    }
}

// Runs the planet scene frame update and rendering.
pub fn run(
    ctx: &FrameContext,
    state: &mut PlanetState,
    scene: &mut Scene,
    greek_font: Option<&Font>,
) {
    if is_key_pressed(KeyCode::Escape) {
        *scene = Scene::MainMenu;
        return;
    }
    if !state.relief_available && state.show_relief {
        state.show_relief = false;
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
    if wy.abs() > 0.001 {
        state.target_distance = (state.target_distance - wy * 0.001 * state.target_distance).clamp(1.8, 100.0);
    }
    state.distance += (state.target_distance - state.distance) * (0.01 * state.distance).clamp(0.05, 100.0);
    let desired_subdivisions =
        zoom_to_subdivisions_hysteresis(state.distance, state.config.subdivisions);
    if state.show_relief && desired_subdivisions != state.config.subdivisions {
        planet_perf_log(&format!(
            "lod switch distance={:.3} old={} new={}",
            state.distance, state.config.subdivisions, desired_subdivisions
        ));
        state.config.subdivisions = desired_subdivisions;
        state.regenerate(512);
    }
    state.poll_regen();
    let frame_time = get_frame_time();
    if frame_time > 0.04 {
        let now = get_time();
        if now - state.last_hitch_log_at > 0.2 {
            state.last_hitch_log_at = now;
            planet_perf_log(&format!(
                "hitch frame_ms={:.2} distance={:.3} subdiv={} regen_in_progress={} pending={} expected={} received={}",
                frame_time * 1000.0,
                state.distance,
                state.config.subdivisions,
                state.regen_in_progress,
                state.regen_pending,
                state.regen_expected_chunks,
                state.regen_received_chunks
            ));
        }
    }
    let yaw_delta = wrap_angle(state.target_yaw - state.yaw);
    state.yaw += yaw_delta * 0.12;
    state.pitch += (state.target_pitch - state.pitch) * 0.12;
    let camera_animating =
        state.dragging
            || wy.abs() > 0.01
            || (state.target_distance - state.distance).abs() > 0.02
            || yaw_delta.abs() > 0.01
            || (state.target_pitch - state.pitch).abs() > 0.01;

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
    let show_labels = false;
    let camera_dir = camera_pos.normalize();
    let use_fallback_relief = camera_animating || state.regen_in_progress;
    for mesh in state.base_texture_meshes.iter() {
        if mesh.vertices.is_empty() {
            continue;
        }
        draw_mesh(mesh);
    }
    if state.show_relief {
        let relief_to_draw = if use_fallback_relief {
            &state.fallback_relief_meshes
        } else {
            &state.relief_meshes
        };
        for (index, mesh) in relief_to_draw.iter().enumerate() {
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
    let base_faces = build_faces(base_vertices, &edges);
    let edge_color = Color::from_rgba(200, 220, 250, 255);
    let hover_color = Color::from_rgba(255, 200, 120, 255);
    let grid_color = Color::from_rgba(70, 78, 86, 255);

    let mut projected_base: Vec<Vec3> = vec![Vec3::ZERO; base_vertices.len()];
    let mut projected_sector: Vec<Vec3> = vec![Vec3::ZERO; vertices.len()];
    let mut screen_points: Vec<Option<Vec2>> = vec![None; base_vertices.len()];
    for (index, v) in base_vertices.iter().enumerate() {
        projected_base[index] = v.normalize() * line_radius;
        screen_points[index] = project_to_screen(&camera, projected_base[index]);
    }
    for (index, v) in vertices.iter().enumerate() {
        projected_sector[index] = v.normalize() * line_radius;
    }

    for (a, b) in edges.iter().copied() {
        draw_arc_on_sphere(projected_base[a], projected_base[b], line_radius, segments, edge_color);
    }

    let use_subface_hover = state.distance <= SUBFACE_HOVER_MAX_DISTANCE;
    let mut hovered_subface: Option<usize> = None;
    let mut hovered_base_face: Option<usize> = None;
    let mut hover_grid_face: Option<[Vec3; 3]> = None;
    if let Some((origin, dir)) = ray_from_mouse(&camera, mouse) {
        if let Some(hit) = ray_sphere_intersection(origin, dir, radius) {
            if use_subface_hover {
                hovered_subface = hovered_face(hit, &faces, vertices);
            } else {
                hovered_base_face = hovered_face(hit, &base_faces, base_vertices);
            }
        }
    }
    if let Some(face_index) = hovered_base_face {
        let face = base_faces[face_index];
        let a = projected_base[face[0]];
        let b = projected_base[face[1]];
        let c = projected_base[face[2]];
        draw_arc_on_sphere(a, b, line_radius, segments, hover_color);
        draw_arc_on_sphere(b, c, line_radius, segments, hover_color);
        draw_arc_on_sphere(c, a, line_radius, segments, hover_color);
    }
    if let Some(face_index) = hovered_subface {
        let face = faces[face_index];
        let a = projected_sector[face[0]];
        let b = projected_sector[face[1]];
        let c = projected_sector[face[2]];
        draw_arc_on_sphere(a, b, line_radius, segments, hover_color);
        draw_arc_on_sphere(b, c, line_radius, segments, hover_color);
        draw_arc_on_sphere(c, a, line_radius, segments, hover_color);
    }
    let show_hover_grid = state.distance <= NEAR_GRID_DISTANCE;
    if show_hover_grid {
        if let Some(face_index) = hovered_subface {
            let face = faces[face_index];
            hover_grid_face = Some([
                projected_sector[face[0]],
                projected_sector[face[1]],
                projected_sector[face[2]],
            ]);
        } else if let Some(face_index) = hovered_base_face {
            let face = base_faces[face_index];
            hover_grid_face = Some([
                projected_base[face[0]],
                projected_base[face[1]],
                projected_base[face[2]],
            ]);
        }
    }
    if is_mouse_button_pressed(MouseButton::Left) {
        if let Some(face_index) = hovered_subface {
            let face = faces[face_index];
            let normal = face_normal(vertices, face);
            state.target_yaw = normal.z.atan2(normal.x);
            state.target_pitch = normal.y.asin();
        } else if let Some(face_index) = hovered_base_face {
            let face = base_faces[face_index];
            let normal = face_normal(base_vertices, face);
            state.target_yaw = normal.z.atan2(normal.x);
            state.target_pitch = normal.y.asin();
        }
    }

    set_default_camera();
    if let Some(face_vertices) = hover_grid_face {
        draw_flat_hex_grid_on_face(
            face_vertices,
            &projected_base,
            &camera,
            grid_color,
            1.2,
            HOVER_GRID_HEXES_ACROSS,
        );
    }
    if state.debug_enabled {
        draw_planet_controls(ctx, state, PLANET_NOISE_PATH);
    }
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
    if let Some(face_index) = hovered_subface {
        let name = face_name(face_index, faces.len(), true);
        draw_text_ex(
            &format!("Sector {}", name),
            20.0,
            32.0,
            TextParams {
                font: greek_font,
                font_size: 30,
                color: Color::from_rgba(255, 235, 180, 255),
                ..Default::default()
            },
        );
    } else if let Some(face_index) = hovered_base_face {
        let name = face_name(face_index, base_faces.len(), false);
        draw_text_ex(
            &format!("Hovered Zone {}", name),
            20.0,
            32.0,
            TextParams {
                font: greek_font,
                font_size: 30,
                color: Color::from_rgba(255, 235, 180, 255),
                ..Default::default()
            },
        );
    }
}

