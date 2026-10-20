# Main overview validation (MM-011)

The user's Overall memory usage comment is implemented as a reversible main-view
action. Expand overview gives the existing chart the workspace; Back to processes,
Escape and process shortcuts reveal the dashboard. This session-local presentation
choice keeps the controller, range, selection, filter and pause semantics intact.

Windows worktree: `D:\worktrees\personal\mem-map\system-memory-redesign`, branch
`feat/system-memory-redesign`, extending published
`3fab34f06bb492a65cba17897b65c5c4589a1737`.
Evidence root: `C:\Users\DanMarshall\AppData\Local\mem-map-build\evidence\overview-main`.

## Verification

- `scripts/check.ps1 all`: final complete gate, 37 Rust tests, 44 frontend tests
  and 21 headless browser checks; full output `all-final.log`. The final browser
  report records exact assets, geometry, cleanup and the real Rust host identity.
- `scripts/build.ps1`: TypeScript/Vite and optimized release build;
  `release-final.log` and `release-exit.json` retain the result. `release-assets.json`
  binds the executable, loader and exact tested frontend assets, checking that
  both built asset filenames are embedded in the release executable.
- From `frontend`: `node --test --experimental-strip-types
  --experimental-test-coverage --test-coverage-include=**/src/*.ts tests/*.test.ts`:
  44 passed; 97.55% lines, 83.12% branches and 79.90% functions across six loaded
  production modules (`coverage.log`). These are not whole-app coverage numbers.
- `node --check scripts/headless-smoke.mjs` and Git whitespace checks pass.

All maintained automated tests remain headless. Real browser geometry uses built
assets in headless Edge with the real Rust controller/sampler via the test-only
stdio host. Copy/export use private test adapters. Native window, clipboard,
save-dialog and Windows UIA acceptance are not claimed. The removed repeated-scroll
test remains removed. No Rust source, backend command or persisted setting changed.

## Criteria, RED/GREEN and refactoring

| Criterion | Assertions and evidence |
|---|---|
| AC1 main workspace | Three new component scenarios initially failed for missing action/expanded height (`main-red.log`), then passed (`main-green.log`). The real-browser `render-red.log` caught narrow reflow flex containers failing to fill the workspace; explicit grid layout corrected it in `render-green.log`. Final headless geometry checks panel coverage, root containment and expanded plot width/height at 585x774 with 75%, 100%, 125% text and 1440x900 at 100%. |
| AC2 current state and controls | Component tests assert keyed chart/row identity, latest values on return, existing filter/selection/pause and no controller actions for the mode switch. The real-browser scenario changes the actual controller range, observes a new collected snapshot while main mode stays active, pauses, and checks filter/group/sort/selection survive the return. |
| AC3 keyboard/pointer and Escape | Component tests cover visible switch focus, Ctrl+F restoration, F8, compact labels owning the first Escape and main mode owning the second, plus modal Escape. The real browser enters with Enter, returns with Escape and pointer, and checks Ctrl+F reveals/focuses the filter. |

At the reference 585x774/75% layout the headless plot is about 536x493 pixels;
at 1440x900/100% it is about 1380x584. Exact measurements are retained per run.
The old sidebar height cap no longer applies in main mode. Compact labels still
follow each chart's own width relative to text scale.

Production refactoring keeps one view transition responsible for panel visibility,
restoration and focus; the chart owns its own expanded sizing. Hidden table work
is refreshed on return instead of rebuilding the UI. Escape handling now lets an
already-dismissed compact chart bubble Escape to its containing main view.
Tests reuse the existing view fixture and protect the observable transition and
rendered geometry rather than new backend interactions. The failed rendering
scenario is retained; no timing threshold or layout assertion was weakened.

Routine component GREEN took about 2.2 seconds; coverage added parallel worker
overhead (about 5.0 seconds). The initial RED had a transient slow Node startup;
no lane membership changed based on that anomaly. Slow real-browser checks remain
in the existing slow lane. Mutation is pending because no calibrated source-bound
lane attribution and safe CoW mutation harness is available; no score is claimed.
Existing isolated adapters and real-controller tests retain their prior boundary.

## Completion review

Independent completion review: PASS (exact candidate and finite publication envelope; retained overview-review-r1 verdict). Full manifest, raw evidence and exact
post-review task/spec synchronization are retained outside the repository in
`..\overview-review-r1`. The browser editing preview, both worktrees, compiled
release, dependencies and warm caches remain retained. No merge or deployment.
