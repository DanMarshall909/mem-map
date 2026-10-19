# MM-007: Coherent system-memory dashboard

status: Resolved

## Objective and authority

Implement the complete user-provided [implementation specification](../../../system-memory-spec.md)
received 2026-10-19: dark grey/orange/green native Windows dashboard answering
system usage, attribution, net change and selected-process history. All 13
numbered sections and the acceptance table own scope. Source base:
`origin/main 796cc5bf73baf791fab051524980fad4150b1c0a`.

Windows worktree: `D:\worktrees\personal\mem-map\system-memory-redesign`.
Branch: `feat/system-memory-redesign`. No merge, deployment or shared-rules
change is authorized. The user's later "cleanup worktrees" instruction owns the
completed administrative cleanup recorded below.

## Acceptance criteria

- AC1 (spec 1-2): shared dark theme, native window, responsive three-region
  layout, remembered splitter, coherent physical totals and real collector state.
- AC2 (spec 3-4): accessible single-select grouping, true ancestry tree while
  preserving Family, debounced full-data filtering, exited/compact controls,
  resizable virtual table, stable selection/order, exact optional/partial values.
- AC3 (spec 5, 11): simultaneous rolling group peak; strict documented change
  baseline and endpoint coverage; confirmed lifecycle; stable instance keys;
  coherent display snapshot, pause display while recording, paused export.
- AC4 (spec 6): zero-to-capacity cumulative physical stack, fixed three executable
  identities, residual/other-process bands, real endpoint/headroom annotations,
  independent ranges, accessible values, explicit failures and accounting gaps.
- AC5 (spec 7-9): accurate selection identity/counts/path copy and disclosure,
  shared summary calculations, independent metric/range/y-axis/follow controls,
  green history with pan/zoom, real retained span and missing-data gaps.
- AC6 (spec 10-12): scoped footer/diagnostics, preferences, keyboard hierarchy,
  accessible controls/table/chart data, DPI and application zoom.
- AC7 (spec 13): deterministic acceptance cases, actual Windows collection,
  native controller/render smoke, visual inspection, static/build checks and
  independent completion review against all numbered requirements.

## Verification evidence

[Dashboard validation](../../../system-memory-validation.md) maps all numbered
requirements to implementation and falsifiable evidence. Current local checks:
35 fast tests, 2 native Windows boundary tests, format, denied-warnings all-target
Clippy and release build pass. The external native UI Automation client passes
39 assertions against the recorded unchanged release bytes.

Data regressions include complete scans outside the detailed-history cap, strict
change/peak, full Name/Path/Family aggregate retention, PID reuse and unknown
lifetimes, selected identity with every known path/member beyond 1,024 tracks,
immediate selected exit, and coherent frozen-selection handoff. UI regressions
cover partial values, stable held ordering, transient chart zoom/reset/range,
Follow-off latching and rendered table/chart/legend layout at enlarged scales.

The first frozen-candidate review returned FAIL for two corrections: initial
elapsed-time-zero snapshot publication and missing named-band failure assertions.
Both are repaired. The real native first-frame regression reproduced the empty
dashboard, then passed with no second frame and after producer closure. Named
reading failure and incomplete enumeration now assert preserved valid system RAM
with neutral unavailable attribution. Combined checks and final exact-binary
native accessibility were rerun. The next independent review identified no
remaining source or test correction, but required successful full-path clipboard
evidence before publication.

On 2026-10-20 Brisbane time, the unchanged executable passes the full native smoke
and a separate 20-assertion actual-button probe. Copy all paths delivers three
distinct originals; Copy path delivers one exact original whose native text width
exceeds the field. Fresh product-owned clipboard payloads match complete UTF-16
bytes including hidden text and terminating NUL. The application exits normally,
and its before/after hashes match. Earlier denied/client failures are retained;
no station, ACL or token privilege change was made.

Independent review passed candidate 3, but a subsequent user screenshot showed
that the dashboard exceeded the real client height and scrollbars obstructed the
layout. That PASS is invalidated before any finalization or publication. Window
fit and scrollbar behavior were reopened under the same task. Genuine native
containment and screen-paint REDs now pass after responsive layout, measured
numeric widths, wrapping controls, reserved scrollbar gutters and supported
push-button owner drawing. Actual screen captures at the supplied window size
and shorter/narrower sizes show the footer and all six columns in view. Selected
history remains 100–155 DIP high with readable axes and caption. The new release
passes 35 fast tests, two Windows boundary tests, native smoke, 39 external UIA
assertions and 20 actual Copy-button assertions. Whole-candidate independent
review returned PASS for the corrected candidate and its exact finalization
plan. Feature-branch publication is recorded separately from integration or
deployment.

Numerical source coverage is unavailable: the current Windows GNU compiler lacks
`profiler_builtins` (fresh E0463 probe). Mutation is pending without calibrated
lane attribution or a verified copy-on-write harness. These are disclosed limits,
not fabricated percentages or scores.

## Decisions and ownership

- Scope remains the entire supplied specification. EX2 private working set and
  private commit retain distinct counter meanings. Actual Process tree ancestry
  is added while earlier launcher-bounded Family grouping remains available.
- Shared rules-sync preflight reported checkout drift; no shared policy changed.
- Root owns the assembled candidate, compiler/runtime slot, tracker, evidence and
  Git. Data, UI and accessibility writers have handed back all source and owned
  processes. A retained independent reviewer assesses the full frozen candidate.
- Genuine RED/GREEN outputs, review findings, repair capsule and captures remain
  under ignored `artifacts/redesign`. They can contain local process paths.
  Retention contract lessons are recorded there; no standing guidance or memory
  update was authorized or made.
- A real GNU BFD TLS-import startup crash was diagnosed with LLDB. The driver
  chooses installed clang/LLD plus Rust's matching GNU runtime, preserving caller
  flags. Immediate UIA later found unchecked startup controls; creation now
  synchronizes loaded preference state before collection.
- Reusable debug/test outputs are preserved. The ignored task release junction
  points to `C:\Users\DanMarshall\AppData\Local\mem-map-build\targets\system-memory-redesign-release`.
  Runtime captures are task-owned on C; no unrelated cache deletion is inferred
  from low D drive capacity.

## User-authorized worktree cleanup

The user requested "cleanup worktrees" during implementation. Root freshly
verified main, complete relevant status/worktree/ref/stash state, live PRs,
ancestry and target-owned processes. Clean integrated `app-icon`, `stacked-area`
and `top-five-overlay` worktrees and their merged local branches were removed
using ordinary Git commands. Remote branches were retained. Primary main and the
active redesign worktree were kept.

Every ignored output/cache was moved to the Git common directory and verified by
length and SHA256. Each source ZIP was checked against every Git blob. Recovery
maps, original heads/paths, hashes and before/after snapshots are retained at
`D:\code\personal\mem-map\.git\codex\worktree-cleanup-20261019-214947`.
No force removal, cache deletion, reset, stash or active-work mutation occurred.

## Resolution

Resolved on 2026-10-20 Brisbane time after independent PASS. The complete scope,
including the user's window-fit correction, is covered by the linked evidence
matrix, 35 fast tests, two real Windows tests, 39 external UIA assertions, native
rendered acceptance and 20 actual Copy-button assertions. The final executable
remains unchanged across the final native acceptance runs. Coverage instrumentation
is unavailable and mutation remains pending, as disclosed in validation.
