# F1 report — implementation progress, hardware acceptance pending

Evidence for the recorded baseline; not a claim that F1 is closed. Registry remains F1 active,
focus F1; A1 planned. Issue #18 remains open until field acceptance and public CI are reconciled.

## Baseline

Clean public `426a44b651146c1d41b352701ba9538950d1ebdb` (DOCINFRA1 acceptance).
Working/publication clone: `/tmp/rustcraft-r2-recovered`; old dirty source clone untouched.
Owner authorizes implementation/publication while hardware evidence is pending, not F1 closure.

## Scope completed

One production WindowEvent input router for normal/developer/scenario sessions. Launch grant
composition is independent of game-owned Development/Survival state. `just client [ARGS…]` is
canonical; survival/dev aliases delegate to it. Existing specialist modes remain harnesses;
DX2 retirement and A1 remote/trust design are not performed.

## Implementation

`session.rs` records one local principal, an explanatory grant role, and existing Control
capabilities. No mechanism branches on role names. Default local sessions can read diagnostics and
configure their transient selector; `--player` denies selector changes; `--devtools` explicitly
admits trusted Control and Rhai. Mutation/config writes/script loading remain absent by default.
Console evaluation and scenario admission preserve the supplied grants. Missing scripts directories
do not prevent untrusted diagnostic startup. Interpreter presence does not select input semantics.

WindowEvent copies public physical key/state/repeat/text into a safe payload used by one complete
client router. It includes developer effects, ordinary controller keys/digits, inventory and focus
transitions. KeyEvent's private platform payload cannot be constructed portably; deterministic
integration tests enter that exact production payload boundary and also feed real Focused events.
The exploratory shared-desktop X11 driver lost marker keys and was replaced; its ignored research
artifact is retained locally under target/f1/research, not used as acceptance evidence.

Held F3 consumes all digit states even on denial; bare F3 toggles on release; F4 repeats cannot
retrigger; focus loss clears the entire human controller and chord. Trusted Rhai stays explicit.
No P1 interpolation/preview/rebase or scheduling policy is redesigned.

Checkpoint callbacks no longer call an extra file sync. Exact installed atomicwrites 0.4.4
`write_with_options` calls callback, then `tmpfile.sync_all()`, then commit. Unix `replace_atomic`
renames and syncs source/destination parents; Windows uses MoveFileExW with WRITE_THROUGH and
REPLACE_EXISTING. RustCraft still syncs its required parent after publication (Windows directory
sync remains the previously documented unavailable guarantee). Only duplicate callback file-content
sync is removed. No measured speedup is claimed; generic/player/world save formats are unchanged.
Worker completion follows successful store return; existing revision/receipt/ownership logic is
unchanged. New transient worker start/end Instants are never persisted.

## Evidence

`just f1-client` wraps the same runtime in sequential dev/release processes. All world/config/cache
paths are disposable under target/f1/run-NONCE. Application Vulkan preflight runs before asset/save
opening; the actual surface adapter is checked again. Missing/non-AMD/non-RADV/non-Vulkan adapters
fail explicitly. One run contains four thirty-second phases; readiness and workload share a bounded
420-second application timeout. Automated pan uses native mouse ingestion and walking uses normal
LocalHumanController intent. This is not physical mouse/input-to-photon or subjective display evidence.

Per-profile summary.json records adapter/device/driver/backend, actual build profile, surface/present
mode, window size, world seed/generator, exact effective C1 config, frame/render/request/redraw/
present/submit/cpu/acquire distributions, authoritative/shown transforms, duplicate counts,
input-to-camera/authority distributions and fixed-step counts. timeline.json records bounded frame
intervals, checkpoint worker publication envelopes, main persistence service spans and fixed budgets
on one monotonic clock. Long-frame overlap is correlated separately with workers and main services;
worker overlap alone cannot establish main-thread blocking. Each phase summary also includes
long-frame counts overlapping workers/main services and distributions of main persistence service
cost/time inside long-frame intervals, so the top-level returned summary supports initial review.
At least twenty successful player checkpoints are required. Publication metrics are envelopes, not isolated fsync counters.

## Measurements

