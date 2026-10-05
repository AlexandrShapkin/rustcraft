# Post-DX1 / pre-M5 consolidation audit

Audited 2026-10-04 against public main **16c6823a739e83f830b6676bbe0390be769ca17e**
(`Record DX1 autonomous acceptance`), immediately after
**008fbac252e4307af5146cda8e033eee3079bcc3**. This is an architecture/source audit and an
accepted execution plan, not implementation or newly measured hardware acceptance.

## DUX1 execution update

The original findings/counts below describe the audited baseline and remain historical evidence.
DUX1 implementation starts from the accepted planning commit `195f53ca3025c9f001f0338d7a51a6e6c6626a80`.
Generic native metadata/selection, shared demand/cadence and targeted inspection now replace the
PM5-007 hard-coded discovery/all-domain refresh path. DUX1 is CLOSED; PM5-007 is resolved.
Local and Ubuntu/Windows public CI acceptance is recorded in
[DUX1_REPORT.md](DUX1_REPORT.md); C1 is CLOSED; every later stage remains inactive. New debug contracts use
ChunkPos/section coordinates and stable EntityId; PM5-001 remains an A1 blocker for older contracts.
No residency lifetime, pacing, storage or legacy-resource cleanup is part of DUX1.

## C1 execution update

C1 is CLOSED from accepted DUX1 closeout f8fb177. Typed operational policy, source precedence,
transactional boundary acknowledgement and shared adapters replace PM5-008's fragmented migrated
knobs. [CONFIGURATION.md](CONFIGURATION.md) inventories every remaining owner/boundary; closure
and measurements are in [C1_REPORT.md](C1_REPORT.md). Original audit findings/counts remain historical.
RSM1 and later stages remain inactive; no lifetime/presentation/storage/API cleanup is folded in.

## Baseline, method and limits

