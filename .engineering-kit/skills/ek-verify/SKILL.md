---
name: ek-verify
description: Plan and record exact-head test and security evidence without bypassing native gates.
---

# Verification evidence

Instruction-only; does not run tests or deploy. Preserve the project's own targeted, native, CI, security and platform gates. Smoke and contract checks alone are insufficient. Before running any tool confirm side effects, installation and target output path; RelayForge's legacy live Cargo path must not be overwritten. Record actual commands, outcomes and complete PR HEAD SHA. Separate PASS, FAIL, BLOCKED and UNVERIFIED. Do not treat source CI as live evidence, and recheck after a new commit.
