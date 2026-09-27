# Codex operating notes

Codex should treat `AGENTS.md` as the compact always-on contract and use the detailed documents and
repo-local skills for task-specific depth.

## Normal loop

1. Read `AGENTS.md` and the documents relevant to the batch.
2. Run `just doctor` once per environment and `just bootstrap-check` before substantial changes when
   the workspace is expected to build.
3. Implement a coherent batch rather than one micro-change at a time.
4. Validate with `just` recipes.
5. Fix blockers immediately; record unrelated non-blocking defects.
6. Update durable decisions/docs only when implementation makes them real.

For behavior/protocol/asset research, run `just refs-status` and use `reference/SOURCES.md` to choose
sources. Do not scan every external repository by default. `just refs-lock` can snapshot the exact
commits consulted for a durable research result.

When available, Serena is preferred for symbol-aware repository navigation/refactors and Context7
for current third-party API documentation. Ordinary shell/git/rg/jq tools remain first-class and
are often faster for simple tasks.

The user wants low-interaction autonomous progress. Do not stop to ask about routine private API
names, helper placement or minor dependency choices. Ask only when a decision materially changes
the product contract and cannot reasonably be inferred.
