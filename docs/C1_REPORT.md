# C1 runtime configuration acceptance

Status: **CLOSED**.
Starting main: `f8fb1778805761bd65ff3a05f686880877a2b1ea`; origin agrees, and both jobs in
[baseline CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37174552527) are green.
The original dirty mounted checkout is preserved; work uses the clean writable publication clone.
M0–M4/DX1/DUX1 remain closed. No RSM1/P1/S1/R2/A1/DX2/READY1/M5/M6/M7 implementation is authorized.

## Architecture and configuration contract

`rustcraft-config` is a leaf depending only on existing serde/serde_json/atomicwrites. Composition
owns the registry explicitly through ControlState; no global singleton/lock. Native consumers cache
validated typed values at boundaries. The startup worker receives the resolved configuration clone,
not another environment/CLI parse. Control reexports the core and owns generic commands/Settings
presentation; Rhai is an adapter. No Minecraft branch is added to the generic mechanism.

Graphical composition registers 16 settings: 14 operational policies and two immutable facts.
The sandbox independently registers/applies its package-owned boolean. Bool/integer/float/text,
enums, bounded millisecond durations and byte quantities have semantic keys, defaults/ranges,
requested/effective/source/provenance/owner/description/persistence/change/availability metadata.
See [CONFIGURATION.md](CONFIGURATION.md) for the definitive migrated/alias/deferred inventory.

Precedence is default < startup user file < environment < CLI < runtime. CLI `--set-config KEY=VALUE`
can repeat; `/config reset KEY ...` removes runtime layers atomically and reveals startup precedence.
One outstanding batch, at most 16 keys, validates before changes; coupled load/retain and mixed-boundary
requests cannot partially apply. Immediate acknowledges synchronously, NextTick at fixed steps,
NextFrame at event-frame turns. Requested is pending until native success. Apply/prepare failure
reverts request and leaves old effective state. Generic Reconfigure success/failure is tested with
an independent provider; production workers remain RestartRequired, with no unsafe live resizing.
ImmutableAfterOpen returns a structural reason. Restart requests do not silently resize pools.

The existing prefer-FIFO/first-supported fallback presentation policy is introspectable; no present
mode/redraw/interpolation changes. Actual selected mode stays in Renderer diagnostics, distinct
from selection policy. Identity/schema/generator/compiled registry changes stay protected/deferred.

## Adapters, persistence and security

Control `/config list|get|describe|set|batch|reset|persist|unpersist|select`, key completion and help;
Rhai `config_get/config_describe/config_set/config_reset`; native consumer fields and DUX Settings
all read one registry. Configuration reads are immutable snapshots; writes use privileged actions,
validation and boundary acknowledgement. config.read/write/persist capabilities are separate;
FutureChat/read-only cannot mutate/persist. ServerAdmin source labels do not authenticate.
No raw filesystem/network/process Rhai grants or executable configuration are introduced.

F4 Settings uses the existing selector/focus model, Left/Right semantic selection, +/- adjustment,
R reset and existing console for exact/batched values. Effective/requested/source/type/range/owner/
policy/description/status are visible; unsupported consumers and errors are explicit. There is no
UI-owned configuration copy. Settings has no broad diagnostic-provider demand.

Version1 tagged JSON, maximum 64 KiB, is separate from worlds: Linux XDG_CONFIG_HOME/HOME .config,
Windows APPDATA, or explicit --config-file/RUSTCRAFT_CONFIG_FILE. Runtime writes never auto-save.
Explicit persist/unpersist edits only the next-launch file; current session startup layers remain
unchanged. Atomic replacement/flush reuses existing atomicwrites, with failed-write preservation
proof. Coupled values persist together and are checked against next-launch defaults. Unknown keys,
malformed source/key values and denied persistence fail clearly. No secrets are intended as values.

Headless server/bootstrap/scenarios compose the same core with native residency and demand cadence.
Graphical upload/presentation, client-only checkpoint and lighting/lookahead work consumers are
marked unavailable when absent; no nonexistent network-server autosave service is fabricated.
Client/server --config-report needs no graphics. server --config-smoke exercises actual shared
Host/Control mutation/readback/reset and negative-coordinate residency planning. Server's dependency
tree remains free of render/wgpu/winit. Specialist stream-bench worker alias also resolves centrally.

## Acceptance evidence

Focused tests pass: type/min/max/invalid enum/unknown/immutable/restart validation; ordered startup
sources and reset; actual environment aliases; pending/boundary publication; atomic radius pair;
generic reconfigure failure/retry; duplicate/closed registration; file write/reopen/CLI override/
explicit removal/unknown/malformed/forced atomic-write interruption; bounded 128-change history.
Native tests grow/shrink radii and preserve tokens/negative-coordinate/save pins, apply work budgets
without discarding queue state, and keep checkpoint clocks/dirty flags/revisions. Shorten then
lengthen player/world autosave while changing state; persisted records reopen and graceful flush
preserves the last change. Existing storage formats/frozen hashes are unchanged.

