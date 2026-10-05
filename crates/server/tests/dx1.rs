//! The existing Ubuntu/Windows workspace test gate validates shipped scripts without graphics.
use std::{path::Path, process::Command};

#[test]
fn shipped_scripts_compile_and_shared_headless_scenario_passes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for arguments in [
        ["--script-check", "scripts"],
        ["--scenario", "scripts/scenarios/dx_smoke.rhai"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rustcraft-server"))
            .current_dir(root)
            .args(arguments)
            .output()
            .expect("launch headless developer tooling");
        assert!(
            output.status.success(),
            "{arguments:?}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(if arguments[0] == "--script-check" {
            "SCRIPT_CHECK 9 scripts"
        } else {
            "DX_RESULT "
        }));
    }
}
