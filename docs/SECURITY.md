# Security baseline

Security matters because servers can request content and third-party code may eventually execute.

Baseline rules:

- never auto-execute native binaries downloaded from a server;
- verify immutable content by strong hashes;
- validate manifests and dependencies before activation;
- sandbox third-party executable modules;
- grant explicit capabilities rather than ambient host access;
- keep credentials and tokens out of the repository;
- treat malformed network/content input as hostile;
- avoid unsafe Rust unless a concrete need and documented invariant justify it.

A future trust/signing policy can build on these boundaries without changing the core content model.

## Current authority and target F1/A1/M5 separation

Current Control Context carries source labels and explicit capabilities for trusted local composition;
its serialization and ServerAdmin label are not authentication. Local trusted developer grants,
authenticated server grants, game-owned player state and sandboxed downloaded-mod grants are separate.
Same executable does not mean same authority: F1's one client path keeps Rhai opt-in/authorization,
controlled roots and quotas. A role grants capabilities; mechanisms check capabilities, not role names.

A1 defines provenance/grant adapters; M5 authenticates principals and lets the host/server assign roles
and capabilities. Gameplay survival/creative/spectator never confers server.admin or script.local.
Downloaded code remains sandboxed and quota-limited; no source label, CLI flag or Rhai context can
manufacture authenticated remote grants. See [NETWORKING](NETWORKING.md) and [MODDING](MODDING.md).
