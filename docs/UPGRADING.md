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
On a clean bounded rollback branch, review a revert of this migration commit:
restore the old entrypoints and remove only task-added source/reference/notice
files. Preserve later local changes and unrelated work; conflicts require explicit
review. For later source updates, revert only the affected channel's reviewed
files/record to its retained previous Git snapshot, then rerun all gates/CI.
Partial writes are not transactional: preserve evidence and restore affected paths
from the pre-write verified Git snapshot before replanning. Never reset user work.
