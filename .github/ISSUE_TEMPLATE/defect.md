---
name: Actionable defect, debt or investigation
about: Record an evidenced problem or accepted debt with bounded acceptance.
title: ''
labels: ''
assignees: ''
---

<!-- Search open AND closed issues first. No speculative roadmap tickets or proprietary fixtures. -->

## Summary

Describe the current concern. Type: defect / architecture-debt / performance / investigation / feature-gap.

## Legacy tracking IDs

Optional IDs or related canonical issue links; omit if none.

## Current evidence

Reproduction, source paths, version/build/config and measurements. Label uncertainty; avoid credentials.

## Why this matters

User-visible consequence or violated/accepted architectural contract.

## Expected invariant

What must hold after resolution?

## Severity and area

P0 / P1 / P2 / P3 with rationale; one primary area. Apply matching labels.
Use bug for defects, enhancement for feature gaps, otherwise the corresponding type:* label.

## Owning stage

Known current ROADMAP owner, or Unassigned backlog. Do not infer stage from a legacy ID.

## Acceptance

- Bounded proof/reproduction and regression coverage.
- Required platform/performance evidence; investigation change/no-change decision gate if relevant.

## References

Relevant source/docs/decisions/reports. See docs/DEFECTS.md for tracking and closure policy.
