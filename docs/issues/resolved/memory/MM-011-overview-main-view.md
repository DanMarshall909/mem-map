# MM-011: Make overall memory the main view

status: Resolved

## Authority and scope

The user's 2026-10-20 browser comment requests the ability to make Overall memory
usage the main view. Continue the same Tauri UI follow-up in
`D:\worktrees\personal\mem-map\system-memory-redesign` on
`feat/system-memory-redesign`, extending published `3fab34f`.
This adds a reversible frontend view switch. Preserve the Rust backend, live
collection, pause/range semantics, compact hover labels and headless-only tests.
No merge, deployment, unrelated cleanup or repeated-scroll regression.

## Acceptance criteria

- AC1: An action beside Overall memory usage makes the overview occupy the main
  workspace, giving its plot the available width and height. The process table,
  splitter and inspector leave the visual and keyboard navigation flow.
- AC2: Back to processes restores the dashboard with its existing filter,
  grouping, selection and current data. Updates, range changes and window/text
  resizing keep the chosen main view. Global pause and recording still work.
- AC3: The switch works by keyboard and pointer. Escape returns from the main
  overview; Ctrl+F restores the process filter before focusing it. Existing
  process/inspector shortcuts reveal their controls. Focus remains visible and
  chart-label Escape and modal dialogs retain their own behavior.

## Plan

- Add behavior tests for the reversible local view transition and chart sizing,
  then headless browser geometry, range/update and restoration checks.
- Keep one keyed chart and existing controller state; no backend action or new
  persisted preference is required for the session-local layout choice.
- Review coverage/refactoring, run complete headless checks and release build,
  then obtain independent completion review before specification sync/publication.

## Evidence and decisions

- AC1-3 have component and headless browser assertions. See
  [validation](../../../overview-main-validation.md) for exact commands, retained
  RED/GREEN, coverage, boundary limits and the final complete gate.
- The browser caught a genuine narrow-layout defect: reflow's flex containers
  prevented full workspace coverage. Expanded mode now explicitly uses grid at
  those breakpoints; normal dashboard reflow is preserved.
- Keep this a session-local frontend choice. No persisted setting, Rust behavior
  or backend command changes. One keyed chart and current controller state are
  retained; no repeated-scroll test is introduced.
- Independent completion review: PASS (exact candidate and finite publication envelope; retained overview-review-r1 verdict). Normal feature commit/push is in scope;
  merge, deployment and unrelated cleanup remain excluded.
