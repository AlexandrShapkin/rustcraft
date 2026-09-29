# M3.8 block-presentation repair validation

Status: complete. The owner manually accepted M3 after this automated repair/validation record.
M4 was not started.

## Root cause and repair

The malformed bookshelf side was the `-Z` face in the separate dropped-item face table. Its
model-space perimeter and triangle indices formed a rectangle, but its lower UV corners were
assigned to the opposite vertices. The two texture triangles therefore had opposite signed UV
areas and folded the atlas tile across the diagonal.

M3.8 removes the independent dropped-item topology. `geometry::FACES` is now the one source for
full-cube positions, outward winding, indices, normals and normalized UV corners. World meshes,
dropped blocks, GUI block items and destroy-overlay positions consume that table. Texture atlas
conversion occurs only in `tile_uv`; dropped and GUI paths no longer apply contextual UV flips.

The GUI issue was separate. Its old path emitted only four faces, discarded depth, derived UVs
from projected screen distances and omitted the final Beta `-90°` Y rotation. The replacement
uses all six canonical faces, the source-derived effective transform, native ten-pixel scale,
deterministic GUI lighting, back-face culling and a depth-cleared GUI item pass. Hotbar,
inventory and cursor stacks call the same resolved `BlockModel` renderer.

## Deterministic render evidence

`just render-test-all`, `just fidelity-m3` and the targeted diagnostic commands left 103 local
PNGs and 101 text manifests under the ignored `target/render-tests/` directory on the llvmpipe
OpenGL adapter. This count includes the intentionally retained pre-repair images and targeted
wireframe/state runs; `render-test-all` does not delete earlier evidence. The offscreen path uses
a dedicated readable RGBA8-sRGB texture and does not require a window surface.

Inspected evidence includes:

- all six isolated faces in solid, four-corner, generated UV-chart and real-atlas modes;
- front and rear bookshelf, log and grass world/dropped views;
- enlarged and native-scale GUI, hotbar, inventory and cursor item views;
- a labeled item grid showing bookshelf, log, grass, stone, cobblestone and planks;
- a wireframe dropped-bookshelf view with triangle edges, vertex marks, normal arrows and face
  labels;
- a semantic orientation example (`cube-log --axis x`).

The generated `-Z` UV chart is a continuous upright chart after repair; the saved pre-repair
chart folds at the triangle diagonal. Bookshelf faces are rectangular in front and rear views.
Log end grain remains on top/bottom and bark on horizontal sides. Grass uses grass top, dirt
bottom and grass-side horizontal materials. Re-rendering `gui-bookshelf-atlas.png` produced the
same SHA-256, confirming deterministic output for that scene.

These captures were implementation evidence rather than a substitute for interactive review. The
owner subsequently completed and accepted that review. They are not a claim of pixel identity with
copyrighted reference screenshots.

## Block-state orientation foundation

The existing eight-byte `BlockState` remains `BlockId + u16 variant`. Typed accessors pack
`Facing`, `Axis` and `HorizontalRotation` without allocation. Content definitions declare one
orientation property and a base model rotation. Presentation composes `base * state`, rotates
positions/normals around the block center and keeps UVs attached to vertices. State-dependent
texture selection remains a separate resolver hook. First-party blocks still use the default
identity/none configuration; no directional-block gameplay was added.

## Validation performed

- `just bootstrap-check`: pass;
- `just ci`: pass, including format check, workspace/all-target check, workspace tests and strict
  Clippy with warnings denied;
- renderer/client focused suite: 30 renderer tests and 20 client tests pass within the workspace
  suite;
- `just smoke`: pass (`break/place=ok`);
- `just survival-scenario`: pass (`drop_pickup=ok`, log-to-planks craft, inventory result);
- `just render-test-all`: pass;
- `just fidelity-m3`: pass;
- `timeout 5s just client-diag`: client and diagnostic scene initialized correctly; terminated by
  the intentional timeout because the interactive event loop remains open;
- `timeout 12s just client-survival`: real client initialized the local terrain, GUI resources and
  meshes correctly; terminated by the intentional timeout.

The software GL environment logs failed Vulkan/Zink probing before selecting llvmpipe. This does
not affect the successful OpenGL rendering/readback path.

## Completed owner acceptance

The owner completed these checks:

1. Ran `just fidelity-m3` and inspected the labeled GUI grid and bookshelf/log/grass captures in
   `target/render-tests/`.
2. Ran `just client-survival` and verified bookshelf in inventory, hotbar and cursor-held contexts.
3. Dropped a bookshelf and walked around it: every visible side remained rectangular and upright
   while the accepted bob/rotation animation continued.
4. Verified log end grain/bark and grass top/side/bottom in GUI and dropped contexts.
5. Confirmed mining cracks, bedrock, stack transactions and dropped-item timing remained unchanged.

No automated or manually observed blocker remains. M3 is complete and closed unless a concrete
regression is reported.
