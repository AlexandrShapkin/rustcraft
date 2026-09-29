//! Server-defined content metadata and deterministic integrity primitives.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceId(pub String);

#[derive(Debug, Clone)]
pub struct LocalResourceResolver {
    root: PathBuf,
}

impl LocalResourceResolver {
    #[must_use]
    pub fn from_environment(default_root: impl Into<PathBuf>) -> Self {
        Self {
            root: std::env::var_os("RUSTCRAFT_TERRAIN_TEXTURE")
                .map(PathBuf::from)
                .unwrap_or_else(|| default_root.into()),
        }
    }
    #[must_use]
    pub fn resolve(&self, resource: &ResourceId) -> PathBuf {
        if self.root.is_file() {
            self.root.clone()
        } else {
            self.root.join(&resource.0)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PackageTarget {
    Client,
    Server,
    Bot,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PackageKind {
    Resource,
    Data,
    SandboxedExecutable,
    Configuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentHash(pub [u8; 32]);
impl ContentHash {
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut lanes = [0xcbf29ce484222325_u64; 4];
        for (i, byte) in bytes.iter().enumerate() {
            let lane = i & 3;
            lanes[lane] ^= u64::from(*byte);
            lanes[lane] = lanes[lane].wrapping_mul(0x100000001b3);
        }
        let mut output = [0; 32];
        for (i, lane) in lanes.iter().enumerate() {
            output[i * 8..i * 8 + 8].copy_from_slice(&lane.to_le_bytes());
        }
        Self(output)
    }
    #[must_use]
    pub fn verifies(self, bytes: &[u8]) -> bool {
        self == Self::from_bytes(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageId(pub String);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageVersion(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDescriptor {
    pub id: PackageId,
    pub version: PackageVersion,
    pub kind: PackageKind,
    pub hash: ContentHash,
    pub dependencies: Vec<PackageId>,
    pub targets: Vec<PackageTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContentManifest {
    pub packages: Vec<PackageDescriptor>,
}

impl PackageDescriptor {
    #[must_use]
    pub fn targets(&self, target: PackageTarget) -> bool {
        self.targets.contains(&target)
    }
}
impl ContentManifest {
    #[must_use]
    pub fn for_target(&self, target: PackageTarget) -> Vec<&PackageDescriptor> {
        self.packages
            .iter()
            .filter(|package| package.targets(target))
            .collect()
    }
    #[must_use]
    pub fn validate_bytes(&self, id: &PackageId, bytes: &[u8]) -> bool {
        self.packages
            .iter()
            .find(|package| &package.id == id)
            .is_some_and(|package| package.hash.verifies(bytes))
    }
    #[must_use]
    pub fn deterministic_hash(&self, target: PackageTarget) -> ContentHash {
        let mut bytes = Vec::new();
        let mut packages = self.for_target(target);
        packages.sort_by(|left, right| left.id.0.cmp(&right.id.0));
        for package in packages {
            bytes.extend_from_slice(package.id.0.as_bytes());
            bytes.extend_from_slice(&package.version.0.to_le_bytes());
            bytes.extend_from_slice(&package.hash.0);
        }
        ContentHash::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn package(id: &str, targets: Vec<PackageTarget>, bytes: &[u8]) -> PackageDescriptor {
        PackageDescriptor {
            id: PackageId(id.into()),
            version: PackageVersion(1),
            kind: PackageKind::Data,
            hash: ContentHash::from_bytes(bytes),
            dependencies: Vec::new(),
            targets,
        }
    }
    #[test]
    fn target_filter_and_hash_validation_are_deterministic() {
        let manifest = ContentManifest {
            packages: vec![
                package("test:bot", vec![PackageTarget::Bot], b"bot"),
                package("test:ui", vec![PackageTarget::Client], b"ui"),
            ],
        };
        assert_eq!(manifest.for_target(PackageTarget::Bot).len(), 1);
        assert!(manifest.validate_bytes(&PackageId("test:ui".into()), b"ui"));
        assert_eq!(
            manifest.deterministic_hash(PackageTarget::Bot),
            manifest.deterministic_hash(PackageTarget::Bot)
        );
    }
}
