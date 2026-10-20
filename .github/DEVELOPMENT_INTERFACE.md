# Shared repository development interface v1

This file has the same path and meaning in **Codex Smart, RelayForge,
Assistant Hub and RustCraft**. It standardizes the *workflow interface*,
not product internals, security capabilities or mandatory check strength.
The owning cross-project task is
[Codex Smart #169](https://github.com/AlexandrShapkin/codex-smart/issues/169).

## Expected layout (shared entrypoints)

| Path / object | Meaning |
| --- | --- |
| `README.md` | Product purpose, build/usage links, platform distinctions |
| `AGENTS.md` | Agent bootstrap and repo-local invariants; never a capability grant |
| `.github/DEVELOPMENT_INTERFACE.md` | This consistent cross-repository workflow |
| `.github/ISSUE_TEMPLATE/task.yml` | Canonical form for ordinary new development tasks |
| `.github/labels.json` | Same non-executable desired GitHub label names/colors; not a server migration |
| `.github/pull_request_template.md` | Shared PR evidence and security checklist |
| `.github/workflows/` | Actual native CI; never replaced by a generic smoke check |
| `automation/project.toml` | Optional **disabled-only** Contract v1 metadata, not an executor |
| `docs/` | Project's authoritative workflow, architecture, decisions and roadmap |
| `scripts/` or project-native tools | Implementation-specific read-only checks / tests |

Some entrypoints are Draft PRs until merged. **A file being proposed here
does not establish its presence on accepted `main`.** Preserve the native
documentation index and filenames instead of renaming project-specific
architecture, stage registers or generated documents.

## One GitHub task model

1. Search **open and closed** Issues in the owning repository. Reuse an Issue
   when it owns the work; otherwise use the common **Task** form. Related
   cross-repository work links to one owning Issue *per affected repository*
   and the cross-repository coordinating Issue; do not mirror status in files.
2. Each actionable Issue records **goal, evidence, scope/non-goals, acceptance,
   verification/platforms, dependencies, risks, priority, work kind and
   primary area**. A roadmap or stage registry gives navigation only.
3. Priority values are `P0` critical/safety, `P1` blocking authorized work,
   `P2` planned important, `P3` later improvement. Recommended eventual
   GitHub labels are `priority:P0` … `priority:P3`. Older Codex Smart
   `severity:P*` labels encode the equivalent priority; retain them until
   an audited migration, never apply duplicate contradictory priorities.
4. Use one existing primary `area:*` label where supported; additional
   project-specific areas remain valid. Work kind uses existing `bug`,
   `enhancement` and local type labels. Avoid invented `in-progress`,
   `approved` or `done` labels: Issue state, linked PR and exact-head
   checks are authoritative. The form's dropdown is **not automatically a
   GitHub label**; no label synchronization is installed here.
5. Use small branches/PRs from a *verified* `main` SHA. Reference the
   owning Issue. Preserve unrelated work and all device/user boundaries.
   A task can have independent parallel PRs but must not silently widen scope.
6. Evidence is separate: **local tests → GitHub exact-head CI/Security →
   technical review → formal merge eligibility/authority → install/live
   device proof**. None implies a later step. Reverify whenever SHA changes.
   Close only when the Issue's documented acceptance criteria are actually
   met; keep investigations and unresolved work open.

## One PR review interface

The standard PR template requires an owning Issue, base/full head SHA, scoped
diff, tests and negative cases, named native/platform checks, security/data
impact, independent review, merge/rollback limits and follow-up blockers.
The PR form does not grant permissions. Any explicitly prohibited PR stays
prohibited regardless of template or green CI.

## Native commands (capability-preserving)

| Repository | Context and mandatory native verification |
| --- | --- |
| Codex Smart | `just status`, `just context`, `just docs-check`, `just check` and applicable cached `just security`; exact-head CI and Security |
| RelayForge | Read `AGENTS.md` when accepted, `docs/development-without-rdc.md`; Rust format/strict Clippy/tests and RDC-independent CI; **device recovery evidence separate** |
| Assistant Hub | `AGENTS.md`, `docs/workflow.md`; Python 3.11–3.13 unit tests/compile/doctor/public-tree plus offline UI tests |
| RustCraft | `just status`, `just codex-context [STAGE]`, `just ci` (not only `just check`), Ubuntu and Windows CI; hardware/GPU/reference proof separately when required |

The **semantics** of status/context/doctor/check/docs-check/smoke should
ultimately be offered through a trusted, offline, read-only or appropriately
bounded adapter in every repository. The common `status`, `context`, `doctor`, `docs-check`, `verify` surface is
being added through separate per-repository Draft PRs. `verify` must run the
full native project checks, not a no-op or weaker smoke recipe. GitHub's
exact-head/platform/security checks and independent authority remain separate.
Do **not** infer command availability on accepted `main` from an unmerged PR,
or automatically install `just`/toolchains to satisfy an absent dependency.

## Required workspace lifecycle and disk-space hygiene

Disk space is a shared development resource. **The end of every completed,
abandoned or superseded task includes an explicit cleanup audit**, and any
reported low-disk condition triggers a read-only inventory **before** starting
another heavyweight build. Do not accumulate obsolete worktrees, short-lived
branches, repeated build products, caches or generated artifacts indefinitely.

1. **Inventory first:** identify the exact checkout/root, disk usage, current
   branch and HEAD; inspect `git worktree list --porcelain`, local/remote
   branches, `git status --porcelain=v1 --untracked-files=all`, open PRs,
   active tasks and the sizes/owners of candidate build/cache directories.
   Enumerating is not permission to delete.
2. **Generated artifacts and caches:** promptly remove no-longer-needed
   *project-owned, reproducible* build/test outputs and caches (for example
   `target/`, `__pycache__/`, `.pytest_cache/`, `node_modules/`,
   `dist/` where actually generated). First confirm their source-based
   regeneration, exact bounded paths, that no process is using them, and that
   they contain no unique reports, downloaded-only inputs or local user data.
   Do not delete an entire directory by name alone.
3. **Worktrees:** remove a non-current worktree promptly after its task
   has been merged/closed or deliberately abandoned **only if** it has no
   staged, unstaged, untracked or otherwise unsaved content; no active
   process/task/PR needs it; its required commits are recoverable from
   verified refs; and the exact worktree path is identified. Preserve the
   active checkout and registered worktrees while a task is open. Do not
   force-remove or prune an uncertain worktree.
4. **Branches:** delete obsolete **local** task branches only after
   confirming their merged/recoverable state or explicit abandonment and
   checking that they have no live Issue/PR, worktree or unsaved commits.
   Never delete `main`, protected/release branches or any open/draft PR
   branch, including a vetoed or retained PR. **Remote** branch deletion
   is a separate, explicitly authorized source-control operation; it is
   not implied by local disk cleanup. Do not perform indiscriminate
   `git branch -D`, `git clean -fdx`, `git worktree prune`,
   `git gc --prune=now` or recursive deletion against guessed paths.
5. **Preserve evidence:** do not silently delete private checkpoints,
   unmerged code, local databases, benchmark baselines, reference assets,
   unique logs/reports, CI/debug evidence, security artifacts or personal
   files. Archive independently valuable material under the owner's
   storage policy first; deletion requires verified redundancy and scope.
   Age, untracked status and absence from the last commit are **not**
   proof that data is disposable.

For every cleanup, record privately the exact paths/ref names, classification,
preconditions, actions, freed disk space if actually measured, and any
protected/uncertain leftovers. On ambiguity, **do not delete**: report a
bounded candidate inventory and the blocking evidence. A policy in Git cannot
authorize execution on the owner's computer, destructive remote operations,
or use of secrets. Do not add a background auto-cleaner or privileged CI
cleanup on the strength of this text alone.

## GitHub administration / migration

This PR standardizes tracked templates and written policy. GitHub server
labels, Projects fields, milestones, branch rules and historical Issue labels
(**including the declarative `.github/labels.json` proposal**)
are **not** modified by adding these files. Harmonize them in a separate
authenticated administrative migration after an inventory, with no removal
of historical labels before mappings and rollback are verified. No
repository-controlled issue, label, workflow or TOML content authorizes a
Codex run, shell invocation, merge, deployment or access to private devices.

## Normative shared agent lifecycle

AGENTS.md reproduces the following shared block byte-for-byte; product rules
follow its section 10 and may be stricter. This extends this interface, not a
second workflow standard. Check offline with
`python3 -B scripts/check_agent_contract.py --project REPOSITORY`; optionally
pass repeated `--peer /path/to/other/repository` for cross-repository comparison.
The checker pins the shared revision and verifies required product gate text;
it does not execute those gates or confer authority.

```markdown
<!-- shared-agent:start -->
# Agent engineering contract

## 1. Purpose and authority

Follow [.github/DEVELOPMENT_INTERFACE.md](.github/DEVELOPMENT_INTERFACE.md), the
single shared workflow interface. These instructions do not grant runtime,
merge, deployment, device, credential or destructive-operation authority.
Preserve stricter product rules in section 10; stop dependent work on a conflict
and continue independent authorized work. Never use RDC for development.

## 2. Repository start (START)

Discover the actual root, full HEAD, branch and Git status. Compare the intended
base SHA with current GitHub evidence; record divergence rather than assuming
freshness. Inventory worktrees, unfinished changes and active PRs, including
intersecting files and dependencies, before creating commits. Do not silently
switch checkout, overwrite unknown work or rewrite existing branch history.

## 3. GitHub task (TASK)

Search open and closed Issues; reuse the owning task and avoid duplicates.
Record goal, scope/non-goals, acceptance, dependencies and risks. Link Issues
and PRs for cross-repository work to the coordinating Issue. Roadmaps and stage
registries provide navigation, not execution authority or a second live queue.

## 4. Necessary context (CONTEXT)

Read AGENTS.md, the shared interface and the product context entrypoint first.
Read only task-relevant contracts, architecture and source; never load the
whole repository by default. Prefer native git/rg and targeted reads for known
locations; use CodeGraph for useful structural context, semantic tools for
symbol questions and Context7 for version-sensitive external APIs when available.
Do not repeat equivalent retrieval or install tools automatically. An absent
tool is a reported limitation, not permission to download or execute it.

## 5. Planning and implementation (IMPLEMENT)

Work on one bounded task/branch from a verified base or explicitly documented
PR dependency. Preserve product architecture and security invariants. Make
minimal coherent changes; add regression and negative tests for behavior.
Create extra clones/worktrees only for a concrete justified need and within
product authorization. Preserve unrelated files, user data and unique evidence.

## 6. Verification and quality (VERIFY)

Run targeted checks first, then the full mandatory product gates in section 10.
A smoke test cannot replace native validation. Report PASS, FAIL, BLOCKED and
UNVERIFIED distinctly; never disable a failing check to obtain green results.
Keep secrets/private data out of source, diagnostics and public reports. CI is
source evidence, not proof of deployment correctness or runtime authority.

## 7. PR, CI, review and acceptance (PR / REVIEW)

Inspect the complete diff, including new files, before committing. Create or
update a scoped PR linked to its Issue; record base and full HEAD SHA, actual
commands/results, exact-head CI, risks and limits. Repeat affected verification
and refresh CI/review evidence after HEAD changes. Keep author verification,
independent technical ACCEPT, formal GitHub APPROVE and merge authority separate.
Never present your own checks as independent review. Keep PRs Draft pending
required independent acceptance. Do not merge, deploy or change GitHub settings
without explicit task authorization and all applicable product gates.

## 8. Temporary resources (CLEANUP)

At completion, abandonment or supersession, inventory obsolete worktrees,
branches and reproducible artifacts; inventory disk usage before expensive work
when space is low. Delete only exact, verified recoverable and unused items
within authority, after checking tasks/PRs, dirty/untracked files and processes.
Never delete active/protected branches, open or Draft PR work, unknown data,
unique results or user files. No blanket git clean, forced worktree removal or
age-based deletion. Preserve uncertain candidates; record what was removed,
what was retained, why, and measured freed space only when actually measured.

## 9. Final evidence (REPORT)

Report: what changed; repository, PR and full SHA; actual tests and CI; remaining
UNVERIFIED evidence; blockers; owner actions; and temporary resources removed
or preserved. Separate source preparation from installation and live-host proof.

## 10. Product-specific rules
<!-- shared-agent:end -->
```
