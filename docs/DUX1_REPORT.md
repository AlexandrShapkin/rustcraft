# DUX1 in-game developer diagnostics acceptance

Status: **CLOSED — local acceptance and Ubuntu/Windows public CI passed**.
Implementation: `f6ede4ceeca2be153cd29a51c525454759de6a3f`.
Ubuntu and Windows: **green** in [implementation CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37174284855).
Starting public main: `195f53ca3025c9f001f0338d7a51a6e6c6626a80`.
The starting Ubuntu and Windows jobs passed in [baseline CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37167717973).
The original dirty mounted checkout is preserved; publication uses `/tmp/rustcraft-dux1`.
M0–M4 and DX1 remain closed, M4-009 remains hardware-conditional. C1 and later stages are inactive.

## Architecture and available surface

`control::diagnostics` owns a native, bounded metadata registry and selector state, independent of
Minecraft, winit, wgpu and Rhai. Semantic IDs (`rustcraft:debug/page/...`,
`rustcraft:debug/overlay/...`) and owner metadata identify views; array positions/key bindings do
not. Composition can register package views. The registry is capped at 64 entries (17 registered
by the graphical composition), with bounded metadata/provider requirements. No future mod UI or
general settings registry is implemented.

The graphical Host prepares requested existing subsystem observations into Control's shared
Snapshot cache. `minecraft-b173::control` provides semantic world/entity values and cheap current
player/tick observation; graphical composition provides streaming, lighting, mesh, renderer, save
and selected-chunk status; DevTools publishes bounded script/scenario/job summaries only when due.
UI, console, Rhai, scenarios and captures reuse these values. No human-only scanner exists.

Pages: Overview, Streaming, World, Entities, Lighting, Meshing, Renderer, Persistence,
Scripts/Scenarios/Jobs, Selected chunk/section, Selected entity. Overlays: nearby residency,
entity/owner markers, player collision, ray target, selected section and selected entity/owner.
Low/Medium/High metadata describes likely collection cost, not a universal duration guarantee.

F4 opens/closes a discoverable keyboard selector. Up/Down select; Tab switches pages/overlays;
Enter selects/toggles; H shows help/provider requirements; C selects the crosshair/player section;
E selects the next bounded active stable EntityId. F3 returns to Overview and retains its existing
shortcut behavior and cached FPS/timing/stream summaries. Hidden legacy overview collection is
suppressed; Control/Rhai `debug().legacy_overview_text` shares its existing cached text; backquote/console and F10 abort remain. Page/overlay active labels, titles,
availability, cost, owner/semantic ID and shortcut/help are visible.

Selector/console focus is mutually exclusive and suppresses human movement, wheel/click and
mouse-look events. Entering clears held input/releases the cursor; closing restores capture and
requires fresh input. The selector does not pause simulation. An explicit scenario controller lease
still owns semantic intent; UI navigation events do not become gameplay intent.

Chunk inspection: coordinates/section, resident, Desired, radius-Retained, Safe, Visible, residency
phase, global lighting backlog, column/selected-section mesh stage, generation, selected GPU-page
presence/count, dirty/save pin and bounded active entity count. Reasons distinguish unavailable/
not-resident/unsafe/not-currently-visible/ready; they do not pretend to prove a general causal chain.
Precise per-column lighting convergence and more detailed retention-pin reasons remain unavailable.

Entity inspection: stable nonzero 128-bit EntityId, semantic type, position, velocity, owner chunk,
age, logical persistence revision and contextual durability. Revision does not prove fsync. Lookup
examines at most 4096 active records; missing/unloaded/removed/truncated possibilities retain the
requested identity. The active list has a 64-record cap/eight-row paging; entity overlay uses twelve
of that same sampled list. Selection does not fall back to an unrelated Vec slot.

Null/absent providers, unavailable surface data, missing selections, stale samples, bounded coverage
and UI viewport truncation are explicit. Full bounded semantic values remain in shared queries and
captures. Native debug text is capped at 28 rows, 110 columns and 2600 characters. Added punctuation
uses the existing project-authored font; no proprietary pixels are committed.

## Demand, costs and reuse

No active page/overlay/query consumer means no provider collection. Captures and terminal/failure
bundles explicitly request bounded available domains once, preserving DX1 diagnostics. Overview reads cheap current
player/tick values and requests no broad domain. First demand samples immediately; ordinary repeated
demand reuses the same value at DX1's 250 ms diagnostic cadence. Selection/toggle changes invalidate
relevant selected/overlay samples. Providers record requests/collections, last/max/total collection
microseconds and sample age; two-cadence age marks stale. Registry discovery never invokes a provider.
Script construction is timed and suppressed before constructing lists when its sample is fresh.

