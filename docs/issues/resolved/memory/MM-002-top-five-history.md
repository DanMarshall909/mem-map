# MM-002: Top-five history comparison

status: Resolved

User scope: add an overlaid history chart for the top five apps/process groups,
alongside the existing pie option. Project attribution remains a proposal because
command lines, wrappers, shared tools and parent processes can give uncertain labels.

- AC1: Show up to five active groups ranked by current private RAM or private commit
  according to the selected metric and Name/Path/Family/PID grouping. Empty and
  fewer-than-five captures remain usable. Ignore table search and growth sorting.
- AC2: Keep each chosen identity and colour slot fixed across samples, exits,
  restarts and table actions. Refresh explicitly chooses the latest top five;
  changing grouping or metric chooses a new relevant comparison.
- AC3: Overlay all series on shared time and numeric scales using captured history;
  preserve missing observations and pauses as gaps. Respect 5/15/30-minute zoom.
- AC4: Provide Pie / Top 5 switching, clickable legends to inspect the exact group
  or process, and keyboard access. Retain the dense table and existing pie drilling.
- AC5: Reuse bounded history without extra sampling or persistent history copies;
  verify native rendering, controller outcomes, full-history resources and footprint.

Design: compact comparison replaces the pie in the sidebar when selected. The
selected-process graph remains available below. Initial view is Top 5. Labels
explain that members are pinned until Refresh, grouping or metric changes.

Validation: deterministic model tests for ranking/identity/series/gaps, asserted
native smoke for switching/refresh/drilling, shared-renderer images, fast and real
Windows lanes, formatting, Clippy, locked build and independent completion review.
Coverage/mutation and external native input acceptance retain their documented gaps.

Workspace: Windows task worktree D:\worktrees\personal\mem-map\top-five-overlay,
branch feat/top-five-overlay, base origin/main 15c7d0c18190a57de7dfd5bfb3d74769066b4ee3.
Primary checkout and its running app/captures are preserved. Build output is local
to this task worktree. Publication uses the existing authorized public repository;
merging into main requires authorization.

## Resolution

AC1–AC5 have local engineering validation recorded in docs/top-five-validation.md.
Independent completion review: PASS, retained privately in artifacts/mm002/review/verdict.md.
Model and native controller assertions, inspected renders and resource stress support
the compact top-five comparison. Project inference remains unimplemented; coverage,
mutation, external native input and hosted acceptance retain their stated gaps.
Feature-branch publication does not establish integration into main.
