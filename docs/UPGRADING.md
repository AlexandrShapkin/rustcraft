# Reviewed source updates and rollback

This initial adoption directly copies twelve publicly licensed Git blobs. It is
not Kit sync, not a format-3 artifact publication, and not evidence of independent
acceptance. The consumer-owned source.json records are not Kit lock schemas.
The canonical six-path ownership sets are unchanged; local files, notices,
taxonomy and native CI stay consumer-owned. Do not feed these records to sync.

## Independently pinned channels

- Kit technical source: `f808af695c7787fb68f332d8ff1214fb2100f9b5`; minimal profile.
- Template administration: `d15d0a20b91f54be1a27b78d90f9d367b2e46408`.
- Licensed accepted source mains: Kit `cdfc2050733388402a70674d22de3f9e7e2a5a74`,
  Template `8550ec7fa8f58dc41a8af652ff912b62ebf78c22`.
- The accepted administrative sync implementation remains
  `21cf2914cb0a1c904ca8c783a36723786373b600`; no local updater is introduced.

The complete source paths and SHA-256 values are in each channel record. Anonymous
exact-commit downloads and Git blobs were compared; all twelve also match their
accepted public mains. Source-level copyright/permission notice is retained
outside the six-path sets. Historical contract links are provenance, not active
policy or permission to execute commands from other projects.

## Update procedure

1. Inventory actual root, full HEAD/status, worktrees and active PR dependencies.
   Select and independently review a full SHA for only the channel being updated.
2. Review its public visibility, rights, source changes and source-path map. Compare
   every installed file with the current source record. Missing/edited files,
   symlinks (including ancestors), unmanaged collisions or a changed root/state
   block dependent writes; preserve local work and resolve the collision in review.
3. Use Git's exact committed blobs, compare all six files/hashes with anonymous
   exact-commit source, inspect every proposed replacement and its preserved local
   constraints. Prepare a new bounded PR and refresh the plan after any state change.
   Never force-overwrite collisions, delete locks or relabel private formats.
4. Commit reviewed bytes and that channel's source record together; leave the
   other channel unchanged. Run source tests, every native gate and exact-head CI,
   then obtain independent acceptance. No autonomous update or execution is implied.
5. To transition to Kit sync later, obtain independent owner approval for the
   exact format-3 digest through a trusted channel and explicitly reconcile the
   existing source-owned paths; initial sync must reject them as unmanaged.
   Do not manufacture a lock or artifact approval to bypass this rejection.

## Rollback

Pre-migration accepted base: `ca1ea1547fbcdd9e0df17e2c8d7a237102053e0c`. Its Git blobs retain every original
entrypoint/template; the migration commit retains the new twelve-file source
snapshot and both source records for rollback. No generated bundle is published.
On a clean bounded rollback branch, review reverts of this migration's commits in reverse order:
restore the old entrypoints and remove only task-added source/reference/notice
files. Preserve later local changes and unrelated work; conflicts require explicit
review. For later source updates, revert only the affected channel's reviewed
files/record to its retained previous Git snapshot, then rerun all gates/CI.
Partial writes are not transactional: preserve evidence and restore affected paths
from the pre-write verified Git snapshot before replanning. Never reset user work.


## Reproducible source-update proposals (Kit #8)

The Kit-owned source-update adapter at proposed implementation commit
`6c8acc6df12eab9009b53a73e1710b05ef12ac83` supports these existing source.json
records without converting them to bundle locks. Independently accept that
implementation before production adoption. The complete operator and rollback
procedure is [pinned in Kit](https://github.com/AlexandrShapkin/codex-engineering-kit/blob/6c8acc6df12eab9009b53a73e1710b05ef12ac83/docs/SOURCE_UPDATES.md).

This proposal advances only the two provenance references to already accepted
public MIT upstream mains: Kit `cdfc2050733388402a70674d22de3f9e7e2a5a74`
and Template `8550ec7fa8f58dc41a8af652ff912b62ebf78c22`. All twelve managed
payloads remain byte-identical; original license_at identities/notices remain.
The original pin list above records initial adoption history. Select future Kit
and Template updates independently; each channel records its own source and hashes.

From a separately trusted Kit checkout, run `source-update plan` with an explicit
`--channel`, consumer `--root`, canonical `--source` and accepted full `--revision`.
Inspect immutable-source diffs and deterministic path/before/after hashes.
Run `source-update write` only with the fresh plan SHA and every changed exact
`--allow` path; records are last. Changed native integrity-record hashes require
an explicit reviewed edit of the corresponding RECORDS entry, never local-rule,
notice or native-CI assertion changes. Repeat plan (empty), run all native gates
and exact-head hosted CI, and open a separate Draft PR for independent review and
explicit owner merge. No automatic merge or artifact authorization is implied.

Rollback this proposal using an ordinary reviewed revert, restoring both source
records, their two integrity-test pins and this additive procedure; rerun native
checks/CI. Later channel updates can plan/write the retained older full revision,
verify original payload hashes/record semantics and separately restore its test
pin. A reviewed Git revert also restores exact historical JSON formatting.
After interrupted I/O, preserve observations and restore only affected managed
files and record from the verified pre-write Git snapshot before replanning.
Never reset user work, delete a lock/record or bypass collisions to recover.
AGENTS.local.md, product/security boundaries, native CI and unrelated PRs remain
consumer-owned. Public source is not approval to distribute generated artifacts.

## Solo-maintainer canonical policy update (Template #16 / Kit #8)

Template PR #17 independently accepted at `355788547e62ce8fc582e847f0131a541702a1b0`,
merged at `cd08531493b34c98f0587544cd09e36c5da8fb11`. This bounded proposal
uses accepted immutable administrative source `935022f96e0654d997d511dce79ab022dcae9d92`
through Kit source-update plan/write at reviewed Kit HEAD
`58bc7a282bcb148d94bfb0b0d609cd20a85fbce1` (Kit PR #23).
The exact plan digest and final-head verification/review evidence are recorded on
this repository’s rollout PR, linked to existing owning Issues. Only the three
changed canonical policy files and administrative source record are written by
the adapter; the corresponding consumer-owned integrity pin is explicitly updated.
The technical record, license_at identities, frozen contracts, local rules, native
platform/security gates and actual branch protection are preserved. Formal APPROVE
is optional only where actual protection permits; exact-head independent ACCEPT
and CI remain mandatory. Owner authorized this named six-repository series only.
Rollback is a reviewed revert including the affected integrity pin, preserving
subsequent work. Isolated /tmp/solo-governance-*-20261023 checkouts and logs are
retained for review/rollback until the coordinated rollout closes.
