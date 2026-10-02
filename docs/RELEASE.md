# Release policy

RustCraft uses pre-1.0 Semantic Versioning. The authoritative Engine/Product SemVer is
`[workspace.package].version` in the root `Cargo.toml`; `just version` prints it. The initial
version was `0.1.0-alpha.1`; the current release candidate is `0.1.0-alpha.2`. The alpha.1 tag's
workflow failed before publishing artifacts and is intentionally left unchanged. Milestone IDs are
implementation scope, not release-version components.

These version domains are deliberately independent:

- Engine/Product SemVer describes the RustCraft application release.
- Game API version describes the public native game-extension contract.
- World metadata, chunk payload, persisted-state schema, player-record and player-component
  versions describe their own storage compatibility boundaries.
- Resource/package SemVer belongs to each content package; it is not inherited from the product.
- A future network protocol will have its own compatibility version.
- `minecraft-b173` package/version identity is game content identity, not engine identity.

Changing product SemVer alone does not migrate or invalidate saves. A save-format change does not
pretend to be a Game API change. Current Cargo crates inherit the workspace version for build
metadata; this does not couple content package versions to it.

## Tags and artifacts

Formal tags use `v` plus the exact workspace version, for example `v0.1.0-alpha.2`. The tagged
GitHub Actions workflow checks this equality before building Linux x86_64 and Windows x86_64
archives, validates their contents, emits SHA-256 sidecars, and creates a GitHub Release. Ordinary
commits do not publish. The alpha.1 tag remains unchanged and has no successful published release.

`just release-build` invokes the same `scripts/release.py` packaging path used by CI and places
archives/checksums in ignored `target/release-dist/`. `just release-check` runs workspace CI and
validates archives by listing/extracting them, checking executable `--version`, running the
headless server smoke path, checking checksums and rejecting local reference/save/build paths.
Build identity embeds product version, short Git commit, dirty status, Cargo profile and target
triple; it is available as `<client> --version` and `<server> --version` before graphics or world
initialization.

The current client/server executables still statically compose the first-party Minecraft package;
runtime game selection is not implemented. The client requires a user-supplied compatible Beta
terrain sheet at `reference/assets/terrain.png` or via `RUSTCRAFT_TERRAIN_TEXTURE`. Release archives
never bundle `reference/assets`, reference repositories, saves, caches or test output. The
`sandbox-test` integration binary remains the proof of a generic non-Minecraft game, not yet a
standalone interactive product flavor.

## Licensing and repository identity

RustCraft-authored source is offered under either MIT or Apache-2.0, at the user's option. These
licenses do not cover Minecraft/Mojang/Microsoft trademarks, original game assets, user-provided
assets, third-party reference repositories, or dependency code. The project does not claim
affiliation with Mojang or Microsoft. The first-party `minecraft-b173` package may interoperate with
compatible user-supplied assets; original Beta assets are not redistributed.

The standard dependency notice collection remains generated from Cargo metadata and dependency
source files. It is independent of the RustCraft project license.

The intended canonical GitHub repository is `https://github.com/AlexandrShapkin/rustcraft`. The
current configured remote may still use the old `rustcraft-b173` URL. The owner must rename it in
GitHub Settings → Repository name → `rustcraft`; then update a local remote if desired:

```sh
git remote set-url origin git@github.com:AlexandrShapkin/rustcraft.git
```

GitHub redirects are only a convenience; documentation uses the canonical `rustcraft` identity.
Local history has been rewritten to remove `reference/assets`. The private remote has not been
changed. If it contains the old history, first inspect every remote branch/tag with
`git ls-remote --heads --tags origin`. The previously fetched `main` tip was
`91e28a77f60097db6e145f2288ac06d7ed7fd00c`; if the remote still has that exact tip and `main` is
the only branch requiring replacement, the owner can run:

```sh
git push --force-with-lease=refs/heads/main:91e28a77f60097db6e145f2288ac06d7ed7fd00c origin refs/heads/main:refs/heads/main
```

The lease makes the push fail if the remote moved. Any other remote branch/tag that retains old
history must be deliberately rewritten or removed too. No push or publication is performed by this
tooling.

Before publication, review the final archives/notices and ensure the canonical repository/history
are ready. After main CI passes for the release preparation commit, create and push the tag matching
the workspace version. Product branding does not change save paths, semantic content IDs, generator
IDs, or world compatibility identity.
