---
name: stage-execution
description: Execute an explicitly authorized repository stage with progressive context, issue acceptance, validation and bounded closeout.
---

# Stage execution

1. Discover `git rev-parse --show-toplevel` from the launch directory; run `just status`, then verify
   HEAD/log and remote baseline. Work in the launched checkout; do not silently select a remembered
   path or create another clone/worktree without explicit owner authorization. Inspect unknown dirty changes;
   never reset/discard them to reach a remembered baseline.
2. Run `just codex-context ID --offline` for local work/recovery. Read that contract and only
   necessary pointers/decision sections. Verify actual source/evidence; query live issues at
   planning/reconciliation and PR/CI at publication, not continuously during local work.
3. Implement only the owner-authorized stage. Registry focus is navigation, not authorization;
   stop for explicit disposition of out-of-scope blockers rather than silently implementing later work.
4. Run focused regression/proof validation first, then wider gates required by the contract. Use
   conservative build jobs and sequential expensive validation. Update canonical docs only when
   contracts change, preserving historical reports and generated-marker boundaries.
5. Follow [issue policy](../../../docs/DEFECTS.md) and canonical [workflow](../../../docs/WORKFLOW.md).
   Use a focused branch, preferably one per stage. Complete local implementation/validation and
   reconcile owned issues; include report/evidence and authorized proposed registry closeout in
   the same branch/PR where appropriate. No unresolved owned P0/P1 may silently remain.
6. Review the complete diff; run docs-sync/docs-check/docs-test. Commit/push only when authorized,
   then publish a compact PR (Scope, Changes, Validation, Issues, Deferred). Use `Closes #N` only
   for fully satisfied acceptance, otherwise `Refs #N`. Wait for final Ubuntu/Windows PR CI.
7. Merge to main only when authorized and acceptance/final PR CI pass. Merge makes proposed
   closeout effective; separate implementation/closeout main pushes or two main CI passes are not
   required. GitHub automation may close satisfied issues; add a closure comment only for extra
   explanation/evidence. Verify actual main, issue and relevant milestone state after merge;
   milestone closure still requires its delivered goal and reconciliation, not one stage merge.
8. Stop before the next stage. Use report/issue notes to make interruptions recoverable without chat.

Navigation/precedence: [documentation index](../../../docs/INDEX.md).
