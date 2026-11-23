# Local REST API

GridGame starts an HTTP API with the game. It binds to `127.0.0.1:39123` only and
does not accept connections from other computers. There is no authentication; any local
process able to connect can issue gameplay commands. The server also rejects unexpected
`Host` and browser `Origin` headers; `POST` commands must use `application/json`.

All endpoints are under `/api/v1`. Requests are handled by the main game loop, which
remains the only code that mutates planet state. If the planet simulation is still loading,
`GET /status` returns `planet_ready: false`; other planet-dependent requests return `503`.

## Read state

| Method and path | Description |
| --- | --- |
| `PUT /scene/planet` | Enter the Planet scene without using the UI; then poll `/status` until `planet_ready` is `true`. |
| `GET /status` | Readiness and planet/building/unit counts. |
| `GET /cells?offset=0&limit=100` | Read paginated cells, terrain, buildability, deposits, triangle-slotted buildings, and adjacent cell IDs. `limit` is 1–250. |
| `GET /buildings` | Read buildings, triangle slots, construction progress, and stored resources. |
| `GET /networks` | Read placed Route and RailTrack segments and their endpoint geometry. |
| `GET /units` | Read planet units, route nodes, location, fuel, and cargo. |
| `GET /bases/{cell}/interior` | Inspect interior blocks, machine inventories, assembler state, arms, forklifts, and logistics requests for a Base. |
| `GET /screenshot` | Capture the currently rendered frame as a PNG image. |

Example:

```sh
curl "http://127.0.0.1:39123/api/v1/cells?offset=0&limit=25"
```

Cell IDs are zero-based simulation-grid indexes. Cell responses may include a unit-vector
`position` `[x, y, z]` for locating a cell on the planet. They also include `network_anchors`
with positions and stable identities for the cell center, polygon corners, and side midpoints.
Each buildable cell has five triangle slots on a pentagon or six on a hexagon. Slot indexes
are zero-based and follow the cell's radial sector order.

Save a screenshot of the current frame:

```sh
curl "http://127.0.0.1:39123/api/v1/screenshot" --output screenshot.png
```

## Change the planet

Place a building:

```powershell
curl.exe -X POST "http://127.0.0.1:39123/api/v1/buildings" `
  -H "Content-Type: application/json" `
  -d '{"cell_index":123,"triangle_index":2,"kind":"mine"}'
```

Supported structure `kind` values: `base`, `builder`, `housing`, `factory`, `mine`,
`warehouse`, `refuel`, and `logistics`. `triangle_index` must be a free slot in the cell;
mines also require an available deposit. Omit `triangle_index` to use slot zero.

Demolish a specific building (this also removes units that reference its cell if no other
building remains there):

```powershell
curl.exe -X DELETE "http://127.0.0.1:39123/api/v1/buildings/123/2"
```

Route is a chained line tool, not a building or CAD object. In the planet UI, select
**Route Line** (or press `0`), right-click successive anchors to draw connected segments. Each
click snaps to the nearest cell center, polygon corner, or shared-side midpoint. Press Enter or
select another tool in the menu to finish. Press `T` to switch between Route and RailTrack.

The API can place a segment between any two snapped anchors:

```powershell
curl.exe -X POST "http://127.0.0.1:39123/api/v1/networks" `
  -H "Content-Type: application/json" `
  -d '{"kind":"route","placement":{"anchor_segment":{"start":{"cell_center":{"cell_index":123}},"end":{"side_midpoint":{"first_face":34,"second_face":57}}}}}'
```

`cell_corner` anchors use a `face_index`; `side_midpoint` anchors use the two adjacent
`face` indexes listed in that cell's `network_anchors`. The older `shared_side`,
`midpoint_segment`, and `radial_span` placement forms remain accepted for compatibility.
Use `kind:"rail_track"` to draw a RailTrack line; it remains placement-only and has no train
behavior. Remove a network segment by its returned ID:

```powershell
curl.exe -X DELETE "http://127.0.0.1:39123/api/v1/networks/7"
```

Create a unit:

```powershell
curl.exe -X POST "http://127.0.0.1:39123/api/v1/units" `
  -H "Content-Type: application/json" `
  -d '{"depot":10,"preset":"hauler","nodes":{"pickup":11,"dropoff":12,"refuel":13}}'
```

`hauler` and `shuttle` require a Logistics depot and `pickup`/`dropoff` building nodes.
`builder_supply` requires a Builder depot, `pickup`/`construction` nodes, and an available
completed Builder Tender kit produced by an assembler in a base interior. The kit is consumed
only when unit creation succeeds. Optional `refuel` nodes must be Refuel buildings. Every
supplied node must refer to a cell containing a building. Unit depots and nodes remain
cell-addressed; when a cell contains multiple structures, cell-based unit interactions use
the lowest triangle-index non-network structure. Slot-specific unit-node targeting is not
implemented yet. Unit behavior and resource deposits otherwise remain under the existing
simulation.

Logistics units are route-bound: connect the depot, pickup, dropoff, and optional refuel
buildings with connected `route` network segments so the pathfinder can reach each node.

## Control a base interior

Interior coordinates use axial `{ "q": ..., "r": ... }` hexes. Block placement, rotation,
and removal use the normal interior bounds and machine simulation; these endpoints do not
edit inventories directly.

| Method and path | Description |
| --- | --- |
| `POST /bases/{cell}/interior/blocks` | Place an interior block. Body: `{"hex":{"q":0,"r":0},"kind":"assembler","rotation":0}`. |
| `PATCH /bases/{cell}/interior/blocks/{q}/{r}` | Set a block rotation. Body: `{"rotation":2}`. |
| `DELETE /bases/{cell}/interior/blocks/{q}/{r}` | Remove a block. |
| `PUT /bases/{cell}/interior/assemblers/{q}/{r}/recipe` | Set a recipe using `builder_tender`, `cargo_hauler`, or `site_shuttle`. |
| `PUT /bases/{cell}/interior/arms/{q}/{r}` | Configure a mechanic arm's inputs, outputs, and part filters. |

Arm configuration example:

```powershell
curl.exe -X PUT "http://127.0.0.1:39123/api/v1/bases/10/interior/arms/1/0" `
  -H "Content-Type: application/json" `
  -d '{"input_mode":"explicit_inputs","input_hexes":[{"q":0,"r":0}],"output_hexes":[{"q":2,"r":0}],"filters":["chassis_frame"]}'
```

`input_mode` is `any_neighbor` (the default) or `explicit_inputs`. Input and output hexes
must be unique, occupied neighbors of the arm. Filters accept component parts such as
`chassis_frame`, `wheel_assembly`, and `engine_core`; completed vehicle kits are outputs,
not arm filter parts. Use `GET /bases/{cell}/interior` to inspect block placement, container
contents, machine inputs and outputs, and current arm/forklift activity.

Legacy building snapshots are not migrated to triangle slots. When opening a planet with
an older building snapshot, recreate its building placements. Unit and resource snapshots
remain in their existing formats.

## Responses and errors

Successful requests return JSON. Errors have this shape:

```json
{
  "error": {
    "code": "building_not_allowed",
    "message": "cell is occupied, not buildable, or has no deposit for a mine"
  }
}
```

Malformed input returns `400`, unsupported routes `404`, invalid game actions `409`,
unsupported POST content types `415`, oversized request bodies `413`, and unavailable or
loading game state `503`. Cell-list responses are paginated and request bodies are limited
to 16 KiB.
