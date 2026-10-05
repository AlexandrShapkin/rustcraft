# Performance strategy

## M4-004 frozen overworld v2 measurements

Release diagnostics use seed 731173 and the same 16-column `[-2,2)` canonical region, four existing
generation workers and bounded admission. The final validation run measured v1 worker CPU at
0.689 ms/column and v2 at 2.378 ms/column (11.030 / 38.049 ms total worker time). Wall time including
semantic hashing and queue collection was 73.714 / 80.043 ms: 217 / 200 columns/s is
**diagnostic throughput**, not pure generation
throughput. V2 remains single-digit milliseconds per column without another pool or world caches.
Stage timers wrap stages, not voxel operations. Temporary storage is eight dense sections plus
small 16x16 height/biome arrays; cave halo four and ore/tree/lake halo one are travel-independent.

Seven fixed seeds sample 2401 columns each (radius 24, 784x784 horizontal cells):

| Seed | Height min/p05/p50/p95/max | Mean | Land fraction | Touched tree/lake origins | Cave blocks |
| --- | --- | --- | --- | --- | --- |
| 0 | 42/47/58/73/79 | 58.95 | .257 | 1467/122 | 303312 |
| 1 | 48/55/67/74/79 | 66.43 | .717 | 4298/464 | 376932 |
| -1 | 44/50/63/71/76 | 61.94 | .390 | 3570/204 | 355390 |
| 2147483647 | 48/55/64/71/76 | 63.67 | .464 | 1437/177 | 354965 |
| -9223372036 | 45/51/61/80/89 | 62.60 | .365 | 1587/157 | 321278 |
| 8675309 | 57/61/67/74/80 | 67.43 | .784 | 4678/342 | 379569 |
| 731173 | 52/57/64/70/74 | 63.65 | .420 | 2536/125 | 398876 |

Origin counters count each destination touched, not distinct global features. Aggregate land is
.485, mean macro height 63.52, and all biomes occur: approximately ocean 38.4%, beach 17.2%,
forest 17.2%, plains 16.9%, desert 7.8%, hills 2.6%. An oceanic origin is valid, not globally
collapsed terrain. Seed 0 startup relocates to (11,-11), seed -1 to (-10,9). Per-seed ore counts
are roughly coal 105k–123k, iron 209k–238k, gold 22k, diamond 9.8k–10.1k, with exclusive upper
Y bands 120/64/32/16.

Biome fractions over the same fixed samples (percent; macro biome, not observed water cover):

| Seed | Ocean | Beach | Plains | Forest | Desert | Hills |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 66.83 | 10.01 | 9.89 | 8.21 | 3.69 | 1.35 |
| 1 | 20.86 | 10.42 | 30.35 | 26.23 | 8.19 | 3.94 |
| -1 | 45.18 | 20.61 | 6.84 | 26.24 | 0.25 | 0.88 |
| 2147483647 | 34.59 | 24.95 | 18.01 | 8.70 | 13.04 | 0.71 |
| -9223372036 | 55.35 | 10.41 | 8.57 | 8.13 | 8.26 | 9.29 |
| 8675309 | 10.32 | 15.77 | 31.89 | 27.44 | 12.88 | 1.69 |
| 731173 | 35.51 | 28.12 | 12.67 | 15.68 | 7.96 | 0.05 |

Canonical-seed radius-24 stage totals for 2401 columns: climate 697.754 ms, terrain 653.556,
surface 751.979, caves 709.477, ores 257.250, vegetation 145.131, lakes 8.022. These are release
diagnostics, not representative-GPU claims. V2 streaming sample (seed 731173, column (4,2), semantic
state encoding) is `da875570efd5ace7a12c73e883a458e7f5ebe95f21d45ef90841b72104cf5b8e`.
Historical v1 stream sample `384029af6b21ecf82326bbee468eee62c1353b0af70b9148a4947dee7192a807`
remains separately identified and is not the v1 canonical region lock.

Travel acceptance uses normal Survival physics and semantic move/view/jump intent. The v1-era
angle-only driver could fall into a static lake undercut and cannot assume swimming exists. The
v2 diagnostic checks the first surface below an intended step, uses a bounded 33x33 local voxel
path search around obstacles, and retraces a maximum 512-point, loop-erased physical breadcrumb
path on return. Neither terrain nor player/residency position is rewritten. Script state is
transient; the reopen test carries its route externally while restoring ordinary durable player,
entity and clock state. Surface runs stop at route convergence or a hard 360-second deadline.
Headless completion likewise requires idle presentation/residency work and a complete current
3x3 Safe+Visible core, not an empty outer boundary-light queue. A regression proves pending outer
lighting may coexist with that core, while removing any core mesh or Safe flag blocks completion.
Final seam/convergence remains separately covered by lighting tests and `world-stream-bench`.
Entity eviction evidence captures stable IDs at successful eviction of their **current** owner,
not at historical spawn-column eviction. Already reloaded entities inside retention are valid.

Four Survival routes passed fresh/reused travel: seed 731173 visited 35/26 columns, forest-start
seed -9223372036 visited 35/31, ocean-origin seed 0 visited 37/34, and large seed 2147483647
visited 29/28. All returned, successfully evicted and restored all ten tracked stable entities,
preserved edits/time/player state, and kept minimum forward Safe/Visible at 0.5/1.0 columns.
The canonical and large-seed routes crossed negative coordinates. Peaks across this suite were
103 columns / 824 sections. The final canonical rerun measured fresh request-to-visible
p50/p95/max 0.288/0.375/0.550 s and reused 0.301/2.316/2.488 s, with 100/101 resident-column peaks.
An explicit frozen-v1 compatibility route also passed: 34 fresh / 32 reused columns, forward
Safe/Visible minima 0.5/1.0, 100-column / 800-section peaks, all ten entities restored, and edits,
player and clock preserved. V1 request-to-visible p50/p95/max was 0.281/0.333/0.385 s fresh and
0.288/0.364/0.472 s reused. Its different dry spawn does not inherit the v2 canonical route's
world-space negative-quadrant assertion; version-independent negative generation tests remain.
The old 185-second graphical deadline gave only 41 seconds to return along a longer physical
trace; both graphical diagnostic recipes now allow a bounded 360 seconds, stopping early on
successful route/convergence. This changes neither movement speed nor acceptance assertions.
`client-stream-auto` defaults its explicit diagnostic window to 320x240 (override with bounded
`RUSTCRAFT_STREAM_WINDOW_SIZE=WIDTHxHEIGHT`); ordinary play and `stream-perf` retain 1280x720.
It also defaults scoped llvmpipe workers to two (`LP_NUM_THREADS` is overridable and irrelevant to
non-llvmpipe hardware drivers). Normal gameplay driver settings are not modified.
The same adapter, GPU rendering, surface presentation, simulation and streaming paths run at either
size. On llvmpipe/GL the 1280x662 drawable completed the v2 return but failed with 0.700 s of dropped
ticks and render/present-dominated frame p95/p99/max 167/195/221 ms. Lower diagnostic resolution
isolates software-rendering cost; it is not evidence of representative-hardware frame performance,
and the tick/margin/input acceptance thresholds are unchanged.

