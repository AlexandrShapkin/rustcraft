# Performance strategy

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
ungenerated deterministic neighbor. Client startup now creates/opens named worlds and autosaves
mutations, but initial assembly waits synchronously for worker results and residency remains a fixed
3x3 neighborhood. Those limitations make these measurements a foundation, not M4 completion.

The M4 player record now uses an outer v2 envelope and three Minecraft-owned component codecs.
Changed revisions checkpoint every two seconds by default (configurable 1–2 seconds); one
coalescing worker keeps filesystem sync off the simulation/render tick. Checkpoint diagnostics
report encoded bytes, write time, sync time and total worker latency; sync measures OS-requested
file/directory synchronization, not guaranteed physical-media latency. The prior manual 116-byte
single-payload save measured 0.072 ms encode and 3.307 ms atomic write+sync, with 0.032 ms read and
0.029 ms decode on reopen. It is a pre-component legacy-format sample, not a comparison against
the final async/two-slot layout. The empty inventory payload was 65 bytes before component framing;
the componentized current payload has not yet been release-profile measured. In a debug-profile
scratch-world run, checkpoint writes reported 0.023–0.035 ms write time, 11.8–68.4 ms requested
file/directory sync time, and 12.7–68.4 ms total worker time for the tiny player record. This local
filesystem sample demonstrates why sync stays off the simulation/render thread; it is not physical
media latency or a release benchmark. A hard crash may lose mutations since the last successful
checkpoint. The isolated scratch client was closed without a graceful-save completion and reopened
at checkpoint revision 203 with all three components, demonstrating recovery of the latest
checkpoint; unsaved motion after it remains within the configured 1–2 second window. Underwater
fog adds only a camera uniform and fragment distance/mix work on world opaque/translucent fragments;
no GPU timing comparison is available yet, so no frame-time cost is claimed.
