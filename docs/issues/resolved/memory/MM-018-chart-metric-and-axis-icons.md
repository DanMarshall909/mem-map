# MM-018: Metric and axis icon toggles on both charts

status: Resolved

Continue the same Tauri UI from487b1cc. Both charts offer independently selected
Working set/Commit and From zero/Fit values icons, with names/tooltips and native
radio semantics. Preserve existing data, pause, filtering, history and clipboard.

- AC1: Overview commit plots retained measured process private commit, including
  Other processes and stable named bands. It may exceed physical RAM. Unknown
  attribution/coverage remains explicit; never invent a system commit limit,
  physical residual/available or physical-RAM percentage for commit. Header stays
  physical from the same displayed snapshot. Baselines match the chosen metric.
- AC2: Overview fit/from-zero affects actual plot bounds and clips stacked bands
  and cursor to the plot. Selected-history controls retain their behavior. Each
  chart's choices are independent; valid settings persist and invalid commands
  cannot partially change the view. An in-flight old poll must not reset controls while a newer action is pending.
- AC3: Both charts offer compact distinct icon toggles for metric and axis, with
  accessible group/option names, tooltips, checked/focus states and keyboard input.

Cosmetic icon presentation needs no TDD; new metric/axis behavior follows owning
Rust/frontend behavior tests. Final headless journey, native build and independent
frozen review precede spec synchronization/resolution/normal feature push. No
merge, cleanup, OS clipboard test or native-window automated test.

Independent completion review: PASS (exact candidate and finite envelope; retained chart-metrics-review-r1 verdict).
