use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

use macroquad::prelude::*;

use crate::core::planet_grid::HexGrid;
use crate::core::planet_texture::{PlanetNoiseConfig, height_value};

const SNAPSHOT_MAGIC: &[u8; 4] = b"PRS1";
const SNAPSHOT_VERSION: u16 = 2;
const COASTAL_HEIGHT_BAND: f32 = 0.08;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanetSurfaceClass {
    Land,
    Coast,
    Water,
}

impl PlanetSurfaceClass {
    fn to_u8(self) -> u8 {
        match self {
            PlanetSurfaceClass::Land => 0,
            PlanetSurfaceClass::Coast => 1,
            PlanetSurfaceClass::Water => 2,
        }
    }

    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(PlanetSurfaceClass::Land),
            1 => Some(PlanetSurfaceClass::Coast),
            2 => Some(PlanetSurfaceClass::Water),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LithologyKind {
    IgneousMafic,
    IgneousFelsic,
    Carbonate,
    Clastic,
    Evaporite,
    PhosphoriteHost,
}

impl LithologyKind {
    fn to_u8(self) -> u8 {
        match self {
            LithologyKind::IgneousMafic => 0,
            LithologyKind::IgneousFelsic => 1,
            LithologyKind::Carbonate => 2,
            LithologyKind::Clastic => 3,
            LithologyKind::Evaporite => 4,
            LithologyKind::PhosphoriteHost => 5,
        }
    }

    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(LithologyKind::IgneousMafic),
            1 => Some(LithologyKind::IgneousFelsic),
            2 => Some(LithologyKind::Carbonate),
            3 => Some(LithologyKind::Clastic),
            4 => Some(LithologyKind::Evaporite),
            5 => Some(LithologyKind::PhosphoriteHost),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlanetResourceKind {
    Iron,
    Copper,
    Gold,
    Lead,
    Zinc,
    Aluminum,
    Lithium,
    Phosphate,
    Stone,
    FreshWater,
    Salt,
}

impl PlanetResourceKind {
    fn to_u8(self) -> u8 {
        match self {
            PlanetResourceKind::Iron => 0,
            PlanetResourceKind::Copper => 1,
            PlanetResourceKind::Gold => 2,
            PlanetResourceKind::Lead => 3,
            PlanetResourceKind::Zinc => 4,
            PlanetResourceKind::Aluminum => 10,
            PlanetResourceKind::Lithium => 5,
            PlanetResourceKind::Phosphate => 6,
            PlanetResourceKind::Stone => 7,
            PlanetResourceKind::FreshWater => 8,
            PlanetResourceKind::Salt => 9,
        }
    }

    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(PlanetResourceKind::Iron),
            1 => Some(PlanetResourceKind::Copper),
            2 => Some(PlanetResourceKind::Gold),
            3 => Some(PlanetResourceKind::Lead),
            4 => Some(PlanetResourceKind::Zinc),
            10 => Some(PlanetResourceKind::Aluminum),
            5 => Some(PlanetResourceKind::Lithium),
            6 => Some(PlanetResourceKind::Phosphate),
            7 => Some(PlanetResourceKind::Stone),
            8 => Some(PlanetResourceKind::FreshWater),
            9 => Some(PlanetResourceKind::Salt),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanetDeposit {
    pub kind: PlanetResourceKind,
    pub initial_amount: u32,
    pub remaining_amount: u32,
    pub grade_permille: u16,
}

#[derive(Clone, Debug)]
pub struct PlanetCellState {
    pub surface: PlanetSurfaceClass,
    pub height: f32,
    pub slope: f32,
    pub lithology: LithologyKind,
    pub deposits: Vec<PlanetDeposit>,
    pub buildable: bool,
}

pub struct PlanetSimWorld {
    pub sim_freq: i32,
    pub seed: u64,
    pub config_hash: u64,
    pub cells: Vec<PlanetCellState>,
    dirty: bool,
}

#[cfg(test)]
pub(crate) fn sim_world_for_tests(cells: Vec<PlanetCellState>) -> PlanetSimWorld {
    PlanetSimWorld {
        sim_freq: 1,
        seed: 0,
        config_hash: 0,
        cells,
        dirty: false,
    }
}

impl PlanetSimWorld {
    pub fn can_build(&self, cell_index: u32) -> bool {
        self.cells
            .get(cell_index as usize)
            .map(|cell| cell.buildable)
            .unwrap_or(false)
    }