GitHub commit API and a fresh HTTPS origin clone agreed on main. `git status --short` and
`git diff --check` were clean in that clone; the five-commit history includes both expected DX1
commits. [Closeout CI](https://github.com/AlexandrShapkin/rustcraft/actions/runs/37157185442)
passed `rust (ubuntu-latest)` (111302875956) and `rust (windows-latest)` (111302875826).
The original mounted checkout is at 4831787 with substantial local modifications and a read-only
Git arrangement; it was preserved. Audit/publication work uses an isolated writable clone.

M0–M4 remain complete, DX1 **CLOSED**, M5/M6/M7 inactive. M4-009 representative-hardware
quantitative evidence is conditional; that is not functional incompleteness. DX1 and M4 reports
are historical validation evidence, not tests rerun by this documentation pass.

Read README, repository instructions, all required architecture/product/contracts/workflow documents,
DECISIONS, DEFECTS, PERFORMANCE, TOOLING, SCRIPTING, DEBUGGING, DX1_REPORT, CONTENT_SYSTEM and the
canonical `docs/ROADMAP.md`. There was no root ROADMAP; a root navigation document now links to the
canonical plan. Inspected all 19 workspace manifests, Cargo metadata/normal dependency trees,
CodeGraph structure/dependencies, source paths below, justfile, scripts, both workflows and ignores.
CodeGraph needed a fresh index; structural queries worked before optional embeddings finished.
Cargo metadata/source are authoritative for crate edges; graph call counts are navigation, not profiling.

`just refs-status` reported optional historical source/assets and all external clones missing in the
isolated checkout. No historical behavior was inferred or imported. This audit evaluates current
resource code, not Beta pixel fidelity. Future observable implementation still requires the bounded
reference-study gate. No proprietary pixels, saves, captures, indexes or generated reports are committed.

No new route, timing, RAM or VRAM measurement was performed. Owner reports motivate investigations;
source observations establish risks, not their measured cause. No documentation/link recipe exists.
Documentation validation is diff/relative-link checking; expensive bootstrap/M4 matrices are reserved
for future implementation and public CI still runs its unchanged normal matrix.

## Risk register

Severity concerns entering M5, not emergency status of the accepted local product:
**BLOCKER** closes before M5; **HIGH** multiplies multiplayer cost if carried forward;
**MEDIUM** valuable but can be waived with recorded evidence; **LOW** cleanup/watchlist.
Disposition A = mandatory before M5, B = worthwhile consolidation, C = defer, D = resolved/obsolete.
Counts here cover unique audit findings, not every historical defect: **1 BLOCKER, 5 HIGH,
5 MEDIUM, 2 LOW**. No active corruption or remotely exploitable defect was established.

| ID | Severity / disposition | Evidence and concrete risk | Closure owner |
| --- | --- | --- | --- |
| PM5-001 | BLOCKER / A | `agent-api::PlaceIntent.block: BlockId`, `BotAction.intent`, and public runtime state cannot become durable/remote contracts unchanged. Bot nearby-item observations lack EntityId. Profile-local numbers or list positions would lock replication to one process registry. | A1.1 identity/consumer contracts |
| PM5-002 | HIGH / A | `MeshScheduler.generations` keeps removed section keys; its result channel is unbounded by channel capacity; GPU removal exists but no repeated-route lifetime ledger proves all ownership converges. Multiple client interests would magnify retention/backpressure mistakes. | RSM1; resolved (see execution update) |
| PM5-003 | HIGH / A | `camera_for` reads fixed-tick player position/yaw/pitch; `Simulation::step` applies look. FIFO rendering alone cannot smooth duplicate authoritative visual states. Prediction would otherwise be layered onto an undefined presentation clock. | P1; resolved (see execution update) |
| PM5-004 | RESOLVED / S1 | Whole-column amplification measured; current backend accepted for bounded initial M5 with explicit caller limits, reconsideration triggers and component/durability seam. See S1_REPORT and S1_PERSISTENCE_DECISION. No production migration required. | S1 CLOSED |
| PM5-005 | HIGH / A | runtime survival/inventory/recipes and mod-api hardness/tool/drop definitions remain Minecraft policy; sample-game bypasses mixed runtime. Replicating those universal-looking fields would cement the wrong game boundary. | A1.2 affected policy migration |
| PM5-006 | HIGH / A | `render::hud::gui_rect` divides historical GUI coordinates by 256; whole HUD/inventory/player sheets remain named roles with internal layout assumptions. Current block crops are already semantic. Content resolution must not equate sheet geometry with resource identity. | R2 active resource roles |
| PM5-007 | MEDIUM / B | Debug names are hard-coded PAGES/OVERLAYS, selected through console/shortcuts; no discoverable selector or selected-chunk query. Collection runs all DX domains every 250 ms even when views are inactive. | DUX1 |
| PM5-008 | MEDIUM / B | CLI, environment and constants split budgets, radii, worker counts and checkpoints; no shared validation/source/change metadata. Server tuning would multiply adapter-specific flags. | C1 |
| PM5-009 | MEDIUM / B | Specialist CLI/just diagnostics coexist with DX scenarios. Their equivalence has not been demonstrated; wholesale deletion would lose correctness coverage. | DX2 inventory/equivalence |
| PM5-010 | MEDIUM / B | CPU CI is strong, graphical/travel/hardware gates separate; existing four-column persistence and one-route travel evidence do not cover large-store churn, repeated lifetime plateaus or display cadence. | subsystem stages + READY1 baselines |
| PM5-011 | MEDIUM / B | Context exposes source and capabilities and developer(ServerAdmin) can grant local developer capabilities; FutureChat factory is read-only. No network caller exists. Reusing a deserialized Context as authentication would confuse provenance/grants. | A1.1 trust adapter contract |
| PM5-012 | LOW / C | Generated outputs normally use ignored target; root perf.data/flamegraph/capture names are not all covered. No tracked generated leak established. | DX2 path policy, no local deletion |
| PM5-013 | LOW / C | Current audit has zero known vulnerabilities, three unmaintained transitive warnings; duplicate platform versions/default graphics features need a real size/support reason to change. | dependency watchlist |

## Actual ownership and dependency map

The crate graph is acyclic for normal dependencies. Composition is not pure just because Cargo
edges are acyclic: `minecraft-b173 -> runtime -> mod-api` still delegates game policy to transitional
owners, and the non-Minecraft proof avoids runtime. There is no generic engine Cargo edge to
minecraft-b173. Native game code remains native; no downloaded native library mechanism exists.

In the table, H = hot path, P = persistence, N = future network relevance. These describe contract
impact, not permission to export native structs as schemas. All dependencies are inward toward
mechanisms; platform composition supplies selected games. Third-party dependency names are omitted
except where containment matters.

| Crate | Responsibility and internal normal dependencies | Public/generic ownership; H/P/N |
| --- | --- | --- |
| engine-core | World/section/state storage, geometry, ray/collision, typed IDs; none | Generic mechanisms. Dense BlockId/ItemId local; EntityId stable. H high; P source state; N identity conversion. |
| content | Semantic names/package targets, canonical hashes, texture discovery/compiler/cache; none | Generic authoring/resolution. H startup; P content compatibility; N manifests, verified assets. PNG dependency is also present headlessly, without GPU. |
| game-api | Voxel definitions, compiled profiles, native schedules, controlled commands; content/core | Generic extension contract. H indexed definitions/command boundary; P semantic resolver; N profile mappings. Native SetBlock BlockState is local, not wire data. |
| mod-api | Legacy block/item registry, hardness/tools/drops; core | Transitional policy-rich compatibility, not universal mod SDK. H gameplay; P legacy resolver; N must not freeze numeric/schema layout. |
| agent-api | Controller/AgentIntent, movement/look/actions; core | Generic input plus legacy game conveniences. H every tick; P transient; N needs explicit semantic adapter for placement. |
| bot-api | v2 semantic observations, BotAction wrapping intent, ScriptedBot; agent/core | Legitimate headless observe/act surface, with Minecraft convenience fields. H bounded observation; P semantic references only; N EntityId/placement gaps. |
| control | Commands, capability/source context, immutable Snapshot, scenario/fixed controls/jobs; agent | Generic developer/admin mechanism, serde JSON values. H optional tools; P diagnostic only; N authenticated adapter required, not wire-ready simply because serializable. |
| scripting-rhai | Trusted local bounded interpreter/sessions, compiler worker, scenario/artifact lifecycle; control | Tool adapter, not gameplay or untrusted sandbox. PNG encoding here supports evidence. H opted-in service; P no game durability; N no remote autoexec. |
| runtime | Simulation, movement, lighting, dirties, spatial recovery; agent/bot/content/core/game-api/mod-api | Mixed owner: reusable mechanisms plus survival/inventory/recipes/items. H high; P dirty/receipts/transfers; N authority/replication danger until affected policy moves. |
| world | Residency/lifecycle, generation/load/save pools, semantic codecs and filesystem backend; core | Generic P owner. H publication/dirty queues; P high; N backend independence/recovery. No render/game dependency. |
| render | RenderWorld snapshots, meshes/jobs, GPU/surface, HUD/offscreen; core | Generic geometry/GPU plus Beta HUD/skin construction debt. wgpu/winit contained here and client. H high; P none; N presentation handles must stay local. |
| render-profile | Texture/voxel render compilation; content/core/game-api/render | Presentation bridge. H indexed resolver; P none; N semantic resources resolve locally, no atlas-page network identity. |
| gameplay-blocks | Legacy first-party block/item definitions and atlas map; core/mod-api | Game-specific, reexported through minecraft-b173. H indexed policy; P semantic legacy adapter; N compatibility only. |
| gameplay-flat-world | Authored flat generator; core/mod-api/world | Game-specific diagnostic terrain. H generation; P generator contract; N no protocol role. |
| minecraft-b173 | Profile, first-party commands, exact v1/v2 worldgen, player/item/clock codecs; agent/content/control/core/game-api/gameplay-*/mod-api/runtime/world | Game policy, currently uses runtime compatibility paths. H native game systems; P owns semantic payloads; N selected game extensions, no private engine shortcut for new work. scripting-rhai is dev-only here. |
| sandbox-test | Independent generated-texture game/Control proof; agent/content/control/core/game-api/render/render-profile/scripting-rhai/world | Architectural integration executable, no Minecraft/runtime dependency. H test only; P generic fixtures; N alternate-game contract proof. |
| client | winit input/event/render composition, streaming/save coordination, diagnostics, specialist modes; core APIs plus minecraft/runtime/mod-api/render/render-profile/world/control/Rhai/content/build-info/agent | Platform root with first-party UI/controller debt. H frame loop; P orchestration; N future client adapter, not authority. |
| server | Headless CLI/composition, regression workloads/scenarios; agent/bot/content/control/core/game-api/minecraft/mod-api/runtime/world/Rhai/build-info | Headless root, no render/wgpu/winit edge. H simulation; P diagnostics; N not yet authoritative multiplayer server. |
| build-info | Build/version metadata; none | Generic build support. H negligible; P none; N product version is not protocol version. |

Convenience coupling worth narrowing: worker requests and client fields use concrete WorldStorage;
render-profile links to render for descriptors (acceptable presentation bridge, never server dependency);
scripting bundles pull PNG into server (no reason to split just for purity). Control and Game API
commands are intentionally distinct: developer validation/grants vs native extension execution.
Bot stepping and scenario stepping share semantic intent/fixed gates but serve different consumers.
Do not merge all APIs or create a general ECS to remove their different shapes.

### Existing ARCH findings rechecked

ARCH-001 runtime policy, ARCH-003 legacy controller/Bot conveniences, ARCH-004 legacy definitions,
and R1.0-001 dual numeric registries are still current: A1 closes the network-touched portions before
M5; unaffected compatibility helpers may remain isolated. ARCH-002 is current for HUD/skin layout:
R2 moves active source-sheet dependence/game construction; a general UI toolkit is deferred.
R1.0-002 single-page renderer and R1.0-004 file-path slots are resolved in R1.1; the old audit's
one-page/temporary-client-atlas action is obsolete. R1.0-005 resource overrides are resolved;
block overrides remain intentionally absent. R1.0-003 block-local variant schema waits for a real
new state consumer, with current u16 schema explicit. R1.1-001 mip/AF/compression, R1.2 transparency
and R1.1-002 adapter-before-compiler redesign remain C. M4 streaming/persistence closures remain
closed, not reopened by scalability questions. Historical decision descriptions remain history;
D-045 and this document supersede stale next-step guidance.

## Source evidence navigation

Paths/symbols below refer to the audited baseline; use symbols rather than assuming line numbers
survive future migrations.

| Domain | Source anchors |
| --- | --- |
| Platform cadence/input | `crates/client/src/main.rs`: LocalHumanController::next_intent, ClientApp::fixed_step/about_to_wait/render, camera_for; `crates/runtime/src/metrics.rs`: FixedStepClock; `runtime/src/lib.rs`: Simulation::step |
| Surface/GPU lifetime | `crates/render/src/lib.rs`: Renderer initialization, render_capture, upload_chunk_pages, remove_section/remove_chunk, buffer_capacity_action; `client/src/gpu_metrics.rs`: provider scope |
| Snapshot/mesh lifetime | `render/src/lib.rs`: RenderWorld::copy_chunk; `render/src/meshing.rs`: MeshScheduler::mark_dirty/remove_section/poll/take_ready; `client/src/main.rs`: service_world_residency/process_mesh_jobs |
| World/save ownership | `world/src/lib.rs`: WorldResidency::evicted, PersistenceDirtyTracker, WorldStore/WorldStorage, SaveScheduler/PlayerSaveScheduler/WorldStateSaveScheduler; `runtime/src/lib.rs`: entity recovery/evict_entity_column |
| Durable game codecs | `minecraft-b173/src/player_persistence.rs`: encode_revision/decode_with_unknown; `minecraft-b173/src/world_persistence.rs`: entity codec/encode_world_state/decode_world_state |
| Shared tools and trust | `control/src/lib.rs`: Context/Action/Host/Snapshot/PAGES/OVERLAYS/FixedControl; `client/src/devtools.rs`: service_devtools; `minecraft-b173/src/control.rs`: MinecraftHost; `scripting-rhai/src/lib.rs`: RhaiRuntime/DevTools |
| Resource boundary | `minecraft-b173/src/lib.rs`: legacy_resource_package; `content/src/resources.rs`: ResourcePackage/AtlasPolicy/compiler; `render-profile/src/lib.rs`: CompiledTextureRegistry/CompiledVoxelRenderRegistry; `render/src/hud.rs`: gui_rect/skin_rect |
| Consumer compatibility | `agent-api/src/lib.rs`: PlaceIntent/AgentIntent; `bot-api/src/lib.rs`: BotAction/ItemEntityObservation; `mod-api/src/lib.rs`: BlockDefinition; `runtime/src/inventory.rs` and `survival.rs` |
| Workflows | `justfile`, `scripts/check.sh`, `scripts/release.py`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `server/tests/dx1.rs`, `.gitignore` |

## Frame pacing and presentation evidence

`client::about_to_wait` advances FixedStepClock, services DX, takes permitted fixed ticks, services
streaming and unconditionally requests redraw. RedrawRequested invokes render. No explicit normal
frame limiter/sleep or explicit ControlFlow selection was found; sleeps located in startup/shutdown
and specialist/headless loops are not proof of double limiting during normal play. No
`pre_present_notify` or monitor-refresh query was found. Investigate platform redraw coordination,
especially Wayland, before prescribing a scheduler. No current evidence establishes compositor fault.

`LocalHumanController` accumulates mouse deltas until next_intent; runtime applies yaw/pitch in
Simulation::step. `camera_for` copies current authoritative position plus eye offset and current
angles; there is no previous/current interpolation pair. Player translation, look and extracted
entity positions can repeat across renders until the next 50 ms tick. This is concrete 20 Hz visual
state stepping, even when production/presentation throughput is much higher. FixedStepClock bounds
catch-up and drops excess full ticks rather than changing dt; P1 must preserve authority and exact
pause/step behavior.

Renderer chooses supported FIFO and surface desired_maximum_frame_latency 2, then calls
SurfaceTexture::present after queue submission. Render/present CPU phase lumps acquisition,
encoding/submission and presentation work; GPU timestamp queries measure passes, not scanout.
Rolling average FPS and 1% low are useful throughput summaries but cannot isolate uneven cadence,
duplicate states or input latency. Refresh misses and uneven present intervals are plausible under
event-thread stalls/acquisition backpressure, not established by this static audit.

Version-specific primary references:
[wgpu 26 PresentMode](https://docs.rs/wgpu/26.0.1/wgpu/enum.PresentMode.html),
[SurfaceTexture](https://docs.rs/wgpu/26.0.1/wgpu/struct.SurfaceTexture.html), and
[winit 0.30 Window](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html#method.pre_present_notify).
FIFO queues presentation and can block acquisition. Calling present is an application event, not
proof of physical scanout. Report separate redraw-request/callback, acquisition begin/end, CPU render,
queue submission and present-call timestamps; do not rename any of these display timestamps.
Physical display timing requires a separately supported platform/provider or external measurement.

## Residency and lifetime evidence

Ownership: WorldResidency desired/retained request entries -> published World columns/sections ->
Simulation lighting/items/recovery -> RenderWorld owned 18³ voxel/light vectors -> cloned MeshJob
snapshots -> completed CPU PageMeshes -> render-thread GpuChunk page buffers -> wgpu submissions/
driver. Shared Arc owns worker receiver/resolver and window/surface, not every resident chunk.

Client save-before-evict removes simulation columns/light state and entity ownership, clears several
latency/readiness maps, advances mesh removal generations, removes GPU sections, and dirties
presentation so absent World sections remove RenderWorld entries. Removal has explicit code paths;
no evidence supports declaring all old chunks permanently GPU-resident. Snapshot removal is staged,
so temporary overlap is expected. `Renderer::remove_section/remove_chunk` drops page buffer owners;
queued GPU work may still hold resources. Buffers reuse geometric capacity on remesh and shrink below
quarter utilization; no global free GPU mesh pool was found. Atlas/depth/HUD/capture resources have
separate lifetime from streamed chunks.

Concrete concern: MeshScheduler::remove_section increments but never deletes `generations`.
Unique long-distance exploration grows historical metadata even if live meshes evict. Do not simply
clear it: an old result arriving after remove/reinsert must never match a recycled generation.
Pending jobs coalesce per section; submitted work finishes and stale replies are rejected at poll
and upload. The result mpsc channel is unbounded by its own capacity, ready is a Vec, and generation/
pending admission is not a whole-pipeline byte quota. Supported resident/inflight behavior limits
ordinary use but does not prove pressure bounds during stalled consumption or changing interests.
Measure and close admission/result/metadata lifetime together, including late jobs on revisit.

Lighting pins only active work while obsolete queued work cancels; persistence pins exact dirty
save generations. Inspect failure pins explicitly: unsaved data must not be discarded to force a
memory plateau. Runtime durable entity/tombstone maps and diagnostics deserve separate accounting.
Some visited-column sets exist only in stream_perf; do not confuse intentional bounded-run evidence
with normal ownership. Metrics histories and DX events/jobs/output are bounded; map capacity and
Vec high-water allocation are distinct from logical entries. Whole-device DRM VRAM is not process
VRAM. Driver/allocator high-water retention remains a valid explanation until logical counts and
bytes, RAM and provider scope are correlated.

## Persistence and mod scalability evidence

Accepted format: world.rcw metadata v2; one signed-coordinate chunks/x.z.rcc column, payload v3
with semantic section palettes and u16 variants, optional whole-payload zlib-fast compression,
checksums/strict lengths; payload v2 reads with zero spatial entities. Each spatial record has
EntityId, revision, semantic entity_type, schema_version and bounded opaque payload; tombstones
name source column. Column dirty/save generations reject stale acknowledgements. Load/generate/save
queues are bounded; missing files may generate, corrupt/incompatible files never regenerate silently.

Player v2 and world-state v1 have independently versioned semantic components, two slots, revision/
checksum fallback, unknown opaque component preservation, coalesced newest checkpoints, background
IO and graceful flush. Player defaults to two-second checkpoints with env clamp 1–2 seconds; world
also defaults two seconds. Motion checkpoints, destination-before-source transfer/tombstone cleanup
and player pickup receipts establish idempotent ordered recovery, not a multi-file transaction.
Writes sync completed data and supported directories. Unix directory sync and Windows file/
replace-write-through guarantees differ; never promise power-loss durability beyond platform support.

Scalability risks: a tiny entity change can serialize/compress/write unchanged voxels; cross-column
motion adds ordered saves; one-file-per-column has filesystem object/open cost; one global/player
component change rewrites its envelope. Existing four-column persistence and entity workload reports
prove function and diagnostic cost, not millions of objects or sustained server churn. Current
sync metrics are aggregate checkpoint phases, not a full fsync distribution/write-amplification ledger.

WorldStore exists, but load/save/player/world-state worker requests and client orchestration use
concrete WorldStorage and its static runtime codecs. Game payload codecs do not interpret physical
filenames; the leak is mostly infrastructure/composition, not Minecraft knowing every path. S1 must
separate logical conversion, backend capabilities, durability acknowledgements and physical storage
before a migration is justified. Runtime layout != disk schema != network schema. No ECS dump.

Player/global envelopes already approach namespace + semantic component key + schema version +
bounded payload. Entity payloads have one semantic type/version rather than independently addressed
component namespaces. Unknown spatial entity types/versions fail contextually; they are not presently
opaque inactive entities. Evaluate safe preservation without activation, required-vs-optional
components, quotas and migration routing. Unknown data must not execute or silently disappear;
mods should use host-owned durable contracts, not unrestricted filesystem ownership. No complete
mod persistence framework or distributed storage is required now.

| Candidate physical model | Benefit to test | Cost/risk to test |
| --- | --- | --- |
| Existing files/records | Simple independent random column reads and atomic replacement; accepted recovery | Full-column amplification, many files/opens/syncs; may remain best for current scale |
| Region/container | Fewer filesystem objects, localized lookup | Allocation/index growth, partial writes, locking, recovery and small-update amplification |
| Append/log | Small ordered writes, recovery history | Replay/startup, indexes, tombstone retention, compaction and bounded tail |
| Transactional embedded KV/database | Indexed components/entities and possible cross-domain transactions | Dependency/runtime cost, page/WAL amplification, sync policy, backup/migration/compaction |
| Hybrid | Separate cold voxels and hot entity/components | More ownership/atomicity boundaries; transfers must remain idempotent |

Do not choose from this table by fashion. S1.1 benchmarks current backend and bounded candidate
spikes with equivalent durability; S1.2 records an ADR accepting current backend or a narrowly
specified migration. A migration is a separately bounded follow-up, never an automatic consequence
of running S1.1. A keep-current decision with measured limits and triggers satisfies evaluation.

## Human diagnostics, configuration and resources

### What DX1 already exposes

Human, in explicit --devtools: backquote console, help/commands and Tab discovery, slash/Rhai queries,
pause/step/resume, script/scenario/reload/capture, F10 abort, page and overlay commands. F3 overview
also exists outside devtools. There is no keyboard page/overlay cycle or discoverable selector;
selection uses console commands. Useful pages: overview, streaming, world, entities (eight per page from first 64
active entries), lighting, renderer, persistence, scripts. Four overlays: streaming residency subset,
entity markers/owner columns, player collision and target. OFF/ON/OFF already works by commands;
DUX1 must retain and test it rather than claim it is absent.

Automation/query rather than dedicated pages: meshing JSON, scenario/job details nested in scripts,
failure bundles/recent events, cooperative compile/capture/job waits and structured results. These
are often also callable manually through Rhai/console: not strictly automation-only. CLI/specialist
modes provide resource reports/atlas inspection, world-info/inspect-chunk, render-scale/offscreen,
worldgen maps/reports, persistence/entity benchmarks and stream/travel acceptance. Native file
command discovery/metadata changes require restart. There is no shared in-game selected chunk
inspection or cost/availability/owner registry. Entity lookup searches the capped snapshot subset,
not arbitrary stable-ID world access. An off-subset miss must not mean the entity does not exist.

`control::Snapshot` (not a new DebugSnapshot type) is the shared immutable source. Client populates
DX domains every 250 ms independent of active page, and MinecraftHost builds semantic player/
27-block/entity values. Disabled tooling returns early; enabled inactive tools still refresh all
these domains. DX1 measured inactive mean 208.52 us versus disabled 4.31 us on its recorded setup;
this audit does not rerun or promote those values to universal budgets. Registry metadata belongs
in generic tooling; package views contribute semantic data through controlled queries, never a
second human-only backend. Existing CommandSpec registration is a useful model; PAGES/OVERLAYS
constants and client switches are not future registries.

DUX1 should be its own small early stage, not wait for a full C1 or become a polished DX2 editor.
Pages/overlays need selectors and collection demand, not a general mutable settings system.
C1 later supplies settings to the same developer surface and Control/Rhai consumers.

| Proposed human view | Useful bounded content / decision |
| --- | --- |
| Overview | Active page/overlays, tick/frame summary and unavailable providers |
| Streaming | Player chunk, Desired/Retained/Safe/Visible, load/generate/light/mesh queues, frontier blockers |
| World | Generator ID/version, resident sections, selected semantic voxel/chunk |
| Entities | Stable EntityId/type, position/velocity, owner, lifecycle/revision/persistence status; bounded paging/filter |
| Lighting | Initial/boundary/incremental state and blocked frontier, no all-voxel scan |
| Meshing | Pending/inflight/ready, stale/failure counters, bytes and selected-section generation |
| Renderer | Live mesh objects/logical/capacity bytes, submissions, adapter/present policy; scanout unknown explicit |
| Persistence | Dirty/pending/inflight, oldest dirty age, revisions, failures and transfer blockers |
| Scripts | Loaded identity/generation, limits and recent bounded errors |
| Scenarios / Jobs | Small summaries/tabs using scripts/job snapshot; no new scheduler or duplicate enumeration |

Separate pages for every domain are not mandatory if a bounded combined view communicates them.
Prioritize Streaming, Meshing/Renderer, Persistence, Entities and Scripts/Jobs for remaining audits.

Overlay priorities: existing streaming/Desired/Retained/Safe/Visible-unready subset; chunk boundaries;
player collision; selected entity AABB and owner column; raycast/target; selected-section mesh/light
frontier state only if bounded/cost justified. Full-world lighting heatmaps are deferred. Boxes use
transient depth-tested presentation, not world state or rebuilt chunk data. Show legend and active
set; budget geometry and collect only requested domains.

Inspection selects ChunkPos/SectionPos from target or explicit coordinate and EntityId from pick/
bounded list or explicit ID. Queries return resident/interest/safety/visibility, light/mesh stage/
generation, entity count, dirty/save pins and relevant jobs with unavailable/truncated/stale metadata.
Selected entity adds semantic type, transform/velocity, owner, age/lifecycle and durable revision.
Never expose raw ECS index or mutable references. Answer why invisible/retained/stale using reasons
from owners, not UI heuristics. Targeted entity query must distinguish absent, unloaded and outside
snapshot coverage. An arbitrary entity database is not required.

Cost policy: inactive view -> no expensive scan; active demand -> bounded queries at measured cadence;
cheap counters may remain shared. Metadata records semantic ID/title/description/owner/availability,
default shortcut, Low/Medium/High cost and enabled state. Shortcuts are defaults, not identity.
Use measured cadence (often a few Hz), collection duration/staleness/list caps, coalesce automation
and UI demand, and test inactive/inactive-with-console/active/overlay/scenario/capture separately.
No unbounded entity/chunk serialization per frame. Disabled normal startup remains unchanged.

### Configuration fragmentation

Client::new/env parsing owns stream radius (3–12), retain=load+1, lookahead, compute worker allocation,
mesh upload count/bytes, lighting units, main streaming soft budget (0.25–8 ms), saves/resource paths,
atlas maximum, autosave and diagnostics/measurement windows. PresentMode and surface latency are
renderer initialization policy. ResourceLimits/AtlasPolicy and Rhai operation/deadline/queue/source
caps have separate owners; scenario time/run/step caps are constants. Some JSON reports literal
limits rather than registry-resolved settings. Environment remains startup input, not current-value
introspection or a safe runtime application contract.

C1 should introduce one generic typed registry with semantic key/type/default/current/validation/
owner/description/source/persistence policy/change policy. Adapters (CLI/env/config, Control/Rhai,
in-game developer settings) resolve through it. Explicit classes: Immediate, NextTick, NextFrame,
Reconfigure, RestartRequired, ImmutableAfterOpen. Not everything must be live: generator/schema/
registry compilation/world dimensions are open/restart-bound; fundamental GPU representation is
structural. Streaming radii, diagnostics, overlays, budgets, frame/presentation policy and autosave
may apply at safe boundaries; worker counts require draining/reconfiguration or restart. Failed
changes preserve prior value; requested and effective values differ during reconfiguration.
No script-only registry, hidden unrestricted file settings or automatic durable world changes.

### Legacy resource normalization

minecraft-b173::legacy_resource_package imports semantic terrain face crops, water and ten crack
stages plus whole inventory/HUD/player sheets; generic content compiles arbitrary cropped PNGs into
pages, render-profile resolves TextureKey -> AtlasRegion/TextureHandle. Blocks already have this
boundary and multipage synthetic tests. Do not reimplement R1.1.

Remaining active dependence is internal HUD hotbar/selector/inventory subrectangles and player skin
layout built in generic render/client; wrapping a whole sheet in a semantic name does not normalize
its subresources. Source-region sampling and authored display geometry must be separated. Current
importer does not establish complete historical item-sheet/particles/font/entity-texture support:
non-block sprite coverage is limited and text uses project-authored glyphs. Keep these unsupported
classes deferred, not imaginary migration obligations. R2 normalizes only implemented roles and
retains the permanent game-owned legacy importer. Runtime pages/handles are local compilation
outputs, never persistent/network content keys.

## Tooling, tests, dependencies and trust

| Workflow class | Current examples | Retirement rule |
| --- | --- | --- |
| Stable human | client/client-survival/dev-client, help, world-info, resource-report/inspect, version/release tooling, refs | Keep named workflows; reduce aliases only with documented compatibility |
| CI/regression | fmt/check/test/test-std/lint/ci, smoke/survival/sample-game, world/player/entity roundtrip, v1/v2 locks, dx-test/shipped scripts | Keep assertions/hashes and platform coverage; scenarios may replace drivers only with equivalence |
| Specialist diagnostics/baselines | bench-m2/m3, render-scale/camera-motion/offscreen/fidelity, worldgen/report/map, persistence/entity/stream/travel, DX overhead | Valuable distinct workloads, not dead because DX1 exists |
| Candidate migration compatibility | old fixed-step/capture drivers, client-diag/capture/client-bench and duplicate source parsing | Inventory exact callers/outputs/failure behavior first; no proven dead recipe in this audit |
| Wrappers | scripts/check.sh -> just ci; doctor/references/release/resource fixture generator | Lightweight stable convenience; no deletion benefit established |

Just is a Bash command surface; Windows public CI directly calls Cargo, so green CI does not prove
all just recipes portable. DX shipped-script server integration runs on both platforms. Release
allowlists authored scripts/docs and prohibits generated/proprietary material. Preserve it.
No deny.toml exists; cargo-deny is not an established policy gate. Ignore policy is coherent for target,
saves/reference clones/caches/__pycache__/.code-graph; profile artifacts written at root need a
path policy. Do not remove developer-local files or broadly ignore authored reports.

Audit commands: cargo metadata --no-deps; cargo tree --duplicates; normal server tree; cargo machete
(clean); cargo audit with a writable /tmp database (default advisory lock was read-only, retried).
RustSec database ef6173cbc5c50ec8166f9a5b28f07834144373ee, updated 2026-10-03: zero known
vulnerabilities; paste 1.0.15 RUSTSEC-2024-0436, smartstring 1.0.1 RUSTSEC-2026-0249,
ttf-parser 0.25.1 RUSTSEC-2026-0192 are unmaintained warnings, not known vulnerabilities.
Rhai stays pinned 1.26.1 with sync in scripting-rhai; not a game/system interpreter. Normal server
contains Rhai/PNG but no wgpu/winit. Duplicated bitflags/rustix and platform Windows dependencies
come from different upstream major/platform paths; deduplication alone is not leverage. Default
wgpu/winit features support current platform backends; trim only after size/startup/support evidence.
No Cargo/lockfile changes made.

Tests: core/registry/math/codec/lighting/survival unit contracts; server shipped-script/actual-CLI
integration; same-source headless and graphical DX scenarios; independent sandbox dependency guard;
M3 offscreen geometry/resource fixtures; frozen M4 hashes/order/seams/recovery/travel; release archives;
CPU workload diagnostics; optional representative hardware. Timeouts bound hanging work, but
wall-clock streaming/script deadlines and software-GPU routes vary with host load. Preserve semantic
assertions; do not harden llvmpipe timings into platform acceptance. A timeout under a heavily loaded host is not
necessarily an algorithm regression. Record timeout/queue/service state on failure.

Future hierarchy: focused touched-crate checks -> subsystem contract gates -> headless/graphical
scenario gates -> full regression -> Ubuntu/Windows CI -> optional representative-hardware
performance. Each stage below states its gates. Expensive route/resource/archive matrices run for
relevant changes/batch closure, not every text edit. Preserve M4 frozen canonical hashes and
correctness/frontier/recovery assertions. Needed baselines before M5: fixed tick cost, streaming
request-to-visible/queues, worldgen versions, persistence amplification/tail latency, residency RAM/
logical GPU plateau, mesh scheduling, cold/warm resource compile, presentation cadence and DX cost.
Tag workload/seed/config/build/hardware/provider/warmup/sample count. Correctness bounds, diagnostic
baselines and hardware performance acceptance are different evidence classes.

Trust: Rhai is trusted local tooling with capped source, canonical roots, no raw filesystem/network/
process/environment host functions, disabled imports/dynamic eval, operation/depth/data/deadline
limits and bounded native calls. Console/scenario privileges require explicit opt-in; scenario lease
returns intent on terminal state. Worker publication validates generation; queues/events/jobs are
bounded. FutureChat factory denies developer escalation, but serializable Context is an internal
trusted object, not an authenticated identity. ServerAdmin is a source label, not network auth.
A1.1 defines host-granted capabilities, source provenance and denial tests before M5 adapters;
remote admin implementation is out of scope. Downloaded/world-local scripts never auto-run; untrusted
executable mods remain future sandboxed WASM. No full sandbox claim for local Rhai.

## Accepted sequence and bounded execution contracts

Recommended order: **DUX1 -> C1 -> RSM1 -> P1 -> S1 -> R2 -> A1 -> DX2 -> READY1 -> M5**.
All new implementation stages are planned/inactive at this audit closeout. Each future pass activates
only its named sub-slice and stops at its acceptance boundary. Do not automatically run the chain.

DUX1 first makes existing diagnostics discoverable and demand-driven, helping every later pass.
C1 comes next because its small generic registry and safe budgets/radius/cadence controls eliminate
ad-hoc experimental flags in RSM1/P1/S1; structural/worker reconfiguration need not be first slice.
RSM1 precedes timing experiments to avoid changing residency/memory workload underneath them.
P1 establishes presentation/authoritative clock separation before prediction. S1 must decide backend
limits/recovery/component direction before network ownership commitments. R2 and A1 can investigate
independently after diagnostics; R2 removes resource-layout coupling before A1 freezes consumer
identities. DX2 retires only demonstrated replacements after APIs settle. READY1 integrates evidence.
A1.1's semantic/provenance rule applies immediately to new contracts throughout earlier stages;
its executable consolidation gate follows R2. No hard dependency requires a database or a new GUI.

Common invariants/gates for all stages: engine mechanism/game policy separation; headless server;
semantic persistent/external identities and profile-local hot handles; bounded asynchronous ownership;
no gameplay change without reference study; no proprietary fixtures; targeted fmt/check/tests/clippy
then relevant subsystem gates, complete batch regression and unchanged Ubuntu/Windows CI. Full
regression is selected by impact, not every historical benchmark on every slice.

### DUX1 — In-game developer diagnostics

**Goal / why before M5:** let a human diagnose visibility, pins and jobs without source/terminal
specialist workflows; use the accepted DX1 backend to accelerate subsequent consolidation.
**Dependencies:** DX1 closed. No C1 dependency for basic selection/demand.
**In scope:** DUX1.1 generic metadata/registration, discoverable selector, active page/overlay labels,
help and measured demand/cadence; DUX1.2 targeted chunk/entity inspection and graphical scenarios.
Native packages can register views without generic Minecraft branches; use existing Snapshot/Control
queries and optional fields rather than a second scanner.
**Out of scope / deferred:** polished player settings/editor, profiler GUI, GUI toolkit rewrite,
remote admin and mod UI framework.
**Invariants:** opt-in devtools; F3/console preserved; views read-only; overlays transient; absent
providers and truncated coverage explicit; shortcuts not registry identity.
**Evidence:** active/inactive service duration, collection counts/caps/staleness, repeated toggles,
real graphical opening/selection/input-focus/capture and independent game registration.
**Acceptance:** useful listed domains accessible without restart (combined Scenarios/Jobs allowed);
existing overlays and justified additions OFF->ON->OFF repeat cleanly; selected chunk explains
light/mesh/save/residency blockers; EntityId inspection gives owner/type/durability or contextual
coverage error. Inactive pages cause no expensive query/scan; automation/UI reuse collected values.
Release/non-devtools startup and same-source snapshots unchanged; cost metadata/help visible.
**Regression gates:** control/client/Rhai contracts, dx-test/dx-console/dx-overhead, shared headless/
graphical smoke, sample-game and renderer line/HUD tests. No per-frame unbounded lists.

### C1 — Runtime configuration plane

**Goal / why before M5:** one auditable settings source and safe application boundaries for later
experiments and server/client operational policy.
**Dependencies:** DUX1 selection surface; settings registry remains UI-independent.
**In scope:** C1.1 registry/typed values/validation/source precedence/readback and a bounded set of
existing radii/budgets/diagnostic/autosave controls; C1.2 Control/Rhai and developer settings adapter,
then only demonstrated presentation/worker reconfiguration. Inventory remaining flags with explicit
migration/restart status, not a requirement to support every benchmark flag live.
**Out of scope / deferred:** general end-user settings, arbitrary mod settings execution, world/schema/
generator/compiled identity hot reload, automatic worker resizing without drain protocol.
**Invariants:** key/type/default/current/range/owner/description/source/persistence/change metadata;
all consumers use same registry; structural values immutable after open or restart-bound.
**Evidence:** source precedence and invalid-input matrix, apply boundary/reconfigure failure results,
requested/effective state, inactive cost and workload behavior before/after changing values.
**Acceptance:** invalid values preserve current state; deterministic Immediate/NextTick/NextFrame
application; explicit Reconfigure/RestartRequired/ImmutableAfterOpen errors; env/CLI compatibility
mapped and documented; radius changes preserve safe frontier/save pins, autosave preserves durability;
Control/console/UI read identical effective settings. No uncontrolled new operational flags.
**Regression gates:** registry/control tests, dx smoke, radius boundary/stream/travel and checkpoint
recovery tests for changed values, server headless tree; relevant graphical setting scenario.

### RSM1 — Residency and memory lifetime

**Goal / why before M5:** prove ownership/accounting is bounded before multiplying interests/entities.
**Dependencies:** DUX1 counters/inspection and C1 controlled workload settings.
**In scope:** RSM1.1 lifetime ledger and repeated route harness; RSM1.2 only measured retention/
backpressure fixes, including safe generation-history retirement and stale reinsertion handling.
**Out of scope / deferred:** GPU pools, custom allocator, renderer rewrite, exact driver VRAM shrink,
M7 optimization and discarding unsaved data to meet a bound.
**Invariants:** save-before-evict, late jobs never resurrect removed meshes, pending/ready/snapshot
ownership has explicit admission/retirement; current buffer capacity reuse is legitimate.
**Evidence:** repeated A->B->C->D->A after warmup, multiple repetitions plus unique exploration and
stalled-consumer/cancel/revisit variants. Log world columns, simulation sections/entities, render
sections/snapshots, pending/inflight/ready meshes/bytes, generation metadata, live GPU objects,
logical GPU bytes, buffer capacity/high-water, RAM and driver VRAM with provider scope/N/A.
**Acceptance:** identical bounded-residency routes converge to a documented plateau; logical live
counts do not grow monotonically; metadata scales with current/unfinished work rather than all travel;
queue pressure has explicit bounds/progress. Pins and transient overlaps are explained. Allocator/
driver capacity may plateau above startup; no requirement to return driver VRAM exactly to baseline.
Measure same-config limits after warmup before selecting quantitative tolerances.
**Regression gates:** remove/reinsert late-result tests, eviction/light/entity/pickup recovery,
world-stream-bench/world-travel-test, render-camera-motion and repeated graphical route; DX soak.

### P1 — Frame pacing and presentation

**Goal / why before M5:** establish smooth presentation/input clocks independent of authoritative
20 TPS before prediction/reconciliation inherits camera stepping.
**Dependencies:** C1 policy readback and RSM1 stable workload; DUX1 timing display.
**In scope:** P1.1 timestamp ledger/refresh discovery/provider limits and diagnosis; P1.2 smallest
supported scheduling/interpolation/camera change justified by stationary, moving, mouse-look and
streaming experiments. Evaluate winit redraw/pre-present coordination and supported present modes,
without assuming a mode switch alone fixes the observation.
**Out of scope / deferred:** networking/prediction, renderer rewrite, M7, physical scanout claims
without a provider, arbitrary uncapped FPS target.
**Invariants:** fixed authoritative dt/exact stepping; presentation is read-only; teleport/pause/
resume/capture discontinuities handled; input consumed once, no double look application/limiting.
**Evidence:** active monitor refresh where discoverable (configured/discovered vs physical cadence
labeled), selected mode/backend/compositor/VRR availability, render/request/present-call intervals,
acquisition waits, p50/p95/p99/jitter and long/missed-refresh-style intervals with threshold method;
state age/duplicates, interpolation and camera/input behavior. Application present timestamp !=
physical scanout. GPU-pass timing != presentation timing.
**Acceptance:** report distributions and observed cause, then demonstrate improvement under matched
workload/refresh settings; authoritative state remains 20 TPS while presentation moves between ticks;
mouse orientation does not wait solely for fixed ticks; no unexplained extra limiter. Monitor changes,
focus/minimize/resize and pause/step recover without backlog. N/A physical timing is explicit.
Hardware-dependent cadence tolerances must be justified from refresh/provider evidence; average FPS
alone never passes. Software-GPU correctness is separate from representative-display acceptance.
**Regression gates:** clock/input/camera/discontinuity tests, exact-step shared DX scenarios,
stream/travel correctness, graphical mouse/motion/resize scenario and representative refresh run.

### S1 — Persistence architecture evaluation and scalability

**Goal / why before M5:** consciously accept backend limits and logical component/recovery boundaries
before network authority/ownership/schema commitments.
**Dependencies:** DUX1 persistence observation, C1 measured save settings; RSM1 steady-state workload.
**In scope:** S1.1 benchmark current model and bounded equivalent-durability candidate spikes;
S1.2 ADR and backend/codec boundary design, independently versioned namespace/key/version/bounded
payload contract evaluation, unknown preservation/required components and migration plan. No
production storage migration bundled into evaluation. If needed, name S1.3 with its own bounded
migration contract before implementation; otherwise accept existing backend and measured triggers.
**Out of scope / deferred:** default database adoption, distributed persistence, full mod system,
reflective ECS serialization, network protocol, unlimited transactional scope.
**Invariants:** runtime != disk != network schema; stable EntityId/semantic palettes; game owns
payload semantics; host owns durability/quotas; recoverable ordering and current compatibility
remain accepted. Backend choice cannot weaken sync guarantees invisibly.
**Evidence:** many stored chunks (including tens of thousands), many active persistent entities,
multiple component namespaces, frequent small edits, region/column crossings, player/world changes,
forced crash/reopen at transfer/pickup/write phases and long churn. Measure logical changed bytes,
physical written bytes/amplification, fsync count/frequency and p50/p95/p99, save queue latency/oldest
dirty age, random chunk and component/entity lookup, startup/open/recovery, filesystem object count,
index memory, compaction if applicable, post-churn storage and migration cost. Separate compression,
encoding, write and sync phases; record filesystem/platform/build/cache/durability assumptions.
**Acceptance:** reproducible workload/report compares current model with justified candidates,
accepts keep-current or names a bounded migration based on measured need; failure-injection reopens
preserve count/identity/receipts and unknown components as specified; document spatial unknown-type
policy, entity component extension and no raw mod filesystem need. Identify concrete WorldStorage
coupling and proposed backend capability/ack boundaries. No universal fast-database claim.
**Regression gates:** semantic codecs/limits/checksums/corruption, v2/v3/player/global migrations,
world-roundtrip/world-state-roundtrip/entity-persistence-bench, ordered cross-column/pickup/crash
recovery, frozen generation hashes and platform sync tests. Evaluation gate can pass without migration.

### R2 — Semantic resource normalization

**Goal / why before M5:** eliminate active source-sheet layout assumptions before content resolution
has to preserve physical legacy coordinates for every client/package.
**Dependencies:** R1.1 already complete; DUX1 inspection; C1 if resource policy settings are exposed.
**In scope:** R2.1 catalog active HUD/inventory/skin subresources and game-authored geometry;
R2.2 importer emits named semantic roles/subresources, generic resource compiler resolves arbitrary
source rectangles/pages, renderer consumes resolved descriptors. Preserve accepted M3 presentation.
**Out of scope / deferred:** all historical assets, particles/font/item sheet features with no current
consumer, proprietary fixtures, mip/AF/compression/bindless or generic UI toolkit.
**Invariants:** user Beta sheets -> game importer -> semantic resources -> generic compiler -> local
TextureHandle/regions -> renderer; source crop belongs in importer; runtime physical page is local.
**Evidence:** synthetic rearranged source sheets/individual files, page changes, resource overrides,
non-Minecraft view, before/after active-role geometry and cold/warm compilation.
**Acceptance:** implemented block/crack/HUD/inventory/player presentation is independent of source
sheet coordinates and packing layout; semantic keys resolve equivalent project-owned fixtures with
changed crops/pages; generic render code has no Beta source-atlas layout contract. Importer remains
supported; geometry/culling/UV and fidelity regressions pass, no proprietary pixels committed.
**Regression gates:** resource compiler/path/bounds/cache/override tests, multipage/offscreen suite,
render-test-all/fidelity-m3, sample-game, resource-stress and targeted HUD/player capture.

### A1 — Network-facing ownership and API consolidation

**Goal / why before M5:** close demonstrated policy/identity hazards without merging consumer APIs
or redesigning the simulation wholesale.
**Dependencies:** S1 decision, R2 active roles; DUX1/Control contracts preserved. Semantic/provenance
rules apply to earlier new work immediately.
**In scope:** A1.1 classify local vs persistent/external contracts; semantic placement adapter/stable
entity observations and typed bounded references; host-granted Source/capability adapter rule/tests.
A1.2 migrate network-touched inventory/mining/drop/pickup/crafting definitions and policy from generic
runtime/mod-api into minecraft-b173 using public extension mechanisms, retaining generic movement/
lighting/world commands/durable entity mechanism. Execute one behavior-preserving domain per pass
with explicit adapters; do not require removal of every legacy helper/file. Extend Game API only for
actual migrated consumers, equally available to sandbox-test/another native game.
**Out of scope / deferred:** M5 packet/transport design, complete ECS or universal command taxonomy,
SDK 1.0 freeze, full module plugin system, unrestricted developer mutations for bots.
**Invariants:** engine makes sense without Minecraft; no engine-to-game Cargo edge/private extension
shortcut; Bot observe/act/step vs Control admin vs Game extension remain legitimate distinct APIs;
dense hot IDs stay local. GPU/pointer/Vec/ECS/atlas identity never external reference.
**Evidence:** before/after dependency and mutation path maps, direct/agent/bot/control equivalence,
registry reorder fixtures, native alternate-game consumption, auth-source denial matrix.
**Acceptance:** PM5-001 closed by explicit stable semantic future-facing contracts and adapters,
not by adding Serialize to AgentIntent; Bot item entity refs use EntityId. Replication-touched game
policy no longer masquerades as engine policy; remaining local compatibility is bounded/documented.
Control snapshot/durable entity semantic type differences are consciously resolved (current item
query uses minecraft_b173:item while persistence uses minecraft_b173:entity/item). Client/server
consume same game rules; semantic actions preserve inventory/count/mining/pickup/recovery semantics.
Untrusted source labels cannot manufacture grants in planned adapter contract; FutureChat denials
persist. No remote auth/admin implementation required.
**Regression gates:** agent/bot/control and public Game API contracts, survival/inventory/mining,
codec/count-conservation/travel/lighting tests, direct/slash/Rhai equivalence, sample-game without
Minecraft/runtime and an alternate consumer of any extracted runtime mechanism, normal server tree.

### DX2 — Repository and legacy workflow retirement

**Goal / why before M5:** reduce redundant entrypoints and clarify stable human/CI workflows after
replacements have proven equivalence, so multiplayer gates remain maintainable.
**Dependencies:** previous stages' interfaces/workloads stable. No prerequisite deletion.
**In scope:** DX2.1 inventory recipes/CLI/scripts/callers/outputs/gates; DX2.2 retire only proven
redundant drivers/aliases, document compatibility and test tiers; generated-output path/ignore policy.
**Out of scope / deferred:** deleting useful specialist diagnostics/frozen hashes, changing milestone
history, aggressive local cleanup, making all tooling cross-platform or replacing just/Cargo.
**Invariants:** stable just workflows, equivalent assertions/failure/artifacts and dependency guards;
release excludes assets/secrets/artifacts; no developer-owned local files removed.
**Evidence:** old/new route equivalence, callers/help/output inventory, timing/cost by tier, tracked/
ignored file check and archive contents. If no replacement is equivalent, retain the old workflow.
**Acceptance:** each deletion has evidence and replacement; no dead classification by name/age;
canonical commands documented, regression hierarchy explicit, docs/recipes/CLI consistent,
generated outputs isolated/ignored precisely and release/script allowlist preserved.
**Regression gates:** affected CLI/scenarios, full regression correctness, script-check/shipped scripts,
release-check, dependency guards and both public CI platforms. No required harness removed silently.

### READY1 — Pre-M5 readiness review

**Goal / why before M5:** integrate closure evidence, measured baselines and any deliberate waivers.
**Dependencies:** mandatory PM5 findings closed; planned MEDIUM work completed or explicit bounded
waiver with rationale/owner/trigger. No waiver of stable external identity or game direction.
**In scope:** final dependency/schema/trust/diagnostics/lifetime/pacing/storage/resources review,
regression and baseline index, focused docs reconciliation; name M5's first bounded slice separately.
**Out of scope / deferred:** transport/protocol implementation, speculative perfection, M6/M7.
**Invariants:** accepted M0–M4/DX1 remain closed; hardware conditional status honestly labeled.
**Evidence / Acceptance:** all checklist items below have linked commits/tests/reports, no unresolved
BLOCKER/HIGH without a concrete corrective closure; medium/low dispositions explicit. Repeat same
workloads with tagged baseline configuration; platform CI green. M5 stays inactive until this gate
is recorded and a separate implementation pass activates it.
**Regression gates:** full functional matrix selected by consolidation impacts, Ubuntu/Windows CI,
representative-hardware optional evidence tracked separately, docs/link/diff checks. READY1 does not
silently reclassify M4-009 hardware evidence as achieved or reopen completed M4 functionality.

## Exact readiness checklist and deliberate deferrals

Enter M5 only after:

- PM5-001–006 mandatory contracts/evidence closed; each MEDIUM finding completed or waived with
  concrete rationale/trigger. No newly exposed corruption/security/architecture blocker left open.
- Generic/game edges intact; minecraft-b173 conceptually removable; an alternate consumer proves
  newly extracted mechanisms. Server remains headless; client/server game authority agrees.
- Persistent/external references semantic/stable; local BlockId/ItemId/texture/GPU/pointer/Vec/ECS
  handles explicitly excluded or translated through a versioned authoritative profile mapping.
- S1 accepts physical backend with measured limits, logical component/unknown-data/migration and
  durability decisions; no implicit runtime/disk/network schema equivalence.
- RSM1 lifetime routes plateau and stale/cancel/revisit ownership is bounded; dirty pins remain safe.
- P1 cadence/state/input behavior understood and validated with labeled timestamp/provider limits;
  representative hardware evidence remains conditional where unavailable, not replaced by high FPS.
- R2 active semantic resources independent of source atlas; future content resolution uses keys/hashes.
- Operational settings have one controlled source/change policy (or explicit limited C1 waiver with
  no uncontrolled new flags); diagnostics usable both automatically and in-game with bounded cost.
- Control provenance/capabilities cannot be confused with network authentication; no untrusted Rhai
  autoexec; future mods remain sandboxed. Planned ServerAdmin adapter grants are host-owned.
- Tests/workflows/docs coherent; correctness locks preserved; pre-network baseline index distinguishes
  diagnostic, correctness and representative-hardware evidence; normal Ubuntu/Windows CI green.

Do not fix pre-M5: hypothetical plugin registries without consumers, general ECS rewrite, renderer
rewrite/GPU pools/greedy/bindless/Hi-Z/LOD, exotic allocator/SIMD/io_uring/PGO, distributed persistence,
full WASM runtime/mod UI, full scripting debugger/state migration, production GUI toolkit/settings
polish, unlimited game commands, block override layering, new block-local schemas without a feature,
all historical asset import coverage, dependency updates merely because newer versions exist,
transparency/OIT without a measured semantic need. These remain future feature/profile-driven work.

Most expensive to defer: (1) external identity boundary, (2) runtime/game policy replicated as universal,
(3) persistence ownership/backend/component recovery decision, (4) residency/late-result lifetime,
(5) presentation/authoritative clock separation. Resource normalization is also a mandatory high risk.

Ugly but leave alone: (1) large client files by themselves, (2) transitional helper crate names,
(3) specialized arrays/value-record entities instead of a general ECS, (4) multiple legitimate Control/
Bot/Game API surfaces, (5) old benchmark recipes whose assertions have no proven replacement.

Developer improvements with most leverage: (1) discoverable page/overlay selector, (2) targeted chunk
blocker inspection, (3) EntityId owner/durability inspection, (4) shared effective runtime setting
readback/safe adjustment, (5) demand-driven cost/cadence and unified lifetime/presentation ledgers.

## Audit closeout scope

This pass changes Markdown only: canonical roadmap, root roadmap navigation, this evidence document
and concise reconciliations. No stage implemented; no product/Cargo/just/scripts/workflow edits,
no version bump/tag/release. Documentation commit/main SHA and its public CI are reported after
publication rather than embedding a self-referential commit hash here. Stop after that CI.

## UX1 bounded execution addition

C1 is CLOSED with Ubuntu/Windows closeout CI run 37182596219. UX1 is CLOSED between C1 and RSM1 (see UX1_REPORT.md, implementation CI 37220087083):
fix held F3/digit routing, console discovery and Unicode rendering before further manual diagnosis.
Scope: shared semantic shortcuts, command/Rhai UX, grapheme/IME editor, bundled-font shaping,
semantic font resources, bounded text caches and C1 scaling. No GUI rewrite, R2 asset migration,
world residency or P1 work. Gates: graphical input/console/multilingual evidence, no-system-font
and package replacement proofs, cache soak and full relevant local/platform validation.
Order: DUX1 -> C1 -> UX1 -> RSM1 -> P1 -> S1 -> R2 -> A1 -> DX2 -> READY1 -> M5.

## RSM1 execution update

RSM1 starts from UX1 closeout `097195340437b16901ec067f19789c973855bbc6`. The pre-fix shared-ledger
baseline measured generation history and ready-payload growth, saved entity-owner metadata retention,
and lighting cleanup faults that retained clean outside-radius world/render/GPU ownership.
Current-token retirement, whole-pipeline admission and narrow entity/lighting corrections are implemented;
RSM1 is CLOSED and PM5-002 resolved with local and Ubuntu/Windows implementation acceptance
recorded in [RSM1_REPORT.md](RSM1_REPORT.md). The historical finding table and stage contract above remain audit history.
DUX1/C1/UX1 remain CLOSED; P1 and every later stage remain inactive. Owner process VRAM is not
measurable on the available llvmpipe surface; logical retention and allocator capacity are distinct.

## P1 execution update

Baseline `f8b1e9b4a4f2846860fb542eda7d20f4099a988e` was instrumented before the repair. The matched
graphical fixture proves repeated fixed-tick camera/player transforms and tick-gated mouse updates.
A transient client position clock and pending-look preview repair those defects; authority remains
20 TPS. The measured callback experiment was removed, leaving FIFO/redraw policy unchanged.
[Detailed results](P1_REPORT.md) distinguish application timing from unavailable physical scanout.
P1 is CLOSED; PM5-003 is resolved after local acceptance and Ubuntu/Windows implementation CI 37256205733. No S1 or subsequent implementation is authorized by this
update. The historical audit findings/contracts above remain the planning baseline.

S1 is CLOSED and PM5-004 is resolved by the measured keep-current decision in [S1_REPORT.md](S1_REPORT.md) and [S1_PERSISTENCE_DECISION.md](S1_PERSISTENCE_DECISION.md). Ubuntu/Windows implementation CI 37271791735 passed. No S1.3 is required; R2 and subsequent stages remain inactive. Final closeout CI must pass before publication acceptance is reported.

## Owner-approved post-audit architecture expansion (post-R2)

This addition supersedes the earlier recommended sequence and inactive-status statements **for current
planning only**. Original findings, counts, execution additions and workload observations above remain
historical evidence; they did not evaluate C2/BG1 or moving voxel spaces. R2 and D-052 are implemented
and closed at `5eb1770`; see [R2_REPORT](R2_REPORT.md) and [ENTITY_TRANSFER_REPAIR](ENTITY_TRANSFER_REPAIR.md).

Current mandatory direction: R2 → F1 → A1 → C2 → BG1 → DX2 → RF1 → VS1 → READY1 → M5.
F1 adds one normal client path, authority/gameplay separation and real-human/hardware validation.
A1 closes external identity/provenance and affected policy debt. C2 composes typed common content and
block-local state; BG1 proves generalized semantic models and independent shapes. DX2 retires only
proven-equivalent workflows and owns diagnostic content. RF1 measures and reduces change/ownership
cost before spatial expansion. [VOXEL_SPACES](VOXEL_SPACES.md) defines VS1's static/kinematic/dynamic
local grids, hierarchy, optional composite physics, lifecycle and persistence. None of these planned
implementation stages is started by this documentation pass.

[ARCHITECTURE_AUDIT](ARCHITECTURE_AUDIT.md) supplies the current source-to-plan gap table;
[ROADMAP](ROADMAP.md) owns complete stage/acceptance contracts. S1/RSM1 conclusions remain valid
for measured single-world workloads. VS1/READY1 must recheck S1 triggers and bounded ownership for
spaces, transform-only updates, split/merge and reference changes, not claim these were already measured.
READY1 must validate the expanded foundational checklist, not simply repeat this original audit.
No known P1 durability/identity blocker may be carried into M5; cube/global-grid limitations are
planned migrations rather than newly discovered functional bugs. M5 requires RF1/VS1/READY1 closure
and a separate activation pass, with space-aware networking from its first real design.
