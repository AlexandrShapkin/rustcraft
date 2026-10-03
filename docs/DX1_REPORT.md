# DX1 developer control and scripting acceptance

Status: **CLOSED — mandatory DX1 acceptance satisfied**.
Starting public SHA: `c459f218ab389d365554a1fd4a8cf5b7b88a3eb1`.
M4 remains functionally complete, with representative-hardware evidence conditional under M4-009.
M5/M6/M7 remain inactive. No product version change, tag or release is part of DX1.

## Architecture and implementation

| Requested end-report item | Final implementation / evidence |
| --- | --- |
| 1–2. Baseline / status | Accepted c459f218 baseline preserved. Functional implementation complete; local and public Ubuntu/Windows acceptance pass; DX1 closed. |
| 3. Ownership | control owns mechanism; scripting-rhai owns interpreter/worker/editor/scenario composition; minecraft-b173::control owns first-party registration/legacy adapter; client::devtools/server compose; render owns transient lines and GPU mapping. |
| 4. Control API | Version 1: Context/Source/capabilities, immutable semantic Snapshot, Action/Host, Registry/parser, FixedControl, Scenario/Predicate/Step, EventRing and real Jobs. No Rhai/Minecraft/render dependency in control. |
| 5. Rhai | Exact 1.26.1, sync, standard AST interpreter, no Grain. Native first-party simulation remains authoritative. |
| 6. Background compile/reload | One worker, 16-slot request/reply queues; file read/parse/AST/metadata validation off event loop. No thread per edit. Bounded pending-shutdown test passes. Small 4 KiB console REPL lines remain immediate and resource bounded. |
| 7. Stale publication | Script identity and request generations; latest requested generation only. Cancelled/stale replies cannot replace AST/handler or inject mutations. Alias paths rejected by async load; use canonical approved path. Race test queues two candidates before publication and proves last wins. |
| 8. Broken reload | Working AST/handler/hash/generation retained; candidate error exposed. Tests prove V1 result A, V2 result B, invalid V3 retains B. Reverting to working source clears candidate error. |
| 9. Jobs | JobRef used for compile/reload, cooperative scenario WaitJob, failure-bundle writes, capture-state writes and PNG writes. GPU mapping is coordinated on render owner thread. |
| 10. Retention/ownership | Pending/Complete/Failed/Cancelled, stable IDs and owners; console/internal reload/scenario compile/run/bundle/capture ownership. Retain at most 32 old terminal jobs plus bounded outstanding work; hard capacity 128. Cancel run-owned work; ignore late results. Required evidence completes before CLI exit. |
| 11. Queries | player/world/streaming/entities/entity(ID)/persistence/lighting/meshing/renderer/scripts plus tick/player_position/block_at. Block snapshot is 27 nearby semantic values; arbitrary available scenario block assertions use Host live query. Entities limited to 64 active entries. Unavailable domain = unit/null. |
| 12. Snapshots | Versioned runtime/player/world/streaming/entities/lighting/meshing/renderer/persistence/scripts-jobs. Client expensive domains at 250 ms. Headless does not fabricate renderer/worker metrics. Dirty generations distinguished from persisted checkpoint revisions. |
| 13. Pages | Overview retained; streaming/world/entities/lighting/renderer/persistence/scripts. Entities paginated eight per page via `/debug page entities N`. Text bounded to 20 rows/100 columns/800 characters; JSON retains complete bounded snapshot. |
| 14. Overlays | At most 64 transient boxes. Nearby streaming: Visible green, Safe yellow, desired-unready red, retained blue, outside grey. Entity magenta marker and blue owner-column outline; collision yellow; target white. No chunk/save geometry. |
| 15. Console | Same ConsoleInput/editor/submit path for winit and autonomous scenario. UTF-8, cursor, backspace/delete, history, completion, scroll, open/close tested. Actual window console capture and close pass. |
| 16. Sessions | Persistent REPL x=40 then x+2=42; new session x undefined. Script/command/scenario scopes isolated. Errors recover without destroying session. No global mutable Scope. |
| 17. Capabilities/security | DeveloperConsole/Script/Scenario/ServerAdmin/FutureChat. FutureChat factory stays read-only. Scenario can receive explicit declared Context; denial test proves no world.write escalation. Script commands get read-only plus declared capability. No raw host filesystem/network/process/environment/world/ECS exposure. |
| 18. Limits | 50k operations; 32 calls/global/function expression depths; 128 variables/functions/maps; 8 KiB strings; 1024 arrays/steps/actions; 64 KiB source. Execution deadline default 10 ms, overrides 1–50 ms; operations overrides 100–1m. Loop/recursion/data/deadline errors bounded. |
| 19. Paths | Canonical scripts/ roots, traversal/absolute-outside/symlink-escape tests pass. Regular files only; async aliases rejected. No OS-level sandbox claim or protection from another privileged host process replacing files concurrently. |
| 20. Native audit | Complete registration inventory in SCRIPTING.md. Queries are bounded copied values; actions/steps queue; reload builds a job step; callbacks bounded. Heavy file/compile/bundle/PNG work stays on worker; no registered blocking native call. |
| 21. Scheduler | Explicit Rust steps, one per event turn; no suspended AST, coroutine fiction, sleep or blocking receive. State/tick/frame/idle/job waits have 30 s step bound and 120 s run bound. |
| 22. Ownership/cancel | Scenario lease overrides Human/StreamAuto; semantic AgentIntent movement. All terminal states clear intent; abort cancels owned pending jobs and prevents future steps. Explicit teleport distinct. F10 and automation use same abort API. |
| 23. Pause/step | Fixed simulation pauses while render/events/streaming/persistence publication and maintenance continue. Shared FixedControl gate consumes exactly N ticks, max eight per turn and 1000 pending. Shared smoke proves paused player/tick across graphical frames, exact 1 then 3 ticks, resume. |
| 24–25. Shared source | dx_smoke.rhai passes headless and real llvmpipe GL client; semantic query/block mutation/checkpoint/exact-step assertions common. Graphical capability enables frame wait/capture; headless has no GPU dependency/PNG requirement. |
| 26–27. Bundles/capture | Automatic assertion failure result + state/player/entities/available domains/scripts/jobs/logs/recent events, then frame.png. Intentional graphical failure exits 1. DX GPU mapping polls without Wait; CPU RGBA copy on render thread; PNG/job writes background. Legacy render captures unchanged. |
| 28. Event ring | Maximum 128 events/32 KiB, messages 512 characters. Command/scenario/step/compile/stale/job/capture/errors, no per-frame engine-event flood. |
| 29. Soak | 40 repeated scenario pass/cancel, compile and bundle cycles; pending purposes drain, <=2 script identities/ASTs, one scenario, bounded terminal jobs/output/events. Additional 300-job/editor bounds check passes. |
| 30. Script benchmark | Final recipe result recorded below; reports compile/query/dispatch/step/runaway behavior. No synthetic-score optimization. |
| 31. Overhead | Real service measurements for disabled/inactive/page/overlay/scenario below; software GPU frame cost excluded from DX service timing. |
| 32. Generic boundary | sample-game dependency guard and game-neutral control/Rhai query pass. Server adds no winit/wgpu/render dependencies; sandbox-test remains Minecraft/runtime independent. |
| 33. Hygiene | cargo machete clean after removing two unused DX dependencies. cargo audit: no vulnerabilities, three unmaintained warnings (paste/smartstring/ttf-parser); no random version changes. No deny policy exists, so cargo deny not used. |
| 34–35. Local/M4 gates | Full local matrix passes; old thresholds and canonical v1/v2 hashes unchanged. |
| 36–39. Commit/main/CI | Implementation/main SHA `008fbac252e4307af5146cda8e033eee3079bcc3`; [public CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37156846278): Ubuntu green, Windows green. Documentation closeout follows as a separate commit. |
| 40. Worktree | Writable HTTPS publication checkout clean; validated source matches published DX1 byte-for-byte. Mounted original .git remains stale/read-only; source comparison leaves only the pre-existing unrelated reference-study skill frontmatter, preserved and excluded. Generated outputs remain ignored. |
| 41. Mandatory gaps | None. Mandatory implementation, local, graphical and both-platform public gates pass. |
| 42. Optional breadth | More game commands, new command-file discovery, multiline editor/debugger, persistent script-state migration, remote tooling and future WASM adapters. Not required for current bounded tooling contract. |
| 43. Persistence observation | Owner intentionally deleted saves between manual runs; revision reset is not recorded as defect. Normal M4 persistence regressions retained. |
| 44–45. Scope | No version/tag/release. M5/M6/M7, frame pacing, residency audit, persistence redesign, texture normalization/config registry/legacy retirement not started. |
| 46. Post-closure inventory | Not performed; no next-slice implementation. |

