# Gameplay contract

The project does not promise exact Beta 1.7.3 simulation semantics. It promises familiar,
coherent behavior.

Use original/reference behavior to answer "what would a player reasonably expect?" and then
implement that behavior cleanly.

Examples:

- breaking a block removes it and yields the expected interaction path;
- placing a valid block updates collision/visibility/state;
- switches, torches, repeaters, doors and lamps form understandable automation primitives;
- block fluids behave as block fluids rather than a realistic fluid solver;
- familiar movement, gravity and AABB collision take priority over historical floating-point quirks;
- timing-sensitive mechanics should use explicit logical durations rather than accidentally depend
  on an arbitrary scheduler rate.

Survival mode is authoritative simulation state. Breaking produces semantic drops and item
entities, pickup inserts through the shared inventory API, and crafting matches registered shaped
or shapeless recipes. Development mode may retain a prefilled loadout and instant breaking for
diagnostics; this distinction is not a renderer-only flag.

When choosing between historical quirk compatibility and a clear native mechanic, prefer the clear
mechanic unless the quirk is central to the expected play style.
