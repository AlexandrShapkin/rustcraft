// Small production-client smoke of the shared persistence observations.
pause();
command("/debug ui open");
debug_page("persistence");
wait_frames(3);
let p = persistence();
assert_true(p.oldest_dirty_ms >= 0.0);
assert_true(p.queue_wait_total_us >= 0);
assert_true(p.stale_save_acks >= 0);
assert_true(p.column_io.application_write_bytes >= 0);
assert_true(p.column_io.durability_envelope_us >= 0);
console_line("persistence()");
console_open(false);
command("/debug ui close");
debug_page("persistence");
if has_capability("render.capture") {wait_frames(3);capture("s1_persistence");wait_frames(3);}
command("/debug ui close");
resume();
wait_ticks(2);
