# M4 Minecraft Beta-like overworld generation study

Question: which player-visible Beta 1.7.3 overworld properties should the first RustCraft
generator preserve, and which implementation details can be replaced to make chunk generation
deterministic across request order and worker scheduling?

## Sources consulted

- Historical/reconstructed Beta server source, `ChunkProviderGenerate.java` and `MapGenCaves.java`
  in `jacobo-mc/mc_b1.7.3_release`, locked revision `740c583901e1`.
- Independent Rust interpretation, `mc173/mc173/src/gen/overworld.rs`, `cave.rs`, `tree.rs` and
  `noise.rs` in `theorzr/mc173`, locked revision `16f39e762da2`.
- Repository policy: `docs/GAMEPLAY_CONTRACT.md`, `docs/REFERENCE_POLICY.md` and `docs/PRODUCT.md`.
  `just refs-status` confirmed both source revisions; local decompiled source/assets are absent.

The historical server's `ChunkProviderGenerate.provideChunk` reseeds terrain per chunk coordinates,
then performs biome selection, density terrain, biome surface replacement and caves. Its
`generateTerrain` interpolates a coarse 4×8×4 density lattice into a 16×128×16 chunk and uses
sea-level Y=64. `replaceBlocksForBiome` supplies grass/dirt-like surface layers, sand/gravel
variants, water below sea level and a randomized bedrock floor in the lowest five levels.
`populate` derives a chunk-local random seed from the world seed and odd coordinate multipliers;
it attempts lakes, ores and biome-dependent trees/plants, with feature origins offset by eight
blocks so generated features can cross chunk borders. Caves are seeded from neighboring chunk
origins and carve paths that can span the target chunk.

The independent `OverworldGenerator::{gen_terrain,gen_surface,gen_carving,gen_features}` confirms
the same important terrain/surface/sea/cave/feature split and explicitly notes that chunk
population touches neighboring chunks. Its per-generator state and Java-compatible RNG are
implementation choices, not RustCraft API requirements. Both implementations make RNG results
depend on explicit world/coordinate seeds rather than camera position; the historical population
write order itself is not a semantic requirement.

## Semantic invariants

- Terrain is coherent across horizontal chunk boundaries and has a meaningful sea/land relationship.
- Surface materials follow broad climate/biome policy; subsurface terrain is predominantly stone.
- The bottom boundary is protected by bedrock-like material; caves and resource deposits are
  underground and recognizable vegetation grows on suitable surfaces.
- A fixed seed and generator identity produce the same authoritative blocks for negative and
  positive coordinates, regardless of request order or worker count.
- A feature whose origin lies outside a chunk still has deterministic effects inside that chunk.

## Safe implementation differences

- Do not target bit-for-bit Java seed parity or duplicate the historical mutable RNG call sequence.
- Use coordinate/stage/feature-derived streams and pure neighborhood-origin evaluation so chunks
  can generate independently and in parallel.
- Use dense generic chunk builders and RustCraft's semantic block registry; do not bring Minecraft
  IDs, noise policy or feature rules into `engine-core`.
- Keep the engine's signed section coordinate model. A Minecraft generator may choose its own
  configured vertical range without imposing that range on generic worlds.
- Defer Beta details not represented by current content definitions when implementing them would
  require broad unrelated gameplay policy; record those exclusions explicitly.

## Optimization opportunities

Generate dense section arrays off-thread, sample smooth coordinate-based noise, clip deterministic
feature bounds to the destination chunk, avoid per-block command/event dispatch, and publish only a
fully generated chunk. Feature evaluation should be bounded by fixed neighboring origin radii.

## Chosen RustCraft behavior and deviations

M4 will provide a generic engine generation contract and a Minecraft-owned generator/configuration.
The generator will use a stable new algorithm keyed by seed, generator ID/version, stage and
coordinates; it is Beta-recognizable, not Java-seed-compatible. Chunk feature application will be
order-independent rather than mutating previously published neighboring chunks in population order.
Implemented systems and intentional omissions will be refined in the M4 architecture/performance
notes as their acceptance tests are added.

## M4-004 feature classification

### Required for generator v2

- Two smooth, coordinate-derived climate fields (temperature and moisture) and a small set of
  materially distinct Minecraft-owned biomes.
- Coherent macro land/water regions, rolling terrain, occasional stronger hills, sea level 64,
  water-filled lowlands, and a bedrock floor within the lowest five blocks.
- A surface pass separate from terrain mass. Grass/dirt, sand and gravel must respond to biome,
  elevation and nearby coast rather than an elevation-only beach band.
- Curved, occasionally branching cave networks evaluated from a bounded origin halo. Historical
  caves reject carving when their boundary encounters water and use lava only at the deepest
  levels; v2 must at least avoid indiscriminately flooding every low cave.
- Explicit coal, iron, gold and diamond policies with recognizable relative abundance/depth and
  origin-halo veins that agree across chunk borders.
- Biome-dependent simple log/leaves trees, including sparse plains, denser forests and none in
  deserts/ocean/beaches. Neighbor origins must make border canopies deterministic.
- Deterministic water-lake cavities with bounded cross-chunk influence.
- A bounded, deterministic, game-owned dry spawn search with solid support and headroom.
- Exact generator-version routing: existing version-1 worlds keep generating version-1 chunks;
  new worlds persist version 2 only after v2 acceptance.

