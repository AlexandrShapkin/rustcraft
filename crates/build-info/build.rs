use std::{path::PathBuf, process::Command};

fn git(root: &PathBuf, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let root = manifest
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    for path in [root.join(".git/HEAD"), root.join(".git/index")] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    if let Some(paths) = git(
        &root,
        &["ls-files", "--cached", "--others", "--exclude-standard"],
    ) {
        for path in paths.lines() {
            println!("cargo:rerun-if-changed={}", root.join(path).display());
        }
    }
    let commit = git(&root, &["rev-parse", "--short=7", "HEAD"])
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    let dirty = git(
        &root,
        &["status", "--porcelain", "--untracked-files=normal"],
    )
    .map(|status| if status.is_empty() { "clean" } else { "dirty" })
    .unwrap_or("unknown");
    println!("cargo:rustc-env=RUSTCRAFT_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=RUSTCRAFT_BUILD_DIRTY={dirty}");
    println!(
        "cargo:rustc-env=RUSTCRAFT_BUILD_PROFILE={}",
        std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_owned())
    );
    println!(
        "cargo:rustc-env=RUSTCRAFT_BUILD_TARGET={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned())
    );
}
