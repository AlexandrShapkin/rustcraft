# Defect and debt tracking policy

[GitHub Issues](https://github.com/AlexandrShapkin/rustcraft/issues) is the **single authoritative live
tracker** for confirmed defects, concrete architecture debt, bounded performance/investigation work
and actionable implementation feature gaps. Issue open/closed state and labels belong to GitHub.
This file contains policy and a fixed migration index, not another manually maintained queue.
If repository prose and an Issue disagree about current state, **GitHub Issues wins**.
[Stage registry](stages.toml) owns sequencing/state; [contracts](stages/INDEX.md) own scope, not issue lifecycle.

## Classification and labels

Reuse existing labels: `bug` means defect; `enhancement` means optional feature gap. Do not add
synonymous type:defect/type:feature-gap labels. Other types are `type:architecture-debt`,
`type:performance`, `type:investigation`. Use one primary type, severity and `area:*` label;
add `stage:*` only when current documentation assigns an owner. Historical discovery IDs do not
assign current stages. No speculative area/stage labels are needed before consumers exist.
Preserve the `stage:*`, `area:*`, `severity:*` and `type:*` taxonomy and existing bug/enhancement
conventions; do not introduce duplicate status labels. Issue state belongs to GitHub. Keep titles
as clean descriptions of the concern; severity/type belong in labels, not title prefixes.

| Severity | Contract |
| --- | --- |
| P0 | Security, corruption or data-loss class; immediate blocker. |
| P1 | Current-stage blocker or core-contract violation; fix immediately. |
| P2 | Visible/functional issue; bounded deferral permitted. |
| P3 | Polish, cleanup, optional behavior or performance suspicion/backlog. |

P0/P1 cannot be hidden in backlog or waived by simply migrating tracking. Create/update the Issue,
report it prominently and establish explicit disposition before unrelated implementation proceeds.
If a tracking-only pass discovers a blocker, do not silently expand into implementation without
authorization. Architecture entry risks (e.g. before-M5 identity debt) are distinct from claims of
current save corruption or an already active stage violation; preserve documented severity meaning.

A ticket needs concrete current evidence or an accepted debt contract, plus bounded acceptance.
Missing future arbitrary meshes, spaces, physics, WASM or multiplayer alone are not defects.
Optional gameplay gaps remain optional. Performance/investigation tickets reproduce representative
release workloads, collect relevant distributions/costs, identify dominant causes and make an explicit
change/no-change decision; "make faster" is not acceptance.

## Discovery and lifecycle

1. Search open **and closed** Issues for the same root cause before creating one.
2. Update an existing canonical ticket with new evidence; otherwise create one with summary,
   reproduction/source evidence, type/severity/area/stage where known, expected invariant and acceptance.
3. Use the [issue template](../.github/ISSUE_TEMPLATE/defect.md); do not create a parallel Markdown entry.
4. Before implementation, inspect the ticket, revalidate it and reference it in notes/commits.
5. Follow [WORKFLOW](WORKFLOW.md#github-issue-reconciliation-and-stage-closure) for PR references,
   acceptance/CI-gated merge and completed closure. The PR/report provide normal resolution evidence;
   add a separate comment only when extra explanation/evidence is needed.
6. Use **not_planned** for intentionally declined, abandoned, superseded or unreproducible concerns with
   rationale. Use **duplicate** only with a canonical ticket link. Deferral alone leaves a ticket open.
   A bounded investigation can complete when its promised evidence/decision is delivered, even if no
   optimization is justified; distinguish that from abandoning the investigation.

Optional additional-evidence closure comment:

```text
Resolved by <SHA>.
Evidence:
- focused tests and required wider validation
- acceptance artifacts/results
- Ubuntu CI link/result
- Windows CI link/result
Resulting invariant: <what is now guaranteed>
```

Do not close issues merely because code was written or a tracking migration finished.
Before any stage closeout, query its open stage-labelled issues and account for each: resolved with
acceptance, moved with rationale or explicitly waived/deferred. An unresolved owned P0/P1 prevents
silent stage closure. See [WORKFLOW](WORKFLOW.md) and [CODEX](CODEX.md).
GitHub access is a tracking/publishing requirement, not an offline build/test dependency. If unavailable,
retain temporary local discovery notes until access returns, then search/update/create; never establish
another permanent Markdown live queue or claim tracker reconciliation was completed offline.

## ISSUE1 legacy migration index

Revalidated at `4452e43`: 18 active/deferred/narrowed rows, 17 actionable, no stale/fixed active rows,
one trigger-only limitation. Eighteen Issues cover the independently accepted concerns. This mapping
is identity/navigation only: **no current open/closed status is copied here**.

| Legacy ID(s) | Canonical GitHub Issue |
| --- | --- |
| M2-001, R1.2-001 | [#1](https://github.com/AlexandrShapkin/rustcraft/issues/1) — Evaluate translucent ordering across sections and atlas pages |
| M2-003 | [#2](https://github.com/AlexandrShapkin/rustcraft/issues/2) — Measure emissive-edit lighting latency in release workloads |
| M2-004 | [#3](https://github.com/AlexandrShapkin/rustcraft/issues/3) — Extend optional telemetry beyond Linux DRM providers |
| M2-005 | [#4](https://github.com/AlexandrShapkin/rustcraft/issues/4) — Report supported loose assets accurately in refs-status |
| M3-001 | [#5](https://github.com/AlexandrShapkin/rustcraft/issues/5) — Add optional shift-click inventory transfers |
| M3.2-002 | [#6](https://github.com/AlexandrShapkin/rustcraft/issues/6) — Add optional right-click crafting-slot transactions |
| M3.1-001 | [#7](https://github.com/AlexandrShapkin/rustcraft/issues/7) — Profile dropped-item merge and pickup scaling |
| ARCH-001 | [#8](https://github.com/AlexandrShapkin/rustcraft/issues/8) — Move replication-touched Minecraft rules out of generic runtime |
| ARCH-002 | [#9](https://github.com/AlexandrShapkin/rustcraft/issues/9) — Separate game-owned inventory layout from generic HUD rendering |
| ARCH-003 | [#10](https://github.com/AlexandrShapkin/rustcraft/issues/10) — Isolate legacy game conveniences from universal agent intent |
| ARCH-003 | [#11](https://github.com/AlexandrShapkin/rustcraft/issues/11) — Replace process-local block identity at external intent boundaries |
| ARCH-003 | [#12](https://github.com/AlexandrShapkin/rustcraft/issues/12) — Expose durable EntityId in bot item observations |
| ARCH-004 | [#13](https://github.com/AlexandrShapkin/rustcraft/issues/13) — Separate Minecraft policy from legacy content definition contracts |
| R1.0-001 | [#14](https://github.com/AlexandrShapkin/rustcraft/issues/14) — Retire active legacy numeric registry dependencies |
| R1.0-003 | [#15](https://github.com/AlexandrShapkin/rustcraft/issues/15) — Define scalable block-local canonical state schemas |
| R1.1-001 | [#16](https://github.com/AlexandrShapkin/rustcraft/issues/16) — Evaluate atlas-safe texture quality policies |
| R1.2-002 | [#17](https://github.com/AlexandrShapkin/rustcraft/issues/17) — Collect representative renderer profiles before GPU design changes |
| F1-001 | [#18](https://github.com/AlexandrShapkin/rustcraft/issues/18) — Fix F3 chord routing in the normal client path |

Deduplication/migration boundaries:

- M2-001 and R1.2-001 are one section/page-coarse translucent ordering concern.
- M3-001 owns shift-click. M3.2-002's repeated shift-click clause points to that concern; its independent
  crafting right-click gap has separate acceptance. Do not create another shift-click ticket.
- ARCH-003 splits into legacy game conveniences, external placement identity and durable Bot references;
  PM5-001 is supporting audit evidence for the latter two, not three interchangeable acceptance gates.
- ARCH-001 runtime rules, ARCH-004 definitions and R1.0-001 registry migration remain related separate
  A1 boundaries. R1.0-003 belongs to C2; BG1 is its consumer, not a reason to postpone C2 ownership.
- R1.2-002's renderer attribution is independent of F1 human routing/cadence. F1 field results are input
  to broader profiling, not proof that GPU design decisions have already been evaluated.
- ARCH-002 remains unassigned after R2 narrowing; no stage ownership was invented.
- R1.1-002 did not migrate: graceful configured-limit/runtime validation remains; reorder only after a
  demonstrated startup consumer/failure. Its historical trigger is retained, not an open task queue.

Closed/fixed legacy rows were not recreated as issues. Their original evidence/closure descriptions,
including M1.1, M3.8, M4, release/CI and R1 repairs, remain in
[DEFECTS_HISTORY](DEFECTS_HISTORY.md), Git history and unchanged acceptance reports.
