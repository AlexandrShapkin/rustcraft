# RSM1 — Residency & memory lifetime

Status: **CLOSED** — local acceptance and Ubuntu/Windows implementation CI verified;
this documentation closeout must also pass both public jobs before final publication is accepted.
Starting main: `097195340437b16901ec067f19789c973855bbc6` (UX1 closeout).
Verified UX1 implementation run 37220087083 and closeout run 37220398319: Ubuntu and Windows
successful. DUX1/C1/UX1 remain CLOSED. P1 and all subsequent stages remain inactive.

## Ownership and shared observation

World resident columns/sections and simulation entities -> RenderWorld voxel/light snapshots ->
coalesced pending snapshots -> queued/running mesh jobs -> completed messages -> ready payloads ->
section/page GPU records -> vertex/index buffers. Lighting and persistence can temporarily pin
columns. Durable entity transfer markers and pickup receipts have acknowledgement-owned lifetimes.
Disk records are not RAM-resident entities.

The demand-driven shared `Residency` domain adds the semantic `memory` page to the existing DUX
registry. `residency()` in trusted Rhai, Control snapshots, human text and capture/failure bundles
read the same sampled value. Disabled/inactive consumers do not request this provider. Diagnostic
history contains bounded scalar JSON only, never resources or snapshot Arcs. Chunk inspection adds
active-lighting and cleanup-pressure eviction reasons. Existing semantic identities and capabilities
remain unchanged; this is not a separate profiler or a configuration system.

The ledger includes residency interests and actual storage; load/generation/initial-light work;
dirty/save/light/other blockers; active/durable entities, receipts, tombstones and frozen columns;
render snapshot counts/bytes; current generation metadata and map capacity; pending, submitted,
completed and ready mesh counts/logical/capacity bytes; stale/coalesced/cancelled work; RustCraft-owned
GPU mesh/page/buffer counts, logical/capacity vertex/index bytes and cumulative creation/reuse/retirement;
lighting source/cleanup/completion ownership; Linux RSS and explicit VRAM availability. Relevant
seven effective C1 values accompany samples; full configuration remains in existing capture evidence.

GPU retirement counts describe RustCraft ownership ending, not physical GPU completion or driver
allocation release. Driver staging and physical VRAM lifetime are unavailable through these counters.
UX1's one 2048×2048 RGBA8 text page is a fixed **16,777,216-byte global allocation**, separately
reported from world mesh bytes; the resource atlas is also global. No text resource was redesigned.

## Mandatory pre-fix measurement

Instrumentation and diagnostic workloads were added before ownership logic changed. The instrumented
pre-fix patch and logs were retained locally under `/tmp/rsm1-prefixed-*`; raw graphical JSON is in
ignored `target/rsm1/`. The following results drove changes, rather than optimization hypotheses.

| Workload | Before | After / contract |
| --- | --- | --- |
| 10,000 unique dirty/removal section identities | 10,000 generation entries, capacity 14,336 | current entries 0, pending-order entries 0; capacity reuse allowed |
| 256 requests, polling continues but upload consumption stalls | ready 256, logical CPU payload 258,048 bytes | window 6 for 2 workers/4 queued: ready 6, pending 250; resume accepts all 256 |
| 1,000 saved/evicted entity owners | active 0, durable metadata 1,000, capacity 1,792 | active/durable/recovery/frozen logical counts 0; recovery references retained until ack |
| Stale light propagation after 100 physical evictions | 100 orphan light sections recreated | light/orphan sections 0 |
| Async integration/removal of 50 unique columns | completion metadata 50 with live columns 0 | completion metadata 0 |
| Zero-work cleanup between queued removals | no active work, queued 1, unfinished true | queue empty, unfinished false |
| Revisit before old cleanup executes | resident column's new direct source deleted | source remains 15; obsolete cleanup cancelled |

