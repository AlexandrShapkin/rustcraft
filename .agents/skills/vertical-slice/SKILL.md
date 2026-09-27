---
name: vertical-slice
description: Implement a coherent end-to-end RustCraft capability that crosses the necessary crates, runs headlessly where possible, and finishes with validation instead of stopping at interface stubs.
---

Use this when a requested feature spans multiple project layers.

1. Read the relevant architecture docs and existing implementation.
2. Define the smallest end-to-end behavior that proves the feature.
3. Implement through real consumers, not placeholder traits with no caller.
4. Add focused tests for the durable contracts.
5. Run the nearest `just` validation recipes.
6. Fix blockers immediately; record unrelated non-blocking defects.
7. Update `docs/DECISIONS.md` only for durable architecture choices.
8. Finish with what actually works and measured evidence, not aspirational claims.
