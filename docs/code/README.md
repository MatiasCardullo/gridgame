# Code and workflow documentation

This folder documents how to run, change, and verify GridGame, plus the workflows
implemented by its scenes and local API. The separate gameplay guides
cover broader game and world-building material; they remain outside this code reference.

## Local developer workflow

Run commands from the repository root. Asset lookup and runtime persistence use
working-directory-relative paths.

```powershell
cargo run
cargo build
cargo test
cargo fmt --check
cargo clippy --all-targets
```

`r.bat` is a convenience wrapper for `cargo run`. To run a focused test, pass its
name or module path, for example:

```powershell
cargo test parses_cell_query_with_bounded_pagination
cargo test core::api::tests
```

Tests live beside the implementation under `#[cfg(test)]`, with additional
integration tests under `tests/`. Make the smallest relevant change, run the
focused tests first, then formatting and broader checks appropriate to the change.

## Runtime and data workflow

`src/main.rs` owns the frame loop, scene routing, shared runtime state, and API
channel. `PlanetLoader` builds planet data asynchronously; the frame loop applies
loader results and all live planet mutations on the main thread. Scene functions
compose shared simulation and UI helpers rather than owning separate runtime loops.

Gameplay and generated files are written relative to the repository root:

- `config.json` stores runtime configuration.
- `map.json` stores the legacy map state used by the main menu.
- `planet_data/` contains terrain parameters and caches, simulation resources,
  buildings, networks, units, and optional exports.
- `assets/objects/` contains tracked GLB models. CAD face groups, names, and UCS
  metadata live in the adjacent `.cad.json` sidecar.

Treat caches and generated files as runtime data, not source. When changing a
persisted format or generation inputs, update the corresponding loading, save,
cache-key, and compatibility paths together.

## End-to-end workflows

| Workflow | Start here |
| --- | --- |
| Generate/load a planet, place buildings, mine, and save gameplay state | [Planet scene](planet.md) |
| Configure logistics and builder units | [Planet units](planet_units.md) |
| Open a Base interior, place machines, and tune arms | [Build Template](build_template.md) |
| Browse and edit GLB assets and metadata | [CAD editor](cad_editor.md) |
| Observe or control the running game over HTTP | [Local API](api.md) |

## Settings workflow

From the main menu, open **Configuracion** to adjust zoom, text and line sizing,
legacy tile-generation parameters, and UI color channels. Settings and color
changes are saved to `config.json` as they are made. **Regenerar tiles** rebuilds
the legacy tile map using the current settings; the changed map is saved when
exiting the application.

The local API is part of the running game, not a standalone server. Keep command
parsing in `core/api.rs`, route commands through its channel, and apply mutable
planet commands in the main loop. The
[API guide](api.md#request-lifecycle) describes that path.

## Code map

- `src/main.rs`: application loop, scene enum, loader application, and API dispatch.
- `src/scenes/`: scene-specific UI and coordination.
- `src/scenes/planet.rs` and `src/scenes/planet/`: planet scene facade plus loader,
  persistence, and UI modules.
- `src/scenes/cad.rs` and `src/scenes/cad/`: CAD scene facade plus transform and UI
  modules.
- `src/core/`: shared simulation, planet topology/resources, persistence, object
  assets, API, and reusable UI.
- `src/core/ui.rs` and `src/core/ui/`: stable shared UI facade and its components.
- `tests/`: integration tests.

See the repository [README](../../README.md) for a quick project overview and
the [gameplay documentation](../gameplay/starting-game-step-by-step.md) for the
separate gameplay guide collection.
