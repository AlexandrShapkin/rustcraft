// Real console editor/dispatch/REPL, shared with platform input. No physical keyboard dependency.
pause();
console_open(true);
console_line("/help");
console_line("/commands");
console_line("/inspect");
console_line("player()");
console_line("let x = 40;");
console_line("x + 2");
console_line("assert_eq(x + 2, 42)");
console_line("let broken = ;");
console_line("/missing-command");
console_line("x + 2");
if has_capability("render.capture") { wait_frames(3); }
capture("dx_console_open");
if has_capability("render.capture") { wait_frames(3); }
console_open(false);
reload_script("dev/inspect_player.rhai");
resume();
wait_ticks(2);
