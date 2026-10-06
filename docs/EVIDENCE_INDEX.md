# Historical evidence index

Navigation only: reports/results describe their recorded baseline, not current stage state. Use
[registry](stages.toml)/[contracts](stages/INDEX.md) for planning and source for implementation facts.
Read only evidence relevant to the task. Update this index at accepted closeout; never invent missing
SHAs, CI runs or metrics. This is not a second evidence database.

## Foundations and early milestones

| Evidence | Location | Identity/CI navigation |
| --- | --- | --- |
| M0/M1 completed foundation/owner acceptance | [Historical roadmap](ROADMAP.md#m0--foundation--first-headless-vertical-slice) | Separate acceptance report is not recorded here; inspect history if needed. |
| M2 telemetry/sandbox measurements | [M2 validation](M2_VALIDATION.md) | Baseline-specific evidence in the document. |
| M3 survival and inventory | [M3 validation](M3_VALIDATION.md) | Historical optional gaps now map through [DEFECTS](DEFECTS.md). |
| M3.8 presentation diagnostics | [M3.8 validation](M3_8_VALIDATION.md) | Presentation evidence, not generalized BG1 proof. |
| Engine/game alignment | [Alignment validation](ARCHITECTURE_ALIGNMENT_VALIDATION.md) | Non-Minecraft boundary proof. |
| R1 resource/renderer scalability | [Historical roadmap](ROADMAP.md#r1--resourcerender-scalability), [PERFORMANCE](PERFORMANCE.md) | Relevant R1 baseline sections only. |
| M4 generation/streaming/persistence | [Historical roadmap](ROADMAP.md#m4--world-generation--persistence-foundation), [PERFORMANCE](PERFORMANCE.md), [reference notes](../reference/notes) | Frozen generation/save and measured route evidence remain in their existing sections. |

## Accepted hardening and platform batches

| Batch | Report / results | Recorded implementation / CI |
| --- | --- | --- |
| DX1 | [Report](DX1_REPORT.md) | `008fbac252e4307af5146cda8e033eee3079bcc3`; [CI 37156846278](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37156846278) |
| DUX1 | [Report](DUX1_REPORT.md) | `f6ede4ceeca2be153cd29a51c525454759de6a3f`; [CI 37174284855](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37174284855) |
| C1 | [Report](C1_REPORT.md) | `a3554807a20dc4fff6d9878cb018659c23b1a7b2`; [implementation CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37178493694). Recorded closeout `585de94b666257ddb5e4f2a902429db84be010ca`, [CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37182596219). |
| UX1 | [Report](UX1_REPORT.md) | `83c3cdcd954d72f4c8a7ba98f8a3918b4efa652e`; [CI 37220087083](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37220087083) |
| RSM1 | [Report](RSM1_REPORT.md), [results](RSM1_RESULTS.json) | `e3117949d2d90473ae4be3263b24fc04867bbc19`; [CI 37228702009](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37228702009) |
| P1 | [Report](P1_REPORT.md), [results](P1_RESULTS.json) | `524a2a6dc7edab72453f31ad7eda02b8dcd9d58a`; [CI 37256205733](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37256205733) |
| S1 | [Report](S1_REPORT.md), [results](S1_RESULTS.json), [backend decision](S1_PERSISTENCE_DECISION.md) | `711637aa297b2a3bf195ccf5af46d64e4981fb77`; [CI 37271791735](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37271791735) |
| Durable entity transfers | [Repair evidence](ENTITY_TRANSFER_REPAIR.md), [R2 recovery context](R2_REPORT.md) | Repair `a3a1b4dcc136bafe558cc49729e2662352e76260`, recorded in R2 report; no separate CI link invented. |
| R2 semantic presentation | [Report](R2_REPORT.md) | `f3909b7b94aa8c86eae2c98588384976ea296225`; [CI 37325633913](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37325633913). Closeout identity remains discoverable through Git history. |
| DOCINFRA1 maintenance | [Report](DOCINFRA1_REPORT.md) | Baseline and final publication evidence in report; not a product stage. |

## Audits and historical concerns

[PRE_M5_AUDIT](PRE_M5_AUDIT.md) preserves original chronology plus owner-approved expansion;
[ARCHITECTURE_AUDIT](ARCHITECTURE_AUDIT.md) maps migration pressure. [DEFECTS_HISTORY](DEFECTS_HISTORY.md)
preserves completed pre-Issue evidence. These do not override newer stage planning or GitHub live state.

## Accepted stage evidence

| Stage | Accepted artifact | Evidence |
| --- | --- | --- |
| F1 | [Accepted report](F1_REPORT.md) | Matched 1280x720 six-phase AMD/RADV/Vulkan dev/release, owner manual release smoothness, production input/capability tests, checkpoint correlation, durability/recovery and both-platform CI. |
| A1 | [Accepted report](A1_REPORT.md) | Semantic placement/profile mapping, durable Bot EntityId, typed game adapters, native policy registration, local provenance and independent sandbox; [identity CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37375463910), [policy/trust CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37377613595), both platforms green. |
| C2 | [Accepted report](C2_REPORT.md) | Composed voxel/item content, typed properties and indexed tags/contracts, command-based Use, canonical compact state and semantic codec/profile reorder proof; [implementation CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37397475236), both platforms green. #15 remains open with BG1 residual consumer acceptance. |
