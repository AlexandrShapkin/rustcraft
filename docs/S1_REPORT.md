# S1 — Persistence architecture evaluation and scalability

Starting public main: `e07d8bd4110a972588c2ceafe55df400521d72cb` (P1 closeout).
P1 implementation/closeout CI 37256205733 / 37256646904: Ubuntu and Windows verified green.
S1 is ACTIVE. Production backend and format identities remain unchanged.

## Physical model verified from source

`world::WorldStorage` uses `<saves_root>/<validated_world_name>/`:

| Object | Physical ownership / framing |
|---|---|
| `world.rcw` | World format v1, metadata v2: seed, semantic game/generator identity/version, profile fingerprint, persisted state schema v1; BLAKE3 checksum. |
| `chunks/<signed_x>.<signed_z>.rcc` | One flat directory; chunk v3 (v2 readable); coordinate/header, semantic palette per section, voxel palette indices, spatial entities and recovery tombstones. Entire column uses zlib-fast when smaller, otherwise raw. BLAKE3 covers framing and stored payload. |
| `players/<validated_id>.{0,1}.rcp` | Player format v2, sorted semantic component ID/schema/opaque bytes, revision, checksum. Alternate the slot other than newest valid; legacy single `.rcp` v1 remains readable. |
| `world-state.{0,1}.rcs` | Global format v1, revision and sorted bounded versioned components, checksum, alternate valid slots. |
| `.atomicwrite*/tmpfile.tmp` | Short-lived temporary directory/file on the destination filesystem, removed by AtomicFile on normal success/error. An actual process crash may leave an orphan; startup ignores it. No normal operation requires enumeration. |

For a fresh store with N saved columns and P players with both slots populated: N+1+2P+2 committed files,
`chunks` plus optional `players` directories. No per-column directories, entity sidecars or global
in-memory column index. Retained legacy player files and crash-orphan temp objects add to the
fresh-store formula; no silent cleanup migration is added. Spatial tombstones live inside destination columns; pickup receipts live
inside the player's game component. Light is derived/rebuilt, **not persisted**; a storage workload
cannot count light bytes as application IO. Runtime arrays and GPU handles never enter the codec.

Column/metadata writes: one write_all to AtomicFile's temp file; AtomicFile file sync, atomic replace,
then its directory syncs on Unix; RustCraft additionally syncs the destination directory. Ordinary
column writes therefore invoke one file sync and three Unix directory syncs (temp and destination
parents differ). Player/global checkpoint helper also explicitly syncs inside the callback, then
AtomicFile syncs again: two file syncs and three Unix directory syncs per checkpoint, plus first
player-directory publication. These pre-existing sequences were measured, not optimized away.
On Windows AtomicFile uses replace-existing/write-through MoveFileExW; RustCraft directory sync is
unsupported/no-op. No Windows directory-entry power-loss guarantee is claimed. On any platform,
an error after publication can mean the new valid value is present although acknowledgement failed.

The client uses one column-save worker with eight queued jobs; completed replies also have an
8-entry channel. Player/global workers each coalesce one newest pending snapshot behind an inflight
write. Exact dirty generation must be acknowledged before eviction; a newer mutation or failure
stays dirty. `flush()` syncs supported directories but does not drain workers; composition must
finish pending work first. Transfer/pickup ordering remains accepted M4 behavior.

## Instrumentation and workloads

`world` adds phase metrics and fixed atomic totals shared by storage clones. Measured reads separate
file IO, decompression and semantic decoding. Save completions include queue wait; scheduler totals
and max remain fixed scalars. Dirty timestamps exist only for currently dirty columns and retire
on the matching successful generation. Oldest age is scanned only by the demanded Persistence
provider. The existing shared snapshot supplies DUX, Control, Rhai and failure/capture consumers.
There is no parallel profiler or retained payload history.

