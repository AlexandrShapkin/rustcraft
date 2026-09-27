# Performance strategy

The project is performance-oriented, but optimization claims require evidence.

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
