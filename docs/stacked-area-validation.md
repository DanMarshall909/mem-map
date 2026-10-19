# Stacked area validation - MM-006

Windows, 2026-10-19, Rust 1.99.1 x86_64-pc-windows-gnu. This extends the icon
candidate 4362d4b01fe002c2e6951f89101265b1f8ff910e, already based on main
0ac30df9595b990deb345e8c7e9697b4951efaf3. Independent completion review: PASS; private verdict retained in artifacts/stacked/review/verdict.md.

| Criterion | Local evidence |
| --- | --- |
| AC1 Cumulative bands | The owning model test asserts exact lower/upper boundaries, individual current values, RAM versus commit, zoom, three bands, zero values and an empty comparison. Distinct peak times assert the maximum concurrent stack rather than a sum of independent peaks. Existing ranking tests and native smoke retain the five-member bound. |
| AC2 Honest gaps | Genuine RED fails because the former independent lines plot an upper process at a timestamp with an unknown lower baseline. GREEN omits that area while retaining its known individual legend value. Exact segments assert recovery after missing observations, pause-sized gaps and an all-missing current sample. |
| AC3 Native rendering and controls | The shared GDI polygon renderer fills upper points forward and lower points in reverse; isolated samples are vertical band spans. Native smoke passes pinned Refresh, grouping, metric, legend drilling, pie switching, parent navigation and both zoom paths. Inspected full-history screenshot shows five contiguous coloured bands, a cumulative axis and individual legend values. Existing 200 percent overflow rendering remains readable. |

Twenty fast tests pass in 0.04s, excluding compilation. Two real Windows tests
pass in 0.76s. No timing evidence justifies changing the existing lane selections.
Formatting, locked Clippy with denied warnings, locked release and native smoke
pass. Commands are scripts/check.ps1 all, scripts/cargo.ps1 fmt --check, the
existing cargo driver with clippy --all-targets --locked -- -D warnings and
build --release --locked, and scripts/smoke.ps1 -OutputDirectory
artifacts/stacked/smoke. Private logs, RED/GREEN, screenshots and CSV remain under
ignored artifacts/stacked. Smoke reports result=PASS; its outer PowerShell command
exited 0.

Release size: 491,008 bytes. SHA256:
`4529e160d2283cc5e5fd0202fdf2c076b56eed4d19e27c057e08254ab7abf98f`.
The native run observed 343 process instances. Private RAM was 2,551,808 bytes
and working set 15,880,192 before stress. With 329 tracks by 901 points, private
RAM was 8,179,712 and working set 21,516,288 bytes. The 105 full-history redraws
took 3,600ms; GDI counts remained 28 to 28. These are workload measurements,
not a long-duration soak or total GPU/kernel accounting. The full-history image
uses diagnostic repeated counter values, restored before real history export;
it does not represent 30 minutes of live observations.

The comparison reuses the same bounded history and sampler. It adds one temporary
baseline vector and per-band polygon vertices during redraw, with no persistent
capture or runtime dependency. Production refactoring reuses the existing polygon
resource lifecycle and common graph times; tests reuse the sample/frame fixtures
and a small numeric boundary constructor. Independent upper values cannot supply
a missing cumulative baseline. The individual-process history stays single-series.

Source coverage retains the verified Windows GNU profiler_builtins blocker.
Post-session mutation remains pending for this final source snapshot: no current
source-bound lane coverage map or isolated CoW harness is established. No score is
claimed. Native smoke uses in-process window messages and the shared renderer;
external input, actual monitor moves, full UI Automation, hosted CI and user
acceptance remain unproved.

Publication updates the existing feature PR #2 with the icon and stacked chart.
The isolated local branch feat/stacked-area pushes normally to origin/feat/app-icon.
The older local app-icon branch/worktree and running captures stay at their prior
revision; they are preserved, as are other worktrees, evidence and warm caches.
No merge or cleanup is included.

## Cosmetic follow-up: remove grid lines

At the user's request, removed the horizontal grid strokes drawn over the stacked
bands. Axis labels, coloured areas and legend hit targets remain. Inspected the
new full-history render in artifacts/grid-free/smoke/stacked-full-history.bmp.
All 22 existing tests, formatting, denied-warning Clippy, locked release and native
smoke pass; GDI counts remain 28 to 28 across 105 redraws. No new test is needed
for this reversible drawing-only deletion. The independent review above records
the original stacked-area candidate; this follow-up uses visual verification.
The 491,008-byte follow-up release has SHA256
`2704876334b93f5ed3d408788d1191a5f68a76d8cdbf4bb9f230328fbd2410f4`.