AtomicFile hides its internal individual sync/rename timers. `durability_envelope` includes temp
creation, internal file/directory sync, replacement, extra directory sync and cleanup; it is **not
isolated fsync time**. A separate 128-operation 4 KiB append/sync calibration reports actual file
sync duration on the same filesystem. Application write bytes are not device/NAND writes; stored
bytes exclude filesystem block allocation and metadata. Counters cover successful column writes
since storage open, not failed attempts/player/global records. Concurrent totals are approximate
scalar observations, not an atomic transaction snapshot.

Benchmark-only `world/examples/s1.rs` uses project-owned deterministic data: eight sections,
layered strata/air/caves, nine semantic palette keys, varied variants/compressibility, 96-byte
opaque entities. Creation fixture construction is outside individual store-operation timing but
included in creation wall time. No terrain generation or renderer participates. Initial 10k store
contains 10k entities; a separate dense column exercises 1,000 entities. Three namespaces and
48 independently versioned components with 64–256-byte payloads exercise global/player envelopes.
Unknown components remain byte-preserved while known data changes.

Phases: fresh creation; 1% deterministic small voxel changes; 1% one-byte entity payload changes;
100 dense entity changes; destination-before-source transfer and tombstone pruning; 128 revisions
each of player/global checkpoint; 100 churn cycles modifying 1% of columns; 2,000 seeded random
and 1,000 sequential reads; new storage and separate process reopen; saturated finite bounded worker
drain; 50 dirty columns/s paced source; graceful drain/flush. Logical-change denominators are
changed **payload bytes** (voxel variant 2, entity payload 1), excluding revision/framing; transfer
normalization is 256 logical bytes across three publications. They are not estimates of NAND
amplification. A tiny edit can legitimately require a large recoverable snapshot.

`support::Samples` is benchmark-only, with finite operation counts from bounded tier arguments;
production retains only fixed totals. Result JSON contains aggregates, never per-operation arrays.
Each benchmark generates a unique disposable directory alongside its result and removes it on
success. Standard 10,000/100 is explicit release acceptance, not CI. CI runs 16-column/two-cycle
correctness plus deterministic fault tests on both platforms. `just s1-bench` sequentially compares
current and isolated split blobs under identical data and durability; `just s1-test` is the focused
gate. Candidate selection follows the completed current baseline.

## Host and cache assumptions

Linux 7.2.7-zen1-1-zen, x86_64; AMD Ryzen 5 PRO 2500U, 8 logical CPUs; Rust 1.98.0;
release optimization. Store/results reside on ext4 `/dev/nvme0n1p2`, Intel SSDPEKKF256G8L NVMe,
not `/tmp` (tmpfs). This is one laptop/filesystem, not a universal server performance target.
Fresh-directory writes; warm sequential/random reads; process reopen retains OS cache. No root,
cache drop, destructive device test or purported cold-device latency. Existing C1 effective defaults
are embedded in the current result; save workers are composition policy, not a new env flag.
The main client has world/player cadence 2,000 ms. Benchmark source rate/size are explicit diagnostic
arguments, independent of operational config, and never change normal autosave scheduling.

## Current baseline and comparison

The first completed current-only 10k/100 baseline predates any executable candidate. Creation
133.397 s including fixture construction; successful store operations 95.291 s. Encode excluding
compression 38.720 s; compression 7.786 s; write_all 0.494 s; durability envelope 47.916 s.
Sparse/dense one-byte entity changes issue 23,622 / 37,215 application bytes per changed byte.
100 churn cycles retain 10,005 files; store size 235,620,779 → 235,623,855 bytes, a 3,076-byte delta,
not growth proportional to historical writes. Random reads p50/p95/p99 1.516/2.486/3.593 ms;
storage reopen 0.927 ms. Exploratory two-worker/four-slot drain: 111.56 writes/s, 1,000 completions,
zero remainder, acknowledgement p50/p95/p99 68.491/94.746/112.504 ms. This exploratory pool is
explicitly distinguished from the final production-equivalent one-worker/eight-slot measurement.
The baseline is retained as bounded canonical aggregates in S1_RESULTS, not guessed after a fix.

Matched final results, large tier, candidate decision and final acceptance follow below.

## Concrete coupling inventory