The isolated actual-client canonical v2 route passed on llvmpipe 23.1.1 / GL with X11,
`vblank_mode=0`, `LP_NUM_THREADS=2`, and the 320x240 diagnostic window. It visited 33 columns,
returned, released the startup core in 640 ms, and kept forward Safe/Visible minima at 0.5/1.0
columns. Request-to-visible p50/p95/max was 0.483/0.919/2.110 s; oldest critical-stage backlog was
4.699 s. Event-gap p95/max was 133.86/159.61 ms and frame p95/p99/max 135.18/145.34/158.63 ms,
dominated by measured software render/present waits. TPS was 19.50 with **zero dropped ticks**;
input-to-simulation p95/max was 97.39/131.19 ms and input-to-render 82.41/97.09 ms. These do not
establish hardware frame performance or a universal <100 ms event bound. The unchanged automatic
software-present attribution policy excludes measured external present waits, not streaming work.
Resident peak was 284 columns / 2272 sections, including bounded lighting/eviction transitions;
estimates were 79.9 MiB authoritative voxel/light, 113.7 MiB snapshots and 108.3/167.7 MiB logical/
capacity meshes. The smaller headless peaks above must not be substituted for actual-client values.

The same graphical profile passed ocean-origin seed 0: dry spawn column (11,-11), startup core
767 ms, 35 columns visited and successful return. Forward Safe/Visible minima were 0.5/1.0;
visibility p50/p95/max 0.487/0.969/1.440 s; critical backlog max 4.832 s. Event-gap p95/max was
79.03/129.39 ms, frame p95/p99/max 79.07/91.55/130.05 ms, TPS 20.25. Dropped simulation time was
0.100 s (the unchanged acceptance limit), not zero; software render/present was the measured long
task. Input-to-simulation p95/max was 75.76/96.35 ms and input-to-render 78.11/90.80 ms.
Peak residency was 286 columns / 2288 sections; approximately 80.4 MiB authoritative voxel/light,
114.5 MiB snapshots and 101.1/158.1 MiB logical/capacity meshes. Representative GPU acceptance
remains conditional under M4-009; neither software run is AMD/Vulkan evidence.

Final local validation passed: formatter, workspace all-target check/tests, all-target/all-feature
Clippy with warnings denied, `bootstrap-check`, `ci`, `smoke`, `survival-scenario`, `sample-game`,
`render-test-all`, `fidelity-m3`, `resource-stress 1000`, `render-scale`, `world-roundtrip`,
`worldgen-bench`, `persistence-bench`, `world-stream-bench`, versioned `world-travel-test`,
`client-stream-auto`, `world-state-roundtrip`, `entity-persistence-bench`, `release-check`, explicit
v1/v2 regression/report/map commands and `git diff --check`. Mandatory tests use semantic states
and public synthetic fixtures, not proprietary assets. `world-state-roundtrip` reports time
99170, five entities before/three after pickup, and stable-ID/merge/cross-column/eviction/block-drop/
delay/velocity/despawn/receipt/unknown-global success. Entity persistence measured 16/1000 records
at 1892/118004 raw bytes and 284/7628 stored bytes; no persistence work was moved onto the event thread.

Implementation commit `ef0db15312058fc58940d8189a4c3df27a05bf5e` passed
[Ubuntu and Windows GitHub Actions](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37135831666),
including the immutable v1 hash and final v2 semantic hash. M4-004 is closed and M4 is functionally
complete; M4-009 representative-hardware quantitative evidence remains conditional. The older manual
water-presentation walkthrough is not claimed: automated liquid geometry/culling, compiled-medium
boundary and distance-fog tests cover the functional repair. Deferred lava/springs/dungeons/plants
are explicit later first-party content scope. No subsequent milestone, version bump, tag or release
was started by this pass.

The project is performance-oriented, but optimization claims require evidence.

Performance and frame-time stability rank above exact Beta fidelity after semantic correctness.
Moderate visual differences are acceptable when they provide a substantial measured or clearly
justified runtime, memory, scalability or extension benefit. Core interactions and state must
remain recognizable and predictable.

## Start with structural wins

Prefer:

- contiguous storage;
- compact typed IDs/state;
- predictable iteration;
- batch processing;
- dirty/event queues instead of world scans;
- buffer reuse;
- explicit ownership and partitioning;
- async/background I/O outside critical simulation work;
- rendering/simulation decoupling;
- no per-block heap object model.

Profile compilation keeps semantic strings at loading, diagnostic and serialization boundaries.
The voxel-to-render hot path is compact `BlockState` -> indexed `CompiledVoxelDefinition` ->
compiled descriptor; it must not perform string hashing or linear definition scans. `BlockState`
remains 8 bytes in R1.0, so a dense 16³ section uses 32 KiB for block state before lighting and
container overhead. Future state canonicalization must be measured instead of expanding global
variant bits for unrelated game concepts.

Renderer/resource candidates include mipmaps, anisotropic filtering, compression, LOD, reduced
distant animation/update rates, simplified distant materials and transparency, stronger
occlusion/culling, dynamic quality, GPU-driven rendering, greedy meshing and optimized lighting.
Assess them against large resource packs, many block states/mods, long view distances and many
entities rather than only a small first-party scene.

## Measure before advanced techniques

Do not add custom allocators, hand SIMD, prefetch instructions, huge pages, NUMA placement,
lock-free structures, io_uring-only paths, PGO/BOLT or a special storage database just because they
sound fast. Benchmark representative workloads first.

Useful measurement layers later:

- whole-command timing: `hyperfine`;
- microbenchmarks: Criterion or equivalent when justified;
- CPU profiling: `samply`, `perf`/flamegraph;
- allocation profiling where needed;
- binary size: `cargo-bloat`;
- test throughput: `cargo-nextest`.

Every reported win should state workload, hardware, before/after numbers and regression checks.
Decision notes for substantial candidates also compare performance/frame-time, memory,
scalability and extensibility benefits with visual/semantic cost, complexity and maintenance.

## R1.1 resource measurements (2026-10-02)

Measurements use the Cargo development profile on this Linux workspace; compiler phase timers are
inside the process and exclude Cargo startup/build time. The finalized first-party workload has 32
resolved textures. Adaptive packing chooses two 512x512 RGBA8 pages at 26.8% occupancy, versus the
earlier fixed 1024x1024 single page at about 13.4%. Source compressed bytes are 83,381; resolved
decoded/cropped RGBA bytes are 562,176; compiled CPU atlas bytes and estimated uncompressed RGBA8
GPU-format bytes are each 2,097,152. These are not measured device VRAM.

With a fresh isolated cache, the first-party compiler measured 66.013 ms total (2.770 ms hashing,
30.792 ms parallel decode, 0.809 ms crop extraction, 24.872 ms packing/cache write). The immediate
warm run measured 4.073 ms total with no decode or pack. The earlier fixed-page cold baseline was
approximately 52 ms and is not claimed as a regression comparison because cache state and the
final security/adaptive work differ.

The project-owned 1,000-resource stress workload reuses one physical 8x8 PNG through distinct
semantic crops/resources. Adaptive packing chooses ten 128x128 pages at 39.1% occupancy: 186 source
compressed bytes, 256,000 decoded/cropped bytes and 655,360 compiled/estimated-RGBA8 bytes. Cold was
32.615 ms (2.776 ms hash, 0.906 ms decode, 0.511 ms extraction, 12.314 ms pack); warm was 11.727 ms.
The prior fixed 256x256 baseline was roughly 34 ms cold / 13 ms warm with three pages at 32.6%, so
the adaptive result lowers allocated atlas bytes while retaining similar development-build time.

The renderer metric is `atlas_upload_submit_ms`: CPU time for texture creation and
`queue.write_texture` submission. It does not wait for GPU completion and is not GPU transfer
latency.

## R1.2 structural renderer measurements (2026-10-02)

