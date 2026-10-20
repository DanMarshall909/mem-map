# MM-019: Inspector follows chart context

status: Resolved

Continue the same Tauri UI from 6d8ce3f. The user explicitly chose both metric
and historical chart cursor context for the inspector summary values.

- AC1: The inspector summary uses the selected-history metric for change and
  peak. Inspecting a historical chart sample updates working set, commit,
  change, peak, coverage and time context coherently to that sample. Baselines
  and peaks never use future samples. Missing/partial data remains explicit.
- AC2: Leaving the chart or dismissing keyboard inspection restores the latest
  displayed snapshot, even when the history viewport is panned. Metric changes,
  live updates, pause and changing selection preserve coherent context. Table
  values and overall memory settings remain independent.
- AC3: Executable paths align left with the filename beside the folder when
  space permits, while retaining truncation and full-path accessibility/copy.

Use behavior-first tests for AC1/AC2, visual/build verification for cosmetic AC3.
All automated checks remain headless. Review before completion/publication;
retain worktrees, preview profile, caches and evidence. No merge or cleanup.

Independent completion review: PASS (exact candidate and finite envelope; retained inspector-context-review-r1 verdict).
