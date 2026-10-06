# C2 report

Historical evidence for the recorded baseline; the registry owns current stage state.

## Baseline

Started from clean public `baad86dead6be88381f25fe53bf72c62eee7b026` in
`/tmp/rustcraft-r2-recovered`. Owner authorized C2; BG1 and M6 are excluded.
Issue #15 was queried open and revalidated against the actual global orientation implementation.

## Scope completed

Common typed content composition, voxel/item category proof, indexed tags/capabilities, semantic
resources, bounded native Use handlers, compact definition-local state compilation, semantic
serialization and existing renderer/chunk-codec consumers. Implementation acceptance passed on Ubuntu and Windows.

## Implementation

Before: validated NamespacedId/BlockKey -> voxel-only GameRegistry registration -> explicit package
order/lexical definitions -> compiled dense BlockId -> renderer global orientation bits. Legacy
mod-api definitions provide local geometry/item/policy adapters. BlockState is BlockId + u16 variant;
chunk storage already uses semantic keys + variants. A1 placement resolves compiled profile identity;
Minecraft supplies work/recipe/drop policy through public registration.

After: ContentDefinition owns semantic identity, metadata, typed mass/friction parameters, independent
tag/capability sets, semantic dependencies and registered handler bindings. VoxelDefinition composes
category collision/material/light/textures/state; ItemDefinition composes stack limit/icon. The common
layer has no voxel-only assumption. Entity/fluid specialization remains future work, without an ECS.

Validated definitions compile into category vectors and deterministic lexical tag/capability catalogs
with bitset membership. Typed properties are accessed directly. Handler keys resolve to sorted native
slots once at compilation. No GPU handle, authored numeric identity, dynamic property map or reflection
is introduced. A1 grants remain distinct from content contract capabilities.

Native Use dispatch supplies immutable definition/state/position context and the existing CommandBuffer.
It validates source/result states, same-position effects and a 64-command output bound before returning
commands for authoritative application. It exposes no mutable World. Handler bindings require declared
interactable support. Native code remains trusted; M6 execution sandbox/quotas are not implemented.
Item handler registration is rejected until a real category consumer exists.

StateSchema uses typed facing/axis/half/shape/powered/connection domains, semantic field keys/defaults
and optional forbidden full combinations. Fields and shape choices sort deterministically; defaults
occupy zero and mixed-radix strides encode at most 65536 combinations into u16. Invalid schemas,
values, combinations, noncanonical serialized ordering and incompatible versions fail explicitly.
Compiled value slots and orientation slots avoid parsing/allocation in renderer state lookup.

SemanticVoxelState contains a semantic block key, schema version and semantic values; no dense IDs.
Current chunk envelopes remain semantic key + variant. Legacy orientation adapters retain existing
Minecraft/untouched state bits, including previously uninterpreted bits; canonical states validate
strictly. Schema evolution requires explicit compatibility/migration. Canonical schema contracts
contribute to profile fingerprints; legacy fingerprints remain unchanged.

## Evidence

- Independent reactor and charge item share the common contract and diagnostic tag, with different
  capability sets. The reactor uses typed mass, semantic texture resources, a native handler and
  facing/powered state. The normal sample workflow dispatches and applies its registered command.
- Existing renderer bridge resolves canonical rotation; collision_for_state validates the schema
  for current Empty/FullCube lookup. No generalized model/shape/collision system is implemented.
- Multiple schemas combine facing/powered/connections, axis, and half/semantic shape choices.
  Exhaustive domain roundtrips prove compact encoding; reversed authoring order preserves meaning.
- Two reordered compiled package profiles assign different dense IDs while semantic JSON roundtrips,
  tag indexes and schema fingerprints retain meaning. Invalid handler effects/properties are rejected.
- Existing actual chunk codec saves/reopens the canonical reactor variant under a reordered profile;
  semantic state is unchanged, and invalid variants fail the schema-aware resolver.
- Minecraft voxels compose common identity/capabilities through the existing adapter; representative
  stick item derives stack semantics from the authoritative retained descriptor. Work/recipe/drop
  policy stays in minecraft-b173. Retained legacy slot/damage/worldgen/render/persistence adapters
  remain explicit; no second authored mining/tool taxonomy is introduced.

## Measurements

BlockState remains 8 bytes; variant remains u16. No per-voxel maps, strings or heap state are added.
Compiled definition access is vector indexed, membership is bitset indexed, properties are typed
fields and state extraction is indexed arithmetic. Event-boundary semantic decode may allocate;
it is not inserted into inner voxel/render loops. No performance improvement is claimed.

## Tests

Focused Game API/schema, sandbox, renderer bridge and Minecraft preservation tests passed.
`just sample-game` and `just smoke` passed. Sample offscreen GL/llvmpipe is composition evidence,
not representative graphics performance. The local `just ci` documentation/format/check/test gates
passed (347 tests, two existing skips); its initial strict Clippy stage reported three style errors.
After narrow corrections and a forbidden-combination-order regression, all 13 Game API tests and
strict workspace all-target/all-feature Clippy passed. Unaffected expensive workspace tests were not
repeated; public CI validates the final complete suite. All substantial Cargo commands
run sequentially with CARGO_BUILD_JOBS=2; recovery debug settings do not alter owner profiles.

## Issues resolved

No issue was falsely closed. The C2-owned foundation of #15 is accepted; the issue remains open
for its explicitly transferred BG1 consumer acceptance. No new ticket was created.

## Issues remaining / waivers

#15: C2 foundation accepted; generalized geometry/model/collision consumer proof transferred to BG1
per the stage contract and explicit owner authorization. The issue is OPEN with stage:BG1; its
body and evidence comment record the exact completed portion and residual acceptance.
BG1 must demonstrate state-selected generalized model/collision/selection data, including half/shape
combinations, in a coherent independent real consumer with codec/CPU/platform regressions.
This is a scoped owner transfer, not a waiver of BG1 acceptance or completion of R1.0-003.

## Known limitations

Only voxel Use dispatch and current Empty/FullCube collision exist. No generalized geometry, fluid
simulation, ECS, WASM/mod downloading, complete mod SDK or scripting DSL is implemented. Canonical
schema version/layout changes need an explicit game migration policy; C2 does not invent a generic
save-format migration. Existing chunks carry no per-definition version: a version bump alone cannot
protect old key+variant data. Published layouts must stay frozen or use a new semantic key / explicit
game migration before reinterpretation. Legacy variant permissiveness is preserved compatibility only.

## Implementation SHA

`bbfa3bf4395aea2992a68daea9f3c1a99edc0402` — composed content, schemas, native handler/renderer and codec proof.

## CI

[Implementation CI 37397475236](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37397475236):
Ubuntu success, Windows success, including the final complete workspace suite.
Closeout CI is discoverable by the closeout commit below; both platforms must pass.

## Closeout SHA

The commit titled `Record C2 acceptance` introducing this finalized report is the closeout identity;
resolve with `git log -1 --format=%H --grep='^Record C2 acceptance$'`.
Registry records C2 closed, focus BG1 and BG1 planned. BG1 implementation was not started.