The deterministic `just render-scale` workload ran in Cargo release profile on x86_64 Linux.
It times `RenderWorld` extraction and CPU meshing separately; world construction and GPU execution
are outside those phase timers. The untouched pre-snapshot release baseline below is retained from
the initial R1.2 profile. The final run uses the same workload after contiguous halo snapshots and
dense interior copies. Deltas are absolute final-minus-baseline, with percent change relative to
that baseline.

| Workload | Sections | Baseline extract / mesh (ms) | Final extract / mesh (ms) | Delta extract / mesh (ms) |
|---|---:|---:|---:|---:|
| Large radius 8 | 289 | 376.396 / 110.148 | 32.767 / 24.886 | -343.629 (-91.3%) / -85.262 (-77.4%) |
| High exposure | 9 | 4.000 / 37.343 | 0.609 / 31.248 | -3.391 (-84.8%) / -6.095 (-16.3%) |
| Dirty remesh | 81 | 73.656 / 29.576 | 6.248 / 7.577 | -67.408 (-91.5%) / -21.999 (-74.4%) |

The intermediate contiguous-snapshot result was large 86.942/32.297 ms, exposed 1.899/35.551 ms
and dirty 19.214/8.333 ms (extract/mesh). The final additional dense-interior copy accounts for
the lower extraction time; it copies the section's dense 16³ arrays directly and performs world
lookups only for halo cells. Final large-scene emitted geometry was 596,224 vertices and 894,336
indices (298,112 triangles; 25,041,408 logical CPU mesh bytes across generated meshes). The exposed
scene emitted 442,368 vertices and 663,552 indices (221,184 triangles; 18,579,456 logical CPU
mesh bytes). These are workload output totals, not resident CPU/GPU allocations.

The same final release command reports small radius 1 (9 sections) at 0.945 ms extraction / 1.120
ms meshing and medium radius 4 (81 sections) at 8.262 / 7.454 ms. Each 18³ snapshot is 52,488
bytes (18³ `BlockState` plus 18³ light values), so the 289-section large snapshot set is 15,169,032
bytes. CPU pending/completed mesh bytes and GPU logical/capacity bytes are separate live diagnostics.

The final synthetic dirty-remesh stress requested 108 generations across 9 unique sections with a
one-worker scheduler and queue capacity 2: 10 jobs submitted/completed, 99 dirties coalesced, one
old generation discarded stale, 9 latest results drained in 9 upload-budget waves, maximum ready
backlog 9, and 18,575,424 logical bytes passed through the CPU harness. Snapshot extraction was
0.667 ms and summed worker meshing time 41.978 ms. GPU upload submission is not applicable to this
headless CPU harness; live renderer diagnostics measure CPU-side buffer/queue submission time.

The actual surface camera-motion diagnostic used llvmpipe (LLVM 22.1.8, 256-bit) over the GL
backend. Across the 18 resident sections, the facing/turned-away/restored samples showed 16/0/16
visible sections and 2/18/2 culled, respectively; every camera-only phase had zero mesh rebuilds.
Three 30-second release normal-lit runs on the same software backend reported 50.72, 49.57 and
51.38 FPS (mean 50.56), 19.72, 20.17 and 19.46 ms mean frame time (mean 19.78), and 26.94, 28.90
and 28.03 ms 1%-low frame time (mean 27.96). Each had 18 resident, 18 meshed, 12 visible, 6 culled,
12 section-page draws, 12 draw calls, 7 workers and no pending, in-flight, stale or coalesced jobs
after initial convergence. Initial uploads submitted 454,608 logical mesh bytes in 1.15–1.43 ms
CPU time; GPU timing was unavailable/near-noise on software GL, so no GPU speedup is claimed. A
bounded development-profile startup did initialize AMD Radeon Vega 8 Graphics over Vulkan, but
the longer repeatable release samples selected llvmpipe GL in this environment. Earlier ~35.74 FPS
/ 7.93 ms 1%-low and intermediate ~38.45 FPS / 9.90 ms samples were short and are not compared as
controlled before/after evidence.

The camera-mode renderer diagnostic performed three same-size upload cycles across 18 resident
sections: 108 existing vertex/index buffers were reused, with zero additional allocations or
reallocations, and total CPU-side GPU-buffer/queue submission time was 1.376 ms. Its compatibility
expectation for fresh allocation on each repeat is 108 additional buffer allocations; the test
directly verifies reuse via the production renderer uploader. Across all startup uploads there
were 36 buffer allocations, 0 reallocations and 108 reuse operations.
Capacity uses checked geometric power-of-two growth (256-byte minimum) and shrinks only when the
new logical size falls below one quarter of capacity; this intentionally trades some temporary
retention for fewer small remesh reallocations.

Page-first deterministic opaque/cutout submission yielded zero page-bind switches in the sampled
single-page visible set and zero additional pipeline switches between world section batches. The
camera diagnostic still reported 3 pipeline transitions to the other overlay passes. This does not
alter translucent ordering; fractional-alpha sorting remains simplistic and is a separate defect.
Greedy meshing remains deferred: high-exposure geometry is large and meshing consumes 36.9 ms in
the nine-section synthetic CPU workload, but optimizing its UV repetition/atlas semantics is not
part of this structural pass. Re-profile a representative hardware renderer and geometry workload
before choosing greedy meshing or advanced GPU submission. Current measured dominant CPU cost at
large radius is snapshot extraction (31.8 ms), followed by CPU mesh build (26.8 ms); the software
renderer sample is too platform-specific to identify the next production GPU bottleneck.

## M4 generation and persistence baseline (2026-10-02)

Measurements use Cargo `release` on Linux x86_64, rustc 1.98.0, AMD Ryzen 5 PRO 2500U (4 cores / 8
threads); the worldgen and persistence commands do not initialize a GPU. `just worldgen-bench`
generates a fixed 4x4 chunk region with four bounded workers at seed 731173. The current run took
27.697 ms wall time, summed 10.427 ms across worker generation calls, and produced 16 columns / 128
sections (4 MiB estimated authoritative dense `BlockState` storage), or 577.69 columns/s. Canonical
hash: `e0d1f83c16b281124b7a9c190f667d7eddaa8b2f35ef5ef98ccb4bab434c1bb6`. Tests also compare hashes
for reverse request order and one-versus-four workers. These are early generator baselines, not
large-region scalability claims.

An additional release verification run on the same workload measured 29.643 ms wall time and
11.086 ms summed worker time (539.77 columns/s); the canonical hash remained identical. The spread
is retained as run-to-run evidence rather than presented as a performance regression or gain.
The liquid presentation repair rerun measured 26.205 ms wall time and 10.409 ms summed worker time
(610.58 columns/s), again with the identical canonical hash; generated water remains variant 0.

`just persistence-bench` generated and round-tripped four 128-high columns. Explicit palette
payloads totaled 264,248 raw bytes; Zlib fast compression produced 10,163 stored bytes including
framing/checksums (3.85% of raw). Semantic palette encoding took 7.905 ms total, compression CPU
0.951 ms, compression plus atomic write/sync 14.304 ms, and load/decompress/decode 16.541 ms. The
write timer includes CPU compression and OS file/directory sync submission; it is not a physical
device latency guarantee. All four chunks selected compression because it reduced storage. There
is no separate GPU or VRAM metric for world persistence.

A repeated release persistence run produced the same raw/stored sizes and selection; it measured
8.261 ms palette encoding, 0.846 ms compression CPU, 13.309 ms compression plus atomic write/sync,
and 15.442 ms load/decompress/decode. These are short local-filesystem samples, not durable device
latency guarantees.

