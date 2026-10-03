// Same semantic program in headless and graphical composition.
command("/help");
command("/inspect");
pause();
checkpoint();
if has_capability("render.capture") { wait_frames(3); }
assert_tick_delta(0);
assert_player_unchanged();
step(1);
wait_tick_delta(1, 5000);
assert_tick_delta(1);
step(3);
wait_tick_delta(4, 5000);
assert_tick_delta(4);
let position = player_position();
let x = position[0].floor().to_int();
let z = position[2].floor().to_int();
set_block(x, 10, z, "minecraft_b173:stone");
assert_block(x, 10, z, "minecraft_b173:stone");
debug_page("scripts");
overlay("collision", true);
resume();
wait_tick_delta(6, 5000);
capture("dx_smoke");
