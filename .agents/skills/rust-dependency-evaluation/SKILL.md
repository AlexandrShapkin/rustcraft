---
name: rust-dependency-evaluation
description: Evaluate a Rust crate or tool before adding it to RustCraft, using current documentation, maintenance signals, API fit, runtime cost and a small spike when the choice affects architecture.
---

1. Identify the concrete capability needed; do not search for a framework without a problem.
2. Prefer official/current documentation. Use Context7 when available.
3. Check maintenance, MSRV/toolchain constraints, dependency weight and platform implications.
4. For architecture-sensitive alternatives, build the smallest representative spike/benchmark.
5. Choose the smallest dependency that satisfies the actual contract.
6. Record the decision only when it has durable architecture impact.