The headless `just world-roundtrip` integration generated two neighboring columns on two workers,
persisted a semantic-state edit, closed/reopened the world, and verified the edit and a still-
ungenerated deterministic neighbor. The client now assembles its safe startup neighborhood on a
background bootstrap worker and requests a bounded moving residency area; interactive travel and
full M4 acceptance remain outstanding.

The M4 player record now uses an outer v2 envelope and four Minecraft-owned component codecs:
transform, game mode, inventory/crafting and bounded pickup receipts.
Changed revisions checkpoint every two seconds by default (configurable 1–2 seconds); one
coalescing worker keeps filesystem sync off the simulation/render tick. Checkpoint diagnostics
report encoded bytes, write time, sync/replace time and total worker latency; sync/replace measures
the atomic checkpoint transaction including OS-requested file/directory synchronization, not
guaranteed physical-media latency. The prior manual 116-byte
single-payload save measured 0.072 ms encode and 3.307 ms atomic write+sync, with 0.032 ms read and
0.029 ms decode on reopen. It is a pre-component legacy-format sample, not a comparison against
the final async/two-slot layout. The empty inventory payload was 65 bytes before component framing;
pickup receipts add a two-byte zero-count payload before component framing. The componentized
current payload has not yet been release-profile measured. In a debug-profile
scratch-world run, checkpoint writes reported 0.023–0.035 ms write time, 11.8–68.4 ms requested
file/directory sync time, and 12.7–68.4 ms total worker time for the tiny player record. This local
filesystem sample demonstrates why sync stays off the simulation/render thread; it is not physical
media latency or a release benchmark. A hard crash may lose mutations since the last successful
checkpoint. The isolated scratch client was closed without a graceful-save completion and reopened
at checkpoint revision 203 with the then-current three components, demonstrating recovery of the latest
checkpoint; unsaved motion after it remains within the configured 1–2 second window. Underwater
fog adds only a camera uniform and fragment distance/mix work on world opaque/translucent fragments;
no GPU timing comparison is available yet, so no frame-time cost is claimed.

M4-003 keeps normal spatial persistence on bounded worker paths. Chunk file open, read,
decompression, generic record parsing, compression, atomic replacement and sync run in the
load/save workers; the client tick only snapshots a bounded column, resolves game-owned item
records from an already-loaded result, and applies lifecycle transitions. Spatial records are
capped at 1,024 per column. Global-state checkpoint file work similarly runs in one bounded,
coalescing worker; the tick snapshots the eight-byte clock component and polls revision
completions. The final release `entity-persistence-bench` measured 16 records at 1,892 raw/284
stored bytes, 0.024 ms encode, 0.518 ms compressed atomic write+sync and 0.059 ms read/decode. Its
1,000-record stress case measured 118,004 raw/7,628 stored bytes, 0.456 ms encode, 0.898 ms
write+sync and 0.653 ms read/decode. These short local-filesystem samples demonstrate bounded cost;
they are not physical-media latency guarantees.

`just world-stream-bench` is a headless release workload over 91 deterministic interest centers,
including negative coordinates, multiple travel reversals, save-before-evict, and edits in two
separated columns. Runtime simulation spreads new-column light seeding/propagation and eviction
boundary cleanup over bounded 4,096-voxel slices. The client admits at most two ready columns per
fixed tick into a lighting queue bounded to eight columns including active work. This matters
because synchronous `Simulation` column publication previously took 26.710 ms in its worst single
call (an earlier sample reached 62.883 ms).

Latest release run: 77 generated, 180 loaded, 77 misses, 741 evicted, 77 saved before eviction,
maximum 14 resident and 5 load/generation pending-or-in-flight, maximum lighting queue 5. Summed
worker load/decode was 1,397.669 ms; generation 43.040 ms; incremental lighting/publication work
3,627.883 ms; maximum measured slice 3.206 ms; total wall time 5,094.440 ms. These vary by run and
describe the headless benchmark, not an interactive frame-time improvement. The 3.206 ms maximum
is 23.504 ms lower than the historical 26.710 ms synchronous sample, while work is spread across
ticks rather than removed. Light-driven
section invalidations coalesce until propagation converges. Two separated edits were observed
after residency reload and persisted decode. `sample_chunk_state_hash=384029af…7192a807` is the
canonical voxel-state hash of one independently regenerated benchmark column at `(4,2)`; it is not
the fixed-region worldgen hash and is not a hash of the edited streaming world. The separate
16-column canonical worldgen hash remains
`e0d1f83c16b281124b7a9c190f667d7eddaa8b2f35ef5ef98ccb4bab434c1bb6`.

Interactive release diagnostics on the owner’s AMD Radeon Vega 8 identify RADV Vulkan, not
llvmpipe. Before the current repair, the owner observed ~37.8 FPS, 26.46 ms frames, ~6 FPS 1% low,
TPS 18.8, and ~0.92 ms GPU frame time while traveling; the same screenshot showed ~6.03 s lighting
work, 602 rebuilds, 80 stale meshes, and 7 mesh workers. In a local release reproduction with
those 7 workers, the converged scene returned to ~57–59 FPS and TPS~20, but active travel left up
to 7 lighting columns queued and resident columns grew 29→57 without eviction. This isolates the
regression to CPU-side streaming/lighting/remesh pressure rather than GPU execution, while present
FIFO accounts for most of the ~17 ms converged frame interval. The newer repair defers direct-sky
scan from publication into bounded slices, avoids boundary seeds for opaque non-emissive cells,
coalesces presentation invalidation until a column's lighting converges, removes the per-publication
resident-section scan by carrying the new column's section list into lighting, and queues cleanup
for unrelated evictions rather than globally blocking eviction while any lighting work is active.
It also coordinates default CPU work to four mesh, one load, and one generation worker on an
8-logical-CPU host; `RUSTCRAFT_MESH_WORKERS` remains available for a real-device sweep. CPU phase
timings and request→voxel/light/all-section-upload latency windows are now exposed in F3. These
changes have focused correctness tests, but must not be described as a measured interactive
improvement until a same-device travel/convergence run completes. `just stream-perf` launches the
isolated scratch-world forward-travel workload with F3 tracing enabled.

Residency request ordering now uses bounded speed × time on a stable travel heading, measured from
actual per-fixed-tick displacement, to scale a bias along a normalized 70% movement / 30% camera
horizontal-direction blend.
A single sample is clamped to 8 blocks, speed to 32 blocks/s, the movement horizon to 12 seconds,
and lookahead to three columns. A heading change resets the sustained duration; stopping decays it.
This avoids a high-priority jump from a brief fast nudge while moving requests ahead in the visible
direction during committed travel; actual movement remains the stronger directional signal, and
camera rotation alone does not increase the priority horizon.
F3 `motion_lead` reports the current lookahead. Eviction is
considered every fixed tick outside retain radius; dirty, save-in-flight, lighting and dropped-item
columns remain protected.

