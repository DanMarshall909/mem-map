# Mem Map

A small, native Windows memory monitor written in Rust. Run **MemMap.exe** after
building, or launch `target\release\mem-map.exe`. No installer or background service.

- A dense table occupies most of the window. The narrow sidebar holds the pie,
  selected process details and history. Click an app slice or legend to inspect
  its processes; double-click a group or use Expand to do the same.
- **Top 5** overlays the five largest active groups on one time and memory scale.
  Click a legend to inspect its members. Switch to **Pie** for the physical-memory
  breakdown. The selected-process history remains available below either view.
  Top 5 follows Name/Path/Family/PID grouping and the RAM/Commit metric, independently
  of table search and growth sorting. Colours and identities stay pinned across
  samples and exits until **Refresh**, grouping or metric changes. Missing data
  and pauses leave gaps; **Unavailable** does not mean zero. Both charts share the
  5/15/30-minute zoom. Comparison history reuses the existing bounded capture.
- Physical-memory pie: the largest apps, other apps, system/shared/unmeasured RAM,
  and available RAM. Shared mappings are not added repeatedly across processes.
- **Name** combines same-name processes across install paths, giving all `node.exe`
  instances one collective total. **Path** separates installs; **Family** combines
  parent/child processes; **PID** lists individual instances. Families stop at
  general Windows launchers (Explorer, service managers, shells and terminals),
  validate parent creation time and retain known ancestry after the parent exits.
- Select a row for its RAM or private-commit graph; hover for an exact sample.
- Individual process rows and details include the parent name and PID. Parent names
  come from observed processes older than their children and are retained after
  exit. A name that cannot be established is **Unavailable**; PID reuse does not
  replace an already observed parent. Processes without a recorded parent show **None**.
- Click a highlighted parent in a row or the details to jump to that exact process
  instance. Navigation clears search and group scope and includes inactive history.
  Expired or unobserved parents are not links; a reused PID cannot redirect them.
- Consolas is used throughout, including the native search and growth controls.
  Scale the interface from 75% to 200%; scrollbars expose enlarged content.
- Individual details include executable path, PID, parent PID, RAM, private commit,
  working set, growth, peak and retained sample count.
- Sort by RAM or RAM growth, search names/parent names/paths/PIDs, and inspect inactive history.
- **Peak RAM** is the highest sampled private RAM in retained history, up to 30
  minutes. Group peaks use simultaneous totals, so peaks from different times
  are not added together. Spikes between samples are not measured.
- **Growth over** is independent of graph zoom. Select a preset or type `90s`,
  `7m` or `15` (minutes). Range: 1s–30m at two-second sample resolution.
  **Session** uses all available retained history.
- 30 minutes of session history, sampled every two seconds. Zoom either chart's
  time window from 30 seconds to 30 minutes, or choose 5/15/30-minute presets.
  Zoom preserves captured history and pinned comparison identities.
  Pause freezes collection; resume leaves time gaps visible.
- CSV export includes process identity, elapsed seconds, RAM and commit in KiB.
  Missing observations are blank, rather than zero. Nothing is saved automatically.
- The monitor shows its own private RAM and total working set.

Requires **64-bit Windows 10 22H2 or Windows 11 22H2 with September 2023 updates,
or newer**. It uses documented
[EX2 counters](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex2).
Normal user privileges suffice for accessible processes. Protected processes are
counted as unmeasured; their RAM remains in the system remainder. No elevation,
process termination or memory trimming is attempted.

## Reading the numbers

**Private RAM** is physical memory belonging exclusively to that process.
**Commit** is private committed virtual memory, which may reside in RAM or have
backing elsewhere. It is not actual pagefile usage. Comparing the two helps find
growth even when Windows pages memory out. **Available** includes standby memory.
See Microsoft's [working set explanation](https://learn.microsoft.com/en-us/windows/win32/memory/working-set).

