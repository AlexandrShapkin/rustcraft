# Modding

First-party gameplay modules and third-party mods share semantic concepts but do not need the same
execution mechanism.

The fundamental extension contract is the Game API, not a Minecraft-owned mod layer.
`minecraft_b173`, another native game, and native first-party extensions all register packages,
definitions, systems and capabilities through the same public mechanisms. Downloaded third-party
code will use a versioned sandbox adapter over those concepts rather than privileged engine access.

## First-party

Native Rust, statically linked into the normal product build. Registration happens at startup;
hot gameplay paths should compile down to efficient native data/systems without a WASM boundary or
per-object trait-object tax.

## Third-party

Planned sandboxed WASM with:

- versioned semantic API;
- capability-based host access;
- no arbitrary filesystem/process/network access by default;
- handles/IDs instead of internal pointers;
- bulk reads/writes/events where possible;
- quotas/limits;
- explicit client/server/bot targeting.

A normal gameplay feature should usually be expressible as a module. If it requires Beta-specific
code in `engine-core`, first ask whether a generic platform capability is missing.

Renderer backend, allocator implementation, storage backend and transport internals are
infrastructure, not mods.

## Local developer Rhai

DX1 Rhai is an explicit local tooling adapter over semantic control, with capabilities,
resource limits and controlled script roots. It exposes no raw filesystem/network/process APIs.
It does not replace native first-party systems or the future WASM untrusted-mod runtime.

## Target shared platform contracts (C2/BG1/VS1)

Native and future WASM adapters reuse the composed definitions, tags, typed properties, supported
capabilities, handlers and compact state schemas in [CONTENT_SYSTEM](CONTENT_SYSTEM.md), semantic
model/shape contracts and [VOXEL_SPACES](VOXEL_SPACES.md). WASM does not get a second block/model/space
language. Runtime representations may differ; no stable ABI or completed WASM API is promised today.
Event/query context produces bounded commands for authoritative mutation, never raw mutable World.

Downloaded executable code remains sandboxed, capability-limited and quota-limited, without arbitrary
native-library autoexec or ambient host access. A content interface capability does not itself grant
host security authority. Trusted local Rhai remains an explicitly authorized tooling adapter, not the
untrusted mod runtime. Server roles and gameplay modes cannot bypass these boundaries.

## Current C2 native contract

Composed ContentDefinition, voxel/item specialization, typed mass/friction parameters, indexed
classification/contract membership and compact definition-local state are implemented in Game API.
Native use handlers receive controlled immutable context and emit validated commands. The independent
sandbox registers reactor/charge content through these APIs; no Minecraft or renderer dependency is
required by authoring. Item handlers, entity/fluid specialization and downloaded execution remain future
work. The future WASM adapter must preserve this semantic command boundary and add execution quotas;
C2 does not implement a sandbox, stable ABI or complete mod SDK.
