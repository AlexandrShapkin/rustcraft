# Runtime configuration (C1)

One composition-owned `rustcraft-config` registry supplies typed operational values. It is a leaf
with serde/serde_json and the existing atomicwrites dependency, without game/render/window/Rhai.
Native subsystem fields are updated at boundaries; voxel/entity inner loops do not look up keys.
Control owns the adapter; package composition may register native namespaced settings before open.

## Sources and persistence

Precedence: compiled default < startup user file < environment compatibility < CLI < runtime.
`--set-config KEY=VALUE` may repeat. `--config-file PATH` selects a separate user settings file;
otherwise `RUSTCRAFT_CONFIG_FILE`, then Linux `$XDG_CONFIG_HOME/rustcraft/config-v1.json`
(or `$HOME/.config/...`) / Windows `%APPDATA%/rustcraft/config-v1.json`. Without a platform home,
use `./rustcraft/config-v1.json`. This is not a world save or secret store.

Version 1 JSON has `version` and `values`; values are typed `{ "type": "DurationMs", "value": 250 }`
(or Bool, Integer, Float, Text, Bytes). Keys, metadata, values, file size (64 KiB), registration
(64), batches (16) and history (128) are bounded. Unknown keys/type/range errors identify the
source/key. Malformed JSON identifies path/line. No executable configuration.

Runtime writes never persist implicitly. `/config persist KEY ...` atomically writes successful
current values (or restart requests) into the next-launch file. `/config unpersist KEY ...` removes
those file entries. Persist coupled radius keys together; invalid next-launch pairs are rejected.
File edits do not retroactively change this session's startup source layers. `/config reset KEY ...`
removes only its runtime layer and reveals the next winning startup source, not necessarily the
default. Reopen to load explicit file edits. File writes flush and use atomicwrites' cross-platform
replacement, reusing the same implementation family as existing world writes. Windows directory-entry durability is not claimed. Runtime persistence does not touch worlds.

`--config-report` on client/server prints structured effective/requested metadata without graphics.
`/config list`, `get KEY`, `describe KEY`, `set KEY VALUE`, `batch KEY=VALUE ...`, `reset KEY ...`,
`persist KEY ...`, `unpersist KEY ...` use the existing command registry and completion. `select KEY`
selects the developer Settings row. Trusted Rhai: `config_get`, `config_describe`, `config_set`
(string values use the same typed parser), `config_reset`. Script actions apply after evaluation;
a query later in the same evaluation still reads its immutable input snapshot.

Read/write/persist use config.read/config.write/config.persist capabilities. Source labels do not
authenticate. FutureChat/read-only cannot mutate or persist. No raw filesystem capability is given
to Rhai. Explicit file persistence is restricted to the composed settings path.

## Application contract

Immediate synchronously acknowledges policy. NextTick/NextFrame leave requested pending until the
fixed-tick/event-frame boundary adopts typed native values. Reconfigure uses the same acknowledge/
failure state machine; production pools remain restart-only because no safe live drain exists.
RestartRequired retains old effective value and exposes a next-launch request; explicit persistence
is needed to carry it across launches. ImmutableAfterOpen returns its reason. One outstanding batch
is allowed; mixed boundaries/duplicate keys/invalid pairs are rejected before any native changes.
Adapters must prepare atomically and keep their old state on failure. Failure discards the request,
keeps known-good effective state and records structured bounded change evidence.

F4 -> Runtime settings exposes bounded metadata and requested/effective/source/type/range/owner/
policy/status. Left/Right selects a semantic key; +/- changes numeric/bool values; R resets. Use the
existing console for exact strings/enums, atomic pairs or persistence. Existing DUX focus/demand
rules remain; no second authoritative UI configuration. Snapshots/captures/failure bundles include
configuration and bounded history. The inherited 250 ms default is configurable 50..5000 ms.

## Definitive operational inventory

Native defaults/bounds are authoritative in `crates/config/src/settings.rs`; this table records
migration ownership and compatibility. All keys below have the `rustcraft:` namespace.

