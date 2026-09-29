# M3.3 exact item and UI presentation study

This note is the implementation contract for the M3.3 fidelity pass.  It was written before
production changes from the locked Beta 1.7.3 reconstruction and the local reference pack.

> Historical scope: this exact-fidelity contract applied to the completed M3 repair campaign.
> Current project policy uses Beta as a semantic/style reference and permits intentional measured
> or justified deviations for performance, scalability and Game API cleanliness.

## Sources inspected

- `reference/external/mc_b1.7.3_release` revision `740c583901e1`:
  `GuiIngame`, `GuiInventory`, `GuiContainer`, `Container`, `ContainerPlayer`, `Slot`,
  `InventoryPlayer`, `RenderItem`, `RenderBlocks`, `RenderGlobal`, `EntityItem`,
  `PlayerControllerSP`, `RenderHelper`, and `ScaledResolution`.
- `reference/assets/gui/gui.png`, `gui/inventory.png`, `gui/items.png`, `gui/icons.png`,
  and `terrain.png` (all 256x256 RGBA PNGs).
- `reference/external/mc173` revision `16f39e762da2` was used only to clarify naming where the
  reconstructed source is obfuscated.  The locked Beta reconstruction remains authoritative.

## Exact-fidelity implementation table

| Feature | Source method | Source asset | Exact constants/transforms | Beta behavior | Current RustCraft behavior | Required change | Verified |
|---|---|---|---|---|---|---|---|
| Hotbar background | `GuiIngame.renderGameOverlay` | `gui/gui.png` | Source `(0,0,182,22)`; destination `(scaledW/2-91, scaledH-22)` | 182x22 textured bar centered at bottom | Procedural/generated panel | Bind semantic `ui.hotbar` and blit exact rectangle | source ✓ |
| Hotbar selector | `GuiIngame.renderGameOverlay` | `gui/gui.png` | Source `(0,22,24,22)`; destination `scaledW/2-92+slot*20, scaledH-23` | 24x22 selector overlaps bar by one pixel | Approximate highlight | Use exact source/destination and shared layout | source ✓ |
| Hotbar item position | `GuiIngame.renderInventorySlot` | terrain/gui item atlas | `x=scaledW/2-90+slot*20+2`, `y=scaledH-19` | Item renderer and overlay share slot origin | Independent approximate positions | Centralize exact slot geometry | source ✓ |
| Crosshair | `GuiIngame.renderGameOverlay` | `gui/icons.png` | Source `(0,0,16,16)`, destination `(scaledW/2-7,scaledH/2-7)` | GUI icon with alpha blending | Generated crosshair | Resolve semantic icon resource | source ✓ |
| GUI background | `GuiInventory.drawGuiContainerBackgroundLayer` | `gui/inventory.png` | Full source `(0,0,176,166)` at `((screenW-176)/2,(screenH-166)/2)` | Original 176x166 player inventory | Texture pass exists but layout/items approximate | Use exact background and coordinates | source ✓ |
| Player preview | `GuiInventory.drawGuiContainerBackgroundLayer` | player skin (`mob/char.png`) | Translate `(x+51,y+75,50)`; scale `(-30,30,30)`; rotate Z 180; Y 135/-135; X `-atan((mouseY-(y+75-50))/40)*20`; yaw `atan((mouseX-(x+51))/40)*20`, body/head relation uses `*40`; viewY 180 | Animated mouse-facing player model in black window | Empty/black preview | Add minimal skin-mapped humanoid preview | source ✓ |
| Inventory slots | `ContainerPlayer` | `gui/inventory.png` | Output `(144,36)`; 2x2 input `(88+col*18,26+row*18)`; main `(8+col*18,84+row*18)`; hotbar `(8+col*18,142)` | Hotbar is final/bottom row | Positions close but not all paths share geometry | One layout object for drawing/hit testing | source ✓ |
| GUI block item | `RenderItem.drawItemIntoGui` + `RenderBlocks.renderBlockOnInventory` | `terrain.png` | `translate(x-2,y+3,-3)`, `scale(10,10,10)`, `translate(1,.5,1)`, `scale(1,1,-1)`, rotate X 210°, Y 45°, tint, rotate Y -90°, inventory block faces at brightness 1 | Isometric 3D cube with per-face textures | Compressed approximate polygon | Implement matrix-equivalent shared block renderer | source ✓ |
| GUI item lighting | `GuiIngame` / `GuiContainer` + `RenderHelper.enableStandardItemLighting` | n/a | Flat shading; two lights: normalized `(0.2,1,-0.7)` and `(-0.2,1,0.7)`; diffuse `.6`; ambient model `.4` | Crisp directional item shading | Simple face colors | Match directional brightness in item shader | source ✓ |
| Dropped block transform | `RenderItem.doRenderItem` | `terrain.png` | Bob `sin((age+partial)/10+hoverStart)*.1+.1`; Y rotation `((age+partial)/20+hoverStart)*57.295776`; normal block scale `.25` | Rotating/bobbing small block | 3D item but arbitrary transform | Use exact formulas, pivot and scale | source ✓ |
| Dropped stack copies | `RenderItem.doRenderItem` | `terrain.png` | Copies: `<=1:1`, `>1:2`, `>5:3`, `>20:4`; RNG seed 187; displacement per extra copy `random[-1,1]*.2/scale` | Larger stacks show layered copies | One model/entity | Batch deterministic copies in presentation | source ✓ |
| Sprite dropped items | `RenderItem.doRenderItem` | `terrain.png` or `gui/items.png` | Scale `.5`; billboard rotation `180-playerViewY`; terrain for block-like sprite, gui/items for ordinary item | Billboard for true sprites | Block/sprite distinction inconsistent | Semantic presentation kind selects path | source ✓ |
| Stack overlay | `RenderItem.renderItemOverlayIntoGUI` | font renderer | For count >1: shadow text at `x+19-2-textWidth`, `y+6+3`; durability bar follows same path | Count in every GUI context | Missing/inconsistent cursor counts | One shared render-item + overlay path | source ✓ |
| Cursor-held stack | `GuiContainer.drawScreen` | same item atlas | Render after slots at mouse-relative slot origin, then overlay | Same renderer as slot item | Separate simplified path | Reuse semantic renderer and overlay | source ✓ |
| Left/right clicks | `GuiContainer.mouseClicked` -> `Container.slotClick` | n/a | Empty cursor + occupied: left full, right `(count+1)/2`; cursor + empty: left capped full, right one; compatible: left up to capacity, right one; incompatible: swap if cursor <= slot limit; outside -999 drops whole/one | Deterministic transaction semantics | Partial custom cases | Implement semantic transaction matrix | source ✓ |
| Destroy stages | `RenderGlobal.drawBlockBreaking` | `terrain.png` | Tile index `240 + int(progress*10)`, clamp 240..249; atlas tiles are 16x16 at x=0..9,y=15 | Target block geometry re-rendered with crack tile | Generic crack geometry | Override actual block face geometry | source ✓ |
| Destroy blend/depth | `RenderGlobal.drawBlockBreaking` | terrain atlas | Initial blend `(SRC_ALPHA,ONE)`; crack blend `(DST_COLOR,SRC_COLOR)`; color `(1,1,1,.5)`; polygon offset `(-3,-3)`; alpha disabled during geometry | Multiply-like dark/light crack over block | Ordinary alpha and generic cube | Match wgpu blend factors/depth bias | source ✓ |
| Entity item physics | `EntityItem` | n/a | Size `.25`; yOffset `height/2`; initial Y velocity `.2`; bounce `-.5`; ground friction `.588`; age despawn 6000 | Small resting item | Similar but hover/pivot differs | Preserve world-space rest and use exact presentation pivot | source ✓ |
| GUI scaling | `ScaledResolution` | n/a | Scale starts 1; increases while guiScale and `width/(factor+1)>=320`, `height/(factor+1)>=240`; dimensions ceil-divided | Arbitrary window-relative scaling | Approximate scaling | Use compatible integer scaled coordinate space | source ✓ |

