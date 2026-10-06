# WF1 — workspace & workflow normalization

## Goal

Make checkout selection, command directory, live Git/stage state and normal development commands unambiguous before BG1.

## Context

Owner-authorized bounded maintenance after C2, immediately before BG1. Registry owns sequence/state.

## Current state

Verify actual root, HEAD and dirty work; recovery and historical acceptance paths are not normal workspaces.

## Target state

Document ~/Projects/rustcraft as owner convention; discover actual roots without hardcoded paths.
Agents work in the launched checkout and report mismatches before editing.

## Scope

Portable read-only status helper/recipe, focused tests/CI, workspace and local asset conventions,
normal command navigation and accepted report/evidence.

## Invariants

No unknown work is reset, cleaned, stashed or overwritten. No new clone/worktree without explicit
owner authorization. No proprietary assets committed; no dependency on a historical checkout.

## Out of scope

BG1, broad DX2 retirement, gameplay, renderer, content architecture and persistence changes.

## Implementation constraints

Use Python standard library and Git discovery; report actual command/root directories, not a remembered path.
Retain historical recipes; prefer just commands over owner-shell-specific snippets.

## Acceptance

Status reports repository root, branch, HEAD, upstream, clean/dirty, remote URL, focus and focused state.
Test isolated nested roots/paths, dirty/unborn/detached/missing-upstream cases and invalid registry errors.
Document normal client/release, smoke, sample-game, test and CI surfaces and ignored local assets.

## Validation

Run just workspace-test, docs-test, docs-check and status. Tracked tooling requires Ubuntu/Windows CI.
No repeated expensive Rust validation when source is unchanged; public CI retains its normal gates.

## Documentation updates

Update AGENTS, CODEX, WORKFLOW, TOOLING, report and evidence index; sync generated stage navigation.

## Issue reconciliation

Follow [issue policy](../DEFECTS.md) and [workflow](../WORKFLOW.md); no automatic new issue/label creation.

## Closeout

After accepted publication/CI, close WF1, return focus to BG1 planned, sync/check/test, publish closeout
and require Ubuntu/Windows green. Stop without starting BG1.