| Key | Owner / consumer | Policy / default | Legacy environment alias |
|---|---|---|---|
| streaming/load_radius | engine residency / client, headless | NextTick / 4, 3..12 | RUSTCRAFT_STREAM_RADIUS |
| streaming/retain_radius | engine residency / client, headless | NextTick / 5, load..16 | RUSTCRAFT_STREAM_RETAIN_RADIUS (new compatibility alias) |
| streaming/lookahead | client residency priority | NextTick / false | RUSTCRAFT_STREAM_LOOKAHEAD |
| diagnostics/sample_interval_ms | shared DUX providers/text | Immediate / 250, 50..5000 | --set-config |
| persistence/player_interval_ms | client player checkpoint | NextTick / 2000, 1000..2000 | RUSTCRAFT_PLAYER_AUTOSAVE_SECONDS (seconds -> ms) |
| persistence/world_interval_ms | client world checkpoint | NextTick / 2000, 100..60000 | RUSTCRAFT_WORLD_AUTOSAVE_SECONDS (seconds -> ms) |
| meshing/upload_sections | client existing mesh upload | NextFrame / 4, 1..64 | RUSTCRAFT_MESH_UPLOAD_SECTIONS |
| meshing/upload_bytes | client existing mesh upload | NextFrame / 8 MiB, 256 KiB..64 MiB | RUSTCRAFT_MESH_UPLOAD_BYTES |
| lighting/work_budget | client boundary light work | NextTick / 32, 1..256 | RUSTCRAFT_LIGHTING_WORK_BUDGET |
| streaming/main_budget_ms | client service | NextFrame / 2, .25..8 | RUSTCRAFT_STREAM_MAIN_BUDGET_MS |
| meshing/workers | client mesh pool | RestartRequired / CPU-derived 1..3, allowed1..32 | RUSTCRAFT_MESH_WORKERS |
| lighting/workers | initial light pool | RestartRequired / 1, 1..4 | RUSTCRAFT_LIGHT_WORKERS |
| renderer/present_policy | surface | RestartRequired / PreferFifo only (fallback to first supported mode); P1 chooses future policies | none |
| scripts/reload_interval_ms | trusted tooling worker polling | Immediate / 500, 100..5000 | --set-config |
| identity/chunk_width | engine persisted structure | ImmutableAfterOpen /16 | none |
| identity/control_version | Control contract | ImmutableAfterOpen /1 | none |

Headless has no consumer for meshing/presentation, lookahead/light work or client checkpoint settings; metadata marks them unavailable
and runtime writes reject. The current server is a bootstrap/scenario/harness composition, not a
long-running network server. Its native residency adapter applies radii in scenario/smoke paths;
checkpoint defaults remain introspectable but runtime writes are unavailable; no nonexistent server autosave loop is fabricated.
Legacy radius-only environment input derives retain=load+1 unless a file/explicit retain is supplied.
Invalid aliases now fail with source/key errors rather than silently falling back/clamping.

Remaining intentional boundaries, with owner/reason/future work:

