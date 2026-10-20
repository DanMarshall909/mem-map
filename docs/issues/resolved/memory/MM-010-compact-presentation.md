# MM-010: Flatten the compact memory dashboard

status: Resolved

## Authority and scope

The user's 2026-10-20 browser comments request row-wide memory backgrounds,
percentage change colored by direction/magnitude, fewer box borders, and chart
labels revealed on hover at small widths. Continue the Tauri UI follow-up on
`feat/system-memory-redesign`, based on published `8c87f47`, in
`D:\worktrees\personal\mem-map\system-memory-redesign`. Preserve the Rust backend,
raw data/CSV, current byte-change sorting, headless tests and live editing preview.
These explicit presentation requests supersede the old in-cell/change bars and
always-visible narrow chart labels. No merge, deployment or unrelated cleanup.

## Acceptance criteria

- AC1: Private working set paints a subtle proportional background across each
  whole row; exact numbers, selection and keyboard focus remain readable.
- AC2: Table and inspector changes show signed percentage relative to the prior
  reading. Growth is red, decline green, greater magnitude increases color strength,
  and exact zero is white. Unknown/partial values remain unavailable, a zero
  baseline with growth is New, and valid confirmed exits can show -100%.
  Raw byte values and reasons remain available in accessible text/tooltips/CSV.
- AC3: At narrow chart widths relative to text scale, labels/legend/readout do not
  consume normal layout space. Hover, keyboard focus and tap expose them without
  changing plot coordinates; wide charts retain inline labels and keyboard data
  inspection. Verify the user's 585x774 viewport at 75% scale headlessly.
- AC4: Panels, row boxes, legend and segmented controls use a flat presentation;
  focus, selection, input and resizing affordances remain clear.

## Plan and evidence

- Reuse the clean active Tauri worktree as the same UI follow-up; fetch and
  fast-forward checks found no upstream change. The primary checkout stays main.
- Add focused DOM/format regression cases, retain existing fast tests, then
  verify actual browser geometry/styles with the headless slow suite.
- Percentage presentation derives baseline from exact current bytes minus the
  backend's exact delta; the backend remains authoritative for measurement quality
  and byte-change sorting. No finite percentage is invented for growth from zero.

## Validation and decision log

- AC1-4 implemented and locally verified: 37 Rust tests, 41 frontend tests and
  19 headless browser checks passed on the final source. TypeScript/Vite and the
  optimized release build passed. See [validation](../../../compact-presentation-validation.md)
  for exact commands, coverage, RED/GREEN evidence, limits and retained artifacts.
- Compact labels initially intercepted plot input in the actual browser. Moving
  them to a viewport-clamped popover beside the plot preserved input and geometry.
  The final headless check reports 90.77% plot width and zero popup/plot overlap.
- Growth/decline colors were brightened after a regression test exposed weak
  contrast on filled, hovered rows. Percent and inspector formatting share one
  BigInt implementation; no Rust/backend or byte-change sorting changes.
- Mutation analysis is pending; safe CoW isolation and source-bound lane attribution
  are unavailable. No mutation result is claimed. All tests remain headless and
  the explicitly removed repeated-scroll regression remains removed.
- Independent completion review: PASS (exact candidate and finite publication envelope; retained compact-review-r1 verdict). Retain the active worktree, browser
  editing preview, compiled outputs, dependencies and evidence. Normal feature
  publication is in scope; merge, deployment and unrelated cleanup are excluded.