Rhai query preflight is conservative and bounded: source domain references request their shared
providers; REPL-defined functions retain source requirements; a registered command requests its own
source's domains. Comments/names can over-request. Existing `entity(STABLE_ID)` still demands the
bounded active list; new selected inspection queries use their selected-provider value. Metadata
queries do not scan the world. C1 will own general runtime settings/cadence, not a competing DUX1
configuration system.

The client reuse test observes World once, formats that value for the native page, then reads
`world().block_query_radius` through the actual DevTools/Rhai path. Collection count remains one.
It also checks existing stable entity queries collect the active list on demand. Native sandbox
registration presents `sandbox_test:debug/pulse` and Rhai reads `world().pulse` from that same value;
`just sample-game`'s normal-dependency guard proves no Minecraft/runtime dependency.

Final isolated `just dx-overhead` measurements (service microseconds):

| Mode | Mean | p50 | p95 | p99 | Max |
|---|---:|---:|---:|---:|---:|
| disabled | 3.99 | 3.91 | 5.31 | 5.66 | 5.80 |
| overview | 99.51 | 74.94 | 296.34 | 379.38 | 428.48 |
| low_page | 117.25 | 83.95 | 333.56 | 456.76 | 825.46 |
| high_page | 120.38 | 80.74 | 323.16 | 434.83 | 712.46 |
| overlay | 139.89 | 79.76 | 454.32 | 553.85 | 675.02 |
| scenario | 668.63 | 643.45 | 916.32 | 1138.14 | 1339.36 |

Disabled/Overview collect no broad domains; Lighting collected 34 times, Renderer 32 and
overlays 33. After switching away, each earlier provider collected zero times. These are
whole DX service costs, including maintenance/polling, not universal budgets or GPU targets.
The harness reports
mean/p50/p95/p99/max service microseconds for disabled, Overview, Lighting, Renderer, representative
Residency/Collision overlays and a cooperative scenario, with 240 event turns per mode and 20
warmup turns excluded. Provider deltas cover all 240 turns. Software-GPU frame time is separate.

## Validation and evidence

Final real-surface receipt: `target/test-runs/scenario/4653-1791084539773145851/result.json`,
status pass, 26.502 seconds, tick 10. All required local gates below passed; `just ci` ran
265 tests successfully with one optional skipped test. The real-surface scenario passed four cycles with page selection, both on/off repetitions of
all six overlays, target/entity selection, selector/console handoff, resumed semantic gameplay and
three captures. It held 17 registered views, at most 11 cached domains, zero terminal geometry and a
128-event ring. Generic state tests run 100 cycles; client highlight/focus tests run 24 cycles.
At most 54 simultaneously enabled boxes fit the existing 64-AABB line-pass cap; inactive geometry
clears rather than accumulating GPU buffers or subscriptions.

The large initial twelve-cycle script exceeded the unchanged 10 ms Rhai step-construction deadline;
the accepted harness is four cycles and runs in release. No deadline/operation limit was raised.

Actual surface/offscreen correctness uses llvmpipe OpenGL (LLVM 23.1.1), 1280x720. Owner AMD access and
representative GPU/display performance are not claimed. Captures/local saves remain ignored; the
acceptance harness uses a unique PID/nonce fixture world and one item, not the normal user's world.

Required gates: fmt/check/workspace tests/strict all-feature Clippy, just ci, dx-test, dx-console,
dx-overhead, sample-game, dux-client and M3 fidelity (including ignored multipage render proof).
M4 frozen correctness/persistence/streaming tests remain in workspace/CI. No world residency,
GPU lifetime, generation, pacing or storage behavior was changed, so expensive M4 travel matrices
are not rerun solely for documentation/diagnostic getters.

## Deferred breadth and stop boundary

Deferred: polished/mouse-driven UI, profiler/editor/mod UI, precise column-light causal inspection,
additional game entity kinds, remote admin, future command breadth and general settings controls.
No C1, RSM1, P1, S1, R2, A1 or DX2 implementation was started. M5/M6/M7 remain inactive. No version
bump, tag or release was created. See [DEBUGGING.md](DEBUGGING.md), [ROADMAP.md](ROADMAP.md) and
[PRE_M5_AUDIT.md](PRE_M5_AUDIT.md). Both implementation platform jobs are green; publication of this acceptance record is also monitored.

Selection/configuration actions require existing `debug.configure`; shared diagnostic reads retain
their read capabilities. Read-only/FutureChat contexts cannot change the selector or inspection target.
