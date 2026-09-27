---
name: performance-investigation
description: Investigate a measured RustCraft performance problem, establish a repeatable workload and profile before changing data structures, concurrency, allocation or low-level optimizations.
---

1. Define the workload and metric.
2. Record hardware/build/profile details that materially affect the number.
3. Establish a baseline using the simplest suitable tool (`hyperfine`, benchmark harness, profiler).
4. Locate the bottleneck before proposing exotic optimization.
5. Make one coherent change or tightly related set of changes.
6. Re-run the same workload and correctness checks.
7. Keep the change only if the result is meaningful and does not degrade required behavior.
8. Report before/after numbers and remaining uncertainty.