Real-surface radius-6 release profile on AMD Radeon Vega 8 / RADV Vulkan (4 mesh workers, FIFO,
fixed flight for 30 s, then idle until queues converged): active travel averaged about 55–56 FPS
(18.0 ms frame), 1% low 23.8–24.7 ms, TPS 19.9–20.0, GPU frame 1.0–1.2 ms. Fully converged final
sample was 56.94 FPS, 17.56 ms frame, 29.34 ms 1% low, TPS 19.92, 13.4% process CPU and 1.59 ms GPU
frame. The full desired radius first became ready at 51.98 s from world-ready; startup-safe was
242.7 ms. At convergence: 113 desired, 123 resident columns (retain area plus temporary/pinned
state), 984 resident sections, 162 visible sections, 15 evictions, and no async/lighting/mesh backlog.
The moving window reported request→voxel p95 4.32 s, request→lighting p95 7.47 s, and
request→all-section-upload p95 7.60 s (max 10.12 s). The run completed 4,176 mesh jobs with 14
stale and 2 coalesced, used 221.6 MiB process RSS, 49.3 MiB snapshot estimate, 28.7 MiB logical GPU
mesh bytes and 57.6 MiB allocated buffer capacity. It confirms a short far-latency tail but does not
show whether a manually controlled player outruns terrain; at that checkpoint M4-002/M4-009
remained open pending visual acceptance and improved arrival latency. Earlier radius-6 attempt on
llvmpipe/GL was closed and
excluded from hardware conclusions.

A second real-surface radius-6 release flight after the sustained-motion priority update converged
at 57.90 FPS, 17.27 ms frame, 29.54 ms 1% low, TPS 19.99, and 1.60 ms GPU frame. During travel it
averaged about 57 FPS with 23.9–25.3 ms 1% lows. It reached the full 113-column desired area in
50.91 s; request→voxel p95 was 4.36 s, request→lighting p95 7.55 s, and request→all-section-upload
p95 7.69 s (max 10.23 s). It completed 4,186 mesh jobs (21 stale, 2 coalesced), converged at 120
resident columns with 15 evictions, and had no async/lighting/mesh backlog at final sampling. The
priority lead reached its configured three-column cap during sustained flight and decayed to zero
after stopping; however, the aggregate visible-latency p95 did not improve beyond run variance.

After adding camera-view direction to the bounded motion bias, the same auto-flight diagnostic on
AMD Radeon Vega 8 / RADV Vulkan completed and closed itself after queue convergence. Final sample:
57.83 FPS, 17.29 ms frame, 29.23 ms 1% low, TPS 19.71, GPU frame 1.56 ms, 13.3% process CPU,
220.2 MiB RSS. During the 30-second flight FPS remained roughly 57–59; the view/motion lead reached
3 columns and returned to zero while idle. The 113-column desired area became ready in 46.64 s;
request→voxel p95 was 3.93 s, request→lighting p95 7.00 s, and request→all-section-upload p95
7.05 s (max 8.67 s). At convergence, 120 columns/960 sections were resident, 11 columns had been
evicted, all async/lighting/mesh queues were empty, and 3,870 mesh jobs had 7 stale results. This is
consistent with the previous runs but does not establish that a manually controlled player never
outruns visible terrain.

Controlled 60-second AMD Radeon Vega 8 / RADV Vulkan sweeps used the same 30-second four-leg flight,
four mesh workers, and fresh worlds. These pre-repair runs compare radius/lookahead; p95 values are
request-to-all-section-upload, not a claim about forward visible margin:

| Load radius | Retain | Lookahead | Desired | Generated | Visible p95 | Request→lighting p95 | Lighting queue p95 | FPS / TPS |
| ---: | ---: | :---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 3 | 4 | on | 29 | 81 | 6.60 s | 6.58 s | 4.25 s | 58.3 / 20.64* |
| 4 | 5 | on | 49 | 103 | 7.41 s | 12.43 s | 9.53 s | 58.3 / 19.92 |
| 4 | 5 | off | 49 | 103 | 7.40 s | 12.54 s | 9.54 s | 58.1 / 19.99 |
| 5 | 6 | on | 81 | 118 | 7.51 s | 11.70 s | 7.03 s | 58.1 / 19.67 |
| 6 | 7 | on | 113 | 135 | 7.70 s | 7.55 s | 2.86 s | 58.3 / 19.99 |

`*` TPS is the client's whole-run rolling estimate and occasionally exceeds 20; it is not a stable
fixed-step-drift measurement. The radius-4 A/B is effectively indistinguishable (well below run
variance), so this route does not establish a measurable lookahead benefit. More importantly, the
radius sweep does not show larger radii reducing arrival latency. The controlled traces locate the
large tail in serialized work: radius-3 request→voxel p95 was 3.26 s, lighting queue wait p95
4.25 s, while lighting's per-column accumulated CPU p95 was about 52 ms; mesh queue/execution p95
was about 48/12 ms, upload wait about 103 ms, and publication CPU below 0.1 ms. Load and generation
stages report their true submit→worker queue delay separately now; prior
reports subtracted worker CPU from request latency and are not actual queue-wait measures.

A separate same-route worker sweep after the boundary fast path held radius 3 and lookahead on,
varying total heavy workers as mesh workers + one load + one generation worker (save worker was idle):

| Total heavy workers | Mesh workers | FPS | 1% low | TPS | Visible p95 | Lighting queue p95 | Mesh queue / execution p95 | Upload wait p95 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 3 | 1 | 58.55 | 29.69 ms | 20.00 | 6.01 s | 4.38 s | 551 / 5.0 ms | 31 ms |
| 4 | 2 | 58.15 | 29.95 ms | 19.91 | 6.13 s | 4.47 s | 221 / 7.5 ms | 45 ms |
| 5 | 3 | 58.31 | 30.02 ms | 19.68 | 6.15 s | 4.17 s | 112 / 9.5 ms | 178 ms |
| 6 | 4 | 58.55 | 29.78 ms | 19.99 | 6.02 s | 4.47 s | 93 / 11.9 ms | 195 ms |

This sweep does not show a material FPS/TPS or visible-latency advantage from more than three
workers; it does reduce mesh queue wait while moving more time into mesh execution/upload waiting.
The existing four-mesh-worker default (six heavy workers including load/generation) is retained for
now because it keeps the mesh queue tail short without changing the dominant ~4.4 s lighting queue
wait. This is a measurement of this route/device, not a universal worker formula.

On the same radius-3 route, current-distance lighting reprioritization without aging produced a
starvation tail (maximum lighting wait about 31 s), so that version was discarded. Aging bounds
that behavior but did not materially improve p95. The retained queue-aging policy is tested; it is
not yet accepted as a throughput fix. Skipping border scans for sides without resident neighbors
reduced boundary inspections from 1.18M to 0.525M in the route sample, but visible p95 only moved
from about 6.6 s to about 6.0 s across runs. This is a measurable operation reduction, not closure
of M4-009. Canonical worldgen remains unchanged. The primary unresolved KPI is still request→useful
terrain visible to the player, and a directional ready/visible-margin metric plus owner-controlled
normal-speed travel acceptance remain necessary.

The client default is now load radius 4 / retain radius 5; `RUSTCRAFT_STREAM_RADIUS=3..12`
remains supported. The final autonomous route and margin evidence are recorded below.
Outside-retain columns are considered for eviction every simulation tick,
while dirty, saving and boundary-lighting-dependent columns remain resident safely. Persistable
dropped-item columns now freeze, checkpoint and evict rather than pinning residency indefinitely.
The owner has reported responsive input but slow terrain arrival during manual travel; this batch's
controlled flight is scripted and does not replace that manual acceptance. Initial bulk lighting
now runs on a bounded worker; boundary reconciliation is advanced in 32-operation resumable
quanta under a shared 2 ms event-loop streaming budget (`RUSTCRAFT_STREAM_MAIN_BUDGET_MS`,
diagnostic override). Dirty snapshots/mesh requests are coalesced and processed a section at a
time, and result/request/eviction application yields when the same callback budget expires. Hardware GPU
timestamps are available for the Vega 8/Vulkan runs (~0.7–1.3 ms), while measured CPU present time
includes FIFO/vsync. These measurements do not justify renderer optimization; the unresolved work is
streaming/lighting throughput and verified player-visible margin.