## Transaction table

The semantic inventory command layer must implement the source `Container.slotClick` transitions:

| Cursor | Slot | Button | Result |
|---|---|---|---|
| empty | occupied | left | take whole stack |
| empty | occupied | right | take `(count+1)/2` (ceil half), leave remainder |
| occupied | empty | left | place as many as slot limit permits |
| occupied | empty | right | place one |
| occupied | compatible | left | transfer up to capacity |
| occupied | compatible | right | transfer one if capacity exists |
| occupied | incompatible | left/right | swap when cursor count fits slot limit |
| any | outside | left/right | drop whole/one from cursor |

Compatibility is semantic item identity plus subtype/damage; stack limits come from the slot and
item definition.  Crafting output remains a container transaction, never a direct UI mutation.

## Intentional deviations

The engine uses wgpu and semantic content registries instead of OpenGL fixed-function state.
The player preview currently uses a compact presentation silhouette while the skin-mapped
humanoid renderer is being completed; this is the one known visible deviation in the pass.
The engine uses a small Rust mesh renderer rather than the original entity renderer. These are
internal substitutions; the constants, ordering, visible geometry, blending intent and input
semantics above remain the target. Proprietary reference images remain local and are not committed.

## Acceptance checklist

- [ ] `gui/gui.png` hotbar and selector rectangles are used at exact scaled coordinates.
- [ ] GUI block items use the documented RenderItem transform and shared face renderer.
- [ ] Inventory uses the 176x166 `inventory.png` layout, including bottom hotbar row and player preview.
- [ ] All GUI item contexts share count/durability overlay and cursor rendering.
- [ ] Right-click split, one-item placement, compatible merge, capacity and incompatible swap match the table.
- [ ] Dropped block copies, bob, rotation, scale and world pivot match the documented formulas.
- [ ] Destroy stages use terrain tiles 240..249, actual block geometry, special blend and depth bias.
- [ ] F3/target overlays are suppressed while the inventory GUI is open.
- [ ] `just fidelity-m3` exposes deterministic hotbar, inventory, item, drop and destroy-stage scenes.

