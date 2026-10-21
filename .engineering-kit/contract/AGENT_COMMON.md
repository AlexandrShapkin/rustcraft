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