Hardware unavailable in recovery: /dev/dri absent; vulkaninfo reports no valid GPU. Radeon ICD
installation alone does not establish a usable adapter. No software-GPU substitute or AMD result
is claimed. Dev/release stationary/pan/walk/walk+pan distributions and checkpoint correlation are
pending owner-machine evidence. No subjective or physical scanout acceptance is claimed.

## Tests

Focused production routing, local grant, presentation/correlation, console admission and storage
regressions run with CARGO_BUILD_JOBS=2. Local quota-constrained builds disable debug symbols and
incremental compilation only for validation; normal owner dev/release profiles are unchanged.
Client tests: 55 passed, one existing explicit R2 GPU test ignored. Scripting: 18 passed. World:
41 passed plus two example tests. New publication-failure test preserves the previous acknowledged
checkpoint, reports failure, retries successfully and recovers from a torn newest slot. Existing
S1 interruption/reopen and entity-transfer tests are retained. Python wrapper safety/schema/rejection
tests pass. Local `just ci` passed: docs tests/check, formatting, workspace/all-target checking, 331
nextest tests passed (two existing ignored tests skipped), and workspace/all-feature Clippy with
warnings denied. The real `just f1-client dev` command compiled/started its Vulkan preflight,
failed with the expected no-adapter error, wrote valid schema-v1 failure JSON under
`target/f1/run-1791225560351787334/`, and stopped before save opening/release launch. This is
failure-path validation, not representative hardware acceptance. Public CI is still required.

## Issues resolved

None closed. Implementation references #18; required hardware/CI acceptance is pending.

## Issues remaining / waivers

#18 remains open. Owner explicitly requires AMD/RADV/Vulkan evidence before closure. This is a
pending requirement, not a waiver. No new issue is needed for the inaccessible recovery GPU.

## Known limitations

| F1 acceptance question | Current evidence / remaining gate |
| --- | --- |
| A: one normal runtime/input path | Same ClientApp/WindowEvent/router/composition; startup variants only set state/grants/harnesses. |
| B: game mode separate | Same initialize_simulation; game/session mode and launch grants are independent. |
| C: capabilities | Local Control grants, including explicit script.load; no role-name effects. |
| D–F: F3/F4/digits/focus/repeat | Deterministic production-router tests; no interpreter-existence branch. |
| G–H: canonical workflow/aliases | just client accepts arguments; historical aliases delegate; specialist harnesses retained until DX2. |
| I–K: hardware/smoothness | Pending actual AMD/RADV/Vulkan dev/release phases; no conclusion yet. |
| L: checkpoint/frame correlation | Instrumentation/tests ready; representative correlation pending. |
| M: duplicate durability work | Proven callback duplicate removed; library/pre-publication/replace/directory ordering preserved; focused recovery regression. |
| N: P1/S1/R2/D-052 | No presentation policy/save format/transfer logic change; existing regressions retained; local wider regression green; public CI pending. |
| O: issue reconciliation | #18 open; no acceptance closure yet. |

## Implementation SHA

Core implementation: `ed33bd7a26789f583597d0cc448e08b3a3e4a5ff` (Refs #18).
A follow-up adds concise per-phase checkpoint/frame overlap and main-service cost summaries;
its identity is discoverable through Git history. Hardware acceptance is still pending.

## CI

Core implementation [CI 37358378714](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37358378714):
Ubuntu success; Windows success. The field-summary follow-up also requires both platform jobs;
its final public run is discoverable from that commit. This is implementation CI, not F1 hardware acceptance.

## Closeout SHA

None. F1 stays active until owner hardware evidence and all acceptance requirements pass.

## Normal-speed hardware follow-up

The owner confirms existing AMD/RADV/Vulkan evidence is valid and automated motion looked smooth,
but its low pan rate leaves normal-speed subjective acceptance inconclusive. Existing evidence
must be preserved. A narrow harness extension retains the original phases and adds fast_pan and
walk_fast_pan through production mouse ingestion, plus a 45-second owner-controlled release
recording via `just f1-manual`. No P1 presentation, persistence or runtime authority design changes
are made. Faster hardware measurements and the owner's observation remain pending; F1 stays active,
Issue #18 stays open, and A1 stays planned.
