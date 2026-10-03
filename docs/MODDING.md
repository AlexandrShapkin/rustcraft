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
