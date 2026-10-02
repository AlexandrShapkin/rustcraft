# M4 depth-tested block selection outline

## Question and sources

How should the selected voxel outline behave when parts of its bounding box are hidden by world
geometry or cross the camera near plane?

- Historical Beta 1.7.3 source: `reference/external/mc_b1.7.3_release`, revision
  `740c583901e1`, `RenderGlobal.drawSelectionBox` and `drawOutlinedBoundingBox`.
- Independent source inventory checked with `just refs-status`; no independent implementation
  was needed for the specific depth-test behavior.

## Reference behavior and invariants

The historical selection box uses the selected block's state-derived bounding box, expands it by
`0.002` world units, draws a black alpha-0.4 line outline, and disables depth writes without
disabling depth testing. Consequently, scene depth hides rear edges; the camera projection clips
segments at the near plane.

Implementation-independent expectations: front-visible line fragments remain visible, fragments
behind opaque scene depth do not, and camera motion changes visibility without changing outline
geometry semantics. A screen-space HUD overlay cannot satisfy these expectations.

## RustCraft behavior

Selection remains generic and uses the current selected voxel AABB expanded by `0.002`, emitted in
world space as line-list geometry. A dedicated renderer pipeline uses the normal camera matrix,
depth comparison `LessEqual`, depth writes disabled, and black alpha blending. It is submitted
after world geometry has populated the depth buffer and before HUD rendering. Hardware line
clipping handles near-plane intersections; the previous CPU projection/clipping path is removed.

## Intentional deviations

The current generic interaction target exposes block position rather than a state-specific selected
collision AABB, so this pass outlines its unit voxel bounds. GPU line width remains the portable
one-pixel line primitive rather than requesting a non-portable 2-pixel line width. Translucent
objects continue to follow the renderer's existing simple ordering/depth-write semantics.

## Acceptance criteria

- Opaque scene depth hides selected-box rear edges while leaving camera-visible edges visible.
- The outline is slightly expanded, blended black, and does not write depth.
- Near-plane crossing produces clipped finite lines rather than screen-spanning artifacts.
- Selection geometry is camera independent; only its projection/visibility changes with camera.
- Regression coverage validates pipeline state, expansion, and world-space edge topology.
