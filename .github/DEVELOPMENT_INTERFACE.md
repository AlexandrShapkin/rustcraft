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
