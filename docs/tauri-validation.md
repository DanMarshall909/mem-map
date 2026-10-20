# Tauri migration validation

Candidate status: independent completion review PASS; local verification complete.

## Scope and source

MM-008 replaces the entire production UI with Tauri while retaining the Rust
backend. The user's 2026-10-20 instruction removes the repeated-scroll test:
the reported corruption belonged to the superseded native Windows surface.
The maintained desktop suite contains no repeated-wheel or pixel-restoration
test. Earlier raw runs are historical investigation evidence only.

Windows task path: `D:\worktrees\personal\mem-map\system-memory-redesign`.
Branch: `feat/system-memory-redesign`; starting commit/upstream
`b2adc8ba05c4446054d26ce91b9faf2cfb05a6ca` (full identity in review manifest).
No merge or deployment is part of this change.

`sampler.rs`, `dashboard.rs` and `preferences.rs` have no change from the
starting revision. `model.rs` only loses the obsolete native viewport/zoom
helpers and their test. The UI-neutral application controller calls the retained
model rather than reproducing its calculations in TypeScript.

## Specification evidence

| Specification | Implementation and outcome assertions |
|---|---|
| 1: Layout/theme | Shared CSS tokens; keyed DOM; desktop split workspace; adaptive normal-flow reflow for constrained window/text scale. DOM layout transitions plus actual native resize/zoom geometry and screenshots. |
| 2: Header | Exact byte strings, unknown capacity/percentage, frozen displayed snapshot with live collector health. Controller coherence/pause tests; real pause and native CSV timestamp assertions. |
| 3: Toolbar | Five Rust groupings, filter, exited, density and change interval. Existing hierarchy tests, controller frozen-filter test, DOM controls and actual five-group/filter journey. |
| 4: Table | Virtualized semantic treegrid; stable selection/expansion/order; private resident values, exact unknown/partial coverage; integral scaled column resizing. Existing domain cases, held-order controller case, DOM navigation/identity/focus/resize tests and actual 125% pointer resize. |
| 5: Peak/change | Existing simultaneous group peak and full-interval baseline calculations unchanged. Existing domain tests and controller/formatter assertions. |
| 6: Overview | Rust physical accounting, fixed layer identities, real capture timestamps, gaps, explicit capacity/used/available labels. Existing attribution tests; controller partial-reading/timestamp tests; scaled SVG labels and pointer geometry tests. |
| 7: Identity/path | Rust-owned opaque selection and full path lists; repeated explicit reveal; native copy takes no caller text. PID-reuse/identity tests, hidden/offscreen keyboard entry, full-path DOM tests and independent actual clipboard readback. |
| 8: Summary | Same Rust readings and partial reasons projected into exact textual readings. Controller precision/unknown tests and DOM partial assertions. |
| 9: History | Rust metric/range/follow/manual viewport/reset; SVG gaps and keyboard data. Controller navigation tests, DOM controls and scaled hover/drag geometry tests. |
| 10: Footer/diagnostics | Coverage and collector status from Rust; keyboard dialog semantics. Existing diagnostics tests, coherent controller view, frontend controls. |
| 11: Updates | Complete revisions, serialized actions, stale poll rejection, same-revision health, failure recovery and causal explicit reveal. Transport tests and keyed DOM tests. |
| 12: Keyboard/scaling | Semantic controls, arrows/parents, Ctrl+F/Escape, F8, separate time/UI zoom. DOM navigation/shortcuts, responsive transitions; actual native resize/zoom and Windows UI Automation. |
| 13: Delivery | Old GDI production files deleted; narrow desktop commands; backend/unit, actual Windows sampler and desktop lanes; independent whole-candidate review and normal feature-branch publication. |

## Commands and boundaries

Raw logs are retained in the ignored `artifacts/tauri/` directory. Desktop data,
profiles and screenshots stay under `%LOCALAPPDATA%\mem-map-build\evidence`;
they can contain local process paths and are not committed.

- `scripts/check.ps1 fast`: 34 Rust library tests and one desktop-origin test
  pass. The final `npm run test:fast` after responsive repairs passes 24 frontend
  tests (2.28 seconds); the earlier integrated run caught an intermediate
  HappyDOM unit-conversion expectation and is not the final frontend result.
- `scripts/check.ps1 slow`: two actual Windows tests pass, asserting owned-child
  parent identity, private allocation, stable process identity and handle lifetime.
