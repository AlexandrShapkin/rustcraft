# WF1 report

Historical evidence for the recorded baseline; registry owns current stage state.

## Baseline

Started from public `463c10edfa6f75b9ed8b7ff3935f4764e6631eea` in the existing authorized recovery
checkout. Launch discovery exposed the old dirty checkout at `4831787`; it was inspected read-only
and left untouched. Prior explicit session authorization identified the existing recovery workspace;
no additional clone/worktree was created. The new workflow requires future agents to surface such
mismatches and obtain explicit checkout direction before edits, not infer a remembered path.

## Scope completed

WF1 inserted immediately before BG1 and activated with the implementation batch. Added portable
read-only just status and focused helper tests, normal command navigation, launch-root verification,
owner workspace convention and separate ignored asset policy. BG1 remains planned; no DX2 retirement.

## Implementation

scripts/workspace.py discovers the caller's Git root and reports repository/command directories,
branch, full HEAD, upstream, clean/dirty (including untracked files), selected remote URL, registry
focus and focused state. It handles unborn/detached HEAD and missing upstream/remote explicitly.
Invalid Git/registry context fails clearly. Git optional locks are disabled; there is no fetch,
checkout switch, clone, config write or registry mutation. HTTPS embedded credentials are redacted.
The helper is Python 3.11+ standard library and has no fixed workspace-path dependency.

AGENTS/CODEX/stage skill begin with git rev-parse --show-toplevel and HEAD/status verification.
Agents use the launched checkout, never silently invent/select another clone/worktree. The owner
convention is ~/Projects/rustcraft; it creates/migrates nothing and is not enforced by tests.
/tmp and milestone acceptance clones are temporary/evidence workspaces, not normal development.

WORKFLOW/TOOLING foreground just client, client-release, smoke, sample-game, test and ci. Just recipes
operate at the discovered justfile directory by default; helper status exposes its actual working
directory. Existing interpreter override supports installations named python rather than python3.
Historical acceptance commands remain unchanged; broader retirement stays with DX2.

Ignored reference/assets/vanilla-b1.7.3/ is local owner-supplied proprietary data, separate from
tracked source. No assets were committed/copied, and no legacy checkout is required to provide them.

## Evidence

Six isolated Git fixture tests cover nested paths with spaces, actual HEAD/upstream/remote,
clean/untracked/modified state, unborn/detached/missing upstream, invalid registry/outside-repo errors,
caller-directed JSON output and credential redaction. Fixtures use temporary roots, not owner paths.
Public CI runs the helper tests on Ubuntu and Windows; just ci includes them locally.

## Measurements

Not a performance stage; no gameplay/render/storage changes or performance claims.

## Tests

just workspace-test: six passed. just docs-test: 21 passed. just docs-check passed with 12 stages.
just status reported the actual recovery root, command directory, implementation baseline, origin/main,
expected dirty implementation work and WF1 active. Diff checks passed before publication.
No expensive local Rust rebuild repeated: Rust source is unchanged from the accepted C2 baseline.
Public CI retains all required Rust gates with CARGO_BUILD_JOBS=2.

## Issues resolved

No issue closed or new ticket created. Live stage:WF1 query returned no owned issue.

## Issues remaining / waivers

None owned by WF1. #15 remains open under BG1 and was untouched.

## Known limitations

The documented owner location is a convention; this maintenance does not relocate the recovery clone
or owner saves/assets. Status reports live local facts, not remote freshness or stage authorization.
Existing just/toolchain prerequisites still apply; no broad cross-platform recipe retirement performed.

## Implementation SHA

`83b2137b4edd7b3df27029e0fa6d2a45be9e4dfd` — workspace discovery helper, tests and workflow convention.

## CI

[Implementation CI 37408417592](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37408417592):
Ubuntu success, Windows success, including workspace helper tests and all retained Rust gates.
Closeout CI is discoverable by the closeout commit below; both platforms are required.

## Closeout SHA

The commit titled `Record WF1 acceptance` introducing this finalized report is the closeout identity;
resolve with `git log -1 --format=%H --grep='^Record WF1 acceptance$'`.
WF1 closed, focus BG1, BG1 planned. No BG1 implementation or broad DX2 work was started.