    #[allow(dead_code)]
    pub fn extract_from_cell(
        &mut self,
        cell_index: u32,
        amount: u32,
    ) -> Vec<(PlanetResourceKind, u32)> {
        let Some(cell) = self.cells.get_mut(cell_index as usize) else {
            return Vec::new();
        };
        if amount == 0 || cell.deposits.is_empty() {
            return Vec::new();
        }
        let active_indices: Vec<usize> = cell
            .deposits
            .iter()
            .enumerate()
            .filter(|(_, deposit)| deposit.remaining_amount > 0)
            .map(|(index, _)| index)
            .collect();
        if active_indices.is_empty() {
            return Vec::new();
        }

        let mut remaining = amount;
        let mut mined_by_kind: std::collections::HashMap<PlanetResourceKind, u32> =
            std::collections::HashMap::new();
        while remaining > 0 {
            let mut mined_any = false;
            for &index in active_indices.iter() {
                if remaining == 0 {
                    break;
                }
                let deposit = &mut cell.deposits[index];
                if deposit.remaining_amount == 0 {
                    continue;
                }
                deposit.remaining_amount -= 1;
                *mined_by_kind.entry(deposit.kind).or_insert(0) += 1;
                remaining -= 1;
                mined_any = true;
            }
            if !mined_any {
                break;
            }
        }

        if mined_by_kind.is_empty() {
            return Vec::new();
        }
        self.dirty = true;
        let mut mined = mined_by_kind.into_iter().collect::<Vec<_>>();
        mined.sort_by_key(|(kind, _)| kind.to_u8());
        mined
    }