- `scripts/cargo.ps1 fmt --check`: pass.
- `scripts/cargo.ps1 -CargoArgs @('clippy','--all-targets','--locked','--','-D','warnings')`:
  pass, no denied warnings.
- `scripts/build.ps1`: optimized GNU/LLVM Tauri build passes; final bundled
  asset identity and desktop proof are recorded below. One intermediate rebuild
  was stopped during an idle Cargo wait; the offline retry completed normally.
- `scripts/smoke.ps1`: launches only the owned binary with an isolated WebView2
  profile/preferences; verifies normal process exit and unchanged executable
  digest. Native save never accepts an existing destination; independent CSV
  parsing verifies snapshot time and distinct process identities. Clipboard
  readback is restricted to a fresh sequence owned by the launched app.

Real native checks are separate from browser fixture checks. The original save
automation failures involved typed UIA provider discovery and common-dialog
filename binding. The maintained helper uses the exact owned edit, ordinary
character input and a committed filename model, then verifies the saved file.
No production dialog workaround or arbitrary frontend path API was added.

## Regression and diagnostic evidence

Genuine RED/GREEN logs cover controller boundaries, origin rejection, column
rounding, overview labels, unavailable capacity, repeated explicit reveal,
scaled chart pointer geometry, hidden/offscreen keyboard entry and responsive
layout. The native layout RED independently found a 120px inspector at an
ordinary 1280x800 window. Earlier zoom width-only checks passed and did not prove
usable vertical layout; they are not claimed as RED evidence.

The normal frontend fast lane is separate from the real Windows/WebView2 lane.
Node V8 coverage passes all 24 fast tests and reports 96.72% lines, 78.79%
branches and 73.71% functions for the six loaded production modules (test files
excluded). This is not whole-app or native-shell coverage. `main.ts` is exercised
by the actual desktop journey,
not imported by the fixture tests. Branch/function gaps remain; no full coverage
claim is made.

A fresh same-toolchain Rust coverage capability probe fails with E0463:
`profiler_builtins` is absent in Rust 1.99.1 Windows GNU. Numerical Rust coverage
is unavailable; passing behavior tests are not a substitute coverage percentage.
The raw probe and exit code are in `rust-coverage-capability.log`.

Post-session mutation is pending, without a passing score. C is NTFS and holds
the isolated build cache; D is ReFS but has about 171 MiB free. No calibrated,
immutable CoW baseline/target with verified phase-specific fast-lane selection
was established. No full-copy/shared-write mutation fallback or destructive
cache cleanup was used. Actual desktop and Windows live tests are excluded
from mutation. Any later run needs a fresh pinned source/tool/test manifest,
capacity/CoW calibration, and a hand-injected fast-lane fault proof first.

The smoke records the current Windows DPR and application zoom; it does not
alter global Windows text-size/DPI settings or claim every monitor/OS setting.
Customer acceptance, merge and deployment remain separate boundaries.

On the user's memory question, a separate launch of the initial optimized Tauri
binary (no debugging port) measured all owned descendants with the same EX2
private-working-set counter. At about 15 seconds it used 234.9 MiB total private
resident memory: 8.2 MiB host and 226.7 MiB across six WebView2 processes;
private commit was 294.6 MiB. This is a startup sample, not a steady-state bound
or a measurement of a full retained 30-minute history. Raw samples and the
one-off probe are in `evidence/tauri-memory-initial-release` on C.

## Final candidate evidence

Final `scripts/build.ps1` exits 0 in 2m22s on the cached optimized build.
`MemMap.exe` is 3,667,456 bytes, SHA256
`df526b8f9c41cbcaa1f4e2902625fd86449bf30a4ce65590001681405d7a0d8b`.
Embedded Vite asset names were verified in the binary:
`index-BifVTpAs.js` (SHA256
`3bdd985afacd77a93b813e3c13f7a71bdb512f50f55bc4d6c84f2c64ef5519cd`)
and `index-CP00Mapc.css` (SHA256
`d7e7d657a5081877071a62d966893a87a4f98fb318ecd413b39c140c09b231dc`).
GNU's matching `WebView2Loader.dll` is copied beside the executable.

The final `scripts/smoke.ps1` run passes 13 checks in 29.652 seconds against
that exact immutable binary, exits normally with code 0 and uses no forced
cleanup. Evidence is in `evidence/tauri-release-final-3` on C; raw console output
is `artifacts/tauri/desktop-release-final-3.log`. The suite has no repeated-scroll
test. It independently verifies 572 CSV identities and their frozen timestamp,
actual clipboard paths and Windows UIA selection/expansion on measured rows
(1 selected, 1 expanded, 28 measured rows exposed).

