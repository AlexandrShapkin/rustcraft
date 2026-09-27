# Development workflow

Work in coherent vertical batches. The owner wants low interaction overhead: make routine technical
decisions autonomously, implement them, validate them and report results.

## Canonical commands

Use `just` recipes instead of repeatedly inventing command sequences.

Typical loop:

```text
just doctor
just refs-status        # when reference material may matter
just bootstrap-check

implement a coherent batch

just ci
just smoke
```

Fetch public research inputs with `just refs-fetch` when needed. Do not make normal builds depend on
network access or on reference repositories being present.

## Defect batching

Fix immediately: build blockers, crashes in the required scenario, security/memory-safety issues,
data corruption, broken contracts and architecture mistakes that become expensive if left.

Record lower-priority functional/polish/cleanup issues in `docs/DEFECTS.md` and repair related
issues together later.

## Documentation discipline

Update architecture/decision documents when the implementation changes a durable contract. Do not
rewrite documentation as a substitute for code.

## Research

For current Rust/library behavior, prefer official docs. If Context7 is available, use it. For
codebase-wide symbol work, prefer Serena when available. Use local tools (`rg`, `git`, `jq`,
`cargo metadata`, `cargo tree`, source reading) as a reliable fallback.

For Beta behavior, protocol, legacy formats or visual conventions, read `reference/SOURCES.md`, run
`just refs-status` and select the narrowest source that answers the question. Cross-check
nontrivial ambiguities rather than inheriting one reference implementation blindly. Durable
project conclusions belong in `reference/notes/`, not as pasted source excerpts.

`just refs-lock` may be used after significant research to record the exact third-party reference
commits consulted.
