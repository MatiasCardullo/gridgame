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
mod tests {
    use super::{
        BuildingSlot, CellRadius, NetworkAnchor, NetworkPlacement, building_slot_available,
    };
    use std::collections::HashSet;

    #[test]
    fn building_slots_cover_pentagon_and_hexagon_triangles() {
        assert!(
            BuildingSlot {
                cell_index: 12,
                triangle_index: 4
            }
            .is_valid(5)
        );
        assert!(
            !BuildingSlot {
                cell_index: 12,
                triangle_index: 5
            }
            .is_valid(5)
        );
        assert!(
            BuildingSlot {
                cell_index: 18,
                triangle_index: 5
            }
            .is_valid(6)
        );
    }

    #[test]
    fn building_slots_reject_duplicate_and_out_of_range_occupancy() {
        let occupied = HashSet::from([BuildingSlot {
            cell_index: 12,
            triangle_index: 2,
        }]);
        assert!(!building_slot_available(
            BuildingSlot {
                cell_index: 12,
                triangle_index: 2
            },
            6,
            occupied.iter().copied()
        ));
        assert!(building_slot_available(
            BuildingSlot {
                cell_index: 12,
                triangle_index: 3
            },
            6,
            occupied.iter().copied()
        ));
        assert!(!building_slot_available(
            BuildingSlot {
                cell_index: 12,
                triangle_index: 5
            },
            5,
            occupied.iter().copied()
        ));
    }

    #[test]
    fn network_placements_validate_cell_references_and_connectivity() {
        assert!(
            NetworkPlacement::SharedSide {
                first_cell: 2,
                second_cell: 3
            }
            .is_valid(6)
        );
        assert!(
            !NetworkPlacement::SharedSide {
                first_cell: 2,
                second_cell: 2
            }
            .is_valid(6)
        );
        let midpoint = NetworkPlacement::MidpointSegment {
            cell_index: 4,
            first_neighbor: 0,
            second_neighbor: 2,
        };
        assert!(midpoint.is_valid(5));
        assert_eq!(midpoint.connected_cells(), vec![0, 4, 2]);
        assert!(
            !NetworkPlacement::MidpointSegment {
                cell_index: 4,
                first_neighbor: 0,
                second_neighbor: 4,
            }
            .is_valid(5)
        );
        let radial = NetworkPlacement::RadialSpan {
            cell_index: 4,
            neighbor_cell: 0,
            radius: CellRadius::Short,
        };
        assert!(radial.is_valid(5));
        assert_eq!(radial.connected_cells(), vec![4, 0]);
        assert_eq!(
            NetworkPlacement::SharedSide {
                first_cell: 4,
                second_cell: 2
            }
            .canonicalized(),
            NetworkPlacement::SharedSide {
                first_cell: 2,
                second_cell: 4
            }
        );

        let neighbors = vec![vec![1, 2], vec![0], vec![0]];
        assert!(
            NetworkPlacement::SharedSide {
                first_cell: 0,
                second_cell: 1
            }
            .is_valid_for_neighbors(&neighbors)
        );
        assert!(
            NetworkPlacement::MidpointSegment {
                cell_index: 0,
                first_neighbor: 1,
                second_neighbor: 2
            }
            .is_valid_for_neighbors(&neighbors)
        );
        assert!(
            !NetworkPlacement::SharedSide {
                first_cell: 1,
                second_cell: 2
            }
            .is_valid_for_neighbors(&neighbors)
        );
    }

    #[test]
    fn snapped_network_anchors_validate_and_connect_adjacent_cells() {
        let neighbors = vec![
            vec![1, 2, 3, 4],
            vec![0, 3, 4],
            vec![0, 4],
            vec![0, 1, 4],
            vec![0, 1, 2, 3],
        ];
        let mut faces = vec![[0, 0, 0]; 15];
        faces[10] = [0, 1, 3];
        faces[11] = [0, 1, 2];
        faces[12] = [0, 2, 4];
        faces[13] = [1, 3, 4];
        faces[14] = [2, 3, 4];
        let center = NetworkAnchor::CellCenter { cell_index: 0 };
        let corner = NetworkAnchor::CellCorner { face_index: 11 };
        let midpoint = NetworkAnchor::SideMidpoint {
            first_face: 11,
            second_face: 10,
        };
        assert!(center.is_valid_for_topology(&neighbors, &faces));
        assert!(corner.is_valid_for_topology(&neighbors, &faces));
        assert!(midpoint.is_valid_for_topology(&neighbors, &faces));
        assert_eq!(corner.incident_cells(&faces), vec![0, 1, 2]);
        assert_eq!(midpoint.incident_cells(&faces), vec![0, 1]);

        let segment = NetworkPlacement::AnchorSegment {
            start: center,
            end: midpoint,
        };
        assert!(segment.is_valid_for_topology(&neighbors, &faces));
        assert_eq!(
            segment.connected_cell_edges(&neighbors, &faces),
            vec![(0, 1)]
        );
        assert_eq!(
            segment.canonicalized(),
            NetworkPlacement::AnchorSegment {
                start: center,
                end: midpoint.canonicalized(),
            }
        );
        assert!(
            !NetworkAnchor::SideMidpoint {
                first_face: 10,
                second_face: 12
            }
            .is_valid_for_topology(&neighbors, &faces)
        );
    }

    #[test]
    fn network_placement_json_keeps_legacy_and_snapped_forms_readable() {
        let legacy: NetworkPlacement =
            serde_json::from_str(r#"{"shared_side":{"first_cell":3,"second_cell":7}}"#).unwrap();
        assert_eq!(
            legacy,
            NetworkPlacement::SharedSide {
                first_cell: 3,
                second_cell: 7
            }
        );

        let snapped: NetworkPlacement = serde_json::from_str(
            r#"{"anchor_segment":{"start":{"cell_center":{"cell_index":3}},"end":{"side_midpoint":{"first_face":9,"second_face":12}}}}"#,
        )
        .unwrap();
        assert_eq!(
            snapped.canonicalized(),
            NetworkPlacement::AnchorSegment {
                start: NetworkAnchor::CellCenter { cell_index: 3 },
                end: NetworkAnchor::SideMidpoint {
                    first_face: 9,
                    second_face: 12
                }
            }
            .canonicalized()
        );
    }
}
