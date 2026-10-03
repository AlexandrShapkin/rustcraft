# M4 dropped-item persistence and lifecycle

## Question

Which current dropped-item fields affect future gameplay, and what lifecycle must survive
column eviction and world reopen without importing the historical entity architecture?

## Sources

- `jacobo-mc/mc_b1.7.3_release` at `740c583901e1`, reconstructed
  `EntityItem::onUpdate`, `writeEntityToNBT`, `readEntityFromNBT`, and player pickup behavior.
- `theorzr/mc173` at `16f39e762da2`, independent Rust interpretation in
  `entity/tick.rs::tick_item`, `entity/tick_state.rs`, and `entity/mod.rs::Item`.
- RustCraft accepted M3 behavior in `runtime::survival::ItemEntity` and `Simulation`.

## Reference behavior

- A dropped item carries a stack, world position and motion, an age/lifetime, and a pickup-delay
  counter. Physics and pickup advance on simulation ticks.
- The historical item expires after 6,000 ticks (five minutes at 20 TPS).
- Historical NBT stores stack and age. Position/motion are supplied by the generic entity record;
  random render orientation is presentation state. The independent implementation likewise treats
  lifetime and pickup delay as simulation fields and removes the entity at 6,000 ticks.

## Semantic invariants

- Closing, streaming out, and reopening must not refresh an old item or erase its remaining
  pickup delay.
- A moving item resumes from its authoritative position and velocity.
- Stack identity is semantic content identity, never a process-local item handle.
- An item exists once: merge, pickup, despawn, and cross-column movement cannot resurrect or
  multiply it after durable recovery.
- Render bobbing/orientation remains derived from authoritative age/position and is not stored.

## Details safe to change

- Persistent framing, entity identifiers, column ownership, checkpoint cadence, and recovery
  protocol are RustCraft mechanisms rather than historical NBT/region architecture.
- RustCraft keeps its accepted floating-point seconds representation and existing 0.25-second
  block-drop pickup delay. Exact historical random launch motion and item health are not current
  RustCraft state and are not invented for this pass.

## Chosen RustCraft behavior

- The current authoritative fields are stable entity ID, `ItemStack`, position, velocity, age,
  and remaining pickup delay. They are encoded by the Minecraft game package into a generic
  versioned spatial record stored with the owning column.
- Item age and world simulation time pause while the world/column is unloaded. Reopen does not
  apply wall-clock time and therefore cannot double-advance age.
- Items are removed once age reaches 300 seconds. This repairs the current implementation, which
  stopped simulating them after 300 seconds but left them resident forever.
- Presentation extraction continues through the ordinary dropped-item snapshot path after decode.

## Optimization opportunities

- Spatial partitioning keeps load/decode proportional to resident columns and permits entity
  columns to participate in normal save-before-evict.
- Continuous motion is dirty-tracked but checkpointed/coalesced; it must not fsync every tick or
  permanently pin a column.
- The existing pairwise merge pass is unchanged unless persistence measurements expose a new P1
  blocker; its M3.1 performance defect remains separate.

## Acceptance criteria

- Non-default stack, damage, position, velocity, age, and pickup delay round-trip exactly within
  the persisted numeric representation.
- An entity keeps stable identity across close/reopen and save/evict/reload.
- Cross-column movement, merge, pickup, block-drop, and despawn retain exactly-once durable
  outcomes under the documented recovery ordering.
- Near-expiry items despawn once and remain absent after another save/reopen.
- Loaded entities re-enter normal simulation and renderer extraction without a special render
  path.
