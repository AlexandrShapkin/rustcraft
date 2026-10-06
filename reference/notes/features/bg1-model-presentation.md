# BG1 model presentation

Question before edits: how can general static models reuse accepted GUI/drop transforms without
changing current ordinary-cube Beta presentation?

Sources: accepted project feature evidence `m3-exact-item-ui-rendering.md`, originally locked to
`mc_b1.7.3_release` `740c583901e1` RenderItem.drawItemIntoGui/doRenderItem and
RenderBlocks.renderBlockOnInventory; D-022 canonical presentation; current renderer geometry,
inspection, HUD projection and dropped-item extraction at `8af0f161`.
`just refs-status` at entry reports historical source repositories absent, owner vanilla assets
present. No fresh historical-source claim is made; the existing accepted constants are retained.
No external assets are needed for the new geometry proof.

Invariants: outward CCW triangles, top-left UV convention, material role/tint attached through
orientation, unchanged GUI matrix/projection/light convention, unchanged dropped-item scale .25,
bob/rotation/copy formulas. World, GUI and drops use the same resolved static model.

Chosen RustCraft behavior: keep canonical cube specialization; general bounded triangle geometry
uses identical presentation transforms and atlas grouping. Collision and selection stay independent
of visual triangles. Box/wedge/static-mesh content is project-owned generic proof, not a claim of
historical Minecraft slab/stair policy. No new Beta mechanics or class hierarchy is introduced.

Safe changes/optimization: compiled shared model geometry and compact state-selected handles;
no shape generation per voxel. Intentional deviation: general static shapes have no historical
Beta equivalent, but their presentation follows the existing semantic transforms.

Acceptance: ordinary cube render tests retain UV/positions; generic model has identical resolved
triangles in world/GUI/drop inspection, correct winding/normals, page/tint handling, rotated wedge,
partial-neighbor and section-boundary tests. Generated visual captures remain ignored.
