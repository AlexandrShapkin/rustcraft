# rustcraft governance context

Native architecture, security, CI and product rules remain consumer-owned; see
[AGENTS.local.md](../AGENTS.local.md), [native procedures](RUSTCRAFT_WORKFLOW.md), the existing roadmap and documentation index.
Template alone owns the active shared lifecycle. Kit snapshots and optional skills
are inert technical references; no skill is installed or activated.

## Source adoption

Owning Issue: [#35](https://github.com/AlexandrShapkin/rustcraft/issues/35);
coordinator: [Kit #8](https://github.com/AlexandrShapkin/codex-engineering-kit/issues/8).
See [provenance and update/rollback](UPGRADING.md) and independent channel records
[Kit](../.engineering-kit/source.json) / [Template](../.project-template/source.json).
This is a reviewed source migration, **not an accepted Kit sync installation**.
There are no format-1/2 bundles or fabricated sync locks in this adoption.
Public source licenses do not approve a generated format-3 artifact digest.
MIT attribution is retained in [GOVERNANCE-NOTICE.txt](../GOVERNANCE-NOTICE.txt).

## Native stage map

Use [stage registry](stages.toml), [stage contracts](stages/INDEX.md) and [roadmap](ROADMAP.md). Lifecycle adoption is #35; product stages and existing milestones remain unchanged.
GitHub Issues own live status; this is navigation, not another backlog.
No new milestone is warranted: this task is not a confirmed product release.

## Product release applicability

Applicable: existing Linux tar.gz and Windows zip client/server assets and checksum sidecars. Preserve existing release/version workflows, proprietary-assets boundary and full native platform qualification.
The shared release requirements resolve through the canonical interface; product
version history remains independent of Kit and Template. No tag, Release,
release asset or publication automation is introduced by this migration.

## Dependencies and acceptance

Existing lifecycle Drafts overlap these entrypoints. They remain untouched and
must be reconciled explicitly during independent review; do not merge either
policy activation blindly. This branch starts from main, not an unreviewed stack.
Author checks, exact-head native CI, independent acceptance and merge authority
remain separate. No consumer is ACCEPTED by opening this Draft.

## Bounded release matrix qualification

| Product | Targets / format | Qualification |
| --- | --- | --- |
| Client and server | Linux tar.gz; Windows zip | Existing `scripts/release.py` build/check/publication-check and both native CI platforms; package SHA-256 sidecars. |

A significant release changes the user-facing client/server delivery. Keep the
existing pre-release history immutable; a final `vMAJOR.MINOR.PATCH` release is a
separate decision. Native checks, content/package compatibility and proprietary
asset exclusions in `docs/RELEASE.md` remain required. Notes must describe player
and operator effects, upgrade/rollback and known limitations; generated PR-title
notes alone do not satisfy the shared narrative requirement.

Contract v1 adoption remains inert. Its canonical schema and passive validator
are Codex Smart #166/#167; independently pinned adapter mappings and Git evidence
must be accepted before claiming authenticated readiness. GitHub Issues remain
the live queue. No runtime or optional Hub integration is enabled.
