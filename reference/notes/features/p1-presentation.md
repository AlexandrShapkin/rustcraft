# P1 presentation study

Question: preserve fixed-tick gameplay while rendering continuous camera/movement; distinguish
application timing from display scanout. Baseline f8b1e9b4a4f2846860fb542eda7d20f4099a988e.

Locked local historical source: mc_b1.7.3_release 740c583901e1ff1150e9ef37e37dab5bc0e4f807,
1.7.3-LTS/src/minecraft/net/minecraft/src/EntityRenderer.java. `updateCameraAndRender` consumes
mouse movement during rendering (lines 339–355); camera/entity positions use last/current tick
positions and a partial-tick fraction (443–445, 651–653). This is behavioral evidence, not code to port.
Independent BetrockPlusPlus 0fc3a3d63345f2e2c4da12033ebb9167113bac2f,
shared entities/entity.h identifies transient ledge/yaw/pitch smoothing. It does not establish a
matching client presentation clock; no stronger independent historical cadence claim is made.
Publication clone refs-status reports absent clones; the original repository's read-only locked
references above are available and used. No reference implementation was executed or copied.

Invariants: authoritative 20 TPS, collision/mining/placement and bot observations stay authoritative;
one mouse intent is applied once; visual state never persists; local look should react without a
full-tick delay; discontinuities must snap rather than interpolate across worlds. Camera-relative
HUD stays on the chosen camera. Gameplay targets remain authoritative; visual target inspection
must make any transient difference explicit. No Minecraft rules in a generic interpolation primitive.

Safe differences: choose native Rust transient transforms/monotonic clocks, shortest-path angles,
explicit reset/pause semantics and modern platform redraw notification. No historical timer or Java
architecture is inherited. Any intentional visual translation delay is documented and measured.

Before a repair, instrument the existing path and run stationary, pan, walk, walk+pan and streaming
phases on the same surface/settings. Only measured duplication/input/scheduling defects justify
changes. No mode/limiter change or broad renderer rewrite is pre-authorized by this study.
Acceptance: matched interval distributions, duplicate/motion deltas, application input-to-camera
latency, exact authoritative stepping, discontinuity/catch-up/focus tests and graphical regression.
Physical monitor/VRR/scanout evidence is unavailable unless a real provider supplies it.
