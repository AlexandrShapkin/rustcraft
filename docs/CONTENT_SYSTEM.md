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
virtual importer exposes terrain, inventory-panel, hotbar, selector and preview-part crops through
the same generic compiler. Active HUD/player contracts refer to semantic subresources rather than
whole sheets. Individual semantic files, rearranged crops and later-package overrides resolve
through the same compiled regions/cache.

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

Minecraft generator resolution is game-owned and exact. Saved
`minecraft_b173:overworld:v1` selects the frozen v1 implementation even after v2 becomes the
new-world default; an unknown ID/version never falls forward. Generator identity is independent of
the compiled profile fingerprint and persistence schema. V2 biome identity is Minecraft-owned and
semantic in diagnostics, but derives from seed/version/coordinates rather than being persisted as
profile-local handles or per-voxel strings.

The generic storage container is versioned independently at world metadata, chunk-payload,
world-global envelope, spatial entity and player-record boundaries. The generic player record is an opaque bounded, checksummed, atomic
envelope keyed by a validated persistent player ID; the active game owns its versioned payload
codec. Minecraft v1 persists position/orientation, mode, all inventory slots, selected hotbar slot,
cursor transaction stack, and crafting grid using semantic item names plus count/damage. Runtime
`ItemId` handles are resolved on load and never stored as persistent identity. Dropped items use
the semantic entity type `minecraft_b173:entity/item`, a stable 128-bit EntityId and the same
semantic ItemKey/count/damage contract; unknown entity types, unknown item keys and unsupported
known schemas are explicit compatibility errors. Generic chunk v3 storage sees only bounded opaque
spatial records and tombstones, never Minecraft stack fields. A missing record in
an older world means initialize the normal first-time player; missing semantic items in an existing
record are explicit compatibility errors. Chunk compression is tagged per payload and bounded on
decode.

The world-global `RCSTATE` v1 envelope similarly stores sorted independently versioned opaque
components. Minecraft's `minecraft_b173:world/clock` v1 stores simulation ticks and pauses while
closed. Unknown global components are preserved opaquely; unsupported known versions fail.

PERSISTED: world metadata, authoritative voxel columns, current dropped item entities, world time,
pickup recovery receipts and current durable local-player state. DERIVED/REBUILT: lighting, item
render bob/orientation, render snapshots, meshes, GPU state, collision contacts and raycasts.
TRANSIENT: mining progress, residency/scheduler queues, lifecycle tokens, input/controller intent
and UI interpolation. No other current gameplay entity/global systems exist to persist.

Voxel presentation keeps authored material and interaction semantics through profile compilation.
`Liquid` is distinct from ordinary translucent material: it selects blended section/page submission
and static lowered-surface geometry, while `targetable` controls the normal interaction ray rather
than being inferred from solidity. Minecraft Beta water currently uses source variant 0, empty
collision and is not a normal break/place target. Minecraft supplies an underwater fog policy;
the client resolves it via compiled liquid material/surface metadata and generic world shaders apply
distance attenuation. Flow levels, propagation and swimming remain future gameplay policy; generic
storage and rendering do not identify Minecraft block IDs.

## C2: composed definitions and local state

`game-api::ContentDefinition` composes existing validated `ContentId` identity, typed metadata,
immutable `DefinitionProperties`, semantic tags, supported contract capabilities, semantic resources
and handler bindings. `VoxelDefinition` adds voxel collision/material/light/face resources and state;
`ItemDefinition` adds stack limit and an optional semantic texture icon. Common data contains no
voxel assumption, permitting future entity/fluid specializations without implementing those systems.
Categories use Rust composition, not inheritance, a universal dynamic map or engine class switches.

| Concept | Implemented meaning |
| --- | --- |
| Tag | Namespaced query/classification membership; changes no behavior. |
| Property | Typed immutable data: optional finite nonnegative mass and friction; not mining/tool policy. |
| Capability | A supported content contract, compiled independently from tags; not a host/session grant. |
| Handler | A native function bound by semantic key to the demonstrated `Use` event. |
| State | Compact mutable variant interpreted by this definition's schema. |
| Resource | A semantic dependency; category texture bindings remain semantic `TextureKey`s. |

Profile compilation validates authoring, follows explicit package order and lexical definition order,
and produces dense voxel/item vectors. A deterministic lexical catalog maps tag/capability keys to
profile-local typed indexes; membership is a bitset lookup. Property access is typed direct field
access. Resource references remain semantic until the existing resource bridge resolves them.
`ItemDefinitionId`, tag/capability indexes, BlockId and GPU/atlas handles are never authored identity.
No per-frame key parsing, property reflection or per-voxel heap object is introduced.

### Definition-local state

`StateSchema` contains versioned semantic fields, typed domains/defaults and optional prohibited full
combinations. Facing, axis, half, semantic shape choices, powered and six-bit connection domains are
supported; each definition selects only its own fields. Compilation sorts fields lexically and shape
choices semantically, places each declared default first, then uses checked mixed-radix encoding.
Default variant is zero. At most 16 fields and 65536 combinations fit `u16`; invalid domains,
combinations, duplicate/missing fields and out-of-range variants fail explicitly. Forbidden defaults
are rejected. Encoding depends on the schema, never package/dense-ID order or Beta metadata.