Source search: `rg -n WorldStorage crates --glob '*.rs'`, grouped by callable ownership below.
All current concrete-use locations are covered by these groups; line numbers may move as counters
are added. `WorldStore` appears as the world trait and filesystem implementation, not as the worker
job type. No production generic runtime/game crate takes a WorldStorage dependency.

| Location | Concrete use | Classification / future seam |
|---|---|---|
| world::LoadJob / LoadScheduler::request | Clone storage into async load/semantic decode. | Convenience/migration blocker for substitution; backend read plus codec resolver capability. |
| world::SaveJob / SaveScheduler::submit | Clone storage into bounded column-save worker; measured write ack. | Convenience/migration blocker; logical revision request + durability ack. |
| world::PlayerSaveJob / PlayerSaveScheduler::submit | Two-slot checkpoint worker. | Current concrete checkpoint strategy; future player-record capability must retain revision/idempotency/coalescing. |
| world::WorldStateSaveJob / WorldStateSaveScheduler::submit | Global checkpoint worker. | Same global-record capability; no broad simulation rewrite. |
| world::WorldStorage and impl WorldStore | Root/path, validation, framing/codec, replacement and directory sync. | Necessary backend operations; extract codec and capability only when implementing another production backend. |
| client::App and StartupWorker world_storage fields, initialize_world | Filesystem composition/open, metadata, migration compatibility, player/global load and bootstrap columns. | Composition roots legitimately choose backend; existing startup helper methods are concrete convenience coupling. |
| client startup encode_runtime_chunk and persisted_runtime_column | Semantic snapshot conversion uses associated static helpers. | Codec convenience, not a requirement for filesystem paths; future codec module seam. |
| client/configuration tests and client main tests | Disposable stores, player/global reopen/flush, v1/v2 compatibility. | Filesystem-specific test fixtures, preserve as original-backend regression. |
| server run_world_info / run_chunk_info | Open and inspect paths/metadata/file sizes. | Necessary filesystem inspection/tooling; a future generic inspector should request backend capabilities. |
| server persisted_column / store_simulation_column / activate_stored_column | Codecs and writes used by headless acceptance. | Tooling convenience; same logical request/codec seam. |
| server roundtrip/entity/persistence/stream benches | Fresh/reopen deterministic physical stores and fixtures. | Benchmark/tooling only; retain for current backend. |
| world unit tests; S1 examples/support/tests | Direct primitive/path inspection and disposable storage. | Intentionally filesystem-specific evidence, not game API. |

No mechanical trait-object conversion was made. A new production backend would require changing
four worker job seams and composition roots, not granting mods paths or routing filesystem IO into
simulation. Server dependency checks retain the graphics-free normal graph.

## Fault and recovery matrix

| Interruption / failure | Expected and tested result |
|---|---|
| Before temp write | AtomicFile callback returns error; prior file preserved. |
| Partial temp write | Prior file preserved; temporary directory cleaned on controlled error. |
| Complete write before sync | Prior file preserved on callback error. |
| Explicit temp-file sync before replace | Prior file preserved on callback error. |
| After publication / before caller acknowledgement | New valid file may already be present; retries must be idempotent. No claim of rollback after rename. |
| Incomplete newer checkpoint slot | Highest valid older revision selected, recovery flag true; unknown components preserved on next rewrite. |
| Destination durable, source stale | Both blobs reopen; highest entity revision wins; destination tombstone suppresses stale source (both activation orders covered by runtime regression). |
| Source cleaned, tombstone not yet pruned | Exactly one newest entity remains; marker is safe/recoverable. |
| Marker pruned after source ack | Single destination owner remains. |
| Player receipt ack before source removal; after source/prune | Existing game/runtime pickup recovery tests prove idempotent inventory/no duplicate pickup and bounded pruning; world-state roundtrip exercises disk reopen. |
| Failed/newer-generation save ack | Dirty ownership and first-dirty age retained until matching success. |
| Corrupt candidate conversion destination | Checksum rejects it; separate source remains byte/logically valid. |