An initial synchronous lighting probe consumed completions itself and did not reproduce completion
history; the corrected async probe did. These are distinct tests, not contradictory measurements.
Stopping result polling alone was already admission-limited by submitted bookkeeping. The actual
unbounded path was polling into `ready` while uploads stopped; an unbounded channel type alone was
not proof of an unlimited production result backlog.

The final pre-fix production route receipt was
`target/rsm1/140-1791138706501836096/samples.json`. Warm-up: 49 columns/392 sections/392 generation
entries. First settled distant points: 56 columns/448 sections/1,504 metadata, then
55 columns/440 sections/2,016 metadata. Return to origin failed its bounded settle timeout:
88 resident columns, 34 clean outside-retain columns, 704 render/GPU section records, generation
entries 2,136, 43,984,080 logical GPU bytes and 68,092,928 capacity bytes. Dirty/save pins were zero.
This is a **failed baseline**, not a completed pre-fix five-cycle plateau.

After the first metadata/entity/orphan-light fixes, an intermediate route still failed with
126 columns/1,008 section records, 70 clean outside-retain columns and no active lighting work,
while queued cleanup remained. That measurement isolated the zero-work queue progression defect;
no dirty pin was bypassed to reduce these counts.

## Narrow correctness fixes

1. Mesh work uses a checked globally monotonic u64 token and a current-section token map. Removal
   deletes current metadata and pending/ready work. Revisit never reuses the old token. Results are
   checked both when polled and at upload handoff. Token exhaustion fails explicitly rather than wraps.
2. Admission covers **submitted-unconsumed + ready**, not only the worker queue. Completed messages
   remain charged to submitted ownership until polled. Sends stay nonblocking, avoiding result-channel
   shutdown deadlocks. Pending work still coalesces per live section and useful work resumes after a stall.
3. Job and message RAII account for snapshot and completed payload ownership, including channel/drop
   paths. No accounting object retains the measured payload beyond its actual owner.
4. Stable saved entity-owner metadata retires on eviction. Active entities, transfer tombstones and
   pickup receipt references retain it until existing acknowledgements permit removal. Disk schema,
   stable EntityId, save ordering and recovery rules are unchanged.
5. Propagation refuses physically nonresident columns; legitimate sky/light-only sections in resident
   columns remain supported. Re-publication cancels obsolete queued cleanup and replaces direct seeds.
   Cleanup retires completion metadata and drains synchronous zero-work jobs until real work starts or
   queues empty. Queued integration for physically absent columns is discarded as obsolete.

No GPU allocator/pool rewrite, forced shrink-to-fit, renderer rewrite, greedy meshing, presentation
change, persistence-format change or general optimization was introduced.

## Structural pressure bounds

For W bounded workers and Q worker queue slots, the result window is **W + max(Q,1)**. Production
3+6 gives 9. Running, queued and completed-unconsumed jobs share this window with ready results.
A completed message is a subset of submitted ownership, not another independent 9-slot allowance.
Pending is one coalesced snapshot per current section; old submitted jobs may temporarily retain
snapshots after eviction but at most the window permits. RenderWorld owns one snapshot per live section.
Metadata scales with live sections and unfinished work, not historical unique travel.

The existing emitter has at most 16³×6 quads per result. Four vertices and six u32 indices per quad
yield **4,128,768 logical bytes/result**. Conservative Vec payload/page capacity envelope:
**11,010,048 bytes/result**. The production window therefore allows at most **37,158,912 logical**
and **99,090,432 capacity bytes** of completed/ready payload. These are mesh payload envelopes,
not all HashMap overhead or mesh-building temporary allocations. A large checkerboard fixture also
checks capacity and forward progress; tiny single-block meshes are not the only pressure evidence.

