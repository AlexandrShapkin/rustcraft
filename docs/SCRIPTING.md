# Developer scripting (DX1)

DX1 is engine tooling ahead of inactive M5. It does not replace native simulation or authorize
third-party native code. The public Control API version is 1; `control_version()` reports it.

`rustcraft-control` owns immutable snapshots, semantic actions, command registry/parser, sources,
capabilities, fixed-step gate, event ring and explicit scenario program. It depends on Engine/Agent
values and serialization, not Minecraft, graphics or Rhai. `rustcraft-scripting-rhai` is the leaf
interpreter/console/scenario adapter. `minecraft-b173::control` registers `tp` and `setblock` and
adapts the transitional simulation. Client/server own composition, workers and capture presentation.

Release archives include only the project-owned allowlisted example scripts and this guide; local scripts
are not automatically packaged. Run developer binaries from the directory containing scripts/.

Start explicitly with `just dev-client`. Backquote toggles the console; Escape closes it;
F10 cancels the scenario. Arrows/Home/End/Backspace edit, Up/Down browse history, PageUp/PageDown
scroll output and Tab completes command names. Ordinary release/debug startup enables no console
and executes no scripts. `--scenario PATH` explicitly opts into scenario tooling.

```text
/help
/inspect
/debug page streaming
/debug overlay collision on
/pause
/step 1
/resume
/capture frontier
/script dev/inspect_player.rhai
/scenario scenarios/dx_smoke.rhai
/reload keep_scope
```

Non-slash lines are Rhai. `let x = 40;` followed by `x + 2` returns 42. Each REPL owns a persistent
Scope; scripts/command functions/scenarios use separate scopes. `/reload reset_scope` explicitly
resets the REPL; default keep_scope retains it. Reloading files never changes a running scenario's
already-built program. Command-file state is stateless. No automatic arbitrary state migration exists.

`/script help` or `control_help()` lists the adapter functions. Position values are copied arrays;
semantic block keys are strings. No script receives storage references or dense durable IDs.
Native commands and Rhai actions converge at `control::execute`, then the composition Host.
SetBlock uses the existing Game API CommandBuffer; the simulation application hook maintains
lighting, section/render and persistence dirty tracking. Slash commands and script evaluations queue at most
1,024 actions and drain one per event turn, preserving source order without a bulk mutation stall. A failed Rhai evaluation discards its queued mutations; commands already applied
by an earlier evaluation are not rolled back.

First-party registrations (not generic Minecraft policy):

```text
/tp 8.5 70 8.5
/setblock 0 10 0 minecraft_b173:stone
```

Teleport requires an available destination column. Setblock requires a resident column and the
first-party vertical range; unknown keys fail. Give, gamemode, fill and time commands are not added.

## Command files and reload

`scripts/commands/where.rhai` illustrates `command_spec()` returning an inspectable map containing
id/name/usage/help/capability, and `command(args)` returning a value or queuing one action. Commands
register as ordinary registry handlers, including scenario dispatch. Command sessions receive the
read-only profile plus their declared capability. Metadata changes require restart; code changes
compile and validate before replacing both AST and handler. Invalid edits retain the old generation.
A portable 500 ms poll and `/reload` check registered files. New command files require restart.

Script access resolves inside the canonical `scripts/` root, including symlinks. Traversal,
outside absolute paths and non-Rhai files are rejected; reads and source size are capped at 64 KiB.
The accepted roots are commands/, scenarios/ and dev/ within that tree. Rhai has no raw filesystem,
network or process API, and imports/dynamic eval are disabled. Source access is a host tooling
operation, not an interpreter filesystem binding. Never auto-run downloaded/world-local scripts.

## Limits and native-call audit

Rhai is pinned to 1.26.1, standard AST interpretation with the sync feature, not Grain. A runtime
owns its configured Engine and callback bridge; a session owns Scope and diagnostics. There is no
global mutable Engine/Scope. AST compilation/cache and function calls use the checked crate APIs.

Defaults: 50,000 operations; 32 call levels; 32 expression nesting levels in global/functions;
128 variables/functions; 8 KiB strings; 1,024 array elements; 128 map entries. Engine limits remain
enabled. Operation override is clamped to 100–1,000,000; execution deadline to 1–50 ms (default 10).
The progress callback records operation count and aborts after the monotonic deadline. Rhai cannot
preempt a blocking registered Rust call. Compilation is source/depth bounded, not progress-preempted.

Every registered native function is immediate and bounded: version/help/capability inspect static
values; tick/player_position copy snapshot values; action functions validate and append to a capped
queue; scenario functions append at most 1,024 Rust steps; print/debug retain at most 128 bounded
messages. None performs generation, saving, networking, process creation or GPU waits. The host
capture request is deferred to the renderer. DX GPU mapping is asynchronous; PNG encoding and writes are worker jobs. Legacy specialist captures retain their synchronous path.

