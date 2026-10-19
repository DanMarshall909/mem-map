# Top-five comparison validation — MM-002

Local engineering validation on Windows, 2026-10-19, Rust 1.99.1
x86_64-pc-windows-gnu. Base: main 15c7d0c18190a57de7dfd5bfb3d74769066b4ee3.
Independent completion review: PASS; private verdict retained in artifacts/mm002/review/verdict.md.

| Criterion | Evidence |
| --- | --- |
| AC1 Ranking | Deterministic current-RAM ordering of seven active apps, distinct commit ordering, fewer-than-five and empty results. Existing Name/Path/Family/PID grouping tests protect aggregate membership. Table search/growth sorting are not inputs to comparison ranking. |
| AC2 Stable identities | Model assertions preserve pinned identity after exits and distinguish reused PIDs by creation time; explicit refresh chooses a new set. Native smoke clears comparison members before Refresh, asserts restoration, checks range preserves order, and verifies grouping refreshes identities. Colour slots are member indices. |
| AC3 Shared history | Exact numeric expectations for shared range/maximum, RAM versus commit values, missing observations, pause-sized gaps, inactive current values and an empty chart. Segments contain only consecutive observed points; no missing value becomes zero. |
| AC4 Native controls | Smoke asserts Top 5 initially, Pie and Refresh controls, exact legend selection/member filtering and return, both mode actions, and existing pie arc/legend drilling. Inspected top-five, pie, group and process renders. F6 switches view; F7 refreshes. Shared metric/range controls retain their existing paths. |
| AC5 Resources | Comparison stores at most five identities/names, reuses bounded History, and produces temporary plot data on redraw. No extra sampler or capture stream. Both native test lanes, Clippy with denied warnings, formatting, locked release and full-history redraw smoke pass. |

Commands use the existing drivers in README: check.ps1 all, cargo.ps1 fmt --check,
locked Clippy with -D warnings, cargo.ps1 build --release --locked, and smoke.ps1.
Logs, original RED/green outputs and private renderer images are retained under
artifacts/mm002, excluded from public source because they contain process paths.

Seventeen fast tests pass in 0.05 seconds, excluding compilation; one real Windows
allocation/identity/handle test passes in 0.53 seconds. These timings do not justify
a lane change. Model REDs fail on empty ranking and missing shared-plot behavior.
Native RED fails with the specific missing "pie switch control" assertion.
A later smoke fixture initially compared identity across an intentional Refresh;
the fixture now snapshots identities after Refresh, then checks range stability.

The release executable is 459,264 bytes, SHA256
`a2c00e424c16447be9dfee8c8db3f3908d4f5a2c43206d6e078c21a088c7e1d1`.
Successful smoke observed 383 accessible instances, with 11,374,592 bytes working
set and 2,109,440 private bytes before diagnostic stress. Full diagnostic history
(383 tracks by 901 points) measured 17,158,144 bytes working set and 7,884,800
private bytes. The full-history 105 redraws took 3,823 ms; GDI counts stayed 13→13.
The diagnostic repeated counter values remain separate from visible/exported
observations. These process counters do not include all GPU/kernel/GDI resources,
and are workload measurements, not a promise for every machine or process count.

Coverage retains the verified Windows GNU profiler_builtins blocker documented in
validation.md. No coverage percentage is claimed. Mutation remains unrun: source
coverage/lane mapping and an isolated CoW harness have not been established.
External mouse/keyboard/save-dialog automation, complete UI Automation semantics,
hosted CI and business acceptance remain unproved. Controller/hit-test assertions
and images are narrower engineering evidence.

Project grouping is not implemented. Name, executable path and validated process
families remain available. Project inference can mistake wrappers, shared tools,
monorepo roots and editor-launched helpers for ownership; future labels should
show their evidence and allow manual correction.

The primary checkout, its running app and captures are preserved. This task uses
its own canonical worktree and build outputs. Publication is a feature branch;
integration into main is separate. No cache, process or unrelated worktree cleanup
is planned.
