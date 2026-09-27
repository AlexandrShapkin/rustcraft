# Roadmap

## M0 — foundation / first headless vertical slice

- typed world/chunk/block primitives;
- multi-chunk world container;
- block registry and first-party gameplay registration;
- flat-world module;
- semantic agent/controller intent;
- basic movement/AABB/voxel collision;
- block break/place path;
- initial Bot API observations/actions;
- initial content manifest/package/target/hash model;
- deterministic headless scenario;
- focused contract tests.

## M1 — local playable client

- renderer/window loop;
- input adapter -> agent intent;
- chunk meshing/culling;
- texture/resource loading;
- camera;
- local world streaming;
- basic save/load abstraction.

## M2 — recognisable sandbox

- terrain generation;
- inventory/hotbar;
- item/tool model;
- crafting;
- lighting;
- fluids;
- day/night;
- first meaningful performance baselines.

## M3 — survival/gameplay

- health/damage;
- mobs/drops;
- farming/furnaces;
- simple redstone-like automation;
- weather where appropriate.

## M4 — multiplayer + server content resolution

- authoritative server;
- QUIC evaluation/transport;
- snapshots/deltas/interest management;
- prediction/reconciliation;
- manifest handshake;
- content cache/fetch/integrity;
- headless remote bot transport.

## M5 — third-party modding

- WASM runtime;
- capability API;
- stable versioning;
- example mod;
- target-specific content;
- quotas/profiling.

## M6 — evidence-driven optimization campaign

Evaluate renderer paths, data layouts, storage engines, SIMD, io_uring, allocators, PGO/BOLT,
large-view-distance LOD and high-player-count partitioning only against representative benchmarks.