Capabilities: world.read/write, player.read/control, entity.read, debug.inspect/capture/pause/configure,
script.load. Persistence administration is not granted or exposed. FutureChat gets the explicit
read-only profile, never developer privileges. REPL/scenario developer profiles are explicit local
opt-ins; command sessions get their declared capability. Denials include capability and source.
WASM remains the future untrusted executable-mod runtime; this is not a claim of a general host sandbox.

## Scenarios and artifacts

```rhai
pause();
checkpoint();
assert_tick_delta(0);
assert_player_unchanged();
step(1);
wait_tick_delta(1, 5000);
assert_tick_delta(1);
resume();
wait_ticks(2);
capture("after_step");
```

The interpreter builds a Rust-owned program. The scheduler advances one step per event turn;
it never suspends Rhai or sleeps inside the event loop. Actions, slash commands, absolute tick
predicates, checkpoint-relative exact tick assertions, relative tick/frame waits, idle-domain waits, player-near/block/tick assertions, semantic
movement and captures are supported. Explicit waits are clamped to 30 seconds; every step has a
30-second safety bound and the run a 120-second bound. Frame waits fail headlessly. Movement is
AgentIntent, never hidden teleport. While active, the scenario lease overrides human/StreamAuto
intent; end/abort releases held intent and returns ownership. Cancellation is a structured terminal
result. Pause stops fixed simulation only; events, rendering and asynchronous streaming/persistence
workers and publication continue. Step N consumes exactly N fixed ticks in batches of at most eight.

`just scenario-headless` and `just scenario-client` use the same dx_smoke.rhai. Result JSON includes
schema/scenario/run identity, pass/fail/error/timeout/cancelled, failed step/assertion, tick/time,
capabilities, script generation/hash and error. `DX_RESULT` prints the stable result-file location.
Failure bundles under ignored target/test-runs include result/state/player/entities, available
streaming/lighting/meshing/renderer/persistence domains, script diagnostics, bounded console/scenario
logs and recent-events JSONL. Unavailable domains remain null and are not emitted as fabricated
files. Graphical runs also capture frame.png automatically through the existing render path.
GL surfaces without COPY_SRC use the same passes on a copyable attachment before normal presentation.
Observed final bundles: about 6–8 KiB headless and 750–850 KiB graphical at 1280x662.

The event ring holds at most 128 entries/32 KiB, with 512-character messages. It records command/scenario/compile/job/capture lifecycle transitions and tooling errors.

## Background work and jobs

File reading, AST parsing/compilation and command metadata validation run on one `dx1-worker`
thread with 16-slot request and publication queues. There is no thread per edit. Workers own
only canonical script roots, copied requests, ASTs and serialized diagnostic values, never
simulation/renderer references. Shutdown stops admission, discards queued work and joins the
worker; the pending-work shutdown test verifies bounded termination. Source is at most 64 KiB,
regular-file only. Canonical aliases are rejected by asynchronous loading; use the approved
canonical path. Filesystem replacement by another privileged host process is outside this local
trust model; Rhai cannot perform filesystem changes.

Each compile request has script identity, request generation and JobRef. Only the latest requested
generation can publish. A stale/cancelled reply cannot update the AST, registry handler, scenario
or queued mutations. Valid candidates replace AST and command handler together on the owning
thread. Invalid candidates retain the working generation and expose a candidate error. Automatic
polling checks at most four loaded files per 500 ms, rotating through the bounded cache; new-file
command discovery remains restart-only optional breadth. A running scenario retains its built
program. Scope reset/retention rules remain explicit.

`reload_script("dev/inspect_player.rhai")` in a scenario requests a real compile job and becomes
an inspectable `WaitJob` step with a 30-second timeout. The scheduler yields while the job is
Pending, continues on Complete, and records Failed/Cancelled explicitly. `scripts().jobs` exposes
stable job IDs, owners and status; this is not a mutable worker reference. Job owners distinguish
console, internal reload, scenario compile/run, required bundle and capture work. Abort cancels
scenario-owned work; late replies are ignored. Required failure evidence is allowed to finish.
The store retains at most 32 old terminal jobs plus bounded outstanding work (128 hard capacity),
pruning at admission. Every lifecycle has request/publication/terminal events in the bounded ring.

Failure-bundle JSON encoding and file writes run on the same worker. Exit waits for bundle
completion with a 30-second bound and explicitly reports partial diagnostic failures. Graphical
DX captures use existing rendering/copy passes with asynchronous map callbacks and nonblocking
`Poll`; no GPU `Wait` or blocking receive occurs on the DX path. The renderer copies completed
mapped RGBA data on its owning thread (bounded by window size), then the worker writes PNG.
Terminal scenario state is refreshed in state/scripts/result files together; expensive engine domains
retain their documented snapshot cadence. Exit waits for the required PNG job. Legacy specialist render captures retain their existing
synchronous diagnostic path. Capture directories are unique across reruns.

## Semantic query coverage

