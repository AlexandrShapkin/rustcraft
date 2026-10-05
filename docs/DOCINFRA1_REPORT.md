# DOCINFRA1 documentation infrastructure report

Historical maintenance evidence; not a product milestone or stage activation.

## Baseline and scope

Starting clean public main: `81b7b3abd643dbae26e5840fc21b06d2f4367064` (ISSUE1).
R2 evidence is retained; GitHub Issues #1–#18 remain unresolved and unchanged. No production Rust,
Cargo dependency, gameplay, persistence-format, input, renderer or network implementation changed.

## Documentation architecture

`docs/stages.toml` version 1 owns focus, ordered ID sequence and stage entries. Entries contain ID,
title, bounded state, contract path, issue label, relevant docs, decision IDs and source-area hints.
States: planned, active, blocked, closed, inactive. At most one active stage exists and it must be
focus; focus references an existing stage. Inactive is intentionally not currently actionable.
Closed extracted stages remain in sequence. Older completed history remains in the roadmap/reports.
Registry metadata contains no domain implementation design.

The eleven future contracts were extracted from the baseline ROADMAP, preserving authored scope,
constraints and acceptance while removing mutable status sentences. Concept documents remain canonical
(e.g. VOXEL_SPACES), not generated or copied wholesale. Only the stage index and marked ROADMAP block
are generated. Root ROADMAP is a stable pointer. INDEX defines precedence; EVIDENCE_INDEX locates
accepted reports/results/recorded identities without reproducing measurements.

## Commands and validation coverage

One Python 3.11+ stdlib script owns check/sync/context/new-stage. Just exposes docs-test, docs-check,
docs-sync, codex-context, stage-new and project-tree. State/focus remain explicit manual TOML edits.
Doctor reports the new Python baseline; Ubuntu/Windows CI installs Python 3.11 and validates docs
before Rust. Local ci runs offline documentation checks before existing Rust gates.

Check validates schema/version, unique IDs/sequence, complete sequence, focus/active/state invariants,
contracts/headings, document and source paths/globs (including exact case), decision references and
duplicate IDs, label syntax, templates, generated-view consistency, root pointer and absent tree
snapshot. Local authored Markdown file links are checked; remote URLs and fragment IDs are not fetched
or validated. No private assets or GitHub credentials are required. Context optionally queries at most
ten issue summaries with an eight-second timeout; offline/unavailable enrichment is nonfatal.

## Measurements and proof

Before: AGENTS 70 lines / 3,686 bytes; 13 mandatory domain/operating documents / 258,364 bytes
(excluding AGENTS). After: AGENTS 35 lines / 2,433 bytes; one always-on document (AGENTS), followed
by context and the selected contract, with no unconditional domain/history reading list.

Measured offline stdout before publication (Git HEAD + dirty-path count included):

| Stage | Context lines | Context bytes | Contract bytes | AGENTS + context + contract bytes |
| --- | --- | --- | --- | --- |
| F1 | 31 | 1021 | 4805 | 8259 |
| C2 | 31 | 957 | 5488 | 8878 |
| VS1 | 36 | 1177 | 4911 | 8521 |

No token-count estimate is used. Context differs in docs, decisions, sources and label by task; it
never dumps document/issue bodies or a diff/tree. Relevant documents are selected later as needed.

21 stdlib unit tests passed: IDs/sequence/focus/active/state/label rejection; source paths/globs;
local broken/wrong-case/space/reference links; duplicate decisions; contract headings; root pointer;
snapshot/image-target rejection; Unicode stage titles; unavailable/timeout GH; offline success without GH; sync idempotence (bytes and
mtime unchanged); stage insertion, duplicate/unknown targets and rollback on injected write failure.

A full current-repository temporary fixture inserted TESTX between BG1 and DX2. Twelve-stage check
passed. Exactly stages.toml, the new TESTX contract and the two generated navigation views changed;
AGENTS/WORKFLOW/CODEX/TOOLING/README/root ROADMAP/PRE_M5 and all domain docs were unchanged.
Fixture removed; TESTX is absent from the repository. Local real-tree docs-check passes with eleven
stages, sixty decision IDs and 327 local links. Online F1 context discovers unresolved issue #18.

Local `CARGO_HOME=/tmp/rustcraft-r2-cargo CARGO_BUILD_JOBS=2 just ci` passed docs tests/check,
formatting and workspace/all-target checking. Test artifact compilation hit the host disk quota in
ash/naga; tests and Clippy did not complete locally. The initial default-Cargo-cache attempt stopped
before compilation because the cache was read-only. No source/dependency workaround was introduced
and the expensive build was not repeated. Required Ubuntu/Windows CI remains the final wider gate.

Changing scope now edits one contract (plus domain docs only if architecture changes). Future closure
updates registry, report/evidence and issues, then sync/check; operational files need no status edits.


## Removed/replaced artifacts

Removed committed PROJECT_TREE.txt; project-tree prints current tracked paths. Replaced root roadmap
status text with pointers. Replaced AGENTS mandatory full-doc list with progressive loading, and future
ROADMAP scope with contracts/generated navigation. Historical reports/results and completed roadmap
sections are preserved. The PRE_M5 original chronology is untouched; only its post-audit owner link
is updated to the new contract/registry owners. Operational docs reference registry state.

## Publication

Implementation SHA: `c332c8b3c135488796f4af11b56a79480a2bebac`. Published to public
`main` by non-force push from `/tmp/rustcraft-r2-recovered`.

Public implementation CI: [run 37342382225](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37342382225).
Ubuntu (`rust (ubuntu-latest)`): success. Windows (`rust (windows-latest)`): success.
Both required platform jobs completed successfully; DOCINFRA1 publication is accepted.

No issue is closed by this batch; no subsequent implementation stage is started.

## Limitations

This tooling is navigation/integrity, not an architectural policy engine or Markdown parser. It checks
local file targets, not remote URL freshness or section fragments. Stage anchors require refreshing
after refactors. New-stage creates a planned skeleton, not a valid product design or permission to
execute. Manual scope/architecture/issue review remains required. No numeric performance budgets or
storage/backend changes are introduced.
