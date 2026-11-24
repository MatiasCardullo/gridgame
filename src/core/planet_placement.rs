use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BuildingSlot {
    pub cell_index: u32,
    pub triangle_index: u8,
}

impl BuildingSlot {
    pub fn is_valid(self, side_count: usize) -> bool {
        matches!(side_count, 5 | 6) && usize::from(self.triangle_index) < side_count
    }
}

pub fn building_slot_available(
    slot: BuildingSlot,
    side_count: usize,
    occupied_slots: impl IntoIterator<Item = BuildingSlot>,
) -> bool {
    slot.is_valid(side_count) && !occupied_slots.into_iter().any(|occupied| occupied == slot)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellRadius {
    Short,
    Long,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkAnchor {
    CellCenter { cell_index: u32 },
    CellCorner { face_index: u32 },
    SideMidpoint { first_face: u32, second_face: u32 },
}

fn shared_face_edge(first_face: u32, second_face: u32, faces: &[[u32; 3]]) -> Option<[u32; 2]> {
    let first = faces.get(first_face as usize)?;
    let second = faces.get(second_face as usize)?;
    let mut shared = [0; 2];
    let mut count = 0;
    for cell in first {
        if second.contains(cell) {
            if count == shared.len() {
                return None;
            }
            shared[count] = *cell;
            count += 1;
        }
    }
    (count == shared.len()).then_some(shared)
}

impl NetworkAnchor {
    pub fn canonicalized(self) -> Self {
        match self {
            Self::SideMidpoint {
                first_face,
                second_face,
            } => Self::SideMidpoint {
                first_face: first_face.min(second_face),
                second_face: first_face.max(second_face),
            },
            anchor => anchor,
        }
    }

    pub fn is_valid_for_topology(self, neighbors: &[Vec<u32>], faces: &[[u32; 3]]) -> bool {
        match self {
            Self::CellCenter { cell_index } => (cell_index as usize) < neighbors.len(),
            Self::CellCorner { face_index } => faces
                .get(face_index as usize)
                .is_some_and(|face| face.iter().all(|cell| (*cell as usize) < neighbors.len())),
            Self::SideMidpoint {
                first_face,
                second_face,
            } if first_face != second_face => shared_face_edge(first_face, second_face, faces)
                .is_some_and(|[first, second]| {
                    neighbors
                        .get(first as usize)
                        .is_some_and(|adjacent| adjacent.contains(&second))
                }),
            Self::SideMidpoint { .. } => false,
        }
    }

    pub fn incident_cells(self, faces: &[[u32; 3]]) -> Vec<u32> {
        match self {
            Self::CellCenter { cell_index } => vec![cell_index],
            Self::CellCorner { face_index } => {
                let mut cells = faces
                    .get(face_index as usize)
                    .map(|face| face.to_vec())
                    .unwrap_or_default();
                cells.sort_unstable();
                cells
            }
            Self::SideMidpoint {
                first_face,
                second_face,
            } => shared_face_edge(first_face, second_face, faces)
                .map(|mut cells| {
                    cells.sort_unstable();
                    cells.to_vec()
                })
                .unwrap_or_default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkKind {
    Route,
    RailTrack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPlacement {
    SharedSide {
        first_cell: u32,
        second_cell: u32,
    },
    MidpointSegment {
        cell_index: u32,
        first_neighbor: u32,
        second_neighbor: u32,
    },
    RadialSpan {
        cell_index: u32,
        neighbor_cell: u32,
        radius: CellRadius,
    },
    AnchorSegment {
        start: NetworkAnchor,
        end: NetworkAnchor,
    },
}

impl NetworkPlacement {
    pub fn canonicalized(self) -> Self {
        match self {
            Self::SharedSide {
                first_cell,
                second_cell,
            } => Self::SharedSide {
                first_cell: first_cell.min(second_cell),
                second_cell: first_cell.max(second_cell),
            },
            Self::MidpointSegment {
                cell_index,
                first_neighbor,
                second_neighbor,
            } => Self::MidpointSegment {
                cell_index,
                first_neighbor: first_neighbor.min(second_neighbor),
                second_neighbor: first_neighbor.max(second_neighbor),
            },
            Self::AnchorSegment { start, end } => {
                let start = start.canonicalized();
                let end = end.canonicalized();
                if start <= end {
                    Self::AnchorSegment { start, end }
                } else {
                    Self::AnchorSegment {
                        start: end,
                        end: start,
                    }
                }
            }
            placement => placement,
        }
    }

    pub fn is_valid(self, cell_count: usize) -> bool {
        match self {
            Self::SharedSide {
                first_cell,
                second_cell,
            } => {
                first_cell != second_cell
                    && (first_cell as usize) < cell_count
                    && (second_cell as usize) < cell_count
            }
            Self::MidpointSegment {
                cell_index,
                first_neighbor,
                second_neighbor,
            } => {
                first_neighbor != second_neighbor
                    && first_neighbor != cell_index
                    && second_neighbor != cell_index
                    && (cell_index as usize) < cell_count
                    && (first_neighbor as usize) < cell_count
                    && (second_neighbor as usize) < cell_count
            }
            Self::RadialSpan {
                cell_index,
                neighbor_cell,
                ..
            } => {
                neighbor_cell != cell_index
                    && (cell_index as usize) < cell_count
                    && (neighbor_cell as usize) < cell_count
            }
            Self::AnchorSegment { start, end } => start.canonicalized() != end.canonicalized(),
        }
    }

    pub fn is_valid_for_neighbors(self, neighbors: &[Vec<u32>]) -> bool {
        let is_neighbor = |cell: u32, other: u32| {
            neighbors
                .get(cell as usize)
                .is_some_and(|adjacent| adjacent.contains(&other))
        };
        match self {
            Self::SharedSide {
                first_cell,
                second_cell,
            } => is_neighbor(first_cell, second_cell),
            Self::MidpointSegment {
                cell_index,
                first_neighbor,
                second_neighbor,
            } => {
                is_neighbor(cell_index, first_neighbor) && is_neighbor(cell_index, second_neighbor)
            }
            Self::RadialSpan {
                cell_index,
                neighbor_cell,
                ..
            } => is_neighbor(cell_index, neighbor_cell),
            Self::AnchorSegment { .. } => false,
        }
    }

    pub fn is_valid_for_topology(self, neighbors: &[Vec<u32>], faces: &[[u32; 3]]) -> bool {
        match self {
            Self::AnchorSegment { start, end } => {
                start.canonicalized() != end.canonicalized()
                    && start.is_valid_for_topology(neighbors, faces)
                    && end.is_valid_for_topology(neighbors, faces)
            }
            placement => placement.is_valid_for_neighbors(neighbors),
        }
    }

    pub fn connected_cells(self) -> Vec<u32> {
        match self {
            Self::SharedSide {
                first_cell,
                second_cell,
            } => vec![first_cell, second_cell],
            Self::MidpointSegment {
                cell_index,
                first_neighbor,
                second_neighbor,
            } => vec![first_neighbor, cell_index, second_neighbor],
            Self::RadialSpan {
                cell_index,
                neighbor_cell,
                ..
            } => vec![cell_index, neighbor_cell],
            Self::AnchorSegment { start, end } => match (start, end) {
                (
                    NetworkAnchor::CellCenter { cell_index: start },
                    NetworkAnchor::CellCenter { cell_index: end },
                ) => vec![start, end],
                _ => Vec::new(),
            },
        }
    }

    pub fn connected_cell_edges(
        self,
        neighbors: &[Vec<u32>],
        faces: &[[u32; 3]],
    ) -> Vec<(u32, u32)> {
        let Self::AnchorSegment { start, end } = self else {
            return self
                .connected_cells()
                .windows(2)
                .map(|pair| (pair[0].min(pair[1]), pair[0].max(pair[1])))
                .collect();
        };
        let start_cells = start.incident_cells(faces);
        let end_cells = end.incident_cells(faces);
        let mut edges = Vec::new();
        for &first in &start_cells {
            for &second in &end_cells {
                if first != second
                    && neighbors
                        .get(first as usize)
                        .is_some_and(|adjacent| adjacent.contains(&second))
                {
                    edges.push((first.min(second), first.max(second)));
                }
            }
        }
        edges.sort_unstable();
        edges.dedup();
        edges
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NetworkSegment {
    pub id: u32,
    pub kind: NetworkKind,
    pub placement: NetworkPlacement,
}

#[cfg(test)]
#[path = "../../tests/core/planet_placement.rs"]
mod tests;