`player()`, `world()`, `streaming()`, `entities()`, `entity("32-digit-EntityId")`,
`persistence()`, `lighting()`, `meshing()`, `renderer()` and `scripts()` return immutable
semantic values. Missing domains are Rhai unit/JSON null, never invented zero metrics. Player
includes position, orientation, chunk, mode, held/selected item, velocity and grounded state.
Entities are limited to 64 active entries; entity lookup filters that set using stable identity.
`block_at(x,y,z)` reads the 27-block neighborhood around the player from the current immutable
query snapshot; outside it reports unavailable. Scenario `assert_block` resolves arbitrary
available positions through the live Host query. No dense BlockId is a durable script result.
Client expensive diagnostics refresh at 250 ms. Headless omits graphical/worker domains it cannot
provide. Domain conversion caps depth at eight, maps at 128 and arrays at 1024.

## Registered native-function boundedness inventory

| Functions | Classification / bound |
| --- | --- |
| control_version, control_help, has_capability | Immediate static/value lookup. |
| tick, player_position | Immediate immutable values, capability checked. |
| player, world, streaming, entities, entity, persistence, lighting, meshing, renderer, scripts, block_at | Immediate bounded snapshot conversion/lookup; no live-world scan or filesystem. |
| pause, resume, step, teleport, set_block, capture, debug_page, overlay | Validate capability/value and append to 1024-entry command/step queue. |
| command, wait_ticks, wait_frames, wait_tick, wait_tick_delta, wait_idle, assert_tick, assert_tick_delta, assert_player_unchanged, assert_block, assert_near, move_player, checkpoint | Build at most 1024 Rust steps; no interpreter wait. |
| reload_script | Build a Rust job-request step; worker I/O/compile happens later. |
| console_open, console_line | Build steps that exercise the same editor/dispatcher as platform input, retaining scenario capabilities. |
| assert_true, assert_eq (integers), fail | Immediate bounded assertion/error with expected/actual values. |
| print/debug callbacks | At most 128 messages of 512 characters per evaluation. |
| progress callback | Immediate operation accounting/deadline comparison. |
| Raw filesystem, network, process, environment, mutable world/ECS, sync generation/save/GPU wait | Forbidden / not exposed. |

`just dx-test` covers editor/dispatch/REPL recovery, async reload/stale/shutdown, cooperative job
wait/cancellation and a 40-cycle lifecycle soak. `just dx-console` renders the actual console.
`just dx-overhead` measures five configurations through the actual client service and writes
`target/dx-overhead.json`. The responsiveness scenario can be run with explicit automation abort:
`rustcraft-client --devtools --scenario scripts/scenarios/dx_responsive.rhai --dx-abort-after-frames 100`.
These flags execute only explicitly trusted project tooling; none is normal-startup autoexec.

Worker queue admission failures become terminal Failed jobs, never orphan Pending entries.
A focused full-queue test proves admitted/pending accounting and drainage. PNG/backend failures
update machine result and write capture-error.json when the bundle directory is writable; stdout/
stderr explicitly reports partial evidence otherwise. Required evidence jobs have a 30-second
completion bound. The CPU copy from a completed GPU map is the unavoidable bounded synchronous
render-ownership operation; filesystem/PNG work is not executed there.

## DUX1 shared diagnostic queries

`debug()`, `chunk_inspection()` and `entity_inspection()` read the same demand-driven samples as
native developer pages. Explicit `/debug chunk X Z SECTION_Y`, `/debug entity HEX_ID` and
`/debug ui open/close/next/previous/tab/activate/help/target/entity` select transient diagnostic state.
Scenario `assert_debug(PATH, BOOL/STRING/INT)` checks bounded current debug metadata. Existing
`entity(STABLE_ID)` retains its bounded active-list contract. Data can be sampled/stale; provider
age, availability and coverage are explicit. These additions grant no filesystem, network, process
or mutation capability. See [DEBUGGING.md](DEBUGGING.md) for focus/cadence and [DUX1_REPORT.md](DUX1_REPORT.md).

## C1 configuration adapter

Trusted local Rhai uses config_get(key), config_describe(key), config_set(key, string_value), config_reset(key). Queued mutation keeps validation/capabilities/boundaries; another read in the same evaluation sees its original immutable snapshot. Scenarios use assert_config(key, integer) and assert_config_field(key, field, integer_or_string). config.read/write/persist do not derive privilege from a source label. Diagnostic cadence defaults250ms but effective C1 configuration now controls it; Settings remains demand-driven. See [CONFIGURATION.md](CONFIGURATION.md).


UX1 console: `/` enters COMMAND with a slash prefilled; Backquote enters empty RHAI.
`/help <command>` reads registry metadata; Tab completes commands, subcommands and config keys.
The console uses extended Unicode grapheme editing and IME commit/preedit where supported.
These usability paths preserve existing Control capabilities and trusted-local Rhai boundaries.

P1 adds capability-checked `presentation()` shared diagnostics. Its `statistics_columns` labels
bounded numeric summary tuples; `authoritative_transform` and `presentation_transform` are explicitly
different. Existing `player()` remains authoritative. No physical scanout or new mutation privilege.
