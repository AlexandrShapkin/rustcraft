// Real surface selector, shared query providers, transient overlays and focus ownership.
move_player(1.0,0.0,2);
pause();
checkpoint();
command("/debug ui open");
assert_debug("selector_open",true);
assert_debug("registered_views",18);
assert_debug("selector_input_focus",true);
command("/debug ui next");
command("/debug ui activate");
command("/debug ui help");
if has_capability("render.capture") {wait_frames(3);capture("dux1_selector");wait_frames(3);}
for cycle in 0..4 {
    command("/debug ui open");
    for page in ["overview","streaming","world","entities","lighting","meshing","renderer","persistence","scripts","chunk","entity"] {debug_page(page);wait_frames(2);}
    for name in ["streaming","entities","collision","target","chunks","selected_entity"] {
        overlay(name,true);wait_frames(2);
        overlay(name,false);wait_frames(1);
        overlay(name,true);wait_frames(2);
        overlay(name,false);wait_frames(1);
    }
    command("/debug ui target");
    wait_frames(2);
    command("/debug ui entity");
    wait_frames(2);
    assert_debug("selected_entity_present",true);
    command("/debug ui close");
    assert_debug("selector_open",false);
    assert_debug("geometry_count",0);
    assert_debug("registered_views",18);
}
assert_tick_delta(0);
assert_player_unchanged();
console_open(true);
console_line("assert_true(entities().len > 0)");
console_line("chunk_inspection()");
console_line("entity_inspection()");
console_open(false);
command("/debug ui open");
debug_page("entity");
overlay("selected_entity",true);
if has_capability("render.capture") {wait_frames(3);capture("dux1_entity");wait_frames(3);}
command("/debug ui target");
overlay("chunks",true);
if has_capability("render.capture") {wait_frames(3);capture("dux1_chunk");wait_frames(3);}
command("/debug ui close");
overlay("chunks",false);
assert_debug("geometry_count",2);
overlay("selected_entity",false);
assert_debug("geometry_count",0);
resume();
move_player(1.0,0.0,2);
wait_ticks(2);
pause();
assert_debug("selector_open",false);
