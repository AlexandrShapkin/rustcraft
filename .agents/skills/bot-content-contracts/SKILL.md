---
name: bot-content-contracts
description: Design or change Agent API, Bot API, server content manifests, package targeting or mod-aware semantic definitions while preserving the controller boundary and content security model.
---

When touching these contracts:

- keep the simulation independent from keyboard/window state;
- keep Bot API semantic and versionable, not a mirror of internal Rust structs;
- use observe/act capabilities rather than unrestricted mutable world access;
- let bots understand unknown content through IDs/tags/properties/interaction metadata;
- keep client/server/bot package targeting explicit;
- never make automatic server content loading execute arbitrary native libraries;
- add focused serialization/validation tests once wire/file representations exist.
