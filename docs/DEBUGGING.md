# Developer diagnostics

See [SCRIPTING.md](SCRIPTING.md) for commands, semantic queries, security and the native-call audit.
F3 retains Overview. `/debug page NAME` selects overview, streaming, world, entities, lighting,
renderer, persistence or scripts. `/debug page entities N` paginates the bounded 64-entry active
subset (eight entries per page). Expensive snapshots refresh every 250 ms. Presentation is capped
at 20 rows, 100 columns and 800 characters; complete bounded values remain available in JSON.

`/debug overlay NAME on/off` controls streaming, entities, collision and target. At most 64 transient
AABBs use the existing depth-tested line pass and never enter meshes/saves. Streaming shows a 5×5
nearby subset: green Visible, yellow Safe, red desired/unready, blue retained, grey outside interest.
Entity markers are magenta and owner-column outlines blue. Collision is yellow, target white.

`just dx-test` verifies the shared console editor/input state and async lifecycle. `just dx-console`
opens, executes and captures the real console before closing. `just dx-overhead` reports mean,
p50/p95/p99/max DX service time for disabled, inactive, page, overlay and scenario configurations,
with 240 event turns per mode and 20 warmup turns excluded. It measures DX CPU work separately
from total software-GPU frame cost. See [DX1_REPORT.md](DX1_REPORT.md) for final evidence.

`just script-bench` reports Rhai version, compile, cached queries, command dispatch, scenario steps
and bounded runaway termination. Existing M4 specialist acceptance modes remain intact.

## Planned DUX1 scope

Current DX1 already supports runtime page/overlay selection through the console. DUX1 is a
planned early stage for discoverable metadata/selection, targeted ChunkPos/EntityId inspection
and demand-driven collection over the same Control Snapshot/query backend. Meshing and
Scenarios/Jobs may use bounded combined views. Inactive views must not scan expensive domains;
active collection uses measured bounded cadence and explicit cost/coverage. C1 later connects
shared runtime settings. See [PRE_M5_AUDIT.md](PRE_M5_AUDIT.md); no menu/registry is implemented
by this planning pass.
