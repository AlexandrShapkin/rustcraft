// Cooperative compile jobs, rendered frame progress, then explicit automation/human abort.
for iteration in 0..12 {
    reload_script("dev/inspect_player.rhai");
    if has_capability("render.capture") { wait_frames(1); }
}
overlay("streaming", true);
capture("dx_responsive");
if has_capability("render.capture") { wait_frames(10000); }
else { wait_ticks(10000); }
