# M4 underwater presentation reference note

## Question and sources

How should the camera communicate that it is inside static water while keeping nearby geometry
readable? Reference workflow: `just refs-status` on 2026-10-02. Primary source is the locked
`mc_b1.7.3_release` reference, `EntityRenderer.setupFog` and `EntityRenderer.getFOVModifier`;
the release revision is `740c583901e1`.

## Reference behavior and invariants

Beta 1.7.3 selects a distinct water fog path when the render-view entity is inside water. The
renderer uses exponential fog with density 0.1 and a blue fog color (approximately 0.4, 0.4, 0.9);
the underwater FOV is also narrowed to 60 degrees. The useful semantic invariant is an immediate,
distance-dependent atmospheric change on crossing the water volume, not the historical GL state
API or exact values.

## RustCraft behavior

The client resolves the camera's block through compiled generic liquid presentation metadata. The
game supplies water fog policy; the renderer applies a distance-dependent world fog to opaque and
translucent world geometry. The selected linear range is intentionally tuned to preserve nearby
readability. No Minecraft block ID or liquid color is embedded in generic renderer code.

## Intentional deviations and acceptance

RustCraft does not reproduce legacy exponential GL fog or underwater FOV in this repair. It does
not add swimming, buoyancy, breath or fluid simulation. Crossing the static water surface changes
the camera medium; nearby geometry stays legible and distant geometry is visibly attenuated. Air
presentation returns after leaving the volume.
