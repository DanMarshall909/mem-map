# MM-019 inspector context validation

Continuation of 6d8ce3f in the Windows system-memory-redesign worktree.

AC1/AC2 use behavior-first Rust and frontend tests. Rust inputs deliberately
distinguish working set 100/400/200/300 from commit 900/1100/700/1500.
Assertions protect metric-specific peak/change, no future peak or baseline,
partial endpoint, skipped partial and stale baselines, incomplete enumeration,
exited latest versus retained history, pause and viewport independence. Readings
are computed once per retained time; a monotonic deque computes rolling peaks.
Table and selected snapshot calculations retain their original ownership.

Frontend tests exercise pointer and keyboard sample selection, metric/live refresh,
unavailable readings, selection reset and Escape/leave returning to latest outside
a panned viewport. Historical percentages do not inherit a later exited lifecycle.
Summary text and accessible raw-value labels use one coherent sample. Existing
presentation fixtures now supply the backend inspection fields instead of leaving
their newly independent summary projection stale. No new native command is added.

AC3 is cosmetic: the directory fragment no longer grows into all spare width.
Filename preservation, full-path accessibility/copy and narrow truncation remain.
Use visual/build verification rather than new CSS-mirroring tests.

Genuine RED receipts: LOCALAPPDATA/mem-map-build/evidence/inspector-context/red.log
and frontend-red.log. An initial TypeScript parameter-property implementation was
unsupported by Node strip-types; corrected to an ordinary field assignment. That
syntax/setup failure is not claimed as behavioral RED. Refactor reused the chart
readout lifecycle and separated summary rendering from the latest-only update.

Final check gate: PASS in split runs (47 Rust: 44 library + 1 origin + 2 live
sampler; 70 frontend; 25 real-host headless checks). The initial
scripts/check.ps1 all passed Rust/frontend/live lanes, then the added headless
journey raced a preceding asynchronous metric restoration. The test now waits
for both restored backend settings and the rendered checked radio before starting
the next journey. The complete scripts/smoke.ps1 rerun passed; the initial failure
is retained as headless-setup-race.log, not presented as product RED or a passing
all invocation. No production sources changed after those fast/live checks.
Frontend coverage: 70 passed; 97.75% lines, 85.62% branches, 84.95% functions across
the measured frontend modules/fixture. Rust formal source coverage is not configured.
Native scripts/build.ps1 and tested-asset correspondence: PASS (retained release log and artifact hashes).
Rendered path alignment and current preview: PASS (retained preview-verification.json and screenshots).
Independent completion review: PASS (exact candidate and finite envelope; retained inspector-context-review-r1 verdict).

All automated checks remain headless; no native-window/OS clipboard test. Mutation
is pending because safe CoW storage and calibrated fast/slow source attribution
remain unavailable. No mutation score or business/hosted acceptance is claimed.
Retain warm caches, worktrees, native release, preview profile and evidence.

Local finding: latest selected snapshot, visible history endpoint and inspected
sample are distinct contexts. The typed inspection projection and owning tests
carry this invariant; no standing rule or memory update is proposed.
