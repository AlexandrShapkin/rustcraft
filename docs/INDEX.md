# Documentation index

Start with [AGENTS](../AGENTS.md) and `just codex-context [STAGE]`; read the contract and only the
pointers needed for the authorized task. No report or giant domain file is an unconditional bootstrap.

## Ownership and precedence

When sources disagree, identify the kind of fact rather than taking the newest paragraph blindly:

| Fact | Canonical owner | Read/update rule |
| --- | --- | --- |
| Implemented behavior/baseline | Actual Git/source state | Verify code/HEAD; plans and reports cannot prove current implementation. |
| Mutable order/state/focus | [stages.toml](stages.toml) | Edit registry once; run sync/check. Generated views are derivatives. |
| Detailed implementation scope/acceptance | [Stage contracts](stages/INDEX.md) | Read selected contract; update it when authorized scope changes. No local Status field. |
| Live actionable concerns | [GitHub Issues](https://github.com/AlexandrShapkin/rustcraft/issues) | Search/update current issues; [DEFECTS](DEFECTS.md) owns policy and static legacy mapping only. |
| Architecture/target invariants | [ARCHITECTURE](ARCHITECTURE.md) and domain docs below | Read affected concept; update shared contract here, not copied planning prose. |
| Durable rationale/supersession | [DECISIONS](DECISIONS.md) | Open relevant IDs from context; append explicit decisions, preserve old rationale. |
| Measurements/accepted implementation evidence | [EVIDENCE_INDEX](EVIDENCE_INDEX.md) and linked reports | Read only relevant baseline; append truthful evidence without inventing missing results. |
| Source ownership/navigation | [ARCHITECTURE_AUDIT](ARCHITECTURE_AUDIT.md) now; CODE_MAP after RF1 | Verify source anchors; refresh after structural migration. |
| Generic agent rules | [AGENTS](../AGENTS.md) | Compact always-on rules; [CODEX](CODEX.md) explains recovery/context usage. |

Owner instructions authorize changes; registry focus alone does not start a stage. New canonical
planning can supersede older audit recommendations; historical evidence does not silently override it.

## Canonical product and architecture concepts

- [PRODUCT](PRODUCT.md): product identity/non-goals; read for scope changes, update on owner decisions.
- [ARCHITECTURE](ARCHITECTURE.md): mechanism/policy, current versus target boundaries; update durable changes.
- [GAMEPLAY_CONTRACT](GAMEPLAY_CONTRACT.md): game versus engine responsibilities; read for gameplay/API work.
- [CONTENT_SYSTEM](CONTENT_SYSTEM.md): semantic packages, compilation, C2/BG1 concepts; update shared semantics.
- [VOXEL_SPACES](VOXEL_SPACES.md): complete spatial concept; VS1 contract scopes implementation, not a copy.
- [NETWORKING](NETWORKING.md): M5 architectural consequences, not an implemented wire protocol.
- [MODDING](MODDING.md): native/sandbox extension boundary and planned WASM consequences.
- [AGENTS_AND_BOTS](AGENTS_AND_BOTS.md): semantic automation identity/controller responsibilities.
- [CONFIGURATION](CONFIGURATION.md), [DEBUGGING](DEBUGGING.md), [SCRIPTING](SCRIPTING.md): current operational
  contracts and explicitly labelled targets; read only the affected surface.
- [PERFORMANCE](PERFORMANCE.md): measurement methodology and baseline results; use relevant sections only.

## Planning and decisions

[ROADMAP](ROADMAP.md) is a human overview with completed history and generated navigation.
[Stage index](stages/INDEX.md) is generated, not another owner. Edit one registry entry and one contract
for new work (`just stage-new`), then sync/check. Closed extracted stages remain in sequence/history;
older completed milestones retain their existing roadmap/report evidence rather than being re-modelled.
Maintenance batches are reports, not product stages.

Templates: [stage](templates/STAGE.md), [report](templates/REPORT.md), [decision](templates/DECISION.md).
Decision bodies remain in [DECISIONS](DECISIONS.md); context prints only selected IDs/titles.

## Operations and developer workflow

[WORKFLOW](WORKFLOW.md) owns implementation/issue acceptance/closeout procedure.
[TOOLING](TOOLING.md) owns commands, Python baseline and portable invocation.
[CODEX](CODEX.md) owns progressive loading/interruption recovery; repository
[stage-execution](../.agents/skills/stage-execution/SKILL.md) and
[documentation-maintenance](../.agents/skills/documentation-maintenance/SKILL.md) skills encode reusable procedure.
Update these when operating rules change, never to repeat a current stage status.

## Historical evidence and audits

[EVIDENCE_INDEX](EVIDENCE_INDEX.md) locates reports/results without loading them all.
[PRE_M5_AUDIT](PRE_M5_AUDIT.md) preserves historical chronology and a baseline-specific owner expansion;
[ARCHITECTURE_AUDIT](ARCHITECTURE_AUDIT.md) maps current migration pressure. Read relevant findings,
verify them against source/issues, and keep measured claims attached to their baseline.
[DEFECTS_HISTORY](DEFECTS_HISTORY.md) preserves pre-Issue completed entries and trigger-only evidence;
GitHub alone determines live concern state.

## Security, release and references

[SECURITY](SECURITY.md) owns trust/capability limits; [RELEASE](RELEASE.md) and
[RELEASE-RUNNING](RELEASE-RUNNING.md) own packaging/run procedures;
[THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES.md) owns bundled notices.
[REFERENCE_POLICY](REFERENCE_POLICY.md) and [reference sources](../reference/SOURCES.md) govern evidence
and proprietary materials. Read for behavior/asset research; never make offline correctness depend
on private assets. Historical captures remain evidence, not redistributed project resources.
