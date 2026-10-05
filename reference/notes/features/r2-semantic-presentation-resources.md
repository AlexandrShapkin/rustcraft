# R2 semantic presentation resources

Question: can the accepted hotbar, selector, inventory panel and planar player preview retain
layout/interaction while rendering independently of original sheet coordinates and compiled pages?

Read SOURCES.md and REFERENCE_POLICY.md and ran `just refs-status` in the fresh clone. Optional
references there are absent. Studied the existing read-only reference checkout in the old source:
mc_b1.7.3_release revision 740c583901e1ff1150e9ef37e37dab5bc0e4f807, GuiIngame lines 52–53,
GuiInventory background method, and ModelBiped part texture origins. Cross-checked the accepted
project-authored m3-exact-item-ui-rendering feature note at the public e82 baseline. No reference
code/assets are copied or executed.

Historical hotbar source rectangles are 182x22 at (0,0) and selector 24x22 at (0,22). Inventory
uses the 176x166 upper-left panel. Destination centering, integer GUI scale, selector offsets,
slot hit testing, item geometry, text, crafting and cursor semantics must remain unchanged.
Existing planar preview part destinations remain the current policy; this is not a humanoid model
or animation campaign. Historical physical crops belong solely in Minecraft's importer and
historical tests. Generic drawing consumes normalized local UVs resolved through semantic regions.

Safe changes: source layout, filenames, standalone versus cropped inputs, provider package and
compiled page/packing. None may change destination geometry or semantic selection. Preserve alpha,
nearest filtering and quad order even when adjacent quads use different pages. No optimization
claim is made. Reuse existing resource discovery, override, validation, cache and offscreen paths;
do not add a GUI framework.

Inspection found background rebuilding clears already-built preview vertices. Preserve preview
until the next HUD build instead; test the build/background sequence so the expected preview stays
visible and does not accumulate across updates. This restores the existing intended presentation.

Acceptance: equivalent project-owned pixels from original/rearranged crops and direct semantic
files, package override, cold/warm/content-changed cache, arbitrary pages, full-range local UVs,
unchanged destination geometry/slot semantics, actual offscreen evidence, independent sandbox and
release asset policy. Keep all proprietary inputs optional and outside public artifacts.
