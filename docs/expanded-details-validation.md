# Expanded graph validation (MM-012)

Windows: `D:\worktrees\personal\mem-map\system-memory-redesign`, branch
`feat/system-memory-redesign`, extending `4925afa516f7e53213b1ae0a5147b66c7eaa65d2`.
Evidence root:
`C:\Users\DanMarshall\AppData\Local\mem-map-build\evidence\expanded-details`.
The latest user requests supersede earlier pending MM-012 candidates: the legend
is top-left only in the expanded graph, with one aligned row per band, names at
most 20 characters, optional program names and change percentages. Pointer exit
returns to latest even with chart focus. Repeated totals and Chart data UI are
removed; both axes show age ending at 0s and actions use accessible icons.

## Verification

- Frontend fast lane: 56 tests pass in 3.226 seconds (`legend-fast-r3.log`). Final
  source coverage run: 56 pass in 3.245 seconds (`legend-coverage-r3.log`), covering
  six loaded production modules: 97.53% lines, 85.45% branches, 81.12% functions.
  Charts: 97.67% lines, 87.59% branches, 85.48% functions. This is not whole-app
  coverage; browser/native entry points have separate boundary evidence.
  Command from `frontend`: `node --test --experimental-strip-types
  --experimental-test-coverage --test-coverage-include=**/src/*.ts tests/*.test.ts`.
- Real-host headless browser: 22 checks pass
  (`legend-headless-r3.log`, `legend-headless-r3/report.json`). Command:
  `scripts/smoke.ps1 -BackendExecutable <existing headless-test-host.exe>
  -OutputDirectory <evidence>/legend-headless-r3`. The report records exact frontend
  asset hashes, command, actual headless Edge identity, source samples/geometry,
  normal host exit and browser/server cleanup with no forced cleanup.
- Rust source, dependencies and headless host are unchanged. The prior 37 Rust
  tests passed at accepted `4925afa` (`../overview-main/all-final.log`). The exact
  backend hash, also recorded in the earlier acceptance and current smoke, is
  `96b55e430164d2b68492e5b75314cf0b88f03b0cce7433af51de0d8615a347a7`.
  That acceptance is reused; no fresh Rust-only unit run is claimed.
- `scripts/build.ps1` rebuilds TypeScript/Vite and the optimized native executable.
  `legend-release-r3.log`, `legend-release-exit-r3.json` and `release-assets.json` retain
  the result and correspondence between tested and embedded frontend asset names.
  The final native build is a required gate before independent PASS/publication.
- Node syntax and Git whitespace checks pass. Every automated test remains
  headless. Private copy/export adapters validate data rather than native dialogs,
  clipboard, UIA or native-window runtime health. No repeated-scroll test is added.

## Criteria and behavior evidence

| Criteria | Evidence |
|---|---|
| AC1 age and density | Component assertions cover adaptive grid density, exact age endpoints, zero/one-second ranges and sidebar density. Actual paused-browser axes end at 0s. Existing main-view size/scale matrix remains passing. |
| AC2/4 aligned legend | `corner-red.log` rejects the always-visible sidebar legend. `corner-fast.log` passes placement/visibility. `legend-row-red.log` rejects the old two-line rows; `legend-row-green.log` records the first implementation plus intentionally obsolete expectation failures; final 56-test runs pass one-line/four-column alignment, 20-character names, program-name preference/fallback and full help. Real browser checks row/column geometry, text bounds, collapsed visibility and source values. |
| AC2/3 coherent samples | Historical sample with 16 GiB capacity versus current 32 GiB, exact shares beyond floating-point precision, unknown system/attribution/capacity, empty history and real Rust raw bytes all have assertions. Cursor and labels use the same actual sample without changing viewBox. |
| AC3/6 change and exit | Component tests assert latest +50%, historical +100%, selected 60-second interval, valid four-second-old versus stale baseline, zero baseline, unknown attribution and unconditional latest on pointer departure with focus retained. Real browser verifies missing five-minute baselines in its new session, aligned changes and focus-retaining latest restoration. Existing percentage tests protect red/green/white, integer precision and unknown/zero semantics. |
| AC5 clutter/actions | Component and browser checks reject repeated summaries, visible disclosure and sidebar legend. Icon buttons retain accessible names, tooltips and normal commands; compact hover/focus/tap and Escape remain passing. |

The original grid/detail work also retained RED/GREEN evidence (`ticks-red.log`,
`inspection-red.log`, `sample-step-red.log`): the browser exposed keyboard stepping
from an unsnapped pointer as a no-op, and the focused RED/GREEN proves navigation
from the nearest actual sample. Those earlier candidates were superseded; their
native build, screenshots and pending review are not current acceptance claims.

Refactoring shares nearest-sample selection and existing exact-byte percentage
formatting, removes obsolete legend/disclosure code, and floats legend/help so
hover cannot consume plot space. SVG clips each single-line field; full values
remain in help. Original line endings are retained on unchanged smoke-script
lines. No new chart framework, backend command, metadata collector, persistence,
additional attribution/history or inferred friendly-name map is introduced.
Optional program-name data is supported, but the current backend supplies only
executable names. Legend baseline availability is limited to displayed samples;
a range shorter than the selected change interval may therefore show unavailable.

Routine feedback remained about 3 seconds; no lane was reclassified. Mutation is
pending because safe CoW storage and calibrated source-bound fast/slow attribution
are unavailable. No mutation score is claimed; E2E remains excluded. Artifacts,
dependencies, caches, active preview and both worktrees are retained.

The first independent review (`details-review-r2/independent-review.md` in the
retained evidence folder) returned FAIL for F1: a zero-span first frame put its
sole 0s tick on the left. `zero-age-red.log` reproduces the exact 52-versus-982
coordinate failure on both chart kinds. `zero-age-green.log` passes the repaired
right position/end anchor, first history point and first overview cursor. The
56-test/22-browser records above and rebuilt executable cover the repaired source.
The complete candidate and finite envelope are returned to the same reviewer.

Independent completion review: PASS (exact candidate and finite publication envelope; retained details-review-r3 verdict).
