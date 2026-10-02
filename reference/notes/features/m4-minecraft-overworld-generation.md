# M4 Minecraft Beta-like overworld generation study

Question: which player-visible Beta 1.7.3 overworld properties should the first RustCraft
generator preserve, and which implementation details can be replaced to make chunk generation
deterministic across request order and worker scheduling?

## Sources consulted

- Historical/reconstructed Beta server source, `ChunkProviderGenerate.java` and `MapGenCaves.java`
  in `jacobo-mc/mc_b1.7.3_release`, locked revision `740c583901e1`.
- Independent Rust interpretation, `mc173/src/gen/overworld.rs`, `cave.rs`, `tree.rs` and
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

## Acceptance criteria

- Positive and negative neighboring chunks have no request-order-dependent seams.
- A feature crossing a border appears identically when either chunk is generated first.
- Canonical block-state hashes match for normal, reverse and shuffled chunk orders and for one vs.
  multiple generation workers.
- Seed, generator ID/version, profile semantic fingerprint and chunk coordinate are represented
  in generation/persistence identity.