## M3.4 RenderBlocks face contract

The locked source uses the same `RenderBlocks` face emitters for inventory blocks and world
blocks. For a full unit cube (`min=0,max=1`) with no rotation flags, the non-AO vertex order and
UV corner order are:

| Face | Normal | Vertex order (xyz) | UV order |
|---|---|---|---|
| bottom (-Y) | `(0,-1,0)` | `(minX,minY,maxZ)`, `(minX,minY,minZ)`, `(maxX,minY,minZ)`, `(maxX,minY,maxZ)` | `(u22,v26)`, `(u12,v16)`, `(u20,v24)`, `(u14,v18)` |
| top (+Y) | `(0,1,0)` | `(maxX,maxY,maxZ)`, `(maxX,maxY,minZ)`, `(minX,maxY,minZ)`, `(minX,maxY,maxZ)` | `(u14,v18)`, `(u20,v24)`, `(u12,v16)`, `(u22,v26)` |
| east (-Z) | `(0,0,-1)` | `(minX,maxY,minZ)`, `(maxX,maxY,minZ)`, `(maxX,minY,minZ)`, `(minX,minY,minZ)` | `(u20,v24)`, `(u12,v16)`, `(u22,v26)`, `(u14,v18)` |
| west (+Z) | `(0,0,1)` | `(minX,maxY,maxZ)`, `(minX,minY,maxZ)`, `(maxX,minY,maxZ)`, `(maxX,maxY,maxZ)` | `(u12,v16)`, `(u22,v26)`, `(u14,v18)`, `(u20,v24)` |
| north (-X) | `(-1,0,0)` | `(minX,maxY,maxZ)`, `(minX,maxY,minZ)`, `(minX,minY,minZ)`, `(minX,minY,maxZ)` | `(u20,v24)`, `(u12,v16)`, `(u22,v26)`, `(u14,v18)` |
| south (+X) | `(1,0,0)` | `(maxX,minY,maxZ)`, `(maxX,minY,minZ)`, `(maxX,maxY,minZ)`, `(maxX,maxY,maxZ)` | `(u22,v26)`, `(u14,v18)`, `(u20,v24)`, `(u12,v16)` |

Here `u12/u14/u20/u22` and `v16/v18/v24/v26` are the source rectangle corners computed from
the 16x16 atlas tile, including the source's `-0.01` inset. Inventory rendering calls these
emitters after the `RenderItem` matrix sequence; it does not use the world mesh face convention.

`RenderHelper.enableStandardItemLighting` uses flat shading and two normalized directional lights
at `(0.2,1,-0.7)` and `(-0.2,1,0.7)`, each diffuse `0.6`, with model ambient `0.4`.

`EntityItem` stores `age` in ticks and initializes `hoverStart` to a per-entity random angle. The
world renderer computes `sin((age+partial)/10 + hoverStart)*0.1 + 0.1` and rotation
`((age+partial)/20 + hoverStart)*57.295776`; it does not mutate the authoritative resting Y for
this animation.

`ModelBiped` proportions are head `8x8x8` at `(-4,-8,-4)`, body `8x12x4` at `(-4,0,-2)`, arms
`4x12x4` at `(-8,0,-2)` and `(4,0,-2)`, and legs `4x12x4` at `(-4,12,-2)` and `(0,12,-2)`.
`GuiInventory` applies the documented mouse-dependent yaw/pitch transform before calling the
player renderer. M3.4 now resolves the local `mob/char.png` regions for the six visible body
parts; full perspective cuboid/model articulation remains a documented follow-up.

