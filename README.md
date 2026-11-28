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
authentication, so only use it on a trusted computer. See [the API guide](docs/code/api.md)
for endpoints and request examples.

## Documentation

- [Code and developer workflows](docs/code/README.md)
- [Planet scene](docs/code/planet.md)
- [Planet units](docs/code/planet_units.md)
- [Build Template](docs/code/build_template.md)
- [CAD editor](docs/code/cad_editor.md)

## Controls

### Planet

- Left click: focus/select a cell; click an existing building to open its information window.
- Right click: place the selected building or route segment; with no build tool selected, request demolition.
- Mouse wheel: zoom.
- Middle mouse drag: orbit planet camera.
- Arrow keys: orbit planet camera.
- `1`: clear building tool.
- `2`-`9`: select Base, Builder, Housing, Factory, Mine, Warehouse, Refuel, or Logistics.
- `0`: select Route Line; `T` toggles Route/RailTrack; `Enter` finishes the chain.
- `Delete`: remove hovered building or selected/hovered network; `Backspace` confirms network removal.
- `P`: export test hex map PNG.
- `Escape`: return to menu and save state.

### Base interior

- Open it from a Base's information window using **Open Base Interior**.
- Select a panel item, then left-click an empty hex to place a machine block.
- Right-click a block to inspect/configure it; with no block selected, left-click an occupied hex to request removal.
- Hold right mouse and move to look around.
- `WASD`: move camera.
- `Space` / `Ctrl`: move camera vertically.
- `R`: rotate the hovered block or next placement.
- `Enter` / `Escape`: finish/cancel arm input/output picking; `Escape` returns to Planet when no selection or pick mode is active.

### CAD object viewer

- Select an object category and model in the left panel.
- Right mouse drag: orbit; middle mouse drag: pan; mouse wheel: zoom.
- Select **Edit**, then choose vertex, edge, or face selection.
- Shift-click vertices to add or remove them from the selection.
- Use the move and scale buttons to edit the selection; select a face and use **Extrude** to add geometry.
- **Save** writes changes to the selected asset; choose **Discard** in the unsaved-changes prompt to restore the last saved version.
- Radial Repeat groups, UCS origin placement, and editor metadata persistence are described in [the CAD guide](docs/code/cad_editor.md).

## Runtime data

The game creates `planet_data/` as needed:

- `planet_noise.json`: terrain parameters and sector values.
- `planet_sim_grid.bin`: cached simulation grid.
- `planet_resources.bin`: resource snapshot.
- `planet_buildings.json`: building and base-interior snapshot.
- `planet_networks.json`: route and rail network snapshot.
- `planet_units.json`: unit snapshot.
- `zone_maps/`: exported map images.
- `planet_perf.log`: optional performance log.
- `planet_mesh_points.csv`: optional mesh-point export.

## Project layout

- `src/main.rs`: application loop and scene routing.
- `src/scenes/`: main menu, configuration, planet, CAD, and build-template scenes.
- `src/scenes/planet/`: planet loading, persistence, and UI support modules.
- `src/scenes/cad/`: CAD transform and UI support modules.
- `src/core/`: shared grid, planet, unit, interior, persistence, and UI systems.
- `src/core/ui/`: reusable editor controls and palette components.
- `src/core/object_assets.rs`: GLB mesh loading, saving, object catalog, and edit operations.
- `docs/code/`: technical references and developer/gameplay workflow documentation.
- `assets/objects/`: tracked GLB models grouped into `buildings/`, `units/`, and `machine_blocks/`.
- `config.json`: persisted runtime settings.
