# Codex operating notes

Codex should treat `AGENTS.md` as the compact always-on contract and use the detailed documents and
repo-local skills for task-specific depth.

## Normal loop

1. Read `AGENTS.md` and the documents relevant to the batch.
2. Run `just doctor` once per environment and `just bootstrap-check` before substantial changes when
   the workspace is expected to build.
3. Implement a coherent batch rather than one micro-change at a time.
4. Validate with `just` recipes.
5. Search/update/create GitHub Issues for actionable findings; do not maintain a Markdown live queue.
   Report P0/P1 immediately and fix within authorized scope, or obtain explicit disposition.
6. Update durable docs for implemented contracts or explicit owner-approved plans, clearly distinguishing them.

For behavior/protocol/asset research, run `just refs-status` and use `reference/SOURCES.md` to choose
sources. Do not scan every external repository by default. `just refs-lock` can snapshot the exact
commits consulted for a durable research result.

When available, Serena is preferred for symbol-aware repository navigation/refactors and Context7
for current third-party API documentation. Ordinary shell/git/rg/jq tools remain first-class and
are often faster for simple tasks.

The user wants low-interaction autonomous progress. Do not stop to ask about routine private API
names, helper placement or minor dependency choices. Ask only when a decision materially changes
the product contract and cannot reasonably be inferred.

## Issue tracking contract

Read [DEFECTS](DEFECTS.md) for policy and [WORKFLOW](WORKFLOW.md) for implementation closure.
Search both open and closed GitHub Issues; revalidate current evidence and avoid duplicate/speculative
tickets. Every new actionable concern needs type/severity/area, stage where assigned, evidence and
acceptance. Do not duplicate current Open/Closed state in repository Markdown.

Inspect the issue before implementation; reference it with `Refs #N`, avoiding automatic close keywords
until required tests/acceptance and Ubuntu/Windows CI have passed. Post final SHA/evidence/resulting
invariant, then close using completed, not_planned or duplicate appropriately. A stage closeout must
reconcile its labelled open issues and cannot silently leave owned P0/P1 unresolved. ROADMAP is the
stage contract, GitHub the actionable queue. No later stage starts just because its Issue exists.
