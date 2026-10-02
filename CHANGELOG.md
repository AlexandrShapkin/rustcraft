# Changelog

This changelog starts with the first formal pre-1.0 version. It does not reconstruct a release
history from milestone commits.

## Unreleased

Infrastructure and M4 work in progress; not a published release.

## 0.1.0-alpha.1 — first pre-1.0 alpha (prepared, unpublished)

### Added

- Generic Rust voxel engine/runtime and public Game API foundation.
- First-party `minecraft-b173` package plus an independent `sandbox-test` game proof.
- Semantic resource compilation, multi-page atlas rendering, chunk meshing/culling, and asynchronous
  remeshing foundations.
- Deterministic generated worlds, versioned semantic chunk persistence, componentized player state,
  and crash-recoverable local player checkpoints.
- Release identity, dual-license metadata and reproducible local release packaging.

### Known issues

- M4 remains active; this alpha does not represent completion of the intended world-generation and
  persistence scope.
- General world travel/loading/eviction is incomplete, and Minecraft-like generation remains
  incomplete relative to the intended Beta-recognizable scope.
- Advanced entity and world-time persistence are not complete.
- Networking is not a complete public multiplayer experience.
- The first-party client requires compatible user-supplied Minecraft terrain assets; original Beta
  assets are not redistributed.

### Compatibility

- No stable API, save, network, or package compatibility is promised before 1.0.
- This version is prepared but has not been tagged or published; Minecraft Beta assets are not
  redistributed and the first-party client requires user-supplied compatible assets.
