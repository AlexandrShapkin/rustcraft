# P1 — Frame pacing and presentation

Starting baseline: `f8b1e9b4a4f2846860fb542eda7d20f4099a988e`, **Record RSM1 autonomous
acceptance**, verified against origin/main. RSM1 implementation/closeout CI runs 37228702009 /
37229222678 passed Ubuntu and Windows. P1 local implementation and public acceptance are recorded
below; S1 and subsequent stages remain inactive. Measurements: 2026-10-05.

## Timing architecture and experiment

Authority remains `Simulation::step(intent, 0.05)` and `FixedStepClock::DT = 0.05`; at most five
catch-up ticks execute per event turn. Whole excess ticks are dropped with fractional remainder
retained. No variable-delta gameplay, bot observation, persistence, mining or collision change.

The application uses winit 0.30.13's event loop: `AboutToWait` advances the fixed accumulator,
services streaming and requests redraw; `RedrawRequested` produces the frame. Normal operation has
no application sleep/frame-cap loop and no explicit Poll override. Sleeps found in source belong
to diagnostic/headless travel/shutdown paths. Surface policy is unchanged: prefer FIFO, configured
through C1's startup-only `rustcraft:renderer/present_policy=PreferFifo` (Default source). This
surface supports only FIFO; maximum frame latency remains two. No new C1 knob or limiter is added.

The clocks/events are intentionally distinct:

- monotonic `Instant` timestamps: native mouse ingestion, actual tick begin/completion, redraw
  requests/events, render start/camera extraction, acquire begin/end, renderer preparation,
  queue-submit call, `SurfaceTexture::present()` call and renderer-call completion;
- authoritative time: integer simulation ticks, unaffected by the visual state;
- presentation fraction: fixed-clock remainder / 50 ms, clamped to [0,1].

Intervals are rolling 512-sample scalar rings. Summaries include count/mean/p50/p95/p99/max,
population standard deviation, refresh-relative buckets (<0.75, 0.75–1.25, 1.25–1.75,
1.75–2.5, >=2.5), cadence opportunities >1.5 configured-refresh intervals and long intervals
>2.5. A 60 Hz target is 16.667 ms, so the opportunity threshold is 25 ms; at 144 Hz it would
be 10.417 ms. Tests prove refresh-relative classification and unavailable-target behavior.
Refresh changes reset cadence rings; focus/resize/reopen/teleport/hitch rebases discard interval
bridges and pending input timestamps. A simulated ten-second suspension is not a normal sample.

These are **application** observations. `present()` schedules presentation; neither it nor
`request_redraw` timestamps physical scanout. CPU renderer-call wall time can include driver waits
and is not GPU execution time. Input-to-camera means oldest pending input timestamp to the camera
selected at render start, not input-to-photon or GPU completion. No physical feedback/VRR provider
is available. Configured monitor: winit DP-6, **60000 mHz**, scale 1, windowed Wayland. Compositor
identity unavailable. Adapter: **llvmpipe (LLVM 23.1.1, 256 bits), OpenGL**. Owner AMD/Vulkan and its
physical display cadence are inaccessible. This closes application-side correctness with an
explicit conditional representative-display validation, not an AMD performance claim.

`--p1-acceptance` / `just p1-client` creates a nonce disposable world and a semantic Stone walking
pad, waits for the Safe/Visible core and drained current mesh/dirty work (normal frontier lighting
may continue), then runs six **20-second** phases: stationary, pan, walk, walk+pan, stop, streaming.
Pan uses 0.65 rad/s synthetic mouse input through the same native ingestion method and ordinary
LocalHumanController consumption; walk bounces at pad edges; streaming leaves the pad with jump
intent. No fake renderer or display timestamp. Duration/configuration/surface/backend match across
runs. Each phase resets scalar history; motion records are not mixed with capture frames. The
first baseline stationary tick count includes a five-tick warm-up boundary catch-up; other motion
phases measure approximately 20 TPS. Effective C1 sources/settings are included in raw receipts.

