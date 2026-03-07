use macroquad::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::mpsc::{self, Receiver};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use crate::core::{FrameContext, Scene};
use crate::core::debug::{
    draw_planet_controls, format_log_timestamp, planet_perf_log, PLANET_DEBUG_UI_ENABLED_DEFAULT,
    SAVE_MESH_POINTS_RUNTIME,
};
use crate::core::planet_grid::{
    HexGrid, align_vertices_to_poles, build_faces, build_frequency_geodesic, build_geodesic_sphere,
    build_hex_grid_from_data, icosahedron_edges, icosahedron_vertices, load_hex_grid_cache,
    save_hex_grid_cache,
};
use crate::core::planet_resources::{
    PlanetResourceKind, PlanetSimWorld, PlanetSurfaceClass, build_or_load_sim_world,
};
use crate::core::planet_texture::{
    PlanetNoiseConfig, color_from_height, height_value, load_or_build_heightmap, texture_params_match,
};
use crate::core::ui::{draw_build_panel, draw_window, ui_button, WindowState, WindowStyle, WINDOW_TITLE_HEIGHT};
use crate::core::map_common::{build_panel_layout, handle_window_drag};

const PLANET_DATA_DIR: &str = "planet_data";
const MESH_POINTS_PATH: &str = "planet_data/planet_mesh_points.csv";
const PLANET_NOISE_PATH: &str = "planet_data/planet_noise.json";
const SIM_GRID_CACHE_PATH: &str = "planet_data/planet_sim_grid.bin";
const PLANET_RESOURCES_PATH: &str = "planet_data/planet_resources.bin";
const PLANET_BUILDINGS_PATH: &str = "planet_data/planet_buildings.json";
const HEIGHTMAP_SIZE: u16 = 2048;
const TEXTURE_BASE_SUBDIVISIONS: usize = 2;
const TEXTURE_LAYER_OFFSET: f32 = 0.004;
const SUBDIVISION_HYSTERESIS: f32 = 0.25;
const FALLBACK_RELIEF_SUBDIVISIONS: usize = 1;
const REGEN_FACE_GRAIN: usize = 12;
const ICOSAHEDRON_FACE_COUNT: usize = 20;
const SUBFACE_HOVER_MAX_DISTANCE: f32 = 5.6;
const SIM_GRID_HEXES_ACROSS: i32 = 160;
const PLANET_OVERLAY_OFFSET: f32 = 0.01;
const PLANET_SIM_SEED: u64 = 0xDA7A_51C4_1234_8B9E;
const MINE_EXTRACT_RATE_PER_SEC: f32 = 6.0;

