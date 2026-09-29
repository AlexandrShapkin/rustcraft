# Grass atlas material, M1.1 diagnostic finding

Question: why does the normal flat world remain grey after the independent stone/checker
diagnostics prove texture sampling works?

Evidence inspected:

- Local `reference/assets/terrain.png`: 256x256 PNG; tile (0,0) is a grayscale grass top.
- `mc-b173-release`, revision `740c583901e1`,
  `1.7.3-LTS/src/minecraft/net/minecraft/src/BlockGrass.java`, texture selection and
  `colorMultiplier`: top tile 0, bottom tile 2, ordinary side tile 3; grass supplies a color
  multiplier from temperature/humidity.
- The unlit normal-world GPU capture before tint visibly samples the grayscale pattern.
  The capture after tint uses the same mesh, UVs, atlas and camera and shows green grass.

Conclusion: texture loading was working in this controlled scene. The material multiplier was
missing. This is independent of camera distortion and independent of illumination.

Project decision: use a fixed, authored sRGB `#7cbd6b` grass-top tint, converted to linear RGB
before multiplying the decoded sRGB texture. Preserve ordinary dirt/sides and stone without tint.
This is a deliberate flat-world simplification; the exact chosen color is not claimed to match
a historical biome. No biome simulation, reference implementation code, or proprietary assets
are added to the repository.
