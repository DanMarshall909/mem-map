# MM-003: Parent process names

status: Resolved

User scope: show the parent name alongside the recorded parent PID so process
ownership is easier to inspect, particularly for ChatGPT, Codex and Node helpers.
This extends the same app feature branch and PR #1 before integration; the
top-five comparison remains present. No merge or process termination is authorized.

- AC1: Resolve a parent name from captured process identity only when the parent
  predates its child. Retain a previously observed parent name after the parent
  exits. Never substitute the name of a younger process reusing the parent PID.
  Distinguish no recorded parent from an unavailable name.
- AC2: Show parent name and PID in individual process rows and selected-process
  details, preserving the dense layout and existing memory fields. Searching a
  parent name includes its matching children.
- AC3: Reuse existing capture data and bounded track metadata; introduce no
  additional sampler or OS queries. Verify real native parent identity and the
  visible renderer/controller paths without disturbing existing captures.

Validation: owning fast model regressions, real Windows parent/child capture,
native smoke and inspected process-detail renderer, format, Clippy, locked build
and proportionate independent review. Existing coverage/mutation/external input
limitations remain disclosed.

Workspace: continue D:\worktrees\personal\mem-map\top-five-overlay on
feat/top-five-overlay, starting at ab575327730c322f4243eafb7b8735de840d26af.
Upstream origin/feat/top-five-overlay has the same commit; origin/main remains
15c7d0c18190a57de7dfd5bfb3d74769066b4ee3. Primary main is clean and preserved.

## Resolution

AC1–AC3 have local engineering validation in docs/parent-name-validation.md.
Independent completion review: PASS, retained privately in artifacts/mm003/review/verdict.md.
Observed parent names and PIDs appear in process rows and details and are searchable.
Native parent/child and handle tests, controller smoke and inspected rendering pass.
The existing coverage, mutation and external-input/hosted acceptance gaps remain.
This updates the same feature branch and PR #1; integration into main is separate.
