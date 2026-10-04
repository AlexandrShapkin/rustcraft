# DUX1 in-game diagnostics reference study

Question: preserve recognizable quick debug access and clean gameplay/UI focus while adding
RustCraft-owned developer selection/inspection. No historical menu or architecture is being ported.

Sources: reconstructed Beta release 740c583901e1ff1150e9ef37e37dab5bc0e4f807, read from the owner's
existing ignored reference clone (isolated checkout refs-status reports optional inputs absent).
Minecraft.java displayGuiScreen, setIngameFocus/setIngameNotInFocus and F3 handling; GuiIngame.java
showDebugInfo path. Existing RustCraft DX1/DUX1 audit is the accepted developer-tool contract.

Baseline: F3 toggles quick debug information. Opening a screen releases mouse capture and resets
player keys; closing it restores gameplay focus. Beta has no contract for RustCraft semantic
page/overlay registry, Rhai console, targeted EntityId inspection or provider-demand metrics.

Invariants: F3 remains quick access; UI input has one owner; opening clears held movement/look;
closing never resurrects a held key; console/selector mutually exclusive; overlays are transient.
Read-only inspection uses stable semantic IDs and the shared Control Snapshot. No proprietary assets.

Safe deviations: F4 discoverable keyboard selector, bounded text UI, explicit developer opt-in,
registered native views, demand/cadence/cost metadata, contextual unavailable/stale/truncated values.
No pause implied merely by opening diagnostics. No physical display-timing claim.

Optimization: collect only demanded domains; reuse cached observations for UI/Control/Rhai;
cheap counters and bounded targeted scans, no per-frame world serialization. Existing line/HUD
rendering remains mechanism; game package owns entity/persistence semantics.

Acceptance: real selector navigation/toggles/help/inspection/focus/capture; same observations in
queries/UI; inactive provider counts stop; repeated cycles have bounded geometry/cache/history;
non-Minecraft registration, headless tests, full relevant regression and Ubuntu/Windows CI.
No C1/RSM1/P1/S1/R2/A1/DX2 implementation.
