# Mem Map

A native Windows memory monitor in Rust. Build and run **MemMap.exe**. No installer,
elevation, background service, process termination or memory trimming is required.

The dark dashboard combines a virtual process tree grid, an overall-memory
stacked area chart and a scrolling inspector. Header, table, overview and summary
readings come from one completed collection snapshot.

- **Overall memory usage** always represents physical RAM from zero to capacity.
  Three prominent executable names are pinned after the first collection. Other
  measured private working sets and residual system usage complete the stack.
  Filtering, sorting and selection do not replace the bands. Failed system
  samples leave gaps; uncertain attribution retains a neutral system total.
- **Name**, **Path**, **Process tree** and **Individual processes** show executable
  groups, complete-path groups, actual ancestry and flat instances. **App families**
  retains the earlier launcher-bounded grouping. Process-tree rows show each
  process's own measurements. Filtering retains matching descendants' ancestors
  and keeps group totals based on full membership.
- **Private working set** measures private resident physical memory; **Private
  commit** measures private committed virtual memory. Peak uses simultaneous
  group totals within the retained 30-minute window. Change uses the last valid
  complete sample at or before the requested target, at most two collection
  intervals old. Insufficient or incomplete endpoints have explanations.
- **Pause display** freezes displayed readings, ordering and chart endpoints while
  recording continues. Selection, filtering and full-path copying remain usable.
  Resume advances to the latest completed snapshot. A history chart with
  Follow latest unchecked retains its inspected endpoint.
- Selected history has independent metric, range, Y-axis and Follow latest
  controls. Wheel zoom and dragging navigate time. Reset view restores following
  while retaining metric and requested range. Missing observations remain gaps.
- **Export CSV** exports distinct filtered instances from the displayed snapshot,
  including offscreen records. Expansion cannot duplicate aggregate values. The
  identity, elapsed-time and raw KiB columns are retained, with capture timestamp,
  exact byte values, lifecycle, measurement reason and provisional generation
  appended. Unknown creation times and readings stay blank. Paused exports use
  the paused snapshot.
- **Details** contains collection failures, coverage, last successful collection,
  retained samples, omitted-track information and the monitor's own footprint.

A worker collects every two seconds. Creation time distinguishes PID reuse;
unidentified lifetimes have separate provisional generations. Current rows and
CSV use the complete scan independently of bounded historical retention. History
is session-only: up to 30 minutes, 901 sample times and 1,024 detailed process
tracks, with selected history receiving retention priority. Confirmed exits and
inaccessible measurements have distinct states.

Selected identity is retained separately from detailed history: names, full known
paths, member counts and one last leaf observation remain inspectable after an
exit. Incomplete path coverage is labelled explicitly.

Presentation preferences are stored in `%LOCALAPPDATA%\MemMap\preferences.ini`.
They include grouping, density, splitter, columns, chart ranges, Y-axis mode and
UI zoom. Process paths, filters and captured history are not saved there. CSV is
saved only on request; local captures and binaries are excluded from Git.

## Keyboard and accessibility

Tab moves through native controls, table and inspector. Segments have radio
behavior; Space activates focused checkboxes/buttons. Table arrows navigate,
expand, collapse or move to a parent. Ctrl+F focuses the filter; Escape clears it.
Ctrl+wheel and Ctrl+plus/minus scale the application; Ctrl+0 restores normal UI
zoom. Chart navigation remains separate. F2 cycles grouping, F3 switches memory/
change sorting, F4 toggles exited instances, F5 switches history metric and F8
navigates to a retained parent.

Native controls and a UI Automation provider expose names, measurements,
hierarchy, expansion, selection and sort state. Read-only chart value fields
support inspection without a mouse. DPI, Windows text size and UI zoom adapt the
layout. The table measures numeric widths before allocating bar space, controls
wrap, and ordinary windows keep their header and footer visible. Long process
lists and inspector details scroll inside their panels; extreme window sizes
retain an overflow fallback for readable access.

## Build and verify

Requires 64-bit Windows 10/11 22H2 with September 2023 updates or newer for the
[EX2 private-working-set counter](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex2).
Available memory includes immediately reusable standby, free and zeroed pages;
see [Microsoft's memory-status definition](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/ns-sysinfoapi-memorystatusex).

Use stable Rust and a compatible Windows linker/resource compiler. The PowerShell
driver detects the isolated toolchain under `%LOCALAPPDATA%\mem-map-build`; it
does not install tools. For this Windows GNU setup it selects LLVM clang/LLD
and Rust's matching runtime. The affected GNU BFD linker can produce malformed
imports with the current Windows bindings despite a successful link.

```powershell
.\scripts\build.ps1
.\scripts\check.ps1 fast  # model, preferences, provider and collector-status contracts
.\scripts\check.ps1 slow  # real Windows allocation, identity and handle lifetime
.\scripts\check.ps1 all
.\scripts\cargo.ps1 fmt --check
.\scripts\cargo.ps1 -CargoArgs @('clippy', '--all-targets', '--', '-D', 'warnings')
.\scripts\smoke.ps1
```

The native smoke uses real collection and controls, checks paused recording,
export and path copying, renders several sizes/scales and measures GDI resources.
A separate diagnostic history exercises full retention without inventing visible
or exported observations. Captures can contain local process paths and remain in
ignored `artifacts/`.

The accepted redesign is in [the specification](docs/system-memory-spec.md).
Current evidence is in [dashboard validation](docs/system-memory-validation.md).
Earlier `docs/*-validation.md` records remain historical evidence for superseded
views. The icon source and exports remain in `assets/`; `scripts/icon.ps1`
regenerates exports with Windows WPF.
