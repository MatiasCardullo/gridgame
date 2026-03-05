use macroquad::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

#[derive(Debug)]
pub struct HexGrid {
    pub freq: i32,
    pub vertices: Vec<Vec3>,
    pub faces: Vec<[u32; 3]>,
    pub face_centers: Vec<Vec3>,
    pub vertex_faces: Vec<Vec<u32>>,
    pub base_face_buckets: Vec<Vec<u32>>,
    pub base_face_neighbors: Vec<Vec<usize>>,
}

// Builds the icosahedron edge list used for arcs and face construction.
pub fn icosahedron_edges() -> Vec<(usize, usize)> {
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for i in 2..6 {
        edges.push((0, i));
        edges.push((11, 4 + i));
    }
    for i in 1..5 {
        edges.push((i, i + 5));
        edges.push((i, i + 6));
    }
    edges.push((1, 5));
    edges.push((5, 10));
    edges.push((10, 6));
    for i in 0..11 {
        edges.push((i, i + 1));
    }
    edges
}

// Returns the 12 vertices of a unit icosahedron.
pub fn icosahedron_vertices() -> [Vec3; 12] {
    let phi = (1.0 + 5.0_f32.sqrt()) * 0.5;
    [
        vec3(-1.0, phi, 0.0),
        vec3(0.0, 1.0, phi),
        vec3(1.0, phi, 0.0),
        vec3(0.0, 1.0, -phi),
        vec3(-phi, 0.0, -1.0),
        vec3(-phi, 0.0, 1.0),
        vec3(0.0, -1.0, phi),
        vec3(phi, 0.0, 1.0),
        vec3(phi, 0.0, -1.0),
        vec3(0.0, -1.0, -phi),
        vec3(-1.0, -phi, 0.0),
        vec3(1.0, -phi, 0.0),
    ]
}

// Aligns icosahedron vertices so a tip points up the Y axis.
pub fn align_vertices_to_poles(vertices: &[Vec3; 12]) -> Vec<Vec3> {
    let mut out = vertices.to_vec();
    let mut max_y = -f32::MAX;
    let mut max_index = 0usize;
    for (idx, v) in out.iter().enumerate() {
        if v.y > max_y {
            max_y = v.y;
            max_index = idx;
        }
    }
    let top = out[max_index].normalize();
    let target = vec3(0.0, 1.0, 0.0);
    let axis = top.cross(target);
    let mut angle = top.dot(target).clamp(-1.0, 1.0).acos();
    if axis.length() < 1e-5 {
        angle = 0.0;
    }
    let axis = if axis.length() < 1e-5 {
        vec3(0.0, 1.0, 0.0)
    } else {
        axis.normalize()
    };
    for v in out.iter_mut() {
        *v = rotate_vec3(*v, axis, angle);
    }
    out
}

// Rotates a vector around an axis using Rodrigues' rotation formula.
pub fn rotate_vec3(v: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let axis = axis.normalize();
    let cos_theta = angle.cos();
    let sin_theta = angle.sin();
    v * cos_theta + axis.cross(v) * sin_theta + axis * (axis.dot(v) * (1.0 - cos_theta))
}

