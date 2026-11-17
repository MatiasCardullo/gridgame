# GridGame

GridGame is a Rust and Macroquad hex-grid colony prototype. Current gameplay centers on generating a planet, placing buildings, mining resources, and supplying construction with logistics units.

## Requirements

- Rust and Cargo

## Run

```text
cargo run
```

Build and test:

```text
cargo build
cargo test
```

## Current implementation

- Main menu and runtime configuration screen.
- CAD object browser for planet buildings, units, and base-interior machine blocks, backed by editable GLB assets.
- In-game object viewer/editor with orbit, zoom, pan, vertex/face selection, transforms, extrusion, save, and discard.
- Procedurally generated spherical planet with cached mesh and simulation grid.
- Depletable per-cell resources: iron, copper, gold, zinc, lead, aluminum, and water.
- Planet buildings: Base, Builder, Housing, Factory, Mine, Warehouse, Refuel, Logistics, and Route.
- Persistent planet state for terrain parameters, resources, buildings, and units under `planet_data/`.
- Logistics and Builder units with cargo, depots, routes, construction supply, and role-specific auto-supply.
- Base interior scene with a 3D camera, conveyors, mechanic arms, pallets, crates, chests, assemblers, furnaces, and forklifts.
- Build-template sandbox for testing interior machine layouts and first-person movement.
- Local REST API for observing and controlling planet buildings and logistics units.

## Local REST API

When GridGame starts, it serves a versioned JSON API at `http://127.0.0.1:39123/api/v1`.
It binds only to loopback; clients on other machines cannot connect. The API has no
authentication, so only use it on a trusted computer. See [docs/api.md](docs/api.md)
for endpoints and request examples.

## Controls

### Planet

- Left click: select or place building.
- Right click: place selected building or confirm demolition.
- Mouse wheel: zoom.
- Middle mouse drag: orbit planet camera.
- Arrow keys: orbit planet camera.
- `1`: clear building tool.
- `2`-`9`: select Base, Builder, Housing, Factory, Mine, Warehouse, Refuel, or Logistics.
- `0`: select Route.
- `Delete`: select demolition.
- `P`: export test hex map PNG.
- `Escape`: return to menu and save state.

### Base interior

- Left click: place or delete machine block.
- Right click drag: move 3D camera.
- `WASD`: move camera.
- `Space` / `Ctrl`: move camera vertically.
- `R`: rotate selected block.
- `Escape`: return to planet.

### CAD object viewer

- Select an object category and model in the left panel.
- Right mouse drag: orbit; middle mouse drag: pan; mouse wheel: zoom.
- Select **Edit**, then choose vertex or face selection.
- Shift-click vertices to add or remove them from the selection.
- Use the move and scale buttons to edit the selection; select a face and use **Extrude** to add geometry.
- **Save** writes changes to the selected GLB asset; **Discard** restores its last saved geometry.

## Runtime data

The game creates `planet_data/` as needed:

- `planet_noise.json`: terrain parameters and sector values.
- `planet_sim_grid.bin`: cached simulation grid.
- `planet_resources.bin`: resource snapshot.
- `planet_buildings.json`: building and base-interior snapshot.
- `planet_units.json`: unit snapshot.
- `zone_maps/`: exported map images.
- `planet_perf.log`: optional performance log.
- `planet_mesh_points.csv`: optional mesh-point export.

## Project layout

- `src/main.rs`: application loop and scene routing.
- `src/scenes/`: main menu, configuration, planet, and build-template scenes.
- `src/core/`: shared grid, planet, unit, interior, persistence, and UI systems.
- `src/core/object_assets.rs`: GLB mesh loading, saving, object catalog, and edit operations.
- `src/scenes/cad.rs`: CAD object browser, viewer, camera, and mesh editor.
- `assets/objects/`: tracked GLB models grouped into `buildings/`, `units/`, and `machine_blocks/`.
- `config.json`: persisted runtime settings.
- `roadmap.md`: implementation status and next milestones.