| Item / family | Classification and change policy | Owner / explicit reason / future stage |
|---|---|---|
| WORLD_SEED, GENERATOR_VERSION, existing world metadata/generator ID/version | Identity / ImmutableAfterOpen | minecraft-b173 world-open validation; current file metadata authoritative, no migration/hot reload (S1/A1 if needed) |
| world/chunk/player/world-state record versions, structural dimensions/palettes/component envelope bounds | Identity / ImmutableAfterOpen | world; persisted/schema safety, not configurable algorithms; S1 evaluates backend without casual schema edits |
| compiled semantic registry, content manifest and runtime texture representation | Identity / ImmutableAfterOpen | game/content/render-profile; compile at composition; R2/A1 own future contracts |
| TERRAIN_TEXTURE, RESOURCE_CACHE, MAX_ATLAS_DIMENSION; AtlasPolicy and ResourceLimits max files/file bytes/decoded bytes/pages/dimensions | Operational startup / deferred RestartRequired | resource composition/content; source paths, compilation/cache/layout need rebuild; R2 evaluates normalization; no general resource hot reload |
| SAVES_DIR, WORLD_NAME; --world/--saves and world inspection arguments | Identity/open target / RestartRequired | client/server; select storage before open, never redirect active writes; S1 may refine composition |
| Legacy F3 full-metrics1s collection, process/GPU telemetry polling and trace cadence | Diagnostic/deferred launch policy | client/debug; preserved legacy cost contract outside shared bounded provider cadence; RSM1/P1 measure before tuning |
| F3, F3_TRACE; overlay/page selection | Diagnostic transient / Immediate through existing DUX controls | client/DUX; selection/console state is transient, not a parallel general settings registry |
| MEASURE_SECONDS, STREAM_PERF_SECONDS, STREAM_WINDOW_SIZE; --bench-m2/m3, --render-scale, --camera-motion, --stream-perf | Diagnostic/test-only; launch-bound | client specialist harnesses; preserve frozen workflows and thresholds, DX2 retire only after equivalence |
| --capture/diagnostic-stage/resource-report/version/survival; server smoke/worldgen/map/info/inspect/persistence/entity/stream modes | Historical specialist / explicit launch mode | composition; modes/positional workload inputs are not hot operational values; DX2 inventory, preserve tests |
| --devtools/--scenario/--script-check/--script-bench/--dux-acceptance/--dx-overhead/abort-frame | Diagnostic/trust admission / launch-bound | Control/Rhai/composition; opt-in/capabilities/harness lifetimes remain explicit |
| fixed 20 TPS/.05 simulation dt; collision/math constants, light algorithm bounds | Structural/semantic invariant | runtime/game; no casual simulation-frequency change; no C1 knob |
| load/generation workers1 and queue4; save worker1/queue8; light admission8; mesh queue2×workers; checkpoint queue2 | Operational deferred RestartRequired | client/world/runtime; no safe pool drain/rebuild/backpressure protocol; RSM1 measures residency, future bounded migration on evidence |
| publication quantum1, core3×3 startup, 60s startup deadline; urgency/lookahead three-column cap and timeout quanta | Operational deferred / restart-bound | client/world residency; fairness/readiness invariant requires separate measured tuning; RSM1 baseline, do not expose internal algorithm constants blindly |
| Rhai operations/deadline/stack/string/array/map/source bounds; worker queues16, job limits128, scenario32K event bytes/128 history | Security/resource limits, launch-bound | scripting-rhai/control; constrained trusted executable boundary, not casually raised; no new privileges |
| DUX registry64, entity list64/query4096/overlay12, line cap64, text28×110/2600 and cache11 | Diagnostic resource bounds / launch-bound | Control/client/render; cost/safety contract; no broad scans/timer accumulation |
| render staging/HUD capacities, mesh allocation round-up/shrink thresholds, telemetry windows4096 | Implementation capacity/deferred restart | render/client; lifetime/caching policy RSM1, renderer optimizations M7; no new memory allocator |
| backend/adapter selection (WGPU_BACKEND), physical surface1280×720, surface present policy, redraw rate | Platform startup policy / deferred RestartRequired | wgpu/winit/client; environment owned by library, P1 owns presentation evidence/fixes; no frame limiter invented |
| resource-tool CLI atlas dimensions/limits/output, just benchmarks/recipes and test timeouts | Specialist launch parameters | content CLI/tooling/tests; reproducible workload inputs, DX2 proven-equivalence review |
| build-info env, CARGO/CI/toolchain/OS path env | Build/platform facts | build-info/toolchain/platform, not runtime settings or secret values |

The inventory was verified by repository-wide env/CLI/constant/duration/budget/queue searches across
client/server/runtime/world/render/content/render-profile/control/scripting-rhai and minecraft-b173.
No historical specialist workflow is deleted. RSM1/P1/S1/R2/A1/DX2 remain separate stages.

## UX1 text scale

`rustcraft:ui/font_scale`: engine text owner; Float 0.5..3, default 1; NextFrame, persist allowed,
graphical-only availability. Native renderer caches the effective value. Control/Rhai/Settings
use the same registry; invalid requests preserve effective scale. Pixel sizes are rounded and
positions snapped. Font-resource family replacement is startup/profile composition only, not a
second live settings system. UX1 increases engine registration from 16 to 17 settings.

P1 adds no presentation setting or second configuration owner. Existing
`rustcraft:renderer/present_policy=PreferFifo` remains startup-only; selected surface mode/capabilities
are reported explicitly. Timing uses effective diagnostic cadence and captures all effective C1
workload settings. Interpolation is a transient client mechanism, not a mutable simulation policy.

S1 introduces no operational setting or env alias. Benchmarks record C1 compiled-default effective
settings explicitly and use diagnostic size/cycle/source-rate arguments. They bypass autosave timing
to measure durable physical operations directly. Worker probes use the current composition's one
save worker/eight slots; these remain inventory-classified startup policy. No security/schema quota
is casually raised through C1 for the component evaluation.

## Current admission versus target F1 authority

Current `--survival` chooses game state and `--devtools` admits trusted diagnostic/Rhai tooling;
UX1 diagnostic keyboard routing is gated by tooling presence. These flags are not authenticated roles.
F1 will unify the normal runtime/input path while keeping session authority, principal, roles/grants
and game-owned gamemode distinct. Operational config source labels cannot grant capabilities; C1
remains the single typed settings owner. See [DEBUGGING](DEBUGGING.md) for current routing and field
validation, and [SECURITY](SECURITY.md) for trusted local versus server/sandbox grants.