Controlled failure tests exercise the actual AtomicFile callback/format helpers and storage reopen;
they are not random kill races or a power-cut certification. Internal rename-before-directory-sync
failure is not separately hookable inside atomicwrites; its implementation syncs both parents on
Unix. RustCraft's extra directory-sync failure is an ambiguous post-publication failure. Platform
CI proves API/format behavior; hardware/controller power-loss guarantees remain outside this test.

The backend queue probes use the actual production worker configuration, but a benchmark owner
admits independently of rendering/ticks. The current client `fixed_step -> service_world_saves`
admits at most one column per 50 ms tick, and selects the first dirty entry; an already-saving first
entry can defer other admission. Therefore **20 column admissions/s is an upper bound of the current
client caller**, and actual drain may be lower. The 50/s physical probe is not an end-to-end client
throughput promise. Server/M5 composition must judge admitted workload and oldest dirty against
its own caller budget. This caller policy is inventoried, not changed into an unrelated scheduling
rewrite or used as evidence that a database migration is required.

## Matched optimized comparison

All times below are application measurements on the documented ext4/NVMe host, with warm OS caches; neither NAND writes nor universal server limits.

| Workload | Current | Isolated split prototype |
|---|---:|---:|
| 10k creation wall seconds | 125.683 | 173.116 |
| Sparse entity application bytes / changed byte | 23,622 | 204 |
| Dense entity application bytes / changed byte | 37,215 | 5,236 |
| Transfer application bytes / normalized changed byte | 275.9 | 2.57 |
| Small voxel edit amplification | 11,811 | 11,701 |
| 100 mixed churn cycles, store-operation seconds | 89.449 | 121.632 |
| Spatial file count | 10,000 | 20,000 |
| Full column random read p50/p95/p99 ms | 1.523/2.371/2.903 | 2.020/2.971/4.956 |

Current creation store operations total 89.882 s. During mixed churn encoding excluding compression costs 37.137 s, compression 5.638 s, and the atomic durability envelope 46.131 s. Durability and encoding dominate elapsed work; whole-column entity placement dominates semantic write amplification. `write_all` alone is small. These are separate findings, not a claim that every fsync costs the entire publication envelope.

