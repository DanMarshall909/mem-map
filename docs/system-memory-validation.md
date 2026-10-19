# System-memory dashboard validation — MM-007

The complete [specification](system-memory-spec.md), including the user's window-fit
correction, is implemented and locally validated. Independent completion review
passed for the exact corrected candidate and publication plan;
[MM-007](issues/resolved/memory/MM-007-system-memory-dashboard.md) records resolution.
Git records feature-branch publication separately from integration or deployment.

Windows worktree: `D:\worktrees\personal\mem-map\system-memory-redesign`.
Branch: `feat/system-memory-redesign`; base: `origin/main`
`796cc5bf73baf791fab051524980fad4150b1c0a`. Primary main remains separate.

## Candidate verification

The final driver runs formatting, fast and slow lanes, denied-warnings all-target
Clippy and the release build. All passed: **35 fast tests** (26 library, 9 native
binary/controller/COM contracts) and **2 real Windows boundary tests**. The slow
tests observe an owned touched 64 MiB allocation, real creation/parent identities
and repeated handle release. Native external UI Automation passed **39 assertions**
against release SHA256
`956afa41a894d83e51d477a916c0cb1dadde071602eadc28bfbe5ab2e94af918`.
The binary remained unchanged and the owned test application closed normally.

Private command, output, duration, exit-code, binary and source-hash evidence is
retained under ignored `artifacts/redesign/window-fit-repair`, with earlier
verification and failed attempts preserved. Local captures can
contain process paths and are excluded from publication. These are local checks,
not CI, deployment or stakeholder acceptance.

## Requirement and evidence map

| Specification | Implementation | Evidence and limit |
|---|---|---|
| 1: theme, layout, density | Shared drawing tokens, native controls, remembered thin splitter, responsive client layout and scrolling inspector | Eight native fit cases, including 1300x700 and 1300x800, injected 168 DPI, app/text scaling, plus extreme-size access checks. Actual foreground screen captures at the user's 2304x1452 size and shorter/narrower sizes show the footer and all six columns without outer or table-horizontal scrolling. |
| 2: header, pause, CSV | Coherent displayed History, worker collection, display-only freeze and distinct-instance export | Actual native first-at-zero header/table/overview/inspector publication with no second frame and a closed producer; pause/record/resume and export assertions; exact-byte/filter/original-path CSV test; stopped/stale recorder test. |
| 3: toolbar and filtering | Radio grouping, real ancestry plus retained Family grouping, 200 ms debounce, full-data filter, exited/compact/change controls | Filtered hierarchy/full totals test; native checkbox/radio UIA; group/filter/overview invariance, density and label checks. |
| 4: process table | Owner-data native tree grid, stable identities/order, sortable resizable columns, measured numeric minima and shared bar scales | Independent COM hierarchy/selection/expansion/value contracts; actual header/value geometry, 10-digit PID reserve, numeric width before decorative bars, external selection/header-sort checks, row pixels and held ordering. Narrow tables scroll only when the readable value budget requires it. |
| 5: peak and change | Simultaneous full group samples, rolling 30-minute peak, last complete baseline at/before target with maximum age two intervals | Strict insufficient-history, simultaneous peak, baseline, omitted Family member/exit and retained-group-span regressions. No shortened comparison or sum of separate peaks. |
| 6: physical overview | Fixed three names, unique private resident attribution, other processes, residual, zero-to-capacity geometry and explicit gaps | Deterministic 32/8/6/3 GiB decomposition; private commit separation; named-member reading failure and incomplete enumeration preserve neutral valid system totals with explicit unknown attribution; inconsistent decomposition tests; native fixed-band/filter invariance, legend/caption geometry and rendered capture checks. |
| 7: selected identity/path | Full known paths, path hierarchy, selected-identity cache separate from detailed history, explicit incomplete-path labels | Beyond-1,024-member, immediate-exit, frozen-selection, original-case paths and compact same-name/different-path tests. Actual native Copy path and Copy all paths actions pass exact original UTF-16 clipboard comparisons, including hidden text and three distinct paths. |
| 8: inspector summaries | Shared summary/reading calculations, explicit partial current/commit/peak coverage | Partial-group controller test and model peak/change tests; native numeric labels and external accessible values. |
| 9: selected history | Independent metric/range/y-axis, transient navigation, latched Follow-off endpoint, Reset, practical plot height | Range-combination test; native zoom/reset/range/follow/pause/resume transitions; full green plot, units, axes and requested/recorded caption at 150/200%, with 100–155 DIP plot height. Missing/partial values have no ordinary complete endpoint. |
| 10: diagnostics | Current observed/measured/enumeration scope separate from historical failures, scrollable Details | Current unavailable/enumeration regression; native recording state; source and UI inspection of retained samples, omitted tracks and own footprint. |
| 11: update/retention/preferences | Immutable display freeze, worker snapshots, PID/creation/provisional identities, full aggregate history and selected cache | PID reuse, unknown lifetimes, parent metadata, complete-scan exit, cap-pressure/frozen selection regressions; native paused collection; preference round trip and invalid-field rejection. |
| 12: keyboard/scaling | Native controls plus table provider, documented shortcuts, separate UI/time zoom, wrapping and local scrolling | External 39-assertion client, real COM tests and native scale/capture checks. Only extreme sizes need outer overflow to retain readable controls. This is automated/native inspection, not a human screen-reader usability study. |
| 13: acceptance | Combined deterministic, real Windows, controller, rendering and independent review gates | All local cases map above and pass on the corrected executable; independent whole-candidate completion review returned PASS. |

