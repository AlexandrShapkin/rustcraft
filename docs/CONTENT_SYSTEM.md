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

Package versions are independent semantic versions (`major.minor.patch` with optional SemVer
prerelease/build metadata); they are not derived from the RustCraft product version. `GameProfile`
is the native composition boundary above these primitives. It names the packages,
resources and systems that form one game instance; it does not replace manifest resolution or
content-addressed delivery. A Minecraft profile composes `voxel_std` and `minecraft_b173`; another
game can compose `voxel_std` and its own package without loading Minecraft.

Definitions and resources use owned, validated namespaced IDs. Shared validation underlies
`PackageId`, `ResourceId`, `BlockKey` and `TextureKey`; typed wrappers prevent accidental category
mixing. IDs may originate from runtime-loaded strings and do not require static Rust literals.

Packages author semantic definitions and never choose final global numeric block IDs. A profile
compiler processes packages in explicit profile order and definitions in deterministic package
lexical `BlockKey` order, reserves the declared default voxel first, and emits semantic-to-handle and
O(1) handle-to-definition indexes. `BlockId` is a dense profile-local handle, never a permanent
identity. Saves and protocols must use semantic IDs or include an authoritative, versioned mapping.

`GameProfile.packages` and `.resources` use `PackageId` and `ResourceId`; block and texture
definitions use `BlockKey` and `TextureKey`. All wrappers share `NamespacedId` validation/storage,
while generic system/capability identifiers remain `ContentId`. Duplicate semantic block
definitions are rejected atomically. Block-definition override layering remains deferred. Resource
textures are different: packages are processed in explicit profile order and a later package
deterministically replaces an earlier provider of the same `ResourceId`; the compiler records each
replacement for inspection.

## Content-addressed cache

Immutable blobs are identified with domain-separated BLAKE3 digests. Manifest identity uses an
explicit canonical binary encoding: length-prefixed strings, fixed-width integer fields, package
sorting by semantic ID, and sorted/deduplicated dependency and target sets. It includes package ID,
version, kind, content digest, dependencies and targets.

R1.1 atlas cache identity covers compiler version, resolved semantic ID and provider, source-byte
hash, crop, selected page dimensions/page limit, padding, sampler, mip count and anisotropy policy.
Corrupt entries are ignored and rebuilt. Writes use process/counter-unique create-new temporary
files followed by atomic rename. Cache write failure is non-fatal but emits a warning and is
recorded in metrics.

Texture definitions retain semantic `TextureKey` values until resource compilation. Renderer-facing
`TextureHandle` plus normalized `AtlasRegion` values describe physical placement without assuming
a tile size or atlas page count. Native discovery considers only
`assets/<namespace>/textures/**/*.png`; future `models`, `sounds`, `data` and other classes are
ignored by the texture compiler, while unsupported files inside `textures` fail deterministically.
Symlinks, traversal and malformed semantic paths remain rejected.

PNG headers are inspected before parallel decode. Dimensions and checked `width * height * 4` are
validated, and predicted bytes are summed once per unique physical source against the global decode
budget before any worker allocates output. Decode uses at most eight workers. The Beta package's
virtual importer exposes terrain crops and UI/player sheets through the same generic compiler.

Packing is deterministic height/width/semantic-ID ordered shelf placement with two-pixel padding
and edge extrusion by default. Page dimensions are selected adaptively from 128, 256, 512 and
larger powers of two, bounded by the configured/device-safe maximum and page-count policy. Every
physical page has one sampler policy. Nearest and uniform-linear pages are represented through GPU
upload; mixed policies within one page are rejected. R1.1 uses one mip level and anisotropy 1.

## Target filtering

A graphical client may need textures, sounds and UI. A headless bot normally does not. Packages
must therefore declare runtime targets so bots do not download irrelevant rendering/audio assets.

## Security boundary

Automatic content delivery must never mean automatically loading arbitrary `.so`, `.dll` or
`.dylib` from an untrusted server. Downloadable executable logic belongs in a sandbox such as WASM
with explicit capabilities, validation and resource limits.

## World persistence identity

Persistent chunks store semantic `namespace:block` identities and the current compact `variant`
value through a local section palette. Loading resolves those keys against the active game package
definitions before producing dense runtime `BlockState` arrays; unknown keys fail with a controlled
compatibility error. `CompiledGameProfile::semantic_fingerprint` hashes the semantic profile ID and
sorted block identity/gameplay descriptors, excluding texture/resource manifests and profile-local
numeric handles. It is diagnostic full-profile identity, not a hard saved-voxel compatibility gate:
presentation, collision, targetability and material changes do not invalidate semantic palettes.
Loading resolves every persisted key and variant through the active profile; missing semantic keys
fail explicitly. Metadata separately stores the persisted-state schema version (currently semantic
block key + `u16` variant, v1), game/profile family, and generator ID/version. A schema or family
mismatch is a compatibility error. Generator mismatch only blocks generating absent chunks; already
persisted chunks are independently loadable. Legacy metadata v1 is interpreted as persisted-state
schema v1 and rewritten as metadata v2 after successful palette resolution.

The generic storage container is versioned independently at world metadata, chunk-payload and
player-record boundaries. The generic player record is an opaque bounded, checksummed, atomic
envelope keyed by a validated persistent player ID; the active game owns its versioned payload
codec. Minecraft v1 persists position/orientation, mode, all inventory slots, selected hotbar slot,
cursor transaction stack, and crafting grid using semantic item names plus count/damage. Runtime
`ItemId` handles are resolved on load and never stored as persistent identity. A missing record in
an older world means initialize the normal first-time player; missing semantic items in an existing
record are explicit compatibility errors. Chunk compression is tagged per payload and bounded on
decode.

PERSISTED: world metadata, authoritative voxel columns, and current durable local-player state.
DERIVED/REBUILT: lighting, render snapshots, meshes, GPU state, collision contacts, raycasts and
input/UI interpolation. NOT YET PERSISTED: dropped item entities, other entities and world time.

Voxel presentation keeps authored material and interaction semantics through profile compilation.
`Liquid` is distinct from ordinary translucent material: it selects blended section/page submission
and static lowered-surface geometry, while `targetable` controls the normal interaction ray rather
than being inferred from solidity. Minecraft Beta water currently uses source variant 0, empty
collision and is not a normal break/place target. Minecraft supplies an underwater fog policy;
the client resolves it via compiled liquid material/surface metadata and generic world shaders apply
distance attenuation. Flow levels, propagation and swimming remain future gameplay policy; generic
storage and rendering do not identify Minecraft block IDs.
