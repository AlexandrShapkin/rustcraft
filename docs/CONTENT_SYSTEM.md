# Server-defined content system

Joining a multiplayer server should not require manual installation of a loader, matching mod
folder and resource pack.

Expected flow:

1. connect/handshake;
2. receive a content manifest;
3. resolve dependencies and target requirements;
4. reuse locally cached content by cryptographic hash;
5. fetch only missing/outdated content;
6. verify integrity and policy/capabilities;
7. initialize permitted modules/resources;
8. enter the world.

## Core concepts

The architecture should eventually provide equivalents of:

- `ContentManifest`;
- `PackageId` / `PackageVersion`;
- `ContentHash`;
- dependency constraints;
- package target (`client`, `server`, `bot` or combinations);
- package kind (`resource`, `data`, sandboxed executable, configuration, etc.);
- `ContentCache`;
- dependency resolver;
- downloader/source resolver;
- integrity verifier;
- capability resolver;
- resource manager.

`GameProfile` is the native composition boundary above these primitives. It names the packages,
resources and systems that form one game instance; it does not replace manifest resolution or
content-addressed delivery. A Minecraft profile composes `voxel_std` and `minecraft_b173`; another
game can compose `voxel_std` and its own package without loading Minecraft.

Definitions and resources use validated namespaced IDs. Numeric registry handles remain valid for
compact runtime storage after semantic registration/resolution.

## Content-addressed cache

Immutable blobs should ultimately be keyed by a strong content hash so identical data shared by
multiple servers is downloaded once.

## Target filtering

A graphical client may need textures, sounds and UI. A headless bot normally does not. Packages
must therefore declare runtime targets so bots do not download irrelevant rendering/audio assets.

## Security boundary

Automatic content delivery must never mean automatically loading arbitrary `.so`, `.dll` or
`.dylib` from an untrusted server. Downloadable executable logic belongs in a sandbox such as WASM
with explicit capabilities, validation and resource limits.
