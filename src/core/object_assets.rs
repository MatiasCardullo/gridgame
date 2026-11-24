use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use macroquad::prelude::{Color, Mesh, Quat, Vec2, Vec3, Vertex, draw_mesh, vec3};
use serde::{Deserialize, Serialize};

use crate::core::MachineBlockType;

pub const NORMALIZED_MODEL_EXTENT: f32 = 2.4;
pub const BUILDING_PREVIEW_CELL_RADIUS: f32 = 0.9;
pub const BUILDING_PREVIEW_SCALE: f32 = 0.28;
pub const BUILDING_PREVIEW_CELL_GUIDE_RADIUS: f32 =
    BUILDING_PREVIEW_CELL_RADIUS / BUILDING_PREVIEW_SCALE;
pub const BUILDING_CELL_FILL_RATIO: f32 =
    NORMALIZED_MODEL_EXTENT * BUILDING_PREVIEW_SCALE / BUILDING_PREVIEW_CELL_RADIUS;

pub fn mesh_center_and_scale(mesh: &EditableMesh) -> (Vec3, f32) {
    let Some(first) = mesh.vertices.first() else {
        return (Vec3::ZERO, 1.0);
    };
    let mut min = first.position;
    let mut max = first.position;
    for vertex in &mesh.vertices[1..] {
        min = min.min(vertex.position);
        max = max.max(vertex.position);
    }
    let center = (min + max) * 0.5;
    let extent = max - min;
    let scale = NORMALIZED_MODEL_EXTENT / extent.x.max(extent.y).max(extent.z).max(0.001);
    (center, scale)
}