### Representative or deliberately simplified

- RustCraft v2 uses smooth two-dimensional macro height fields plus three-dimensional carving.
  Beta's coarse 4x8x4 density interpolation is evidence for broad density variation, not a
  requirement to pay for a full historical density stack. The selected model preserves coast,
  lowland, rolling and hill silhouettes while remaining bounded and cheap per column.
- Biomes are reduced to ocean, beach, plains, forest, desert and hills because those classes can
  differ with current blocks and vegetation. Beta's larger climate lookup is not copied when a
  label would have no corresponding surface or feature behavior.
- Trees remain the current recognizable small oak topology. Large trees and species selection are
  deferred until their semantic blocks and gameplay distinction exist.
- Lakes use a bounded ellipsoid, inspired by Beta's 16x8x16 lake mask, but each origin is
  independently keyed and clipped into the destination column. This removes population-order
  mutation while retaining a recognizable shallow lake cavity.
- Ore veins use short overlapping ellipsoids along a deterministic segment. Attempt counts and
  depth bands preserve Beta's relative shape (coal common/high, iron common/lower, gold rare,
  diamond rare/deep) without Java RNG parity.
- Generated liquids are static authoritative source voxels. Flowing-fluid simulation is outside
  this generation slice.

### Deferred with reason

- Taiga, tundra, rainforest, seasonal forest, savanna, shrubland and swampland are deferred: snow,
  conifer/birch species, swamp water/foliage and biome plants are not current semantic content, so
  most would be labels without meaningful generation differences.
- Flowers, mushrooms, reeds, pumpkins, clay deposits and snow cover are deferred until their
  blocks and associated gameplay/content policies exist.
- Dungeons are deferred because recognizable Beta dungeons require spawners, mobs, mossy
  cobblestone, chests and loot/container semantics. Adding placeholders would expand M4 into
  unrelated gameplay and create misleading structures.
- Underground liquid springs are deferred until static sources can coexist with a defined future
  fluid-update contract; lake cells and sea water provide the accepted M4 liquid generation.
- Lava generation is deferred by the completed content audit: the active profile has water but no
  semantic lava block, lava resource mapping, distinct camera medium or emission definition. M4
  deliberately avoids an untextured or water-disguised substitute. A later content slice may add
  static lava through the normal block/profile/light path in a new generator version; frozen v2
  output, like v1 output, must not be silently changed.

### Out of project scope for this pass

- Java `Random` parity, exact Beta seed equivalence and historical population call-order effects.
- Historical bugs and generation that mutates already-published neighboring chunks.
- Villages and all post-Beta-1.7.3 structures; Nether generation; mobs, weather, fire, damage,
  flowing liquids and container gameplay.

## Reference-derived constants and behavior

The historical climate manager samples independent temperature and humidity octave fields and
classifies on their joint value (humidity is effectively moderated by temperature). RustCraft
keeps the two-field semantic foundation but uses its own smooth value-noise domains. The
historical surface pass uses biome top/filler blocks plus sand, gravel and depth noise around sea
level; this supports a separate, context-sensitive surface stage rather than v1's height band.

Historical population attempts water lakes in roughly one quarter of chunks and lava lakes less
often; coal uses 20 attempts up to world height, iron 20 below Y64, gold 2 below Y32 and diamond 1
below Y16, with vein sizes 16/8/8/7. These are calibration evidence, not exact-count contracts.
Historical caves evaluate neighboring origins, curve and branch, refuse a carve volume that meets
water, and replace only stone/dirt/grass. RustCraft adopts those visible principles with fixed
halos and independently derived streams.

Every v2 random decision derives from `(world seed, generator version, stage domain, feature
domain, origin coordinate, feature index)`. There is no shared mutable stream across stages. Thus
adding an unrelated stage cannot reshuffle ores, trees or lakes inside version 2, and row-major,
reverse, shuffled and parallel requests produce identical semantic blocks.

## Acceptance criteria

Stabilization found a real origin-halo defect: length-49 cave tunnels plus radius below 3.3 can
reach beyond a two-column halo. Seed 17 reproduces different carving against a wider reference.
V2 now evaluates four origin columns in each direction, and compares against five in permanent
tests. This justified replacing the interrupted candidate hash; no v1 output changed.
The final semantic v2 canonical region is seed 731173, X/Z `[-2,2)`, ordered X then Z, section Y
then dense voxel order. BLAKE3 includes little-endian coordinates, semantic key length/UTF-8 bytes
and variant. Its hash is `4e134fa137fa5477fc3d28c9a610afd14019b0d0304246e43cd9940deb4684e7`.
The historical v1 lock remains `e0d1f83c16b281124b7a9c190f667d7eddaa8b2f35ef5ef98ccb4bab434c1bb6`.

- Positive and negative neighboring chunks have no request-order-dependent seams.
- A feature crossing a border appears identically when either chunk is generated first.
- Canonical block-state hashes match for normal, reverse and shuffled chunk orders and for one vs.
  multiple generation workers.
- Seed, generator ID/version, profile semantic fingerprint and chunk coordinate are represented
  in generation/persistence identity.
