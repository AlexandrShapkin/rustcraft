# Networking direction

Java protocol compatibility is not required.

The long-term leading transport candidate is QUIC, subject to measurement and library maturity.
The protocol should distinguish durable/state-sensitive data from supersedable real-time state.

Likely reliable/state-sensitive examples:

- inventory/crafting transactions;
- commands/chat where delivery matters;
- authoritative gameplay transitions;
- content manifests and package metadata.

Likely replaceable examples:

- position/orientation snapshots;
- transient animation/effects.

QUIC streams are reliable; unreliable semantics require an appropriate datagram mechanism rather
than treating a reliable stream as unreliable.

Design eventually for batching, interest management, compact typed messages, deltas where useful,
buffer reuse, client prediction/reconciliation and authoritative server state.

Do not implement world sharding before the single-process server and protocol have real workloads.
