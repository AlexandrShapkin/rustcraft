# F1 normal client input study

Question: preserve ordinary hotbar selection and quick F3 information while making the
existing UX1 F3/F4 controls independent of trusted scripting admission.

Sources: project-authored [UX1](ux1-developer-controls-text.md) and
[DUX1](dux1-in-game-diagnostics.md) findings from reconstructed Beta release
`740c583901e1ff1150e9ef37e37dab5bc0e4f807`: Minecraft.java F3 handling,
displayGuiScreen/setIngameFocus/setIngameNotInFocus; GuiIngame.java showDebugInfo.
`just refs-status` on the F1 baseline reports optional reference trees/assets absent.
The existing bounded studies answer this routing-only question; no historical code is copied.

Reference behavior: F3 toggles information; UI focus releases capture and clears player keys.
Ordinary number keys select hotbar slots. Beta defines no RustCraft developer selector,
capability model or trusted Rhai contract.

Semantic invariants: one input owner; held F3 consumes digit presses/releases/repeats even
when a capability is denied; bare F3 acts on release; focus loss clears held chords and
movement; normal digits retain selection. Gameplay mode never grants tooling privileges.

Implementation-independent expectations: WindowEvent routing always recognizes shortcuts,
then checks the local session grant before effects. Rhai requires explicit script permission.
Roles/principals describe local authority; mechanism checks capabilities, not role names.

Safe changes: replace legacy branch selection with one router; keep compatibility launch
aliases until DX2. No renderer, gameplay rule or generic persistence-format change.
Optimization opportunities: preserve demand-driven cached diagnostics; keep the script worker idle for ordinary diagnostic use.

Chosen behavior/deviations: existing registered F3 pages/F4 selector are RustCraft controls;
local normal sessions may inspect/configure diagnostics, while ordinary player contexts deny
configuration through the same route. Trusted scripts remain an explicit launch grant.
Acceptance: real WindowEvent integration plus focused chord/digit/repeat/focus/denial tests;
matched AMD/RADV/Vulkan dev/release field phases and checkpoint correlation. Missing hardware
is an unmet acceptance requirement, not evidence of smoothness.
