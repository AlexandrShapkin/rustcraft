# S1 persistence decision (D-051)

Status: accepted — Ubuntu/Windows implementation CI 37271791735 passed; final closeout publication requires both CI jobs green.

## Context and decision criteria

M4 logical durability is accepted. S1 evaluates its physical operating envelope before M5;
it does not equate scalability with database adoption. Priority is recoverable acknowledged
state first, then sustainable save/oldest-dirty behavior, payload rewrite amplification, read/open
cost, file/index size, churn/recovery, platform support, operational burden and migration cost.
No arbitrary numerical score or universal fsync latency target is used.

## Alternatives screened after current baseline

| Model | Evidence/consequence | Disposition |
|---|---|---|
| Current column files | Stable post-churn size, no startup global index, ~24–37 KiB entity-payload rewrite; straightforward isolated damage/recovery. | Retention candidate. |
| Region/container | Reduces file count, but does not alone fix mutable entity placement; needs allocation/index publication, fragmentation and compaction. Current open has no enumeration. | No spike without measured file/open blocker. |
| Append log + index | Cheap small append, but historical writes require bounded compaction/index/recovery scan and backup protocol. | Not prototyped; adds recovery/compaction mechanism without established need. |
| Embedded transactional KV | Multi-record atomicity possible, but existing transfers/receipts already have accepted ordered recovery. Adds dependency/cache/compaction/export/migration burden. | No speculative DB dependency. |
| Split terrain/entity blobs | Directly removes terrain encode/compression from entity-only writes; same AtomicFile primitive and existing bounded blob codecs; doubles spatial file count and recovery domains. | The sole executable benchmark-only candidate. |

The split candidate does not claim atomic terrain+entity publication. Entity transfer remains
three ordered recoverable writes; a mixed edit needs two independent publications. This is an
explicit consequence rather than a durability shortcut. Each blob receives the same file sync,
replace and platform-directory guarantees. Both models retain old-state/new-state atomic file
publication, checksums, bounds and recovery ordering; checkpoint mechanisms remain unchanged.
No buffered/no-sync comparison or default save routing uses the prototype.

## Logical and physical capability boundary

Runtime layout, persistent disk schema and future network schema are separate. Backend selection
cannot serialize ECS/Vec indices, BlockId, GPU handles or raw Rust structs. Semantic palette/codecs
and opaque components remain logical contracts above replaceable physical storage.

A future worker seam should accept logical column/player/global operations and return an explicit
**DurableAck** carrying domain, revision/generation and durability scope. Scope must distinguish
file-content sync, directory-publication guarantee/unsupported, and any atomic transaction group.
Current column generations are transient request/dirty tokens, not a persisted chunk revision or
network identity. Existing owner serialization and stale-generation acknowledgements remain required.
Errors after publication are ambiguous and retry/idempotency must use revisions. No vague `save`
ack should be mistaken for asynchronous queue admission. Required capabilities: bounded load,
revision-aware store (or explicit owner-side per-domain serialization), ordered/batched publication where requested, explicit enumerate only
for tools/migration, and drain/flush/shutdown. Filesystem roots and chunk paths belong to backend
composition/tools, not game logic. Current `WorldStore` is a partial logical seam but workers still
use concrete `WorldStorage`; mechanically changing every reference is not required by evaluation.

Atomic groups: each column snapshot (voxel/entities/tombstones), each player inventory+receipt
checkpoint, each global component record. Recoverable ordered groups: destination+source+tombstone
prune, player receipt acknowledgement+source entity removal+receipt prune. Independent player/global
checkpoint revisions are not one mandatory world-wide transaction. Consistent directory backup
requires quiescing admissions and draining all domains; copying an active directory is not promised
as a point-in-time snapshot. A transactional backend would need its own snapshot/export contract.

## Future game/mod component contract

Durable envelope: namespaced semantic component key, explicit independently owned schema version,
bounded opaque payload; host validates framing/quota/provider requirements, game/mod owns semantic
codec/migration. Optional unknown components are preserved byte-for-byte; unavailable required
components fail compatibly with context, never silently disappear. Existing player/global envelopes
already demonstrate 48 keys across three namespaces. Known unsupported schemas fail explicitly.
Spatial entity type is required today: unknown type aborts activation, not voxel regeneration.

Choose a future **hybrid logical entity envelope**: required game-owned entity core plus optional
independently versioned extension components. This is a schema/API decision, not an ECS rewrite or
an implementation of a mod SDK. Host must preserve optional unknown bytes during known-core changes.
Aggregate extension/core envelope must remain within the current 4 KiB spatial payload bound until
a separately justified schema change. Initial entity extension count is bounded to 64 (the existing
host component-count precedent) as well as the 4 KiB aggregate; these are future envelope requirements,
not claims of an implemented entity component SDK. Global/player bounds remain 64 components, 32 KiB/component,
64 KiB/record (including framing), semantic ID <=256 bytes. Count and aggregate bounds both matter;
64 maximal components cannot fit one record. Package-declared required keys/versions and optional
providers are validated before activation. Extension code gets no arbitrary paths/filesystem access.
The benchmark does not claim that multiple spatial extension components are already implemented.

Payload migration belongs to its provider; component schema version, physical backend-format version,
world persisted-state schema, generator version and game/content identity are independent. A future
backend switch needs its own explicit metadata identity, not a generator/content hash. No metadata
reservation or format bump is introduced now. Conversion must build a separate destination, preserve
the valid source, validate all logical records and recovery references, publish explicit completion,
then clean source later. No destructive in-place conversion is the only recovery path.

Final decision, limits/triggers and consequences are recorded after benchmark comparison.

## Decision: keep current backend

Accept the current per-column filesystem backend for initial bounded M5. No S1.3 is required. The release evidence covers 10k stored columns/entities, 100 one-percent mixed churn cycles, ordered transfers, checkpoints, one-worker/eight-slot saturation and a six-second 50/s backend source; 50k creation/reopen is supplemental. This is not a guaranteed player-count envelope. The current client admits at most 20 saves/s and may admit fewer; server composition must keep its dirty production below its measured admission/drain capacity and show oldest-dirty convergence.

Reconsider when an intended workload exceeds measured coverage, sustained dirty age/queue grows after producers settle, shutdown drain violates that host's operational budget, entity-only churn makes the measured whole-column amplification unacceptable, or measured directory/read/backup cost at a larger store becomes an actual blocker. Test the actual caller and payload mix before assigning a universal threshold. Exceeding caller admission first requires evaluating scheduling; it does not alone prove physical migration necessary.

Reject immediate split migration: about 116x lower sparse entity bytes is valuable, but mixed churn is 36% slower, files double, full reads and conversion cost increase, and recovery domains multiply. Region/container has no demonstrated file-count/open bottleneck here; append-log/KV adds index, recovery, compaction and migration obligations without a measured mandatory transaction blocker. No database dependency is added. Extensions require a stable namespaced/versioned bounded component contract above a replaceable backend, not inherently a database.

Consequences: preserve current formats, ordered recovery and inspectable files; retain scalar diagnostics and repeatable benchmarks. Backups require quiesced admissions plus drained durability domains. Larger/high-frequency entity workloads must be evaluated explicitly. Physical backend identity remains separate from generator/game/component versions. Implementation and final public CI are recorded in S1_REPORT.md.