    pub fn save_if_dirty(&mut self, path: &str) {
        if !self.dirty {
            return;
        }
        save_snapshot(path, self);
        self.dirty = false;
    }
}

#[derive(Clone, Copy)]
struct CellMetrics {
    dir: Vec3,
    height: f32,
    slope: f32,
    dryness: f32,
    fracture: f32,
    volcanic: f32,
}

pub fn build_or_load_sim_world(
    path: &str,
    sim_grid: &HexGrid,
    config: &PlanetNoiseConfig,
    seed: u64,
) -> PlanetSimWorld {
    let config_hash = sim_config_hash(config, seed);
    if let Some(world) = load_snapshot(path, sim_grid.freq, seed, config_hash, sim_grid, config) {
        return world;
    }
    let world = generate_world(sim_grid, config, seed, config_hash);
    save_snapshot(path, &world);
    world
}

pub fn classify_surface(height: f32, sea_level: f32) -> PlanetSurfaceClass {
    if height < sea_level {
        PlanetSurfaceClass::Water
    } else if height < sea_level + COASTAL_HEIGHT_BAND {
        PlanetSurfaceClass::Coast
    } else {
        PlanetSurfaceClass::Land
    }
}

fn generate_world(
    sim_grid: &HexGrid,
    config: &PlanetNoiseConfig,
    seed: u64,
    config_hash: u64,
) -> PlanetSimWorld {
    let neighbors = build_vertex_neighbors(sim_grid);
    let mut metrics: Vec<CellMetrics> = Vec::with_capacity(sim_grid.vertices.len());

    for (index, dir) in sim_grid.vertices.iter().enumerate() {
        let dir = dir.normalize();
        let height = height_value(dir, config);
        let mut slope_acc = 0.0_f32;
        let mut slope_count = 0usize;
        for &neighbor in neighbors[index].iter() {
            let n_dir = sim_grid.vertices[neighbor as usize].normalize();
            let n_h = height_value(n_dir, config);
            slope_acc += (n_h - height).abs();
            slope_count += 1;
        }
        let slope = if slope_count == 0 {
            0.0
        } else {
            slope_acc / slope_count as f32
        };
        let dryness = rand01_dir(dir, seed ^ 0xA35D_4A7C_921B_8E13);
        let fracture = rand01_dir(dir, seed ^ 0x1C22_7E39_D040_BA51);
        let volcanic = rand01_dir(dir, seed ^ 0x6FF3_0A8E_2A1D_C993);
        metrics.push(CellMetrics {
            dir,
            height,
            slope,
            dryness,
            fracture,
            volcanic,
        });
    }

    let mut cells: Vec<PlanetCellState> = Vec::with_capacity(metrics.len());
    for metric in metrics.iter() {
        let surface = classify_surface(metric.height, config.sea_level);
        let lithology = choose_lithology(metric, surface, config, seed);
        let deposits = generate_deposits(metric, surface, lithology, config, seed);
        let buildable = matches!(surface, PlanetSurfaceClass::Land);
        cells.push(PlanetCellState {
            surface,
            height: metric.height,
            slope: metric.slope,
            lithology,
            deposits,
            buildable,
        });
    }

    PlanetSimWorld {
        sim_freq: sim_grid.freq,
        seed,
        config_hash,
        cells,
        dirty: false,
    }
}

fn choose_lithology(
    metric: &CellMetrics,
    surface: PlanetSurfaceClass,
    config: &PlanetNoiseConfig,
    seed: u64,
) -> LithologyKind {
    let latitude = metric.dir.y.abs();
    let relief = (metric.height - config.sea_level).max(0.0);
    let r = rand01_dir(metric.dir, seed ^ 0x9D5C_015F_33B2_7781);
    let mafic_score = metric.volcanic * 0.75 + metric.slope * 4.0 + relief * 0.6;
    let felsic_score = (1.0 - metric.volcanic) * 0.55 + relief * 0.9 + r * 0.35;
    let carbonate_score = (1.0 - metric.slope * 5.0).clamp(0.0, 1.0) * (1.0 - latitude * 0.45);
    let clastic_score = (1.0 - metric.slope * 3.0).clamp(0.0, 1.0) * 0.65 + metric.fracture * 0.2;
    let evaporite_score = metric.dryness * 0.8
        + if matches!(surface, PlanetSurfaceClass::Coast) {
            0.4
        } else {
            0.0
        };
    let phosphorite_score = if matches!(surface, PlanetSurfaceClass::Coast) {
        0.55
    } else {
        0.1
    } + (1.0 - metric.slope * 6.0).clamp(0.0, 1.0) * 0.35;

    let mut best = (LithologyKind::Clastic, clastic_score);
    for (kind, score) in [
        (LithologyKind::IgneousMafic, mafic_score),
        (LithologyKind::IgneousFelsic, felsic_score),
        (LithologyKind::Carbonate, carbonate_score),
        (LithologyKind::Clastic, clastic_score),
        (LithologyKind::Evaporite, evaporite_score),
        (LithologyKind::PhosphoriteHost, phosphorite_score),
    ] {
        if score > best.1 {
            best = (kind, score);
        }
    }
    best.0
}

fn generate_deposits(
    metric: &CellMetrics,
    surface: PlanetSurfaceClass,
    lithology: LithologyKind,
    config: &PlanetNoiseConfig,
    seed: u64,
) -> Vec<PlanetDeposit> {
    let sea = config.sea_level;
    let h_relief = (metric.height - sea).max(0.0);
    let seabed_depth = (sea - metric.height).max(0.0);
    let is_water = matches!(surface, PlanetSurfaceClass::Water);
    let structure = metric.fracture;
    let volcanic = metric.volcanic;
    let p_iron = if matches!(
        surface,
        PlanetSurfaceClass::Land | PlanetSurfaceClass::Coast
    ) {
        lithology_match(lithology, LithologyKind::IgneousMafic, 0.65)
            + lithology_match(lithology, LithologyKind::IgneousFelsic, 0.25)
            + h_relief * 0.4
            + metric.slope * 2.4
    } else if is_water {
        lithology_match(lithology, LithologyKind::IgneousMafic, 0.34)
            + seabed_depth * 0.9
            + metric.slope * 1.1
    } else {
        0.0
    };
    let p_copper = if matches!(
        surface,
        PlanetSurfaceClass::Land | PlanetSurfaceClass::Coast
    ) {
        volcanic * 0.8 + structure * 0.4 + metric.slope * 3.0 + h_relief * 0.25
    } else if is_water {
        volcanic * 0.55 + structure * 0.35 + metric.slope * 1.4 + seabed_depth * 0.65
    } else {
        0.0
    };
    let p_gold = if matches!(
        surface,
        PlanetSurfaceClass::Land | PlanetSurfaceClass::Coast
    ) {
        volcanic * 0.5 + structure * 0.7 + metric.slope * 2.2 + h_relief * 0.25
    } else if is_water {
        volcanic * 0.32 + structure * 0.35 + metric.slope * 0.95 + seabed_depth * 0.25
    } else {
        0.0
    };
    let p_lead = if matches!(
        surface,
        PlanetSurfaceClass::Land | PlanetSurfaceClass::Coast
    ) {
        lithology_match(lithology, LithologyKind::Carbonate, 0.7) + structure * 0.5
    } else if is_water {
        lithology_match(lithology, LithologyKind::Carbonate, 0.26) + seabed_depth * 0.6
    } else {
        0.0
    };
    let p_zinc = if matches!(
        surface,
        PlanetSurfaceClass::Land | PlanetSurfaceClass::Coast
    ) {
        lithology_match(lithology, LithologyKind::Carbonate, 0.75) + structure * 0.45
    } else if is_water {
        lithology_match(lithology, LithologyKind::Carbonate, 0.24) + seabed_depth * 0.62
    } else {
        0.0
    };
    let p_aluminum = if matches!(
        surface,
        PlanetSurfaceClass::Land | PlanetSurfaceClass::Coast
    ) {
        lithology_match(lithology, LithologyKind::Clastic, 0.55)
            + lithology_match(lithology, LithologyKind::IgneousFelsic, 0.45)
            + (1.0 - metric.slope * 4.5).clamp(0.0, 1.0) * 0.4
            + metric.dryness * 0.22
    } else {
        0.0
    };
    let p_lithium = if matches!(surface, PlanetSurfaceClass::Land) {
        lithology_match(lithology, LithologyKind::Evaporite, 0.75)
            + metric.dryness * 0.65
            + (1.0 - metric.slope * 5.0).clamp(0.0, 1.0) * 0.35
    } else {
        0.0
    };
    let p_phosphate = if matches!(surface, PlanetSurfaceClass::Coast) {
        lithology_match(lithology, LithologyKind::PhosphoriteHost, 0.9)
            + (1.0 - metric.slope * 4.0).clamp(0.0, 1.0) * 0.5
    } else if is_water {
        lithology_match(lithology, LithologyKind::PhosphoriteHost, 0.45)
            + (1.0 - metric.slope * 3.0).clamp(0.0, 1.0) * 0.35
    } else {
        0.0
    };
    let p_stone = if !is_water {
        0.35 + metric.slope * 1.2
    } else {
        0.2 + seabed_depth * 0.8 + metric.slope * 0.6
    };
    let p_fresh_water = if matches!(surface, PlanetSurfaceClass::Land) {
        (1.0 - metric.dryness) * 0.6 + (1.0 - metric.slope * 4.0).clamp(0.0, 1.0) * 0.3
    } else {
        0.0
    };
    let p_salt = if matches!(surface, PlanetSurfaceClass::Coast) {
        lithology_match(lithology, LithologyKind::Evaporite, 0.8) + metric.dryness * 0.5
    } else if is_water {
        lithology_match(lithology, LithologyKind::Evaporite, 0.35) + metric.dryness * 0.2
    } else {
        0.0
    };

    let mut deposits = Vec::new();
    for (kind, score) in [
        (
            PlanetResourceKind::Iron,
            p_iron + mineral_noise(metric.dir, seed, PlanetResourceKind::Iron) * 0.7,
        ),
        (
            PlanetResourceKind::Copper,
            p_copper + mineral_noise(metric.dir, seed, PlanetResourceKind::Copper) * 0.72,
        ),
        (
            PlanetResourceKind::Gold,
            p_gold * 0.55 + mineral_noise(metric.dir, seed, PlanetResourceKind::Gold) * 0.66,
        ),
        (
            PlanetResourceKind::Lead,
            p_lead + mineral_noise(metric.dir, seed, PlanetResourceKind::Lead) * 0.68,
        ),
        (
            PlanetResourceKind::Zinc,
            p_zinc + mineral_noise(metric.dir, seed, PlanetResourceKind::Zinc) * 0.68,
        ),
        (
            PlanetResourceKind::Aluminum,
            p_aluminum + mineral_noise(metric.dir, seed, PlanetResourceKind::Aluminum) * 0.65,
        ),
        (
            PlanetResourceKind::Lithium,
            p_lithium + mineral_noise(metric.dir, seed, PlanetResourceKind::Lithium) * 0.74,
        ),
        (
            PlanetResourceKind::Phosphate,
            p_phosphate + mineral_noise(metric.dir, seed, PlanetResourceKind::Phosphate) * 0.62,
        ),
    ] {
        if let Some(deposit) = deposit_from_score(kind, score) {
            deposits.push(deposit);
        }
    }

    if let Some(deposit) = deposit_from_score(PlanetResourceKind::Stone, p_stone) {
        deposits.push(deposit);
    }
    if let Some(deposit) = deposit_from_score(PlanetResourceKind::Salt, p_salt) {
        deposits.push(deposit);
    }
    if !is_water {
        if let Some(deposit) = deposit_from_score(PlanetResourceKind::FreshWater, p_fresh_water) {
            deposits.push(deposit);
        }
    }

    deposits.sort_by_key(|deposit| deposit.kind.to_u8());
    deposits
}

fn deposit_from_score(kind: PlanetResourceKind, score: f32) -> Option<PlanetDeposit> {
    let threshold = match kind {
        PlanetResourceKind::Gold => 0.84,
        PlanetResourceKind::Lithium => 0.70,
        PlanetResourceKind::Phosphate => 0.68,
        PlanetResourceKind::Salt => 0.66,
        PlanetResourceKind::FreshWater => 0.63,
        PlanetResourceKind::Stone => 0.55,
        PlanetResourceKind::Aluminum => 0.64,
        _ => 0.62,
    };
    if score < threshold {
        return None;
    }

    let grade = ((score * 620.0) + 280.0).clamp(80.0, 1000.0) as u16;
    let base_amount = match kind {
        PlanetResourceKind::Gold => 3_000,
        PlanetResourceKind::Lithium => 12_000,
        PlanetResourceKind::Phosphate => 15_000,
        PlanetResourceKind::FreshWater => 18_000,
        PlanetResourceKind::Stone => 28_000,
        PlanetResourceKind::Salt => 14_000,
        PlanetResourceKind::Aluminum => 20_000,
        _ => 16_000,
    };
    let richness = (0.6 + score * 0.8).clamp(0.55, 1.55);
    let initial_amount = (base_amount as f32 * richness) as u32;
    Some(PlanetDeposit {
        kind,
        initial_amount,
        remaining_amount: initial_amount,
        grade_permille: grade,
    })
}

fn mineral_noise(dir: Vec3, seed: u64, kind: PlanetResourceKind) -> f32 {
    let kind_seed = match kind {
        PlanetResourceKind::Iron => 0x00A1_1023_u64,
        PlanetResourceKind::Copper => 0x00B2_2045_u64,
        PlanetResourceKind::Gold => 0x00C3_3089_u64,
        PlanetResourceKind::Lead => 0x00D4_40AB_u64,
        PlanetResourceKind::Zinc => 0x00E5_50CD_u64,
        PlanetResourceKind::Aluminum => 0x00F6_60EF_u64,
        PlanetResourceKind::Lithium => 0x0017_7123_u64,
        PlanetResourceKind::Phosphate => 0x0028_8235_u64,
        _ => 0x0039_9347_u64,
    };
    fbm3_resource(
        dir.x * 2.6 + 11.0,
        dir.y * 2.6 - 7.0,
        dir.z * 2.6 + 5.0,
        seed ^ kind_seed,
        4,
    )
}

fn lithology_match(current: LithologyKind, wanted: LithologyKind, value: f32) -> f32 {
    if current == wanted {
        value
    } else {
        value * 0.16
    }
}

fn build_vertex_neighbors(grid: &HexGrid) -> Vec<Vec<u32>> {
    let mut sets: Vec<std::collections::HashSet<u32>> = (0..grid.vertices.len())
        .map(|_| std::collections::HashSet::new())
        .collect();
    for face in grid.faces.iter() {
        let a = face[0];
        let b = face[1];
        let c = face[2];
        sets[a as usize].insert(b);
        sets[a as usize].insert(c);
        sets[b as usize].insert(a);
        sets[b as usize].insert(c);
        sets[c as usize].insert(a);
        sets[c as usize].insert(b);
    }
    sets.into_iter()
        .map(|set| set.into_iter().collect::<Vec<_>>())
        .collect()
}

fn sim_config_hash(config: &PlanetNoiseConfig, seed: u64) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for value in [
        config.noise_scale.to_bits() as u64,
        config.height_amp.to_bits() as u64,
        config.height_bias.to_bits() as u64,
        config.lat_bias.to_bits() as u64,
        config.sea_level.to_bits() as u64,
        config.ice_start.to_bits() as u64,
        config.ice_strength.to_bits() as u64,
        config.subdivisions as u64,
        seed,
    ] {
        h ^= value;
        h = h.wrapping_mul(0x1000_0000_01B3);
    }
    h
}

fn rand01_dir(dir: Vec3, seed: u64) -> f32 {
    let xi = (dir.x * 100_000.0) as i64 as u64;
    let yi = (dir.y * 100_000.0) as i64 as u64;
    let zi = (dir.z * 100_000.0) as i64 as u64;
    let mut x = seed
        ^ xi.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ yi.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ zi.wrapping_mul(0x1656_67B1_9E37_79F9);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x as f64 / u64::MAX as f64) as f32
}

fn load_snapshot(
    path: &str,
    sim_freq: i32,
    seed: u64,
    config_hash: u64,
    sim_grid: &HexGrid,
    config: &PlanetNoiseConfig,
) -> Option<PlanetSimWorld> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).ok()?;
    if &magic != SNAPSHOT_MAGIC {
        return None;
    }
    let version = read_u16(&mut reader)?;
    if version != SNAPSHOT_VERSION {
        return None;
    }
    let stored_freq = read_i32(&mut reader)?;
    if stored_freq != sim_freq {
        return None;
    }
    let stored_seed = read_u64(&mut reader)?;
    if stored_seed != seed {
        return None;
    }
    let stored_hash = read_u64(&mut reader)?;
    if stored_hash != config_hash {
        return None;
    }
    let cell_count = read_u32(&mut reader)? as usize;
    if cell_count != sim_grid.vertices.len() {
        return None;
    }

    let mut cells: Vec<PlanetCellState> = Vec::with_capacity(cell_count);
    for idx in 0..cell_count {
        let surface = PlanetSurfaceClass::from_u8(read_u8(&mut reader)?)?;
        let lithology = LithologyKind::from_u8(read_u8(&mut reader)?)?;
        let buildable = read_u8(&mut reader)? != 0;
        let deposit_count = read_u16(&mut reader)? as usize;
        let dir = sim_grid.vertices[idx].normalize();
        let height = height_value(dir, config);
        let slope = 0.0;
        let mut deposits = Vec::with_capacity(deposit_count);
        for _ in 0..deposit_count {
            let kind = PlanetResourceKind::from_u8(read_u8(&mut reader)?)?;
            let grade = read_u16(&mut reader)?;
            let initial_amount = read_u32(&mut reader)?;
            let remaining_amount = read_u32(&mut reader)?;
            deposits.push(PlanetDeposit {
                kind,
                initial_amount,
                remaining_amount,
                grade_permille: grade,
            });
        }
        cells.push(PlanetCellState {
            surface,
            height,
            slope,
            lithology,
            deposits,
            buildable,
        });
    }

    let neighbors = build_vertex_neighbors(sim_grid);
    for (index, neighbor_list) in neighbors.iter().enumerate() {
        let height = cells[index].height;
        let mut slope_acc = 0.0;
        for &neighbor in neighbor_list.iter() {
            slope_acc += (cells[neighbor as usize].height - height).abs();
        }
        cells[index].slope = if neighbor_list.is_empty() {
            0.0
        } else {
            slope_acc / neighbor_list.len() as f32
        };
    }

    Some(PlanetSimWorld {
        sim_freq,
        seed,
        config_hash,
        cells,
        dirty: false,
    })
}

