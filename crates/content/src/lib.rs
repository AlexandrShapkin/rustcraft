//! Server-defined content metadata. Downloading and sandbox execution are intentionally deferred.

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDescriptor {
    pub id: PackageId,
    pub kind: PackageKind,
    pub hash: ContentHash,
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