Pre-fix baseline was recorded **before** interpolation/look changes, in ignored
`target/p1/baseline.json`; the instrumentation patch was preserved separately. Accepted motion is
`target/p1/accepted-motion.json`; rejected callback experiment `target/p1/post-notify.json`.
[Canonical distributions and configuration](P1_RESULTS.json) contain bounded summaries, not raw
frame timestamps. The developer-enabled/UI-inactive experiment excludes concurrent RustCraft
compilation. Cross-run software-GPU/system variation remains a limitation.

## Measured causes and bounded decision

Proven: rendering read fixed-tick position and orientation directly. Pan repeated a camera on
**44.13%** of render transitions; walking repeated a position on **36.10%**; walking+pan repeated
both on **36.62%**. High production FPS therefore did not mean equally frequent distinct motion.
Mouse deltas waited for the next tick before affecting the camera.

Rejected: no normal double limiter was found; surface acquisition was typically ~0.06–0.07 ms,
so acquire blocking does not explain these state repetitions. FIFO itself was not changed or
claimed defective. Software CPU/driver work and streaming produce long application intervals;
this is not proof of the owner's AMD/compositor/VRR/physical-scanout behavior.

A documented `Window::pre_present_notify()` experiment was measured after the first fix. On this
Wayland/software surface it worsened pan p50/p95/p99 to 34.855/54.063/64.320 ms, jitter 8.667 ms.
The identical run without it returned to 28.323/36.920/41.858 ms, jitter 4.692 ms. **The experimental
notification change was removed.** No speculative platform scheduling repair remains. Whether
notification benefits representative hardware needs separate display evidence; this software
result does not establish a universal winit policy. No mode, redraw scheduling or limiter change
is included in P1.

## Transient presentation model

`client::presentation::ViewState` owns two small transform samples, with no world/GPU references.
Actual fixed ticks record before/after authority; zero-tick frames interpolate previous/current
translation using alpha; multiple catch-up ticks leave the final two samples. Camera eye offset,
projection and package targeting choices remain in client composition. No interpolated position
is written back, persisted, exported as a bot observation or treated as a network identity.

Translation uses traditional one-tick-delayed interpolation: approximately **50 ms behind the
fixed simulation timeline**; relative to the latest tick sample, lag is `(1-alpha)*50 ms`.
Recorded alpha distributions and the unit translation fixture make that delay explicit. Authority
state age is measured separately, not mislabeled as the visual timeline's age. Mouse orientation
is deliberately asymmetric: latest authoritative yaw/pitch plus unconsumed human look delta,
with the same existing sensitivity and pitch clamp. Once `next_intent()` drains the delta and the
tick consumes it, authority already includes it and the preview becomes zero: no double apply,
snap-back or overshoot. Leased scenarios, inventory and developer focus do not receive human preview.
No yaw interpolation crosses a wrap; the continuous authoritative/pending angle is used directly.
Shortest-path diagnostic yaw deltas are tested at 359° -> 1°.

Pause displays exact authority without positional drift; a single step snaps coherently and
advances exactly once. Resume, Control teleport, world load, external out-of-tick reposition,
focus/resize and dropped-tick hitches rebase samples. Alpha never extrapolates. Tests cover zero /
one / multiple ticks, invalid alpha protection, real Simulation consumption of multiple mouse
events, pitch clamping, shortest wrap, teleport, pause/step and final-two-tick catch-up. The graphical
harness also pauses across rendered frames, steps once, resumes, teleports and captures the page.

HUD selection uses a read-only raycast from the **shown camera** with the existing targetability /
range policy. Break/place/mining still resolve through authoritative simulation rules after intent
consumption. This preserves visible outline alignment without granting visual state authority;
translation delay can create a small temporary visual/authoritative target difference. Crack
geometry stays on the actual authoritative mining block and uses the same displayed camera matrix;
held-item/crosshair paths remain camera-relative. Existing ray/face/placement/crack/selection
regressions remain required. Dropped-item/entity translations remain tick-stepped: not a general
ECS/prediction rewrite. Water-medium classification remains authoritative and may transition one
presentation tick apart at a boundary; no M3/world-policy redesign is claimed.

