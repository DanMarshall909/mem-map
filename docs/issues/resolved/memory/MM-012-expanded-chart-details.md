# MM-012: Show more detail on the expanded memory graph

status: Resolved

## Authority and scope

The user's 2026-10-20 request is “on the larger graph we can show more details”,
refined to remove repeated summaries and the separate Chart data UI,
place the legend over the top-left of the expanded graph only, prefer accessible icons and show age from zero at
the right edge instead of elapsed time since startup.
Continue the Tauri UI follow-up on `feat/system-memory-redesign`, extending
published `4925afa`, in `D:\worktrees\personal\mem-map\system-memory-redesign`.
Enrich the expanded overview using its existing coherent history samples. Keep
the backend, compact hover labels and headless-only
tests. No invented attribution/history, new backend command, persistence change,
merge, deployment, unrelated cleanup or repeated-scroll regression.

## Acceptance criteria

- AC1: Expanded overview uses its available plot width/height for additional
  readable time and memory ticks/grid lines; the ordinary sidebar keeps its
  existing density and compact labels remain on demand.
- AC2: Expanded details show the inspected sample's
  per-band bytes and share of physical RAM, capture time and measurement coverage.
  Pointer and keyboard inspection use the same actual sample and that sample's
  own capacity. Unknown system/attribution/capacity remains unavailable, with
  reasons retained, rather than inventing zeroes or attribution.
- AC3: A cursor marks the actual inspected time and known used-memory point.
  Hover inspection updates details without changing plot geometry; ordinary
  latest readings resume when pointer inspection ends. Main-view resizing,
  navigation and returning to the process dashboard continue to work headlessly.
  Pointer exit restores latest readings even while the chart retains focus.
- AC4: Only the expanded overview shows the band legend, over the top-left
  corner of the plot. Each band has one line with aligned names, bytes, physical-
  memory share and change percentages. Names are at most 20 Unicode characters;
  supplied program names take precedence, with executable-name fallback. No legend
  is displayed in the sidebar. There is no repeated capacity/available summary
  or visible Chart data disclosure. Compact axis annotations remain on demand.
- AC5: Both chart axes show sample age from the displayed right endpoint: zero
  seconds on the right and increasing age to the left, including paused views.
  Action buttons use recognizable icons, accessible names and tooltips.

- AC6: Band change uses the selected change interval and the last valid sample
  at/before its target, no older than four seconds. Missing baselines/attribution
  remain unknown, zero-baseline and red/green/white conventions match the table.

## Plan

- Add component RED/GREEN for adaptive tick density, then exact sample details
  and cursor/unknown/precision cases. Reuse the real-controller headless main-view
  journey for pointer/keyboard inspection and geometry.
- Run frontend coverage, the full frontend suite and real headless browser lane.
  Rust sources/commands are unchanged; retain the prior exact-backend 37-test
  acceptance at `4925afa` instead of repeating unchanged Rust-only tests.
- Rebuild the native release, review the frozen candidate independently and
  synchronize the implemented specification only within an earned PASS envelope.

## Evidence and decisions

- The latest legend-row and latest-on-exit requests supersede earlier pending
  candidates/reviews. [Validation](../../../expanded-details-validation.md)
  records current source-bound evidence and its limits.
- Changes use existing overview samples rather than filtered table rows. A short
  visible range can omit the requested baseline, which stays unavailable.
- Optional program-name metadata is recognized; the current Rust projection
  supplies executable names only. No friendly-name guesses or metadata collector.
- Backend is unchanged; exact prior 37-test acceptance is retained. Frontend
  coverage, real-host headless journeys and native build are refreshed.
- Independent review F1 found a misplaced 0s label on zero-span startup views.
  Position RED/GREEN now covers both chart kinds, their endpoint/cursor and
  end anchoring; refreshed full checks and native build precede retained review.
- Independent completion review: PASS (exact candidate and finite publication envelope; retained details-review-r3 verdict).
