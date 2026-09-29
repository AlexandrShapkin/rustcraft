# M3.2 inventory, item and mining reference study

Feature: Beta-like player inventory, item presentation and block-destroy feedback.

Sources inspected (locked by `just refs-status` on 2026-09-29):

- `mc_b1.7.3_release` revision `740c583901e1`, especially `GuiInventory.java`,
  `GuiContainer.java`, `ContainerPlayer.java`, `RenderItem.java`, `RenderGlobal.java`,
  `PlayerControllerSP.java` and `EntityItem.java`.
- Local `reference/assets/gui/inventory.png` (256×256 sheet containing a 176×166 inventory
  region), `reference/assets/terrain.png` and `reference/assets/gui/items.png`.
- `reference/SOURCES.md` and the independent source map; no separate local vanilla-b1.7.3 pack
  directory is present.

Observed behavior and geometry:

- `GuiContainer` centers a 176×166 container. `ContainerPlayer` places the 2×2 crafting grid at
  (88,26), output at (144,36), main inventory at x=8+18*column and y=84+18*row, and hotbar at
  y=142. The hotbar is the final/bottom row.
- Item rendering is shared by GUI and world paths. Renderable blocks use a small lit 3D block;
  other items use a camera-facing sprite. `RenderItem` bobs and rotates dropped items and chooses
  1/2/3/4 duplicated models by stack size. This batch keeps one model per stack but preserves the
  semantic presentation split.
- `RenderItem.renderItemOverlayIntoGUI` draws stack counts for every GUI context, including the
  cursor-held stack, after the item visual.
- `GuiContainer` uses full-stack left click and half-stack/right-click semantics through container
  click operations. The minimum target is half pickup, one-item placement and one-item merge.
- `RenderGlobal.drawBlockBreaking` renders terrain atlas indices 240 through 249 over the target
  block. `PlayerControllerSP` supplies normalized damage and resets it on release or target change.
  The local terrain atlas contains these ten destroy stages in row 15, tiles 0..9.
- The inventory screen is a GUI layer above world presentation; target/crack overlays must not be
  drawn while it is open.

Intentional deviations:

- The project keeps a compact Rust presentation path and does not render the Beta player model in
  the inventory screen yet.
- One dropped stack is rendered instead of Beta's stack-size-dependent duplicated models.
- The local resource is resolved from the current content asset root and never committed.

Acceptance matrix:

| Behavior | Beta reference | RustCraft target |
|---|---|---|
| Inventory background | `gui/inventory.png`, centered 176×166 | same local resource and geometry |
| Hotbar row | final row at y=142 | final/bottom row |
| Block slot item | lit 3D block render | semantic Block3D cube |
| Normal item | sprite path | semantic Sprite2D path |
| Cursor count | overlay when count >1 | shared count overlay |
| Right click | half pickup, place/merge one | same operations |
| Dropped block | bobbing/rotating 3D block | world-space 3D cube with bob/rotation |
| Mining cracks | terrain atlas 240..249 | same local atlas stages |

Acceptance checklist: centered inventory; bottom hotbar; correct 2×2/output coordinates; counts in
all contexts; right-click split/place; 3D block items in GUI and world; crack stage 0..9; no target
outline or crack overlay over GUI.
