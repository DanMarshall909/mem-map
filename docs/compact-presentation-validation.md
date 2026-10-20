# Compact presentation validation (MM-010)

The 2026-10-20 presentation changes implement the user's row background,
percentage color, flat surface and narrow-chart label requests. Rust collection,
controller, history, byte-change sorting and CSV semantics are unchanged.

## Candidate and local verification

Windows worktree: `D:\worktrees\personal\mem-map\system-memory-redesign`, branch
`feat/system-memory-redesign`, extending `8c87f47df7fff9c6db6e2558aca93e33e1cfe5d4`.
Retained evidence root:
`C:\Users\DanMarshall\AppData\Local\mem-map-build\evidence\compact-presentation`.

- `scripts/check.ps1 all`: PASS, 37 Rust tests, 41 frontend tests and 19 headless
  browser checks. Full output: `all-final.log`. The final headless report is
  `..\headless-20261020-112150-180\report.json` (30.019 seconds, normal host exit,
  browser/server closed, no forced cleanup).
- `scripts/build.ps1`: PASS, TypeScript check, Vite build and optimized Rust build
  (4m 27s). Full output: `release-final.log`. The rebuilt root `MemMap.exe` is
  3,668,992 bytes. Exact binary and asset digests are retained in
  `release-assets.json`; the release embeds the same named assets whose SHA-256
  values are in the final headless report.
- `node --test --experimental-strip-types --experimental-test-coverage
  --test-coverage-include=**/src/*.ts tests/*.test.ts` from `frontend`: PASS, 41
  tests, 96.95% lines, 82.08% branches and 75.92% functions over the six loaded
  production modules (`frontend-coverage-final.log`). This is not whole-app
  coverage; browser/native entry points are separately exercised or outside the
  headless boundary.
- `node --check scripts/headless-smoke.mjs` and Git whitespace checks: PASS.
  No Rust source changed; existing Rust checks still ran in the complete gate.

All automated verification remains headless. The Rust test host runs the real
sampler/controller through stdio and the built frontend runs in headless Edge.
Private test adapters check copy/export data, not OS clipboard, native dialogs,
Tauri command authorization or Windows UIA. The removed repeated-scroll test
remains removed. No native window was opened for these tests or the build.

## Criterion evidence and refactoring

| Criterion | Executable evidence and retained RED/GREEN |
|---|---|
| AC1 full-row working-set background | `presentation.test.ts` checks proportional row fill, keyed focus, exact values and unknown reset. `row-red.log` rejects the old in-cell bar; `row-green.log` passes. Real-browser computed background, focus and old-bar absence pass in the final smoke. |
| AC2 signed percentage and color | Twelve table/inspector cases cover growth, decline, zero, zero baseline, New, unknown/partial, confirmed exit, inconsistent baseline, large integers and tiny nonzero changes. `percent-red.log` rejects byte-only labels; `percent-green-final.log` passes. `change-contrast-red.log` rejects the old text palette; `frontend-coverage-final.log` passes the brighter palette with at least 4.5:1 contrast over the conservative filled-hover background. |
| AC3 compact hover/focus/tap labels | `chart-red.log` rejects always-inline compact labels. Real-browser `compact-render-red*.log` exposed labels intercepting plot input; the fixed popover refactor passed `compact-render-green.log`. Height and keyboard-focus dismissal have separate RED/GREEN logs. Final headless checks verify 585x774 at 75%, 90.77% plot width, zero popup/plot overlap, popup inside viewport, stable coordinates, Escape, actual touch taps, keyboard sample navigation and backend pan/reset. |
| AC4 flat presentation and affordances | `flat-red.log` rejects boxed panels; `flat-green.log` passes. DOM and real computed styles check borderless panels/rows, retained input underline, selection/focus and resize affordances. Forced colors retain essential boundaries. |

Percentage calculation uses exact BigInt bytes: baseline = current - delta,
percent = delta / baseline x 100. Confirmed exits use zero current memory. Growth
from zero says New; partial/unknown endpoints say unavailable; tiny real changes
retain direction as less than 0.1%. Exact byte values and reasons remain in
tooltips/accessibility text and CSV. The header explicitly discloses that sorting
continues to use byte change. Shared formatting prevents inspector/table drift.

Compact charts use their own available width relative to UI scale. A native HTML
popover places labels beside the plot when possible, clamped to the viewport,
without consuming plot layout or changing data coordinates. This replaced the
initial overlay after the real browser exposed pointer interception. No extra
chart framework, backend command or production permission was introduced.

Fast frontend feedback was normally 2.2-2.7 seconds; transient longer runs were
not used to reassign test lanes. Mutation evidence is pending: no calibrated,
source-bound fast/slow attribution and safe copy-on-write mutation harness is
available here. No mutation score or successful mutation run is claimed, and E2E
tests remain outside mutation scope. The final immutable source can be used for
a later isolated mutation follow-up without blocking this verified change.

## Review and publication

Independent completion review: PASS (exact candidate and finite publication envelope; retained compact-review-r1 verdict). The complete manifest, raw evidence,
upstream identity and exact post-review specification/task transformations are
retained outside the repository in `..\compact-review-r1`.

The browser editing preview remains at `http://127.0.0.1:1420/`. The rebuilt native
release and the browser preview are separate deliverables; headless checks and a
successful build do not establish a new native-runtime acceptance result.
