# F1 report — accepted unified client path and field validation

Historical acceptance evidence for the recorded implementation baseline. The registry owns current
stage state. The owner authorized final closeout after matched hardware acceptance; A1 implementation
was not started.

## Baseline

Started from clean public `426a44b651146c1d41b352701ba9538950d1ebdb` (DOCINFRA1 acceptance).
Final implementation baseline: `3d8f48b47ec20f2c4feb5c0e432dec4a94b9ec6e`.
Publication workspace: `/tmp/rustcraft-r2-recovered`; old dirty source clone untouched.

## Scope completed

One normal rustcraft-client runtime/input path, capability-gated developer operations, independent
game-owned gameplay mode, production input-route regressions, sequential dev/release hardware
measurements, normal owner-controlled release acceptance, checkpoint/frame correlation and removal
of one proven duplicate file sync. No A1 remote authentication, DX2 alias retirement, P1 presentation
redesign or save-format migration.

## Implementation

`session.rs` composes a bounded local principal and Control grants independently of gameplay mode.
Role labels explain grants but do not authorize effects. Default local diagnostic grants do not
permit world mutation, configuration writes or trusted Rhai; explicit trusted launches grant
`script.load`. Interpreter existence never selects keyboard semantics.

WindowEvent public physical key/state/repeat/text feeds one complete production client router.
Portable tests enter that exact production payload boundary (winit KeyEvent has private platform
fields) and feed actual Focused events. This is not shortcut-helper-only coverage or host-global
keyboard injection. Held F3 digits are consumed even when privileges deny effects; bare F3 toggles
on release; F4/repeat, ordinary hotbar digits, inventory, cursor and focus transitions use this router.
Focus loss clears the chord and entire human controller. Native mouse ingestion and semantic
LocalHumanController intent remain the normal gameplay path.

`just client` is canonical. client-survival/dev-client delegate to it; specialist scenario/acceptance
aliases wrap the same runtime and remain until DX2 proves retirement safe. Survival/Development
state does not choose a parallel implementation. The server retains its distinct authority process.

## Evidence

Owner-confirmed AMD Radeon Vega 8 / RADV / Vulkan evidence is accepted. Automated dev/release runs
use disposable state and the same normal client, with six thirty-second phases: stationary, pan,
walk, walk_pan, fast_pan and walk_fast_pan. Original pan is 325 counts/s (about 37.24 degrees/s);
fast pan is 1500 counts/s (about 171.89 degrees/s) through normal mouse ingestion at unchanged
0.002 radians/count sensitivity, not direct presented-camera animation.

Final top-level automated result is `measured`, with both effective window_pixels equal to
[1280, 720]. The generic fixed physical client-area request preserves one window/runtime path and
comparison checks. Earlier evidence at `1c30f15526ee304b49bd0706eed742d1b55e785f` remains valid:
both profiles individually measured, but top-level comparison failed solely because dev was
1280x662 and release 1920x1012. That reproducibility defect is resolved by the final rerun;
it was not evidence of a camera/render defect.

The accepted manual release run used the same AMD/RADV/Vulkan machine, clean worktree, 1280x662,
45 seconds of normal owner-controlled input, status measured and zero long frames in that phase.
The owner reports the camera is substantially better than before and sufficiently smooth during
ordinary walking and fast mouse-look. Manual mode was unaffected by automated sizing and needed
no repeat. Existing hardware evidence and persistence correlation are preserved.

Ignored target/f1/run-NONCE directories contain root/per-profile schema-v1 summaries and timeline
JSON. They record actual adapter/driver/backend, profile, window/present mode, effective C1 config,
world/generator, frame/render/present cadence, duplicate transforms, input-camera timing and fixed
steps/catch-up/drops. The owner supplied acceptance outcomes in this session rather than the raw
numeric distribution files; this report does not invent their values or a dev/release speedup.

## Measurements

Representative release evidence reveals no long-frame problem requiring P1 reopening; normal-speed
owner-visible release behavior is accepted. Dev and release measured all six phases at matched
1280x720; no unsupported claim that debug is equally smooth or a specific percentage faster is made.
Use `just client-release` for the accepted play profile; `just client` remains normal development use.

Frame intervals, worker checkpoint start/end/publication envelopes, main-thread persistence service
spans and fixed-step budgets share one monotonic Instant timeline. Summaries distinguish worker
intersection from measured main-thread service cost and time inside long frames. The accepted
correlation evidence does not justify attributing judder to worker sync tails; no persistence-induced
representative release stall requiring repair was established. Publication timings include replacement,
directory work and cleanup rather than isolated fsync. No causal claim follows from overlap alone.

The recovery workspace lacked /dev/dri and usable Vulkan hardware. Its preflight correctly failed
before save opening; software rendering was never substituted for representative owner evidence.