## Matched results

Milliseconds; render-start rolling windows contain up to 512 samples. Duplicate fractions use the
whole phase's render transitions. No FPS/cadence improvement is inferred from a different phase.

| Phase | Before p50/p95/p99 | After p50/p95/p99 | Jitter before -> after | Shown duplicate fraction before -> after |
| --- | --- | --- | --- | --- |
| Stationary | 28.355 / 41.044 / 52.206 | 24.904 / 33.480 / 37.840 | 6.231 -> 4.487 | 100% -> 100% (expected) |
| Pan | 26.923 / 35.975 / 38.775 | 28.323 / 36.920 / 41.858 | 4.208 -> 4.692 | camera 44.13% -> 0% |
| Walk | 31.902 / 53.820 / 64.005 | 33.750 / 58.157 / 68.488 | 9.840 -> 11.079 | position 36.10% -> 0% |
| Walk+pan | 29.464 / 39.208 / 43.609 | 32.655 / 47.632 / 56.491 | 4.641 -> 6.848 | both 36.62% -> 0% |
| Stop | 37.651 / 49.912 / 56.020 | 41.766 / 56.239 / 65.226 | 6.449 -> 6.591 | position ~100% -> ~100% (expected) |
| Streaming | 43.600 / 69.009 / 88.614 | 44.643 / 64.720 / 84.527 | 12.551 -> 12.180 | position 12.96% -> 0% |

Pan cadence-opportunity fraction: **72.27% -> 78.13%** on this software surface; not physical
missed vblanks. Application cadence is not proven improved. Pan production: 717 -> 702 frames /
20 s; walking 616 -> 577; no average-FPS gain is claimed. Pan yaw-delta coefficient of variation
**0.960 -> 0.161**; walking displacement CV **0.725 -> 0.325**, despite variable frame intervals.
Pan input-to-camera extraction mean/p50/p95/p99: **26.619/30.903/44.011/53.290 ->
5.279/4.925/8.065/13.536 ms**. State-age p50/p95/p99 in pan:
**6.239/38.183/51.823 -> 6.389/38.508/47.169 ms**; the fixed authority remains comparable.
Motion phases remain ~19.96–20.04 TPS. Motion acceptance requires a >20 FPS fixture and at most
two boundary/rest duplicate shown transforms in each continuous pan/walk/walk+pan phase; stationary
and paused repetitions are correct, and software cadence is not forced to 60 FPS.

## Shared diagnostics, lifetime and overhead

Semantic view `rustcraft:debug/page/presentation` is registered through DUX metadata (Medium cost),
available in F4, `/debug page presentation`, and Rhai `presentation()` with `debug.inspect`.
`Snapshot.presentation` is distinct from authoritative `Snapshot.player`; absent headless provider
is explicitly null/unavailable. One provider publishes the bounded ledger to UI, Control, Rhai,
captures and failure bundles. A UI/Rhai readback test leaves provider collections unchanged at
1,000 after both reads. Inactive provider queries collect **zero** samples. C1 diagnostic cadence
remains authoritative; opening the page sorts cached scalar rings at that cadence, not a second
world scanner. No capability, source authentication or configuration mutation privilege changed.

At most 16 x 512 f64 samples (~64 KiB payload), counters and transform/Instant scalars are retained.
No texture, SurfaceTexture, encoder, GPU buffer, Arc/world snapshot or entity map enters history.
The disabled normal client records no presentation ring samples; devtools-enabled inactive pages
retain cheap scalar observations so opening the page has useful recent data. Existing RSM1 queues,
generation ownership and fixed UX1 text allocation are unchanged. Server gains no winit/wgpu/render
or client presentation-clock dependency.

Overhead and final validation results are appended below after the final gates. Full-ring, real
service costs are distinguished from the one-frame forced-provider and scalar-recorder micro-tests;
none are universal thresholds.

## Remaining conditions and acceptance

