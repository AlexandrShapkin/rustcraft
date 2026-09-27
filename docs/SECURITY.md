# Security baseline

Security matters because servers can request content and third-party code may eventually execute.

Baseline rules:

- never auto-execute native binaries downloaded from a server;
- verify immutable content by strong hashes;
- validate manifests and dependencies before activation;
- sandbox third-party executable modules;
- grant explicit capabilities rather than ambient host access;
- keep credentials and tokens out of the repository;
- treat malformed network/content input as hostile;
- avoid unsafe Rust unless a concrete need and documented invariant justify it.

A future trust/signing policy can build on these boundaries without changing the core content model.
