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
  - regeneration cache and async state.

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
- When near (`NEAR_GRID_DISTANCE`), selects the cell from the global grid.
- Global grid (dual of the triangulation) cached at `planet_data/planet_hex_grid.bin`.
- Draws a single polygon (pent/hex) with an outer ring at `overlay_radius` and an inner ring at `surface_radius`.

7. Left click:
- Reorients the camera to the selected face normal.

8. UI:
- Debug controls (`draw_planet_controls`).
- Text for the hovered sector/zone name.

## 5) Key Picking/Coordinate Functions
- `project_to_screen`: world -> screen with clipping.
- `point_in_frustum`: fast culling in clip-space.
- `hovered_face`: picks the face by max dot(normal, hit direction).
- `face_name`: greek label for base faces/subfaces.


