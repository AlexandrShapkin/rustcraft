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

## Target C2: common composed definitions and local state

Current: Game API registers semantic `VoxelDefinition` with face resources, Empty/FullCube collision
and a capability list; transitional mod-api has separate policy-rich BlockDefinition/ItemDefinition.
There is no complete common definition, handler catalog or definition-local state schema yet.
C2 is the owning planned migration; A1 first secures external identities and affected policy boundaries.

Target common `ContentDefinition` layer composes identity, metadata, tags, properties, capabilities,
handlers and resources. Voxel/block, item, entity, fluid and future categories add only meaningful
domain contracts. This is Rust composition, not inheritance or a universal definition full of
irrelevant fields. Keep these meanings distinct:

| Concept | Meaning |
| --- | --- |
| Tag | Classification or group membership. |
| Property | Typed authored data/parameter. |
| Capability | Supported semantic interface/mechanism. |
| Handler | Reaction to a registered event/lifecycle hook. |
| State | Compact mutable state of an individual instance. |
| Resource | Semantic presentation/data dependency. |

Content capabilities are supported interfaces; security capabilities are grants authorizing use.
They are related through validated composition, not interchangeable tags or permissions.

Authored definitions are flexible, namespaced and validated. Profile compilation produces typed,
validated, indexed, cache-friendly runtime definitions, dense local handles and fast capability
lookup. Semantic strings stay at composition/serialization/debug boundaries. Do not introduce
`HashMap<String, DynamicValue>` or per-voxel/entity string lookups in hot loops.

C2 closes R1.0-003 with deterministic definition-local canonical state schemas: facing, axis, half,
shape, powered and connection-mask properties are examples, not global reserved-bit assignments.
Compile each schema into compact encoding, retaining compact BlockState where practical. Semantic
serialization must validate the schema independently of Beta metadata; models, handlers and collision
use the same canonical state meaning. Current key+u16 schema requires explicit compatibility/versioned
migration if encoding changes; this documentation pass changes no format.

Handlers bind explicitly registered events such as place, break, use, tick, neighbor change, entity
contact, damage and craft. Event/query context → handler/system/rule → CommandBuffer → authoritative
mutation remains the boundary. No raw mutable World mod contract or giant universal callback API.
Native first-party Rust and future WASM adapters share semantics, not necessarily runtime representation.

Packages may eventually contain typed voxel/item/entity/fluid definitions, models, shapes and permitted
handler/system metadata as well as other resources. Existing texture discovery remains texture-only;
this is not an implemented universal loader. Typed semantic wrappers remain useful; do not erase domain
safety by forcing everything into one untyped ResourceId. Dense handles remain local compilation data.

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
