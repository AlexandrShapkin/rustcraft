//! Compile-time product and source identity for executable diagnostics.

/// Human-readable product version and build identity.
#[must_use]
pub fn identity() -> String {
    let dirty = match env!("RUSTCRAFT_BUILD_DIRTY") {
        "dirty" => "-dirty",
        _ => "",
    };
    format!(
        "RustCraft {} ({}{}, {}, {})",
        env!("CARGO_PKG_VERSION"),
        env!("RUSTCRAFT_BUILD_COMMIT"),
        dirty,
        env!("RUSTCRAFT_BUILD_PROFILE"),
        env!("RUSTCRAFT_BUILD_TARGET"),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn build_identity_contains_product_and_platform_fields() {
        let identity = super::identity();
        assert!(identity.starts_with("RustCraft 0.1.0-alpha.1 ("));
        assert!(identity.contains(env!("RUSTCRAFT_BUILD_PROFILE")));
        assert!(identity.contains(env!("RUSTCRAFT_BUILD_TARGET")));
    }
}