Current random-edit publication p50/p95/p99: 17.079/20.743/21.998 ms; sparse entity 16.852/21.160/23.514; dense entity 19.502/23.072/29.476; transfer publications 15.844/20.275/21.693. Each transfer has three ordered publications. The marker denominator is the actual 32-byte encoding (not an earlier draft's 36); measured IO/times are unchanged. Player/global 256 checkpoint publications issue 2,334,976 bytes, latency 10.903/13.038/15.120 ms, with 512 file and 768 Unix directory sync calls.

Current stored bytes across churn: 235,620,779 to 235,623,855; split: 235,425,288 to 235,426,030. Neither replacement model accumulates historical writes or requires compaction. Split mixed operations publish two files, so its per-publication timing must not be compared as an entire current operation. No global application index exists in either model.

## Scale, queue, recovery and instrumentation

The required 10k-column/10k-entity/100-cycle release workload completed. The 50k diagnostic creation completed in 897.827 s under concurrent validation load. Its tail child launch failed after another build replaced the running executable; argv-based subprocess launch was corrected and a separate fresh process reopened the completed store (metadata 2.414 ms; first 100 reads 5.347/7.654/11.316 ms). Only saved large creation and separate reopen aggregates are accepted; lost tail aggregates are not reconstructed. Exact 1k/10k/50k file/byte measurements are in S1_RESULTS.json. No 100k run is claimed.

One-worker/eight-slot saturated 1,000-save drain: 57.43 writes/s, queued+inflight maximum nine, zero remainder. Queue wait p50/p95/p99 134.423/156.451/222.495 ms. A separate paced 50/s, 300-save backend source peaks at five dirty records and drains to zero at every 50-completion checkpoint: dirty acknowledgement 15.458/47.179/76.236 ms; queue wait .035/22.402/50.463 ms. This is six seconds of backend evidence, not the current client's admission rate or a multi-hour server certification. Client admission is capped at one per fixed tick and can be lower.

Isolated 4 KiB file-sync calibration: .934/1.385/2.464 ms. Unchanged-directory calibration is not the cost of syncing a newly renamed entry. Graceful flush after explicit drain costs .188 ms; drain time is reported separately. Worker Drop cancels rather than promising to finish queued saves: composition must drain completions before dropping workers. The auxiliary test explicitly drains eight accepted snapshots and verifies all on reopen.

100 entity create/delete cycles restore all 16 original column bytes, file count and store size. Twenty corrupted-newer checkpoint recovery cycles preserve unknown components. The 48-component/three-namespace fixture stays within existing host limits. Existing world-state acceptance covers durable entity transfer, pickup receipts, eviction/reload and pruning. No format/version/hash changes were made.

Instrumentation uses seven fixed atomic totals and scalar dirty timestamps tied to current dirty ownership. In a hot batch, counter read is approximately 12.2 ns; demanded oldest-dirty scan over 121 entries p50/p95/p99 .005238/.005308/.010616 ms. Inactive diagnostics do not scan dirty entries. These diagnostic microbenchmarks are not universal thresholds. Shared Persistence text prioritizes pressure/age and bounded phase totals; Control/Rhai/captures read the same provider. The graphical smoke uses llvmpipe/OpenGL only for UI correctness.

The split prototype's 1k conversion costs 13.868 s plus 6.014 s full validation, approximately 47.1 MB combined source/destination temporary footprint. Corrupt destination rejection leaves source valid. There is no production switch or migration tooling. Region/log/KV candidates were screened but not executable-spiked because current read/file/open/queue behavior establishes no corresponding mandatory blocker.

## Decision and acceptance

**Keep the current physical backend for bounded initial M5; no S1.3 is required.** The canonical ADR defines the measured envelope, caller limits, future component/capability contract and reconsideration triggers. PM5-004 closure awaits public Ubuntu/Windows acceptance. No unlimited world size, player count, sustained entity churn rate or Windows performance equivalence is claimed.

Local acceptance passed formatter, all-target check, workspace tests, strict all-feature Clippy, just ci/dx-test/dx-console/sample-game, S1 correctness/examples, world/player/global/entity/pickup/transfer/corruption/migration/generator persistence gates, C1 smoke and focused RSM1/P1 regressions. Full graphical residency/presentation campaigns are unnecessary because their ownership and presentation code was not changed. Server normal dependency graph remains graphics-free. Machete is clean; audit reports no known vulnerabilities and three existing allowed unmaintained warnings (paste, smartstring, ttf-parser). No new third-party package was added.

Known limitations: controlled faults are not hardware power-cut certification; Windows directory sync differs; benchmarks are fresh-directory/process-reopen/warm-cache host evidence; application write bytes exclude device-level amplification; schema/security quotas are not casual C1 operational knobs. Required spatial extension components are a future contract, not an implemented mod SDK. RSM1/P1/UX1/C1/DUX1 remain closed. R2/A1/DX2/READY1/M5/M6/M7 were not started; no version, tag or release was created.

Graphical Persistence smoke: `target/test-runs/scenario/17-1791180525250074072/result.json` status pass; inspected `target/captures/s1_persistence-17-1791180526005762555/frame.png`. Evidence is generated/ignored, not committed. File scaling after initial creation: 1k/10k/50k columns produce 1,001/10,001/50,001 files including metadata, 23,557,549/235,578,880/1,177,904,255 stored bytes. Warm listing of the completed 50k chunk directory took 157.79 ms; startup does not perform that enumeration.

Supplemental comparison: split isolated file-sync p50/p95/p99 1.024/1.367/1.536 ms, using the same 4 KiB calibration, not internal per-column fsync attribution. Auxiliary drain+flush of eight accepted saves took 56.766 ms with four queued and one inflight at the observed start; all eight reopened. Existing generated-v2 persistence bench (four columns) encoded 264,873 raw / 9,379 stored bytes (ratio .035), whereas the varied synthetic pressure corpus is roughly .35. Thus the amplification figures are payload-specific, not claimed exact compression behavior for every production world.
