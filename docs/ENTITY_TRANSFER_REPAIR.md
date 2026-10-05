# Durable entity transfer repair

The four reviewed defects exist in public e82d42d2; they are baseline correctness repairs,
independent of R2. No persistence hunk was reverted to the obsolete checkout's HEAD.

| Finding | Root cause | Repair |
| --- | --- | --- |
| Full pickup | Column encoder enumerated current runtime entities after immediate removal. | Encode the receipt's pre-transfer record until its exact player checkpoint transition is acknowledged. |
| Partial pickup | Reduced source and increased inventory had no shared recovery identity. | Quantity/revision receipt in the same player checkpoint as inventory; preserve pre-state until player acknowledgement and reduce exactly once during recovery. |
| Migration | Runtime column ownership controlled source removal; late acknowledgements were ignored when runtime moved again. | Retain the acknowledged old-owner payload and acknowledge immutable save records; only destination success allows old-owner cleanup. |
| Failed snapshots | Success short-circuited snapshot removal. | Both normal and shutdown completion paths retire every terminal snapshot; only successful results reach runtime acknowledgement. |

Full pickup: preview inventory/removal in memory, preserve pre-transfer source encoding, persist
inventory plus receipt, acknowledge that exact receipt, publish source deletion, acknowledge source
absence, then checkpoint receipt pruning. Partial pickup follows the same order, with post-state
reduction replacing deletion. Pending partial remainders cannot move/merge/despawn/pick up again
until their source transition completes. Subsequent receipts for the same EntityId have distinct
revisions; an old player completion cannot authorize a newer transition.

Recovery checks entity revision against the receipt's pre-transition revision and assigns the
recorded remaining quantity/after revision. It never subtracts on each reopen. An already newer
record is left unchanged. Full-removal receipts suppress old copies. Old-source terrain stays
resident while a later acknowledgement requires cleanup; pickups/merges wait for migration cleanup
so receipt retirement cannot orphan an older spatial copy.

The existing generic player envelope, chunk v3, spatial item v1, world-global envelope, metadata,
backend and frozen generators are unchanged. Only Minecraft's independently versioned pickup-receipt
component writes v2 (before/after revisions and before/accepted/remaining quantities); v1 receipts
still load. The old component lacks the information needed to represent partial transfers safely.

## Deterministic fault evidence

All tests use disposable disk stores and production checkpoint/game codecs. They reopen storage,
decode player/entity state and assert semantic total quantity; migration also asserts one EntityId
in either activation order.

| Interruption | Test |
| --- | --- |
| Full pickup, source checkpoint before player durability | `full_pickup_source_before_player_and_failed_player_checkpoint` |
| Full pickup, actual player write failure | Same test; replaces the players directory temporarily with a file. |
| Partial pickup, player durable before source reduction | `partial_pickup_both_save_orders_and_repeated_recovery` |
| Partial pickup, source-save opportunity first | Same test, opposite order; source reopens with the full stack and old inventory. |
| Partial recovery repeated, including repeated activation | Same test; remainder stays 18 after transferring 12 from 30. |
| Migration, source save first | `migration_source_first_destination_failure_and_destination_before_cleanup` |
| Migration, actual destination write failure | Same test; failure does not acknowledge ownership and source reopens. |
| Destination durable, crash before source cleanup | Same test; both activation orders choose one EntityId at destination. |
| Eight failed successive dirty generations, then successful latest retry | Client `failed_entity_save_generations_retire_and_latest_reopens`; map empties each completion, no failed ownership acknowledgement, final reopened entity/player quantity is conserved. |
| Old player completion after a new partial transfer | `old_player_ack_cannot_authorize_another_partial_transition` |
| Late pre-cleanup source acknowledgement | `late_source_ack_keeps_migration_marker_until_actual_cleanup` |
| Reopen a full-removal receipt, schedule source cleanup and durably prune it | `restored_full_receipt_schedules_loaded_source_cleanup_and_prunes` |
| Move back to source before destination acknowledgement | `migration_return_to_source_retires_superseded_marker` |

Focused validation passed: all 48 runtime tests; all 40 world library tests plus both existing S1
example correctness tests; all eight player-codec tests (including v1 receipt compatibility); all seven
transfer fault tests; the client failed-generation test; affected runtime/Minecraft/client/server
all-target Clippy with warnings denied. Commands ran sequentially with two Cargo jobs and debug
information disabled. Full workspace/R2 acceptance and public CI remain subsequent publication gates.

Second diff review checked source retention, exact player receipt acknowledgement, source-post
acknowledgement, both migration activation orders, late source completion/tombstone retirement,
return migration, eviction residency and both failed-save completion paths. The requested four
findings are addressed by the code and focused matrix above.