At the machine's DPR 1.75, native outer sizes 1280x800, 980x740 and 1440x900
retain full root containment. At 760x540 and at 200% application zoom the document
reflows vertically, without horizontal root overflow or overlapping panels; both
table and inspector remain usable. The actual 125% pointer column resize passes
the integral Rust preference boundary. Complete filename text rectangles and
visible path beginnings fit the field at every size and 200% zoom. Focusing the
original readonly path keeps its full value and field height. Global zoom/reset
work after that input loses focus. No renderer exceptions occurred.

| Criterion | Final evidence |
|---|---|
| AC1 | Legacy production UI deleted; unchanged sampler/dashboard/preferences; complete Rust controller and bundled TypeScript UI; release launch. |
| AC2 | Normal/responsive flow tests and native layout/path bounds. Repeated-scroll test expressly removed by the user. |
| AC3 | Retained domain tests, nine controller tests and frontend semantic/transport/geometry cases, plus real pause/filter/five-group journey. |
| AC4 | Full native clipboard/CSV/UIA outcomes, preference round-trip and real scaled pointer action; keyboard/body-focus and visible-path tests. |
| AC5 | 37 Rust and 24 frontend tests, format/lint/build and 13 actual desktop checks pass. Independent whole-candidate review: PASS. |

The independent reviewer previously identified missing keyboard entry when
selection was hidden/offscreen, mismatched scaled chart coordinates and a
filename hidden by double truncation. Each is repaired with focused tests;
actual native bounds cover the inspector-height and filename presentation gaps.
The immutable review packet records the complete manifest, retained failure
capsules and finite status/link/publication envelope.

## Independent review repair and browser editing

R1 independently reconstructed the complete candidate and returned FAIL for one
remaining specification-7 defect: fixed-character path shortening followed by
terminal CSS ellipsis hid the executable filename at 1280x800. The repaired
preview has an independently laid-out directory prefix and protected executable
tail. Only the directory gets ellipsis; unusually long filenames wrap. The full
readonly input, tooltip, accessibility description and Rust copy source retain
the unmodified path. Focus hides the preview with visibility, preserving height.

A genuine DOM RED detects the missing protected tail; a second RED prevents an
already-fitting filename shrinking in favor of directory text. All 24 frontend
tests pass after repair. A genuine actual-desktop RED against R1 also detects
the absent protected filename. The maintained native check now measures complete
tail text rectangles and visible leading-prefix space at four actual native
sizes and 200% zoom, and checks full original input/focus-stable field height.
The R2 whole-candidate packet includes the fresh release/native evidence and exact finalization envelope.

The user's additional browser-editing request is fulfilled at
`http://127.0.0.1:1420` in a retained in-app tab. Vite serves the current source
with live reload; a one-off development helper under
`%LOCALAPPDATA%\mem-map-build\preview\tauri-ui` forwards only the four existing
commands to an isolated, hidden Tauri carrier with the unchanged Rust controller.
Loopback binding, exact Host/Origin checks and an ephemeral request token guard
the development route. This helper is not part of the production bundle/source
candidate. The preview server and its owned carrier are intentionally retained;
they are excluded from cleanup and any no-active-runtime claim. The desktop
acceptance runner uses a different exact PID, profile, preferences and port.

The rendered-path focus check also exposed a specification-12 defect: after
leaving an input, focus can be on the document body, outside the dashboard
subtree that owned global shortcuts. Ctrl+F/UI zoom/F8 now bind at the document
level while preserving input editing behavior. The real desktop RED is
`desktop-release-final-2.log` (zoom remained100 after blur); the strengthened
fast test sends keys from the actual body and passes after the fix. An initial
unit failure attempted to format entire DOM objects and exhausted memory; its
RangeError is diagnostic only, not accepted behavior-RED evidence. Assertions
now compare booleans with concise messages. A readonly regression probe against the preserved R1 source snapshot also fails
with a concise Ctrl+F assertion and passes against the current source. The final
release/native proof above includes the repaired global binding. The earlier
metadata-mapping build error is not counted as passing evidence.

Independent completion review returned PASS for the complete candidate and exact
finite finalization envelope. Publication uses an unchanged-upstream fetch check
and ordinary feature-branch commit/push. No merge or deployment is included.
