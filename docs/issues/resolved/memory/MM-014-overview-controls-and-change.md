# MM-014: Overview controls and reliable changes

status: Resolved

Continue the same Tauri UI follow-up in system-memory-redesign from 5f72182.
User browser comments authorize this bounded slice; preserve the Rust collector,
clipboard boundaries, existing process state and headless-only automated tests.

- AC1: Expanded legend change percentages use available valid retained history,
  independently of the visible chart window; unknown readings and genuinely
  missing baselines remain honest with a reason. Pointer/latest behavior stays.
- AC2: Overview history is a single-selection toggle group (5m/15m/30m/All),
  preserving existing controller semantics, accessibility and keyboard operation.
- AC3: Collection Details always has a reachable close action, including long
  content and small windows, and Escape dismisses it without changing main view.
- AC4: 50% scale works through selection, shortcuts, controller and persistence;
  retain current upper bound and reject invalid values.
- AC5: Remove app heading, routine live/update text, footer counts and visible
  previous/next sample arrows. Keep keyboard chart inspection, paused/error
  feedback and collection diagnostics. Snapshot copy matches back icon accent.

Plan: owning behavior RED/GREEN/coverage per criterion, final real-controller
headless journey and native build; independent exact-candidate review before
spec synchronization, resolution and normal feature push. No merge/cleanup.

Independent completion review: PASS (exact candidate and finite envelope; retained overview-controls-review-r1 verdict).
