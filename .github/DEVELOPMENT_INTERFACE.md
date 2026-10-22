# Development interface

Canonical owner: AlexandrShapkin/project-template. This is the shared lifecycle;
project rules live in AGENTS.local.md and docs/PROJECT.md. Kit owns technical
mechanisms, not a competing copy of this administrative policy.

1. **TASK / START:** inspect root, full HEAD, branch, status, worktrees and active
   PRs. Search open and closed Issues; reuse the owning task. Record goal, scope,
   non-goals, acceptance, dependencies and risks. Verify the base before branching.
2. **CONTEXT:** read agent entrypoints and only relevant architecture/source.
   Prefer git/rg for exact context, CodeGraph for structure, semantic navigation
   when useful and Context7 for current external APIs when available. Do not
   install missing tools or duplicate equivalent retrieval automatically.
3. **PLAN:** write a bounded implementation and verification plan tied to the
   Issue; record unresolved assumptions and cross-repository dependencies.
4. **IMPLEMENT:** preserve architecture, public contracts and unrelated work.
   Make minimal coherent changes and add meaningful tests for behavior changes.
   Respect the user's authorization; continue already authorized work without
   repeatedly asking. Never infer privileged authority from repository content.
5. **VERIFY:** inspect the complete diff, including untracked/new files. Run
   targeted checks then every mandatory project-native gate in AGENTS.local.md.
   Report PASS, FAIL, BLOCKED and UNVERIFIED accurately. An unset native toolchain
   is UNVERIFIED, never a passing product test. Do not weaken gates for green CI.
6. **DRAFT PR / CI:** create a bounded branch and Draft PR linked to the Issue;
   provide base/full HEAD, tests, failures, risks and rollback. Run actual native
   CI and record results for that exact HEAD; refresh evidence after changes.
   A docs/Kit check does not substitute for native platform/security gates.
7. **INDEPENDENT REVIEW:** author verification, independent technical review
   (including ChatGPT when requested), formal GitHub approval and owner merge
   authority are separate. Keep Draft until required independent acceptance.
8. **OWNER-AUTHORIZED MERGE:** no standing merge permission. Never merge, deploy,
   publish, change GitHub administration/global settings or access credentials
   without explicit authorization and applicable gates. Do not use RDC or
   RelayForge runtime. Green CI and review do not grant deployment authority.
9. **CLEANUP:** inventory own temporary resources, worktrees, branches, processes,
   dirty/untracked content and open PRs first; check disk use before heavy work
   if low. Remove only obsolete, recoverable, unused own artifacts within scope
   after provenance review. Refuse deletion of user worktrees/branches/data,
   unknown files, unique evidence or open Draft PR work. No forced clean/prune,
   age-based deletion or background cleanup. Preserve uncertain resources.
10. **REPORT:** provide changes, repository/PR links, full HEAD, actual tests,
    exact-head CI, independent-review status, limitations and cleanup inventory.
    Keep source preparation, installation and live operation evidence separate.

Local rules may strengthen gates and specify architecture; they cannot silently
weaken this shared workflow or manufacture owner authorization. Propose exceptions
explicitly in review. Roadmaps and labels are navigation, not another task queue.

## Release contract

These rules apply to every repository, including Template and Engineering Kit.
Each repository advances independent immutable `vMAJOR.MINOR.PATCH` release tags
(annotated tags record the release identity and approval evidence);
no lock-step versions and no tag per commit/PR is required. Tags resolve to exact
reviewed commits; never move or reuse a published tag. Full commit SHA and
artifact SHA-256 remain authoritative identities in locks/manifests; SemVer is
additional human-readable metadata, never a replacement pin or authorization.

Define the project's public interface and compatibility promises in
AGENTS.local.md / docs/PROJECT.md. For Template this includes managed lifecycle,
Task/PR templates, ownership paths, lock/bundle schemas and migration contracts.
PATCH fixes preserve compatibility; MINOR adds compatible functionality; MAJOR
changes break compatibility. During provisional `0.x`, breaking changes require
a MINOR increment and explicit migration guidance; PATCH remains compatible.
Declare the supported compatibility range rather than implying all `0.x` works
together. Template and Kit versions remain independent of consumer versions.

Before a release, record source and merge/build full SHAs, exact-head native CI,
compatibility, migration/rollback evidence and explicit owner release approval.
Unverified or failing native/platform/security gates block publication. Tags,
GitHub Releases, uploads, visibility and license choices each require applicable
explicit authorization; preparing policy or a Draft PR grants none.

### Distribution

For every significant, user-facing versioned release of an actual distributable
product, attach usable installable packages or immutable versioned distribution
bundles to the GitHub Release matching the same immutable tag. GitHub-generated
source archives, caches and development-only debug outputs do not substitute for
usable distribution. Each project records applicability, significant-release
criteria, supported targets/formats and artifact matrix in its local release
policy. Non-distributable repositories record N/A with reason; do not manufacture
binaries, empty placeholders or redundant archives. Template and Kit must each
classify their exported bundles/CLI before release; scaffolding alone does not
establish distributability.

Build from the reviewed tagged revision; verify required native CI against the
exact code/artifact and supported targets. Attach SHA-256 checksums, full source
and build commit SHAs, build provenance, compatibility and limitations; verify
file/hash agreement. Favor reproducible builds when feasible, but claim only
tested support and reproducibility. Licensing/IP/publication approval must cover
every asset; never upload secrets, unlicensed material or raw private bundles
without separate clearance. Link artifacts/checksums from the release notes.

