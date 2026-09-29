# M2 content and classic lighting evidence

Consulted local references after `just refs-status`:

- `mc_b1.7.3_release`, commit `740c583901e1`, client `Block.java`, `BlockSandStone.java`:
  atlas indices, leaf opacity, glass material and sandstone face mapping.
- `mc173`, commit `16f39e762da2`, `mc173/src/block/material.rs`:
  independent cross-check of air/glass/leaf opacity and classic emission ranges.
- User-local ignored `reference/assets/terrain.png`, 256x256, 16px cells: visual mapping evidence.
- Previous `grass-material.md` conclusions remain in force.

Authored conclusions: stone=1, dirt=2, grass top/side=0/3, planks=4, brick=7,
cobble=16, bedrock=17, sand/gravel=18/19, log side/end=20/21, bookshelf=35,
glass=49, leaves=52, sandstone top/side/bottom=176/192/208. The temporary lamp uses tile 105.
Only the first-party resource resolver knows these historical coordinates. Generic block IDs
need not equal historical numeric IDs. Glass is solid but passes light; leaves attenuate light.
Air has no item. Bedrock is unbreakable. The temporary lamp emits level 15 and is deliberately
not a torch. Sand/gravel do not fall in M2; no entity/fluid/survival mechanics are implied.

No reference implementation was ported. FIFO fixed-point light propagation and direct sky seeds
are project-authored; historical update-order bugs are not reproduced. No proprietary texture or
font was added to tracked content. Text uses project-authored glyph geometry.

External API references: wgpu RenderPassTimestampWrites documentation (via Context7), checked
against the workspace's wgpu 26 build; Linux kernel /proc and amdgpu sysfs documentation linked
from D-018. Timestamp support and optional hardware telemetry were demonstrated on RADV RAVEN.