Genuine RED outputs and subsequent repairs are retained privately. Preliminary
review found group-span truncation, omitted Family history, selected identity and
freeze losses, partial endpoint/summary inconsistencies and navigation defects.
Later rendering checks found a blank native table, overlapping overview caption,
lost inspector capture origin and clipped enlarged plot. Those checks remain
strict after repair. An immediate external UIA check also found initially
unchecked grouping controls; window creation now synchronizes preference state,
and the unchanged client passes on the rebuilt executable.

The first independent frozen-candidate verdict was FAIL. Its two corrections are
now repaired and revalidated: the first completed elapsed-time-zero frame publishes
immediately, and the overview contract explicitly asserts neutral attribution on
named-band measurement failure and incomplete enumeration. The first-frame native
regression reproduced an unavailable header, zero table rows and empty chart
caches before repair; it now checks actual native controls and cached measurements
before a second frame and after producer closure. Independent review of these
repaired bytes found no remaining source or test correction and returned BLOCKED
solely for the missing successful full-path clipboard result. A resumed-session
native smoke and a separate 20-assertion actual-button probe now supply that
evidence on the same executable. That candidate subsequently passed independent
review, but the user's later screenshot invalidated its fit acceptance before
any finalization, commit or publication. The corrections below formed the new
candidate that received independent whole-candidate PASS.

## Window fit and native screen painting

The user-observed defect came from a forced virtual canvas and fixed minimum
column widths. The earlier checks proved scroll reachability relative to virtual
panels, rather than actual client containment. Genuine native RED output now
records the footer below the client and 1,086 pixels of headers in an 829-pixel
table. Corrected checks observe populated native controls and actual ancestor
bounds at ordinary sizes, DPI and text scales. Exact numeric labels take priority
over bar and identity-column space; toolbar and inspector controls wrap.

Actual foreground screen captures at 2304x1452, 2304x1240 and 1900x1452 physical
pixels pass at the real monitor's 144 DPI. Header, footer and all six table
columns fit. Scrollbars for long lists and inspector content use their own
gutters; the ordinary window has no outer scrollbar. A 640x480 window at 200%
zoom is an explicit extreme fallback: outer scrolling preserves access to the
last history-value and zoom controls without shrinking the fonts.

Real native text updates also reproduced white legend, Pause and Copy buttons
despite correct WM_PRINT captures. Supported owner drawing now covers custom
push buttons while preserving checkbox/radio types and native invocation.
Actual window-DC pixels, external UIA and Copy actions pass after label changes.
The inspector history retains a readable 100–155 DIP plot with its full axes,
units and requested/recorded caption; it is not compressed to fit by hiding data.

## Native boundary and performance

The corrected release passes the full native smoke. A separate external UI
Automation probe passes 20 assertions
through the actual Copy buttons: Copy all paths returns three distinct original
paths (226 UTF-16 units, 454 bytes including NUL), and Copy path returns one exact
original (95 units, 192 bytes). Target-native EDIT character positions prove a
text-width lower bound of 978 pixels exceeds the unfocused field's 439-pixel
client width. Both payloads match every original UTF-16 byte, including hidden
text and the terminator. Reads occur only after the invoked product owns a fresh
clipboard sequence. The owned application closes normally; binary hashes match.

Earlier session access-denied failures and client-probe failures are preserved.
No station, ACL or token privilege was changed to obtain the passing result, and
no unrelated clipboard content was read. The changed session capability is an
observation; its cause is not inferred.

The final native run retained six real samples and 432 process tracks; 100
redraws took 200 ms with GDI objects stable at 44. A separate diagnostic retention
history used 432 tracks across 901 timestamps: Name refresh 316 ms, chart buffers
41 ms, 100 redraws 343 ms, GDI 44 to 44, private commit 67,604,480 bytes and working
set 80,429,056 bytes. These are diagnostic observations, not performance guarantees
or proof at the maximum 1,024-track cap. Real collection and synthesized retention
runs are labelled separately. Final captures, command results and independent
review are retained in the private packet.

## Toolchain and instrumentation limits

A pre-main crash was traced with LLDB to unresolved TLS imports produced by GNU
BFD 2.44. The existing driver now selects installed clang/LLD 23.1.3 with Rust's
matching self-contained GNU runtime. Loaded imports resolve correctly; caller raw
and encoded Rust flags are preserved idempotently. No tool is installed by this
change.

The recorded Rust 1.99.1 Windows GNU coverage capability probe lacks
`profiler_builtins` and fails with E0463. No line or branch percentage is
claimed. Mutation is pending, with no calibrated per-lane source attribution or
verified copy-on-write mutation harness. Its future scope is the final immutable
model/dashboard/preferences/controller source with slow and E2E tests excluded;
pending tooling is not passing mutation evidence and does not itself block review
or publication. No safe mutation worker has been started.
