# M1.1 rendering isolation

M2 is implemented and accepted. This development path leaves normal simulation/render extraction architecture
in place. It never advances automatically: select the next stage only after inspecting the current
one. `just client-diag` starts at the triangle. Escape closes the window.

## Commands and acceptance order

Run `just client-diag STAGE`, one stage at a time:

| Stage | Enabled layer | Expected image |
|---|---|---|
| `triangle` | Hard-coded 3 vertices, identity model, constant red output | Centered upright triangle, no distortion |
| `cube-no-cull` | Independent 24-vertex, 36-index cube, no backface culling | Red +Z front, yellow +Y top, upright cube |
| `cube` | Same cube, CCW front faces and backface culling restored | Same exterior image |
| `checker` | Project-owned 2x2 texture, full-face UVs | Red/green upper quadrants, blue/white lower quadrants, continuous across diagonal |
| `atlas` | One local atlas tile, (1,0), stone | One complete stone texture per face |
| `platform` | Nine hard-coded blocks, x/z=-1..1, y=0 | Horizontal textured platform below the camera |
| `chunk` | Real mesher, one manually filled section (0,0,0), 3x3 layer at local y=1 | Horizontal platform, 30 exposed quads |
| `section-zero` | Same mesher with fixed distant camera | Origin (0,0,0) |
| `section-negative` | Section y=-1, same camera | Origin (0,-16,0), geometry moves down only |
| `section-positive` | Section y=+1, same camera | Origin (0,16,0), geometry moves up only |
| `normal` | Normal extraction, multiple chunks, dirty system, player camera, unlit | Flat textured green terrain, level boundary, eye above ground |
| `normal-lit` | Normal world plus existing face brightness | Same orientation and materials |

Triangle through platform bypass runtime, generation, RenderWorld, dirty queues and the real mesher.
The chunk/section stages manually fill exactly one RenderChunk and a neighbor lookup cache; they do
not call world extraction or generation. They use one tile and unit brightness. The early scenes
do not load an external atlas at all. The color fragment entry point does not sample textures.
Lighting stays disabled through `normal`; material tint is separate from lighting.

The cube bounds are (-0.5,0,-0.5) to (0.5,1,0.5). Its face colors are +Z red, -Z cyan, +X green,
-X magenta, +Y yellow and -Y blue. The required fixed eye (0,1.6,4), target (0,0.5,0), up +Y shows
front/top; tests check both triangles of all six faces, including hidden faces. The real-mesher
camera is (1.5,3.6,7), aimed at (1.5,1.5,1.5). Section comparisons keep eye (1.5,10,48) and target
(1.5,1.5,1.5) unchanged, so they cannot conceal an origin error by following the geometry.

For a captured frame from the actual window surface:

```sh
just client-diag-capture triangle /tmp/my-triangle.png
just client-diag-capture checker /tmp/my-checker.png
```

The output path must not exist. Capture requires a surface supporting COPY_SRC and RGBA8/BGRA8;
failure is explicit. It copies the rendered surface before presentation, writes PNG, and exits.
It does not synthesize a screenshot from CPU geometry or store proprietary assets in the repo.

Atlas stages resolve the normal content path (`RUSTCRAFT_TERRAIN_TEXTURE` or
`reference/assets/terrain.png`). A missing, invalid or incorrectly sized atlas fails with an
explicit resource error and nonzero exit. Expected dimensions are 256x256: 16x16 tiles of 16px.
PNG and UV origins are both top-left; there is no Y flip. The diagnostic stone tile has UV bounds
(1/16,0) to (2/16,1/16). Sampling is nearest-neighbor, Rgba8UnormSrgb, with no grey fallback.

## Exact matrix convention

- World: +X horizontal east/west axis, +Y up, +Z horizontal depth. Screen right depends on heading.
- Right-handed view: forward maps to camera -Z, right to +X, up to +Y.
- Diagnostic look-at: F=normalize(target-eye), R=normalize(F cross world_up), U=R cross F.
- CPU `[[f32;4];4]` helpers store rows. View rows are
  `(Rx,Ry,Rz,-R dot eye)`, `(Ux,Uy,Uz,-U dot eye)`,
  `(-Fx,-Fy,-Fz,F dot eye)`, `(0,0,0,1)`.