// Wraps an angle to the [-PI, PI] range for smooth camera interpolation.
fn quat_from_forward_up(forward: Vec3, up: Vec3) -> Quat {
    let forward = forward.normalize();
    let mut up = up - forward * forward.dot(up);
    if up.length() < 1e-5 {
        up = vec3(1.0, 0.0, 0.0) - forward * forward.dot(vec3(1.0, 0.0, 0.0));
    }
    if up.length() < 1e-5 {
        up = vec3(0.0, 0.0, 1.0) - forward * forward.dot(vec3(0.0, 0.0, 1.0));
    }
    let up = up.normalize();
    let right = up.cross(forward).normalize();
    let up = forward.cross(right).normalize();
    let mat = Mat3::from_cols(right, up, forward);
    Quat::from_mat3(&mat)
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

fn start_hex_grid_build(
    cache_path: &'static str,
    freq: i32,
    base_vertices: &[Vec3],
    base_faces: &[[usize; 3]],
) -> Receiver<HexGrid> {
    let (tx, rx) = mpsc::channel();
    let base_vertices = base_vertices.to_vec();
    let base_faces = base_faces.to_vec();
    std::thread::spawn(move || {
        let t0 = Instant::now();
        let grid = if let Some((vertices, faces)) =
            load_hex_grid_cache(cache_path, freq)
        {
            planet_perf_log(&format!(
                "hex grid cache loaded path={} freq={} vertices={} faces={} took_ms={}",
                cache_path,
                freq,
                vertices.len(),
                faces.len(),
                t0.elapsed().as_millis()
            ));
            build_hex_grid_from_data(freq, vertices, faces, &base_vertices, &base_faces)
        } else {
            let (vertices, faces) = build_frequency_geodesic(freq, &base_vertices, &base_faces);
            ensure_planet_data_dir();
            save_hex_grid_cache(cache_path, freq, &vertices, &faces);
            planet_perf_log(&format!(
                "hex grid built path={} freq={} vertices={} faces={} took_ms={}",
                cache_path,
                freq,
                vertices.len(),
                faces.len(),
                t0.elapsed().as_millis()
            ));
            build_hex_grid_from_data(freq, vertices, faces, &base_vertices, &base_faces)
        };
        let _ = tx.send(grid);
    });
    rx
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum PlanetBuildingKind {
    Base,
    Builder,
    Housing,
    Factory,
    Mine,
    Warehouse,
    Logistics,
    Route,
}

impl PlanetBuildingKind {
    fn label(self) -> &'static str {
        match self {
            PlanetBuildingKind::Base => "Base",
            PlanetBuildingKind::Builder => "Builder",
            PlanetBuildingKind::Housing => "Housing",
            PlanetBuildingKind::Factory => "Factory",
            PlanetBuildingKind::Mine => "Mine",
            PlanetBuildingKind::Warehouse => "Warehouse",
            PlanetBuildingKind::Logistics => "Logistics",
            PlanetBuildingKind::Route => "Route",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlanetBuildingRecord {
    #[serde(default, alias = "visual_cell")]
    cell_index: u32,
    kind: PlanetBuildingKind,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
struct PlanetBuildingsSnapshot {
    #[serde(default)]
    buildings: Vec<PlanetBuildingRecord>,
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
    pub camera_rot: Quat,
    pub target_rot: Quat,
    pub distance: f32,
    pub target_distance: f32,
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
    sim_grid: Option<HexGrid>,
    sim_grid_rx: Option<Receiver<HexGrid>>,
    sim_world: Option<PlanetSimWorld>,
    selected_build_tool: Option<PlanetBuildingKind>,
    panel_collapsed: bool,
    buildings: HashMap<u32, PlanetBuildingKind>,
    buildings_dirty: bool,
    mine_progress: HashMap<u32, f32>,
    stockpiles: HashMap<PlanetResourceKind, u32>,
    min_cell_edge_dist_unit: f32,
    info_window: WindowState,
    info_target_cell: Option<u32>,
    confirm_window: WindowState,
    confirm_target_cell: Option<u32>,
    relief_cache: HashMap<ReliefCacheKey, ReliefCacheEntry>,
    pending_relief_key: Option<ReliefCacheKey>,
    regen_rx: Option<Receiver<RegenMessage>>,
    pub regen_in_progress: bool,
    pub regen_pending: bool,
    pub regen_version: u64,
    pub regen_expected_chunks: usize,
    pub regen_received_chunks: usize,
    last_hitch_log_at: f64,
    last_resource_save_at: f64,
    last_buildings_save_at: f64,
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
        let base_faces = build_faces(&base_vertices, &icosahedron_edges());
        let sim_grid_rx = Some(start_hex_grid_build(
            SIM_GRID_CACHE_PATH,
            SIM_GRID_HEXES_ACROSS,
            &base_vertices,
            &base_faces,
        ));
        let initial_rot = Quat::from_rotation_y(0.0) * Quat::from_rotation_x(0.3);
        let mut state = Self {
            camera_rot: initial_rot,
            target_rot: initial_rot,
            distance: 6.0,
            target_distance: 6.0,
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
            sim_grid: None,
            sim_grid_rx,
            sim_world: None,
            selected_build_tool: Some(PlanetBuildingKind::Mine),
            panel_collapsed: false,
            buildings: HashMap::new(),
            buildings_dirty: false,
            mine_progress: HashMap::new(),
            stockpiles: HashMap::new(),
            min_cell_edge_dist_unit: 0.010,
            info_window: WindowState {
                title: String::new(),
                rect: Rect::new(40.0, 120.0, 260.0, 170.0),
                open: false,
                target: None,
                dragging: false,
                drag_offset: Vec2::ZERO,
                show_units: false,
            },
            info_target_cell: None,
            confirm_window: WindowState {
                title: "Confirm".to_string(),
                rect: Rect::new(40.0, 120.0, 250.0, 120.0),
                open: false,
                target: None,
                dragging: false,
                drag_offset: Vec2::ZERO,
                show_units: false,
            },
            confirm_target_cell: None,
            relief_cache: HashMap::new(),
            pending_relief_key: None,
            regen_rx: None,
            regen_in_progress: false,
            regen_pending: false,
            regen_version: 0,
            regen_expected_chunks: 0,
            regen_received_chunks: 0,
            last_hitch_log_at: 0.0,
            last_resource_save_at: 0.0,
            last_buildings_save_at: 0.0,
            regen_generation: Arc::new(AtomicU64::new(0)),
        };
        state.load_buildings_snapshot(PLANET_BUILDINGS_PATH);
        state
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
        if let Some(sim_world) = self.sim_world.as_mut() {
            sim_world.save_if_dirty(PLANET_RESOURCES_PATH);
        }
        self.sim_world = None;
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

    pub fn can_build_on_sim_cell(&self, cell_index: u32) -> bool {
        self.sim_world
            .as_ref()
            .map(|world| world.can_build(cell_index))
            .unwrap_or(false)
    }

    fn load_buildings_snapshot(&mut self, path: &str) {
        let Ok(contents) = fs::read_to_string(path) else {
            return;
        };
        let Ok(snapshot) = serde_json::from_str::<PlanetBuildingsSnapshot>(&contents) else {
            return;
        };
        self.buildings.clear();
        for record in snapshot.buildings.into_iter() {
            self.buildings.insert(record.cell_index, record.kind);
        }
        self.buildings_dirty = false;
    }

    fn save_buildings_if_dirty(&mut self, path: &str) {
        if !self.buildings_dirty {
            return;
        }
        ensure_planet_data_dir();
        let snapshot = PlanetBuildingsSnapshot {
            buildings: self
                .buildings
                .iter()
                .map(|(cell_index, kind)| PlanetBuildingRecord {
                    cell_index: *cell_index,
                    kind: *kind,
                })
                .collect(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
            let _ = fs::write(path, json);
            self.buildings_dirty = false;
        }
    }

    fn can_place_building(&self, cell_index: u32, kind: PlanetBuildingKind) -> bool {
        if self.buildings.contains_key(&cell_index) {
            return false;
        }
        if !self.can_build_on_sim_cell(cell_index) {
            return false;
        }
        match (kind, self.sim_world.as_ref()) {
            (PlanetBuildingKind::Mine, Some(world)) => world
                .cells
                .get(cell_index as usize)
                .and_then(|cell| cell.deposit)
                .map(|deposit| deposit.remaining_amount > 0)
                .unwrap_or(false),
            _ => true,
        }
    }

    fn place_building(&mut self, cell_index: u32, kind: PlanetBuildingKind) -> bool {
        if !self.can_place_building(cell_index, kind) {
            return false;
        }
        self.buildings.insert(cell_index, kind);
        self.buildings_dirty = true;
        true
    }

    fn remove_building(&mut self, cell_index: u32) -> bool {
        let removed = self.buildings.remove(&cell_index).is_some();
        if removed {
            self.mine_progress.remove(&cell_index);
            self.buildings_dirty = true;
        }
        removed
    }

    fn tick_mining(&mut self, dt: f32) {
        let Some(sim_world) = self.sim_world.as_mut() else {
            return;
        };
        let mine_cells: Vec<u32> = self
            .buildings
            .iter()
            .filter_map(|(cell_index, kind)| {
                if *kind == PlanetBuildingKind::Mine {
                    Some(*cell_index)
                } else {
                    None
                }
            })
            .collect();
        for cell_index in mine_cells.into_iter() {
            let progress = self.mine_progress.entry(cell_index).or_insert(0.0);
            *progress += dt * MINE_EXTRACT_RATE_PER_SEC;
            let units = progress.floor() as u32;
            if units == 0 {
                continue;
            }
            *progress -= units as f32;
            let resource_kind = sim_world
                .cells
                .get(cell_index as usize)
                .and_then(|cell| cell.deposit)
                .map(|deposit| deposit.kind);
            let Some(resource_kind) = resource_kind else {
                continue;
            };
            let mined = sim_world.extract_from_cell(cell_index, units);
            if mined > 0 {
                *self.stockpiles.entry(resource_kind).or_insert(0) += mined;
            }
        }
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

fn draw_global_hex_cell(
    hex_grid: &HexGrid,
    camera: &Camera3D,
    color: Color,
    line_thickness: f32,
    overlay_radius: f32,
    surface_radius: f32,
    selected_cell: Option<u32>,
    hover_hit: Option<Vec3>,
    hovered_base_face: Option<usize>,
) -> Option<u32> {
    let best_index = if let Some(selected_cell) = selected_cell {
        selected_cell
    } else {
        let Some(hover_hit) = hover_hit else {
            return None;
        };
        let Some(face_index) = hovered_base_face else {
            return None;
        };
        let hover_dir = hover_hit.normalize();
        let Some(best_index) = hovered_hex_vertex(hex_grid, hover_dir, face_index) else {
            return None;
        };
        best_index
    };

    let vertex_dir = hex_grid.vertices[best_index as usize];
    let face_list = &hex_grid.vertex_faces[best_index as usize];
    if face_list.len() < 3 {
        return None;
    }

    let mut centers: Vec<Vec3> = Vec::with_capacity(face_list.len());
    for &face_index in face_list {
        centers.push(hex_grid.face_centers[face_index as usize]);
    }

    let mut tangent_x = Vec3::ZERO;
    for center in centers.iter() {
        let candidate = *center - vertex_dir * vertex_dir.dot(*center);
        if candidate.length() > 1e-5 {
            tangent_x = candidate.normalize();
            break;
        }
    }
    if tangent_x.length() < 1e-5 {
        let fallback = vec3(0.0, 1.0, 0.0) - vertex_dir * vertex_dir.dot(vec3(0.0, 1.0, 0.0));
        if fallback.length() < 1e-5 {
            return None;
        }
        tangent_x = fallback.normalize();
    }
    let tangent_y = vertex_dir.cross(tangent_x).normalize();
    if tangent_y.length() < 1e-5 {
        return None;
    }

    let mut ordered: Vec<(f32, Vec3)> = centers
        .into_iter()
        .map(|center| {
            let angle = center.dot(tangent_y).atan2(center.dot(tangent_x));
            (angle, center)
        })
        .collect();
    ordered.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut outer_screen: Vec<Vec2> = Vec::with_capacity(ordered.len());
    let mut inner_screen: Vec<Vec2> = Vec::with_capacity(ordered.len());
    for (_, dir) in ordered.iter() {
        let world_outer = *dir * overlay_radius;
        let world_inner = *dir * surface_radius;
        let Some(screen_outer) = project_to_screen(camera, world_outer) else {
            return None;
        };
        let Some(screen_inner) = project_to_screen(camera, world_inner) else {
            return None;
        };
        outer_screen.push(screen_outer);
        inner_screen.push(screen_inner);
    }

    let sides = outer_screen.len();
    for i in 0..sides {
        let p0 = outer_screen[i];
        let p1 = outer_screen[(i + 1) % sides];
        draw_line(p0.x, p0.y, p1.x, p1.y, line_thickness, color);
    }

    let inner_color = Color::new(color.r, color.g, color.b, (color.a * 0.9).clamp(0.0, 1.0));
    for i in 0..sides {
        let p0 = inner_screen[i];
        let p1 = inner_screen[(i + 1) % sides];
        draw_line(
            p0.x,
            p0.y,
            p1.x,
            p1.y,
            (line_thickness * 0.95).max(0.6),
            inner_color,
        );
    }

    let radial_color = Color::new(color.r, color.g, color.b, (color.a * 0.7).clamp(0.0, 1.0));
    for i in 0..sides {
        let p_outer = outer_screen[i];
        let p_inner = inner_screen[i];
        draw_line(
            p_outer.x,
            p_outer.y,
            p_inner.x,
            p_inner.y,
            (line_thickness * 0.9).max(0.6),
            radial_color,
        );
    }
    Some(best_index)
}

fn draw_sim_buildings(
    visual_grid: &HexGrid,
    _sim_world: Option<&PlanetSimWorld>,
    _config: &PlanetNoiseConfig,
    buildings: &HashMap<u32, PlanetBuildingKind>,
    radius: f32,
    min_cell_edge_dist_unit: f32,
) {
    let target_apothem = (min_cell_edge_dist_unit * 0.9 * 0.62).max(0.0005);
    // Keep buildings above surface but below overlay lines (PLANET_OVERLAY_OFFSET=0.01).
    let surface_lift = 0.0035;
    for (cell_index, kind) in buildings.iter() {
        let Some(vertex_dir) = visual_grid.vertices.get(*cell_index as usize) else {
            continue;
        };
        let vertex_dir = vertex_dir.normalize();
        let face_list = &visual_grid.vertex_faces[*cell_index as usize];
        let sides = face_list.len();
        if sides < 3 {
            continue;
        }
        let mut centers: Vec<Vec3> = Vec::with_capacity(face_list.len());
        for &face_index in face_list.iter() {
            centers.push(visual_grid.face_centers[face_index as usize]);
        }
        let mut tangent_x = Vec3::ZERO;
        for center in centers.iter() {
            let candidate = *center - vertex_dir * vertex_dir.dot(*center);
            if candidate.length() > 1e-6 {
                tangent_x = candidate.normalize();
                break;
            }
        }
        if tangent_x.length() < 1e-6 {
            continue;
        }
        let tangent_y = vertex_dir.cross(tangent_x).normalize();
        if tangent_y.length() < 1e-6 {
            continue;
        }
        let mut ordered: Vec<f32> = centers
            .iter()
            .map(|center| {
                let p = vec2(center.dot(tangent_x), center.dot(tangent_y));
                p.y.atan2(p.x)
            })
            .collect();
        ordered.sort_by(|a, b| a.total_cmp(b));
        let start_angle = ordered[0];
        let angle_step = std::f32::consts::TAU / sides as f32;
        let cos_half = (std::f32::consts::PI / sides as f32).cos().max(1e-4);
        let outer_radius = target_apothem / cos_half;
        let mut outer_dirs: Vec<Vec3> = Vec::with_capacity(sides);
        for i in 0..sides {
            let a = start_angle + i as f32 * angle_step;
            let ring = tangent_x * a.cos() + tangent_y * a.sin();
            outer_dirs.push((vertex_dir + ring * outer_radius).normalize());
        }

        let color = match kind {
            PlanetBuildingKind::Base => Color::from_rgba(80, 220, 220, 255),
            PlanetBuildingKind::Builder => Color::from_rgba(255, 214, 102, 255),
            PlanetBuildingKind::Housing => Color::from_rgba(118, 203, 152, 255),
            PlanetBuildingKind::Factory => Color::from_rgba(170, 170, 196, 255),
            PlanetBuildingKind::Mine => Color::from_rgba(240, 170, 90, 255),
            PlanetBuildingKind::Warehouse => Color::from_rgba(116, 176, 224, 255),
            PlanetBuildingKind::Logistics => Color::from_rgba(255, 128, 128, 255),
            PlanetBuildingKind::Route => Color::from_rgba(220, 220, 120, 255),
        };
        let base_radius = radius + surface_lift;
        let height_scale = match kind {
            PlanetBuildingKind::Route => 0.42,
            PlanetBuildingKind::Logistics => 0.74,
            PlanetBuildingKind::Warehouse => 0.70,
            PlanetBuildingKind::Housing => 0.82,
            PlanetBuildingKind::Builder => 0.90,
            PlanetBuildingKind::Mine => 1.00,
            PlanetBuildingKind::Factory => 1.08,
            PlanetBuildingKind::Base => 1.15,
        };
        let apex_height = ((target_apothem * radius * 1.9) * height_scale).max(0.008);
        let apex = vertex_dir * (base_radius + apex_height);
        let base_center = vertex_dir * base_radius;
        let base_color = Color::new(color.r * 0.75, color.g * 0.75, color.b * 0.75, 1.0);
        let mut vertices: Vec<Vertex> = Vec::with_capacity(sides * 6);
        let mut indices: Vec<u16> = Vec::with_capacity(sides * 6);
        for i in 0..outer_dirs.len() {
            let j = (i + 1) % outer_dirs.len();
            let p0 = outer_dirs[i] * base_radius;
            let p1 = outer_dirs[j] * base_radius;

            let tri0 = vertices.len() as u16;
            vertices.push(Vertex::new2(apex, vec2(0.0, 0.0), color));
            vertices.push(Vertex::new2(p0, vec2(0.0, 0.0), color));
            vertices.push(Vertex::new2(p1, vec2(0.0, 0.0), color));
            indices.extend_from_slice(&[tri0, tri0 + 1, tri0 + 2]);

            let tri1 = vertices.len() as u16;
            vertices.push(Vertex::new2(base_center, vec2(0.0, 0.0), base_color));
            vertices.push(Vertex::new2(p1, vec2(0.0, 0.0), base_color));
            vertices.push(Vertex::new2(p0, vec2(0.0, 0.0), base_color));
            indices.extend_from_slice(&[tri1, tri1 + 1, tri1 + 2]);
        }
        let mesh = Mesh {
            vertices,
            indices,
            texture: None,
        };
        draw_mesh(&mesh);
    }
}

fn hovered_hex_vertex(hex_grid: &HexGrid, hover_dir: Vec3, base_face_index: usize) -> Option<u32> {
    if base_face_index >= hex_grid.base_face_buckets.len() {
        return None;
    }
    let mut candidates: Vec<u32> = Vec::new();
    candidates.extend(hex_grid.base_face_buckets[base_face_index].iter().copied());
    for &neighbor in hex_grid.base_face_neighbors[base_face_index].iter() {
        candidates.extend(hex_grid.base_face_buckets[neighbor].iter().copied());
    }
    candidates.extend(0..12u32);
    if candidates.is_empty() {
        return None;
    }
    let mut best_index = candidates[0];
    let mut best_dot = -1.0_f32;
    for index in candidates.into_iter() {
        let dir = hex_grid.vertices[index as usize];
        let dot = dir.dot(hover_dir);
        if dot > best_dot {
            best_dot = dot;
            best_index = index;
        }
    }
    Some(best_index)
}

// Computes the minimum center-to-edge distance (apothem) across all grid cells on the unit sphere.
fn min_cell_edge_distance_unit(hex_grid: &HexGrid) -> f32 {
    let mut min_dist = f32::MAX;
    for (vertex_index, vertex_dir) in hex_grid.vertices.iter().enumerate() {
        let face_list = &hex_grid.vertex_faces[vertex_index];
        if face_list.len() < 3 {
            continue;
        }
        let vertex_dir = vertex_dir.normalize();
        let mut centers: Vec<Vec3> = Vec::with_capacity(face_list.len());
        for &face_index in face_list.iter() {
            centers.push(hex_grid.face_centers[face_index as usize]);
        }

        let mut tangent_x = Vec3::ZERO;
        for center in centers.iter() {
            let candidate = *center - vertex_dir * vertex_dir.dot(*center);
            if candidate.length() > 1e-6 {
                tangent_x = candidate.normalize();
                break;
            }
        }
        if tangent_x.length() < 1e-6 {
            continue;
        }
        let tangent_y = vertex_dir.cross(tangent_x).normalize();
        if tangent_y.length() < 1e-6 {
            continue;
        }

        let mut ordered: Vec<(f32, Vec2)> = centers
            .iter()
            .map(|center| {
                let p = vec2(center.dot(tangent_x), center.dot(tangent_y));
                (p.y.atan2(p.x), p)
            })
            .collect();
        ordered.sort_by(|a, b| a.0.total_cmp(&b.0));

        for i in 0..ordered.len() {
            let a = ordered[i].1;
            let b = ordered[(i + 1) % ordered.len()].1;
            let edge = b - a;
            let denom = edge.length_squared();
            if denom <= 1e-12 {
                continue;
            }
            let t = (-a).dot(edge) / denom;
            let t_clamped = t.clamp(0.0, 1.0);
            let closest = a + edge * t_clamped;
            min_dist = min_dist.min(closest.length());
        }
    }
    if min_dist.is_finite() && min_dist > 0.0 {
        min_dist
    } else {
        0.010
    }
}

// Draws a filled triangle into an RGBA8 image buffer.
fn draw_triangle_rgba(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    a: Vec2,
    b: Vec2,
    c: Vec2,
    color: (u8, u8, u8, u8),
) {
    let min_x = a.x.min(b.x.min(c.x)).floor().max(0.0) as i32;
    let max_x = a.x.max(b.x.max(c.x)).ceil().min((width - 1) as f32) as i32;
    let min_y = a.y.min(b.y.min(c.y)).floor().max(0.0) as i32;
    let max_y = a.y.max(b.y.max(c.y)).ceil().min((height - 1) as f32) as i32;
    let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if area.abs() < 1e-6 {
        return;
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = vec2(x as f32 + 0.5, y as f32 + 0.5);
            let w0 = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            let w1 = (c.x - b.x) * (p.y - b.y) - (c.y - b.y) * (p.x - b.x);
            let w2 = (a.x - c.x) * (p.y - c.y) - (a.y - c.y) * (p.x - c.x);
            if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0) {
                let idx = ((y as usize) * width + x as usize) * 4;
                pixels[idx] = color.0;
                pixels[idx + 1] = color.1;
                pixels[idx + 2] = color.2;
                pixels[idx + 3] = color.3;
            }
        }
    }
}

// Draws a filled convex polygon (fan triangulation) into an RGBA8 image buffer.
fn draw_polygon_filled_rgba(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    poly: &[Vec2],
    color: (u8, u8, u8, u8),
) {
    if poly.len() < 3 {
        return;
    }
    for i in 1..(poly.len() - 1) {
        draw_triangle_rgba(pixels, width, height, poly[0], poly[i], poly[i + 1], color);
    }
}

fn regular_polygon(center: Vec2, radius: f32, sides: usize) -> Vec<Vec2> {
    let mut poly = Vec::with_capacity(sides);
    for i in 0..sides {
        let a = (std::f32::consts::TAU * i as f32 / sides as f32) - std::f32::consts::FRAC_PI_2;
        poly.push(center + vec2(a.cos(), a.sin()) * radius);
    }
    poly
}

// Saves a flat equilateral-triangle PNG filled with a regular hex grid (cells_per_side per edge).
fn save_test_triangle_hex_grid_png(path: &str, cells_per_side: i32, size: u16) {
    let side_len = cells_per_side.max(1);
    let n = side_len - 1;
    let pad = 24.0;
    let w = size as usize;
    let h = size as usize;
    let mut pixels = vec![0u8; w * h * 4];
    for i in 0..(w * h) {
        let idx = i * 4;
        pixels[idx] = 15;
        pixels[idx + 1] = 19;
        pixels[idx + 2] = 25;
        pixels[idx + 3] = 255;
    }

    let sqrt3 = 3.0_f32.sqrt();
    let mut centers: Vec<Vec2> = Vec::new();
    let mut min = vec2(f32::MAX, f32::MAX);
    let mut max = vec2(-f32::MAX, -f32::MAX);
    for r in 0..side_len {
        for q in 0..(side_len - r) {
            let x = sqrt3 * (q as f32 + 0.5 * r as f32);
            let y = 1.5 * r as f32;
            let p = vec2(x, y);
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
            centers.push(p);
        }
    }
    let span = (max - min).max(vec2(1e-6, 1e-6));
    let sx = (size as f32 - pad * 2.0) / span.x;
    let sy = (size as f32 - pad * 2.0) / span.y;
    let scale = sx.min(sy);
    let hex_radius = scale * 0.9;
    let mut px_centers: Vec<(i32, i32, Vec2)> = Vec::with_capacity(centers.len());
    for r in 0..side_len {
        for q in 0..(side_len - r) {
            let p = vec2(sqrt3 * (q as f32 + 0.5 * r as f32), 1.5 * r as f32);
            let x = pad + (p.x - min.x) * scale;
            let y = pad + (p.y - min.y) * scale;
            px_centers.push((q, r, vec2(x, y)));
        }
    }
    for (q, r, c) in px_centers.iter() {
        let is_corner = (*q == 0 && *r == 0) || (*q == n && *r == 0) || (*q == 0 && *r == n);
        let sides = if is_corner { 5 } else { 6 };
        let poly = regular_polygon(*c, hex_radius, sides);
        draw_polygon_filled_rgba(&mut pixels, w, h, &poly, (80, 130, 95, 255));
    }
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let image = Image {
        bytes: pixels,
        width: size,
        height: size,
    };
    image.export_png(path);
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

fn surface_label(surface: PlanetSurfaceClass) -> &'static str {
    match surface {
        PlanetSurfaceClass::Land => "Land",
        PlanetSurfaceClass::Coast => "Coast",
        PlanetSurfaceClass::Water => "Water",
    }
}

fn resource_label(kind: PlanetResourceKind) -> &'static str {
    match kind {
        PlanetResourceKind::Iron => "Iron",
        PlanetResourceKind::Copper => "Copper",
        PlanetResourceKind::Gold => "Gold",
        PlanetResourceKind::Lead => "Lead",
        PlanetResourceKind::Zinc => "Zinc",
        PlanetResourceKind::Lithium => "Lithium",
        PlanetResourceKind::Phosphate => "Phosphate",
        PlanetResourceKind::Stone => "Stone",
        PlanetResourceKind::FreshWater => "FreshWater",
        PlanetResourceKind::Salt => "Salt",
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
        if let Some(sim_world) = state.sim_world.as_mut() {
            sim_world.save_if_dirty(PLANET_RESOURCES_PATH);
        }
        state.save_buildings_if_dirty(PLANET_BUILDINGS_PATH);
        *scene = Scene::MainMenu;
        return;
    }
    if !state.relief_available && state.show_relief {
        state.show_relief = false;
    }
    if state.sim_grid.is_none() {
        if let Some(rx) = state.sim_grid_rx.as_ref() {
            if let Ok(grid) = rx.try_recv() {
                let t0 = Instant::now();
                let min_edge = min_cell_edge_distance_unit(&grid);
                state.sim_grid = Some(grid);
                state.sim_grid_rx = None;
                state.min_cell_edge_dist_unit = min_edge;
                planet_perf_log(&format!(
                    "sim grid metrics min_cell_edge_dist_unit={:.7} took_ms={}",
                    min_edge,
                    t0.elapsed().as_millis()
                ));
            }
        }
    }
    if state.sim_world.is_none() {
        if let Some(sim_grid) = state.sim_grid.as_ref() {
            let t0 = Instant::now();
            state.sim_world = Some(build_or_load_sim_world(
                PLANET_RESOURCES_PATH,
                sim_grid,
                &state.config,
                PLANET_SIM_SEED,
            ));
            planet_perf_log(&format!(
                "sim world ready freq={} cells={} took_ms={}",
                sim_grid.freq,
                sim_grid.vertices.len(),
                t0.elapsed().as_millis()
            ));
        }
    }
    let now = get_time();
    if now - state.last_resource_save_at > 5.0 {
        if let Some(sim_world) = state.sim_world.as_mut() {
            sim_world.save_if_dirty(PLANET_RESOURCES_PATH);
        }
        state.last_resource_save_at = now;
    }
    if now - state.last_buildings_save_at > 5.0 {
        state.save_buildings_if_dirty(PLANET_BUILDINGS_PATH);
        state.last_buildings_save_at = now;
    }

    let mouse = vec2(mouse_position().0, mouse_position().1);
    if is_mouse_button_down(MouseButton::Middle) {
        if !state.dragging {
            state.dragging = true;
            state.last_mouse = mouse;
        } else {
            let delta = mouse - state.last_mouse;
            let up_dir = state.target_rot * vec3(0.0, 1.0, 0.0);
            let right_dir = state.target_rot * vec3(1.0, 0.0, 0.0);
            let yaw_rot = Quat::from_axis_angle(up_dir, -delta.x * 0.01);
            let pitch_rot = Quat::from_axis_angle(right_dir, -delta.y * 0.01);
            state.target_rot = (yaw_rot * pitch_rot) * state.target_rot;
            state.last_mouse = mouse;
        }
    } else {
        state.dragging = false;
    }

    let frame_time = get_frame_time();
    if is_key_pressed(KeyCode::Key1) {
        state.selected_build_tool = None;
    }
    if is_key_pressed(KeyCode::Key2) {
        state.selected_build_tool = Some(PlanetBuildingKind::Base);
    }
    if is_key_pressed(KeyCode::Key3) {
        state.selected_build_tool = Some(PlanetBuildingKind::Mine);
    }
    if is_key_pressed(KeyCode::Key4) {
        state.selected_build_tool = Some(PlanetBuildingKind::Builder);
    }
    if is_key_pressed(KeyCode::Key5) {
        state.selected_build_tool = Some(PlanetBuildingKind::Housing);
    }
    if is_key_pressed(KeyCode::Key6) {
        state.selected_build_tool = Some(PlanetBuildingKind::Factory);
    }
    if is_key_pressed(KeyCode::Key7) {
        state.selected_build_tool = Some(PlanetBuildingKind::Warehouse);
    }
    if is_key_pressed(KeyCode::Key8) {
        state.selected_build_tool = Some(PlanetBuildingKind::Logistics);
    }
    if is_key_pressed(KeyCode::Key9) {
        state.selected_build_tool = Some(PlanetBuildingKind::Route);
    }
    state.tick_mining(frame_time);

    let mut yaw_input = 0.0;
    let mut pitch_input = 0.0;
    if is_key_down(KeyCode::Left) {
        yaw_input -= 1.0;
    }
    if is_key_down(KeyCode::Right) {
        yaw_input += 1.0;
    }
    if is_key_down(KeyCode::Up) {
        pitch_input += 1.0;
    }
    if is_key_down(KeyCode::Down) {
        pitch_input -= 1.0;
    }
    if yaw_input != 0.0 || pitch_input != 0.0 {
        let speed = 1.6;
        let up_dir = state.target_rot * vec3(0.0, 1.0, 0.0);
        let right_dir = state.target_rot * vec3(1.0, 0.0, 0.0);
        let yaw_rot = Quat::from_axis_angle(up_dir, yaw_input * speed * frame_time);
        let pitch_rot = Quat::from_axis_angle(right_dir, pitch_input * speed * frame_time);
        state.target_rot = (yaw_rot * pitch_rot) * state.target_rot;
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
    let rot_delta = 1.0 - state.camera_rot.dot(state.target_rot).abs();
    state.camera_rot = state.camera_rot.slerp(state.target_rot, 0.12);
    let camera_animating =
        state.dragging
        || wy.abs() > 0.01
        || (state.target_distance - state.distance).abs() > 0.02
        || rot_delta > 0.001;

    let camera_pos = (state.camera_rot * vec3(0.0, 0.0, 1.0)) * state.distance;
    let camera = Camera3D {
        position: camera_pos,
        target: vec3(0.0, 0.0, 0.0),
        up: state.camera_rot * vec3(0.0, 1.0, 0.0),
        fovy: 45.0,
        ..Default::default()
    };
    set_camera(&camera);

    let radius = 1.6;
    let line_radius = radius + PLANET_OVERLAY_OFFSET;
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
    
    let vertices = state.sector_vertices.clone();
    let base_vertices = state.base_vertices.clone();
    let edges = icosahedron_edges();
    let faces = state.sector_faces.clone();
    let base_faces = build_faces(&base_vertices, &edges);
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
    let mut hover_hit: Option<Vec3> = None;
    let mut hovered_cell: Option<u32> = None;
    if let Some((origin, dir)) = ray_from_mouse(&camera, mouse) {
        if let Some(hit) = ray_sphere_intersection(origin, dir, radius) {
            hover_hit = Some(hit);
            hovered_base_face = hovered_face(hit, &base_faces, &base_vertices);
            if use_subface_hover {
                hovered_subface = hovered_face(hit, &faces, &vertices);
            }
            if let (Some(sim_grid), Some(base_face)) = (state.sim_grid.as_ref(), hovered_base_face) {
                hovered_cell = hovered_hex_vertex(sim_grid, hit.normalize(), base_face);
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
    let panel_buttons: [(Option<PlanetBuildingKind>, &'static str); 9] = [
        (None, "Demolish"),
        (Some(PlanetBuildingKind::Base), "Base"),
        (Some(PlanetBuildingKind::Builder), "Builder"),
        (Some(PlanetBuildingKind::Housing), "Housing"),
        (Some(PlanetBuildingKind::Factory), "Factory"),
        (Some(PlanetBuildingKind::Mine), "Mine"),
        (Some(PlanetBuildingKind::Warehouse), "Warehouse"),
        (Some(PlanetBuildingKind::Logistics), "Logistics"),
        (Some(PlanetBuildingKind::Route), "Route"),
    ];
    let panel_layout = build_panel_layout(panel_buttons.len(), state.panel_collapsed);
    let ui_capturing = panel_layout.rect.contains(mouse)
        || (state.info_window.open && state.info_window.rect.contains(mouse))
        || (state.confirm_window.open && state.confirm_window.rect.contains(mouse));
    let show_hover_grid = hovered_cell.is_some();
    if is_mouse_button_pressed(MouseButton::Left) && !ui_capturing {
        if let Some(cell_index) = hovered_cell {
            if let Some(kind) = state.buildings.get(&cell_index) {
                state.info_window.title = kind.label().to_string();
                state.info_window.rect = Rect::new(mouse.x + 12.0, mouse.y + 12.0, 280.0, 190.0);
                state.info_window.open = true;
                state.info_target_cell = Some(cell_index);
            } else if let Some(face_index) = hovered_subface {
                let face = faces[face_index];
                let normal = face_normal(&vertices, face);
                state.target_rot = quat_from_forward_up(normal, vec3(0.0, 1.0, 0.0));
            } else if let Some(face_index) = hovered_base_face {
                let face = base_faces[face_index];
                let normal = face_normal(&base_vertices, face);
                state.target_rot = quat_from_forward_up(normal, vec3(0.0, 1.0, 0.0));
            }
        } else if let Some(face_index) = hovered_subface {
            let face = faces[face_index];
            let normal = face_normal(&vertices, face);
            state.target_rot = quat_from_forward_up(normal, vec3(0.0, 1.0, 0.0));
        } else if let Some(face_index) = hovered_base_face {
            let face = base_faces[face_index];
            let normal = face_normal(&base_vertices, face);
            state.target_rot = quat_from_forward_up(normal, vec3(0.0, 1.0, 0.0));
        }
    }
    if let Some(sim_grid) = state.sim_grid.as_ref() {
        draw_sim_buildings(
            sim_grid,
            state.sim_world.as_ref(),
            &state.config,
            &state.buildings,
            radius,
            state.min_cell_edge_dist_unit,
        );
    }
    if is_mouse_button_pressed(MouseButton::Right) && !ui_capturing {
        if let Some(cell_index) = hovered_cell {
            if let Some(kind) = state.selected_build_tool {
                let _ = state.place_building(cell_index, kind);
            } else if state.buildings.contains_key(&cell_index) {
                state.confirm_window.open = true;
                state.confirm_window.title = "Confirm".to_string();
                state.confirm_window.rect = Rect::new(mouse.x + 12.0, mouse.y + 12.0, 250.0, 120.0);
                state.confirm_target_cell = Some(cell_index);
            }
        }
    }
    if is_key_pressed(KeyCode::Delete) {
        if let Some(cell_index) = hovered_cell {
            let _ = state.remove_building(cell_index);
        }
    }
    if is_key_pressed(KeyCode::P) {
        save_test_triangle_hex_grid_png(
            "planet_data/zone_maps/test_triangle_hex_160.png",
            160,
            2048,
        );
        planet_perf_log("saved test triangle hex png 160");
    }
    set_default_camera();
    if show_hover_grid {
        if let Some(sim_grid) = state.sim_grid.as_ref() {
            let _ = draw_global_hex_cell(
                sim_grid,
                &camera,
                grid_color,
                1.2,
                line_radius,
                radius,
                hovered_cell,
                hover_hit,
                hovered_base_face,
            );
        }
    }
    let panel_result = draw_build_panel(
        panel_layout.pos,
        panel_layout.size,
        state.panel_collapsed,
        mouse,
        1.5,
        &ctx.colors_rt,
        &panel_buttons,
        state.selected_build_tool,
        |kind| match kind {
            PlanetBuildingKind::Base => Color::from_rgba(80, 220, 220, 255),
            PlanetBuildingKind::Builder => Color::from_rgba(255, 214, 102, 255),
            PlanetBuildingKind::Housing => Color::from_rgba(118, 203, 152, 255),
            PlanetBuildingKind::Factory => Color::from_rgba(170, 170, 196, 255),
            PlanetBuildingKind::Mine => Color::from_rgba(240, 170, 90, 255),
            PlanetBuildingKind::Warehouse => Color::from_rgba(116, 176, 224, 255),
            PlanetBuildingKind::Logistics => Color::from_rgba(255, 128, 128, 255),
            PlanetBuildingKind::Route => Color::from_rgba(220, 220, 120, 255),
        },
    );
    if panel_result.toggled {
        state.panel_collapsed = !state.panel_collapsed;
    }
    if let Some(option) = panel_result.clicked_option {
        state.selected_build_tool = option;
    }
    if let Some(tip) = panel_result.hovered_tip {
        let dim = measure_text(tip, None, ctx.font_sm as u16, 1.0);
        let pad = 8.0;
        let x = (mouse.x + 12.0).min(screen_width() - dim.width - pad * 2.0 - 4.0);
        let y = (mouse.y + 14.0).min(screen_height() - dim.height - pad * 2.0 - 4.0);
        draw_rectangle(
            x,
            y,
            dim.width + pad * 2.0,
            dim.height + pad * 2.0,
            ctx.colors_rt.tooltip_bg,
        );
        draw_rectangle_lines(
            x,
            y,
            dim.width + pad * 2.0,
            dim.height + pad * 2.0,
            1.0,
            ctx.colors_rt.tooltip_border,
        );
        draw_text(
            tip,
            x + pad,
            y + pad + dim.height - 2.0,
            ctx.font_sm,
            ctx.colors_rt.text_primary,
        );
    }
    if state.info_window.open {
        handle_window_drag(&mut state.info_window, mouse);
        let close = draw_window(
            &state.info_window,
            WindowStyle {
                bg: ctx.colors_rt.panel_bg,
                border: ctx.colors_rt.panel_border,
                title: ctx.colors_rt.text_primary,
                title_bg: ctx.colors_rt.button_base,
            },
            ctx.font_sm,
            1.4,
            mouse,
        );
        if close {
            state.info_window.open = false;
            state.info_target_cell = None;
        } else {
            let tx = state.info_window.rect.x + 10.0;
            let mut ty = state.info_window.rect.y + WINDOW_TITLE_HEIGHT + 18.0;
            if let Some(cell_index) = state.info_target_cell {
                if let Some(kind) = state.buildings.get(&cell_index) {
                    draw_text(
                        &format!("Cell: {}", cell_index),
                        tx,
                        ty,
                        ctx.font_sm,
                        ctx.colors_rt.text_secondary,
                    );
                    ty += 20.0;
                    draw_text(
                        &format!("Kind: {}", kind.label()),
                        tx,
                        ty,
                        ctx.font_sm,
                        ctx.colors_rt.text_secondary,
                    );
                    ty += 20.0;
                    if let Some(sim_world) = state.sim_world.as_ref() {
                        if let Some(cell) = sim_world.cells.get(cell_index as usize) {
                            draw_text(
                                &format!(
                                    "Surface: {}  Buildable:{}",
                                    surface_label(cell.surface),
                                    if cell.buildable { "yes" } else { "no" }
                                ),
                                tx,
                                ty,
                                ctx.font_sm,
                                ctx.colors_rt.text_secondary,
                            );
                            ty += 20.0;
                            let dep = cell
                                .deposit
                                .map(|d| format!("{} {}/{}", resource_label(d.kind), d.remaining_amount, d.initial_amount))
                                .unwrap_or_else(|| "No deposit".to_string());
                            draw_text(&dep, tx, ty, ctx.font_sm, ctx.colors_rt.text_secondary);
                            ty += 26.0;
                        }
                    }
                    let demolish_rect = Rect::new(tx, ty, 120.0, 28.0);
                    let (clicked, _) = ui_button(
                        demolish_rect,
                        "Demolish",
                        mouse,
                        ctx.font_sm,
                        ctx.button_colors,
                    );
                    if clicked {
                        state.confirm_window.open = true;
                        state.confirm_target_cell = Some(cell_index);
                    }
                }
            }
        }
    }
    if state.confirm_window.open {
        handle_window_drag(&mut state.confirm_window, mouse);
        let close = draw_window(
            &state.confirm_window,
            WindowStyle {
                bg: ctx.colors_rt.panel_bg,
                border: ctx.colors_rt.panel_border,
                title: ctx.colors_rt.text_primary,
                title_bg: ctx.colors_rt.button_base,
            },
            ctx.font_sm,
            1.4,
            mouse,
        );
        if close {
            state.confirm_window.open = false;
            state.confirm_target_cell = None;
        } else {
            let tx = state.confirm_window.rect.x + 10.0;
            let ty = state.confirm_window.rect.y + WINDOW_TITLE_HEIGHT + 22.0;
            let label = if let Some(cell) = state.confirm_target_cell {
                format!("Demolish building in cell {}?", cell)
            } else {
                "Demolish building?".to_string()
            };
            draw_text(&label, tx, ty, ctx.font_sm, ctx.colors_rt.text_secondary);
            let rect_cancel = Rect::new(tx, ty + 18.0, 90.0, 28.0);
            let rect_ok = Rect::new(tx + 100.0, ty + 18.0, 90.0, 28.0);
            let (clicked_cancel, _) =
                ui_button(rect_cancel, "Cancel", mouse, ctx.font_sm, ctx.button_colors);
            let (clicked_ok, _) =
                ui_button(rect_ok, "Delete", mouse, ctx.font_sm, ctx.button_colors);
            if clicked_cancel {
                state.confirm_window.open = false;
                state.confirm_target_cell = None;
            }
            if clicked_ok {
                if let Some(cell) = state.confirm_target_cell {
                    let _ = state.remove_building(cell);
                }
                state.confirm_window.open = false;
                state.confirm_target_cell = None;
            }
        }
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
    if let (Some(sim_world), Some(cell_index)) = (state.sim_world.as_ref(), hovered_cell) {
        if let Some(cell) = sim_world.cells.get(cell_index as usize) {
            let buildable = if state.can_build_on_sim_cell(cell_index) {
                "yes"
            } else {
                "no"
            };
            draw_text_ex(
                &format!(
                    "Cell {}  {}  Buildable:{}",
                    cell_index,
                    surface_label(cell.surface),
                    buildable
                ),
                20.0,
                64.0,
                TextParams {
                    font: greek_font,
                    font_size: 22,
                    color: Color::from_rgba(220, 238, 255, 255),
                    ..Default::default()
                },
            );
            let deposit_text = if let Some(deposit) = cell.deposit {
                format!(
                    "{} {}/{} grade:{}",
                    resource_label(deposit.kind),
                    deposit.remaining_amount,
                    deposit.initial_amount,
                    deposit.grade_permille
                )
            } else {
                "No deposit".to_string()
            };
            draw_text_ex(
                &deposit_text,
                20.0,
                86.0,
                TextParams {
                    font: greek_font,
                    font_size: 20,
                    color: Color::from_rgba(190, 220, 240, 255),
                    ..Default::default()
                },
            );
        }
    }
    let tool_label = match state.selected_build_tool {
        None => "None",
        Some(kind) => kind.label(),
    };
    draw_text_ex(
        &format!(
            "Tool: {}   [1 None] [2 Base] [3 Mine] [4 Builder] [5 Housing] [6 Factory] [7 Warehouse] [8 Logistics] [9 Route] [RMB place] [Del remove]",
            tool_label
        ),
        20.0,
        screen_height() - 56.0,
        TextParams {
            font: greek_font,
            font_size: 20,
            color: Color::from_rgba(180, 210, 230, 255),
            ..Default::default()
        },
    );
    let mut stock_lines: Vec<String> = state
        .stockpiles
        .iter()
        .filter(|(_, amount)| **amount > 0)
        .map(|(kind, amount)| format!("{}: {}", resource_label(*kind), amount))
        .collect();
    stock_lines.sort();
    if stock_lines.is_empty() {
        draw_text_ex(
            "Stockpile: empty",
            20.0,
            screen_height() - 34.0,
            TextParams {
                font: greek_font,
                font_size: 18,
                color: Color::from_rgba(150, 185, 210, 255),
                ..Default::default()
            },
        );
    } else {
        draw_text_ex(
            &format!("Stockpile: {}", stock_lines.join(" | ")),
            20.0,
            screen_height() - 34.0,
            TextParams {
                font: greek_font,
                font_size: 18,
                color: Color::from_rgba(150, 185, 210, 255),
                ..Default::default()
            },
        );
    }
}

