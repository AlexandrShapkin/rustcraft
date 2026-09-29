# Defect queue

Use this for known non-blocking issues. Do not hide P0/P1 problems here instead of fixing them.

| ID | Severity | Area | Description | Reproduction / evidence | Status |
|---|---|---|---|---|---|
| M1.1-AUDIT-001 | P1 | client input | Losing window focus could leave mouse capture/look delta state active for the next frame. | Static event-path audit; `Focused(false)` had no handling. | Fixed: release capture and clear look delta on focus loss. |
| M1.1-AUDIT-002 | P2 | client input | Pressing opposite movement keys could clear the still-held direction when either key was released. | Static controller audit; scalar direction fields could not represent simultaneous keys. | Fixed: track each key and derive axes; regression test added. |

| M2-001 | P3 | materials | Fractional-alpha blending/sorting is deferred; current glass uses binary atlas alpha. Identical leaf blocks suppress internal faces. | Material meshing tests and sandbox capture; accepted M2 transparency limit. | Open |
| M2-002 | P3 | outline | Projected cube outline includes hidden edges and clips individual near-plane endpoints. | Starter pillar capture shows all twelve edges. | Open; depth-tested outline polish later. |
| M2-003 | P2 | lighting latency | Debug-build emissive removal takes about 131 ms in the documented flat scene, plus affected mesh extraction. | `just bench-m2`, M2_VALIDATION.md. | Open; profile a release workload before optimizing. |
| M2-004 | P3 | platform metrics | Non-Linux process providers and GPU providers without matching readable DRM sysfs counters return N/A. | Unsupported-provider tests; AMD counters work on validation hardware. | Open; optional independent providers later. |
| M2-005 | P3 | references | refs-status reports the named vanilla pack paths missing even when the supported loose reference/assets/terrain.png exists. | Local atlas loads successfully in all textured diagnostics. | Open |
| M2-006 | P3 | acceptance | Manual M2 key/mouse/wheel/F3 interaction acceptance was pending during implementation. | Owner manually accepted M2 after interactive inspection. | Closed |
| M3-001 | P3 | inventory UI | Shift-click transfer remains deferred; right-click splitting/merging is implemented and manually accepted. | M3 inventory transaction matrix and owner acceptance. | Open; optional later convenience |
| M3-002 | P3 | item physics | None currently known after centering the item AABB on its resolved resting position. | Regression test requires a dropped item to settle at the block top without sinking. | Fixed in M3.1 |
| M3.1-001 | P3 | item performance | Item merge and pickup still use bounded pairwise scans; 1,000 dropped stacks remain a debug-build warning. | M3 benchmark: approximately 36.84 ms/tick for 1,000 stacks. | Open; dedicated performance pass |
| M3.2-001 | P3 | Beta presentation | Earlier inventory-preview/drop presentation differences were repaired or accepted during M3.3-M3.8. | M3 reference notes and owner acceptance. | Closed |
| M3.2-002 | P3 | inventory input | Right-click semantics are implemented for player inventory slots; right-click crafting-slot distribution and shift-click remain deferred. | Focused M3.2 interaction scope. | Open |
| M3.3-001 | P3 | fidelity review | Automated surface capture limitation required owner comparison of the inventory preview and item presentation. | Owner completed manual M3 acceptance. | Closed; accepted presentation |
| M3.8-001 | P1 | block presentation | Dropped `-Z` block face used a bow-tied UV-to-vertex mapping; GUI block items independently omitted faces/depth and derived UVs from projected pixels. | Pre-repair corner/UV diagnostics isolated valid position topology and opposing UV triangle signs. | Fixed in M3.8; canonical geometry, exact GUI transform/depth pass and offscreen regression scenes added. |
| M3.8-002 | P3 | acceptance | Automated offscreen, client startup and headless checks passed; the owner then completed interactive M3 acceptance. | `docs/M3_8_VALIDATION.md`. | Closed |
| ARCH-001 | P2 | runtime ownership | `runtime` still combines reusable movement/lighting orchestration with Minecraft inventory, mining, drops, pickup and crafting policy. | `docs/ARCHITECTURE_AUDIT.md`; Cargo/source ownership audit. | Open; migrate the affected slice before extending it. |
| ARCH-002 | P3 | UI ownership | Beta hotbar/inventory construction currently lives beside generic HUD rendering in `render`/client. | `render::hud` and client inventory paths. | Open; preserve M3 fidelity and split when UI is next extended. |
| ARCH-003 | P3 | Agent/Bot API | Generic primary/secondary actions coexist with legacy mining, placement, inventory and crafting convenience fields. | `AgentIntent` and Bot observations. | Open; retain compatibility, move game conveniences to a Minecraft layer incrementally. |
| ARCH-004 | P3 | legacy content API | `mod-api::BlockDefinition` still carries Minecraft hardness/tool/drop policy alongside reusable rendering/collision data. | Generic replacement foundation exists in `game-api::VoxelDefinition`. | Open; migrate definitions without an M3 behavior rewrite. |

Severity:

- P0: security, corruption, data loss — fix immediately.
- P1: blocks build/current slice or violates a core contract — fix immediately.
- P2: visible functional issue — may wait for the next defect batch.
- P3: polish/cleanup/performance suspicion — backlog until relevant.