## Event-loop starvation investigation (M4-010)

An owner-controlled `just client-survival` run confirmed strong lags. This environment selected
llvmpipe/GL, so the numbers below diagnose scheduling but are not Vega 8/Vulkan acceptance. The
first ~30 seconds included event-loop gaps up to 1.34 s, fixed-tick gaps up to 339 ms, and 13.45 s
of dropped fixed-step time. `residency_ms` reached p95 183 ms/max 336 ms; synchronous
`RenderWorld::sync_sections` over the accumulated dirty set reached p95 64 ms/max 243 ms. The
debug overlay itself reached p95 ~15.7 ms when enabled. Per simulation tick, boundary reconciliation
could process 8,192 work units; each dirty snapshot performed its own 18³ neighborhood extraction,
and several columns could enqueue many sections before the next event turn. This is consistent
with main-thread work starvation, not a synchronous startup barrier. Input callback handling itself
was small in the trace, while input-to-render p95/max reached ~582/1,291 ms because no frame was
rendered promptly.

The repair makes the event-loop callback the budget boundary: default 2 ms shared streaming budget,
input dispatch first, repeated 32-unit boundary-light slices within a 35% sub-budget, one
initial-light result per turn, and coalesced snapshot/mesh/eviction work resumed over later turns.
On the actual AMD Vega 8/RADV/Vulkan client (radius 3, F3 enabled), the final 93-second run reported
~58.3 FPS, 27.9 ms 1% low, ~20.6 TPS, 0 dropped fixed ticks, GPU frame ~0.91 ms, frame
p50/p95/p99/max ~16.4/29.0/34.0/36.5 ms, and event-loop gap p99/max ~33.8/34.7 ms. Fixed-step
CPU p95 was ~0.12 ms, residency p95 ~0.39 ms, boundary-light service p95 ~3.24 ms (the indivisible
vertical-ray scan and multiple quanta can exceed the loop's soft time budget), snapshot sync p95
~4.29 ms/max ~8.95 ms, and mesh scheduling p95 ~0.015 ms. The once-per-second F3 rebuild caused
rare ~22.6 ms outliers; typical cost was under 0.5 ms. This demonstrates that the multi-second
event-loop starvation was removed on the test device, but it is not full travel acceptance: no
manual movement/turn/reverse confirmation was received, and loaded-world request→visible p95 remained
~88.4 seconds while only 10 of 18 published columns had completed boundary lighting by the end.
Streaming throughput is still a separate open M4-009/M4-002 issue. An intermediate 512-unit pump
was rejected because one uninterruptible batch drove boundary-light p95 to ~22 ms. The current
32-unit pump repeats within the callback budget while allowing one atomic vertical-ray scan (the
lighting algorithm's minimum unit). Individual snapshot/upload costs remain measured because a
wall-time loop budget cannot preempt one operation already executing. The synthetic backlog-yield
test covers retaining work for later turns.

## Bulk initial-lighting repair (M4-009)

The old stream path admitted one active column into `Lighting` and advanced exactly one 8,192-unit
slice each 20 Hz fixed tick on the event/simulation thread. A typical ten-section column scans 256
vertical x/z rays; each ray is charged about 2,560 units. The seed phase alone therefore needs at
least 86 ticks (about 4.3 s) before shaded-cell/emitter relaxation and boundaries, and queued columns
wait behind that single non-preemptive integration. The 52 ms/column CPU sample was thus stretched
across seconds of tick scheduling. This is the measured mechanism behind the initial-light queue
tail; actual old queue-wait p95 on Vega 8 was about 4.25 s.

The new `InitialLightingScheduler` takes owned voxel sections and a cloned immutable registry,
constructs full initial direct-sky and block-light arrays off-thread, and returns the still-unpublished
voxel sections plus light arrays and direct-source data. Direct sky is scanned vertically; the
existing relaxation visits only shaded cells, prior light and actual emitters. The simulation
applies arrays by moving section storage, then performs bounded reconciliation only on faces with
resident neighbors. Adjacent candidates are checked so emitter/light sources at a seam converge
independently of A→B/B→A arrival order. Columns are not available to collision/rendering until full
initial lighting is done; remesh invalidation occurs once after reconciliation. Incremental block
edits remain on the existing update path.

Focused tests pass for worker-vs-synchronous single-column arrays, seam emitter order convergence
(A→B, B→A and synchronous simultaneous reference), queue capacity/result delivery, and the existing
incremental lighting suite. `world-stream-bench` now exercises this scheduler for 257 loaded and
generated columns, retains the edit/evict/revisit path, and reports per-job p50/p95/max queue and
worker elapsed. Release headless worker sweep (load=1, generation=1; no mesh/render consumers):

| Initial-light workers | Columns/s | Queue wait p50/p95/max | Worker elapsed p50/p95/max |
| ---: | ---: | ---: | ---: |
| 1 | 22.2 | 7.5 / 28.0 / 56.4 ms | 12.5 / 20.0 / 22.3 ms |
| 2 | 22.4 | 0.05 / 13.0 / 20.4 ms | 15.8 / 22.3 / 26.4 ms |
| 3 | 23.3 | 0.04 / 0.08 / 13.9 ms | 18.3 / 22.9 / 26.8 ms |
| 4 | 23.6 | 0.04 / 0.09 / 10.5 ms | 17.7 / 23.7 / 27.8 ms |

This is not a same-device render/CPU contention sweep: concurrent full lighting raises each job's
elapsed time and yields only ~6% aggregate throughput gain at four workers. One worker remains the
default (`RUSTCRAFT_LIGHT_WORKERS` override), with three default mesh workers plus load and
generation workers for six compute-heavy workers on the 8-logical-CPU development host. Separate
save/player-checkpoint workers are I/O-oriented and are not included in that count.

An attempted automated 30-second client flight in this execution environment selected
`llvmpipe (LLVM 22.1.8) / GL`, not AMD Vega 8/RADV Vulkan. It was stopped and excluded from hardware
acceptance. That partial software-renderer trace did show initial-light worker queue p95 ~50 ms and
worker elapsed p95 ~15 ms, compared with old multi-second queueing, but boundary-reconciliation
queue wait remained ~1.35 s p95; request→light-ready was still ~1.93 s p95. Software-renderer FPS
and visibility numbers are not comparable to the earlier Vega 8 baseline. No post-repair radius
sweep, movement-direction ready-margin acceptance, or manual Vega 8 walking/turn/reverse check was
possible here. The canonical worldgen hash remains a separately checked acceptance gate and must
remain `e0d1f83c16b281124b7a9c190f667d7eddaa8b2f35ef5ef98ccb4bab434c1bb6`; the benchmark's
`sample_chunk_state_hash=384029af…7192a807` remains only its independent sample-column hash.

At that checkpoint M4-009 and M4-002 therefore remained open. The initial-light multi-second queue
defect is repaired
architecturally, but same-device request→visible p95, residual boundary scheduling, active/converged
frame/TPS, and normal-speed directional render-ready margin are still unverified.

## Render-ready starvation investigation (M4-011)

The shared 2 ms safeguard exposed a separate last-mile defect. The old service order ran boundary
lighting before fixed-step work; residency completion/admission, snapshots and mesh scheduling were
then serviced only from 20 Hz fixed ticks and in a fixed order. Since all four stages consulted the
same deadline, an early stage could consume its remainder, and dirty snapshot/mesh `HashSet`s did
not guarantee that a forward/near section was selected first. The owner did not observe a frontier
in the latest baseline session, so there is no captured per-column frontier lifecycle or post-fix
manual travel result yet. A prior reused-world run reported request→visible p95 near 88 seconds;
that value is the failure baseline, not an isolated queue-stage attribution.

The revised service runs once after input/fixed-step processing on each event-loop turn. It gives
critical completed-result application, boundary reconciliation, snapshot synchronization, mesh
submission, and per-render mesh-ready upload service separate allowances within the existing
nominal budget. Required (3x3), visible (next inner ring), then prefetch urgency is carried by the
generic residency priority score and reused to order snapshot/mesh submission. Completed meshes
prefer camera-forward sections within the same urgency ring. Initial resident meshes are seeded in
urgency/distance order instead of coordinate order. A diagnostic llvmpipe/GL trace showed why
starvation needed to be separated from prerequisite blocking: eight active/queued boundary jobs
exhausted the former eight-column capacity, leaving completed light results waiting for capacity
(critical stage reported 311 unserviced turns despite zero budget skips). Radius-3 desired residency
was 29 columns, so the interim boundary capacity became 32. Final square-radius support uses a
bounded capacity of 256. Locally complete initial-light results become
presentation-ready immediately; later boundary reconciliation only invalidates actual changed-light
sections. This is progressive presentation, not a claim that boundary work has completed. F3 reports
per-stage budget skips, consecutive turns without service, observed backlog age, plus the nearest
forward column's broad lifecycle (`FRONTIER`).
The age is measured since the client first observed a non-empty backlog in this process; it is a
diagnostic lower bound, not a persisted per-item enqueue timestamp. The owner reported no camera or
control lag, but terrain still appeared behind the player and the approached chunk stayed invisible
until block interaction, which made it appear immediately. This indicated delay in the last-mile
presentation path rather than input responsiveness. After the follow-up changes, the owner retested
`client-survival` on the AMD/Vulkan client: the forward chunk appeared on its own without breaking a
block, and controls remained responsive. The visible draw distance still felt short (about one
chunk). This closes the reproduced render-ready starvation symptom, not the broader terrain-ahead/
latency acceptance tracked by M4-002 and M4-009. No post-fix F3 stage trace or sustained turn/reverse
run was captured, so performance percentiles are not claimed. The llvmpipe trace is diagnostic
only, not Vega/Vulkan acceptance. Boundary reconciliation still runs on the event thread in
resumable slices.

## Autonomous streaming completion acceptance

The final area policy uses Chebyshev squares. Radius membership costs are 49/81/121/169 Desired
columns at load radii 3/4/5/6, compared with the former Euclidean 29/49/81/113 and Manhattan
25/41/61/85. The earlier controlled radius sweep showed no terrain-arrival benefit from increasing
past four and made the extra startup work explicit. Radius 3 supplied too little observed visible
margin; the final normal-speed route at radius 4 retained a positive buffer. The selected defaults
are therefore load 4, retain 5, a complete 3x3 control-release core, visible urgency through radius
2. Lookahead defaults off: on/off had the same 0.5-column minimum forward Safe/Visible margin;
request-to-visible p95 was 1.30/1.50 s, which is not a meaningful benefit over run variance.
When explicitly enabled, prediction remains bounded and movement-dominant within (never across)
urgency classes.

`just world-travel-test` uses production simulation, residency, load/generation, initial lighting,
snapshot and meshing paths. It passed fresh generation, normal 4-block/s semantic-controller travel,
straight and 90-degree turns, diagonal and negative-coordinate legs, 180-degree reversal, return
through evicted terrain, persisted edit reload, distant saved-player reopen and continued travel.
Its fresh minimum `[forward Safe, forward Visible, lateral Safe, lateral Visible, rear Visible]`
was `[0.5, 1.5, 1.0, 1.0, 1.0]`; reused minimum was `[1.0, 2.0, 1.0, 1.5, 2.0]`.
Fresh request-to-visible p50/p95/max was 0.300/0.489/0.688 s; reused was
1.232/2.242/2.328 s. Fresh/reused resident peaks were bounded at 101/100 columns and 808/800
sections on the headless path.

The actual-window `just client-stream-auto` ran the seven-leg route for 168 seconds in both fresh
and saved worlds. Only llvmpipe 23.1.1 / GL was available, so these are correctness/scheduler and
software-present responsiveness results, not AMD/Vulkan hardware-performance acceptance:

| Metric | Fresh world | Reused world |
| --- | ---: | ---: |
| Startup complete 3x3 Safe+Visible | 876 ms | 789 ms |
| Distinct player columns / returned | 33 / yes | 33 / yes |
| Minimum forward Safe / Visible | 0.5 / 1.5 col | 0.5 / 0.5 col |
| Request→visible p50 / p95 / max | 0.511 / 0.859 / 1.082 s | 0.524 / 1.297 / 1.973 s |
| Oldest critical-stage backlog max | 6.894 s | 6.595 s |
| Frame p95 / p99 / worst | 63.01 / 69.75 / 74.55 ms | 75.17 / 92.17 / 106.05 ms |
| Event-loop p95 / max | 62.95 / 74.56 ms | 75.35 / 106.53 ms |
| Input→simulation p95 / max | 70.17 / 79.12 ms | 76.58 / 90.62 ms |
| Input→render p95 / max | 71.65 / 80.95 ms | 76.34 / 92.80 ms |
| TPS / dropped simulation | 19.86 / 0 s | 20.60 / 0 s |
| Resident column / section peak | 150 / 1,200 | 160 / 1,280 |

The reused run's >100 ms event-loop maximum was a measured `render_present` wait on llvmpipe;
non-present streaming p95s were fixed step 0.031, residency 0.528, snapshot 0.015, mesh schedule
0.011 and mesh poll/upload 0.023 ms. The fresh run's respective p95s were 0.033, 0.598, 0.015,
0.011 and 0.026 ms. No fixed ticks were dropped. Fresh peak estimates were 42.2 MiB authoritative
voxel/light, 57.7 MiB snapshots, and 49.3/79.1 MiB logical/capacity mesh storage; reused estimates
were 45.0, 55.3, and 48.1/75.8 MiB. The retained radius is 121 columns; transition, active-light
and presentation retirement account for the bounded observed peaks, and obsolete queued lighting
is now cancelled at eviction.

The old apparent boundary was both geometric and lifecycle-induced: an Euclidean disk made unequal
cardinal/diagonal reach, while a full boundary-light queue held already locally-ready columns before
snapshot/mesh publication. Boundary reconciliation no longer gates Safe or Visible. It remains
eventual, deterministic work; its age is excluded from the critical-queue KPI. Four-direction border
mesh tests verify both shared faces after neighbor arrival, and A→B/B→A/simultaneous lighting tests
verify final seam order independence. Canonical generator hash remains
`e0d1f83c16b281124b7a9c190f667d7eddaa8b2f35ef5ef98ccb4bab434c1bb6`; the independent streaming
sample hash remains `384029af6b21ecf82326bbee468eee62c1353b0af70b9148a4947dee7192a807`.

## DX1 developer tooling diagnostics

See [DX1_REPORT.md](DX1_REPORT.md) for final measurements and acceptance. The full existing M4
regression matrix passes without weakened thresholds or changed canonical hashes. The final
client-stream-auto route visited 33 columns and returned, with dropped fixed time 0.050 seconds
on llvmpipe GL; M4-009 representative-hardware acceptance remains conditional.
The release scripting diagnostic measured 1,790 us compilation, 23,907 us for 1,000 cached query
executions, 558 us for 1,000 command dispatches and 13,470 us for 1,000 scenario steps. A runaway
loop stopped after 10,060 us with a deadline error. These are diagnostic observations, not speedup
claims or a representative GPU benchmark.
The isolated actual-client DX CPU service measurements averaged 4.31 us disabled, 208.52 us
inactive, 271.50 us with a streaming page, 282.21 us with overlays, and 871.23 us with a lightweight
cooperative scenario. Expensive domain/page updates are cached at 250 ms; capture/reload work
is measured separately. No frame-pacing or renderer optimization campaign is part of this slice.

## Pre-network measurement contracts

D-045 and [PRE_M5_AUDIT.md](PRE_M5_AUDIT.md) define RSM1 repeated-route logical-resource/RAM/
driver plateau evidence, P1 refresh/request/render/present-call distributions and state/input
cadence, S1 write amplification/sync/recovery/churn evaluation, and demand-driven DX cost baselines.
No new performance measurements are claimed by the audit. Application present is not scanout;
logical GPU bytes/capacity are not driver VRAM. Correctness thresholds, diagnostic baselines and
representative-hardware acceptance remain distinct; M4-009 stays hardware-conditional.

## DUX1 diagnostic service baseline

The final local acceptance run uses a real 1280×720 llvmpipe OpenGL surface.
Values below are diagnostic service microseconds, not display timing or representative GPU
performance. Each mode has 240 event turns, excluding 20 warmup turns for percentiles.

| Mode | Mean | p50 | p95 | p99 | Max |
|---|---:|---:|---:|---:|---:|
| disabled | 3.99 | 3.91 | 5.31 | 5.66 | 5.80 |
| overview | 99.51 | 74.94 | 296.34 | 379.38 | 428.48 |
| low_page | 117.25 | 83.95 | 333.56 | 456.76 | 825.46 |
| high_page | 120.38 | 80.74 | 323.16 | 434.83 | 712.46 |
| overlay | 139.89 | 79.76 | 454.32 | 553.85 | 675.02 |
| scenario | 668.63 | 643.45 | 916.32 | 1138.14 | 1339.36 |

Disabled and Overview requested no broad providers. Lighting, Renderer and representative
residency/collision overlays collected 34, 32 and 33 times respectively when active; all earlier providers
collected zero after their consumers were removed. Cached domains remain bounded at eleven.
Text refresh follows the inherited 250 ms cadence, with immediate UI-action updates; hidden
legacy F3 collection is suppressed. Low/Medium/High labels are guidance rather than hard
timing guarantees. Whole-service costs include existing polling/maintenance and vary with
workload. No universal threshold, M7 optimization or physical scanout claim is introduced.
See [DUX1_REPORT.md](DUX1_REPORT.md) for gates, reuse and bounds.

## C1 configuration and diagnostics acceptance

| Mode | Mean µs | p50 | p95 | p99 | Max |
|---|---:|---:|---:|---:|---:|
| disabled | 4.28 | 3.91 | 5.31 | 5.59 | 59.30 |
| overview | 107.69 | 70.26 | 483.45 | 634.79 | 671.95 |
| low_page | 120.60 | 74.73 | 510.26 | 641.78 | 820.36 |
| high_page | 142.02 | 77.66 | 560.62 | 787.05 | 1437.14 |
| overlay | 159.62 | 75.29 | 701.56 | 933.51 | 1191.08 |
| scenario | 947.64 | 916.61 | 1315.05 | 1687.10 | 2146.87 |

C1 uses the same llvmpipe correctness surface and 240-turn/20-warmup diagnostic-service protocol.
Disabled/Overview collect no broad domains; active Lighting/Renderer/overlays collect 30/32/34
times, and collect zero once inactive. Settings requests no broad provider. The shared diagnostic
cadence is now configurable from 50–5000 ms; hidden legacy F3 scanning remains suppressed.

Optimized core measurements: 10,000 inactive guards 10.616 µs; 10,000 control lookups
702.471 µs; 200 request/apply transactions 542.445276 ms with bounded metadata/history rebuild.
Native steady state caches typed values and checks a pending flag at boundaries; no semantic
lookup/lock enters voxel/entity inner loops. Transaction costs are rare tooling costs. These
measurements are evidence, not universal timing limits or representative GPU performance.
See [C1_REPORT.md](C1_REPORT.md) for source, streaming, persistence and adapter gates.


## UX1 text and diagnostic acceptance

See [UX1_REPORT](UX1_REPORT.md) for optimized layout/raster samples and text cache bounds.
Unchanged text skips shaping/rasterization/upload (observed CPU update 2.37–7.19 µs).
The glyph soak rebuilds at capacity; one fixed 16 MiB GPU composite page is retained.
These are local diagnostic measurements, not hardware performance gates or P1 scanout evidence.

Final DX overhead fixture, microseconds; 220 measured service samples per mode:

| Mode | Mean | p50 | p95 | p99 | Max | Heavy provider collections |
|---|---:|---:|---:|---:|---:|---|
| disabled | 3.91 | 3.84 | 4.82 | 5.38 | 5.80 | none |
| overview | 99.92 | 63.63 | 448.59 | 573.89 | 625.99 | none |
| low_page | 114.44 | 74.52 | 504.33 | 639.61 | 694.58 | {'lighting': 30} |
| high_page | 397.56 | 73.54 | 572.07 | 715.46 | 59664.35 | {'lighting': 0, 'renderer': 30} |
| overlay | 135.16 | 72.56 | 536.39 | 674.67 | 1051.82 | {'lighting': 0, 'overlays': 31, 'renderer': 0} |

Text shaping is measured separately from DX service. llvmpipe graphical acceptance establishes
correctness only. Existing script wall-clock limits and frozen M4 hashes remain unchanged.

The final high-page run includes one 59.66 ms service outlier (p99 715 µs); this is recorded,
not hidden or promoted to a universal threshold. Heavy collection remains demand-suppressed.

## RSM1 ownership baseline

[RSM1_REPORT.md](RSM1_REPORT.md) records pre-fix and post-fix lifetime measurements. Use
`just rsm1-test` for focused stale/revisit/pressure correctness and `just rsm1-client` for a disposable,
C1-controlled graphical five-cycle route plus unique exploration, return and lighting drain.
Logical counts and payload bytes are correctness evidence; retained buffer/container capacity and
RSS are separate high-water evidence. Driver/process VRAM is unavailable on the llvmpipe acceptance
surface. Its frame time is not a representative GPU target. Frozen M4 hashes and existing correctness
thresholds are unchanged. No P1 presentation change or M7 optimization is included.

## P1 application presentation evidence

The matched debug/software graphical fixture measured 44.13% repeated pan-camera and 36.10% repeated
walking-position transitions before repair; both become zero. Pan mouse-to-camera extraction mean
26.619 -> 5.279 ms; yaw-delta CV 0.960 -> 0.161. This improves visual transform uniformity and local
response, **not a proven FPS or physical cadence gain**. FIFO/redraw/limiter remain unchanged after
a rejected Wayland callback experiment. At reported 60 Hz, software frame intervals still frequently
exceed the 25 ms opportunity threshold. Physical scanout, owner AMD/Vulkan and representative-display
validation remain unavailable/conditional. See [P1_REPORT.md](P1_REPORT.md) and [P1_RESULTS.json](P1_RESULTS.json).
