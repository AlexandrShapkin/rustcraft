# M2 implementation and validation — 2026-09-29

M1.1 remains accepted and closed. M2 implementation, automated validation and hardware-backed
frame inspection are delivered. The owner has manually accepted M2; this report records the
implementation-era validation and predates that acceptance. No M3 work was started in this
historical report.

## Implemented result

`just client` opens a finite flat sandbox with an authored building area, 16 block definitions
(air plus 15 building/emissive blocks), 15 placeable items and a nine-slot development hotbar.
The required twelve blocks plus brick, bookshelf, sandstone and a temporary full-cube lamp exist.
Simulation owns inventory and selection; HUD owns neither. Keys 1–9 and accumulated wheel steps
produce semantic selection intents. Placement uses the selected stack, the DDA hit face, empty
cell validation and solid-block/player-AABB rejection, and consumes exactly one item on success.
Breaking is a gameplay action with breakability checks. Both paths update lighting and dirty meshes.

Crosshair, highlight, textured icons/counts, target outline and project-owned pixel text are drawn
through a small bounded 2D path. The target outline uses the same simulation ray as interactions.
World storage carries BlockState and packed light channels. Direct sky seeds plus a local queue
handle sky and emitter addition/removal across signed chunk/section boundaries. Vertex brightness
combines stored light and face direction. Materials distinguish opaque/cutout/translucent/invisible;
current glass uses binary alpha rather than sorted fractional transparency.

Bot API v2 exposes semantic block/item definitions, capabilities, stack slots and selected hotbar.
Selection/use/attack share AgentIntent and simulation with humans. Explicit target actions are
validated rather than granting raw world access. Server/runtime/bot dependency trees contain no
wgpu or winit. See D-015 through D-018 for ownership, algorithms and metric definitions.

## Validation

The initial working tree already contained uncommitted M1.1 and substantial M2 work. Baseline
bootstrap/tests and smoke passed; baseline strict Clippy found collapsible-if failures in those
M2 files. These were fixed while finishing M2, not treated as reopening accepted M1.1.

- `just bootstrap-check`, `just ci`, `just smoke`: pass after changes (format, workspace check,
  56 unit tests, strict all-target/all-feature Clippy, reference status and headless action scenario).
- Focused inventory, ray, interaction, lighting, mesh, frame/TPS/scheduler/sampling, HUD cache,
  unsupported telemetry and input routing tests pass. Normal tests require no GPU.
- `cargo tree -p rustcraft-server -p rustcraft-bot-api -p rustcraft-runtime`: no graphics crates.
- `git diff --check`: clean. Existing uncommitted M1.1/M2 work was preserved; no commit/push made.
- Real Vulkan captures completed with exit 0: triangle, cube-no-cull, cube, checker, atlas,
  platform, chunk, section-negative/zero/positive, normal and normal-lit. Every frame was opened
  and inspected. Accepted camera/texture/origin stages remain healthy.
- Exact `just client-diag` triangle recipe also launched successfully in a bounded five-second
  run (expected timeout 124/SIGTERM); the twelve capture runs above exited normally.
- Real normal client run through `just client`; 12-second hidden/visible telemetry runs completed.
  Final F3 capture shows scene, nine slots, counts, selected border, centered crosshair, target
  outline and live metrics. No keyboard/mouse manual acceptance is claimed.

Local evidence: `/tmp/m2-diag-*.png`, `/tmp/m2-baseline-triangle.png`,
`/tmp/m2-f3-final.png`, `/tmp/m2-f3-final.log`, `/tmp/m2-hidden-final.log`,
`/tmp/m2-bench-final.log`, `/tmp/m2-bootstrap-final.log`, `/tmp/m2-ci-final.log`,
`/tmp/m2-smoke-final.log`. These temporary files are not packaged assets.

## Real-client measurement

Hardware: AMD Radeon Vega 8 Graphics (RADV RAVEN), integrated GPU, Vulkan, eight logical CPUs.
Build: Cargo debug/dev, default features. Surface requested 1280x720; desktop resized it to
1280x662, FIFO, Bgra8UnormSrgb. Each run lasted 12 seconds after world/mesh setup. Values below
are the final rolling history/periodic snapshot, not an entire-run percentile or idle-subtracted CPU.
The world had nine chunks, 18 resident/rendered sections, 12,468 vertices, 18,702 indices,
18 mesh uploads, zero pending dirty sections. Draw calls: 19 hidden, 20 with F3.