fn save_snapshot(path: &str, world: &PlanetSimWorld) {
    let Ok(file) = File::create(path) else {
        return;
    };
    let mut writer = BufWriter::new(file);
    let _ = writer.write_all(SNAPSHOT_MAGIC);
    let _ = writer.write_all(&SNAPSHOT_VERSION.to_le_bytes());
    let _ = writer.write_all(&world.sim_freq.to_le_bytes());
    let _ = writer.write_all(&world.seed.to_le_bytes());
    let _ = writer.write_all(&world.config_hash.to_le_bytes());
    let _ = writer.write_all(&(world.cells.len() as u32).to_le_bytes());
    for cell in world.cells.iter() {
        let _ = writer.write_all(&[cell.surface.to_u8()]);
        let _ = writer.write_all(&[cell.lithology.to_u8()]);
        let _ = writer.write_all(&[if cell.buildable { 1 } else { 0 }]);
        let _ = writer.write_all(&(cell.deposits.len() as u16).to_le_bytes());
        for deposit in cell.deposits.iter() {
            let _ = writer.write_all(&[deposit.kind.to_u8()]);
            let _ = writer.write_all(&deposit.grade_permille.to_le_bytes());
            let _ = writer.write_all(&deposit.initial_amount.to_le_bytes());
            let _ = writer.write_all(&deposit.remaining_amount.to_le_bytes());
        }
    }
    let _ = writer.flush();
}