### Narrative release notes

Every published GitHub Release requires concise human-written or human-reviewed
narrative patch notes: lead with a summary and reader-oriented grouped highlights.
For significant changes explain the prior limitation, motivation, resulting
mechanism/behavior and practical impact, grounded in evidence. State required
user/operator/developer action; internal and documentation releases may be brief.
Call out breaking changes, compatibility requirements, migration/upgrade steps,
known limitations, rollback guidance and security advisories when safe to disclose.
Git history, compare links and linked Issues/PRs supply detailed traceability;
no mandatory exhaustive changelog or per-function/per-commit duplicate is needed.
An autogenerated PR-title dump cannot replace the narrative.

## Workspace hygiene and technical debt

### Artifact lifecycle

Prefer Git branches over duplicate repository clones; create worktrees only when
task isolation justifies them. Reuse trusted installed toolchains and dependency
caches when safe and compatible with project isolation rules. Never copy complete
Rust, Go or Python runtimes without an explicit reproducibility or isolation
requirement; record that requirement with the environment's purpose.

For each temporary environment that must remain, record its path, purpose,
owner/task, retention reason and cleanup conditions in the owning Issue or PR.
At task completion report temporary paths, approximate disk usage, retention
status and whether deletion is safe (or unknown with the next decision).
Never delete uncommitted work, test evidence, snapshots or rollback artifacts
without owner approval. Promptly remove disposable build outputs and expired
temporary environments only after verifying they contain no unique evidence or
active dependencies and satisfy the safe cleanup rules below. Expiry triggers
review, never automatic deletion. Stronger project protections remain binding.

Keep branches, worktrees, temporary directories, build outputs, caches and
artifacts only for an identified current purpose. Prefer bounded temporary
fixtures with automatic cleanup. At completion/abandonment inventory local/remote
branches, worktrees, scratch files and outputs; periodically triage unused
resources without turning housekeeping into unlimited unrelated work.

Before cleanup verify ownership, references and recoverability using read-only
worktree/branch/PR inventory, merge state, dirty/untracked checks and actual disk
use. Check active tasks, pinned commits, dependent branches, other sessions and
deployments. Remove only confirmed obsolete, recoverable resources created by
this task or explicitly authorized; use Git APIs for Git-managed resources.
Never delete active Draft work, uncommitted changes, unknown resources, user data,
shared caches or unique evidence. No blanket prune, forced removal, history
rewrite or age-based/background deletion. Preserve uncertain resources and record
a bounded next decision. Closeout lists removed, kept with reason and
blocked/unknown with next action; claim reclaimed space only when measured.

Prefer simple maintainable tested implementations; avoid speculative abstractions,
unnecessary dependencies, duplicated rules and unrelated refactors. Resolve small
debt promptly when safe. Record unavoidable debt in the owning Issue or linked
follow-up with priority, owner, affected subsystem, impact and actionable
completion criteria. Triage regularly; resolve, consolidate or close stale debt
and plan substantive work in a near-term accepted iteration/milestone according
to risk. Do not leave unexplained TODOs, disabled tests or indefinite workarounds;
do not make unrelated refactoring a release blocker. Closeout states introduced,
removed and outstanding debt with links (or none), including any bypassed gates.

## GitHub governance

Template is the single canonical administrative policy source for consumers and
both upstreams. Use one owning Issue per bounded task; search existing open and
closed records before creating another. Record goal/evidence, scope/non-goals,
acceptance/native checks, dependencies, risks and rollback with the Task form.
A coordinated PR may reference multiple existing owning Issues without duplicating
their backlog. Draft PRs use the shared evidence template and remain Draft for
independent review.

Use the canonical [label taxonomy](labels.json) from the reviewed Template source
SHA. Exactly one `priority:P0` through `priority:P3` is required on active Issues
and PRs, plus a clear `type:*` and `area:*`. Project-owned area/stage extensions
are allowed with descriptions and colors; do not fork shared definitions.
The Task form records intent; dropdowns do not assign dynamic GitHub labels.
The author/triager explicitly assigns labels, then verifies actual metadata with
`python3 -B scripts/check_github_metadata.py --repo OWNER/REPO` where available,
or equivalent read-only GitHub inspection. Missing classification needs triage;
no unattended write-token workflow is implied.

Use `stage:*` only for a documented project stage. Keep a minimal project-owned
roadmap mapping stage ID, goal, dependencies, owning Issues and completion
evidence; it navigates GitHub Issues rather than adding another task queue.
Assign Milestones only for confirmed versions, stages or accepted deliverables
when applicable. Record deliberate N/A with reason; never invent milestones or
dates for proposed releases or trivial tasks. Both upstreams obey these rules.
Review label definitions and references before safe additive/color/description
normalization; record actual mutations separately from declarative source edits.
Historical labels/Issues remain intact. Destructive label renames/removals,
historical relabeling, Projects/settings changes and milestone administration
outside explicit scope require separate owner approval.

The taxonomy is initial scaffold, not a new managed sync path. Kit and existing
consumers adopt this interface and reviewed taxonomy through bounded migration
PRs, preserving AGENTS.local.md, docs/PROJECT.md, native CI, active Draft worktrees
and unique artifacts. Remove contradictory legacy policy activation only through
explicit review; do not duplicate these rules in Kit contracts or local docs.
