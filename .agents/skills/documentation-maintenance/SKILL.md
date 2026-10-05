---
name: documentation-maintenance
description: Maintain canonical planning, context navigation, decisions and documentation integrity without duplicated mutable state.
---

# Documentation maintenance

Read [INDEX](../../../docs/INDEX.md) for ownership/precedence. Git/source proves implementation;
reports/audits prove only their recorded baseline. Registry owns mutable order/state/focus, contracts
own scope, domain docs own architecture, decisions own rationale and GitHub owns live concerns.

- Run `just docs-check` first; use `just codex-context ID --offline` for compact navigation.
- For planning additions, use `just stage-new ID "Title" AFTER`, then author the one contract and
  navigation metadata. Do not copy stage sequence/status into operating docs or create GitHub labels
  automatically. Manual TOML editing remains supported; registry state has one active/focus invariant.
- Change stage scope in its contract; change domain docs/decisions only if architectural intent changes.
  Use templates; no manually maintained Status in contracts. New decisions state explicit whole/partial
  supersession and references; preserve old rationale, never infer precedence from text order alone.
- Run `just docs-sync` only for generated stage index and ROADMAP marker block. Never hand-edit generated
  navigation or generate architecture/contracts/reports. Review unchanged manual prose and idempotence.
- Run documentation unit tests, docs-check and diff/link review. No GitHub/network/private assets are
  required. Validate source path hints after structural changes; keep context output small.
- Update EVIDENCE_INDEX when accepting a report; do not invent SHAs, CI or metrics and do not turn it
  into a second evidence database. Preserve historical chronology and label current versus target.

For issue/implementation lifecycle link [WORKFLOW](../../../docs/WORKFLOW.md) rather than duplicating it.