// Builds triangle faces from the icosahedron edges via planar checks.
pub fn build_faces(vertices: &[Vec3], edges: &[(usize, usize)]) -> Vec<[usize; 3]> {
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

// Subdivides the base icosahedron faces into 4 using shared midpoints.
pub fn subdivide_base_faces(
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
pub fn build_geodesic_sphere(
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
pub fn relax_sphere(
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
pub fn build_vertex_neighbors(faces: &[[usize; 3]], vertex_count: usize) -> Vec<Vec<usize>> {
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

pub fn build_face_neighbors(faces: &[[usize; 3]]) -> Vec<Vec<usize>> {
    let mut edge_map: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (face_index, face) in faces.iter().enumerate() {
        let edges = [
            (face[0], face[1]),
            (face[1], face[2]),
            (face[2], face[0]),
        ];
        for (a, b) in edges {
            let key = if a < b { (a, b) } else { (b, a) };
            edge_map.entry(key).or_default().push(face_index);
        }
    }

    let mut neighbors: Vec<HashSet<usize>> = vec![HashSet::new(); faces.len()];
    for face_list in edge_map.values() {
        if face_list.len() < 2 {
            continue;
        }
        for i in 0..face_list.len() {
            for j in (i + 1)..face_list.len() {
                let a = face_list[i];
                let b = face_list[j];
                neighbors[a].insert(b);
                neighbors[b].insert(a);
            }
        }
    }

    neighbors
        .into_iter()
        .map(|set| set.into_iter().collect())
        .collect()
}

pub fn load_hex_grid_cache(
    path: &str,
    freq: i32,
) -> Option<(Vec<Vec3>, Vec<[u32; 3]>)> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).ok()?;
    if &magic != b"HXG1" {
        return None;
    }
    let mut read_u32 = |reader: &mut BufReader<File>| -> Option<u32> {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf).ok()?;
        Some(u32::from_le_bytes(buf))
    };
    let stored_freq = read_u32(&mut reader)? as i32;
    if stored_freq != freq {
        return None;
    }
    let vertex_count = read_u32(&mut reader)? as usize;
    let face_count = read_u32(&mut reader)? as usize;
    if vertex_count == 0 || face_count == 0 {
        return None;
    }
    let mut vertices: Vec<Vec3> = Vec::with_capacity(vertex_count);
    for _ in 0..vertex_count {
        let mut buf = [0u8; 12];
        reader.read_exact(&mut buf).ok()?;
        let x = f32::from_le_bytes(buf[0..4].try_into().ok()?);
        let y = f32::from_le_bytes(buf[4..8].try_into().ok()?);
        let z = f32::from_le_bytes(buf[8..12].try_into().ok()?);
        vertices.push(vec3(x, y, z));
    }
    let mut faces: Vec<[u32; 3]> = Vec::with_capacity(face_count);
    for _ in 0..face_count {
        let a = read_u32(&mut reader)?;
        let b = read_u32(&mut reader)?;
        let c = read_u32(&mut reader)?;
        faces.push([a, b, c]);
    }
    Some((vertices, faces))
}

pub fn save_hex_grid_cache(path: &str, freq: i32, vertices: &[Vec3], faces: &[[u32; 3]]) {
    let Ok(file) = File::create(path) else {
        return;
    };
    let mut writer = BufWriter::new(file);
    let _ = writer.write_all(b"HXG1");
    let _ = writer.write_all(&(freq as u32).to_le_bytes());
    let _ = writer.write_all(&(vertices.len() as u32).to_le_bytes());
    let _ = writer.write_all(&(faces.len() as u32).to_le_bytes());
    for v in vertices {
        let _ = writer.write_all(&v.x.to_le_bytes());
        let _ = writer.write_all(&v.y.to_le_bytes());
        let _ = writer.write_all(&v.z.to_le_bytes());
    }
    for face in faces {
        let _ = writer.write_all(&face[0].to_le_bytes());
        let _ = writer.write_all(&face[1].to_le_bytes());
        let _ = writer.write_all(&face[2].to_le_bytes());
    }
    let _ = writer.flush();
}

pub fn build_frequency_geodesic(
    freq: i32,
    base_vertices: &[Vec3],
    base_faces: &[[usize; 3]],
) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let freq = freq.max(1) as usize;
    let mut vertices: Vec<Vec3> = base_vertices.iter().map(|v| v.normalize()).collect();
    let mut faces: Vec<[u32; 3]> = Vec::new();
    let mut edge_cache: HashMap<(usize, usize), Vec<usize>> = HashMap::new();

    let mut edge_indices = |a: usize,
                            b: usize,
                            vertices: &mut Vec<Vec3>,
                            edge_cache: &mut HashMap<(usize, usize), Vec<usize>>|
     -> Vec<usize> {
        let key = if a < b { (a, b) } else { (b, a) };
        if let Some(indices) = edge_cache.get(&key) {
            if a < b {
                return indices.clone();
            }
            let mut reversed = indices.clone();
            reversed.reverse();
            return reversed;
        }
        let va = base_vertices[a].normalize();
        let vb = base_vertices[b].normalize();
        let mut indices: Vec<usize> = Vec::with_capacity(freq + 1);
        for t in 0..=freq {
            if t == 0 {
                indices.push(a);
                continue;
            }
            if t == freq {
                indices.push(b);
                continue;
            }
            let alpha = t as f32 / freq as f32;
            let pos = (va * (1.0 - alpha) + vb * alpha).normalize();
            let idx = vertices.len();
            vertices.push(pos);
            indices.push(idx);
        }
        edge_cache.insert(key, indices.clone());
        if a < b {
            indices
        } else {
            let mut reversed = indices;
            reversed.reverse();
            reversed
        }
    };

    for face in base_faces {
        let a = face[0];
        let b = face[1];
        let c = face[2];
        let ab = edge_indices(a, b, &mut vertices, &mut edge_cache);
        let ac = edge_indices(a, c, &mut vertices, &mut edge_cache);
        let bc = edge_indices(b, c, &mut vertices, &mut edge_cache);

        let va = base_vertices[a].normalize();
        let vb = base_vertices[b].normalize();
        let vc = base_vertices[c].normalize();
        let mut grid: Vec<Vec<usize>> = Vec::with_capacity(freq + 1);
        for i in 0..=freq {
            grid.push(vec![0usize; freq + 1 - i]);
        }

        for i in 0..=freq {
            for j in 0..=freq - i {
                let index = if j == 0 {
                    ab[i]
                } else if i == 0 {
                    ac[j]
                } else if i + j == freq {
                    bc[j]
                } else {
                    let wa = (freq - i - j) as f32 / freq as f32;
                    let wb = i as f32 / freq as f32;
                    let wc = j as f32 / freq as f32;
                    let pos = (va * wa + vb * wb + vc * wc).normalize();
                    let idx = vertices.len();
                    vertices.push(pos);
                    idx
                };
                grid[i][j] = index;
            }
        }

        for i in 0..freq {
            for j in 0..(freq - i) {
                let v0 = grid[i][j] as u32;
                let v1 = grid[i + 1][j] as u32;
                let v2 = grid[i][j + 1] as u32;
                faces.push([v0, v1, v2]);
                if i + j < freq - 1 {
                    let v3 = grid[i + 1][j + 1] as u32;
                    faces.push([v1, v3, v2]);
                }
            }
        }
    }

    (vertices, faces)
}

pub fn build_hex_grid_from_data(
    freq: i32,
    vertices: Vec<Vec3>,
    faces: Vec<[u32; 3]>,
    base_vertices: &[Vec3],
    base_faces: &[[usize; 3]],
) -> HexGrid {
    let face_centers = faces
        .iter()
        .map(|face| {
            let v0 = vertices[face[0] as usize];
            let v1 = vertices[face[1] as usize];
            let v2 = vertices[face[2] as usize];
            (v0 + v1 + v2).normalize()
        })
        .collect::<Vec<_>>();
    let mut vertex_faces: Vec<Vec<u32>> = vec![Vec::new(); vertices.len()];
    for (index, face) in faces.iter().enumerate() {
        let idx = index as u32;
        vertex_faces[face[0] as usize].push(idx);
        vertex_faces[face[1] as usize].push(idx);
        vertex_faces[face[2] as usize].push(idx);
    }

    let base_face_normals = base_faces
        .iter()
        .copied()
        .map(|face| face_normal(base_vertices, face))
        .collect::<Vec<_>>();
    let base_face_neighbors = build_face_neighbors(base_faces);
    let mut base_face_buckets: Vec<Vec<u32>> = vec![Vec::new(); base_faces.len()];
    for (index, dir) in vertices.iter().enumerate() {
        let mut best_face = 0usize;
        let mut best_dot = -1.0_f32;
        for (face_index, normal) in base_face_normals.iter().enumerate() {
            let dot = dir.dot(*normal);
            if dot > best_dot {
                best_dot = dot;
                best_face = face_index;
            }
        }
        base_face_buckets[best_face].push(index as u32);
    }

    HexGrid {
        freq,
        vertices,
        faces,
        face_centers,
        vertex_faces,
        base_face_buckets,
        base_face_neighbors,
    }
}

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
