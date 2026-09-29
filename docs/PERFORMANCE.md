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
