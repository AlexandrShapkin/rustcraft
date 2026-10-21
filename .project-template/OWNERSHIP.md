# Ownership and compatibility

| Files / rules | Canonical owner | Consumer treatment |
| --- | --- | --- |
| AGENTS.md, .github/DEVELOPMENT_INTERFACE.md, Task/PR templates, docs/WORKFLOW.md, this map | project-template | Hash-locked managed administration; exact-path review |
| README, docs/INDEX, ARCHITECTURE, TESTING, SECURITY, PROJECT, ADR scaffolds, editorconfig, gitignore | project-template initial scaffold | Consumer-owned after creation; manual reviewed improvements |
| AGENTS.local.md, native CI, product rules and code | Consumer | Never synchronized; stronger gates retained |
| CLI, bundles, locks, sync safety, validators and technical skills | Engineering Kit | Separately pinned Kit channel |
| Kit contract/AGENT_COMMON.md and contract/DEVELOPMENT_INTERFACE.md | Kit legacy v0.1 snapshot | Historical provenance only in new template; not active policy |

Audit: the legacy Kit contracts duplicate lifecycle, review and cleanup policy;
the interface also repeats the agent block and names historical products/native
gates. Preserve bytes for accepted v0.1 consumers and checker compatibility.
Do not extend those copies with new shared lifecycle policy. New consumers use
the template interface exclusively; technical skills remain opt-in references.
Existing consumers migrate in an explicit PR: inventory active references, retain
native gates in AGENTS.local.md, review replacement of old entrypoints, adopt the
administrative lock, then remove legacy policy activation. Never silently replace
an existing AGENTS.md: initial collision must fail. Existing snapshots continue
to work until individually migrated; no global switch or contract rewrite.

Administrative releases use full source commit SHAs in .project-template/lock.json;
Kit uses .engineering-kit/lock.json independently. Scaffold/local docs deliberately
do not receive automatic updates. Propose manual improvements without overwriting
project content. Source repository visibility and licensing are distinct from
transport classification. Formats 1/2 remain private; reviewed public delivery
uses format 3 and its external approved digest. Never relabel an existing lock.
Retain the canonical source copyright/permission notice in project-owned licensing,
outside these six managed paths; source publication does not expand sync ownership.
