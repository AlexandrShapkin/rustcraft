# Developer diagnostics

Current F1 implementation: normal startup and explicit trusted `--devtools` use the same
WindowEvent router and diagnostic presentation. `just client` grants local diagnostic inspection
and selection (`debug.inspect`, `debug.configure` and read capabilities), without mutation or Rhai
permission. `--player` demonstrates an ungranted diagnostic selector through the same route;
unavailable actions report capability denial. `--devtools` grants trusted local Control/Rhai
operations; it changes grants, not keyboard routing or gameplay mode. **F4 opens the in-game
selector**. Arrow keys select, Tab switches pages/overlays, Enter activates/toggles, H shows help,
C selects the crosshair/player section, E selects the next bounded active EntityId, and Escape/F4
closes. Backquote requires `script.load`; Slash opens native command entry. F10 aborts scenarios.
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
scanner. C1 now owns configurable diagnostic cadence through the shared settings registry;
four Hz is the default, not a second settings owner.

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

## C1 Settings

F4 -> Runtime settings uses the shared configuration plane. Left/Right selects a key, +/- adjusts supported scalar values, R resets its runtime override. Backquote opens the existing console for exact /config values, coupled batch/reset or explicit persistence. Effective/requested/source/type/range/owner/policy/error remain visible; only privileged tooling mutates. Settings adds no broad-provider demand. See [CONFIGURATION.md](CONFIGURATION.md).

## UX1 controls and text

Hold F3 then press 1 Streaming, 2 World, 3 Entities, 4 Lighting,
5 Meshing, 6 Renderer, 7 Persistence, 8 Scripts, 9 Settings. Digits are consumed and never select
hotbar slots. Bare F3 toggles Overview on release. Repeat/focus loss cannot leave a chord active.
F4 remains the complete selector; shortcuts are metadata conveniences, not view identities.

Slash opens `COMMAND > /`; Backquote opens empty `RHAI >`. Leading slash executes the existing
Control registry, other text executes the trusted Rhai REPL. `/help config`, `/help debug`,
`/help scenario` show registry usage/capabilities/aliases. `/commands`, Tab and Up/Down discover
commands/history; Esc closes. Text input uses winit text/IME; arrows/delete/backspace operate on
extended graphemes, with IME commit and visible preedit. The caret is drawn at shaped cluster
positions rather than inserted into the text. Native input tests cover composed/decomposed Latin,
Cyrillic, emoji, family ZWJ and flags. Real IME composition across all desktop IMEs is not claimed.

Text uses bundled fonts exclusively; see [UX1_REPORT.md](UX1_REPORT.md) for licenses, coverage,
semantic replacement and bounds. `/config set rustcraft:ui/font_scale 1.5` changes shared scale.

## RSM1 residency and memory view

F4 exposes **Residency / Memory** (`memory`, Medium cost). It reads the same demand-driven
`Residency` sample as Control snapshots and Rhai `residency()`; captures include `residency.json`.
Counts distinguish live world/render/snapshot/GPU ownership, retired-but-unfinished jobs/lighting,
logical payload bytes and reusable capacity. Chunk inspection explains active-lighting, dirty/save
acknowledgement and cleanup-pressure blockers. Inactive pages do not collect this domain.

Current generation entries should track current render sections, not all visited positions. The mesh
result window includes submitted-unconsumed plus ready results. Device-wide GPU counters are labelled
as such; process VRAM may be unavailable. Fixed UX1 text capacity is global, not travel growth.
See [RSM1_REPORT.md](RSM1_REPORT.md) for bounds, workloads and telemetry limitations.

## Presentation timing

F4 -> Presentation, `/debug page presentation`, or Rhai `presentation()` reads the same cached
`Snapshot.presentation`. It exposes configured refresh, actual backend/mode, render/request/redraw/
submit/present-call distributions, acquisition wait, alpha/state age, authoritative and shown
transforms, duplicate counts and application mouse-to-camera extraction latency. Physical scanout
and VRR are explicitly unavailable. Medium-cost summaries sample at C1 diagnostic cadence; inactive
views do not call the provider. Local diagnostic admission retains bounded scalar recent timing, not resources.
The viewport rounds timing values; JSON tuples identify columns in `statistics_columns`.
Stationary/paused duplicate transforms are normal. See [P1_REPORT.md](P1_REPORT.md).

S1 Persistence observations include successful column raw/application-write bytes, encode/compression/
write/atomic-publication envelope totals, save queue wait total/max, current oldest dirty age,
coalesced pre-save mutations and stale acknowledgements. These are the same demanded Snapshot fields
returned by `persistence()` in Rhai/Control and captured in bundles. Totals reset on store open and
exclude checkpoint/failed-write bytes; they are not SSD/NAND write or isolated fsync counters.
Use `just scenario-client scripts/scenarios/s1_diagnostics.rhai` for a disposable graphical smoke.
Use isolated save/config paths as for other acceptance runs. Full storage evaluation is headless.

## F1 routing and field acceptance (pending hardware)

The production `WindowEvent` adapter copies physical key/state/repeat/text into one safe payload
and feeds the complete client router. That router owns developer admission, gameplay digit/ordinary
key routing, inventory transitions and focus reset. winit's KeyEvent has private platform fields;
deterministic integration tests exercise this production payload boundary plus real Focused events,
not only the shortcut helper. Shared-desktop global injection is not a CI dependency.

Held F3 digits are consumed even if capability admission fails; ordinary digits select the hotbar.
Bare F3 toggles on release, repeats do not retrigger developer effects, and losing focus clears the
chord and entire human controller. Default local diagnostic grants do not grant world mutation,
configuration writes or trusted Rhai. Game mode remains independently game-owned session state.

`just f1-client` runs matched dev then release field workloads sequentially, through the normal
client runtime, with disposable world/config/cache below `target/f1/run-NONCE/`. Optional argument
`dev` or `release` runs one profile. It requires AMD/RADV/Vulkan and rejects missing/other graphics;
set `RUSTCRAFT_TERRAIN_TEXTURE` to a local licensed terrain PNG if the default local asset is absent.
The normal owner build profiles are retained. Each run has 30-second stationary/pan/walk/walk+pan
phases after readiness and a 420-second application timeout (wrapper 600 seconds).

Return `target/f1/run-NONCE/summary.json`; per-profile summaries and `timeline.json` preserve exact
adapter/driver/backend, profile, present mode, window/surface, effective C1 settings, presentation
statistics, fixed-step budgets, mouse extraction latency, duplicate transforms and checkpoint events.
Motion is automated through normal human-controller/mouse ingestion, not a physical mouse device.
At least twenty successful player checkpoints are required. CLI checkpoint cadence is 1000 ms in
both disposable runs. Worker publication envelopes and main-thread submission/completion service
spans share the monotonic frame timeline; overlap is correlation, not proof of a frame stall.
`sync_ms` includes atomic publication/directory work, not isolated fsync. No scanout, input-to-photon
or subjective smoothness is measured. Hardware acceptance and any necessary owner observation remain
pending; see [F1_REPORT](F1_REPORT.md). P1 presentation policy is unchanged.
