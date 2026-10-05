# DX2 — tooling equivalence, workflow convergence & retirement

## Goal

Tooling equivalence, workflow convergence & retirement.

## Context

The [registry](../stages.toml) owns execution order, state and focus. This contract owns scope and acceptance. Its authored requirements were extracted from the roadmap at `81b7b3a`.

## Current state

Verify actual source and labelled issues before starting; historical reports are baseline evidence, not live status.

## Target state

Meet the accepted requirements below through the public mechanism/game-policy boundary.

## Scope

Owner: PM5-009 and obsolete developer workflow debt.

Goals:

- prove canonical `just` workflows exercise the same real runtime paths used by humans and CI;
- retire redundant milestone-only aliases and alternate launch paths after equivalence is demonstrated;
- converge client scenarios/acceptance onto Control/scenario orchestration rather than bespoke client
  variants where practical;
- retain specialist low-level diagnostics only where they test a unique contract;
- update tooling/docs so a contributor does not need historical milestone knowledge to run, debug or
  validate the project;
- keep public CI deterministic, bounded and free of private assets.

DX2 owns a generic first-party diagnostic content package using public Game API and project-owned
assets, extending independent sandbox-style cube/material/emission/model/item/entity/medium/UI proofs
where contracts exist; READY1 verifies it. No Minecraft admin blocks hardcoded into engine-core.
The conceptual core command surface is client/server/test/ci, with specialist harnesses retained where
they prove unique contracts. See [TOOLING](../TOOLING.md).

DX2 is cleanup and equivalence proof, not a feature milestone.

## Invariants

Preserve all explicit semantic, authority, performance and compatibility constraints in Scope; architecture/domain documents own shared invariants.

## Out of scope

Do not implement other stages or expand the explicit exclusions in Scope without owner authorization.

## Implementation constraints

Use the navigation anchors and relevant decisions in the registry. Changes to these hints do not change public API ownership.

## Acceptance

Prove every required behavior and acceptance condition in Scope. Do not infer closure from code completion alone.

## Validation

Run focused regression/proof checks first, then required wider headless/render/platform checks and Ubuntu/Windows CI. Record conditional hardware evidence honestly; never invent budgets.

## Documentation updates

Update changed canonical contracts and record `docs/DX2_REPORT.md` using the [report template](../templates/REPORT.md); add evidence navigation in [EVIDENCE_INDEX](../EVIDENCE_INDEX.md).

## Issue reconciliation

Query the registry issue label. Follow [DEFECTS](../DEFECTS.md) and [WORKFLOW](../WORKFLOW.md); account for closure, justified owner transfer or explicit waiver. No owned P0/P1 may silently remain unresolved.

## Closeout

After acceptance and required CI, record report/evidence, reconcile issues, update registry state/focus deliberately, run docs-sync/docs-check, and stop before the next stage. M5 activation remains a separate owner-authorized pass.
