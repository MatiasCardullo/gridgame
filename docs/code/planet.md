# Planet generation and gameplay workflow

The Planet scene coordinates terrain presentation, cell selection, construction,
mining, logistics, and persistence. Its implementation is split across
`src/scenes/planet.rs` and `src/scenes/planet/`: the scene facade owns state and
simulation coordination, while loader, persistence, and UI presentation helpers
live in focused submodules.

## Start and load a planet

The application starts `PlanetLoader` from `src/main.rs`. It builds or loads the
planet topology and height data asynchronously, publishes texture data when
available, then sends the completed relief data. The main frame loop polls these
results and applies them to `PlanetState`; background work does not mutate live
scene state. If Planet is entered before the state is ready, the loading screen
shows progress, log messages, and any loader error.

The simulation grid is shared by hover selection and gameplay. Its topology/cache
is built in `core/planet_grid.rs`; terrain textures and heightmaps are handled by
`core/planet_texture.rs`; per-cell surface and resource state is handled by
`core/planet_resources.rs`. Relief regeneration is asynchronous and cache-aware.

## Terrain and simulation details

Planet generation starts from the icosphere topology, samples terrain settings
to load or generate a heightmap, and builds the textured and relief meshes. Relief
updates are chunked and cached by relevant generation settings so changing terrain
does not block the frame loop.

Each simulation cell stores its surface class, height, slope, lithology, buildable
state, and zero or more resource deposits. Surface classes are Land, Coast, and
Water; only Land cells are currently buildable. Deposits can include Iron,
Copper, Gold, Lead, Zinc, Aluminum, Lithium, Phosphate, Stone, FreshWater, and
Salt. Mines deplete each deposit's persisted `remaining_amount`; mine expansion
can include additional zones.

Units, construction, mining, and active Base interiors are updated by the Planet
scene. Construction buildings need their required materials before progress can
advance. Simulation resources are stored in `planet_data/planet_resources.bin`,
buildings and interiors in `planet_data/planet_buildings.json`, networks in
`planet_data/planet_networks.json`, and units in `planet_data/planet_units.json`.

## Play and build

- Middle-mouse drag orbits the planet; arrow keys rotate the view; the mouse wheel
  zooms. Zooming close to the surface can enter a focused cell view.
- Left-click a cell to focus the camera on it. Clicking an existing building
  opens its information window, which provides building-specific controls,
  including **Open Base Interior** for a Base.
- Select a building tool from the build panel or use `2`-`9`: `2` Base, `3`
  Builder, `4` Housing, `5` Factory, `6` Mine, `7` Warehouse, `8` Refuel, and `9`
  Logistics. `1` clears the tool. Right-click a valid hovered cell to place the
  selected building; normal buildability and resource rules still apply.
- Press `0` for Route Line. Right-click snapped anchors to place connected route
  segments; press `T` to switch between Route and RailTrack and `Enter` to finish
  the current chain. Select another tool to leave route placement.
- With no building tool selected, right-click a building to request demolition.
  `Delete` removes the hovered building or selected/hovered network segment;
  network deletion can also be confirmed with `Backspace`.
- `Escape` saves gameplay state and returns to the main menu.

New buildings begin under construction and need their required materials before
progress advances. Mines extract available deposits over time. Hauler and shuttle
units move between configured nodes over Route networks; builder supply units
deliver construction materials. Refueling requires a completed Refuel building.
See [Planet units](planet_units.md) for unit setup and behavior.

## Persistence and runtime flow

Resource, building/network, and unit state is saved when dirty during play and
again when leaving the Planet scene. Terrain settings, simulation topology/cache,
heightmaps, and exports also live below `planet_data/`. These are working-directory
relative runtime files; do not treat generated caches as source assets.

`src/main.rs` also drains API commands each frame and dispatches them to the live
planet state. The API does not mutate the simulation on its listener thread; see
the [API request lifecycle](api.md#request-lifecycle) and [endpoint guide](api.md).


