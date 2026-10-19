# MM-006: Stacked area memory comparison

status: Resolved

User scope: make the graph a stacked area chart. Apply this to the multi-process
Top 5 comparison; preserve its pinned identities, colours, clickable legend,
metric/group selection and time zoom. The individual-process history remains a
single-series graph. Retain the accepted app icon and prior captures.

- AC1: Bands stack captured RAM or commit values in fixed member order at each
  sample, with a shared cumulative scale. Band thickness equals that member's
  own usage; legend values stay individual. Work with zero, one and up to five
  members, bounded to the existing retained history.
- AC2: Missing observations and sampling gaps break the relevant areas. Missing
  lower-band values must not become invented zero baselines for upper bands.
  An upper member may retain its known legend value while its baseline is missing.
- AC3: Render filled native areas, preserve process selection/legend drilling,
  pie switching, colour identity, zoom and bounded GDI/memory behavior. No new UI
  framework, dependency, sampler or persistent history.

Windows worktree D:\worktrees\personal\mem-map\stacked-area, feat/stacked-area.
Explicit dependency: freshly fetched origin/feat/app-icon at
4362d4b01fe002c2e6951f89101265b1f8ff910e retains the user-accepted icon while PR #2
is open. Source work happens in this new isolated worktree. Publish a normal
fast-forward to the same origin/feat/app-icon and update PR #2; leave the older
icon worktree/local branch and active preview unchanged. No merge or cleanup.


## Resolution

AC1-AC3 pass local engineering validation in docs/stacked-area-validation.md.
Exact cumulative-boundary and missing-baseline RED/GREEN, all 22 tests, native
control/render/resource smoke, formatting, Clippy and locked release pass.
Independent completion review: PASS, retained in artifacts/stacked/review/verdict.md.
Existing source coverage, mutation, external-input and hosted acceptance gaps
remain disclosed. Publish normally to the existing feature PR #2; no merge or
cleanup. Earlier worktrees, app captures and warm caches are preserved.

User-requested cosmetic follow-up: remove the grid lines running through the
stacked chart. Removed only those grid strokes, keeping axes and bands. Visual
verification and existing checks pass; see docs/stacked-area-validation.md.