Lighting retains current source columns plus retired-but-unfinished cleanup. Existing 256 work slots
bound queue admission; this may legitimately delay clean eviction under pressure. Final stop/drain
checks retired sources and queues, rather than requiring them to vanish at every moving frontier.
No unsaved data is dropped to meet a bound. Normal retain radius 4 implies at most 81 columns plus
one active-lighting pin at settled samples. Persistent failures may legitimately retain additional
columns and are surfaced as pins, not classified as leaks.

GPU logical bytes describe occupied vertices/indices; capacity follows existing buffer reuse/shrink
rules. The campaign checks current capacity ≤ 4×logical payload + 512×page count. Container map
capacities can remain at exploration high-water; no per-eviction shrinking is required.

## Reproducible graphical campaign

`just rsm1-client` uses explicit devtools and a nonce disposable world with effective C1 load/retain
radii 3/4, light work 256, streaming main budget 8 ms and upload sections 64. Upload bytes remain
8 MiB and shared diagnostics 250 ms. It uses production streaming, extraction, async meshing,
upload and render removal, with checked Safe/render-ready one-column Control hops at height 96.
The simulation is paused to avoid terrain/controller accidents; streaming services remain production.

A, B, C, D are 10 columns apart. Warm-up A; five B→C→D→A cycles; eight additional unique points;
return to A after pressure/exploration; full lighting drain; actual capture job completion. This is
380 checked hops and 31 scalar samples. Native diagnostic coordination is used because the campaign
exceeds Rhai's existing execution deadline; no scripting safety limit is raised.

Intermediate hops require destination Safe and render-ready. Endpoint settle requires a complete
3×3 Safe/Visible core; no mandatory load/generation/initial-light work; mesh pipeline and dirty
extraction arrays empty; dirty saves acknowledged; outside-retain ownership at most the active-light
pin. Bulk-lit Safe columns may still undergo bounded background boundary reconciliation. Final drain
requires no lighting integration/cleanup and requests a fresh shared sample, avoiding cached-provider
staleness at the transition. Overall bound 1,800 s, no-progress bound 300 s. Failure records phase,
location, last ledger, configuration and bounded recent events; success waits for the real capture.

The first successful moving route (29 samples, 290 hops, 441.2 s) established the following envelope
before the final return/drain/capture checkpoint was added:

| Cycle | Columns B/C/D/A | GPU pages B/C/D/A | GPU logical MiB B/C/D/A | RSS MiB B/C/D/A |
| --- | --- | --- | --- | --- |
| 1 | 57/57/57/56 | 346/344/336/292 | 27.09/27.61/27.15/28.96 | 353.34/362.84/365.96/427.84 |
| 2 | 56/56/56/56 | 341/339/331/254 | 26.27/27.46/24.11/17.24 | 418.19/419.75/305.62/292.91 |
| 3 | 56/56/56/56 | 326/339/331/292 | 19.78/27.49/26.91/29.75 | 294.10/294.48/294.64/299.66 |
| 4 | 56/56/56/56 | 341/339/331/292 | 26.52/27.33/26.91/29.06 | 299.66/299.66/299.66/299.66 |
| 5 | 56/56/56/56 | 341/339/331/292 | 26.40/27.42/26.61/29.59 | 299.66/299.66/299.66/299.66 |

All eight unique endpoints had 56 columns/448 render sections/448 generation entries, zero orphan
light sections and empty mesh queues/job snapshots. GPU capacity varied 25.37–45.04 MiB rather than
increasing every cycle. RSS settled near allocator high-water; it is supporting evidence, not an
ownership threshold. The structural bounds above, exact current section agreement and absence of
per-cycle logical growth are the acceptance basis, not an invented percentage tolerance.

The final campaign passed in **697.499 s**, 380 hops/31 samples, receipt
`target/rsm1/14-1791142208614583503/samples.json`. The actual capture completed at
`target/captures/rsm1_final-14-1791142208354095684/` (frame/state). Screenshot review confirms the
Memory page is readable and bounded inside its background; displayed sample age is explicit. The
capture can show the immediately preceding UI sample while the receipt records the fresh drain.
A bounded machine-readable before/after summary is [RSM1_RESULTS.json](RSM1_RESULTS.json).