Engine commands: help, commands, script, scenario, reload, inspect, capture, debug, pause, step,
resume. First-party minecraft-b173 registrations: tp, setblock. Command metadata includes semantic
ID, aliases, usage/help/capability; parser supports bounded quoted/escaped UTF-8 tokens. Completion
is cached/bounded for commands/aliases/files/debug names/semantic block IDs. Both slash/Rhai mutate
through control::execute -> Host -> existing Game API CommandBuffer/runtime dirty/lighting/render/
persistence tracking; direct/slash/Rhai equivalence test verifies effects.

Project-owned scripts: dev/inspect_player, commands/where, scenarios/dx_smoke, dx_console and
dx_responsive. Native REPL/commands are trusted local opt-ins, not the future untrusted mod runtime.
No normal startup autoexec, filesystem imports, downloadable native execution or hidden developer
privilege. RhaiRuntime owns Engine/config/bridge; RhaiSession owns Scope/identity/diagnostics.
Command-file convention is command_spec() + command(args), atomic code reload with stable metadata.
Reload keep_scope/reset_scope is explicit. API v1 is discoverable, not promised frozen 1.0.

The bounded fixed-step/block/capture workflow is migrated to common scenario/control primitives.
Long M4 client-stream-auto/world-travel/render/worldgen acceptance modes remain independent intact.

## Actual graphical and headless acceptance