## M3.5 GUI transform derivation

The malformed GUI cube was caused by an incorrect effective X rotation: the prior Rust code
mixed X/Y components instead of rotating the post-Y-rotation Y/Z components. The locked source
operation order is `translate(slot-2,slot+3,-3)`, `scale(10)`, `translate(1,.5,1)`, negative-Z
scale, `rotate X 210`, `rotate Y 45`, `rotate Y -90`, then centered `RenderBlocks` geometry.

RustCraft keeps the canonical model in `[0,1]^3`, centers it to `[-.5,.5]^3`, applies the
negative-Z handedness conversion once, then applies `Ry(45)` followed by `Rx(210)`:

```text
q = (x-.5, y-.5, -(z-.5))
r = (q.x*cos45 + q.z*sin45, q.y, -q.x*sin45 + q.z*cos45)
s = (r.x, r.y*cos210 - r.z*sin210, r.y*sin210 + r.z*cos210)
```

The result is projected orthographically into the slot; gameplay camera/FOV/aspect are not
involved. The previous implementation used an XY rotation and folded the cube into a diamond.
`beta_gui_project_point` and its golden-corner test are now the CPU reference.

### M3.7 dropped-face correction

The remaining bookshelf mismatch was localized to the dropped model's face UV table. The east
(-Z), west (+Z), and north (-X) entries had been assigned neighboring face corner orders; the
global dropped-only V flip in M3.6 hid part of the error while making the affected side appear
vertically inverted. The canonical table now uses the exact `RenderBlocks` normalized orders for
all six faces, and dropped items use the same `tile_uv` convention as GUI/world geometry. No
model rotation or animation change is involved.

The single convention is: atlas origin is top-left, increasing U is right, increasing V is
down, CPU face UVs are normalized in that convention, and the shader samples without a context-
specific flip. Bookshelf, log and grass are the required directional diagnostics.

## M3.6 orientation, size and unbreakable semantics

`RenderItem.doRenderItem` applies the world-item bob/rotation outside `RenderBlocks`; the block
geometry itself remains the inventory renderer's face geometry. RustCraft therefore keeps the
world transform separate from the GUI transform. Diagnostic blocks with directional textures
(bookshelf, log and grass) are required to verify that the shared face basis is not vertically
mirrored or reused with a GUI-only inversion.

GUI size is measured from the transformed canonical-cube bounds in slot pixels. The renderer
uses the same `GuiBlock3D` path for inventory, hotbar and cursor, with only the slot translation
changing; the model projection uses a `0.65` slot-relative pixel scale so the transformed bounds
fit the 16x16 icon region with the same breathing room as Beta rather than touching neighboring
slot edges.

Beta marks bedrock through `Block.setBlockUnbreakable()`, which sets negative hardness. The
content equivalent is the existing semantic `BlockDefinition.breakable` property. Unbreakable
blocks must short-circuit mining before progress accumulation: they cannot finish breaking and do
not show a normal advancing destroy stage. First-party bedrock sets `breakable: false`; this is a
content property rather than a bedrock-specific runtime match.

## M3.8 — topology audit (supersedes M3.3–M3.7 completion claims)

Locked reconstruction `740c583901e1`, inspected RenderBlocks.renderEastFace and all six
inventory face calls, RenderItem.drawItemIntoGui, RenderHelper and GuiContainer.drawScreen.
The actual -Z position perimeter is valid; its UV polygon was `(1,0),(0,0),(1,1),(0,1)`.
The lower two UV corners are exchanged: the UV triangles have opposite signed areas.
The correct order is `(1,0),(0,0),(0,1),(1,1)`. This is a UV bow-tie, not an axis flip.

GUI code did NOT share that table: four independently authored faces omitted +/-X,
`poly` derived UV from clamped screen distances, Z was discarded, no depth was attached,
and the source transform's final Y rotation was missing. The old bounds test was insufficient.

Canonical repair contract: unit positions [0,1]^3, perimeter ordered outward CCW, indices
0,1,2 / 0,2,3. UV origin top-left, U right, V down. Side V=1-y. Each triangle's UV determinant
must have equal sign and nonzero magnitude; opposite rectangle corners sum equally in position
AND UV. Keep the existing correct world mapping (including top/bottom convention) as the
single source. No dropped-only flips or rotated-texture compensation.