Growth compares current RAM with the first captured value inside the chosen
lookback, limited to available history and the most recent continuous segment.
Missing observations or long pauses reset that baseline. Group growth compares
simultaneous group totals, including processes joining or leaving. Availability
changes also affect measured totals. A lookback shorter than the sample interval
may contain only one point and show zero observed change.
“Inactive” means not observed in the latest scan, including exited or inaccessible
processes. History is session-only; closing the app clears it.

To bound overhead, retain at most **1,024 process instances** and **901 points per
instance**. Old inactive instances are evicted first when space is needed. The
footer reports omitted tracks. Compact history points use 12 bytes (KiB precision),
with roughly 12 MiB of allocated ring storage at the hard cap, plus metadata and
the native drawing buffer. It does not scan individual memory pages or modules.
Sampling is not atomic; pie slices are capped at measured in-use memory.

## Controls

| Action | Shortcut |
| --- | --- |
| Pause / resume | Space (outside search) |
| Cycle Name / Path / Family / PID | F2 |
| Expand selected group | Enter or double-click |
| Return from expanded group | Backspace or All groups |
| RAM / growth sort | F3 |
| Include inactive | F4 |
| RAM / commit graph | F5 |
| Top 5 / pie overview | F6 |
| Refresh top five | F7 |
| Go to selected process's parent | F8 or click highlighted parent |
| Graph window | 1 / 2 / 3 |
| Chart time zoom | + / −, chart buttons or wheel over either chart |
| Interface zoom | Ctrl+wheel, Ctrl+plus / minus or UI − / + |
| Reset interface zoom | Ctrl+0 or click the percentage |
| Select process | Up / Down |
| Search | Ctrl+F |
| Clear search / return to table | Escape |
| Scroll process list | Mouse wheel outside charts |
| Pan enlarged interface | Window scrollbars |

## Build and verify

Install stable Rust with either the MSVC desktop C++ prerequisites or a working
MinGW toolchain. The PowerShell driver also detects the isolated toolchain used
for this workspace at `%LOCALAPPDATA%\mem-map-build`; it does not install tools.

```powershell
.\scripts\build.ps1
.\scripts\check.ps1 fast    # deterministic model tests; target <2s excluding compilation
.\scripts\check.ps1 slow    # actual Windows counters, 64 MiB allocation and handle lifetime
.\scripts\check.ps1 all
.\scripts\cargo.ps1 fmt --check
.\scripts\cargo.ps1 -CargoArgs @('clippy', '--all-targets', '--', '-D', 'warnings')
.\scripts\smoke.ps1
```

The hidden smoke run collects real samples and asserts view, metric, range,
selection, custom/invalid growth duration, peak invariants, pie hit-testing and
drill-down outcomes. Pause must freeze observations
across two sampling intervals; resume must add an observation with a visible gap.
It exports CSV, renders main/group/process BMPs, fills a **separate diagnostic history** to its
cap with repeated counter values, measures the resulting footprint, and checks
GDI resource counts across 100 redraws of full diagnostic history. Repeated values never enter
visible or exported history. Artifacts may contain local process paths;
`artifacts/` is ignored. This is automated engineering evidence, not a screen-reader
or user acceptance test. Top-five switching, refresh, stable identities and legend
drilling also have controller assertions. Charts and custom buttons do not currently expose full
UI Automation semantics; keyboard controls are provided.
The native smoke also drives parent links, chart/Ctrl wheel zoom, overflow scrollbars
and DPI messages; it checks actual font metrics and native control placement.
These in-process message checks do not prove external mouse/keyboard input or
physical monitor transitions.

Source: [DanMarshall909/mem-map](https://github.com/DanMarshall909/mem-map).
Validation details and known limits are in [docs/validation.md](docs/validation.md).
Top-five validation is recorded in [docs/top-five-validation.md](docs/top-five-validation.md).
Parent-name validation is recorded in [docs/parent-name-validation.md](docs/parent-name-validation.md).
Navigation/font/zoom validation is in [docs/navigation-zoom-validation.md](docs/navigation-zoom-validation.md).
Local process captures, screenshots, compiled executables and build caches are
excluded from the repository. Build `MemMap.exe` using the commands above.