fn resource_noise3(x: f32, y: f32, z: f32, seed: u64) -> f32 {
    let x0 = x.floor();
    let y0 = y.floor();
    let z0 = z.floor();
    let tx = x - x0;
    let ty = y - y0;
    let tz = z - z0;

    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    let tx = smooth(tx);
    let ty = smooth(ty);
    let tz = smooth(tz);

    let sample = |ix: f32, iy: f32, iz: f32| {
        let p = vec3(ix, iy, iz).normalize_or_zero();
        rand01_dir(p, seed)
    };
    let c000 = sample(x0, y0, z0);
    let c100 = sample(x0 + 1.0, y0, z0);
    let c010 = sample(x0, y0 + 1.0, z0);
    let c110 = sample(x0 + 1.0, y0 + 1.0, z0);
    let c001 = sample(x0, y0, z0 + 1.0);
    let c101 = sample(x0 + 1.0, y0, z0 + 1.0);
    let c011 = sample(x0, y0 + 1.0, z0 + 1.0);
    let c111 = sample(x0 + 1.0, y0 + 1.0, z0 + 1.0);

    let c00 = c000 + (c100 - c000) * tx;
    let c10 = c010 + (c110 - c010) * tx;
    let c01 = c001 + (c101 - c001) * tx;
    let c11 = c011 + (c111 - c011) * tx;
    let c0 = c00 + (c10 - c00) * ty;
    let c1 = c01 + (c11 - c01) * ty;
    c0 + (c1 - c0) * tz
}

