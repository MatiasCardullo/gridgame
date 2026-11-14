# `src/scenes/build_template.rs` - Current Behavior

## Goal
3D base-interior editor scene for machine layout, logistics tuning, and quick sandbox tests.

## Controls
- Camera: `RMB` look, `WASD` move, `Shift` sprint, `Space/Ctrl` vertical.
- Build: select panel item, `LMB` place, `R` rotate.
- Delete: select `Delete`, `LMB` occupied hex.
- Block detail: `RMB` on block.
- Pick modes: `Enter` finish, `Esc` cancel.

## Runtime Systems In Scene
- Hex floor + occupancy highlight.
- Segmented mechanic-arm model with articulated links and a claw.
- Conveyor, mechanic arm, assembler, container visuals.
- Belt item interpolation and stop-at-end behavior.
- Forklift body centered on movement path, carried payload offset to fork arms.
- First-person collision against occupied block hexes and forklift hexes.

## Notes
- Interior simulation tick runs each frame via planet state active-base context.
- Scene exits to `Planet` on `Esc` when no selection/pick mode active.
