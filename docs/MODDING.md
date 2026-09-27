# Modding

First-party gameplay modules and third-party mods share semantic concepts but do not need the same
execution mechanism.

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