The client adapter proof changes cadence through actual DevTools/Rhai, then reads native cadence,
Control Snapshot, console get, Rhai get and Settings text. All see 50 ms; no broad provider is collected.
Read-only native and FutureChat Rhai mutations are rejected. Sandbox owns its own semantic setting.
Config metadata and bounded structured change history are included in captures/failure bundles;
important boundary changes enter DX's bounded event ring. Inactive native guards are constant-time;
string lookup/parsing/I/O/metadata collection do not enter voxel/entity loops.

Final real graphical C1 receipt `target/test-runs/scenario/650-1791089188175602843/result.json`:
pass, 3.834 s, tick 13. DUX graphical regression receipt `707-1791089193585885619`: pass.
Captures show Settings at 300 ms Runtime then rejected 1 ms with effective 250 ms preserved.
Pending radius 6/effective 4, subsequent native 6/7 then 3/4 and reset; upload budget change/reset;
checkpoint interval change/reset; selector/console handoff and resumed gameplay all pass.
Real 1280×720 llvmpipe OpenGL surface (LLVM 23.1.1), not owner AMD or representative GPU performance.
Disposable PID/nonce world; local captures/assets are ignored, proprietary pixels never committed.
Headless smoke passes with 16 settings, native/Control readback, reset, invalid preservation and
negative coordinates. A real server subprocess test covers user-file/environment/CLI precedence.

## Measurements and final gates

Diagnostic service measurements: 240 event turns per mode, 20 warmup turns excluded; microseconds,
not physical display timing. Whole-service polling/snapshot work is included.

| Mode | Mean µs | p50 | p95 | p99 | Max |
|---|---:|---:|---:|---:|---:|
| disabled | 4.28 | 3.91 | 5.31 | 5.59 | 59.30 |
| overview | 107.69 | 70.26 | 483.45 | 634.79 | 671.95 |
| low_page | 120.60 | 74.73 | 510.26 | 641.78 | 820.36 |
| high_page | 142.02 | 77.66 | 560.62 | 787.05 | 1437.14 |
| overlay | 159.62 | 75.29 | 701.56 | 933.51 | 1191.08 |
| scenario | 947.64 | 916.61 | 1315.05 | 1687.10 | 2146.87 |

Disabled/Overview collect zero broad domains. Lighting collects 30, Renderer 32, overlays 34 while
active; each previous domain collects zero after demand ends. Overlay geometry returns to zero.
Settings adds no broad-domain demand; history is capped at 128, registration at 64 and batches at 16.
Optimized core: 10,000 inactive guards 10.616 µs; 10,000 semantic control lookups 702.471 µs;
200 request/apply transactions 542.445276 ms including bounded history/metadata rebuild.
These rare control-path costs are not per-voxel/entity work and are not a universal target.

Final fmt, workspace all-target check/tests, strict all-feature Clippy and `just ci` pass:
277 tests passed, one optional skipped. `dx-test`, `dx-console`, `dx-overhead`, `sample-game`, C1
headless/graphical and DUX graphical pass. Native autosave tests cover shorten/lengthen while dirty,
reopen and graceful flush; full workspace includes player/entity/world-state persistence coverage.
`world-stream-bench` passes (91 centers, peak resident 18, save-before-evict 81, negative/revisit edits 2);
`world-travel-test` passes negative/revisit, edit/entity eviction/reload, distant-player/world-time reopen.
Frozen semantic hashes and thresholds are unchanged. No full unrelated historical matrix was rerun.

Cargo machete finds no unused dependencies. Cargo audit finds no known vulnerabilities, with three
unmaintained warnings: paste (RUSTSEC-2024-0436), smartstring (RUSTSEC-2026-0249), ttf-parser
(RUSTSEC-2026-0192). Existing serde/serde_json/atomicwrites are reused; no dependency version update.
Normal server dependency tree remains free of render/wgpu/winit. Implementation `a3554807a20dc4fff6d9878cb018659c23b1a7b2`: Ubuntu and Windows green in [CI run 37178493694](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37178493694).

## Deferred scope and stop

Only documented startup/specialist/resource/security/structural policies remain outside the plane;
see CONFIGURATION inventory with owners/reasons/future stages. Polished settings/editor/mod UI,
hot pool resizing, general resource reload, persistence backend, GPU lifetime/queue/eviction fixes,
presentation timing/interpolation and external Agent/Bot identity migration are deferred.
DUX1 remains CLOSED; its added Settings registration raises graphical views 17->18 without a second
backend. C1 local acceptance and both public platform jobs are green. C1 is CLOSED; RSM1 remains inactive.
No version bump, tag or release is part of this pass.
