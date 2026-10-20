# MM-021 band navigation validation

Continuation of c09692f in the Windows system-memory-redesign worktree.

AC1: Rust tests navigate to a named program from a filtered process grouping with
held order and paused data, require an explicit matching reveal, retain chart
settings and reject invalid/non-band IDs with no View/Preferences mutation.
Exited targets enable visibility only when required. Frontend tests bind actual
band double-click, expanded legend return and keyboard Enter. The headless
journey locates a point inside a real SVG path using its fill and screen hit test,
double-clicks it, and verifies controller selection, visible row and inspector.

AC2: Rust retains up to 12 ranked initial names and keeps their identities fixed.
Independent group inputs conserve both working-set and commit residual totals.
Frontend tests compare roomy versus small renderings, exact folded bytes above
JavaScript integer precision, folded comparison baselines, preserved source View,
stable colors on resize, measured commit totals and unknown attribution. A bounded
presentation-only projection folds hidden arrays; backend accounting is unchanged.

AC3: Tests cover idle/hover/focus/Escape legend state, name/size-only normal legend,
unchanged plot coordinates and the detailed expanded legend. Existing headless
compact hover/focus/tap and enlarged-text/window journeys remain in the final gate.
Native checkbox/icon toolbar behavior from MM-020 remains unchanged.

Behavioral RED receipts are external under evidence/band-navigation. Production
refactor reuses existing chart readout/selection, typed command validation and
explicit reveal; responsive folding is isolated from source data. Tests use
distinguishable allocation inputs and rendered/public outputs, with no CSS-only
tests. No new sampler, permission, process launch or OS clipboard operation.

The real-browser journey exposed hover legend hit interception: the normal
legend is now click-through. A redraw regression then protects SVG/legend common
ancestor double-clicks by resolving the current hit at the pointer. Existing
legend groups stay attached while their readings update. The test also stopped
requiring a one-shot reveal flag from a later poll: Rust verifies the action's
reveal response, while the browser verifies the actually visible selected row.
That receipt assertion correction is not product RED. Superseded release builds
were stopped by verified owned compiler lineage after frontend fixes arrived;
their cancellation logs are retained separately from the final release receipt.

Independent review round 1 rejected two edge cases: Enter on the compact info
button and a named band whose unprotected process history had expired. Repairs
retain the button's native Enter behavior and reject a band without a real
navigable retained summary before mutation. The reviewer reassesses the complete
repaired candidate; the earlier FAIL is retained, not relabelled as approval.

Final check gate after review repairs: scripts/check.ps1 all PASS: 54 Rust
(51 library + 1 origin + 2 live sampler), 78 frontend and 26 headless checks.
The original pre-repair all invocation exposed legend interception and remains
a retained failure; the final repaired invocation passed every lane, rebuilt the
headless host, and completed without renderer exceptions.
Frontend coverage after repairs: 78 passed; 98.06% lines, 86.52% branches, 86.16%
functions in the measured frontend modules/fixture. Coverage proves execution,
not mutation.
Native scripts/build.ps1 and tested-asset correspondence: PASS (retained release log and artifact hashes).
Rendered preview and band navigation: PASS (retained preview-verification.json and screenshots).
Independent completion review: PASS (exact candidate and finite envelope; retained band-navigation-review-r1 verdict).

All automated checks remain headless. Rust formal source coverage is not
configured. Mutation is pending: safe CoW storage and calibrated source-bound
fast/slow attribution remain unavailable; no score is claimed. Retain worktrees,
preview profile, reusable caches and evidence. No merge, new PR or cleanup.
