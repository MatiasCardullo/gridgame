# `src/scenes/planet.rs` - Step-by-Step Explanation

## 1) File Goal
`planet.rs` implements the 3D planet scene:
- Planet mesh loading/generation (texture + relief).
- Orbital camera with zoom (free rotation).
- Dynamic LOD/subdivisions based on distance.
- Async mesh/texture regeneration with caching.
- Hover selection via raycast and line/grid overlays on the sphere.

## 2) Main Building Blocks

### Configuration and Core Utilities
- Paths and cache constants (`PLANET_DATA_DIR`, `PLANET_NOISE_PATH`, `HEIGHTMAP_SIZE`).
- Render/behavior constants (`SUBFACE_HOVER_MAX_DISTANCE`, `PLANET_OVERLAY_OFFSET`, etc.).
- Math/helpers:
  - `wrap_angle`
  - Icosahedron geometry + subdivisions live in `src/core/planet_grid.rs`
  - Planet surface/resource simulation lives in `src/core/planet_resources.rs`
  - Procedural noise + heightmap live in `src/core/planet_texture.rs`

### Noise Configuration/Data
- `PlanetNoiseConfig`: noise/biome/relief parameters.
- `PlanetNoiseSnapshot`: serialized config + heights.

### Build/Texture Payloads
- `PlanetBuildData`: full result (relief meshes + texture + normals + values).
- `PlanetTextureData`: partial payload to show the texture before final relief is ready.

### Runtime State
- `PlanetState` contains:
  - camera (quaternion rotation + distance/targets),
  - meshes (`base_texture_meshes`, `relief_meshes`, fallback),
  - topology (`base_vertices`, `sector_vertices`, `sector_faces`),
  - a single planet grid (`freq=160` default) used for hover + gameplay simulation,
  - per-cell simulation world (surface class + multi-deposit resource state),
  - regeneration cache and async state.
- Persisted gameplay data:
  - buildings: `planet_data/planet_buildings.json`
  - units: `planet_data/planet_units.json`

### Async Loader
- `PlanetLoader` starts a thread that:
  1. builds/loads base geometry,
  2. loads or generates the heightmap,
  3. sends `TextureReady`,
  4. builds final relief,
  5. sends `Done`.
- Progress is reported via events (`PlanetLoadEvent`).

### Incremental Background Regeneration
- In `PlanetState::request_regen/start_regen/poll_regen`:
  - work is split into chunks (`REGEN_FACE_GRAIN`),
  - cached by `ReliefCacheKey`,
  - texture is refreshed when relevant params change,
  - results are applied chunk-by-chunk without freezing frames.

## 3) Planet Visual Generation Pipeline

1. Base topology:
- An icosahedron is built, oriented, and subdivided (see `src/core/planet_grid.rs`).

2. Height/color sampling:
- `height_value` uses fBm and latitude bias.
- `color_from_height` decides sea/land/ice.

3. Mesh building:
- `build_planet_chunk_data_for_faces` creates vertices/indices per face.
- In relief mode, triangles below sea level are discarded.
- In texture mode, equirectangular UVs are used.

4. Heightmap cache:
- `load_or_build_heightmap` tries to load a PNG cached by parameter hash.
- If missing, pixels are generated and exported as PNG.

## 4) `run(...)` Flow (Frame Loop)

1. Input/camera:
- `Esc` returns to the menu.
- Middle-mouse drag rotates (quaternion with local axes).
- Arrow keys rotate the camera (quaternion with local axes).
- Mouse wheel adjusts `target_distance`.
- Smooth interpolation toward targets.

2. LOD:
- Distance -> subdivisions (`zoom_to_subdivisions_hysteresis`).
- If it changes and relief is enabled, triggers async regeneration.

3. Mesh rendering:
- Draws base texture.
- If relief is enabled, draws relief (fallback during animation/regeneration).

4. Icosahedron line overlay:
- Uses `line_radius = radius + PLANET_OVERLAY_OFFSET`.
- Draws great-circle arcs for edges at that radius.

