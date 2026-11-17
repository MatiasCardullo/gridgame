use std::collections::HashSet;
use std::fs;
use std::path::Path;

use macroquad::prelude::{Color, Mesh, Vec2, Vec3, Vertex, draw_mesh, vec3};

use crate::core::MachineBlockType;

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
    PlanetRoute,
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
    building(ObjectId::PlanetRoute, "Route", "route.glb"),
    ObjectDefinition {
        id: ObjectId::PlanetUnit,
        category: ObjectCategory::Units,
        label: "Planet unit",
        path: "assets/objects/units/planet_unit.glb",
        states_path: None,
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
        states_path: if matches!(id, ObjectId::PlanetRoute) {
            Some("assets/objects/buildings/route/states")
        } else {
            None
        },
        path: match id {
            ObjectId::PlanetBase => "assets/objects/buildings/base.glb",
            ObjectId::PlanetBuilder => "assets/objects/buildings/builder.glb",
            ObjectId::PlanetHousing => "assets/objects/buildings/housing.glb",
            ObjectId::PlanetFactory => "assets/objects/buildings/factory.glb",
            ObjectId::PlanetMine => "assets/objects/buildings/mine.glb",
            ObjectId::PlanetWarehouse => "assets/objects/buildings/warehouse.glb",
            ObjectId::PlanetRefuel => "assets/objects/buildings/refuel.glb",
            ObjectId::PlanetLogistics => "assets/objects/buildings/logistics.glb",
            ObjectId::PlanetRoute => "assets/objects/buildings/route.glb",
            _ => file_name,
        },
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

impl EditableMesh {
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
}

#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub definition: ObjectDefinition,
    pub mesh: Option<EditableMesh>,
    pub error: Option<String>,
    pub states: Vec<CatalogState>,
    pub states_error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CatalogState {
    pub key: String,
    pub mesh: Option<EditableMesh>,
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
                let (mesh, error) = match load_glb(Path::new(definition.path)) {
                    Ok(mesh) => (Some(mesh), None),
                    Err(error) => (None, Some(error)),
                };
                let (states, states_error) = load_states(definition.states_path);
                CatalogEntry {
                    definition,
                    mesh,
                    error,
                    states,
                    states_error,
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

    pub fn get_state_mut(&mut self, id: ObjectId, key: &str) -> Option<&mut EditableMesh> {
        self.entries
            .iter_mut()
            .find(|entry| entry.definition.id == id)?
            .states
            .iter_mut()
            .find(|state| state.key == key)?
            .mesh
            .as_mut()
    }

    pub fn save(&mut self, id: ObjectId, mesh: EditableMesh) -> Result<(), String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.definition.id == id)
            .ok_or_else(|| format!("unknown object id {id:?}"))?;
        save_glb(Path::new(entry.definition.path), &mesh)?;
        entry.mesh = Some(mesh);
        entry.error = None;
        Ok(())
    }

    pub fn save_state(
        &mut self,
        id: ObjectId,
        key: &str,
        mesh: EditableMesh,
    ) -> Result<(), String> {
        if key.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(format!("invalid state key '{key}'"));
        }
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.definition.id == id)
            .ok_or_else(|| format!("unknown object id {id:?}"))?;
        let states_dir = entry
            .definition
            .states_path
            .ok_or_else(|| format!("object {id:?} has no state assets"))?;
        let directory = Path::new(states_dir);
        fs::create_dir_all(directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        let path = directory.join(format!("{key}.glb"));
        save_glb(&path, &mesh)?;
        if let Some(state) = entry.states.iter_mut().find(|state| state.key == key) {
            state.mesh = Some(mesh);
            state.error = None;
        } else {
            entry.states.push(CatalogState {
                key: key.to_string(),
                mesh: Some(mesh),
                error: None,
            });
            entry.states.sort_by(|left, right| left.key.cmp(&right.key));
        }
        Ok(())
    }
}

fn load_states(states_path: Option<&str>) -> (Vec<CatalogState>, Option<String>) {
    let Some(states_path) = states_path else {
        return (Vec::new(), None);
    };
    let directory = Path::new(states_path);
    let mut states = Vec::new();
    let variants_path = directory.join("variants.glb");
    if variants_path.exists() {
        match load_glb_variants(&variants_path) {
            Ok(loaded) => states.extend(loaded.into_iter().map(|(key, mesh)| CatalogState {
                key,
                mesh: Some(mesh),
                error: None,
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
            Ok(mesh) => CatalogState {
                key: key.to_string(),
                mesh: Some(mesh),
                error: None,
            },
            Err(error) => CatalogState {
                key: key.to_string(),
                mesh: None,
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
    fn route_and_conveyor_neighbor_states_load_from_subfolders() {
        let catalog = ObjectCatalog::load();
        let route_states = &catalog
            .entries()
            .iter()
            .find(|entry| entry.definition.id == ObjectId::PlanetRoute)
            .expect("route entry should exist")
            .states;
        let conveyor_states = &catalog
            .entries()
            .iter()
            .find(|entry| entry.definition.id == ObjectId::ConveyorBelt)
            .expect("conveyor entry should exist")
            .states;
        assert_eq!(route_states.len(), 972);
        assert_eq!(conveyor_states.len(), 784);
        assert!(
            catalog
                .get_state(ObjectId::PlanetRoute, "p6-000000")
                .is_some()
        );
        assert!(
            catalog
                .get_state(ObjectId::ConveyorBelt, "f0-b0-s00")
                .is_some()
        );
        assert!(
            catalog
                .get_state(ObjectId::PlanetRoute, "p6-100000")
                .is_some()
        );
        assert!(
            catalog
                .get_state(ObjectId::ConveyorBelt, "f1-b0-s00")
                .is_some()
        );
        for id in [ObjectId::PlanetRoute, ObjectId::ConveyorBelt] {
            let entry = catalog
                .entries()
                .iter()
                .find(|entry| entry.definition.id == id)
                .expect("state object should exist");
            for state in &entry.states {
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
