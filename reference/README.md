# Reference material

`reference/` is the project's research perimeter. It is deliberately separate from production
Rust code.

Public third-party reference repositories are declared in `sources.json` and cloned locally into
`reference/external/` with:

```bash
just refs-fetch
```

They are ignored by this repository and are not vendored into bootstrap archives.

Useful commands:

```bash
just refs-status      # show availability, branch and revision
just refs-fetch       # clone missing enabled references
just refs-update      # fast-forward existing reference clones
just refs-lock        # record exact checked-out revisions in LOCK.json
```

The user's own decompiled/reference Beta 1.7.3 source may be placed at:

`reference/minecraft-beta-1.7.3-src/`

The original/default Beta 1.7.3 texture pack may be placed either as:

- `reference/assets/vanilla-b1.7.3/` (extracted), or
- `reference/assets/vanilla-b1.7.3.zip` (archive).

Both are intentionally ignored by git and must not be redistributed by this project by default.

Read `SOURCES.md` and `docs/REFERENCE_POLICY.md` before using these materials in implementation
work.