## Tests

Focused production-router/capability/digit/focus/repeat, presentation/input extraction, wrapper
isolation/schema/GPU rejection and fixed-size/mismatch tests passed. Latest local client validation:
57 passed, one existing R2 GPU-only test ignored; seven wrapper tests passed; client all-target/all-feature
Clippy, formatting and docs-check passed. Earlier scripting validation: 18 passed; world: 41 plus two
example tests. Earlier full just ci: 331 nextest tests passed, two existing tests skipped, with workspace
checks and strict Clippy green. Closeout docs-sync/check and all 21 docs tests passed. Sequential just ci passed again: 333
nextest tests, two existing skips, workspace/all-target check, formatting and workspace/all-feature
Clippy with warnings denied. Seven wrapper tests also passed. Local symbol/incremental overrides are recovery validation settings, not owner build profiles.

## Persistence durability conclusion

Only duplicate callback file-content sync was removed. Exact pinned atomicwrites 0.4.4 ordering is
callback, tmpfile.sync_all(), then commit. Unix atomic replacement syncs required parents; Windows
uses MoveFileExW WRITE_THROUGH and REPLACE_EXISTING. RustCraft's required parent sync remains;
Windows directory durability retains its documented platform limitation. Successful completion still
follows successful store publication. No measured durability speedup is claimed.

Fault/reopen regressions preserve previous acknowledged checkpoints on publication failure, retry
successfully and recover a torn newest slot. Existing S1 interruption/recovery and D-052 entity-transfer
regressions remain green: exact durable player receipt before full pickup retirement, quantity/revision-aware
partial receipt, destination durability before migration retirement, failed-generation transient cleanup,
and no persistence acknowledgement on failure. Save formats and transfer ordering are unchanged.
P1 20 TPS authority, transient interpolation and local preview/rebase are preserved; R2 remains intact.

## Issues resolved

[Issue #18](https://github.com/AlexandrShapkin/rustcraft/issues/18) is reconciled with the production
route tests, final hardware/owner acceptance and both-platform implementation CI. Final evidence
[comment](https://github.com/AlexandrShapkin/rustcraft/issues/18#issuecomment-6002552887)
preceded completed closure; current GitHub state was verified CLOSED/COMPLETED. Unrelated issues
are untouched.

## Issues remaining / waivers

No other F1-labelled issue was found. No F1-owned acceptance requirement is waived or left pending.
A1 remains planned and its implementation was not started.

## Known limitations

Application timing is not physical scanout or input-to-photon measurement. Subjective acceptance is
the owner's observation, not an automated inference. These limits do not block F1 because representative
owner-visible normal-speed behavior is explicitly accepted. Raw timing distributions remain in owner
artifacts; only supplied outcomes are reproduced here.

| Acceptance | Accepted evidence |
| --- | --- |
| A–C: one path, mode/grants separation | One ClientApp/router; game-owned mode; capability-authorized effects. |
| D–F: F3/F4/digits/focus/repeat | Complete production-route tests in granted/denied normal contexts. |
| G–H: workflow/aliases | Canonical just client; compatibility/harness aliases retained until DX2. |
| I–K: hardware and normal-speed behavior | Matched six-phase dev/release measured; manual release and owner smoothness accepted. |
| L: persistence correlation | Shared monotonic worker/main/frame traces; no unjustified causal attribution. |
| M: durability | Proven duplicate only; atomic/file/directory ordering and recovery tests preserved. |
| N: P1/S1/R2/D-052 | No redesign/format change; correctness regressions and public CI green. |
| O: issue reconciliation | #18 final evidence and completed closure; no other owned issue. |

## Implementation SHA

- `ed33bd7a26789f583597d0cc448e08b3a3e4a5ff`: unified input/grants, durability proof, field harness.
- `39afe363365db2ddcd409da40f6d4b0f3c2ad4b5`: concise checkpoint/frame correlation.
- `1c30f15526ee304b49bd0706eed742d1b55e785f`: fast-motion and manual release evidence.
- `3d8f48b47ec20f2c4feb5c0e432dec4a94b9ec6e`: matched automated window reproducibility.

## CI

Each implementation run was reverified: Ubuntu success and Windows success.

- [Core 37358378714](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37358378714).
- [Correlation 37359466704](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37359466704).
- [Fast/manual 37365477477](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37365477477).
- [Window reproducibility 37369126446](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37369126446).

Closeout CI is associated with the closeout commit in public GitHub Actions; formal publication
acceptance requires both platforms green before the final user report.

## Closeout SHA

The commit containing this accepted report and deliberate registry transition, titled Record F1
acceptance, is discoverable in Git history; its SHA cannot be embedded in its own contents.