No physical input-to-photon, physical scanout, AMD/Vulkan cadence, VRR or multi-monitor hardware
claim. Configured refresh discovery/rebase is tested at the logic level. Real minimize/monitor moves
and owner-display visual acceptance remain conditional. The software surface does not meet a 60 Hz
frame budget; streaming/CPU/driver long frames remain visible. No broad renderer optimization,
remote prediction/reconciliation, entity-wide interpolation, surface-policy framework, persistence
redesign or next-stage work is included. RSM1/C1/UX1/DUX1 remain CLOSED. No version/tag/release.

PM5-003 can close after final local/public acceptance because the measured fixed-state duplication
and tick-gated local camera are repaired, input is consumed exactly once, authority and deterministic
stepping remain unchanged, discontinuities are explicit and timing/physical-provider limitations
are recorded. Closure does not claim physical refresh synchronization or a throughput speedup.

## Local acceptance record

`cargo fmt --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, strict
all-target/all-feature Clippy and `just ci` passed. The normal CI workflow ran **304 tests, 304 passed,
one opt-in test skipped**. `just dx-test`, `dx-console`, `dx-overhead`, `sample-game`, `rsm1-test`,
`world-stream-bench`, `render-camera-motion`, `config-smoke`, `dux-client`, `ux-client` and graphical
`--c1-acceptance` passed. These include existing collision/raycast/selection/face/mining, deterministic
headless/Bot/frozen-hash, entity/pickup/save, stale-token/pressure and Unicode/focus regressions.
Final `just p1-client` passed after code-quality changes: receipt `target/p1/34.json`, status complete,
five graphical check stages, pan/walk+pan/streaming zero shown-camera duplicates; walk one boundary
repeat over 670 transitions (0.15%). Final capture
`target/captures/p1_final-34-1791167499761513193/frame.png` was inspected: readable bounded timing
page, correct background/outline/camera and explicit scanout unavailability. Captures remain ignored.
The matched comparison table uses the earlier accepted isolation run; this final run independently
reproduces the repair with the final validator/rounded UI and explicit last-render authority tick.
The expensive full RSM1 route/travel campaign is not repeated: only transient presentation and
scalar instrumentation change; no production streaming/residency/persistence ownership changes.

Real-surface DX service probe: 220 retained samples per 240 event-turn mode, release build,
1280x720 llvmpipe. Values are microseconds (whole DX service/text, not isolated GPU time):

| Mode | Mean | p50 | p95 | p99 | Max | Presentation collections |
| --- | --- | --- | --- | --- | --- | --- |
| Disabled | 3.846 | 3.841 | 4.819 | 5.588 | 5.727 | 0 |
| Overview / inactive | 507.716 | 81.016 | 2592.673 | 3395.855 | 4013.328 | 0 |
| Presentation page | 193.116 | 75.430 | 736.204 | 1034.079 | 1493.710 | 36 |
| Scenario / Presentation inactive | 988.645 | 954.879 | 1428.198 | 1685.286 | 2011.518 | 0 |

Scalar recorder micro-test (10,000 calls, debug): disabled mean/p50/p95/p99/max
**2.574/1.956/4.820/5.308/43.302 µs**, enabled **4.256/4.749/5.169/7.054/80.528 µs**.
One-frame forced provider summary (1,000 calls) active **88.262/85.626/129.557/165.176/238.650 µs**;
inactive **4.112/4.749/5.238/5.308/36.318 µs**. The active micro-test has a one-frame cache, not
512 populated samples; real page service above is the representative full-ring observation.
P1 scenario recording uses enabled scalar recording and six bounded summary extractions; it adds
no independent scan/provider subscription. These observations are diagnostic, not speed targets.

`cargo machete` clean. `cargo audit`: no known vulnerabilities; three unchanged allowed
unmaintained warnings: paste RUSTSEC-2024-0436, smartstring RUSTSEC-2026-0249, ttf-parser
RUSTSEC-2026-0192. No dependency/lockfile changes. `cargo tree -p rustcraft-server --edges normal`
contains no render/client/winit/wgpu dependencies. No frozen hashes, fonts/licenses, version,
release, tag, M5 protocol or later-stage files were altered.