| Metric | F3 hidden | F3 visible |
|---|---:|---:|
| Rolling FPS | 53.78 | 56.75 |
| Rolling frame ms | 18.594 | 17.622 |
| 1% low FPS | 29.80 | 29.97 |
| Actual TPS / target | 19.91 / 20 | 19.43 / 20 |
| Mean simulation tick ms | 0.0498 | 0.0518 |
| Process RSS MiB | 180.77 | 182.85 |
| Process CPU, one core = 100% | 15.80% | 21.31% |
| Threads | 10 | 10 |
| GPU world+HUD ms | 0.428 | 0.539 |
| GPU utilization, device-wide | 0% | 0% |
| Device VRAM used / total MiB | 845.46 / 1024 | 845.46 / 1024 |
| Mean initial per-section mesh/upload ms | 1.882 | 1.647 |

GPU 0% is the sampled kernel counter, not an inferred utilization. VRAM belongs to the entire
adapter, including other applications, and on this integrated GPU is the driver-reported VRAM
pool rather than all shared system RAM. GPU timing excludes presentation waits. All requested
hardware categories were available on this adapter. On unsupported devices/providers they remain
N/A; 1% low is N/A before 100 frames, TPS before its first interval, and CPU before two samples.
Temperature/power/system CPU are not implemented (optional, not displayed).

F3 overhead: paired wall-frame means are dominated by desktop/FIFO variability; the visible run
being faster is not evidence of a speedup. Process CPU increased about 5.5 percentage points,
RSS by 2.1 MiB and sampled GPU duration by 0.11 ms. Before caching, a separate flat-scene pair
was 55.23 FPS hidden versus 45.11 visible (18.107 versus 22.168 ms). A repeatable CPU workload
identified unchanged text generation: 3.0755 ms/build before, 0.0328 ms/build after caching over
240 identical snapshots. Real F3 refreshes at 250 ms, so that microbenchmark measures reuse,
not the cost of refreshing every text snapshot. No broad optimization campaign was performed.

## CPU-only repeatable workload

Run `just bench-m2`. It uses the original flat 33x33 terrain, nine chunk columns, 18 geometry
sections, 10,824 vertices, debug build; no GPU/window required. Per-edit extraction+meshing is
reported separately from propagation. Lighting initialization: 3421.17 ms; initial CPU meshing:
31.76 ms. Mean of the last 240 of 1000 idle simulation steps: 0.0120 ms.

| Edit | Add light ms | Remove light ms | Resident sections rebuilt | Add/remove extraction+mesh ms |
|---|---:|---:|---:|---:|
| Interior stone (8,1,8) | 0.607 | 0.451 | 1 / 1 | 10.384 / 8.379 |
| Boundary stone (15,1,8) | 0.447 | 0.518 | 2 / 2 | 17.495 / 20.372 |
| Emissive cube (15,1,8) | 83.533 | 131.376 | 11 / 11 | 75.683 / 74.465 |

The lamp notifies 12 section coordinates; 11 have resident geometry. Ordinary stone edits visit
37 light cells; lamp add/remove visits 6681/10192 queue entries. These are measurements of this
small workload, not promises about large worlds. Initialization and emitter latency warrant a
future release-profile investigation; they are recorded in DEFECTS.

## Remaining limitations and next milestone

Non-blocking limitations: binary-alpha glass, suppressed internal leaf faces, outline hidden edges,
finite authored world, debug-build lighting latency, optional platform telemetry coverage, and
refs-status's loose-atlas detection. See DEFECTS. M2 has no crafting, drops, survival restrictions,
fluids, procedural Beta terrain, persistence or network transport. Item acquisition/drops and the
basic survival/crafting loop are the next logical milestone after manual M2 acceptance.

Reference evidence: `reference/notes/m2-content-lighting.md`; Beta release `740c583901e1`,
mc173 `16f39e762da2`, local terrain atlas, wgpu documentation via Context7, and the primary kernel
[/proc](https://www.kernel.org/doc/html/latest/filesystems/proc.html) and
[amdgpu telemetry](https://docs.kernel.org/gpu/amdgpu/thermal.html) documentation. No proprietary
assets/fonts were committed or copied into distributable resources.
