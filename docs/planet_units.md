# Planet Units

## Overview

Planet units use a declarative runtime in `src/core/planet_units.rs` and a higher-level unit catalog in `src/core/planet_unit_catalog.rs`.

- A unit owns:
  - a `preset_id`
  - a `definition_id`
  - a `behavior` with prioritized rules
  - node bindings like `pickup`, `dropoff`, `refuel`, and `construction`
  - runtime state like `path`, `current_action`, `current_target`, `cargo`, and `fuel`
- Each tick the runtime:
  - evaluates building construction progress
  - evaluates unit rules from highest to lowest priority
  - resolves the current target node
  - replans a path when the target/action changes
  - moves the unit and spends fuel
  - executes `pickup`, `deliver`, `build`, or `refuel` on arrival

## Runtime Presets vs Catalog Definitions

- `PlanetUnitPresetId` is the low-level runtime behavior choice.
- `PlanetUnitDefinitionId` is the higher-level catalog entry shown to the player.
- A definition can provide:
  - label shown in UI
  - category
  - unit kit item id
  - required parts/subassemblies
  - supported attachments
  - optional mapping to a runtime preset
- Current runtime-spawnable definitions map onto existing presets:
  - `Cargo Hauler` -> `Hauler`
  - `Site Shuttle` -> `Shuttle`
  - `Builder Tender` -> `BuilderSupply`
- Catalog-only definitions already exist for future expansion, even if they are not yet spawnable:
  - `Modular Excavator`
  - `Compact Utility Loader`
  - `Drill Rig`

## Categories

Current unit catalog categories:

- `CargoLogistics`
  - route cargo and shuttle style units
- `LightUtility`
  - builder/support machines and compact utility platforms
- `ExcavationModular`
  - modular excavator-family machines
- `Drilling`
  - dedicated drilling platforms

## Parts and Attachments

Unit definitions now declare required part items as coarse subassemblies instead of fine-grained components.

Current part items include:

- `Chassis Frame`
- `Engine Core`
- `Control Module`
- `Fuel Assembly`
- `Primary Tool Mount`
- `Cargo Module`
- `Construction Rig`
- `Drill Head`

Attachments are modeled in the catalog but are not active gameplay equipment yet.

- Excavation modular units support:
  - `Bucket`
  - `Hydraulic Hammer`
  - `Grapple`
  - `Auger`
  - `Ripper`
- Light utility units support:
  - `Bucket`
  - `Saw`
  - `Drill`
  - `Blade`
  - `Mixer`
- Cargo/logistics and drilling units are currently treated as fixed-tool platforms in the catalog.

## Presets

Current presets:

- `Hauler`
  - refuels when fuel is below 25%
  - picks up when empty
  - delivers when carrying cargo
- `BuilderSupply`
  - refuels when fuel is below 25%
  - picks up exact construction requirements from the chosen source
  - goes to the construction target and pays the build
  - builder-owned units poll for new work every 1 second while resting at the depot
  - builder-owned units reserve distinct build targets; if no free target exists they rest
  - base-owned units return to depot and despawn when the target is already paid
- `Shuttle`
  - refuels when fuel is below 25%
  - travels between pickup and dropoff style nodes

## Building Window Controls

### Base

The base window has `Auto builder units` and runtime unit creation for the logistics category.

- Enabled: the base may auto-spawn a single `BuilderSupply` for pending construction
- Disabled: no automatic construction unit is spawned from base
- Manual logistics creation currently exposes:
  - `Cargo Hauler`
  - `Site Shuttle`
- The UI also shows each unit definition's category and required parts.

### Builder

The builder window has three auto-selection toggles plus manual creation for the `Builder Tender` definition:

- `Auto source`
- `Auto build target`
- `Auto refuel`

When enabled, the window can auto-fill the corresponding node when creating a new builder unit:

- `Auto build target`: nearest unpaid construction target
- `Auto source`: nearest valid source with all materials needed by the selected build target
- `Auto refuel`: nearest completed `Refuel` building

Manual node buttons still work when the matching auto toggle is off.

The builder window also shows:

- unit category
- required parts
- compatible attachments for the spawned definition

## Persistence

Planet unit snapshots are saved to `planet_data/planet_units.json`.

- The current format stores preset, definition id, behavior, node bindings, path, cargo, fuel, and current action/target
- Old unit snapshot formats are not migrated

Building-level unit control flags are saved in `planet_data/planet_buildings.json`.

## Current Constraints

- Automatic construction from base is limited to one active base unit at a time
- Manual builder units can be multiple
- Construction delivery is atomic: builder units only take and spend the full material set required by the target build
- Builder/logistics units are route-bound; only base auto-builder units may use direct off-road travel
- Refuel is only possible at completed `Refuel` buildings
