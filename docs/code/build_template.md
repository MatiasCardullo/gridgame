# Base interior and Build Template workflow

The Build Template scene is a 3D editor and live sandbox for a Base's machine
layout. The same `BaseInteriorState` simulation is used in the Planet scene and
here; edits update the selected base rather than a separate template file.

## Open and edit an interior

1. Enter the Planet scene and select a Base.
2. Open its information window and choose **Open Base Interior**.
3. Select a machine block in the build panel. Left-click an empty in-bounds hex
   to place it. Press `R` to rotate the block under the cursor, or rotate the
   next placement when the cursor is over an empty hex.
4. Right-click a placed block to inspect its state and available controls.
   Mechanic-arm settings can start input/output pick mode; left-click neighboring
   hexes to toggle them, `Enter` to finish, or `Escape` to cancel.
5. To remove a block, clear the selected block type and left-click an occupied
   hex; confirm the removal in the prompt.
6. Press `Escape` to leave the scene when no pick mode or block selection is
   active. The scene returns to the Planet.

The editor prevents placement on occupied hexes and limits arm input/output picks
to the supported neighboring cells. Changed interiors are marked dirty and saved
with the planet building state.

## Navigation

- Hold right mouse and move the mouse to look around; UI panels capture the pointer.
- `WASD` moves horizontally, `Space` moves up, and `Ctrl` moves down.
- Hold `Shift` to move faster.

## What the sandbox runs

The scene renders the hex floor, placement preview, machine blocks, conveyors,
mechanic arms, containers, assemblers, furnaces, and forklifts. The shared interior
simulation continues to tick, so item movement, machine activity, and arm/forklift
behavior can be inspected while arranging blocks. Camera collision accounts for
occupied machine and forklift hexes.

See [Planet units](planet_units.md) for the production and consumption of vehicle
kits, and the [API guide](api.md#control-a-base-interior) for programmatic interior
inspection and block/arm configuration.
