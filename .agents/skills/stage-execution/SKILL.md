---
name: stage-execution
description: Execute an explicitly authorized repository stage with progressive context, issue acceptance, validation and bounded closeout.
---

# Stage execution

1. Discover `git rev-parse --show-toplevel` from the launch directory; run `just status`, then verify
   HEAD/log and remote baseline. Work in the launched checkout; do not silently select a remembered
   path or create another clone/worktree without explicit owner authorization. Inspect unknown dirty changes;
   never reset/discard them to reach a remembered baseline.
2. Run `just codex-context ID`; use `--offline` for local recovery. Read that contract and only
   necessary pointers/decision sections. Verify actual source, relevant evidence and owned issues.
3. Implement only the owner-authorized stage. Registry focus is navigation, not authorization;
   stop for explicit disposition of out-of-scope blockers rather than silently implementing later work.
4. Run focused regression/proof validation first, then wider gates required by the contract. Use
   conservative build jobs and sequential expensive validation. Update canonical docs only when
   contracts change, preserving historical reports and generated-marker boundaries.
5. Follow [issue policy](../../../docs/DEFECTS.md) and [workflow](../../../docs/WORKFLOW.md): evidence,
   non-auto-close implementation references, required acceptance/CI, final issue comment then closure.
6. Review diff, commit and push non-force only when authorized. Wait for required Ubuntu/Windows CI;
   publication or code completion alone does not satisfy acceptance.
7. Reconcile stage-owned issues, record a baseline-specific report and evidence-index pointer. No
   unresolved owned P0/P1 may silently remain. Close registry state/advance focus only after accepted
   closure and deliberate owner-authorized planning; run docs-sync/docs-check.
8. Stop before the next stage. Use report/issue notes to make interruptions recoverable without chat.

Navigation/precedence: [documentation index](../../../docs/INDEX.md).