pub fn building_model_scale_for_cell(cell_radius: f32) -> f32 {
    cell_radius * BUILDING_CELL_FILL_RATIO / NORMALIZED_MODEL_EXTENT
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectCategory {
    Buildings,
    Units,
    MachineBlocks,
}

impl ObjectCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Buildings => "Buildings",
            Self::Units => "Units",
            Self::MachineBlocks => "Machine blocks",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObjectId {
    PlanetBase,
    PlanetBuilder,
    PlanetHousing,
    PlanetFactory,
    PlanetMine,
    PlanetWarehouse,
    PlanetRefuel,
    PlanetLogistics,
    PlanetUnit,
    ConveyorBelt,
    MechanicArm,
    Chest,
    Assembler,
    Furnace,
    Pallet,
    Crate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectDefinition {
    pub id: ObjectId,
    pub category: ObjectCategory,
    pub label: &'static str,
    pub path: &'static str,
    pub states_path: Option<&'static str>,
    pub placement_profile: PlacementProfile,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementProfile {
    #[default]
    CellCenter,
    TriangleSlots,
    NetworkSegments,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantMode {
    #[default]
    None,
    Radial,
    Mirror,
    Custom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorAxis {
    #[default]
    Longitudinal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadialNeighborState {
    Empty,
    Route,
    OtherBuilding,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "condition", rename_all = "snake_case")]
pub enum FaceGroupProgram {
    #[default]
    Geometry,
    RadialRepeat,
    IfNeighbor(RadialNeighborState),
}

#[cfg(test)]
pub fn reflected_belt_state_key(key: &str) -> Result<String, String> {
    let mut parts = key.split('-');
    let front = parts
        .next()
        .and_then(|part| part.strip_prefix('f'))
        .and_then(|value| value.parse::<u8>().ok())
        .ok_or_else(|| format!("invalid belt state key '{key}'"))?;
    let back = parts
        .next()
        .and_then(|part| part.strip_prefix('b'))
        .and_then(|value| value.parse::<u8>().ok())
        .ok_or_else(|| format!("invalid belt state key '{key}'"))?;
    let side = parts
        .next()
        .and_then(|part| part.strip_prefix('s'))
        .and_then(|value| u8::from_str_radix(value, 16).ok())
        .ok_or_else(|| format!("invalid belt state key '{key}'"))?;
    if parts.next().is_some() || front > 6 || back > 6 || side > 0x0f {
        return Err(format!("invalid belt state key '{key}'"));
    }
    let reflect_orientation = |value: u8| if value == 0 { 0 } else { 7 - value };
    let reflected_side =
        (side & 0b0001) << 3 | (side & 0b0010) << 1 | (side & 0b0100) >> 1 | (side & 0b1000) >> 3;
    Ok(format!(
        "f{}-b{}-s{reflected_side:02x}",
        reflect_orientation(front),
        reflect_orientation(back)
    ))
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VariantConfig {
    pub mode: VariantMode,
    #[serde(default)]
    pub placement_profile: Option<PlacementProfile>,
    pub radial_sector_groups: Vec<RadialSectorGroup>,
    pub mirror_axis: MirrorAxis,
    pub mirror_pairs: Vec<MirrorPair>,
    pub custom_patterns: Vec<CustomVariantPattern>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadialSectorGroup {
    pub sides: usize,
    pub sector: usize,
    pub face_group_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorPair {
    pub source_key: String,
    pub reflected_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomVariantPattern {
    pub name: String,
    pub condition_key: String,
    pub state_key: String,
}

impl VariantConfig {
    pub fn validate(&self) -> Result<(), String> {
        let mut sectors = HashSet::new();
        let mut group_ids = HashSet::new();
        for assignment in &self.radial_sector_groups {
            if assignment.sides != 5 && assignment.sides != 6 {
                return Err(format!(
                    "radial sector group has unsupported side count {}",
                    assignment.sides
                ));
            }
            if assignment.sector >= assignment.sides {
                return Err(format!(
                    "radial sector {} is outside the 0..{} range",
                    assignment.sector, assignment.sides
                ));
            }
            if !sectors.insert((assignment.sides, assignment.sector)) {
                return Err(format!(
                    "radial sector {} for a {}-sided cell is assigned more than once",
                    assignment.sector, assignment.sides
                ));
            }
            if !group_ids.insert((assignment.sides, assignment.face_group_id)) {
                return Err(format!(
                    "face group {} is assigned to multiple radial sectors for {}-sided cells",
                    assignment.face_group_id, assignment.sides
                ));
            }
        }
        if self.mode == VariantMode::Radial {
            for sides in [5, 6] {
                if (0..sides).any(|sector| !sectors.contains(&(sides, sector))) {
                    return Err(format!(
                        "radial variants require a face group for every sector of {sides}-sided cells"
                    ));
                }
            }
        }

        let mut mirror_keys = HashSet::new();
        for pair in &self.mirror_pairs {
            if !valid_state_key(&pair.source_key) || !valid_state_key(&pair.reflected_key) {
                return Err("mirror pairs must use valid state keys".to_string());
            }
            if pair.source_key == pair.reflected_key
                || !mirror_keys.insert(pair.reflected_key.as_str())
            {
                return Err(format!(
                    "invalid or duplicate reflected state key '{}'",
                    pair.reflected_key
                ));
            }
        }

        let mut names = HashSet::new();
        let mut condition_keys = HashSet::new();
        let mut custom_state_keys = HashSet::new();
        for pattern in &self.custom_patterns {
            if pattern.name.trim().is_empty() || !valid_state_key(&pattern.condition_key) {
                return Err("custom patterns require a name and valid condition key".to_string());
            }
            if !valid_state_key(&pattern.state_key) {
                return Err(format!(
                    "custom pattern '{}' has invalid state key '{}'",
                    pattern.name, pattern.state_key
                ));
            }
            if !names.insert(pattern.name.trim().to_lowercase())
                || !condition_keys.insert(pattern.condition_key.as_str())
                || !custom_state_keys.insert(pattern.state_key.as_str())
            {
                return Err(format!(
                    "custom pattern '{}' duplicates a name, condition, or state key",
                    pattern.name
                ));
            }
        }
        Ok(())
    }
}

fn valid_state_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

pub const OBJECT_DEFINITIONS: &[ObjectDefinition] = &[
    building(ObjectId::PlanetBase, "Base", "base.glb"),
    building(ObjectId::PlanetBuilder, "Builder", "builder.glb"),
    building(ObjectId::PlanetHousing, "Housing", "housing.glb"),
    building(ObjectId::PlanetFactory, "Factory", "factory.glb"),
    building(ObjectId::PlanetMine, "Mine", "mine.glb"),
    building(ObjectId::PlanetWarehouse, "Warehouse", "warehouse.glb"),
    building(ObjectId::PlanetRefuel, "Refuel", "refuel.glb"),
    building(ObjectId::PlanetLogistics, "Logistics", "logistics.glb"),
    ObjectDefinition {
        id: ObjectId::PlanetUnit,
        category: ObjectCategory::Units,
        label: "Planet unit",
        path: "assets/objects/units/planet_unit.glb",
        states_path: None,
        placement_profile: PlacementProfile::CellCenter,
    },
    machine(
        MachineBlockType::ConveyorBelt,
        ObjectId::ConveyorBelt,
        "Conveyor belt",
        "conveyor_belt.glb",
    ),
    machine(
        MachineBlockType::MechanicArm,
        ObjectId::MechanicArm,
        "Mechanic arm",
        "mechanic_arm.glb",
    ),
    machine(
        MachineBlockType::Chest,
        ObjectId::Chest,
        "Chest",
        "chest.glb",
    ),
    machine(
        MachineBlockType::Assembler,
        ObjectId::Assembler,
        "Assembler",
        "assembler.glb",
    ),
    machine(
        MachineBlockType::Furnace,
        ObjectId::Furnace,
        "Furnace",
        "furnace.glb",
    ),
    machine(
        MachineBlockType::Pallet,
        ObjectId::Pallet,
        "Pallet",
        "pallet.glb",
    ),
    machine(
        MachineBlockType::Crate,
        ObjectId::Crate,
        "Crate",
        "crate.glb",
    ),
];

const fn building(id: ObjectId, label: &'static str, file_name: &'static str) -> ObjectDefinition {
    ObjectDefinition {
        id,
        category: ObjectCategory::Buildings,
        label,
        states_path: None,
        path: match id {
            ObjectId::PlanetBase => "assets/objects/buildings/base.glb",
            ObjectId::PlanetBuilder => "assets/objects/buildings/builder.glb",
            ObjectId::PlanetHousing => "assets/objects/buildings/housing.glb",
            ObjectId::PlanetFactory => "assets/objects/buildings/factory.glb",
            ObjectId::PlanetMine => "assets/objects/buildings/mine.glb",
            ObjectId::PlanetWarehouse => "assets/objects/buildings/warehouse.glb",
            ObjectId::PlanetRefuel => "assets/objects/buildings/refuel.glb",
            ObjectId::PlanetLogistics => "assets/objects/buildings/logistics.glb",
            _ => file_name,
        },
        placement_profile: PlacementProfile::TriangleSlots,
    }
}

const fn machine(
    _kind: MachineBlockType,
    id: ObjectId,
    label: &'static str,
    file_name: &'static str,
) -> ObjectDefinition {
    let _ = file_name;
    ObjectDefinition {
        id,
        category: ObjectCategory::MachineBlocks,
        label,
        states_path: if matches!(id, ObjectId::ConveyorBelt) {
            Some("assets/objects/machine_blocks/conveyor_belt/states")
        } else {
            None
        },
        path: match id {
            ObjectId::ConveyorBelt => "assets/objects/machine_blocks/conveyor_belt.glb",
            ObjectId::MechanicArm => "assets/objects/machine_blocks/mechanic_arm.glb",
            ObjectId::Chest => "assets/objects/machine_blocks/chest.glb",
            ObjectId::Assembler => "assets/objects/machine_blocks/assembler.glb",
            ObjectId::Furnace => "assets/objects/machine_blocks/furnace.glb",
            ObjectId::Pallet => "assets/objects/machine_blocks/pallet.glb",
            ObjectId::Crate => "assets/objects/machine_blocks/crate.glb",
            _ => file_name,
        },
        placement_profile: PlacementProfile::CellCenter,
    }
}

pub fn machine_object_id(kind: MachineBlockType) -> ObjectId {
    match kind {
        MachineBlockType::ConveyorBelt => ObjectId::ConveyorBelt,
        MachineBlockType::MechanicArm => ObjectId::MechanicArm,
        MachineBlockType::Chest => ObjectId::Chest,
        MachineBlockType::Assembler => ObjectId::Assembler,
        MachineBlockType::Furnace => ObjectId::Furnace,
        MachineBlockType::Pallet => ObjectId::Pallet,
        MachineBlockType::Crate => ObjectId::Crate,
    }
}

#[derive(Clone, Debug)]
pub struct EditableMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MeshPrimitive {
    Cube,
    Polygon,
    Sphere,
    Cylinder,
}

impl EditableMesh {
    pub fn paint_faces(&mut self, faces: &[usize], color: Color) -> Result<(), String> {
        let selected_faces = faces.iter().copied().collect::<HashSet<_>>();
        if selected_faces.is_empty() {
            return Err("Select at least one face to paint".to_string());
        }
        let face_count = self.face_count();
        if let Some(face) = selected_faces.iter().find(|&&face| face >= face_count) {
            return Err(format!("Paint face {face} is out of range"));
        }
        let mut selected_vertices = HashSet::new();
        let mut unselected_vertices = HashSet::new();
        for (face, triangle) in self.indices.chunks_exact(3).enumerate() {
            for &index in triangle {
                if index as usize >= self.vertices.len() {
                    return Err(format!("Mesh index {index} is out of range"));
                }
                if selected_faces.contains(&face) {
                    selected_vertices.insert(index as usize);
                } else {
                    unselected_vertices.insert(index as usize);
                }
            }
        }
        let shared_vertices = selected_vertices
            .intersection(&unselected_vertices)
            .copied()
            .collect::<Vec<_>>();
        let mut duplicated = HashMap::with_capacity(shared_vertices.len());
        for index in shared_vertices {
            let duplicate = u32::try_from(self.vertices.len())
                .map_err(|_| "Mesh has too many vertices to paint".to_string())?;
            self.vertices.push(self.vertices[index]);
            duplicated.insert(index as u32, duplicate);
        }
        for face in selected_faces {
            let start = face * 3;
            for index in &mut self.indices[start..start + 3] {
                if let Some(&duplicate) = duplicated.get(index) {
                    *index = duplicate;
                }
            }
        }
        let mut paint_indices = selected_vertices
            .into_iter()
            .map(|index| {
                duplicated
                    .get(&(index as u32))
                    .copied()
                    .unwrap_or(index as u32) as usize
            })
            .collect::<Vec<_>>();
        paint_indices.sort_unstable();
        self.paint_vertices(&paint_indices, color)
    }

    fn paint_vertices(&mut self, indices: &[usize], color: Color) -> Result<(), String> {
        let rgba = [
            (color.r.clamp(0.0, 1.0) * 255.0).round() as u8,
            (color.g.clamp(0.0, 1.0) * 255.0).round() as u8,
            (color.b.clamp(0.0, 1.0) * 255.0).round() as u8,
            (color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
        ];
        for &index in indices {
            let vertex = self
                .vertices
                .get_mut(index)
                .ok_or_else(|| format!("Paint vertex {index} is out of range"))?;
            vertex.color = rgba;
        }
        Ok(())
    }

    pub fn primitive(primitive: MeshPrimitive) -> Self {
        let color = Color::from_rgba(205, 190, 160, 255);
        match primitive {
            MeshPrimitive::Cube => {
                let vertices = [
                    vec3(-0.5, -0.5, -0.5),
                    vec3(0.5, -0.5, -0.5),
                    vec3(0.5, 0.5, -0.5),
                    vec3(-0.5, 0.5, -0.5),
                    vec3(-0.5, -0.5, 0.5),
                    vec3(0.5, -0.5, 0.5),
                    vec3(0.5, 0.5, 0.5),
                    vec3(-0.5, 0.5, 0.5),
                ];
                Self::from_positions_and_indices(
                    &vertices,
                    &[
                        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0,
                        4, 7, 0, 7, 3, 1, 2, 6, 1, 6, 5,
                    ],
                    color,
                )
            }
            MeshPrimitive::Polygon => {
                Self::regular_polygon(6).expect("the default polygon side count is valid")
            }
            MeshPrimitive::Sphere => Self::sphere_primitive(color),
            MeshPrimitive::Cylinder => Self::cylinder_primitive(color),
        }
    }

    pub fn regular_polygon(sides: usize) -> Result<Self, String> {
        if !(3..=16).contains(&sides) {
            return Err("Polygon sides must be between 3 and 16".to_string());
        }
        let mut positions = Vec::with_capacity(sides + 1);
        positions.push(Vec3::ZERO);
        for side in 0..sides {
            let angle =
                std::f32::consts::TAU * side as f32 / sides as f32 - std::f32::consts::FRAC_PI_2;
            positions.push(vec3(angle.cos() * 0.5, 0.0, angle.sin() * 0.5));
        }
        let mut indices = Vec::with_capacity(sides * 3);
        for side in 0..sides {
            let current = side as u32 + 1;
            let next = ((side + 1) % sides) as u32 + 1;
            indices.extend_from_slice(&[0, next, current]);
        }
        Ok(Self::from_positions_and_indices(
            &positions,
            &indices,
            Color::from_rgba(205, 190, 160, 255),
        ))
    }

    fn from_positions_and_indices(positions: &[Vec3], indices: &[u32], color: Color) -> Self {
        Self {
            vertices: positions
                .iter()
                .map(|&position| Vertex::new2(position, Vec2::ZERO, color))
                .collect(),
            indices: indices.to_vec(),
        }
    }

    pub fn render(&self) -> Result<(), String> {
        if self.vertices.len() > u16::MAX as usize + 1 {
            return Err(format!(
                "mesh has {} vertices; Macroquad supports at most {} per draw",
                self.vertices.len(),
                u16::MAX as usize + 1
            ));
        }
        if let Some(index) = self
            .indices
            .iter()
            .find(|&&index| index as usize >= self.vertices.len())
        {
            return Err(format!("mesh index {index} is out of range"));
        }
        let indices = self
            .indices
            .iter()
            .map(|&index| {
                u16::try_from(index).map_err(|_| format!("mesh index {index} exceeds u16"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mesh = Mesh {
            vertices: self.vertices.clone(),
            indices,
            texture: None,
        };
        draw_mesh(&mesh);
        Ok(())
    }

    fn sphere_primitive(color: Color) -> EditableMesh {
        const SEGMENTS: usize = 16;
        const RINGS: usize = 8;
        let mut positions = Vec::with_capacity(2 + (RINGS - 1) * SEGMENTS);
        positions.push(vec3(0.0, 0.5, 0.0));
        for ring in 1..RINGS {
            let latitude =
                std::f32::consts::FRAC_PI_2 - std::f32::consts::PI * ring as f32 / RINGS as f32;
            let ring_radius = latitude.cos() * 0.5;
            let y = latitude.sin() * 0.5;
            for segment in 0..SEGMENTS {
                let longitude = std::f32::consts::TAU * segment as f32 / SEGMENTS as f32;
                positions.push(vec3(
                    ring_radius * longitude.cos(),
                    y,
                    ring_radius * longitude.sin(),
                ));
            }
        }
        let bottom = positions.len() as u32;
        positions.push(vec3(0.0, -0.5, 0.0));
        let mut indices = Vec::new();
        for segment in 0..SEGMENTS {
            let next = (segment + 1) % SEGMENTS;
            indices.extend_from_slice(&[0, 1 + segment as u32, 1 + next as u32]);
        }
        for ring in 0..RINGS - 2 {
            let upper = 1 + ring * SEGMENTS;
            let lower = upper + SEGMENTS;
            for segment in 0..SEGMENTS {
                let next = (segment + 1) % SEGMENTS;
                let a = (upper + segment) as u32;
                let b = (upper + next) as u32;
                let c = (lower + next) as u32;
                let d = (lower + segment) as u32;
                indices.extend_from_slice(&[a, d, b, b, d, c]);
            }
        }
        let last_ring = 1 + (RINGS - 2) * SEGMENTS;
        for segment in 0..SEGMENTS {
            let next = (segment + 1) % SEGMENTS;
            indices.extend_from_slice(&[
                bottom,
                (last_ring + next) as u32,
                (last_ring + segment) as u32,
            ]);
        }
        EditableMesh::from_positions_and_indices(&positions, &indices, color)
    }

    fn cylinder_primitive(color: Color) -> EditableMesh {
        const SEGMENTS: usize = 16;
        let mut positions = Vec::with_capacity(SEGMENTS * 2 + 2);
        for y in [-0.5, 0.5] {
            for segment in 0..SEGMENTS {
                let angle = std::f32::consts::TAU * segment as f32 / SEGMENTS as f32;
                positions.push(vec3(angle.cos() * 0.5, y, angle.sin() * 0.5));
            }
        }
        let bottom_center = positions.len() as u32;
        positions.push(vec3(0.0, -0.5, 0.0));
        let top_center = positions.len() as u32;
        positions.push(vec3(0.0, 0.5, 0.0));
        let mut indices = Vec::with_capacity(SEGMENTS * 12);
        for segment in 0..SEGMENTS {
            let next = (segment + 1) % SEGMENTS;
            let bottom = segment as u32;
            let bottom_next = next as u32;
            let top = (SEGMENTS + segment) as u32;
            let top_next = (SEGMENTS + next) as u32;
            indices.extend_from_slice(&[bottom, bottom_next, top_next, bottom, top_next, top]);
            indices.extend_from_slice(&[bottom_center, bottom_next, bottom]);
            indices.extend_from_slice(&[top_center, top, top_next]);
        }
        EditableMesh::from_positions_and_indices(&positions, &indices, color)
    }

    pub fn transformed(&self, mut transform: impl FnMut(Vec3) -> Vec3) -> Self {
        let mut mesh = self.clone();
        for vertex in &mut mesh.vertices {
            vertex.position = transform(vertex.position);
        }
        mesh
    }

    pub fn face_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn extract_faces(&self, face_indices: &[usize]) -> Result<Self, String> {
        if self.indices.len() % 3 != 0 {
            return Err("mesh index count is not divisible by three".to_string());
        }
        let mut selected = vec![false; self.face_count()];
        for &face in face_indices {
            let Some(slot) = selected.get_mut(face) else {
                return Err(format!("face {face} is out of range"));
            };
            *slot = true;
        }
        if !selected.iter().any(|&value| value) {
            return Err("select at least one face to extract".to_string());
        }
        let mut vertex_remap = vec![None; self.vertices.len()];
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for (face, include) in selected.into_iter().enumerate() {
            if !include {
                continue;
            }
            for &old_index in &self.indices[face * 3..face * 3 + 3] {
                let old_index = old_index as usize;
                let vertex = self
                    .vertices
                    .get(old_index)
                    .ok_or_else(|| format!("mesh index {old_index} is out of range"))?;
                let new_index = match vertex_remap[old_index] {
                    Some(index) => index,
                    None => {
                        let index = u32::try_from(vertices.len())
                            .map_err(|_| "mesh has too many vertices to edit".to_string())?;
                        vertices.push(*vertex);
                        vertex_remap[old_index] = Some(index);
                        index
                    }
                };
                indices.push(new_index);
            }
        }
        Ok(Self { vertices, indices })
    }

    pub fn append(&mut self, other: &Self) -> Result<(), String> {
        let offset = u32::try_from(self.vertices.len())
            .map_err(|_| "mesh has too many vertices to edit".to_string())?;
        let vertices_len = self
            .vertices
            .len()
            .checked_add(other.vertices.len())
            .ok_or_else(|| "combined mesh vertex count overflow".to_string())?;
        if vertices_len > u32::MAX as usize {
            return Err("combined mesh has too many vertices to edit".to_string());
        }
        let mut indices = Vec::with_capacity(other.indices.len());
        for &index in &other.indices {
            if index as usize >= other.vertices.len() {
                return Err(format!("mesh index {index} is out of range"));
            }
            indices.push(
                index
                    .checked_add(offset)
                    .ok_or_else(|| "combined mesh index overflow".to_string())?,
            );
        }
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(indices);
        Ok(())
    }

    #[cfg(test)]
    pub fn reflected_longitudinal(&self) -> Result<Self, String> {
        if self.indices.len() % 3 != 0 {
            return Err("mesh index count is not divisible by three".to_string());
        }
        let mut mesh = self.clone();
        for vertex in &mut mesh.vertices {
            vertex.position.z = -vertex.position.z;
        }
        for triangle in mesh.indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
        Ok(mesh)
    }

    pub fn remove_faces(&mut self, removed_faces: &[usize]) -> Result<Vec<Option<usize>>, String> {
        let face_count = self.face_count();
        if self.indices.len() % 3 != 0 {
            return Err("mesh index count is not divisible by three".to_string());
        }
        let mut removed = vec![false; face_count];
        for &face in removed_faces {
            let Some(slot) = removed.get_mut(face) else {
                return Err(format!("face {face} is out of range"));
            };
            *slot = true;
        }
        let kept_count = removed.iter().filter(|&&value| !value).count();
        if kept_count == 0 {
            return Err("cannot delete every face from a model".to_string());
        }

        let mut face_remap = vec![None; face_count];
        let mut kept_indices = Vec::with_capacity(kept_count * 3);
        let mut next_face = 0;
        for (face, is_removed) in removed.iter().copied().enumerate() {
            if is_removed {
                continue;
            }
            face_remap[face] = Some(next_face);
            next_face += 1;
            kept_indices.extend_from_slice(&self.indices[face * 3..face * 3 + 3]);
        }

        let mut used_vertices = vec![false; self.vertices.len()];
        for &index in &kept_indices {
            let Some(used) = used_vertices.get_mut(index as usize) else {
                return Err(format!("mesh index {index} is out of range"));
            };
            *used = true;
        }
        let mut vertex_remap = vec![None; self.vertices.len()];
        let mut compact_vertices = Vec::new();
        for (old_index, vertex) in self.vertices.iter().copied().enumerate() {
            if used_vertices[old_index] {
                let new_index = u32::try_from(compact_vertices.len())
                    .map_err(|_| "mesh has too many vertices to edit".to_string())?;
                vertex_remap[old_index] = Some(new_index);
                compact_vertices.push(vertex);
            }
        }
        let compact_indices = kept_indices
            .into_iter()
            .map(|index| {
                vertex_remap
                    .get(index as usize)
                    .and_then(|mapped| *mapped)
                    .ok_or_else(|| format!("mesh index {index} could not be remapped"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.vertices = compact_vertices;
        self.indices = compact_indices;
        Ok(face_remap)
    }

    pub fn extrude_face(&mut self, face_index: usize, distance: f32) -> Result<(), String> {
        let start = face_index
            .checked_mul(3)
            .ok_or_else(|| "face index overflow".to_string())?;
        if start + 2 >= self.indices.len() || self.indices.len() % 3 != 0 {
            return Err(format!("face {face_index} is out of range"));
        }
        let old = [
            self.indices[start],
            self.indices[start + 1],
            self.indices[start + 2],
        ];
        let points = old.map(|index| {
            self.vertices
                .get(index as usize)
                .map(|vertex| vertex.position)
                .ok_or_else(|| format!("mesh index {index} is out of range"))
        });
        let [a, b, c] = points;
        let [a, b, c] = [a?, b?, c?];
        let normal = (b - a).cross(c - a).normalize_or_zero();
        if normal.length_squared() <= f32::EPSILON {
            return Err("cannot extrude a degenerate face".to_string());
        }
        let originals = old.map(|index| self.vertices[index as usize]);
        let first = u32::try_from(self.vertices.len())
            .map_err(|_| "mesh has too many vertices to edit".to_string())?;
        for vertex in originals {
            let mut extruded = vertex;
            extruded.position += normal * distance;
            self.vertices.push(extruded);
        }
        let top = [first, first + 1, first + 2];
        self.indices.extend_from_slice(&top);
        for edge in 0..3 {
            let next = (edge + 1) % 3;
            let a = old[edge];
            let b = old[next];
            let a_top = top[edge];
            let b_top = top[next];
            self.indices
                .extend_from_slice(&[a, b, b_top, a, b_top, a_top]);
        }
        Ok(())
    }

    pub fn copy_faces_reflected(
        &mut self,
        face_indices: &[usize],
        plane_point: Vec3,
        plane_normal: Vec3,
    ) -> Result<Vec<(usize, usize)>, String> {
        let normal = plane_normal.normalize_or_zero();
        if !normal.is_finite() || normal.length_squared() <= f32::EPSILON {
            return Err("mirror plane must have three non-collinear points".to_string());
        }
        let selected = validated_face_selection(self, face_indices)?;
        let mut candidate = self.clone();
        let mut vertex_map = HashMap::new();
        let mut copied_faces = Vec::with_capacity(selected.len());
        for face in selected {
            let start = face * 3;
            let source = [
                self.indices[start],
                self.indices[start + 1],
                self.indices[start + 2],
            ];
            let mut reflected = [0; 3];
            for (slot, &source_index) in source.iter().enumerate() {
                let target_index = if let Some(&mapped) = vertex_map.get(&source_index) {
                    mapped
                } else {
                    let vertex = *self
                        .vertices
                        .get(source_index as usize)
                        .ok_or_else(|| format!("mesh index {source_index} is out of range"))?;
                    let distance = (vertex.position - plane_point).dot(normal);
                    let position = vertex.position - normal * (2.0 * distance);
                    let mapped = u32::try_from(candidate.vertices.len())
                        .map_err(|_| "mesh has too many vertices to mirror".to_string())?;
                    candidate.vertices.push(Vertex { position, ..vertex });
                    vertex_map.insert(source_index, mapped);
                    mapped
                };
                reflected[slot] = target_index;
            }
            let new_face = candidate.face_count();
            candidate
                .indices
                .extend_from_slice(&[reflected[0], reflected[2], reflected[1]]);
            copied_faces.push((face, new_face));
        }
        *self = candidate;
        Ok(copied_faces)
    }

    pub fn copy_faces_offset(
        &mut self,
        face_indices: &[usize],
        distance: f32,
    ) -> Result<Vec<(usize, usize)>, String> {
        if !distance.is_finite() || distance.abs() <= 1e-6 {
            return Err("offset distance must be finite and non-zero".to_string());
        }
        let selected = validated_face_selection(self, face_indices)?;
        let mut vertex_normals = HashMap::<u32, Vec3>::new();
        for &face in &selected {
            let start = face * 3;
            let indices = [
                self.indices[start],
                self.indices[start + 1],
                self.indices[start + 2],
            ];
            let [a, b, c] = indices.map(|index| {
                self.vertices
                    .get(index as usize)
                    .map(|vertex| vertex.position)
            });
            let [Some(a), Some(b), Some(c)] = [a, b, c] else {
                return Err("mesh contains an out-of-range vertex index".to_string());
            };
            let normal = (b - a).cross(c - a).normalize_or_zero();
            if normal.length_squared() <= f32::EPSILON {
                return Err(format!("cannot offset degenerate face {face}"));
            }
            for index in indices {
                *vertex_normals.entry(index).or_insert(Vec3::ZERO) += normal;
            }
        }
        let normals = vertex_normals
            .into_iter()
            .map(|(index, normal)| (index, normal.normalize_or_zero()))
            .collect::<HashMap<_, _>>();
        if normals
            .values()
            .any(|normal| normal.length_squared() <= f32::EPSILON)
        {
            return Err("selected faces have an ambiguous vertex normal".to_string());
        }

        let mut candidate = self.clone();
        let mut vertex_map = HashMap::new();
        let mut copied_faces = Vec::with_capacity(selected.len());
        for face in selected {
            let start = face * 3;
            let source = [
                self.indices[start],
                self.indices[start + 1],
                self.indices[start + 2],
            ];
            let mut offset = [0; 3];
            for (slot, &source_index) in source.iter().enumerate() {
                let target_index = if let Some(&mapped) = vertex_map.get(&source_index) {
                    mapped
                } else {
                    let vertex = *self
                        .vertices
                        .get(source_index as usize)
                        .ok_or_else(|| format!("mesh index {source_index} is out of range"))?;
                    let normal = *normals
                        .get(&source_index)
                        .ok_or_else(|| "selected vertex normal is unavailable".to_string())?;
                    let mapped = u32::try_from(candidate.vertices.len())
                        .map_err(|_| "mesh has too many vertices to offset".to_string())?;
                    candidate.vertices.push(Vertex {
                        position: vertex.position + normal * distance,
                        ..vertex
                    });
                    vertex_map.insert(source_index, mapped);
                    mapped
                };
                offset[slot] = target_index;
            }
            let new_face = candidate.face_count();
            candidate.indices.extend_from_slice(&offset);
            copied_faces.push((face, new_face));
        }
        *self = candidate;
        Ok(copied_faces)
    }

    pub fn split_faces_by_intersections(
        &mut self,
        target_faces: &[usize],
        cutter_faces: &[usize],
        multiline: Option<(usize, &[Vec3])>,
    ) -> Result<Vec<Vec<usize>>, String> {
        if self.indices.len() % 3 != 0 {
            return Err("mesh index count is not divisible by three".to_string());
        }
        let target_faces = validated_face_selection(self, target_faces)?;
        let cutter_faces = if cutter_faces.is_empty() {
            Vec::new()
        } else {
            validated_face_selection(self, cutter_faces)?
        };
        if target_faces.is_empty() {
            return Err("select at least one target face".to_string());
        }
        if cutter_faces.is_empty() && multiline.is_none() {
            return Err("select cutter faces or draw a multiline path".to_string());
        }
        if let Some((path_face, points)) = multiline
            && (!target_faces.contains(&path_face) || points.len() < 2)
        {
            return Err("the multiline path must cross a selected target face".to_string());
        }

        let mut candidate = self.clone();
        candidate.indices.clear();
        let mut face_map = vec![Vec::new(); self.face_count()];
        let mut generated_vertices = HashMap::<ClippedVertexKey, u32>::new();
        let mut split_count = 0;
        let target_set = target_faces.iter().copied().collect::<HashSet<_>>();
        for (face, mapped_faces) in face_map.iter_mut().enumerate() {
            if !target_set.contains(&face) {
                let start = face * 3;
                let new_face = candidate.face_count();
                candidate
                    .indices
                    .extend_from_slice(&self.indices[start..start + 3]);
                mapped_faces.push(new_face);
                continue;
            }

            let source = triangle_vertices(self, face)?;
            let [a, b, c] = source.map(|vertex| vertex.position);
            let normal = (b - a).cross(c - a).normalize_or_zero();
            let u_axis = (b - a).normalize_or_zero();
            let v_axis = normal.cross(u_axis).normalize_or_zero();
            if normal.length_squared() <= f32::EPSILON
                || u_axis.length_squared() <= f32::EPSILON
                || v_axis.length_squared() <= f32::EPSILON
            {
                return Err(format!("target face {face} is degenerate"));
            }
            let mut polygon = Vec::with_capacity(3);
            let start = face * 3;
            for (slot, vertex) in source.into_iter().enumerate() {
                polygon.push(ClippedVertex {
                    vertex,
                    point: project_to_plane_2d(vertex.position, a, u_axis, v_axis),
                    original_index: Some(self.indices[start + slot]),
                });
            }

            let mut lines = Vec::<(Vec2, Vec2)>::new();
            for &cutter_face in &cutter_faces {
                let cutter = triangle_vertices(self, cutter_face)?;
                let cutter_positions = cutter.map(|vertex| vertex.position);
                if let Some((start, end)) =
                    triangle_plane_intersection_segment(cutter_positions, a, normal)
                {
                    let start_2d = project_to_plane_2d(start, a, u_axis, v_axis);
                    let end_2d = project_to_plane_2d(end, a, u_axis, v_axis);
                    if segment_intersects_polygon_2d(start_2d, end_2d, &polygon) {
                        push_unique_cut_line(&mut lines, start_2d, end_2d - start_2d);
                    }
                }
            }

            let mut polygons = if let Some((path_face, points)) = multiline
                && path_face == face
            {
                split_triangle_by_path(&polygon, points, source, a, normal, u_axis, v_axis)?
            } else {
                vec![polygon]
            };
            if polygons.len() > 1 {
                split_count += polygons.len() - 1;
            }
            for (line_start, line_direction) in lines {
                let mut divided = Vec::new();
                for polygon in polygons {
                    let has_positive = polygon
                        .iter()
                        .any(|vertex| line_side(line_start, line_direction, vertex.point) > 1e-5);
                    let has_negative = polygon
                        .iter()
                        .any(|vertex| line_side(line_start, line_direction, vertex.point) < -1e-5);
                    if has_positive && has_negative {
                        split_count += 1;
                        let positive =
                            clip_polygon_by_line(&polygon, line_start, line_direction, 1.0);
                        let negative =
                            clip_polygon_by_line(&polygon, line_start, line_direction, -1.0);
                        if polygon_area(&positive).abs() > 1e-10 {
                            divided.push(positive);
                        }
                        if polygon_area(&negative).abs() > 1e-10 {
                            divided.push(negative);
                        }
                    } else {
                        divided.push(polygon);
                    }
                }
                polygons = divided;
            }

            for polygon in polygons {
                for triangle in triangulate_polygon(&polygon)? {
                    let points = triangle.map(|index| polygon[index]);
                    let new_face = candidate.face_count();
                    for point in &points {
                        let index = if let Some(index) = point.original_index {
                            index
                        } else {
                            let key = ClippedVertexKey::from_vertex(point.vertex);
                            if let Some(&index) = generated_vertices.get(&key) {
                                index
                            } else {
                                let index = u32::try_from(candidate.vertices.len())
                                    .map_err(|_| "trim creates too many vertices".to_string())?;
                                candidate.vertices.push(point.vertex);
                                generated_vertices.insert(key, index);
                                index
                            }
                        };
                        candidate.indices.push(index);
                    }
                    mapped_faces.push(new_face);
                }
            }
        }
        if split_count == 0 {
            return Err("the cutter geometry does not split any selected target face".to_string());
        }
        *self = candidate;
        Ok(face_map)
    }

    pub fn fillet_edges(
        &mut self,
        edges: &[(usize, usize)],
        radius: f32,
        segments: usize,
    ) -> Result<Vec<Vec<usize>>, String> {
        if !radius.is_finite() || radius <= 1e-5 {
            return Err("fillet radius must be positive and finite".to_string());
        }
        if !(2..=12).contains(&segments) {
            return Err("fillet requires between 2 and 12 segments".to_string());
        }
        if self.indices.len() % 3 != 0 {
            return Err("mesh index count is not divisible by three".to_string());
        }
        if edges.is_empty() {
            return Err("select at least one mesh edge to fillet".to_string());
        }
        let mut edge_faces = HashMap::<(u32, u32), Vec<usize>>::new();
        for face in 0..self.face_count() {
            let start = face * 3;
            let triangle = &self.indices[start..start + 3];
            for side in 0..3 {
                let a = triangle[side];
                let b = triangle[(side + 1) % 3];
                edge_faces
                    .entry(if a < b { (a, b) } else { (b, a) })
                    .or_default()
                    .push(face);
            }
        }
        let mut selected_edges = edges
            .iter()
            .map(|&(a, b)| {
                let a =
                    u32::try_from(a).map_err(|_| "selected edge index exceeds u32".to_string())?;
                let b =
                    u32::try_from(b).map_err(|_| "selected edge index exceeds u32".to_string())?;
                if a == b || a as usize >= self.vertices.len() || b as usize >= self.vertices.len()
                {
                    return Err("selected edge has invalid vertices".to_string());
                }
                Ok(if a < b { (a, b) } else { (b, a) })
            })
            .collect::<Result<Vec<_>, _>>()?;
        selected_edges.sort_unstable();
        selected_edges.dedup();
        if selected_edges.len() != edges.len() {
            return Err("selected edges contain duplicates".to_string());
        }

        #[derive(Clone)]
        struct FilletEdge {
            face_a: usize,
            face_b: usize,
            endpoints: [u32; 2],
            tangents: [[Vertex; 2]; 2],
            arcs: Vec<[Vertex; 2]>,
            centers: [Vec3; 2],
        }

        let mut fillets = Vec::with_capacity(selected_edges.len());
        let mut replaced_faces = HashSet::new();
        let mut used_vertices = HashSet::new();
        for edge in selected_edges {
            let adjacent = edge_faces
                .get(&edge)
                .ok_or_else(|| "selected edge is not part of the mesh".to_string())?;
            if adjacent.len() != 2 {
                return Err("fillet supports only edges shared by exactly two faces".to_string());
            }
            let face_a = adjacent[0].min(adjacent[1]);
            let face_b = adjacent[0].max(adjacent[1]);
            if !replaced_faces.insert(face_a) || !replaced_faces.insert(face_b) {
                return Err("fillet selected edges must not share faces".to_string());
            }
            if !used_vertices.insert(edge.0) || !used_vertices.insert(edge.1) {
                return Err("fillet selected edges must not share vertices".to_string());
            }
            let a = self.vertices[edge.0 as usize].position;
            let b = self.vertices[edge.1 as usize].position;
            let edge_direction = (b - a).normalize_or_zero();
            if edge_direction.length_squared() <= f32::EPSILON {
                return Err("cannot fillet a zero-length edge".to_string());
            }
            let face_vertices = |face: usize| -> Result<[u32; 3], String> {
                let start = face * 3;
                let indices = [
                    self.indices[start],
                    self.indices[start + 1],
                    self.indices[start + 2],
                ];
                if !indices.contains(&edge.0) || !indices.contains(&edge.1) {
                    return Err("edge adjacency is inconsistent".to_string());
                }
                Ok(indices)
            };
            let indices_a = face_vertices(face_a)?;
            let indices_b = face_vertices(face_b)?;
            let opposite_a = *indices_a
                .iter()
                .find(|&&index| index != edge.0 && index != edge.1)
                .ok_or_else(|| "fillet face is degenerate".to_string())?;
            let opposite_b = *indices_b
                .iter()
                .find(|&&index| index != edge.0 && index != edge.1)
                .ok_or_else(|| "fillet face is degenerate".to_string())?;
            let opposite_position_a = self.vertices[opposite_a as usize].position;
            let opposite_position_b = self.vertices[opposite_b as usize].position;
            let inward_a = ((opposite_position_a - a)
                - edge_direction * (opposite_position_a - a).dot(edge_direction))
            .normalize_or_zero();
            let inward_b = ((opposite_position_b - a)
                - edge_direction * (opposite_position_b - a).dot(edge_direction))
            .normalize_or_zero();
            let angle = inward_a.dot(inward_b).clamp(-1.0, 1.0).acos();
            if angle <= 1e-3 || angle >= std::f32::consts::PI - 1e-3 {
                return Err("fillet edge must join two non-coplanar faces".to_string());
            }
            let tangent_distance = radius / (angle * 0.5).tan();
            let height_a = (opposite_position_a - a).cross(edge_direction).length();
            let height_b = (opposite_position_b - a).cross(edge_direction).length();
            if tangent_distance >= height_a - 1e-5 || tangent_distance >= height_b - 1e-5 {
                return Err("fillet radius is too large for the adjacent faces".to_string());
            }
            let bisector = (inward_a + inward_b).normalize_or_zero();
            let center_offset = bisector * (radius / (angle * 0.5).sin());
            if center_offset.length_squared() <= f32::EPSILON {
                return Err("fillet radius produces an invalid arc".to_string());
            }
            let tangent_a_start = a + inward_a * tangent_distance;
            let tangent_b_start = a + inward_b * tangent_distance;
            let radial_start = tangent_a_start - (a + center_offset);
            let radial_end = tangent_b_start - (a + center_offset);
            let arc_angle = edge_direction
                .dot(radial_start.cross(radial_end))
                .atan2(radial_start.dot(radial_end));
            let triangle_attributes =
                |indices: [u32; 3]| indices.map(|index| self.vertices[index as usize]);
            let source_a = triangle_attributes(indices_a);
            let source_b = triangle_attributes(indices_b);
            let tangent_for =
                |source: [Vertex; 3], position: Vec3| interpolate_triangle_vertex(source, position);
            let tangents = [
                [
                    tangent_for(source_a, tangent_a_start)?,
                    tangent_for(source_a, b + inward_a * tangent_distance)?,
                ],
                [
                    tangent_for(source_b, tangent_b_start)?,
                    tangent_for(source_b, b + inward_b * tangent_distance)?,
                ],
            ];
            let mut arcs = Vec::with_capacity(segments + 1);
            for segment in 0..=segments {
                let amount = segment as f32 / segments as f32;
                let rotation = Quat::from_axis_angle(edge_direction, arc_angle * amount);
                let radial = rotation.mul_vec3(radial_start);
                let position_a = a + center_offset + radial;
                let position_b = b + center_offset + radial;
                let second = if segment == 0 {
                    tangents[0][1]
                } else if segment == segments {
                    tangents[1][1]
                } else {
                    interpolate_mesh_vertex(tangents[0][1], tangents[1][1], position_b, amount)
                };
                let first = if segment == 0 {
                    tangents[0][0]
                } else if segment == segments {
                    tangents[1][0]
                } else {
                    interpolate_mesh_vertex(tangents[0][0], tangents[1][0], position_a, amount)
                };
                arcs.push([
                    Vertex {
                        position: position_a,
                        ..first
                    },
                    Vertex {
                        position: position_b,
                        ..second
                    },
                ]);
            }
            fillets.push(FilletEdge {
                face_a,
                face_b,
                endpoints: [edge.0, edge.1],
                tangents,
                arcs,
                centers: [a + center_offset, b + center_offset],
            });
        }

        let mut candidate = self.clone();
        let mut replacements = HashMap::<(usize, u32), Vertex>::new();
        for fillet in &fillets {
            for (side, face) in [fillet.face_a, fillet.face_b].into_iter().enumerate() {
                replacements.insert((face, fillet.endpoints[0]), fillet.tangents[side][0]);
                replacements.insert((face, fillet.endpoints[1]), fillet.tangents[side][1]);
            }
        }
        let mut replacement_indices = HashMap::<(usize, u32), u32>::new();
        let mut face_map = vec![Vec::new(); self.face_count()];
        let mut indices = Vec::with_capacity(self.indices.len() + fillets.len() * segments * 6);
        for (face, mapped_faces) in face_map.iter_mut().enumerate() {
            let start = face * 3;
            let old = &self.indices[start..start + 3];
            let new_face = indices.len() / 3;
            for &index in old {
                if let Some(&vertex) = replacements.get(&(face, index)) {
                    let new_index = u32::try_from(candidate.vertices.len())
                        .map_err(|_| "fillet creates too many vertices".to_string())?;
                    candidate.vertices.push(vertex);
                    replacement_indices.insert((face, index), new_index);
                    indices.push(new_index);
                } else {
                    indices.push(index);
                }
            }
            mapped_faces.push(new_face);
        }
        candidate.indices = indices;
        for fillet in &fillets {
            let first = *replacement_indices
                .get(&(fillet.face_a, fillet.endpoints[0]))
                .ok_or_else(|| "fillet tangent vertex is unavailable".to_string())?;
            let second = *replacement_indices
                .get(&(fillet.face_a, fillet.endpoints[1]))
                .ok_or_else(|| "fillet tangent vertex is unavailable".to_string())?;
            let last_first = *replacement_indices
                .get(&(fillet.face_b, fillet.endpoints[0]))
                .ok_or_else(|| "fillet tangent vertex is unavailable".to_string())?;
            let last_second = *replacement_indices
                .get(&(fillet.face_b, fillet.endpoints[1]))
                .ok_or_else(|| "fillet tangent vertex is unavailable".to_string())?;
            let mut arc_indices = Vec::with_capacity(fillet.arcs.len());
            for (step, arc) in fillet.arcs.iter().enumerate() {
                let row = if step == 0 {
                    [first, second]
                } else if step + 1 == fillet.arcs.len() {
                    [last_first, last_second]
                } else {
                    let first_index = u32::try_from(candidate.vertices.len())
                        .map_err(|_| "fillet creates too many vertices".to_string())?;
                    candidate.vertices.push(arc[0]);
                    let second_index = u32::try_from(candidate.vertices.len())
                        .map_err(|_| "fillet creates too many vertices".to_string())?;
                    candidate.vertices.push(arc[1]);
                    [first_index, second_index]
                };
                arc_indices.push(row);
            }
            for step in 0..segments {
                let [a0, b0] = arc_indices[step];
                let [a1, b1] = arc_indices[step + 1];
                let p00 = candidate.vertices[a0 as usize].position;
                let p11 = candidate.vertices[b1 as usize].position;
                let expected =
                    ((p00 - fillet.centers[0]) + (p11 - fillet.centers[1])).normalize_or_zero();
                let mut triangles = [[a0, b0, b1], [a0, b1, a1]];
                for triangle in &mut triangles {
                    let [a, b, c] =
                        triangle.map(|index| candidate.vertices[index as usize].position);
                    if (b - a).cross(c - a).dot(expected) < 0.0 {
                        triangle.swap(1, 2);
                    }
                    let face = candidate.face_count();
                    candidate.indices.extend_from_slice(triangle);
                    face_map[fillet.face_a].push(face);
                }
            }
        }
        *self = candidate;
        Ok(face_map)
    }
}

fn validated_face_selection(
    mesh: &EditableMesh,
    face_indices: &[usize],
) -> Result<Vec<usize>, String> {
    if mesh.indices.len() % 3 != 0 {
        return Err("mesh index count is not divisible by three".to_string());
    }
    if face_indices.is_empty() {
        return Err("select at least one face".to_string());
    }
    let mut selected = face_indices.to_vec();
    selected.sort_unstable();
    selected.dedup();
    for &face in &selected {
        let start = face
            .checked_mul(3)
            .ok_or_else(|| "face index overflow".to_string())?;
        if start + 2 >= mesh.indices.len() {
            return Err(format!("face {face} is out of range"));
        }
        for &vertex in &mesh.indices[start..start + 3] {
            if vertex as usize >= mesh.vertices.len() {
                return Err(format!("mesh index {vertex} is out of range"));
            }
        }
    }
    Ok(selected)
}

#[derive(Clone, Copy)]
struct ClippedVertex {
    vertex: Vertex,
    point: Vec2,
    original_index: Option<u32>,
}

fn triangle_vertices(mesh: &EditableMesh, face: usize) -> Result<[Vertex; 3], String> {
    let start = face * 3;
    let indices = [
        *mesh
            .indices
            .get(start)
            .ok_or_else(|| format!("face {face} is out of range"))?,
        *mesh
            .indices
            .get(start + 1)
            .ok_or_else(|| format!("face {face} is out of range"))?,
        *mesh
            .indices
            .get(start + 2)
            .ok_or_else(|| format!("face {face} is out of range"))?,
    ];
    let [Some(a), Some(b), Some(c)] =
        indices.map(|index| mesh.vertices.get(index as usize).copied())
    else {
        return Err(format!("face {face} contains an out-of-range vertex"));
    };
    Ok([a, b, c])
}

fn triangle_plane_intersection_segment(
    triangle: [Vec3; 3],
    plane_point: Vec3,
    plane_normal: Vec3,
) -> Option<(Vec3, Vec3)> {
    const TOLERANCE: f32 = 1e-5;
    let distances = triangle.map(|point| (point - plane_point).dot(plane_normal));
    if distances.iter().all(|distance| distance.abs() <= TOLERANCE) {
        return None;
    }
    let mut points = Vec::new();
    for side in 0..3 {
        let next = (side + 1) % 3;
        let start_distance = distances[side];
        let end_distance = distances[next];
        if start_distance.abs() <= TOLERANCE {
            points.push(triangle[side]);
        }
        if start_distance * end_distance < 0.0 {
            let amount = start_distance / (start_distance - end_distance);
            points.push(triangle[side].lerp(triangle[next], amount));
        }
    }
    if distances[2].abs() <= TOLERANCE {
        points.push(triangle[2]);
    }
    points.dedup_by(|a, b| a.distance_squared(*b) <= TOLERANCE * TOLERANCE);
    if points.len() < 2 {
        return None;
    }
    let mut farthest = (0, 1, 0.0);
    for first in 0..points.len() {
        for second in first + 1..points.len() {
            let distance = points[first].distance_squared(points[second]);
            if distance > farthest.2 {
                farthest = (first, second, distance);
            }
        }
    }
    (farthest.2 > TOLERANCE * TOLERANCE).then_some((points[farthest.0], points[farthest.1]))
}

fn push_unique_cut_line(lines: &mut Vec<(Vec2, Vec2)>, start: Vec2, direction: Vec2) {
    let direction = direction.normalize_or_zero();
    if direction.length_squared() <= 1e-10 {
        return;
    }
    if lines.iter().any(|&(existing_start, existing_direction)| {
        existing_direction.perp_dot(direction).abs() <= 1e-4
            && line_side(existing_start, existing_direction, start).abs() <= 1e-4
    }) {
        return;
    }
    lines.push((start, direction));
}

fn segment_intersects_polygon_2d(start: Vec2, end: Vec2, polygon: &[ClippedVertex]) -> bool {
    if point_in_polygon_2d(start, polygon) || point_in_polygon_2d(end, polygon) {
        return true;
    }
    (0..polygon.len()).any(|index| {
        segments_intersect_2d(
            start,
            end,
            polygon[index].point,
            polygon[(index + 1) % polygon.len()].point,
        )
    })
}

fn point_in_polygon_2d(point: Vec2, polygon: &[ClippedVertex]) -> bool {
    let mut inside = false;
    for index in 0..polygon.len() {
        let start = polygon[index].point;
        let end = polygon[(index + 1) % polygon.len()].point;
        if point_on_segment_2d(point, start, end, 1e-5).is_some() {
            return true;
        }
        if (start.y > point.y) != (end.y > point.y)
            && point.x < (end.x - start.x) * (point.y - start.y) / (end.y - start.y) + start.x
        {
            inside = !inside;
        }
    }
    inside
}

fn point_on_segment_2d(point: Vec2, start: Vec2, end: Vec2, tolerance: f32) -> Option<f32> {
    let direction = end - start;
    let length_squared = direction.length_squared();
    if length_squared <= tolerance * tolerance {
        return (point.distance_squared(start) <= tolerance * tolerance).then_some(0.0);
    }
    let amount = (point - start).dot(direction) / length_squared;
    if !(-tolerance..=1.0 + tolerance).contains(&amount) {
        return None;
    }
    let projected = start + direction * amount.clamp(0.0, 1.0);
    (point.distance_squared(projected) <= tolerance * tolerance).then_some(amount.clamp(0.0, 1.0))
}

fn segments_intersect_2d(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let first = b - a;
    let second = d - c;
    let denominator = first.perp_dot(second);
    if denominator.abs() <= 1e-8 {
        return false;
    }
    let displacement = c - a;
    let first_amount = displacement.perp_dot(second) / denominator;
    let second_amount = displacement.perp_dot(first) / denominator;
    (-1e-5..=1.0 + 1e-5).contains(&first_amount) && (-1e-5..=1.0 + 1e-5).contains(&second_amount)
}

fn split_triangle_by_path(
    triangle: &[ClippedVertex],
    points: &[Vec3],
    source: [Vertex; 3],
    plane_origin: Vec3,
    plane_normal: Vec3,
    u_axis: Vec3,
    v_axis: Vec3,
) -> Result<Vec<Vec<ClippedVertex>>, String> {
    if points.len() < 2 {
        return Err("draw a path with at least two points".to_string());
    }
    let path = points
        .iter()
        .map(|&position| {
            if (position - plane_origin).dot(plane_normal).abs() > 1e-4 {
                return Err("multiline points must lie on the selected target face".to_string());
            }
            let point = project_to_plane_2d(position, plane_origin, u_axis, v_axis);
            if !point_in_polygon_2d(point, triangle) {
                return Err(
                    "multiline points must stay inside the selected target face".to_string()
                );
            }
            let vertex = interpolate_triangle_vertex(source, position)?;
            Ok(ClippedVertex {
                vertex,
                point,
                original_index: None,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if path
        .windows(2)
        .any(|pair| pair[0].point.distance_squared(pair[1].point) <= 1e-10)
    {
        return Err("multiline segments must have non-zero length".to_string());
    }
    for first in 0..path.len().saturating_sub(1) {
        for second in first + 2..path.len().saturating_sub(1) {
            if segments_intersect_2d(
                path[first].point,
                path[first + 1].point,
                path[second].point,
                path[second + 1].point,
            ) {
                return Err("multiline path must not cross itself".to_string());
            }
        }
    }
    let start = path[0].point;
    let end = path[path.len() - 1].point;
    if start.distance_squared(end) <= 1e-10 {
        return Err("multiline path endpoints must be different".to_string());
    }
    if !(0..triangle.len()).any(|index| {
        point_on_segment_2d(
            start,
            triangle[index].point,
            triangle[(index + 1) % triangle.len()].point,
            1e-4,
        )
        .is_some()
    }) || !(0..triangle.len()).any(|index| {
        point_on_segment_2d(
            end,
            triangle[index].point,
            triangle[(index + 1) % triangle.len()].point,
            1e-4,
        )
        .is_some()
    }) {
        return Err("multiline path must start and end on the target face boundary".to_string());
    }

    let endpoints = [path[0].point, end];
    let mut boundary = Vec::new();
    for side in 0..triangle.len() {
        let next = (side + 1) % triangle.len();
        boundary.push(triangle[side]);
        let mut inserts = endpoints
            .iter()
            .filter_map(|&point| {
                let amount =
                    point_on_segment_2d(point, triangle[side].point, triangle[next].point, 1e-4)?;
                (amount > 1e-5 && amount < 1.0 - 1e-5).then_some((amount, point))
            })
            .collect::<Vec<_>>();
        inserts.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (amount, _) in inserts {
            boundary.push(interpolate_vertex(triangle[side], triangle[next], amount));
        }
    }
    let boundary_index = |point: Vec2| {
        boundary
            .iter()
            .position(|vertex| vertex.point.distance_squared(point) <= 1e-8)
    };
    let start_index = boundary_index(start)
        .ok_or_else(|| "could not place multiline start on target boundary".to_string())?;
    let end_index = boundary_index(end)
        .ok_or_else(|| "could not place multiline end on target boundary".to_string())?;
    let boundary_arc = |from: usize, to: usize| {
        let mut arc = Vec::new();
        let mut index = from;
        loop {
            arc.push(boundary[index]);
            if index == to {
                break;
            }
            index = (index + 1) % boundary.len();
        }
        arc
    };
    let mut first = boundary_arc(start_index, end_index);
    first.extend(path[1..path.len() - 1].iter().rev().copied());
    let mut second = boundary_arc(end_index, start_index);
    second.extend(path[1..path.len() - 1].iter().copied());
    deduplicate_polygon(&mut first);
    deduplicate_polygon(&mut second);
    if polygon_area(&first).abs() <= 1e-10 || polygon_area(&second).abs() <= 1e-10 {
        return Err("multiline path does not divide the target face".to_string());
    }
    Ok(vec![first, second])
}

fn triangulate_polygon(polygon: &[ClippedVertex]) -> Result<Vec<[usize; 3]>, String> {
    if polygon.len() < 3 {
        return Err("cannot triangulate a degenerate split region".to_string());
    }
    if polygon.len() == 3 {
        return Ok(vec![[0, 1, 2]]);
    }
    let orientation = polygon_area(polygon).signum();
    if orientation == 0.0 {
        return Err("cannot triangulate a zero-area split region".to_string());
    }
    let mut remaining = (0..polygon.len()).collect::<Vec<_>>();
    let mut triangles = Vec::with_capacity(polygon.len() - 2);
    while remaining.len() > 3 {
        let ear = (0..remaining.len()).find(|&slot| {
            let previous = remaining[(slot + remaining.len() - 1) % remaining.len()];
            let current = remaining[slot];
            let next = remaining[(slot + 1) % remaining.len()];
            let a = polygon[previous].point;
            let b = polygon[current].point;
            let c = polygon[next].point;
            if (b - a).perp_dot(c - b) * orientation <= 1e-8 {
                return false;
            }
            !remaining.iter().copied().any(|candidate| {
                candidate != previous
                    && candidate != current
                    && candidate != next
                    && point_in_triangle_2d(polygon[candidate].point, a, b, c, orientation)
            })
        });
        let Some(slot) = ear else {
            return Err("could not triangulate the split face safely".to_string());
        };
        let previous = remaining[(slot + remaining.len() - 1) % remaining.len()];
        let current = remaining[slot];
        let next = remaining[(slot + 1) % remaining.len()];
        triangles.push([previous, current, next]);
        remaining.remove(slot);
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    Ok(triangles)
}

fn point_in_triangle_2d(point: Vec2, a: Vec2, b: Vec2, c: Vec2, orientation: f32) -> bool {
    (b - a).perp_dot(point - a) * orientation >= -1e-7
        && (c - b).perp_dot(point - b) * orientation >= -1e-7
        && (a - c).perp_dot(point - c) * orientation >= -1e-7
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct ClippedVertexKey {
    position: [i64; 3],
    uv: [i64; 2],
    color: [u8; 4],
}

impl ClippedVertexKey {
    fn from_vertex(vertex: Vertex) -> Self {
        let quantize = |value: f32| (value * 1_000_000.0).round() as i64;
        Self {
            position: [
                quantize(vertex.position.x),
                quantize(vertex.position.y),
                quantize(vertex.position.z),
            ],
            uv: [quantize(vertex.uv.x), quantize(vertex.uv.y)],
            color: vertex.color,
        }
    }
}

fn project_to_plane_2d(point: Vec3, origin: Vec3, u_axis: Vec3, v_axis: Vec3) -> Vec2 {
    let relative = point - origin;
    Vec2::new(relative.dot(u_axis), relative.dot(v_axis))
}

fn line_side(start: Vec2, direction: Vec2, point: Vec2) -> f32 {
    direction.perp_dot(point - start)
}

fn interpolate_vertex(a: ClippedVertex, b: ClippedVertex, t: f32) -> ClippedVertex {
    if t <= 1e-6 {
        return a;
    }
    if t >= 1.0 - 1e-6 {
        return b;
    }
    let color: [u8; 4] = std::array::from_fn(|index| {
        (a.vertex.color[index] as f32 * (1.0 - t) + b.vertex.color[index] as f32 * t).round() as u8
    });
    let mut vertex = Vertex::new2(
        a.vertex.position.lerp(b.vertex.position, t),
        a.vertex.uv.lerp(b.vertex.uv, t),
        Color::from_rgba(color[0], color[1], color[2], color[3]),
    );
    vertex.normal = a.vertex.normal.lerp(b.vertex.normal, t);
    ClippedVertex {
        vertex,
        point: a.point.lerp(b.point, t),
        original_index: None,
    }
}

fn interpolate_mesh_vertex(a: Vertex, b: Vertex, position: Vec3, t: f32) -> Vertex {
    let color: [u8; 4] = std::array::from_fn(|channel| {
        (a.color[channel] as f32 * (1.0 - t) + b.color[channel] as f32 * t)
            .round()
            .clamp(0.0, 255.0) as u8
    });
    let mut vertex = Vertex::new2(
        position,
        a.uv.lerp(b.uv, t),
        Color::from_rgba(color[0], color[1], color[2], color[3]),
    );
    vertex.normal = a.normal.lerp(b.normal, t);
    vertex
}

fn interpolate_triangle_vertex(source: [Vertex; 3], position: Vec3) -> Result<Vertex, String> {
    let [a, b, c] = source;
    let v0 = b.position - a.position;
    let v1 = c.position - a.position;
    let v2 = position - a.position;
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);
    let denominator = d00 * d11 - d01 * d01;
    if denominator.abs() <= 1e-12 {
        return Err("cannot fillet a degenerate face".to_string());
    }
    let weight_b = (d11 * d20 - d01 * d21) / denominator;
    let weight_c = (d00 * d21 - d01 * d20) / denominator;
    let weight_a = 1.0 - weight_b - weight_c;
    let blend = |a: f32, b: f32, c: f32| a * weight_a + b * weight_b + c * weight_c;
    let color: [u8; 4] = std::array::from_fn(|channel| {
        blend(
            a.color[channel] as f32,
            b.color[channel] as f32,
            c.color[channel] as f32,
        )
        .round()
        .clamp(0.0, 255.0) as u8
    });
    let mut vertex = Vertex::new2(
        position,
        Vec2::new(blend(a.uv.x, b.uv.x, c.uv.x), blend(a.uv.y, b.uv.y, c.uv.y)),
        Color::from_rgba(color[0], color[1], color[2], color[3]),
    );
    vertex.normal = a.normal * weight_a + b.normal * weight_b + c.normal * weight_c;
    Ok(vertex)
}

fn clip_polygon_by_line(
    polygon: &[ClippedVertex],
    line_start: Vec2,
    line_direction: Vec2,
    keep_side: f32,
) -> Vec<ClippedVertex> {
    if polygon.is_empty() {
        return Vec::new();
    }
    const EPSILON: f32 = 1e-6;
    let mut output = Vec::new();
    let mut previous = *polygon.last().unwrap();
    let mut previous_distance = keep_side * line_side(line_start, line_direction, previous.point);
    for &current in polygon {
        let current_distance = keep_side * line_side(line_start, line_direction, current.point);
        let previous_inside = previous_distance >= -EPSILON;
        let current_inside = current_distance >= -EPSILON;
        if previous_inside != current_inside {
            let t = (previous_distance / (previous_distance - current_distance)).clamp(0.0, 1.0);
            output.push(interpolate_vertex(previous, current, t));
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_distance = current_distance;
    }
    deduplicate_polygon(&mut output);
    output
}

fn deduplicate_polygon(polygon: &mut Vec<ClippedVertex>) {
    let mut clean = Vec::with_capacity(polygon.len());
    for vertex in polygon.iter().copied() {
        if clean
            .last()
            .is_none_or(|last: &ClippedVertex| last.point.distance_squared(vertex.point) > 1e-12)
        {
            clean.push(vertex);
        }
    }
    if clean.len() > 1
        && clean[0]
            .point
            .distance_squared(clean[clean.len() - 1].point)
            <= 1e-12
    {
        clean.pop();
    }
    *polygon = clean;
}

fn polygon_area(polygon: &[ClippedVertex]) -> f32 {
    if polygon.len() < 3 {
        return 0.0;
    }
    polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
        .map(|(a, b)| a.point.perp_dot(b.point))
        .sum::<f32>()
        * 0.5
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FaceMetadata {
    pub face_names: Vec<Option<String>>,
    pub groups: Vec<FaceGroup>,
    #[serde(default)]
    pub radial_pivot: Option<[f32; 3]>,
    next_group_id: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaceGroup {
    pub id: u64,
    pub name: String,
    pub faces: Vec<usize>,
    #[serde(default)]
    pub parent_id: Option<u64>,
    #[serde(default)]
    pub program: FaceGroupProgram,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub hidden: bool,
}

fn extract_or_empty(mesh: &EditableMesh, faces: &[usize]) -> Result<EditableMesh, String> {
    if faces.is_empty() {
        Ok(EditableMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        })
    } else {
        mesh.extract_faces(faces)
    }
}

impl FaceMetadata {
    pub fn for_face_count(face_count: usize) -> Self {
        Self {
            face_names: vec![None; face_count],
            groups: Vec::new(),
            radial_pivot: None,
            next_group_id: 1,
        }
    }

    fn validate(&mut self, face_count: usize) -> Result<(), String> {
        if self.face_names.len() > face_count {
            return Err(format!(
                "face metadata has {} names for {face_count} faces",
                self.face_names.len()
            ));
        }
        self.face_names.resize(face_count, None);
        let mut group_ids = HashSet::new();
        let mut assigned_faces = HashSet::new();
        for group in &mut self.groups {
            if group.name.trim().is_empty() {
                return Err(format!("face group {} has an empty name", group.id));
            }
            if !group_ids.insert(group.id) {
                return Err(format!("duplicate face group id {}", group.id));
            }
            group.faces.sort_unstable();
            group.faces.dedup();
            for &face in &group.faces {
                if face >= face_count {
                    return Err(format!(
                        "face group '{}' references missing face {face}",
                        group.name
                    ));
                }
                if !assigned_faces.insert(face) {
                    return Err(format!("face {face} belongs to more than one group"));
                }
            }
        }
        let parent_ids = self
            .groups
            .iter()
            .filter_map(|group| group.parent_id)
            .collect::<HashSet<_>>();
        self.groups.retain(|group| {
            !group.faces.is_empty()
                || group.program != FaceGroupProgram::Geometry
                || parent_ids.contains(&group.id)
        });
        for group in &self.groups {
            if let Some(parent_id) = group.parent_id {
                let parent = self
                    .groups
                    .iter()
                    .find(|candidate| candidate.id == parent_id)
                    .ok_or_else(|| {
                        format!("face group '{}' has missing parent {parent_id}", group.name)
                    })?;
                match group.program {
                    FaceGroupProgram::RadialRepeat => {
                        return Err(format!(
                            "radial group '{}' cannot have a parent",
                            group.name
                        ));
                    }
                    FaceGroupProgram::IfNeighbor(_) => {
                        if parent.program != FaceGroupProgram::RadialRepeat {
                            return Err(format!(
                                "condition group '{}' must be inside a radial group",
                                group.name
                            ));
                        }
                    }
                    FaceGroupProgram::Geometry => {}
                }
            } else if let FaceGroupProgram::IfNeighbor(_) = group.program {
                return Err(format!(
                    "condition group '{}' must be inside a radial group",
                    group.name
                ));
            }
            let mut ancestors = HashSet::new();
            let mut parent_id = group.parent_id;
            while let Some(id) = parent_id {
                if id == group.id || !ancestors.insert(id) {
                    return Err(format!("face group hierarchy contains a cycle at {id}"));
                }
                parent_id = self
                    .groups
                    .iter()
                    .find(|candidate| candidate.id == id)
                    .and_then(|parent| parent.parent_id);
            }
        }
        let radial_groups = self
            .groups
            .iter()
            .filter(|group| group.program == FaceGroupProgram::RadialRepeat)
            .collect::<Vec<_>>();
        if radial_groups.len() > 1 {
            return Err("only one radial-repeat group is supported per model".to_string());
        }
        if let Some(radial_group) = radial_groups.first() {
            let condition_list = self
                .groups
                .iter()
                .filter_map(|group| match (group.parent_id, group.program) {
                    (Some(parent_id), FaceGroupProgram::IfNeighbor(condition))
                        if parent_id == radial_group.id =>
                    {
                        Some(condition)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mut conditions = HashSet::new();
            if condition_list
                .iter()
                .any(|condition| !conditions.insert(*condition))
            {
                return Err(format!(
                    "radial group '{}' has duplicate neighbor-condition children",
                    radial_group.name
                ));
            }
        }
        let minimum_next_id = self
            .groups
            .iter()
            .map(|group| group.id.saturating_add(1))
            .max()
            .unwrap_or(1);
        self.next_group_id = self.next_group_id.max(minimum_next_id);
        Ok(())
    }

    pub fn group(&self, id: u64) -> Option<&FaceGroup> {
        self.groups.iter().find(|group| group.id == id)
    }

    pub fn radial_main_group_id(&self) -> Option<u64> {
        self.groups
            .iter()
            .find(|group| group.program == FaceGroupProgram::RadialRepeat)
            .map(|group| group.id)
    }

    pub fn faces_for_group(&self, group_id: u64) -> Option<Vec<usize>> {
        let group = self.group(group_id)?;
        Some(self.descendant_faces(group_id, group.program != FaceGroupProgram::RadialRepeat))
    }

    pub fn group_is_locked(&self, group_id: u64) -> bool {
        self.group_flag_in_ancestors(group_id, true)
    }

    pub fn group_is_hidden(&self, group_id: u64) -> bool {
        self.group_flag_in_ancestors(group_id, false)
    }

    pub fn group_has_locked_ancestor(&self, group_id: u64) -> bool {
        self.group_ancestor_has_flag(group_id, true)
    }

    pub fn group_has_hidden_ancestor(&self, group_id: u64) -> bool {
        self.group_ancestor_has_flag(group_id, false)
    }

    fn group_flag_in_ancestors(&self, group_id: u64, locked: bool) -> bool {
        let mut current = Some(group_id);
        let mut visited = HashSet::new();
        while let Some(id) = current {
            if !visited.insert(id) {
                return false;
            }
            let Some(group) = self.group(id) else {
                return false;
            };
            let flag = if locked { group.locked } else { group.hidden };
            if flag {
                return true;
            }
            current = group.parent_id;
        }
        false
    }

    fn group_ancestor_has_flag(&self, group_id: u64, locked: bool) -> bool {
        let Some(group) = self.group(group_id) else {
            return false;
        };
        let mut current = group.parent_id;
        let mut visited = HashSet::new();
        while let Some(id) = current {
            if !visited.insert(id) {
                return false;
            }
            let Some(group) = self.group(id) else {
                return false;
            };
            let flag = if locked { group.locked } else { group.hidden };
            if flag {
                return true;
            }
            current = group.parent_id;
        }
        false
    }

    pub fn locked_faces(&self) -> HashSet<usize> {
        self.groups
            .iter()
            .filter(|group| group.locked)
            .flat_map(|group| self.descendant_faces(group.id, true))
            .collect()
    }

    pub fn hidden_faces(&self) -> HashSet<usize> {
        self.groups
            .iter()
            .filter(|group| group.hidden)
            .flat_map(|group| self.descendant_faces(group.id, true))
            .collect()
    }

    pub fn set_group_locked(&mut self, group_id: u64, locked: bool) -> Result<(), String> {
        let group = self
            .groups
            .iter_mut()
            .find(|group| group.id == group_id)
            .ok_or_else(|| format!("face group {group_id} does not exist"))?;
        group.locked = locked;
        Ok(())
    }

    pub fn set_group_hidden(&mut self, group_id: u64, hidden: bool) -> Result<(), String> {
        let group = self
            .groups
            .iter_mut()
            .find(|group| group.id == group_id)
            .ok_or_else(|| format!("face group {group_id} does not exist"))?;
        group.hidden = hidden;
        Ok(())
    }

    pub fn radial_sector_mesh(
        &self,
        mesh: &EditableMesh,
        condition: RadialNeighborState,
    ) -> Result<Option<EditableMesh>, String> {
        let Some(main_id) = self.radial_main_group_id() else {
            return Ok(None);
        };
        let main_faces = self.descendant_faces(main_id, false);
        let condition_id = self.groups.iter().find_map(|group| {
            (group.parent_id == Some(main_id)
                && group.program == FaceGroupProgram::IfNeighbor(condition))
            .then_some(group.id)
        });
        let mut composed = extract_or_empty(mesh, &main_faces)?;
        if let Some(condition_id) = condition_id {
            let condition_faces = self.descendant_faces(condition_id, true);
            composed.append(&extract_or_empty(mesh, &condition_faces)?)?;
        }
        Ok(Some(composed))
    }

    pub fn set_group_program(
        &mut self,
        group_id: u64,
        program: FaceGroupProgram,
    ) -> Result<(), String> {
        let group_index = self
            .groups
            .iter()
            .position(|group| group.id == group_id)
            .ok_or_else(|| format!("face group {group_id} does not exist"))?;
        if matches!(program, FaceGroupProgram::IfNeighbor(_))
            && self.groups[group_index].parent_id.is_none()
        {
            return Err("neighbor conditions must be nested inside a Radial Repeat group".into());
        }
        if program == FaceGroupProgram::RadialRepeat
            && self.groups.iter().any(|other| {
                other.id != group_id && other.program == FaceGroupProgram::RadialRepeat
            })
        {
            return Err("only one Radial Repeat group is supported per model".into());
        }
        let mut candidate = self.clone();
        candidate.groups[group_index].program = program;
        candidate.validate(candidate.face_names.len())?;
        *self = candidate;
        Ok(())
    }

    fn descendant_faces(&self, root_id: u64, include_root_condition: bool) -> Vec<usize> {
        let mut faces = Vec::new();
        let mut pending = vec![root_id];
        while let Some(group_id) = pending.pop() {
            let Some(group) = self.group(group_id) else {
                continue;
            };
            faces.extend(group.faces.iter().copied());
            for child in self
                .groups
                .iter()
                .filter(|child| child.parent_id == Some(group_id))
            {
                if !matches!(child.program, FaceGroupProgram::IfNeighbor(_))
                    || include_root_condition
                {
                    pending.push(child.id);
                }
            }
        }
        faces.sort_unstable();
        faces.dedup();
        faces
    }

    #[cfg(test)]
    pub fn create_group(&mut self, faces: &[usize]) -> Result<u64, String> {
        self.create_group_under(faces, None)
    }

    pub fn create_group_under(
        &mut self,
        faces: &[usize],
        parent_id: Option<u64>,
    ) -> Result<u64, String> {
        if faces.is_empty() {
            return Err("select one or more faces before adding a group".to_string());
        }
        if let Some(parent_id) = parent_id
            && !self.groups.iter().any(|group| group.id == parent_id)
        {
            return Err(format!("parent face group {parent_id} does not exist"));
        }
        let mut selected = faces.to_vec();
        selected.sort_unstable();
        selected.dedup();
        if let Some(&face) = selected.iter().find(|&&face| face >= self.face_names.len()) {
            return Err(format!("face {face} is out of range"));
        }
        let id = self.allocate_group_id()?;
        let selected_faces = selected.iter().copied().collect::<HashSet<_>>();
        for group in &mut self.groups {
            group.faces.retain(|face| !selected_faces.contains(face));
        }
        self.retain_nonempty_geometry_groups();
        self.groups.push(FaceGroup {
            id,
            name: format!("Group {id}"),
            faces: selected,
            parent_id,
            program: FaceGroupProgram::Geometry,
            locked: false,
            hidden: false,
        });
        Ok(id)
    }

    fn allocate_group_id(&mut self) -> Result<u64, String> {
        let id = self.next_group_id.max(1);
        self.next_group_id = id
            .checked_add(1)
            .ok_or_else(|| "face group id limit reached".to_string())?;
        Ok(id)
    }

    fn retain_nonempty_geometry_groups(&mut self) {
        let parent_ids = self
            .groups
            .iter()
            .filter_map(|group| group.parent_id)
            .collect::<HashSet<_>>();
        self.groups.retain(|group| {
            !group.faces.is_empty()
                || group.program != FaceGroupProgram::Geometry
                || parent_ids.contains(&group.id)
        });
    }

    pub fn assign_faces_to_group(&mut self, faces: &[usize], group_id: u64) -> Result<(), String> {
        if !self.groups.iter().any(|group| group.id == group_id) {
            return Err(format!("face group {group_id} does not exist"));
        }
        let mut selected = faces.to_vec();
        selected.sort_unstable();
        selected.dedup();
        if let Some(&face) = selected.iter().find(|&&face| face >= self.face_names.len()) {
            return Err(format!("face {face} is out of range"));
        }
        let selected_faces = selected.iter().copied().collect::<HashSet<_>>();
        for group in &mut self.groups {
            if group.id != group_id {
                group.faces.retain(|face| !selected_faces.contains(face));
            }
        }
        self.retain_nonempty_geometry_groups();
        let target = self
            .groups
            .iter_mut()
            .find(|group| group.id == group_id)
            .ok_or_else(|| format!("face group {group_id} does not exist"))?;
        target.faces.extend(selected);
        target.faces.sort_unstable();
        target.faces.dedup();
        Ok(())
    }

    pub fn rename_face(&mut self, face: usize, name: String) -> Result<(), String> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err("face name cannot be empty".to_string());
        }
        let slot = self
            .face_names
            .get_mut(face)
            .ok_or_else(|| format!("face {face} is out of range"))?;
        *slot = Some(name);
        Ok(())
    }

    pub fn rename_group(&mut self, group_id: u64, name: String) -> Result<(), String> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err("group name cannot be empty".to_string());
        }
        let group = self
            .groups
            .iter_mut()
            .find(|group| group.id == group_id)
            .ok_or_else(|| format!("face group {group_id} does not exist"))?;
        group.name = name;
        Ok(())
    }

    pub fn remove_group(&mut self, group_id: u64) -> Result<Vec<usize>, String> {
        let group = self
            .groups
            .iter()
            .find(|group| group.id == group_id)
            .ok_or_else(|| format!("face group {group_id} does not exist"))?;
        let remove_radial_pivot = group.program == FaceGroupProgram::RadialRepeat;
        let mut group_ids = HashSet::from([group_id]);
        let mut pending = vec![group_id];
        let mut faces = Vec::new();
        while let Some(parent_id) = pending.pop() {
            if let Some(parent) = self.groups.iter().find(|group| group.id == parent_id) {
                faces.extend(parent.faces.iter().copied());
            }
            for child in self
                .groups
                .iter()
                .filter(|group| group.parent_id == Some(parent_id))
            {
                if group_ids.insert(child.id) {
                    pending.push(child.id);
                }
            }
        }
        self.groups.retain(|group| !group_ids.contains(&group.id));
        if remove_radial_pivot {
            self.radial_pivot = None;
        }
        faces.sort_unstable();
        faces.dedup();
        Ok(faces)
    }

    pub fn remap_after_face_removal(&mut self, face_remap: &[Option<usize>]) {
        let old_names = std::mem::take(&mut self.face_names);
        self.face_names = face_remap
            .iter()
            .filter_map(|mapped| mapped.map(|_| None))
            .collect();
        for (old_face, new_face) in face_remap.iter().enumerate() {
            if let Some(new_face) = new_face
                && let (Some(name), Some(slot)) = (
                    old_names.get(old_face).cloned().flatten(),
                    self.face_names.get_mut(*new_face),
                )
            {
                *slot = Some(name);
            }
        }
        for group in &mut self.groups {
            group.faces = group
                .faces
                .iter()
                .filter_map(|&face| face_remap.get(face).and_then(|mapped| *mapped))
                .collect();
            group.faces.sort_unstable();
            group.faces.dedup();
        }
        self.retain_nonempty_geometry_groups();
    }

    pub fn remap_after_face_rewrite(&mut self, face_map: &[Vec<usize>], new_face_count: usize) {
        let old_names = std::mem::take(&mut self.face_names);
        self.face_names = vec![None; new_face_count];
        for (old_face, new_faces) in face_map.iter().enumerate() {
            if let Some(name) = old_names.get(old_face).cloned().flatten() {
                for &new_face in new_faces {
                    if let Some(slot) = self.face_names.get_mut(new_face) {
                        *slot = Some(name.clone());
                    }
                }
            }
        }
        for group in &mut self.groups {
            group.faces = group
                .faces
                .iter()
                .flat_map(|&face| face_map.get(face).into_iter().flatten().copied())
                .collect();
            group.faces.sort_unstable();
            group.faces.dedup();
        }
        self.retain_nonempty_geometry_groups();
    }

    pub fn inherit_copied_faces(&mut self, copied_faces: &[(usize, usize)]) {
        let required = copied_faces
            .iter()
            .map(|&(_, destination)| destination + 1)
            .max()
            .unwrap_or(self.face_names.len());
        self.face_names.resize(required, None);
        for &(source, destination) in copied_faces {
            let name = self.face_names.get(source).cloned().flatten();
            if let Some(name) = name
                && let Some(slot) = self.face_names.get_mut(destination)
            {
                *slot = Some(name);
            }
        }
        for group in &mut self.groups {
            let copies = copied_faces
                .iter()
                .filter_map(|&(source, destination)| {
                    group.faces.contains(&source).then_some(destination)
                })
                .collect::<Vec<_>>();
            group.faces.extend(copies);
            group.faces.sort_unstable();
            group.faces.dedup();
        }
    }

    pub fn inherit_group_for_extrusion(
        &mut self,
        source_face: usize,
        first_new_face: usize,
        new_face_count: usize,
    ) -> Result<u64, String> {
        self.face_names
            .resize(first_new_face + new_face_count, None);
        let parent_id = self
            .groups
            .iter()
            .find(|group| group.faces.contains(&source_face))
            .and_then(|group| group.parent_id);
        let mut involved_faces = vec![source_face];
        involved_faces.extend(first_new_face..first_new_face + new_face_count);
        let group_id = self.create_group_under(&involved_faces, parent_id)?;
        self.rename_group(group_id, "Extrude".to_string())?;
        Ok(group_id)
    }
}

#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub definition: ObjectDefinition,
    pub mesh: Option<EditableMesh>,
    pub metadata: FaceMetadata,
    pub metadata_error: Option<String>,
    pub error: Option<String>,
    pub states: Vec<CatalogState>,
    pub states_error: Option<String>,
    pub variant_config: VariantConfig,
    pub variant_config_error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CatalogState {
    pub key: String,
    pub mesh: Option<EditableMesh>,
    pub metadata: FaceMetadata,
    pub metadata_error: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ObjectCatalog {
    entries: Vec<CatalogEntry>,
}

impl ObjectCatalog {
    pub fn load() -> Self {
        let entries = OBJECT_DEFINITIONS
            .iter()
            .copied()
            .map(|definition| {
                let states_directory = object_states_directory(&definition);
                let (mesh, error) = match load_glb(Path::new(definition.path)) {
                    Ok(mesh) => (Some(mesh), None),
                    Err(error) => (None, Some(error)),
                };
                let (metadata, metadata_error) = match mesh.as_ref() {
                    Some(mesh) => load_face_metadata(Path::new(definition.path), mesh.face_count()),
                    None => (FaceMetadata::default(), None),
                };
                let (states, states_error) = load_states(&states_directory);
                let (variant_config, variant_config_error) = load_variant_config(&states_directory);
                CatalogEntry {
                    definition,
                    mesh,
                    metadata,
                    metadata_error,
                    error,
                    states,
                    states_error,
                    variant_config,
                    variant_config_error,
                }
            })
            .collect();
        Self { entries }
    }

    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }

    pub fn get(&self, id: ObjectId) -> Option<&EditableMesh> {
        self.entries
            .iter()
            .find(|entry| entry.definition.id == id)?
            .mesh
            .as_ref()
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut EditableMesh> {
        self.entries
            .iter_mut()
            .find(|entry| entry.definition.id == id)?
            .mesh
            .as_mut()
    }

    pub fn get_state(&self, id: ObjectId, key: &str) -> Option<&EditableMesh> {
        self.entries
            .iter()
            .find(|entry| entry.definition.id == id)?
            .states
            .iter()
            .find(|state| state.key == key)?
            .mesh
            .as_ref()
    }

    pub fn metadata(&self, id: ObjectId, state_key: Option<&str>) -> Option<&FaceMetadata> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.definition.id == id)?;
        match state_key {
            Some(key) => entry
                .states
                .iter()
                .find(|state| state.key == key)
                .map(|state| &state.metadata),
            None => Some(&entry.metadata),
        }
    }

    pub fn metadata_mut(
        &mut self,
        id: ObjectId,
        state_key: Option<&str>,
    ) -> Option<&mut FaceMetadata> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.definition.id == id)?;
        match state_key {
            Some(key) => entry
                .states
                .iter_mut()
                .find(|state| state.key == key)
                .map(|state| &mut state.metadata),
            None => Some(&mut entry.metadata),
        }
    }

    pub fn custom_state_key(&self, id: ObjectId, condition_key: &str) -> Option<&str> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.definition.id == id)?;
        if entry.variant_config.mode != VariantMode::Custom {
            return None;
        }
        entry
            .variant_config
            .custom_patterns
            .iter()
            .find(|pattern| pattern.condition_key == condition_key)
            .map(|pattern| pattern.state_key.as_str())
    }

    pub fn variant_config(&self, id: ObjectId) -> Option<&VariantConfig> {
        self.entries
            .iter()
            .find(|entry| entry.definition.id == id)
            .map(|entry| &entry.variant_config)
    }

    pub fn placement_profile(&self, id: ObjectId) -> Option<PlacementProfile> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.definition.id == id)?;
        if entry.definition.category == ObjectCategory::Buildings {
            return Some(PlacementProfile::TriangleSlots);
        }
        Some(
            entry
                .variant_config
                .placement_profile
                .unwrap_or(entry.definition.placement_profile),
        )
    }

    pub fn set_placement_profile(
        &mut self,
        id: ObjectId,
        profile: PlacementProfile,
    ) -> Result<(), String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.definition.id == id)
            .ok_or_else(|| format!("unknown object id {id:?}"))?;
        if entry.definition.category == ObjectCategory::Buildings {
            return Err(
                "planet object placement profiles are fixed by the placement workflow".to_string(),
            );
        }
        let mut config = entry.variant_config.clone();
        config.placement_profile = Some(profile);
        config.validate()?;
        save_variant_config(&object_states_directory(&entry.definition), &config)?;
        entry.variant_config = config;
        entry.variant_config_error = None;
        Ok(())
    }

    pub fn save(&mut self, id: ObjectId, mesh: EditableMesh) -> Result<(), String> {
        let metadata = self
            .metadata(id, None)
            .cloned()
            .ok_or_else(|| format!("unknown object id {id:?}"))?;
        self.save_with_metadata(id, mesh, metadata)
    }

    pub fn save_with_metadata(
        &mut self,
        id: ObjectId,
        mesh: EditableMesh,
        mut metadata: FaceMetadata,
    ) -> Result<(), String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.definition.id == id)
            .ok_or_else(|| format!("unknown object id {id:?}"))?;
        metadata.validate(mesh.face_count())?;
        let path = Path::new(entry.definition.path);
        save_glb(path, &mesh)?;
        save_face_metadata(path, &metadata)?;
        entry.mesh = Some(mesh);
        entry.metadata = metadata;
        entry.metadata_error = None;
        entry.error = None;
        Ok(())
    }
}

fn face_metadata_path(mesh_path: &Path) -> std::path::PathBuf {
    mesh_path.with_extension("cad.json")
}

fn object_states_directory(definition: &ObjectDefinition) -> PathBuf {
    definition
        .states_path
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(definition.path).with_extension("states"))
}

fn variant_config_path(states_directory: &Path) -> PathBuf {
    states_directory.join("variant_rules.json")
}

fn load_variant_config(states_directory: &Path) -> (VariantConfig, Option<String>) {
    let path = variant_config_path(states_directory);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (VariantConfig::default(), None);
        }
        Err(error) => {
            return (
                VariantConfig::default(),
                Some(format!("{}: {error}", path.display())),
            );
        }
    };
    match serde_json::from_slice::<VariantConfig>(&bytes)
        .map_err(|error| format!("{}: {error}", path.display()))
        .and_then(|config| {
            config.validate()?;
            Ok(config)
        }) {
        Ok(config) => (config, None),
        Err(error) => (VariantConfig::default(), Some(error)),
    }
}

fn save_variant_config(states_directory: &Path, config: &VariantConfig) -> Result<(), String> {
    let path = variant_config_path(states_directory);
    fs::create_dir_all(states_directory)
        .map_err(|error| format!("{}: {error}", states_directory.display()))?;
    let json = serde_json::to_vec_pretty(config).map_err(|error| error.to_string())?;
    fs::write(&path, json).map_err(|error| format!("{}: {error}", path.display()))
}

fn load_face_metadata(mesh_path: &Path, face_count: usize) -> (FaceMetadata, Option<String>) {
    let path = face_metadata_path(mesh_path);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (FaceMetadata::for_face_count(face_count), None);
        }
        Err(error) => {
            return (
                FaceMetadata::for_face_count(face_count),
                Some(format!("{}: {error}", path.display())),
            );
        }
    };
    let loaded = serde_json::from_slice::<FaceMetadata>(&bytes)
        .map_err(|error| format!("{}: {error}", path.display()))
        .and_then(|mut metadata| {
            metadata.validate(face_count)?;
            Ok(metadata)
        });
    match loaded {
        Ok(metadata) => (metadata, None),
        Err(error) => (FaceMetadata::for_face_count(face_count), Some(error)),
    }
}

fn save_face_metadata(mesh_path: &Path, metadata: &FaceMetadata) -> Result<(), String> {
    let path = face_metadata_path(mesh_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let json = serde_json::to_vec_pretty(metadata).map_err(|error| error.to_string())?;
    fs::write(&path, json).map_err(|error| format!("{}: {error}", path.display()))
}

fn load_states(directory: &Path) -> (Vec<CatalogState>, Option<String>) {
    let mut states = Vec::new();
    let variants_path = directory.join("variants.glb");
    if variants_path.exists() {
        match load_glb_variants(&variants_path) {
            Ok(loaded) => states.extend(loaded.into_iter().map(|(key, mesh)| {
                let state_path = directory.join(format!("{key}.glb"));
                let (metadata, metadata_error) = load_face_metadata(&state_path, mesh.face_count());
                CatalogState {
                    key,
                    mesh: Some(mesh),
                    metadata,
                    metadata_error,
                    error: None,
                }
            })),
            Err(error) => return (states, Some(error)),
        }
    }
    if !directory.exists() {
        return (states, None);
    }
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            return (states, Some(format!("{}: {error}", directory.display())));
        }
    };
    for file in entries {
        let file = match file {
            Ok(file) => file,
            Err(error) => return (states, Some(error.to_string())),
        };
        let path = file.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("glb")
            || path.file_name().and_then(|name| name.to_str()) == Some("variants.glb")
        {
            continue;
        }
        let Some(key) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let loaded = load_glb(&path);
        let state = match loaded {
            Ok(mesh) => {
                let (metadata, metadata_error) = load_face_metadata(&path, mesh.face_count());
                CatalogState {
                    key: key.to_string(),
                    mesh: Some(mesh),
                    metadata,
                    metadata_error,
                    error: None,
                }
            }
            Err(error) => CatalogState {
                key: key.to_string(),
                mesh: None,
                metadata: FaceMetadata::default(),
                metadata_error: None,
                error: Some(error),
            },
        };
        if let Some(existing) = states.iter_mut().find(|state| state.key == key) {
            *existing = state;
        } else {
            states.push(state);
        }
    }
    states.sort_by(|left, right| left.key.cmp(&right.key));
    (states, None)
}

fn load_glb(path: &Path) -> Result<EditableMesh, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let gltf =
        gltf::Gltf::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    load_gltf_mesh(&gltf, path, None)
}

fn load_glb_variants(path: &Path) -> Result<Vec<(String, EditableMesh)>, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let gltf =
        gltf::Gltf::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    let blob = gltf
        .blob
        .as_deref()
        .ok_or_else(|| format!("{} has no embedded GLB binary buffer", path.display()))?;
    let document = &gltf.document;
    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or_else(|| format!("{} contains no scene", path.display()))?;
    let mut variants = Vec::new();
    let mut names = HashSet::new();
    for node in scene.nodes() {
        append_variant_node(
            node,
            identity_matrix(),
            blob,
            path,
            &mut names,
            &mut variants,
        )?;
    }
    if variants.is_empty() {
        return Err(format!("{} contains no named state meshes", path.display()));
    }
    variants.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(variants)
}

fn load_gltf_mesh(
    gltf: &gltf::Gltf,
    path: &Path,
    target_node: Option<&str>,
) -> Result<EditableMesh, String> {
    let blob = gltf
        .blob
        .as_deref()
        .ok_or_else(|| format!("{} has no embedded GLB binary buffer", path.display()))?;
    let document = &gltf.document;
    let mut mesh = EditableMesh {
        vertices: Vec::new(),
        indices: Vec::new(),
    };

    if let Some(target_node) = target_node {
        let scene = document
            .default_scene()
            .or_else(|| document.scenes().next())
            .ok_or_else(|| format!("{} contains no scene", path.display()))?;
        for node in scene.nodes() {
            append_named_node(node, identity_matrix(), blob, target_node, &mut mesh)?;
        }
    } else if let Some(scene) = document
        .default_scene()
        .or_else(|| document.scenes().next())
    {
        for node in scene.nodes() {
            append_node(node, identity_matrix(), blob, &mut mesh)?;
        }
    } else {
        for mesh_document in document.meshes() {
            append_mesh(&mesh_document, identity_matrix(), blob, &mut mesh)?;
        }
    }

    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return Err(format!("{} contains no triangle mesh", path.display()));
    }
    Ok(mesh)
}

type Matrix4 = [[f32; 4]; 4];

fn identity_matrix() -> Matrix4 {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn multiply_matrix(left: Matrix4, right: Matrix4) -> Matrix4 {
    let mut output = [[0.0; 4]; 4];
    for column in 0..4 {
        for row in 0..4 {
            output[column][row] = (0..4)
                .map(|index| left[index][row] * right[column][index])
                .sum();
        }
    }
    output
}

fn transform_point(matrix: Matrix4, point: Vec3) -> Vec3 {
    vec3(
        matrix[0][0] * point.x + matrix[1][0] * point.y + matrix[2][0] * point.z + matrix[3][0],
        matrix[0][1] * point.x + matrix[1][1] * point.y + matrix[2][1] * point.z + matrix[3][1],
        matrix[0][2] * point.x + matrix[1][2] * point.y + matrix[2][2] * point.z + matrix[3][2],
    )
}

fn append_node(
    node: gltf::Node<'_>,
    parent: Matrix4,
    blob: &[u8],
    output: &mut EditableMesh,
) -> Result<(), String> {
    let transform = multiply_matrix(parent, node.transform().matrix());
    if let Some(mesh) = node.mesh() {
        append_mesh(&mesh, transform, blob, output)?;
    }
    for child in node.children() {
        append_node(child, transform, blob, output)?;
    }
    Ok(())
}

fn append_named_node(
    node: gltf::Node<'_>,
    parent: Matrix4,
    blob: &[u8],
    target: &str,
    output: &mut EditableMesh,
) -> Result<(), String> {
    let transform = multiply_matrix(parent, node.transform().matrix());
    if node.name() == Some(target) {
        if let Some(mesh) = node.mesh() {
            append_mesh(&mesh, transform, blob, output)?;
        }
        return Ok(());
    }
    for child in node.children() {
        append_named_node(child, transform, blob, target, output)?;
    }
    Ok(())
}

fn append_variant_node(
    node: gltf::Node<'_>,
    parent: Matrix4,
    blob: &[u8],
    path: &Path,
    names: &mut HashSet<String>,
    output: &mut Vec<(String, EditableMesh)>,
) -> Result<(), String> {
    let transform = multiply_matrix(parent, node.transform().matrix());
    if let Some(mesh_document) = node.mesh() {
        let name = node
            .name()
            .ok_or_else(|| format!("{} contains an unnamed state mesh", path.display()))?
            .to_string();
        if !names.insert(name.clone()) {
            return Err(format!(
                "{} contains duplicate state name '{name}'",
                path.display()
            ));
        }
        let mut mesh = EditableMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
        append_mesh(&mesh_document, transform, blob, &mut mesh)?;
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return Err(format!(
                "{} state '{name}' contains no triangle mesh",
                path.display()
            ));
        }
        output.push((name, mesh));
    }
    for child in node.children() {
        append_variant_node(child, transform, blob, path, names, output)?;
    }
    Ok(())
}

fn append_mesh(
    mesh: &gltf::Mesh<'_>,
    transform: Matrix4,
    blob: &[u8],
    output: &mut EditableMesh,
) -> Result<(), String> {
    for primitive in mesh.primitives() {
        if primitive.mode() != gltf::mesh::Mode::Triangles {
            return Err(format!(
                "mesh '{}' uses unsupported {:?} primitive mode",
                mesh.name().unwrap_or("unnamed"),
                primitive.mode()
            ));
        }
        let reader = primitive
            .reader(|buffer| matches!(buffer.source(), gltf::buffer::Source::Bin).then_some(blob));
        let positions = reader
            .read_positions()
            .ok_or_else(|| {
                format!(
                    "mesh '{}' has no positions",
                    mesh.name().unwrap_or("unnamed")
                )
            })?
            .collect::<Vec<_>>();
        let color = primitive
            .material()
            .pbr_metallic_roughness()
            .base_color_factor();
        let material_color = Color::new(color[0], color[1], color[2], color[3]);
        let vertex_colors = reader
            .read_colors(0)
            .map(|colors| colors.into_rgba_f32().collect::<Vec<_>>());
        if primitive.get(&gltf::Semantic::Colors(0)).is_some()
            && vertex_colors.as_ref().map(Vec::len) != Some(positions.len())
        {
            return Err(format!(
                "mesh '{}' has an invalid vertex color buffer",
                mesh.name().unwrap_or("unnamed")
            ));
        }
        let base = u32::try_from(output.vertices.len())
            .map_err(|_| "GLB has too many vertices".to_string())?;
        output
            .vertices
            .extend(positions.into_iter().enumerate().map(|(index, position)| {
                let position =
                    transform_point(transform, vec3(position[0], position[1], position[2]));
                let color = vertex_colors
                    .as_ref()
                    .and_then(|colors| colors.get(index))
                    .map(|color| Color::new(color[0], color[1], color[2], color[3]))
                    .unwrap_or(material_color);
                Vertex::new2(position, Vec2::ZERO, color)
            }));
        let local_indices = match (primitive.indices().is_some(), reader.read_indices()) {
            (true, Some(indices)) => indices.into_u32().collect::<Vec<_>>(),
            (true, None) => {
                return Err(format!(
                    "mesh '{}' has an invalid index buffer",
                    mesh.name().unwrap_or("unnamed")
                ));
            }
            (false, _) => (0..(output.vertices.len() as u32 - base)).collect(),
        };
        if local_indices.len() % 3 != 0 {
            return Err("GLB triangle index count is not divisible by three".to_string());
        }
        for index in local_indices {
            let count = output.vertices.len() as u32 - base;
            if index >= count {
                return Err(format!("GLB index {index} is outside vertex range {count}"));
            }
            output.indices.push(base + index);
        }
    }
    Ok(())
}

pub fn save_glb(path: &Path, mesh: &EditableMesh) -> Result<(), String> {
    if mesh.vertices.is_empty() || mesh.indices.is_empty() || mesh.indices.len() % 3 != 0 {
        return Err("cannot save an empty or invalid triangle mesh".to_string());
    }
    for &index in &mesh.indices {
        if index as usize >= mesh.vertices.len() {
            return Err(format!("mesh index {index} is out of range"));
        }
    }

    let mut binary = Vec::new();
    let position_offset = binary.len();
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in &mesh.vertices {
        let p = vertex.position;
        let values = [p.x, p.y, p.z];
        for axis in 0..3 {
            min[axis] = min[axis].min(values[axis]);
            max[axis] = max[axis].max(values[axis]);
            binary.extend_from_slice(&values[axis].to_le_bytes());
        }
    }
    let position_length = binary.len() - position_offset;
    let color_offset = binary.len();
    for vertex in &mesh.vertices {
        for component in vertex.color {
            binary.extend_from_slice(&(component as f32 / 255.0).to_le_bytes());
        }
    }
    let color_length = binary.len() - color_offset;
    let index_offset = binary.len();
    for &index in &mesh.indices {
        binary.extend_from_slice(&index.to_le_bytes());
    }
    let index_length = binary.len() - index_offset;
    while binary.len() % 4 != 0 {
        binary.push(0);
    }

    let document = serde_json::json!({
        "asset": { "version": "2.0", "generator": "GridGame CAD editor" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0 }],
        "meshes": [{
            "primitives": [{
                "attributes": { "POSITION": 0, "COLOR_0": 1 },
                "indices": 2,
                "material": 0
            }]
        }],
        "materials": [{
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                "metallicFactor": 0.0,
                "roughnessFactor": 1.0
            },
            "doubleSided": true
        }],
        "buffers": [{ "byteLength": binary.len() }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": position_offset, "byteLength": position_length, "target": 34962 },
            { "buffer": 0, "byteOffset": color_offset, "byteLength": color_length, "target": 34962 },
            { "buffer": 0, "byteOffset": index_offset, "byteLength": index_length, "target": 34963 }
        ],
        "accessors": [
            {
                "bufferView": 0, "componentType": 5126, "count": mesh.vertices.len(),
                "type": "VEC3", "min": min, "max": max
            },
            {
                "bufferView": 1, "componentType": 5126, "count": mesh.vertices.len(),
                "type": "VEC4"
            },
            {
                "bufferView": 2, "componentType": 5125, "count": mesh.indices.len(),
                "type": "SCALAR"
            }
        ]
    });
    let mut json = serde_json::to_vec(&document).map_err(|error| error.to_string())?;
    while json.len() % 4 != 0 {
        json.push(b' ');
    }

    let total_length = 12 + 8 + json.len() + 8 + binary.len();
    let total_length =
        u32::try_from(total_length).map_err(|_| "GLB file exceeds 4 GiB".to_string())?;
    let mut glb = Vec::with_capacity(total_length as usize);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&total_length.to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F534A_u32.to_le_bytes());
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E4942_u32.to_le_bytes());
    glb.extend_from_slice(&binary);

    fs::write(path, glb).map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_building_scale_matches_the_cad_cell_and_normalizes_mesh_extent() {
        let color = Color::from_rgba(255, 255, 255, 255);
        let mesh = EditableMesh {
            vertices: vec![
                Vertex::new2(vec3(-1.0, 0.0, -2.0), Vec2::ZERO, color),
                Vertex::new2(vec3(3.0, 2.0, 2.0), Vec2::ZERO, color),
            ],
            indices: Vec::new(),
        };
        let (center, scale) = mesh_center_and_scale(&mesh);

        assert_eq!(center, vec3(1.0, 1.0, 0.0));
        assert!((scale - NORMALIZED_MODEL_EXTENT / 4.0).abs() < 1e-6);
        assert!(
            (building_model_scale_for_cell(BUILDING_PREVIEW_CELL_RADIUS) - BUILDING_PREVIEW_SCALE)
                .abs()
                < 1e-6
        );
        assert!(
            (NORMALIZED_MODEL_EXTENT / BUILDING_PREVIEW_CELL_GUIDE_RADIUS
                - building_model_scale_for_cell(BUILDING_PREVIEW_CELL_RADIUS)
                    * NORMALIZED_MODEL_EXTENT
                    / BUILDING_PREVIEW_CELL_RADIUS)
                .abs()
                < 1e-6
        );
        assert!(
            (building_model_scale_for_cell(2.0) / building_model_scale_for_cell(1.0) - 2.0).abs()
                < 1e-6
        );
    }

    #[test]
    fn catalog_has_unique_ids_and_paths() {
        for (index, definition) in OBJECT_DEFINITIONS.iter().enumerate() {
            assert!(
                !OBJECT_DEFINITIONS[index + 1..]
                    .iter()
                    .any(|later| later.id == definition.id || later.path == definition.path),
                "duplicate object id or asset path for {}",
                definition.label
            );
        }
    }

    #[test]
    fn machine_types_have_catalog_assets() {
        let kinds = [
            MachineBlockType::ConveyorBelt,
            MachineBlockType::MechanicArm,
            MachineBlockType::Chest,
            MachineBlockType::Assembler,
            MachineBlockType::Furnace,
            MachineBlockType::Pallet,
            MachineBlockType::Crate,
        ];
        for kind in kinds {
            assert!(
                OBJECT_DEFINITIONS
                    .iter()
                    .any(|definition| definition.id == machine_object_id(kind))
            );
        }
    }

    #[test]
    fn every_catalog_asset_loads_from_glb() {
        let catalog = ObjectCatalog::load();
        for entry in catalog.entries() {
            assert!(
                entry.mesh.is_some(),
                "{} failed to load: {:?}",
                entry.definition.path,
                entry.error
            );
        }
    }

    #[test]
    fn catalog_assets_retain_vertex_colors() {
        let catalog = ObjectCatalog::load();
        for entry in catalog.entries() {
            let mesh = entry
                .mesh
                .as_ref()
                .expect("catalog asset should load before checking colors");
            assert!(
                mesh.vertices
                    .iter()
                    .any(|vertex| vertex.color != [255, 255, 255, 255]),
                "{} has no non-white vertex color: {:?}",
                entry.definition.path,
                mesh.vertices
                    .iter()
                    .take(4)
                    .map(|vertex| vertex.color)
                    .collect::<Vec<_>>()
            );
            assert_eq!(mesh.indices.len() % 3, 0);
            assert!(
                mesh.indices
                    .iter()
                    .enumerate()
                    .any(|(position, &index)| position as u32 != index),
                "{} did not retain its GLB triangle indices",
                entry.definition.path
            );
        }
    }

    #[test]
    fn catalog_assets_preserve_triangle_indices() {
        let catalog = ObjectCatalog::load();
        let mesh = catalog
            .get(ObjectId::PlanetBase)
            .expect("base model should load");
        assert!(
            mesh.indices
                .iter()
                .enumerate()
                .any(|(position, &index)| position as u32 != index),
            "base model indices should come from its GLB accessor"
        );
    }

    #[test]
    fn conveyor_neighbor_states_load_from_subfolder() {
        let catalog = ObjectCatalog::load();
        let conveyor_states = &catalog
            .entries()
            .iter()
            .find(|entry| entry.definition.id == ObjectId::ConveyorBelt)
            .expect("conveyor entry should exist")
            .states;
        assert_eq!(conveyor_states.len(), 784);
        assert!(
            catalog
                .get_state(ObjectId::ConveyorBelt, "f0-b0-s00")
                .is_some()
        );
        assert!(
            catalog
                .get_state(ObjectId::ConveyorBelt, "f1-b0-s00")
                .is_some()
        );
        for state in &catalog
            .entries()
            .iter()
            .find(|entry| entry.definition.id == ObjectId::ConveyorBelt)
            .expect("conveyor state object should exist")
            .states
        {
            let mesh = state
                .mesh
                .as_ref()
                .unwrap_or_else(|| panic!("{} failed to load: {:?}", state.key, state.error));
            assert!(
                mesh.vertices
                    .iter()
                    .any(|vertex| vertex.color != [255, 255, 255, 255]),
                "{} lost its vertex colors",
                state.key
            );
            for triangle in mesh.indices.chunks_exact(3) {
                let a = mesh.vertices[triangle[0] as usize].position;
                let b = mesh.vertices[triangle[1] as usize].position;
                let c = mesh.vertices[triangle[2] as usize].position;
                assert!(
                    (b - a).cross(c - a).length_squared() > 1e-10,
                    "{} contains a degenerate triangle",
                    state.key
                );
            }
        }
    }

    #[test]
    fn removing_faces_compacts_vertices_and_remaps_face_metadata() {
        let vertex =
            |position| Vertex::new2(position, Vec2::ZERO, Color::from_rgba(80, 120, 160, 255));
        let mut mesh = EditableMesh {
            vertices: vec![
                vertex(Vec3::ZERO),
                vertex(Vec3::X),
                vertex(Vec3::Y),
                vertex(Vec3::Z),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        let mut metadata = FaceMetadata::for_face_count(2);
        metadata.face_names[0] = Some("front".to_string());
        metadata.face_names[1] = Some("side".to_string());
        let group = metadata.create_group(&[0, 1]).unwrap();
        metadata.radial_pivot = Some([2.0, 3.0, 4.0]);

        let remap = mesh.remove_faces(&[0]).unwrap();
        metadata.remap_after_face_removal(&remap);

        assert_eq!(remap, vec![None, Some(0)]);
        assert_eq!(metadata.radial_pivot, Some([2.0, 3.0, 4.0]));
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.indices, vec![0, 1, 2]);
        assert_eq!(metadata.face_names, vec![Some("side".to_string())]);
        assert_eq!(metadata.group(group).unwrap().faces, vec![0]);
    }

    #[test]
    fn trim_splits_intersected_target_face_and_keeps_both_sides() {
        let vertex =
            |position| Vertex::new2(position, Vec2::ZERO, Color::from_rgba(255, 255, 255, 255));
        let mut mesh = EditableMesh {
            vertices: vec![
                vertex(vec3(0.0, 0.0, 0.0)),
                vertex(vec3(2.0, 0.0, 0.0)),
                vertex(vec3(0.0, 2.0, 0.0)),
                vertex(vec3(0.5, -1.0, -1.0)),
                vertex(vec3(0.5, 3.0, -1.0)),
                vertex(vec3(0.5, 1.0, 1.0)),
            ],
            indices: vec![0, 1, 2, 3, 4, 5],
        };
        let face_map = mesh.split_faces_by_intersections(&[0], &[1], None).unwrap();

        assert_eq!(mesh.face_count(), 4);
        assert_eq!(face_map[0].len(), 3);
        assert_eq!(face_map[1].len(), 1);
        let centroids = face_map[0]
            .iter()
            .map(|&face| {
                let start = face * 3;
                (0..3)
                    .map(|slot| {
                        mesh.vertices[mesh.indices[start + slot] as usize]
                            .position
                            .x
                    })
                    .sum::<f32>()
                    / 3.0
            })
            .collect::<Vec<_>>();
        assert!(centroids.iter().any(|&x| x < 0.5));
        assert!(centroids.iter().any(|&x| x > 0.5));
    }

    #[test]
    fn trim_rejects_non_intersecting_cutter_faces_without_mutating_mesh() {
        let vertex =
            |position| Vertex::new2(position, Vec2::ZERO, Color::from_rgba(255, 255, 255, 255));
        let mut mesh = EditableMesh {
            vertices: vec![
                vertex(vec3(0.0, 0.0, 0.0)),
                vertex(vec3(2.0, 0.0, 0.0)),
                vertex(vec3(0.0, 2.0, 0.0)),
                vertex(vec3(5.5, -1.0, -1.0)),
                vertex(vec3(5.5, 3.0, -1.0)),
                vertex(vec3(5.5, 1.0, 1.0)),
            ],
            indices: vec![0, 1, 2, 3, 4, 5],
        };
        let original = mesh.clone();

        assert!(mesh.split_faces_by_intersections(&[0], &[1], None).is_err());
        assert_eq!(mesh.indices, original.indices);
        assert_eq!(
            mesh.vertices
                .iter()
                .map(|vertex| vertex.position)
                .collect::<Vec<_>>(),
            original
                .vertices
                .iter()
                .map(|vertex| vertex.position)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn trim_multiline_splits_only_the_selected_target_face() {
        let vertex =
            |position| Vertex::new2(position, Vec2::ZERO, Color::from_rgba(255, 255, 255, 255));
        let mut mesh = EditableMesh {
            vertices: vec![
                vertex(vec3(0.0, 0.0, 0.0)),
                vertex(vec3(2.0, 0.0, 0.0)),
                vertex(vec3(0.0, 2.0, 0.0)),
                vertex(vec3(0.0, 0.0, 1.0)),
                vertex(vec3(2.0, 0.0, 1.0)),
                vertex(vec3(0.0, 2.0, 1.0)),
            ],
            indices: vec![0, 1, 2, 3, 4, 5],
        };
        let path = vec![
            vec3(0.0, 1.0, 0.0),
            vec3(0.4, 0.4, 0.0),
            vec3(1.0, 0.0, 0.0),
        ];
        let face_map = mesh
            .split_faces_by_intersections(&[0], &[], Some((0, &path)))
            .unwrap();

        assert_eq!(face_map[0].len(), 5);
        assert_eq!(face_map[1], vec![5]);
        let target_area = face_map[0]
            .iter()
            .map(|&face| {
                let start = face * 3;
                let a = mesh.vertices[mesh.indices[start] as usize].position;
                let b = mesh.vertices[mesh.indices[start + 1] as usize].position;
                let c = mesh.vertices[mesh.indices[start + 2] as usize].position;
                (b - a).cross(c - a).length() * 0.5
            })
            .sum::<f32>();
        assert!((target_area - 2.0).abs() < 1e-4);
    }

    #[test]
    fn reflected_and_offset_face_copies_keep_source_geometry_and_report_face_mapping() {
        let mut mesh = EditableMesh::regular_polygon(3).unwrap();
        let original_indices = mesh.indices.clone();
        let original_positions = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>();
        let reflected = mesh
            .copy_faces_reflected(&[0], Vec3::ZERO, Vec3::X)
            .unwrap();
        assert_eq!(mesh.face_count(), 4);
        assert_eq!(&mesh.indices[..original_indices.len()], original_indices);
        assert_eq!(reflected, vec![(0, 3)]);
        let source = &original_indices[..3];
        let mirrored = &mesh.indices[9..12];
        for (mirrored_index, source_index) in mirrored.iter().zip([source[0], source[2], source[1]])
        {
            assert_eq!(
                mesh.vertices[*mirrored_index as usize].position.x,
                -original_positions[source_index as usize].x
            );
        }

        let before_offset = mesh.face_count();
        let offset = mesh.copy_faces_offset(&[1], 0.5).unwrap();
        assert_eq!(mesh.face_count(), before_offset + 1);
        assert_eq!(offset, vec![(1, before_offset)]);
        for &index in &mesh.indices[before_offset * 3..before_offset * 3 + 3] {
            assert!((mesh.vertices[index as usize].position.y - 0.5).abs() < 1e-5);
        }
    }

    #[test]
    fn fillet_replaces_a_manifold_crease_with_a_rounded_triangle_strip() {
        let mut mesh = EditableMesh::primitive(MeshPrimitive::Cube);
        let original_face_count = mesh.face_count();
        let face_map = mesh.fillet_edges(&[(0, 1)], 0.1, 4).unwrap();

        assert_eq!(mesh.face_count(), original_face_count + 8);
        assert_eq!(face_map.len(), original_face_count);
        assert_eq!(face_map[0].len() + face_map[4].len(), 10);
        assert!(!mesh.indices.chunks_exact(3).any(|triangle| {
            (0..3).any(|side| {
                let a = triangle[side];
                let b = triangle[(side + 1) % 3];
                (a == 0 && b == 1) || (a == 1 && b == 0)
            })
        }));
    }

    #[test]
    fn fillet_rejects_non_manifold_or_oversized_edges_without_mutation() {
        let mut mesh = EditableMesh::primitive(MeshPrimitive::Cube);
        let original = mesh.indices.clone();

        assert!(mesh.fillet_edges(&[(0, 1)], 10.0, 4).is_err());
        assert_eq!(mesh.indices, original);
    }

    #[test]
    fn extracting_and_composing_face_groups_compacts_and_offsets_indices() {
        let white = Color::from_rgba(255, 255, 255, 255);
        let vertex = |position| Vertex::new2(position, Vec2::ZERO, white);
        let mesh = EditableMesh {
            vertices: vec![
                vertex(Vec3::ZERO),
                vertex(Vec3::X),
                vertex(Vec3::Y),
                vertex(Vec3::Z),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        let mut composed = mesh.extract_faces(&[0]).unwrap();
        let second = mesh.extract_faces(&[1]).unwrap();
        composed.append(&second).unwrap();

        assert_eq!(composed.vertices.len(), 6);
        assert_eq!(composed.indices, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn reflected_belt_state_swaps_sides_and_preserves_winding() {
        let key = "f1-b2-s01";
        let reflected_key = reflected_belt_state_key(key).unwrap();
        assert_eq!(reflected_key, "f6-b5-s08");
        assert_eq!(reflected_belt_state_key(&reflected_key).unwrap(), key);

        let white = Color::from_rgba(255, 255, 255, 255);
        let mesh = EditableMesh {
            vertices: vec![
                Vertex::new2(Vec3::ZERO, Vec2::ZERO, white),
                Vertex::new2(vec3(1.0, 0.0, 2.0), Vec2::ZERO, white),
                Vertex::new2(vec3(0.0, 1.0, -1.0), Vec2::ZERO, white),
            ],
            indices: vec![0, 1, 2],
        };
        let reflected = mesh.reflected_longitudinal().unwrap();
        assert_eq!(
            reflected.vertices[1].position.z,
            -mesh.vertices[1].position.z
        );
        assert_eq!(reflected.indices, vec![0, 2, 1]);
        assert_eq!(reflected_belt_state_key("f0-b0-s00").unwrap(), "f0-b0-s00");
    }

    #[test]
    fn variant_configuration_round_trips_and_checks_radial_slots() {
        let directory =
            std::env::temp_dir().join(format!("gridgame-variant-rules-{}", std::process::id()));
        let mut config = VariantConfig {
            mode: VariantMode::Radial,
            ..VariantConfig::default()
        };
        for sides in [5, 6] {
            for sector in 0..sides {
                config.radial_sector_groups.push(RadialSectorGroup {
                    sides,
                    sector,
                    face_group_id: sector as u64 + 1,
                });
            }
        }
        config.validate().unwrap();
        save_variant_config(&directory, &config).unwrap();
        let (loaded, error) = load_variant_config(&directory);

        assert!(error.is_none());
        assert_eq!(loaded, config);
        fs::remove_file(variant_config_path(&directory)).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn placement_profile_round_trips_in_variant_configuration() {
        let directory =
            std::env::temp_dir().join(format!("gridgame-placement-profile-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let config = VariantConfig {
            placement_profile: Some(PlacementProfile::NetworkSegments),
            ..VariantConfig::default()
        };
        save_variant_config(&directory, &config).unwrap();
        let (loaded, error) = load_variant_config(&directory);
        assert!(error.is_none());
        assert_eq!(
            loaded.placement_profile,
            Some(PlacementProfile::NetworkSegments)
        );
        fs::remove_file(variant_config_path(&directory)).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn planet_buildings_use_triangle_slots() {
        let mut catalog = ObjectCatalog::load();
        assert_eq!(
            catalog.placement_profile(ObjectId::PlanetBase),
            Some(PlacementProfile::TriangleSlots)
        );
        assert!(
            catalog
                .entries()
                .iter()
                .all(|entry| entry.definition.label != "Route")
        );
        assert!(
            catalog
                .set_placement_profile(ObjectId::PlanetBase, PlacementProfile::CellCenter)
                .is_err()
        );
    }

    #[test]
    fn custom_variants_require_exact_condition_key_match() {
        let mut catalog = ObjectCatalog::load();
        let entry = catalog
            .entries
            .iter_mut()
            .find(|entry| entry.definition.id == ObjectId::PlanetBase)
            .unwrap();
        entry.variant_config.mode = VariantMode::Custom;
        entry.variant_config.custom_patterns = vec![CustomVariantPattern {
            name: "Straight route".to_string(),
            condition_key: "p6-002200".to_string(),
            state_key: "custom-straight-route".to_string(),
        }];

        assert_eq!(
            catalog.custom_state_key(ObjectId::PlanetBase, "p6-002200"),
            Some("custom-straight-route")
        );
        assert_eq!(
            catalog.custom_state_key(ObjectId::PlanetBase, "p6-002201"),
            None
        );
    }

    #[test]
    fn face_groups_move_members_and_can_be_renamed() {
        let mut metadata = FaceMetadata::for_face_count(4);
        let first = metadata.create_group(&[0, 1]).unwrap();
        let second = metadata.create_group(&[2]).unwrap();
        metadata.assign_faces_to_group(&[1, 2], second).unwrap();
        metadata.rename_group(second, "Trim".to_string()).unwrap();

        assert_eq!(metadata.group(first).unwrap().faces, vec![0]);
        assert_eq!(metadata.group(second).unwrap().name, "Trim");
        assert_eq!(metadata.group(second).unwrap().faces, vec![1, 2]);
    }

    #[test]
    fn legacy_face_groups_load_as_unparented_geometry_groups() {
        let group: FaceGroup =
            serde_json::from_str(r#"{"id":7,"name":"Existing","faces":[0]}"#).unwrap();
        let metadata: FaceMetadata =
            serde_json::from_str(r#"{"face_names":[],"groups":[],"next_group_id":1}"#).unwrap();

        assert_eq!(group.parent_id, None);
        assert_eq!(group.program, FaceGroupProgram::Geometry);
        assert!(metadata.groups.is_empty());
    }

    #[test]
    fn radial_program_repeats_main_geometry_and_adds_matching_condition_geometry() {
        let vertex = |x| {
            Vertex::new2(
                vec3(x, 0.0, 0.0),
                Vec2::ZERO,
                Color::from_rgba(80, 120, 160, 255),
            )
        };
        let mesh = EditableMesh {
            vertices: (0..12).map(|index| vertex(index as f32)).collect(),
            indices: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        };
        let mut metadata = FaceMetadata::for_face_count(4);
        let main_id = metadata.create_group(&[0]).unwrap();
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
        let other_group = metadata.create_group_under(&[2], Some(main_id)).unwrap();
        metadata
            .set_group_program(
                other_group,
                FaceGroupProgram::IfNeighbor(RadialNeighborState::OtherBuilding),
            )
            .unwrap();
        metadata.validate(4).unwrap();

        assert_eq!(metadata.faces_for_group(main_id), Some(vec![0]));
        assert_eq!(metadata.faces_for_group(route_group), Some(vec![1]));
        assert_eq!(
            metadata
                .radial_sector_mesh(&mesh, RadialNeighborState::Empty)
                .unwrap()
                .unwrap()
                .face_count(),
            1
        );
        assert_eq!(
            metadata
                .radial_sector_mesh(&mesh, RadialNeighborState::Route)
                .unwrap()
                .unwrap()
                .face_count(),
            2
        );
        assert_eq!(
            metadata
                .radial_sector_mesh(&mesh, RadialNeighborState::OtherBuilding)
                .unwrap()
                .unwrap()
                .face_count(),
            2
        );
        assert!(
            metadata
                .radial_sector_mesh(&mesh, RadialNeighborState::Empty)
                .unwrap()
                .is_some()
        );
        let mut without_route = metadata.clone();
        assert_eq!(without_route.remove_group(route_group).unwrap(), vec![1]);
        assert!(without_route.group(route_group).is_none());
        assert_eq!(without_route.radial_main_group_id(), Some(main_id));

        let mut without_radial = metadata.clone();
        without_radial.radial_pivot = Some([1.0, 2.0, 3.0]);
        assert_eq!(without_radial.remove_group(main_id).unwrap(), vec![0, 1, 2]);
        assert!(without_radial.groups.is_empty());
        assert_eq!(without_radial.radial_main_group_id(), None);
        assert_eq!(without_radial.radial_pivot, None);
    }

    #[test]
    fn model_program_groups_can_be_renamed() {
        let mut metadata = FaceMetadata::for_face_count(2);
        let radial = metadata.create_group(&[0]).unwrap();
        metadata
            .set_group_program(radial, FaceGroupProgram::RadialRepeat)
            .unwrap();
        metadata
            .rename_group(radial, "Main sector".to_string())
            .unwrap();

        assert_eq!(metadata.group(radial).unwrap().name, "Main sector");
        assert_eq!(
            metadata.group(radial).unwrap().program,
            FaceGroupProgram::RadialRepeat
        );
    }

    #[test]
    fn group_lock_and_hide_apply_to_descendants_and_serialize() {
        let mut metadata = FaceMetadata::for_face_count(2);
        let parent = metadata.create_group(&[0]).unwrap();
        let child = metadata.create_group_under(&[1], Some(parent)).unwrap();
        metadata.set_group_locked(parent, true).unwrap();
        metadata.set_group_hidden(parent, true).unwrap();

        assert!(metadata.group_is_locked(child));
        assert!(metadata.group_is_hidden(child));
        assert_eq!(metadata.locked_faces(), HashSet::from([0, 1]));
        assert_eq!(metadata.hidden_faces(), HashSet::from([0, 1]));
        assert!(metadata.group_has_locked_ancestor(child));
        assert!(metadata.group_has_hidden_ancestor(child));

        let json = serde_json::to_string(&metadata).unwrap();
        let loaded: FaceMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.locked_faces(), HashSet::from([0, 1]));
        assert_eq!(loaded.hidden_faces(), HashSet::from([0, 1]));
    }

    #[test]
    fn group_programs_are_configured_incrementally_and_reject_duplicate_conditions() {
        let mut metadata = FaceMetadata::for_face_count(3);
        let main = metadata.create_group(&[0]).unwrap();
        let condition = metadata.create_group_under(&[1], Some(main)).unwrap();

        assert_eq!(
            metadata.group(condition).unwrap().program,
            FaceGroupProgram::Geometry
        );
        assert!(
            metadata
                .set_group_program(
                    condition,
                    FaceGroupProgram::IfNeighbor(RadialNeighborState::Empty)
                )
                .is_err()
        );
        metadata
            .set_group_program(main, FaceGroupProgram::RadialRepeat)
            .unwrap();
        metadata
            .set_group_program(
                condition,
                FaceGroupProgram::IfNeighbor(RadialNeighborState::Empty),
            )
            .unwrap();
        let duplicate = metadata.create_group_under(&[2], Some(main)).unwrap();
        assert!(
            metadata
                .set_group_program(
                    duplicate,
                    FaceGroupProgram::IfNeighbor(RadialNeighborState::Empty)
                )
                .is_err()
        );
        assert_eq!(
            metadata.group(duplicate).unwrap().program,
            FaceGroupProgram::Geometry
        );
    }

    #[test]
    fn primitive_meshes_have_valid_non_degenerate_triangles() {
        for (primitive, expected_faces) in [
            (MeshPrimitive::Cube, 12),
            (MeshPrimitive::Polygon, 6),
            (MeshPrimitive::Sphere, 224),
            (MeshPrimitive::Cylinder, 64),
        ] {
            let mesh = EditableMesh::primitive(primitive);
            assert_eq!(mesh.face_count(), expected_faces);
            assert!(
                mesh.indices
                    .iter()
                    .all(|&index| (index as usize) < mesh.vertices.len())
            );
            for triangle in mesh.indices.chunks_exact(3) {
                let a = mesh.vertices[triangle[0] as usize].position;
                let b = mesh.vertices[triangle[1] as usize].position;
                let c = mesh.vertices[triangle[2] as usize].position;
                assert!((b - a).cross(c - a).length_squared() > 1e-10);
            }
        }
    }

    #[test]
    fn regular_polygon_validates_side_count_and_generates_one_triangle_per_side() {
        for sides in [3, 5, 6, 16] {
            let mesh = EditableMesh::regular_polygon(sides).unwrap();
            assert_eq!(mesh.face_count(), sides);
            assert_eq!(mesh.vertices.len(), sides + 1);
        }
        assert!(EditableMesh::regular_polygon(2).is_err());
        assert!(EditableMesh::regular_polygon(17).is_err());
    }

    #[test]
    fn extrusion_creates_group_for_source_and_new_faces_without_names() {
        let mut metadata = FaceMetadata::for_face_count(2);
        metadata.rename_face(0, "source".to_string()).unwrap();
        metadata.create_group(&[0]).unwrap();

        let group = metadata.inherit_group_for_extrusion(0, 2, 7).unwrap();

        assert_eq!(
            metadata.group(group).unwrap().faces,
            vec![0, 2, 3, 4, 5, 6, 7, 8]
        );
        assert_eq!(metadata.face_names.len(), 9);
        assert!(metadata.face_names[2..].iter().all(Option::is_none));
    }

    #[test]
    fn legacy_loose_lines_are_ignored_and_omitted_when_sidecar_is_saved() {
        let directory =
            std::env::temp_dir().join(format!("gridgame-cad-metadata-{}", std::process::id()));
        let mesh_path = directory.join("model.glb");
        let legacy_path = face_metadata_path(&mesh_path);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            &legacy_path,
            r#"{"face_names":[null],"groups":[],"loose_lines":[{"start":[0,0,0],"end":[1,0,0]}]}"#,
        )
        .unwrap();
        let (loaded, error) = load_face_metadata(&mesh_path, 3);

        assert!(error.is_none());
        assert_eq!(loaded.face_names, vec![None; 3]);
        assert_eq!(loaded.groups, Vec::new());
        save_face_metadata(&mesh_path, &loaded).unwrap();
        let saved = fs::read_to_string(&legacy_path).unwrap();
        assert!(!saved.contains("loose_lines"));
        fs::remove_file(legacy_path).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn painting_vertices_changes_color_without_changing_topology() {
        let white = Color::from_rgba(255, 255, 255, 255);
        let mut mesh = EditableMesh {
            vertices: vec![
                Vertex::new2(Vec3::ZERO, Vec2::ZERO, white),
                Vertex::new2(Vec3::X, Vec2::ZERO, white),
                Vertex::new2(Vec3::Y, Vec2::ZERO, white),
            ],
            indices: vec![0, 1, 2],
        };
        let original_indices = mesh.indices.clone();
        mesh.paint_vertices(&[0, 2], Color::from_rgba(12, 34, 56, 78))
            .unwrap();

        assert_eq!(mesh.vertices[0].color, [12, 34, 56, 78]);
        assert_eq!(mesh.vertices[1].color, [255, 255, 255, 255]);
        assert_eq!(mesh.vertices[2].color, [12, 34, 56, 78]);
        assert_eq!(mesh.indices, original_indices);
    }

    #[test]
    fn painting_a_face_does_not_recolor_an_adjacent_face() {
        let white = Color::from_rgba(255, 255, 255, 255);
        let red = Color::from_rgba(220, 70, 70, 255);
        let mut mesh = EditableMesh {
            vertices: vec![
                Vertex::new2(Vec3::ZERO, Vec2::ZERO, white),
                Vertex::new2(Vec3::X, Vec2::ZERO, white),
                Vertex::new2(Vec3::Y, Vec2::ZERO, white),
                Vertex::new2(Vec3::Z, Vec2::ZERO, white),
            ],
            indices: vec![0, 1, 2, 1, 3, 2],
        };
        mesh.paint_faces(&[0], red).unwrap();

        let first = &mesh.indices[..3];
        let adjacent = &mesh.indices[3..];
        assert!(
            first
                .iter()
                .all(|&index| mesh.vertices[index as usize].color == [220, 70, 70, 255])
        );
        assert!(
            adjacent
                .iter()
                .all(|&index| mesh.vertices[index as usize].color == [255, 255, 255, 255])
        );
        assert_eq!(mesh.face_count(), 2);
    }

    #[test]
    fn glb_round_trip_preserves_triangle_positions_and_colors() {
        let mesh = EditableMesh {
            vertices: vec![
                Vertex::new2(
                    vec3(-1.0, 0.0, 0.0),
                    Vec2::ZERO,
                    Color::from_rgba(12, 34, 56, 255),
                ),
                Vertex::new2(
                    vec3(1.0, 0.0, 0.0),
                    Vec2::ZERO,
                    Color::from_rgba(12, 34, 56, 255),
                ),
                Vertex::new2(
                    vec3(0.0, 1.0, 0.0),
                    Vec2::ZERO,
                    Color::from_rgba(12, 34, 56, 255),
                ),
            ],
            indices: vec![0, 1, 2],
        };
        let path = std::env::temp_dir().join(format!(
            "gridgame-object-roundtrip-{}.glb",
            std::process::id()
        ));
        save_glb(&path, &mesh).expect("write test GLB");
        let loaded = load_glb(&path).expect("read test GLB");
        let _ = fs::remove_file(path);
        assert_eq!(loaded.indices, mesh.indices);
        assert_eq!(loaded.vertices.len(), mesh.vertices.len());
        for (actual, expected) in loaded.vertices.iter().zip(mesh.vertices.iter()) {
            assert_eq!(actual.position, expected.position);
            assert_eq!(actual.color, expected.color);
        }
    }

    #[test]
    fn extruding_face_adds_top_and_side_triangles() {
        let mut mesh = EditableMesh {
            vertices: vec![
                Vertex::new2(Vec3::ZERO, Vec2::ZERO, Color::from_rgba(255, 255, 255, 255)),
                Vertex::new2(Vec3::X, Vec2::ZERO, Color::from_rgba(255, 255, 255, 255)),
                Vertex::new2(Vec3::Y, Vec2::ZERO, Color::from_rgba(255, 255, 255, 255)),
            ],
            indices: vec![0, 1, 2],
        };
        mesh.extrude_face(0, 1.0).expect("extrude valid face");
        assert_eq!(mesh.vertices.len(), 6);
        assert_eq!(mesh.face_count(), 8);
    }
}
