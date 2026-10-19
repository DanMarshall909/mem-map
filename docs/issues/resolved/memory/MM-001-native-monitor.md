# MM-001: Lightweight native memory monitor

status: Resolved

User scope: a small Rust app that explains system memory with a pie chart and
per-app/process history, with low memory overhead itself.

- AC1: Physical-memory pie partitions total RAM into measured private process RAM,
  other in-use memory, and available RAM; it never sums shared working sets.
- AC2: Group by executable or inspect individual process instances. Search and sort
  by RAM or growth; distinguish PID reuse using creation time.
- AC3: Graph RAM and private commit for the selected app/process, retain 30 minutes
  at two-second intervals with hard point/series caps, and retain inactive history.
- AC4: Pause/resume inspection and export captured history to CSV on request.
- AC5: Produce a standalone native Windows executable and measure its memory/CPU.
- AC6: Make process details occupy most of the window, with a dense table and a
  compact pie and details sidebar.
- AC7: Clicking an app pie slice or legend drills into its process members with
  RAM/commit history and PID, parent PID, executable path and memory details.
- AC8: Aggregate same-name processes across install paths (especially node.exe),
  with executable-path, parent/child application-family and individual PID modes.
  Family grouping stops at general Windows launchers and validates parent age.
- AC9: Show sampled peak private RAM in the table and details. Group peaks use
  simultaneous totals across retained history, up to 30 minutes.
- AC10: Growth has a configurable lookback separate from graph zoom, with custom
  seconds/minutes bounded by retention. Invalid input preserves the prior setting;
  group changes compare simultaneous measured endpoints, including instance exits.

Implementation: Rust + windows-sys, Win32/GDI. No web runtime, background service,
telemetry, automatic disk history, process termination, or working-set trimming.
Requires current patched Windows 10 22H2 / Windows 11 (EX2 memory counters).

Validation: fast deterministic model tests, slow real-Windows sampler contract,
release build, live allocation probe, rendered/UI smoke, independent review.
Fast lane target: under two seconds excluding compilation; slow lane owns OS I/O.
Coverage and mutation proof are reported separately from passing tests.

Workspace: initially empty directory without a Git repository or remote; work is
created directly in the requested folder. The user subsequently authorized initial
publication to the new public `DanMarshall909/mem-map` repository. Its `main` branch
starts without an existing base commit; no prior upstream work is being replaced.

## Resolution

AC1–AC10 have engineering validation in docs/validation.md. Independent completion review: PASS, recorded in artifacts/review-v3/verdict.md. Local model/native tests, assertive smoke, resource and full-history rendering checks support the compact memory monitor with clickable groups, sampled peaks and configurable growth. Source coverage, mutation, external-input/save-dialog acceptance and full UI Automation remain limited as documented. Initial Git publication is separately authorized; hosted acceptance is unproved. Review artifacts remain private local evidence and are excluded from the public repository.
