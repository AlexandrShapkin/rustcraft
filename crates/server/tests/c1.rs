//! Cross-platform real executable source-precedence and headless configuration acceptance.
use std::{path::Path, process::Command};
#[test]
fn configuration_sources_and_headless_application_pass() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let file = std::env::temp_dir().join(format!(
        "rustcraft-c1-sources-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&file,br#"{"version":1,"values":{"rustcraft:streaming/load_radius":{"type":"Integer","value":5},"rustcraft:streaming/retain_radius":{"type":"Integer","value":7},"rustcraft:diagnostics/sample_interval_ms":{"type":"DurationMs","value":300}}}"#).unwrap();
    for mode in ["--config-report", "--config-smoke"] {
        let output = Command::new(env!("CARGO_BIN_EXE_rustcraft-server"))
            .current_dir(root)
            .args([
                mode,
                "--set-config",
                "rustcraft:diagnostics/sample_interval_ms=400",
                "--config-file",
            ])
            .arg(&file)
            .env("RUSTCRAFT_STREAM_RADIUS", "6")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if mode == "--config-report" {
            let s: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(
                s["settings"]["rustcraft:streaming/load_radius"]["effective"],
                6
            );
            assert_eq!(
                s["settings"]["rustcraft:streaming/load_radius"]["source"],
                "Environment"
            );
            assert_eq!(
                s["settings"]["rustcraft:streaming/retain_radius"]["source"],
                "UserFile"
            );
            assert_eq!(
                s["settings"]["rustcraft:diagnostics/sample_interval_ms"]["source"],
                "Cli"
            );
        } else {
            assert!(
                String::from_utf8_lossy(&output.stdout)
                    .contains("C1_HEADLESS pass settings=17 native/control/readback=6")
            );
        }
    }
    std::fs::remove_file(file).unwrap();
}