`CompiledStateSchema` offers indexed value access and pre-resolved orientation slots. The renderer
bridge uses its rotation; `collision_for_state` validates state before the current Empty/FullCube
lookup. The independent reactor's handler uses facing/powered state. These are bounded C2 consumers;
BG1's generalized geometry/model/collision/selection system is not implemented.

`SemanticVoxelState` serializes a semantic block key, schema version and canonical semantic field
values. Import validates the key/schema/value domains. This is a semantic boundary helper, not an M5
wire protocol or a replacement save envelope. Current chunks still store semantic key plus u16 variant;
the existing codec roundtrip under reordered profiles is tested. A schema version/layout change needs
an explicit game compatibility/migration policy; silently reinterpreting persisted variants is forbidden.
Canonical schema contracts contribute to profile fingerprints.

Existing Minecraft and other untouched voxel definitions explicitly select the legacy orientation
adapter with `state_schema: None`. Their historical bits, semantic fingerprints and persisted variants
are unchanged, including uninterpreted legacy bits. New canonical definitions reject invalid variants;
legacy permissiveness is compatibility, not the new schema contract. No storage width/format changes.

### Bounded native handlers and category proof

A registered `Use` handler receives immutable definition/state/position context and emits the existing
CommandBuffer. It receives no mutable World or ambient host authority. Profile dispatch rejects unknown
bindings, invalid canonical state, more than 64 output commands, out-of-context positions and invalid
resulting states before returning effects for authoritative application. Handler-bearing definitions
must declare the interactable contract. Only voxel use dispatch is currently implemented; item handlers
are rejected until a real category consumer exists. Native code remains trusted; this output bound is
not a completed WASM execution quota/sandbox. M6 is future work.

Sandbox reactor/charge content proves voxel and item categories, typed properties, shared tags,
different capability sets, semantic resources, command-emitting use and state-aware rendering without
Minecraft. The normal sample workflow invokes this registered handler and applies commands. Minecraft
voxel registration composes common data; representative stick item registration derives stack semantics
from the existing explicit adapter. A1 recipe/work/drop policy remains in minecraft-b173; retained
legacy slot/damage/worldgen/render/persistence adapters are not competing universal definitions.

Issue #15's C2 schema/serialization portion is implemented. Its generalized model/collision consumer
acceptance remains BG1-owned; a sample rotation/handler is not proof of generalized shapes.

## Target BG1: semantic models and separate shape contracts

BlockState → semantic ModelKey/model provider → compiled model → local ModelHandle → renderer.
Games map many states to variants (e.g. stair facing/half/straight/inner/outer) without engine classes.
Physical source geometry/layout belongs to packages/importers; generic rendering knows no Minecraft
model names. Full cube is the optimized common case, not the definition of a block.

BG1 supports partial boxes, slabs, stairs, quarter/compound shapes, posts/beams/panels, wedges,
45-degree and inner/outer slopes, composite geometry and bounded arbitrary static authored meshes.
A compact proof set suffices; no huge decorative catalog is required.

Resolve independently: render geometry, collision shape, selection/raycast shape, occlusion/coverage,
and light/coverage behavior. Detailed render triangles need not define physical or targetable shape.
AABB composition remains a fast path; raycast must hit the configured selection shape. Coverage must
express full/partial/no occlusion; light behavior must distinguish blocking, partial and transmitting/
non-occluding states. Exact polygon clipping or physically exact global illumination is not required
without measurement, but six full square faces cannot be the permanent API.

Acceptance: a new decorative 45-degree wedge normally needs a semantic definition, optional local
state schema, model/shape resources, properties/tags and optional placement handler. It must not need
special switches in engine, renderer, serializer, inventory renderer, raycaster or collision loop.
BG1 is a major C2 consumer. [VS1](VOXEL_SPACES.md) reuses these semantic shapes and physical properties
for structure aggregation, with specialized compiled forms allowed by profiling.

## Planned project-owned diagnostic content

DX2 owns a first-party diagnostic/test package using the public Game API; READY1 verifies independence.
Extend sandbox-style proofs with project-owned full cubes, transparent/cutout/translucent and emissive
materials, generalized non-full models, items/entities, useful medium/liquid, UI/resource primitives
and particles where supported. This tests engine mechanisms without Minecraft or proprietary assets;
it is not engine-core hardcoded admin blocks or gameplay policy. Unsupported categories wait for their
own contracts rather than being faked. No separate texture milestone or package implementation here.

## A1 bounded native policy registration

Public Game API WorkDefinition and RecipeDefinition carry existing semantic BlockKey/ResourceId
identities. Games choose effort, tool/item multipliers, reward and recipe content; shared WorkProgress
and recipe matching are mechanisms without Minecraft tool taxonomy. Minecraft registers its values
from minecraft-b173::policy through public native registration, then resolves them once to local
dense handles. Retained mod-api::legacy definitions are explicit local compatibility for worldgen,
renderer, slot transactions, tool-damage codec validation and persistence adapters; their hardness,
tool/drop/placeable fields are not universal external definitions. VoxelDefinition remains the generic
numeric-ID-free voxel contract. C2 now adds composed content and bounded handler dispatch; this A1 adapter still introduces no save format.
