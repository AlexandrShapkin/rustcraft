# C1 developer settings reference study

Question: extend the RustCraft-owned F4 developer selector with bounded settings discovery/editing
while preserving F3 and exclusive gameplay/console focus. This is not a Beta player settings menu.

Sources: DUX1 feature note, locked Beta reconstruction 740c583901e1ff1150e9ef37e37dab5bc0e4f807;
Minecraft.java displayGuiScreen/input-focus behavior, previously studied read-only. refs-status
reports optional reference inputs absent in this isolated checkout; existing locked local source
remains available in the owner's checkout. No reference code/assets are copied.

Invariants: explicit developer opt-in; F3 remains; one input owner; held movement/look is cleared;
closing does not resurrect keys; simulation is not implicitly paused. Semantic setting identities,
requested/effective/source readback and capability checks are RustCraft-owned requirements.

Chosen behavior: Settings native debug page, bounded key selector with edit via existing console
command path and simple increment/decrement/reset keys. UI and scripts use one configuration
registry. Structural settings remain protected. No polished GUI, general editor or P1 changes.

Acceptance: real graphical selection/edit/reset/error/capture; headless parity; input/focus and
DUX1 regression; atomic radius pair application; cadence/budget/checkpoint tests and platform CI.