fn fbm3_resource(x: f32, y: f32, z: f32, seed: u64, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for octave in 0..octaves {
        sum += resource_noise3(x * freq, y * freq, z * freq, seed ^ octave as u64) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    if norm <= 0.0 { 0.0 } else { sum / norm }
}

fn read_u8(reader: &mut BufReader<File>) -> Option<u8> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf).ok()?;
    Some(buf[0])
}

fn read_u16(reader: &mut BufReader<File>) -> Option<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf).ok()?;
    Some(u16::from_le_bytes(buf))
}

fn read_u32(reader: &mut BufReader<File>) -> Option<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).ok()?;
    Some(u32::from_le_bytes(buf))
}

fn read_i32(reader: &mut BufReader<File>) -> Option<i32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).ok()?;
    Some(i32::from_le_bytes(buf))
}

fn read_u64(reader: &mut BufReader<File>) -> Option<u64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).ok()?;
    Some(u64::from_le_bytes(buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_surface_boundaries() {
        let sea = 0.62;
        assert_eq!(
            classify_surface(sea - 0.001, sea),
            PlanetSurfaceClass::Water
        );
        assert_eq!(classify_surface(sea, sea), PlanetSurfaceClass::Coast);
        assert_eq!(classify_surface(sea + 0.04, sea), PlanetSurfaceClass::Coast);
        assert_eq!(classify_surface(sea + 0.09, sea), PlanetSurfaceClass::Land);
    }

    #[test]
    fn extract_does_not_underflow() {
        let mut world = PlanetSimWorld {
            sim_freq: 32,
            seed: 1,
            config_hash: 2,
            cells: vec![PlanetCellState {
                surface: PlanetSurfaceClass::Land,
                height: 0.7,
                slope: 0.2,
                lithology: LithologyKind::Clastic,
                deposits: vec![PlanetDeposit {
                    kind: PlanetResourceKind::Stone,
                    initial_amount: 10,
                    remaining_amount: 3,
                    grade_permille: 500,
                }],
                buildable: true,
            }],
            dirty: false,
        };
        assert_eq!(
            world.extract_from_cell(0, 7),
            vec![(PlanetResourceKind::Stone, 3)]
        );
        assert_eq!(world.cells[0].deposits[0].remaining_amount, 0);
    }

    #[test]
    fn extract_from_cell_splits_output_across_multiple_resources() {
        let mut world = PlanetSimWorld {
            sim_freq: 32,
            seed: 1,
            config_hash: 2,
            cells: vec![PlanetCellState {
                surface: PlanetSurfaceClass::Land,
                height: 0.7,
                slope: 0.2,
                lithology: LithologyKind::Clastic,
                deposits: vec![
                    PlanetDeposit {
                        kind: PlanetResourceKind::Iron,
                        initial_amount: 10,
                        remaining_amount: 3,
                        grade_permille: 500,
                    },
                    PlanetDeposit {
                        kind: PlanetResourceKind::Copper,
                        initial_amount: 10,
                        remaining_amount: 2,
                        grade_permille: 500,
                    },
                ],
                buildable: true,
            }],
            dirty: false,
        };

        assert_eq!(
            world.extract_from_cell(0, 4),
            vec![
                (PlanetResourceKind::Iron, 2),
                (PlanetResourceKind::Copper, 2),
            ]
        );
        assert_eq!(world.cells[0].deposits[0].remaining_amount, 1);
        assert_eq!(world.cells[0].deposits[1].remaining_amount, 0);
    }

    #[test]
    fn fresh_water_is_not_generated_for_water_cells() {
        let config = PlanetNoiseConfig::default();
        let metric = CellMetrics {
            dir: vec3(0.0, 1.0, 0.0),
            height: config.sea_level - 0.1,
            slope: 0.02,
            dryness: 0.0,
            fracture: 0.1,
            volcanic: 0.1,
        };
        let deposits = generate_deposits(
            &metric,
            PlanetSurfaceClass::Water,
            LithologyKind::Clastic,
            &config,
            42,
        );
        assert!(
            deposits
                .iter()
                .all(|deposit| deposit.kind != PlanetResourceKind::FreshWater)
        );
    }
}
