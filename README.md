# GridGame

Hex-grid strategy game built with Rust and Macroquad. Build structures, extract resources, and move logistics units along routes.

**Requirements**
1. Rust and Cargo installed.

**Run**
1. `cargo run`

**Development**
1. `cargo check` for fast type checks.
2. `cargo build` for a debug build.
3. `cargo test` to run tests.
4. `scripts/check.ps1` to run the standard check script.

**Controls**
1. Left click: open a block window.
2. Right click: place a block or start demolish (when "Demolish" is selected).
3. Mouse wheel: zoom.
4. Middle mouse (drag): pan camera.
5. Arrow keys: rotate planet camera.
6. `R`: rotate placement.
7. `Esc`: back to menu (saves the game).
8. Planet scene: `1` no tool, `2` base, `3` mine, right click places building, `Delete` removes building.
9. Planet scene map export: `M` saves hovered zone PNG, `Shift+M` saves all zone PNGs to `planet_data/zone_maps/`.

**Notes**
1. Mines can expand their attached zones from the block window.
2. Logistics and Builder units are created from their block windows and can be listed with "Unit list".
3. Base auto-supply units prioritize non-Route construction and can source from Base or Warehouses.
4. Builder auto-supply units prioritize Route construction, move faster, and travel only along Routes.

**Planet Scene Data**
1. Terrain parameters and sector values are saved in `planet_data/planet_noise.json`.
2. The 2D heightmap texture is regenerated from that JSON when needed (no separate texture cache files).
3. Planet grid cache: `planet_data/planet_sim_grid.bin` (single grid used for hover + simulation).
5. Per-cell resource snapshot (depletable state): `planet_data/planet_resources.bin`.
6. Planet buildings snapshot: `planet_data/planet_buildings.json`.
7. Optional runtime artifacts:
8. `planet_data/planet_perf.log` for perf/hitch logging.
9. `planet_data/planet_mesh_points.csv` only when mesh-point export is enabled.

**Roadmap Notes**
1. `planet_sector.rs` will be deprecated in favor of the 3D `planet.rs` scene now that the global hex grid exists.