Effective GUI derivation (column vectors, OpenGL postmultiplication):
M = T(x-2,y+3,-3) S(10) T(1,.5,1) S(1,1,-1) Rx(210) Ry(45) Ry(-90) T(-.5).
For centered p, r=Ry(-45)p; s=Rx(210)r; t=(s.x,s.y,-s.z).
Screen pixels = (x+8+10*t.x, y+8+10*t.y); eye Z=7+10*t.z.
Screen Y is already down: do NOT negate it again until pixel-to-NDC conversion.
Depth decreases with eye Z. Model normal uses the same orthogonal transform.
Two normalized lights (.2,1,-.7),(-.2,1,.7) transformed by Rx(120), ambient .4,
diffuse .6 each, clamped to 1. Inventory renderer sets normals, not world face brightness.
Cube bounds in native slot pixels: X=[0.928932,15.071068], Y=[0.134339,15.865661].
No empirical .65/.72 scale or slot-width-dependent scaling is permitted.

Acceptance: six single-face solid/corner/UV/atlas images; bookshelf/log/grass world, dropped,
GUI and cursor render paths; CPU planarity, winding, affine UV and golden GUI matrix tests.
Offscreen GPU readback must use its own COPY_SRC color attachment, independent of a window.

### M3.8 implementation and evidence

The pre-repair `before-neg-z-corners.png` is a valid rectangle. In
`before-neg-z-uv.png`, the arrow splits and folds at the triangle diagonal. Thus the
reported diagonal face was **vertex-to-UV correspondence**, not non-planar positions or
crossed position indices. An older unit test explicitly asserted the erroneous UV order;
it is replaced by independent affine/area/winding checks.

One `geometry::FACES` now defines the full cube used by world meshing, GUI icons, dropped
items and destroy-overlay positions. Coordinates are [0,1]^3. Item centering is explicit
(p-.5), never hidden in a second face table. Indices are [0,1,2,0,2,3], outward CCW.

| Normal | Four perimeter vertices (XYZ) | Increasing U | Increasing V |
|---|---|---|---|
| +Z | 001,101,111,011 | +X | -Y |
| -Z | 100,000,010,110 | -X | -Y |
| +X | 101,100,110,111 | -Z | -Y |
| -X | 000,001,011,010 | +Z | -Y |
| +Y | 011,111,110,010 | +X | +Z |
| -Y | 000,100,101,001 | +X | -Z |

All rows attach UVs [(0,1),(1,1),(1,0),(0,0)] to that perimeter. This preserves the
existing world model convention (including its underside) rather than introducing a world
texture regression to repair an item. Legacy `Face::North` means +Z; semantic
`Facing::North` means -Z. Use explicit normals when comparing them.

`tile_uv` is the single image-to-atlas conversion: (tile+[u,v])/16. PNG row zero and texture
V zero are the top. Shader sampling does not invert V. Search found no remaining item
flip_u/flip_v/swap_uv logic. The accepted destroy material retains its own existing tile
orientation; its positions/topology now use the canonical source, and its blend/stages are
unchanged. Screen pixel Y-to-NDC conversion is geometry projection, not a UV flip.

GUI no longer derives UVs from projected pixel distances. It emits all six canonical faces,
keeps Z, culls back faces, clears depth independently of the world, and uses the effective
matrix derived above. Native projected size is **14.142136 × 15.731322 pixels**, independent
of the 18-pixel inventory slot spacing. Cursor items use the same renderer with a separate
foreground depth band. Per-face content textures/tints resolve once in `BlockModel`.

Offscreen evidence is generated by `just render-test-all` / `just fidelity-m3`, using explicit
RGBA8-sRGB color + depth textures and GPU readback. The llvmpipe GL adapter successfully
produces PNGs without a surface. Generated UV charts require no proprietary resources.
The suite includes all six faces (solid/corners/UV/atlas), bookshelf/log/grass plus three
comparison blocks, front/rear world and dropped views, enlarged/native GUI contexts, and a
labeled GUI grid. Each output has a text manifest. PNGs are local ignored evidence, not
redistributed source assets. Wireframe shows triangle edges, vertices, normal arrows and
explicit face names. Original source paths are under
`1.7.3-LTS/src/minecraft/net/minecraft/src/` at revision `740c583901e1`.

Acceptance remains owner-controlled. Captures demonstrate rectangular texture mapping and
correct face resources; they are not a claim of interactive acceptance or exact pixel parity.
