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