Final run cycle counts: first cycle 57/57/57/56 columns, all subsequent cycles 56/56/56/56.
Render/world/GPU section records and current tokens agree at all 31 samples; phases 4 onward have
448 current sections, 448 render snapshots and 23,514,624 snapshot bytes. All endpoint mesh queues
and job snapshots are empty. GPU pages oscillate 291–345 in cycle 1 and 292–341 in cycle 5;
logical bytes are 24.64/25.29/24.61/27.63 MiB in cycle 5, capacity 37.82/37.20/36.50/42.28 MiB.
The unique endpoints remain 56/448/448. Final drained state: 56 source columns, zero retired sources,
zero queued/active lighting, zero orphan light sections, zero dirty/save/other eviction blockers;
448 GPU section records, 288 pages, 576 buffers, 30,093,168 logical and 47,604,224 capacity bytes.
Created buffers 32,650 minus retired 32,074 equals 576 live. No ever-visited metadata remains.

RSS in final cycles 3–5 is approximately 302–380 MiB, with no monotonic cycle-by-cycle increase.
After unique exploration and the long return/drain it reaches **1,034,629,120 bytes** despite the
same bounded logical live state. This increased high-water is reported rather than hidden. Its
allocator/backend/deferred-driver breakdown is **unidentified**; the ledger does not prove a cause
for RSS or physical VRAM. Logical owned resources still satisfy their independent bounds. The
first accepted moving run and final run differ in system activity and allocation history; RSS is
not an invented leak threshold or representative hardware target.

## Focused correctness and regressions

`just rsm1-test` runs meshing tests, runtime RSM tests and shared client ledger/demand tests. The
late-result barrier test removes/revisits while an old worker is inflight, rejects the old token,
accepts the new mesh, then rejects a stale ready payload at upload handoff. Removal while pending,
10,000 unique identities, stalled polling, stalled uploads, large payloads and drop/shutdown accounting
are also covered. No blocking result producer can deadlock worker join. New light tests cover
physical eviction, empty cleanup progress, completion retirement and revisit seeds.

Existing world stale/failed save-generation, radius pin and persistence tests retain dirty ownership
on failure and only release after acknowledgement. Entity tests preserve transfer cleanup references,
pickup receipt ordering and both reload orders. Production travel verifies edit/evict/reload, ten
entity owner columns, cross-column motion, negative coordinates, returning to origin, graceful
player/world checkpoint reopen and unchanged frozen semantic hashes.

Final validation includes format, workspace all-target check/test, strict all-feature Clippy, `just ci`,
DX tests/console/overhead, independent sample game, C1 headless/graphical, DUX/UX graphical,
world-stream-bench, world-travel-test and render-camera-motion. DUX scenario's registered-view count
is updated from 18 to 19 for the Memory registration; test registry limits derive from metadata.

## Cost and telemetry interpretation

Collection scans are bounded by current resident state and occur only on demand/cadence. One
81-column shared test proves 1,000 disabled and 1,000 inactive requests perform zero collections;
1,000 active forced samples collect 1,000 times; UI/Rhai read the same value without another collection.
Final cost and overhead numbers are diagnostic observations, not universal performance targets.

Linux RSS uses existing `/proc/self/status` VmRSS sampling. Other platform provider absence is explicit.
The optional existing Linux DRM sysfs counter is **device-wide**, not RustCraft process VRAM, and
never substitutes for the unavailable process field. Here the adapter is llvmpipe LLVM 23.1.1/OpenGL
on a real Wayland graphical surface. AMD hardware and trustworthy process VRAM are unavailable.

