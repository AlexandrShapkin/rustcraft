# M3 survival gameplay validation

M2 remains accepted and closed. M3 adds an authoritative Development/Survival mode split, dropped
item stacks, pickup/merge/age physics, semantic drops, 2x2 recipe matching, starter tool data,
tool durability, survival mining progress, a compact inventory screen (`E`), item presentation and
Bot observations for nearby items/mining state. The client accepts `--survival`; `just client` keeps
the development loadout and `just client-survival` starts empty.

M3.2 is reference-driven. The study and acceptance matrix are recorded in
`reference/notes/features/m3-inventory-items-mining.md`. The local Beta inventory resource and
terrain destroy-stage resources are resolved by the client presentation path.

The deterministic headless scenario is:

```text
cargo run -p rustcraft-server -- --survival
survival: drop_pickup=ok craft=log_to_planks inventory_slots=2
```

It breaks a log in Survival, simulates pickup, and crafts log into planks through the registry.
`cargo test --workspace` passes all focused inventory, entity, recipe, lighting, mesh, controller
and legacy M0/M1/M2 tests. Strict workspace Clippy and `cargo check --workspace --all-targets`
also pass.

`just bench-m3` measures the same headless simulation with 100 and 1,000 dropped stacks. It is a
debug-build baseline, not a release claim. M2's lighting baseline remains unchanged and was not
optimized in this batch.
The measured run reported `M3 items=100 tick_ms=0.9500` and
`M3 items=1000 tick_ms=36.8377`.

M3.1 repairs the three blocking interaction defects. Dropped items are now world-space crossed
textured quads with depth testing; the inventory screen is a centered 36-slot/2x2 crafting panel
with cursor-held stacks and semantic click operations; and held attack advances fixed-step mining.
The grey screen progress bar was removed. A transient ten-stage world-space crack overlay is used
instead, without rebuilding chunk meshes.

Automated capture alone did not claim interactive acceptance. The owner subsequently completed
manual M3 acceptance and confirmed the survival loop, inventory interactions, item presentation,
held mining/cracks, unbreakable behavior and input/cursor transitions. M3 is complete and closed
unless a concrete regression is reported. No M4 work was started.

M3.3 adds the exact-fidelity source contract in
`reference/notes/features/m3-exact-item-ui-rendering.md`. The implementation now resolves the
local `gui/gui.png` hotbar rectangles, uses semantic Beta container-click transactions (including
odd-stack half splitting and one-item placement), applies the researched dropped-item motion and
copy thresholds, and renders terrain destroy stages 240..249 over target block faces with the
researched blend/depth intent. Automated validation passes; interactive visual parity remains an
owner check because this surface cannot provide captures.

M3.4 extended that historical repair contract with the six Beta `RenderBlocks` face orders/UV mappings, Beta item
animation formulas in tick time plus render alpha, and local `mob/char.png` preview texture
regions. The renderer now loads non-atlas skin dimensions safely. Focused item-face, transform,
animation, GUI-scale, inventory and headless tests passed; the final result was manually accepted.

M3.5 isolated and repaired the GUI block-item projection. The prior X rotation mixed X/Y rather
than Y/Z, folding every GUI cube into a diamond. `beta_gui_project_point` now applies the
Beta-equivalent handedness and rotation order, shared by inventory/hotbar/cursor block items,
with a non-degenerate canonical-corner test.

M3.6 keeps the repair narrow: world dropped-item V coordinates are flipped exactly once at the
world upload boundary; GUI block occupancy uses the documented `0.65` slot-relative scale; and
mining now short-circuits semantic `breakable: false` blocks before progress accumulation.
Bedrock remains intact with no advancing crack state.
