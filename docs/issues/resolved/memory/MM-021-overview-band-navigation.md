# MM-021: Overview band navigation and responsive process detail

status: Resolved

The user requests double-clicking a process band to navigate to its process,
more named process bands when chart space permits, and a name/size-only legend
when hovering the normal usage chart.

- AC1: Double-clicking a named program band opens that program group in the
  process table and inspector, including from expanded overview or another
  grouping/filter. Reveal the row; preserve metrics/ranges and pause. Enter on
  the selected chart band provides keyboard access. Residual bands do not invent
  process navigation. Invalid targets reject without partial mutation.
- AC2: Retain a stable bounded pool of up to 12 named programs; show more as
  chart width/height permits. Fold undisplayed named allocations into Other
  processes exactly, including comparison baselines, for both metrics. Preserve
  totals, unknown/partial attribution, identities and colors through resize.
- AC3: Normal overview hover/focus shows a compact overlaid legend containing
  only program name and memory size, then hides when inspection ends. Keep
  detailed expanded legend, exact values, available plotting area, accessible
  chart inspection and small-window reachability.

Behavior-first verification for navigation/accounting/interaction; no new tests
merely mirroring cosmetic layout. All automated checks remain headless. Review
the exact candidate before publication; no merge or cleanup.

Independent completion review: PASS (exact candidate and finite envelope; retained band-navigation-review-r1 verdict).
