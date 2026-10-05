# READY1 — final pre-M5 readiness gate

## Goal

Final pre-m5 readiness gate.

## Context

The [registry](../stages.toml) owns execution order, state and focus. This contract owns scope and acceptance. Its authored requirements were extracted from the roadmap at `81b7b3a`.

## Current state

Verify actual source and labelled issues before starting; historical reports are baseline evidence, not live status.

## Target state

Meet the accepted requirements below through the public mechanism/game-policy boundary.

## Scope

READY1 is a verification/freeze gate, not another implementation campaign. It validates the
owner-approved post-audit expansion, not merely the original PRE_M5 audit. Required entry evidence:

- one normal client path; gameplay mode distinct from privilege; role → capability authorization;
- A1 semantic external identities and stable entity references/provenance;
- C2 common composition, block-local canonical state and bounded compiled hot paths;
- BG1 non-full/arbitrary static geometry, separate collision/selection/occlusion/light contracts;
- no Minecraft policy in generic engine/renderer; diagnostic content independence;
- DX2 equivalence and RF1 accepted ownership/code graph/navigation;
- VS1 static/kinematic/dynamic spaces, reference frames, hierarchy/lifecycle and persistence/reopen;
- moving-space lifetime bounded; transform motion avoids voxel/remesh churn; static fast path acceptable;
- S1 triggers rechecked against space workloads, without automatic database/backend migration;
- current hardware field evidence understood, including checkpoint/frame correlation and build profile;
- Ubuntu/Windows green; no known P1 durability/identity blocker.

Goals:

- run a fresh whole-repository architecture and risk audit against the post-VS1 tree;
- confirm no pre-M5 BLOCKER/HIGH risk remains without an explicit accepted deferral;
- verify the canonical client/server/tooling workflows from a clean checkout;
- verify public Ubuntu/Windows CI and representative local graphical/headless smoke;
- recheck stable semantic external identities, capability/provenance boundaries, resource identities,
  unified C2 content-definition contracts, BG1 model/shape identities, persistence compatibility and
  generator identities;
- verify no historical private/proprietary assets are required by public build/test/release paths;
- confirm M5 can add transport/replication without first restructuring client startup, code ownership,
  content-definition/model boundaries, persistence or resource identity again;
- produce a concise `READY1_REPORT.md` and exact M5 entry contract.

READY1 must not hide unfinished architecture work by renaming it "M5 follow-up". If the audit finds a
real blocker, create a bounded repair stage before M5 rather than starting networking around it.

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

Update changed canonical contracts and record `docs/READY1_REPORT.md` using the [report template](../templates/REPORT.md); add evidence navigation in [EVIDENCE_INDEX](../EVIDENCE_INDEX.md).

## Issue reconciliation

Query the registry issue label. Follow [DEFECTS](../DEFECTS.md) and [WORKFLOW](../WORKFLOW.md); account for closure, justified owner transfer or explicit waiver. No owned P0/P1 may silently remain unresolved.

## Closeout

After acceptance and required CI, record report/evidence, reconcile issues, update registry state/focus deliberately, run docs-sync/docs-check, and stop before the next stage. M5 activation remains a separate owner-authorized pass.
