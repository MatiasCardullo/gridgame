# `src/scenes/build_template.rs` - Step-by-Step Explanation

## 1) File Goal
`build_template.rs` implements a 2D scene for building internal building templates:
- Place machine blocks (belts, inserters, assemblers, etc.).
- Prototype production layouts without affecting the planet.

## 2) Current State
- Independent hex map (`MapOutline::Hexagon`).
- Tool panel to select a block type or delete.
- Rotation with `R`.
- Simple window on right click for a placed block.

## 3) High-Level `run(...)` Flow
1. Camera handling (zoom + drag).
2. Select the hex under the mouse.
3. Tool panel: choose a block type or delete.
4. Left click to place/delete.
5. Right click to open the block window.
6. Render: grid, placed blocks, preview, and hover highlight.

## 4) Roadmap
- **Future:** some buildings placed on the planet will be able to open this scene for internal customization.
- This scene will become a per-building internal layout editor (not just a sandbox).
