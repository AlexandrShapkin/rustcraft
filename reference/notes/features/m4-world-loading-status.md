# M4 world loading status

## Question and evidence

The current client must remain responsive while local chunks load/generate. The narrow question is
what user-visible status is appropriate while a world is not yet safe to enter. `just refs-status`
reports historical source revision `740c583901e1` for `mc_b173_release`; local texture assets are
not needed for this question. The checked source is `GuiDownloadTerrain.java` and its client
connection flow in `Minecraft.java`.

## Reference behavior

The historical client has a dedicated terrain-download screen that continues updating network
packets and draws a centered “downloading terrain” label. It does not present a partially usable
player while the terrain is unavailable. This is a loading-state semantic, not evidence that
RustCraft should copy its GUI or networking implementation.

## Invariants and safe differences

- Do not enable player simulation before the required startup collision neighborhood is ready.
- Keep the window/event loop responsive and able to close while work proceeds.
- Status text/title is presentation only; it must not own world or generation policy.
- A compact project-owned “Loading world…” status is sufficient. Progress percentages, textures,
  exact layout, network behavior, and historical implementation details are safe to differ.

## RustCraft behavior and acceptance

RustCraft will retain its window and event loop during bounded asynchronous metadata/player setup
and chunk load/generation. A simple title/status indicates loading. Input can close the client;
world simulation starts only after the minimum safe neighborhood around the restored/default player
position has been published. Tests assert that loading does not run simulation and that the ready
transition preserves restored player state.