- Projection rows: `(f/aspect,0,0,0)`, `(0,f,0,0)`,
  `(0,0,far/(near-far),far*near/(near-far))`, `(0,0,-1,0)`,
  where f=1/tan(vertical_fov/2). Near maps to NDC z=0, far to z=1; x/y clip to [-w,+w].
- FOV=70 degrees=1.2217305 radians, aspect=width/height, near=0.05, far=256.
  Zero-sized resize events preserve the previous valid surface/aspect.
- CPU composition is `projection * view * identity_model`. Transpose the row array exactly once
  to produce four contiguous f32 columns (64 bytes) for the uniform upload.
- WGSL uses `camera.matrix * vec4(position,1)` (matrix times column vector); no shader transpose.
  The vertex format is position at byte 0, UV at 12, shade at 20, color at 24, stride 36.
- This repository does **not** use glam. Its `Mat4::to_cols_array_2d()` already produces columns;
  a future glam-based implementation must not transpose that output again.

Primary convention references: [WGSL matrix types](https://www.w3.org/TR/WGSL/#matrix-types),
[WebGPU coordinate systems](https://gpuweb.github.io/gpuweb/#coordinate-systems), and
[glam column export](https://docs.rs/glam/latest/glam/f32/struct.Mat4.html#method.to_cols_array_2d).
The depth endpoints, field-of-view edges, translated target and composition are independently
checked numerically; documentation is not used as a substitute for those checks.

Gameplay yaw=0 faces +Z; the right-handed screen-right vector there is -X. Positive pitch looks
down. Positive controller look deltas turn right/down. Movement and camera use the same right
direction, with no roll and unit-length orthogonal basis vectors even at nonzero pitch.

## Findings and fixes

1. **Earliest failure: camera/matrix layer, before any geometry or material dependency.** The new
   translated-camera regression was run against the previous code before the fix. For eye
   (0,1.6,4), target (0,0.5,0), it returned clip
   `[0, 0.6638689, -0.9001811, -0.006630207]`. Negative w incorrectly put a point in front of
   the camera behind it. The earlier origin-only test missed this.
   Projection coefficients were arranged as columns, view mixed basis rows with translation in
   the last outer array, and row-major multiplication combined them. The boundary transpose could
   not repair that mixture. Fix: consistent row constructors, row multiplication, one column export.
2. **Independent camera-basis defect:** cross(world_up,forward) had the opposite handedness from
   the RH projection; neither right nor up was normalized at nonzero pitch. Fix: normalized
   cross(forward,world_up), then cross(right,forward). Align strafe and mouse turn signs with it.
3. **Independent material defect, isolated at normal-world stage:** the grass-top atlas tile is
   grayscale and lacked its material tint. Stone and checker sampling already passed. Fix: a
   first-party fixed grass-top tint, converted from sRGB to linear, carried separately from shade.
   See `reference/notes/grass-material.md`. No biome system or M2 lighting was added.
4. **Shutdown defect exposed by capture:** completed captures initially exited with SIGSEGV.
   GDB traced teardown into libwayland-client/libEGL_mesa after `run_app` consumed the event loop.
   Fix: release renderer/surface and window in `ApplicationHandler::exiting`, while the display
   connection still exists. Subsequent checker/atlas/platform/chunk/section/normal runs exited 0.

No index-buffer, section-origin, atlas-binding or quad-UV defect was reproduced. Their existing
behavior was retained and covered with stronger tests. PNG handling now explicitly expands palette
input/strips 16-bit channels, validates atlas dimensions, and logs path/dimensions/format once.

## Evidence from this run

Captured on AMD Radeon Vega 8 Graphics, RADV RAVEN, Vulkan, 1280x720. The agent opened and inspected
each capture before advancing to the next layer. PNGs are local temporary evidence, not committed:

| Stage | Capture under `/tmp/` | Result |
|---|---|---|
| Triangle | `rustcraft-diag-triangle.png` | Centered upright triangle |
| Cube without/with culling | `rustcraft-diag-cube-no-cull.png`, `rustcraft-diag-cube.png` | Same upright red/yellow exterior |
| Checker | `rustcraft-diag-checker.png` | Clean quadrants, no diagonal UV split |
| Atlas | `rustcraft-diag-atlas.png` | Visible stone texture |
| Platform | `rustcraft-diag-platform.png` | Horizontal, repeating textures |
| One real chunk | `rustcraft-diag-chunk.png` | Horizontal, 120 vertices/180 indices |
| Section origins | `rustcraft-diag-section-{zero,negative,positive}.png` | Center/down/up, fixed camera |
| Normal before tint | `rustcraft-diag-normal.png` | Orientation correct; grayscale grass exposes missing tint |
| Normal unlit after tint | `rustcraft-diag-normal-tinted.png` | Green textured terrain, horizontal far boundary |
| Normal with face brightness | `rustcraft-diag-normal-lit.png` | Same correct terrain orientation/material |

Normal extraction uploaded 18 sections, 10,824 vertices and 16,236 indices. Spawn logs:
feet=(0.5,2,0.5), eye=(0.5,3.62,0.5), yaw=pitch=0, forward=(0,0,1), right=(-1,0,0), up=(0,1,0).
All pairwise basis dot products were 0; all lengths were 1. The feet start above the y=0 surface;
normal gravity settles the player. The world is finite, so its visible far edge is not an infinite
terrain horizon. No horizon/fog effects were added to disguise orientation.

`just render-test` covers known look-at, translated gameplay camera, projection/depth/FOV/aspect/
resize, composition/no-roll, GPU byte layout, quad/cube indices and both windings, UV order and
continuity, atlas bounds, platform face isolation, signed origins, gameplay control alignment,
grass material mapping and dirty extraction/remeshing. Existing spawn/collision tests remain.

Final checks passed: `just render-test`, `just bootstrap-check` (33 workspace unit tests),
`just ci`, `just smoke`, and `git diff --check`. The bounded audit also reran every diagnostic
stage, verified 17 render tests and 4 client tests, repeated normal-lit startup/teardown cycles,
and covered focus/opposite-key input paths. No new dependencies were added.

The images prove these captured frames on this adapter. They do not constitute a portability claim
for other GPUs. The owner has separately completed interactive movement/look/break/place and camera
acceptance. M1.1 is accepted and ready for M2; this audit did not start M2.

## M2 regression pass

M1.1 remains closed. M2 preserved all twelve permanent diagnostic stages and their original
geometry/terrain composition. Hardware-backed captures for every stage were inspected again on
RADV RAVEN/Vulkan; each completed cleanly. Normal client composition additionally includes M2's
authored building area, inventory HUD, lighting and telemetry. See `M2_VALIDATION.md` for M2-only
measurements, capture evidence and the outstanding manual M2 acceptance distinction.

## BG1 static geometry inspection

`just sample-game` uses the independent project's generated texture packages and public compiled
model resolver. Its proof content includes boxes, box compounds, a 45-degree wedge, an indexed
static tetrahedral mesh and real half/shape/facing/powered/connections consumers. Focused sandbox
and engine tests verify winding/normals, transforms, DDA/slope/AABB queries, partial-neighbor
visibility and signed section/chunk boundaries. World, GUI/drop and selection roles remain distinct.

`just render-scale` measures the existing cube-heavy release workloads. General model emission is
separate from canonical cube quads. The bounded asynchronous stress harness consumes ready results
while waiting for remaining jobs, preserving its two-section upload budget. Otherwise a full ready
window prevents remaining jobs from being submitted. Baseline and post-change results, including
any unavailable/conditional GPU evidence, belong in [BG1_REPORT](BG1_REPORT.md).

Conservative boundary coverage avoids missing exposed faces without arbitrary polygon clipping.
Partially hidden face portions can remain; static surface meshes do not claim full neighbor coverage.
Model compilation limits general geometry to 128 triangles; worker result accounting includes the
larger worst-case general-model payload without allocating that maximum for ordinary cube sections.
