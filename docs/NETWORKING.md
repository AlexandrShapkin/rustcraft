# Networking direction

Java protocol compatibility is not required.

The long-term leading transport candidate is QUIC, subject to measurement and library maturity.
The protocol should distinguish durable/state-sensitive data from supersedable real-time state.

Likely reliable/state-sensitive examples:

- inventory/crafting transactions;
- commands/chat where delivery matters;
- authoritative gameplay transitions;
- content manifests and package metadata.

Likely replaceable examples:

- position/orientation snapshots;
- transient animation/effects.

QUIC streams are reliable; unreliable semantics require an appropriate datagram mechanism rather
than treating a reliable stream as unreliable.

Design eventually for batching, interest management, compact typed messages, deltas where useful,
buffer reuse, client prediction/reconciliation and authoritative server state.

Do not implement world sharding before the single-process server and protocol have real workloads.

## Current boundary and M5 entry

Current server is a headless bootstrap/scenario/test composition, not an implemented authenticated
multiplayer service. No SpaceId or space-aware protocol exists. M5 stays inactive until RF1 and VS1
are complete and READY1 passes, then requires separate activation. Its first real protocol design must
use [voxel spaces](VOXEL_SPACES.md), not freeze today's single-grid assumption. QUIC remains an
evidence-driven leading candidate; no transport library or detailed packets are chosen here.

Target durable/semantic spatial contracts include VoxelSpaceId, parent/reference-space identity where
required, transforms, transform/motion revisions, local voxel changes, entity reference-frame state
and structure lifecycle events. Unchanged content plus changed transform sends transform state, not
N block-coordinate updates. Space-local content deltas and supersedable motion snapshots retain distinct
ordering/acknowledgement requirements; disk, runtime and wire representations are separate.

Interest includes nearby/visible/relevant spaces, their relevant local sections and moving structures
crossing interest boundaries, not merely root-world chunk radius. Prediction/reconciliation understands
reference frames and authoritative discontinuities. A1 prohibits exporting BlockId, ItemId,
TextureHandle, ModelHandle or dense registry indices without an authoritative versioned profile mapping;
semantic identities and stable EntityId/space references define external contracts.

Authenticated principal → host/server role assignment → capabilities → authorized commands/actions.
Source labels, CLI flags, local Rhai Context and game-owned gamemode are not authenticated authority.
Mechanisms check grants, not role names. A1 prepares provenance contracts; M5 supplies authentication
and authoritative grants. Unified graphical startup never grants server administration implicitly.