5. Hover:
- `ray_from_mouse` + `ray_sphere_intersection`.
- Selects `hovered_subface` or `hovered_base_face`.
- Draws a highlighted face outline.

6. Hover grid/cell:
- Selects the cell from the same grid used by simulation.
- Grid cache (dual of the triangulation) at `planet_data/planet_sim_grid.bin`.
- Draws a single polygon (pent/hex) with an outer ring at `overlay_radius` and an inner ring at `surface_radius`.

7. Gameplay cell simulation:
- Uses the same grid cache (`planet_data/planet_sim_grid.bin`) for gameplay state.
- Builds or loads per-cell simulation snapshot (`planet_data/planet_resources.bin`).
- Classifies cells as `Land`, `Coast`, or `Water` from noise/sea-level, then derives lithology and deposits.
- Resource generation now supports multiple deposits per cell:
  - Minerals (`Iron`, `Copper`, `Gold`, `Lead`, `Zinc`, `Aluminum`, `Lithium`, `Phosphate`) each use an independent seeded noise map layered on top of geologic suitability.
  - `Stone`, `Salt`, and `FreshWater` remain more tightly coupled to terrain/surface rules instead of mineral noise.
  - Empty cells are valid.
  - Cells with several deposits are valid.
  - `FreshWater` is never generated for `Water` cells.
- Buildability currently allows only `Land`.
- Construction economy mirrors the legacy 2D sector flow:
  - New buildings start unpaid/under construction.
  - Materials are required before build progress advances.
  - Base auto-builder units are limited to one active unit at a time.
  - Builder-owned construction units can be multiple, but each reserves a distinct build target.
- Declarative unit runtime:
  - Units run preset-driven rules (`Hauler`, `BuilderSupply`, `Shuttle`).
  - Logistics/builder units only move on `Route` networks.
  - Base construction units may fall back to direct off-road travel.
  - Units consume fuel while moving and can only refuel at completed `Refuel` buildings.

8. Left click:
- Reorients the camera to the selected face normal.
- If a node pick is active (`Pickup`, `Dropoff`, `Refuel`, `Construction`), left click selects the target cell.

9. UI:
- Debug controls (`draw_planet_controls`).
- Text for the hovered sector/zone name plus hovered sim-cell info (surface/buildability/deposit).
- Hover/building UI now lists all deposits found in a sim cell instead of only one.
- Build/mining controls:
  - `1`: no tool
  - `2`: `Base`
  - `3`: `Mine`
  - Right click on hovered sim cell: place selected building (validated by `can_build`)
  - `Delete`: remove building on hovered sim cell
  - `M`: export hovered base-face zone PNG to `planet_data/zone_maps/`
  - `Shift+M`: export PNGs for all base-face zones
  - Mines extract continuously from cell deposits and reduce persisted `remaining_amount`.
  - If a mine covers a multi-resource cell, extraction is mixed across the active deposits in that cell.
  - Expanded mine zones aggregate totals across all deposits from all covered cells.
- Building windows now include:
  - Construction status + requirements.
  - Base toggle for automatic builder unit spawning.
  - Logistics node picking (`Pickup` / `Dropoff` / `Refuel`) and preset unit creation.
  - Builder node picking (`Source` / `Build` / `Refuel`), auto-selection toggles, and unit list status.
  - Mine expansion and multi-cell extraction status.

## 5) Key Picking/Coordinate Functions
- `project_to_screen`: world -> screen with clipping.
- `point_in_frustum`: fast culling in clip-space.
- `hovered_face`: picks the face by max dot(normal, hit direction).
- `face_name`: greek label for base faces/subfaces.

## Current Notes
- Mining expansion (`mine_extra`) and multi-cell extraction are implemented.
- Logistics routes and flow overlays are drawn on the sphere for selected buildings.
- `planet_data/planet_units.json` now stores the new declarative unit snapshot format; old unit snapshot formats are not migrated.


