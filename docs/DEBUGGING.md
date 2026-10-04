# Developer diagnostics

Diagnostics require explicit `--devtools` (or a developer scenario/harness). Normal startup has no
selector, console, overlays or diagnostic collection. F3 keeps fast Overview access; **F4 opens the
in-game selector**. Arrow keys select, Tab switches pages/overlays, Enter activates/toggles, H shows
provider/shortcut help, C selects the crosshair/player section, E selects the next bounded active
EntityId, and Escape/F4 closes. Backquote opens the existing console; F10 aborts scenarios.
F3 Overview preserves the existing cached FPS/timing/stream summaries; `debug().legacy_overview_text`
reads that same cached observation. The legacy collector is suppressed when another page, selector
or console hides it. The selector and console are mutually exclusive. UI focus clears held human input/releases the
cursor; closing restores gameplay capture without replaying held movement/look/click events.
Simulation is not implicitly paused. Explicit scenario intent remains a separate controller lease.

`control::diagnostics::ViewRegistry` owns bounded semantic metadata (ID, title, description, owner,
kind, cost, shortcut, provider requirements). Shortcuts and navigation offsets are not identities.
Native packages can register views; the independent sandbox registers `sandbox_test:debug/pulse`
and reads its same World observation through the generic page and Rhai. No mod UI is implemented.

Pages: Overview, Streaming, World, Entities, Lighting, Meshing, Renderer, Persistence,
Scripts/Scenarios/Jobs, Selected chunk/section, and Selected entity. The selector displays active page,
active overlays, availability, Low/Medium/High cost guidance and help. Renderer byte counts are
logical/capacity estimates, not driver VRAM; present mode is not physical display timing.

`/debug page NAME` also accepts registered semantic IDs. `/debug page entities N` paginates the
64-entry active subset, eight entries per page. `/debug chunk X Z SECTION_Y` chooses an explicit
section; C returns to crosshair/player selection. Inspection composes resident/Desired/Retained/Safe/
Visible status, residency phase, mesh stage/generation, dirty/save pin and bounded active entity
coverage. Reasons are conservative observations, not proof of a general causal chain: column-specific
light convergence is explicitly unavailable; global lighting backlog is labelled separately.

`/debug entity HEX_ID` selects a stable, nonzero 128-bit EntityId. Output includes semantic type,
position, velocity, owner column, age and logical persistence revision. Durability is not claimed from
a revision alone. A missing entity retains the requested identity and reports removed/unloaded/
bounded-coverage possibilities rather than selecting a different entity. Targeted lookup examines at
most 4096 active records; list and inspection truncation are explicit. UI viewport truncation and
unavailable/unsampled/stale data are labelled; complete bounded values remain in shared JSON queries.

Overlays: Residency/streaming (5×5 nearby columns), entity markers/owners (12 active records), player
collision, ray target, selected section boundaries and selected entity/owner. `/debug overlay NAME
on/off` or the selector toggles them repeatedly without restart. At most 54 boxes from these views
fit the existing 64-AABB line-pass cap; enabling overlays does not rebuild/persist world data.
Streaming colors: green Visible, yellow Safe, red Desired/unready, blue Retained, grey outside
interest. Entity markers are magenta; owners blue; collision yellow; target white; selected section
cyan; selected entity orange. Disabling every overlay clears geometry immediately.

## Demand and shared observations

The active page, active overlays and explicit Control/Rhai/scenario queries request provider domains.
Captures and terminal/failure bundles explicitly request bounded available domains once.
No consumer means no provider walk. Overview uses cheap current tick/player values. First demand
samples immediately; repeated demand shares the cached observation for DX1's 250 ms cadence.
Selection/toggle changes invalidate relevant samples. Provider metrics record requests, collections,
last/maximum/total collection duration and sample age; values older than two cadences show stale.
Cost classes guide use, not hardware-independent timing guarantees. Metadata discovery does not
invoke providers. Script summaries also honor cadence before constructing bounded loaded/job/error
lists. REPL-defined functions retain conservative domain preflight; registered command preflight uses
that command's source requirements. Comment/name matches can over-request, but never create a second
scanner. Four Hz is inherited diagnostic cadence, not a general runtime settings registry; C1 will
own future configurable cadence/settings.

Automation and humans use the same `Snapshot` fields. Rhai adds `debug()`, `chunk_inspection()` and
`entity_inspection()`; scenario `assert_debug(PATH, BOOL/STRING/INT)` checks current semantic metadata.
`/debug ui open/close/next/previous/tab/activate/help/target/entity` drives the same selector state as
keyboard navigation. All actions remain capability checked; views add no mutation privileges.

`just dux-test` runs focused shared diagnostics/selector tests. `just dux-client` runs the release
real-surface selector/inspection/toggle scenario, creating an isolated `dux1-acceptance-PID-NONCE` world
and one fixture item. It uses normal renderer/capture/control machinery, not a fake selector.
Local user-provided assets remain required and are never committed. The large bounded soak is run
in release so step construction fits trusted Rhai limits; no execution limit is raised.

`just dx-test`, `just dx-console`, `just scenario-headless` and `just sample-game` retain DX1 gates.
`just dx-overhead` measures DX CPU service separately from total GPU frame cost: disabled, Overview,
low-cost Lighting page, higher-cost Renderer page, representative Residency/Collision overlays and
scenario. Each mode has 240 event turns, 20 warmup turns excluded, mean/p50/p95/p99/max and provider
collection deltas. Ignored evidence is under `target/dx-overhead.json`, `target/test-runs` and
`target/captures`. See [DUX1_REPORT.md](DUX1_REPORT.md) for acceptance evidence and limitations,
[SCRIPTING.md](SCRIPTING.md) for trust/commands and [PRE_M5_AUDIT.md](PRE_M5_AUDIT.md) for later stages.

Selection/configuration actions require existing `debug.configure`; shared diagnostic reads retain
their read capabilities. Read-only/FutureChat contexts cannot change the selector or inspection target.
