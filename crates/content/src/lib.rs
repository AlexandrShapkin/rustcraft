//! Server-defined content identity, metadata and deterministic integrity primitives.
pub mod fonts;

use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

pub mod resources;

const PACKAGE_CONTENT_DOMAIN: &str = "rustcraft.package-content.v1";
const MANIFEST_DOMAIN: &str = "rustcraft.manifest.v1";
const TARGET_MANIFEST_DOMAIN: &str = "rustcraft.target-manifest.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidNamespacedId;

impl fmt::Display for InvalidNamespacedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("expected lowercase namespace:path semantic identifier")
    }
}

impl std::error::Error for InvalidNamespacedId {}

/// Stable authored identity. Runtime hot paths use compiled numeric handles instead.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NamespacedId(Arc<str>);

impl NamespacedId {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, InvalidNamespacedId> {
        let value = value.as_ref();
        if !valid_namespaced_id(value) {
            return Err(InvalidNamespacedId);
        }
        Ok(Self(Arc::from(value)))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for NamespacedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("NamespacedId")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for NamespacedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AsRef<str> for NamespacedId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl FromStr for NamespacedId {
    type Err = InvalidNamespacedId;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

fn valid_namespaced_id(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && !path.contains(':')
        && namespace
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
        && path.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-' | b'/' | b'.')
        })
}

macro_rules! typed_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(NamespacedId);

        impl $name {
            pub fn parse(value: impl AsRef<str>) -> Result<Self, InvalidNamespacedId> {
                NamespacedId::parse(value).map(Self)
            }

            #[must_use]
            pub fn as_id(&self) -> &NamespacedId {
                &self.0
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = InvalidNamespacedId;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }
    };
}

typed_id!(ResourceId);
typed_id!(PackageId);

#[derive(Debug, Clone)]
pub struct LocalResourceResolver {
    root: PathBuf,
}

