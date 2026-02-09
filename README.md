# GridGame

Hex-grid strategy game built with Rust and Macroquad. Build structures, extract resources, and move logistics units along routes.

**Requirements**
1. Rust and Cargo installed.

**Run**
1. `cargo run`

**Development**
1. `cargo check` for fast type checks.
2. `cargo build` for a debug build.
3. `cargo test` to run tests.
4. `scripts/check.ps1` to run the standard check script.

**Controls**
1. Left click: open a block window.
2. Right click: place a block or start demolish (when "Demolish" is selected).
3. Mouse wheel: zoom.
4. Middle mouse (drag): pan camera.
5. `R`: rotate placement.
6. `Esc`: back to menu (saves the game).

**Notes**
1. Mines can expand their attached zones from the block window.
2. Logistics units are created from the Logistics block window and can be listed with "Unit list".