The owner's driver-reported VRAM causality therefore remains **not measurable here**. Proven
RustCraft logical retention was independently reproduced and corrected, including clean world/render/
GPU ownership growth under stalled lighting cleanup. Allocator/container high-water is separately
observed. Neither driver caching nor the fraction of the owner's observed VRAM attributable to these
bugs is established by these measurements. Driver VRAM returning to startup is not required.

## Deferred and public acceptance

No P1/S1/R2/A1/DX2/READY1/M5/M6/M7 work, version bump, tag or release. Representative-hardware
M4-009 evidence remains conditional. Process-specific GPU telemetry, physical fence retirement,
allocator internals and workload-wide optimization remain deferred. PM5-002 is **resolved** by the measured fixes and verified implementation acceptance.

## Final local gate evidence

Latest `just ci`: 296 tests passed, 1 existing skipped; formatting, all-target checks and strict
all-feature Clippy passed. Explicit `cargo test --workspace` also passed. Focused mesh/light/entity,
world delayed/stale/failed-save pin and shared demand tests passed. DUX, UX and C1 real-surface
regression receipts report `pass`; headless C1 smoke passed. DX tests/console/overhead and independent
sample-game passed. Final world-stream-bench: 91 centers, 734 evictions, 81 save-before-evict,
negative coordinates and separated revisit edits correct. Final world-travel-test: PASS, fresh/reused
peaks 100 columns/800 sections; returned origin, ten entity column evict/reloads, edit/reload and
player/world reopen correct. All three frozen semantic hashes unchanged. Render-camera-motion:
eight phases; reuse cycles 3 with zero extra allocations/reallocations and 264 reuses.

| Shared 81-column provider test (1,000 turns) | mean µs | p50 | p95 | p99 | max | Residency collections |
| --- | --- | --- | --- | --- | --- | --- |
| Disabled | 4.280 | 4.749 | 5.238 | 5.378 | 39.390 | 0 |
| Enabled, domain inactive | 4.238 | 4.749 | 5.238 | 5.308 | 38.692 | 0 |
| Forced active sample | 206.449 | 207.706 | 256.664 | 408.569 | 928.114 | 1,000 |

The UI/Rhai shared-read proof leaves the collection count at 1,000. This debug-build forced-sample
micro-workload excludes rendering and does not measure a universal latency target.

The real graphical DX overhead probe uses 220 retained event-turn samples/mode. Disabled service:
mean 5.55 µs, p50 4.26, p95 5.24, p99 7.68, max 248.83. Overview with Residency inactive:
mean 957.85 µs, p50 106.92, p95 4,422.13, p99 5,158.56, max 6,596.36, zero Residency collections.
Memory page active: mean 503.86 µs, p50 270.90, p95 1,688.74, p99 2,763.33, max 4,643.36;
62 Residency collections across 240 turns at the shared cadence. These include broader DX service
and text work and concurrent system activity; differing pages are not an isolated speedup comparison.

`cargo machete` clean; `cargo audit` no known vulnerabilities, three unchanged allowed unmaintained
warnings: paste RUSTSEC-2024-0436, smartstring RUSTSEC-2026-0249, ttf-parser RUSTSEC-2026-0192.
No dependencies, lockfile, font resources or third-party licenses changed. Server remains headless.


## Public acceptance

Implementation: `e3117949d2d90473ae4be3263b24fc04867bbc19`, **Bound residency and mesh lifetime**.
[CI run 37228702009](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37228702009):
Ubuntu **success**, Windows **success**, including formatting, check, workspace tests and strict Clippy.
The documentation closeout records RSM1 CLOSED and PM5-002 resolved. Its final Ubuntu/Windows CI
is monitored separately before the final report; no further stage starts during that wait.

PM5-002 closure rests on current-only metadata, bounded payload pressure and forward progress,
late-result rejection, five-cycle and unique-exploration logical bounds, render/snapshot/GPU ownership
agreement, drained light cleanup, preserved dirty/recovery pins and clean shutdown accounting.
It does not depend on a flat driver-VRAM graph or RSS returning to startup.