impl LocalResourceResolver {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolve an already-selected package-relative physical path.
    /// Semantic resource-to-path mapping belongs to content/game composition.
    #[must_use]
    pub fn resolve_relative(&self, relative: impl AsRef<Path>) -> PathBuf {
        if self.root.is_file() {
            self.root.clone()
        } else {
            self.root.join(relative)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PackageTarget {
    Client,
    Server,
    Bot,
}

impl PackageTarget {
    const fn code(self) -> u8 {
        match self {
            Self::Client => 0,
            Self::Server => 1,
            Self::Bot => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PackageKind {
    Resource,
    Data,
    SandboxedExecutable,
    Configuration,
}

impl PackageKind {
    const fn code(self) -> u8 {
        match self {
            Self::Resource => 0,
            Self::Data => 1,
            Self::SandboxedExecutable => 2,
            Self::Configuration => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    #[must_use]
    pub fn from_content_bytes(bytes: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key(PACKAGE_CONTENT_DOMAIN);
        hasher.update(bytes);
        Self(*hasher.finalize().as_bytes())
    }

    #[must_use]
    pub const fn from_digest_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[must_use]
    pub fn verifies(self, bytes: &[u8]) -> bool {
        self == Self::from_content_bytes(bytes)
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", blake3::Hash::from_bytes(self.0).to_hex())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageVersion(pub semver::Version);

impl PackageVersion {
    #[must_use]
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self(semver::Version::new(major, minor, patch))
    }

    pub fn parse(value: &str) -> Result<Self, semver::Error> {
        semver::Version::parse(value).map(Self)
    }
}

impl fmt::Display for PackageVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

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

    /// Canonical digest of every semantic manifest field, independent of vector insertion order.
    #[must_use]
    pub fn canonical_digest(&self) -> ContentHash {
        manifest_digest(MANIFEST_DOMAIN, None, self.packages.iter().collect())
    }

    /// Canonical digest of the packages relevant to one runtime target.
    #[must_use]
    pub fn deterministic_hash(&self, target: PackageTarget) -> ContentHash {
        manifest_digest(
            TARGET_MANIFEST_DOMAIN,
            Some(target),
            self.for_target(target),
        )
    }
}

fn manifest_digest(
    domain: &'static str,
    target: Option<PackageTarget>,
    mut packages: Vec<&PackageDescriptor>,
) -> ContentHash {
    packages.sort_by(|left, right| left.id.cmp(&right.id));
    let mut hasher = blake3::Hasher::new_derive_key(domain);
    match target {
        Some(target) => hasher.update(&[1, target.code()]),
        None => hasher.update(&[0]),
    };
    encode_len(&mut hasher, packages.len());
    for package in packages {
        encode_str(&mut hasher, package.id.as_str());
        encode_str(&mut hasher, &package.version.to_string());
        hasher.update(&[package.kind.code()]);
        hasher.update(package.hash.as_bytes());

        let mut dependencies = package.dependencies.iter().collect::<Vec<_>>();
        dependencies.sort();
        dependencies.dedup();
        encode_len(&mut hasher, dependencies.len());
        for dependency in dependencies {
            encode_str(&mut hasher, dependency.as_str());
        }

        let mut targets = package.targets.clone();
        targets.sort_unstable();
        targets.dedup();
        encode_len(&mut hasher, targets.len());
        for target in targets {
            hasher.update(&[target.code()]);
        }
    }
    ContentHash::from_digest_bytes(*hasher.finalize().as_bytes())
}

fn encode_len(hasher: &mut blake3::Hasher, length: usize) {
    let length = u64::try_from(length).expect("manifest collection length fits u64");
    hasher.update(&length.to_le_bytes());
}

fn encode_str(hasher: &mut blake3::Hasher, value: &str) {
    encode_len(hasher, value.len());
    hasher.update(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(
        id: &str,
        version: u64,
        kind: PackageKind,
        targets: Vec<PackageTarget>,
        bytes: &[u8],
    ) -> PackageDescriptor {
        PackageDescriptor {
            id: PackageId::parse(id).unwrap(),
            version: PackageVersion::new(version, 0, 0),
            kind,
            hash: ContentHash::from_content_bytes(bytes),
            dependencies: Vec::new(),
            targets,
        }
    }

    #[test]
    fn owned_namespaced_ids_validate_and_clone_cheaply() {
        let input = format!("{}:block/{}", "dynamic_mod", "reactor");
        let id = NamespacedId::parse(input).unwrap();
        assert_eq!(id.as_str(), "dynamic_mod:block/reactor");
        assert_eq!(id, id.clone());
        assert!(NamespacedId::parse("stone").is_err());
        assert!(NamespacedId::parse("Bad:block/stone").is_err());
        assert!(NamespacedId::parse("a:block:x:extra").is_err());
    }

    #[test]
    fn content_hash_is_stable_printable_and_domain_separated() {
        let hash = ContentHash::from_content_bytes(b"same bytes");
        assert_eq!(hash, ContentHash::from_content_bytes(b"same bytes"));
        assert_ne!(hash, ContentHash::from_content_bytes(b"other bytes"));
        assert_eq!(hash.to_string().len(), 64);

        let manifest = ContentManifest {
            packages: vec![package(
                "test:data",
                1,
                PackageKind::Data,
                vec![PackageTarget::Server],
                b"same bytes",
            )],
        };
        assert_ne!(hash, manifest.canonical_digest());
    }

    #[test]
    fn canonical_manifest_digest_covers_all_semantic_fields() {
        let dependency_a = PackageId::parse("test:dependency/a").unwrap();
        let dependency_b = PackageId::parse("test:dependency/b").unwrap();
        let base = PackageDescriptor {
            dependencies: vec![dependency_b.clone(), dependency_a.clone()],
            ..package(
                "test:main",
                1,
                PackageKind::Data,
                vec![PackageTarget::Server, PackageTarget::Client],
                b"content-a",
            )
        };
        let other = package(
            "test:other",
            7,
            PackageKind::Resource,
            vec![PackageTarget::Client],
            b"content-b",
        );
        let manifest = ContentManifest {
            packages: vec![base.clone(), other.clone()],
        };
        let reordered = ContentManifest {
            packages: vec![
                other,
                PackageDescriptor {
                    dependencies: vec![dependency_a, dependency_b],
                    targets: vec![PackageTarget::Client, PackageTarget::Server],
                    ..base.clone()
                },
            ],
        };
        assert_eq!(manifest.canonical_digest(), reordered.canonical_digest());

        let mutate = |descriptor: PackageDescriptor| {
            ContentManifest {
                packages: vec![descriptor],
            }
            .canonical_digest()
        };
        let one = ContentManifest {
            packages: vec![base.clone()],
        }
        .canonical_digest();
        assert_ne!(
            one,
            mutate(PackageDescriptor {
                version: PackageVersion::new(2, 0, 0),
                ..base.clone()
            })
        );
        assert_ne!(
            one,
            mutate(PackageDescriptor {
                kind: PackageKind::Resource,
                ..base.clone()
            })
        );
        assert_ne!(
            one,
            mutate(PackageDescriptor {
                hash: ContentHash::from_content_bytes(b"changed"),
                ..base.clone()
            })
        );
        assert_ne!(
            one,
            mutate(PackageDescriptor {
                dependencies: vec![PackageId::parse("test:other-dependency").unwrap()],
                ..base.clone()
            })
        );
        assert_ne!(
            one,
            mutate(PackageDescriptor {
                targets: vec![PackageTarget::Bot],
                ..base
            })
        );
    }

    #[test]
    fn length_prefixes_prevent_concatenation_ambiguity() {
        let left = ContentManifest {
            packages: vec![package(
                "a:bc",
                1,
                PackageKind::Data,
                vec![PackageTarget::Server],
                b"x",
            )],
        };
        let right = ContentManifest {
            packages: vec![package(
                "ab:c",
                1,
                PackageKind::Data,
                vec![PackageTarget::Server],
                b"x",
            )],
        };
        assert_ne!(left.canonical_digest(), right.canonical_digest());
    }

    #[test]
    fn target_filter_and_hash_validation_are_deterministic() {
        let manifest = ContentManifest {
            packages: vec![
                package(
                    "test:bot",
                    1,
                    PackageKind::Data,
                    vec![PackageTarget::Bot],
                    b"bot",
                ),
                package(
                    "test:ui",
                    1,
                    PackageKind::Resource,
                    vec![PackageTarget::Client],
                    b"ui",
                ),
            ],
        };
        assert_eq!(manifest.for_target(PackageTarget::Bot).len(), 1);
        assert!(manifest.validate_bytes(&PackageId::parse("test:ui").unwrap(), b"ui"));
        assert_eq!(
            manifest.deterministic_hash(PackageTarget::Bot),
            manifest.deterministic_hash(PackageTarget::Bot)
        );
    }

    #[test]
    fn package_versions_are_independent_semver_values() {
        let stable = PackageVersion::parse("2.4.1").unwrap();
        let prerelease = PackageVersion::parse("2.4.1-beta.3").unwrap();
        assert_eq!(stable.to_string(), "2.4.1");
        assert_eq!(prerelease.to_string(), "2.4.1-beta.3");
        assert!(prerelease < stable);
        assert!(PackageVersion::parse("2.4").is_err());
    }
}
