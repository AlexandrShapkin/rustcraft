# M4 world residency and view distance

## Question and sources

Question: does the current local residency radius cover the distance the renderer can show, and
what predictable neighborhood and minimum usable core should RustCraft expose while asynchronous
streaming converges?

- Historical Beta 1.7.3 reconstruction: `reference/external/mc_b1.7.3_release`, revision
  `740c583901e1`, `GameSettings.java`, `RenderGlobal.java`, `EntityRenderer.java`,
  `PlayerManager.java`, and `lang/en_US.lang`.
- Independent cross-check: `reference/external/BetrockPlusPlus`, revision `0fc3a3d63345`,
  `server_pconnstate_manager.cpp` and `world.cpp`.
- Source status checked with `just refs-status`; `mc173` is locked at `16f39e762da2`.

## Reference behavior

Beta exposes four named render-distance settings: Far, Normal, Short and Tiny. The default
setting index is zero (Far). `RenderGlobal.func_958_a` sizes the horizontal renderer grid from
`64 << (3 - setting)`, capped at 400 blocks, then converts that distance to 16-block chunk
cells. `EntityRenderer.func_4139_a` sets far clipping to twice `(256 >> setting)`: 512 blocks at
Far and 256 at Normal. Server `PlayerManager` membership is the complete inclusive square
`center +/- playerViewRadius` on both horizontal axes; its spiral changes delivery order, not
membership. BetrockPlusPlus independently uses inclusive square view neighborhoods and requires a
complete radius-3 (7x7) spawn neighborhood before entering play. These are
presentation/availability expectations, not requirements to copy either scheduler.

## Semantic invariants

- The requested rendered area should not substantially exceed the area residency can make
  available; otherwise the player sees a nearby loading edge despite a longer camera far plane.
- Nearby terrain must be prioritized and available before the player reaches it.
- A larger view/residency radius must remain bounded and must not make loading synchronous.
- Simulation and persistence must operate only on authoritative resident data; unloaded space is
  not air.
- The usable core around the player must be contiguous. A traversable column must already have a
  renderable presentation; speculative visible columns may extend beyond the usable core.
- Cross-border light reconciliation may improve an already locally correct presentation, but it
  must not make unrelated locally lit columns unavailable.

## Current RustCraft evidence

The client creates `WorldResidency::new(3, 1)`. Its load radius is a lattice Euclidean disk of
three chunks (29 desired columns, about 48 blocks to the cardinal edge); retain radius is a disk of
four chunks (49 columns). The camera far plane is 256 blocks (16 chunks). Authoritative publication
immediately makes a column available to collision and targeting, while render visibility waits for
all vertical section meshes to upload. Boundary-light work no longer gates initial local
presentation, but uneven load/generation/light/mesh completion cuts holes into the actually
renderable set. The apparently irregular world is therefore a circular Desired boundary combined
with lifecycle skew, not a deliberate irregular or frustum-culling shape.

For radii 3/4/5/6, inclusive column counts are: Euclidean 29/49/81/113,
Manhattan 25/41/61/85, and Chebyshev 49/81/121/169. Euclidean is visually circular but has
stair-stepped cardinal/diagonal reach; Manhattan is a conspicuous diamond with the shortest
cardinal buffer; Chebyshev is a predictable square, matches the familiar complete neighborhood,
and makes a guaranteed travel margin easy to state. Its higher corner cost is explicit rather than
an accidental queue-order side effect.

## Details safe to change

- Load and retain radii may be independently tuned/configured.
- The exact radius need not match Beta presets; deterministic generation, bounded queues, and
  time-to-visible are stronger constraints than historical grid shape.
- The renderer may continue conservative section culling within the available resident world.

## Optimization opportunities and chosen behavior

Use Chebyshev distance for Desired and Retained membership. Keep authoritative residency distinct
from usability: `Desired(center, Rload)` is requested policy, `Visible` means the renderer has a
complete current column presentation, `Safe` is the contiguous visible locally authoritative core
released to collision/targeting, and `Retained(center, Rretain, pins)` is residency hysteresis plus
explicit persistence/light/entity pins. Start control only after a complete 3x3 Safe+Visible core.
Use urgency tiers current / 3x3 safe / visible travel ring / directional prefetch / outer desired;
movement direction dominates camera direction within a tier. Boundary-light convergence remains
eventual and may dirty affected sections without revoking safety.

Measure radii 3 through 6 on one deterministic normal-speed route before selecting the default. A
configurable radius is preferable to silently treating bootstrap size as product view distance,
but a larger default must not hide an inverted or starving pipeline. Retain radius is exactly one
chunk beyond load radius unless evidence demonstrates another bounded hysteresis.

## Intentional deviations

RustCraft uses streaming interest sets, explicit local readiness and async lifecycle state rather
than Beta's fixed renderer grid. It has one local interest center and does not promise Beta's Far
preset or exact distances. Safe availability is deliberately stricter than authoritative
residency, and visible speculative prefetch is allowed outside Safe. Network/multiple-player
interest management is outside this slice.

## Acceptance criteria

- F3 and streaming diagnostics report configured load/retain radii and ready/visible columns.
- A centered ASCII grid distinguishes Desired, Safe, Visible, Retained, pinned and concrete
  lifecycle blockers over at least the retain neighborhood.
- Radius 3/4/5/6 and lookahead off/on are compared on the same deterministic normal-speed route.
- A complete centered 3x3 Safe+Visible core exists before control. Afterward the player never has
  zero forward Safe or Visible margin at normal speed, including turns, diagonal/reverse travel,
  negative coordinates and revisiting evicted persisted terrain.
- `Safe => Visible` holds except inside the same unobservable result-application callback.
- A larger radius does not cause sustained frame/TPS regression, unbounded queues, or unbounded
  resident memory; travel eviction and save-before-evict still pass.
- Neighbor arrival remeshes both border sides as required, and boundary-light final state is
  independent of arrival order.

## Result

Chebyshev radius 4 with retain radius 5 was selected. The final actual-client normal-speed route
kept forward Safe/Visible margins positive in fresh and saved worlds. Lookahead on/off produced the
same 0.5-column minimum forward Safe/Visible margin; p95 request-to-visible was 1.30/1.50 seconds,
within run variance, so lookahead defaults off while the tested movement-dominant override remains.