2026-10-04 final real-surface runs (llvmpipe GL; AMD auth unavailable in agent environment):

| Run | Result / exit | Result file | Screenshot bytes |
| --- | --- | --- | --- |
| Shared headless smoke | pass / 0 | target/test-runs/scenario/4-1791064403343841354/result.json | unavailable |
| Shared graphical smoke | pass / 0 | target/test-runs/scenario/6-1791064405333550178/result.json | 823729 |
| Real console/editor/REPL | pass / 0 | target/test-runs/scenario/35-1791064408669936467/result.json | 793621 |
| Compile/job/capture/abort responsiveness | cancelled / 1 | target/test-runs/scenario/64-1791064412278723640/result.json | 814423 |
| Disposable assertion failure | fail / 1 | target/test-runs/scenario/93-1791064418782795171/result.json | 753133 |

Console screenshot visually inspected: normal world and actual developer text/output present.
Responsive scenario executes twelve reload job waits, overlays/capture, then long frame wait;
automation abort at frame 100 exercises the shared abort path. Snapshot at frame 98 recorded
DX update 857 us, maximum 3960 us in release profile including
startup/publication/capture coordination. Frames/events continued; no multi-second synchronous
compile/reload stall. Under concurrent full-workspace CPU load the 10 ms interpreter deadline
correctly rejected one scenario build; isolated final acceptance passes. Ordinary steady-state
measurements below are separate. Terminal result/state/scripts agreement was verified for all
five final runs. Expensive engine domains retain their documented 250 ms cadence.
Observed bundle sizes: headless 7688 bytes; graphical intentional failure 777924 bytes (frame dominates).
Disposable source removed; all output stays ignored under target/.

## Steady-state release tooling overhead

`just dx-overhead`: same real client/surface, 240 event turns per configuration, first 20 excluded.
Values are measured DX CPU service microseconds, including diagnostic cadence, not total GPU frame time.

| Configuration | Mean | p50 | p95 | p99 | Max |
| --- | ---: | ---: | ---: | ---: | ---: |
| Disabled | 4.31 | 3.84 | 5.31 | 5.73 | 42.60 |
| Enabled, inactive | 208.52 | 82.48 | 684.59 | 806.95 | 877.91 |
| Streaming page | 271.50 | 73.54 | 874.07 | 947.82 | 1161.82 |
| Streaming/collision overlay | 282.21 | 87.86 | 901.03 | 1085.13 | 1273.71 |
| Lightweight cooperative scenario | 871.23 | 827.84 | 1364.01 | 1534.50 | 1770.35 |

Inactive recurring work averages 0.209 ms here. Expensive diagnostic values/page formatting update
at 250 ms; cached vectors are reused, no filesystem/registry scan on keystrokes, no per-frame world
serialization. These are correctness/overhead diagnostics on available hardware, not M4-009 GPU
performance evidence or an unmeasured speedup claim. Capture/reload peaks reported separately.

## Final validation and publication

The requested local matrix passed: fmt; workspace/all-targets check; workspace tests;
all-targets/all-features Clippy with warnings denied; bootstrap-check; ci; smoke; survival-scenario;
sample-game; render-test-all; fidelity-m3; resource-stress 1000; render-scale; world-roundtrip;
worldgen-v1-regression; worldgen-v2-test; worldgen-bench; persistence-bench; world-stream-bench;
world-travel-test; client-stream-auto; world-state-roundtrip; entity-persistence-bench; release-check;
script-check; scenario-headless; dx-smoke; script-bench; dx-test. CI's local nextest report is
256 passed, one skipped. Final bundle-consistency repair is covered by the 40-cycle test and
repeated workspace CI/graphical acceptance. No M4 semantics, thresholds or hashes changed.

Script-bench: Rhai 1.26.1; compile 1790 us; 1000 cached query executions 23907 us; 1000 command
dispatches 558 us; 1000 scenario steps 13470 us; runaway deadline termination 10060 us.
The same headless scenario also passes from a disposable working directory containing only
project scripts and no local owner assets. Release-check packages only five allowlisted authored
scripts and scripting documentation. Dependency audit/machete results are recorded above.

The existing workspace test gate now includes a server integration test that compiles all five
shipped scripts and executes the shared headless scenario via the actual CLI. This runs on both
Ubuntu and Windows without modifying workflow permissions. An initial publication attempt was
rejected because the OAuth token lacks workflow scope; the same checks were moved into the
existing test gate and the workflow retained unchanged.

Implementation committed and non-force pushed to main: `008fbac252e4307af5146cda8e033eee3079bcc3`.
[Public implementation CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37156846278) passed both jobs:
Ubuntu job 111301867154 and Windows job 111301866983. Their unchanged workflow runs format,
workspace/all-targets check, workspace tests (including shipped-script/CLI integration) and
all-targets/all-features Clippy with warnings denied. Mandatory DX1 acceptance is satisfied.
The documentation closeout commit records this evidence; it does not change executable behavior.
Final closeout/main SHA and its CI link are reported with the owner-facing console report because
a document cannot contain its own commit hash. No mandatory gaps remain. No tag/version/release
or subsequent milestone was created.
