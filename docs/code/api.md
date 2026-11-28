# Local REST API

GridGame starts an HTTP API with the game. It binds to `127.0.0.1:39123` only and
does not accept connections from other computers. There is no authentication; any local
process able to connect can issue gameplay commands. The server also rejects unexpected
`Host` and browser `Origin` headers; `POST` commands must use `application/json`.

All endpoints are under `/api/v1`. Requests are handled by the main game loop, which
remains the only code that mutates planet state. If the planet simulation is still loading,
`GET /status` returns `planet_ready: false`; other planet-dependent requests return `503`.

## Request lifecycle

The API listener parses and validates an HTTP request into an `ApiCommand`, then sends
it to the running game's main loop over a channel. The loop dispatches CAD commands to
the CAD scene state and planet commands to the live `PlanetState`; this keeps simulation
mutation on the same thread that updates and renders the world. The listener waits for
the corresponding reply and serializes it as JSON or PNG.

`PUT /scene/planet` changes the active scene but does not wait for planet generation.
Poll `/status` until `planet_ready` is `true` before issuing planet-dependent commands.
The screenshot route is also completed by the frame loop so it captures a rendered frame.

## Read state

| Method and path | Description |
| --- | --- |
| `PUT /scene/planet` | Enter the Planet scene without using the UI; then poll `/status` until `planet_ready` is `true`. |
| `GET /status` | Readiness and planet/building/unit counts. |
| `GET /cells?offset=0&limit=100` | Read paginated cells, terrain, buildability, deposits, building placement, and adjacent cell IDs. `limit` is 1–250. |
| `GET /buildings` | Read buildings, construction progress, and stored resources. |
| `GET /units` | Read planet units, route nodes, location, fuel, and cargo. |
| `GET /bases/{cell}/interior` | Inspect interior blocks, machine inventories, assembler state, arms, forklifts, and logistics requests for a Base. |
| `GET /screenshot` | Capture the currently rendered frame as a PNG image. |

Example:

```sh
curl "http://127.0.0.1:39123/api/v1/cells?offset=0&limit=25"
```

Cell IDs are zero-based simulation-grid indexes. Cell responses may include a unit-vector
`position` `[x, y, z]` for locating a cell on the planet.

Save a screenshot of the current frame:

```sh
curl "http://127.0.0.1:39123/api/v1/screenshot" --output screenshot.png
```

## Change the planet

Place a building:

```powershell
curl.exe -X POST "http://127.0.0.1:39123/api/v1/buildings" `
  -H "Content-Type: application/json" `
  -d '{"cell_index":123,"kind":"mine"}'
```

Supported `kind` values: `base`, `builder`, `housing`, `factory`, `mine`, `warehouse`,
`refuel`, `logistics`, and `route`. Normal buildability rules apply; mines also require
an available deposit.

Demolish a building (this also removes units that reference that cell):

```powershell
curl.exe -X DELETE "http://127.0.0.1:39123/api/v1/buildings/123"
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
supplied node must refer to an existing building. Unit behavior and resource deposits
otherwise remain under the existing simulation.

Logistics units are route-bound: connect the depot, pickup, dropoff, and optional refuel
buildings with `route` buildings so the existing pathfinder can reach each node.

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
