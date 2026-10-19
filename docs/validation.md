# Validation — MM-001

Host: Windows, 2026-10-19. Rust 1.99.1,
x86_64-pc-windows-gnu; windows-sys 0.61.2, windows-link 0.2.1.
The directory began empty, without Git or a remote. After local validation, the
user requested a new public repository at https://github.com/DanMarshall909/mem-map.
Initial publication uses `main` and an empty `origin`: there is no existing base
commit or upstream object to integrate. Source review and local runtime evidence
are separate from Git publication and hosted CI.

Independent review: PASS; see artifacts/review-v3/verdict.md for the reviewed manifest and evidence.

Paths under `artifacts/` refer to private local evidence retained by the developer.
They are excluded from the public repository because captures and logs can contain
local process paths. The source, tests and reproduction commands are published.

| Criterion | Evidence |
| --- | --- |
| AC1 Physical pie | Exact private-RAM/remainder/available partition test; sampling-race and zero-total tests. Only private working sets enter app slices. Inconsistent slices are capped. |
| AC2 Views, search, sort, identity | Grouping/PID-reuse/search/inactive tests; independent multi-row numeric RAM vs growth ordering test; live allocation test checks executable identity and creation time. |
| AC3 Bounded history | Over 2,000 observations and over 1,024 process instances; exact retained endpoints, growth, inactive eviction and expiry. Gap baseline reset and out-of-order rejection. Exact RAM/commit graph expectations. |
| AC4 Pause/export | Hidden window/controller smoke asserts group, sort, inactive, metric, range and selection outcomes. Four actual seconds paused must leave observations unchanged; resume must add an observation and >3s gap. Exact CSV bytes/quoting/units/identity/blank-gap test; Import-Csv reads the actual output. |
| AC5 Footprint | Standalone release under 0.5 MiB, Windows DLL imports only. Real self-counters, full-history stress and 100 redraws with stable GDI counts in artifacts/peak-growth/smoke.txt. |
| AC6 Dense layout | Main/group/process renders in artifacts/peak-growth/. The table occupies roughly two thirds of the width and nearly the whole height: 22 rows at reference size, compared with five in the first layout. |
| AC7 Clickable pie | Smoke computes an interior wedge point, asserts its target, invokes drill-down, and asserts exact member filtering and selected details. Zero/capped app slots have a regression preventing shifted click identities. |
| AC8 Related grouping | Name test combines node.exe across install paths. Family tests cover nesting, parent exit, younger reused parent PID and Windows launcher boundaries. Unicode executable names retain graph membership. |
| AC9 Peak RAM | Alternating per-process peaks assert the highest simultaneous group total, not summed maxima; exited instances remain in peak history. Table and details show sampled private-RAM peak from retained history. |
| AC10 Configurable growth | Exact lookback assertions cover individual/group endpoints, exits, non-aligned windows and gaps. Parser covers presets, custom units, bounds and overflow. Smoke checks custom input, invalid-input preservation and graph-window independence. |

Fifteen fast model tests run in about 0.02 seconds, excluding compilation. One slow
Windows contract test runs in about 0.4 seconds: touch 64 MiB, observe independent
RAM and commit changes, then repeat five scans and check process handle release.
Both lanes, format, Clippy with warnings denied, locked release build and hidden
smoke pass. Exact latest output is retained under artifacts/review-v2/ and
artifacts/peak-growth/. No lane was moved on one timing observation.

Genuine original missing-feature RED command records were extracted byte-for-byte
from this session into artifacts/review/red-records.jsonl; red-provenance.json
records source lines and extraction hash. The first reviewer independently
verified those records. Additional new-group and pie-slot RED output is in
artifacts/review/compact-model-red.log and artifacts/review-v2/unicode-red.log.
Build setup failures are separate and are not RED evidence.

The first review found two acceptance-oracle gaps: pause/view actions were invoked
without assertions, and sort tests did not distinguish RAM order from growth
order. The new smoke predicate retains failed action assertions and checks a pause
across two sampling intervals; the model now asserts both numeric sort orders.
User feedback expanded the scope to dense layout, clickable pie and related/name
grouping, then peak RAM and configurable growth. The full changed candidate receives independent review; an older verdict does not approve new bytes.

The current compact report includes baseline working set and a separate history
capacity stress: repeat one real counter snapshot into 901 slots, keeping those
synthetic timestamps outside displayed/exported real observations. These are PSAPI
process counters, not total GPU/kernel/GDI accounting or a promise for every
machine, DPI, process count or history length. The earlier build observed about
780 processes and measured 21–22 MiB working set with its diagnostic history full;
the latest compact run has its own recorded process count and measurements. CPU
before and including stress is reported separately. GDI counts are sampled after
five warmup redraws and must stay unchanged across the next 100.

Private resident RAM and private commit are independent metrics. Unqueryable or
exited-during-scan processes count as unavailable; their RAM remains in the pie's
system/shared/unmeasured remainder. App totals include measured instances only;
changes in availability can change those totals. Family ancestry is inferred from
Toolhelp parent PID and creation-time order, not a software-publisher classifier.

Source coverage was attempted with -C instrument-coverage and failed because this
Windows GNU compiler lacks profiler_builtins. artifacts/coverage/fast.log retains
the error. No numeric coverage claim is made. Mutation remains unrun: no verified
source/lane coverage map or copy-on-write mutation harness was established on the
ReFS workspace. Passing behavior tests are not coverage or mutation scores.

Rendering is inspected using the app's GDI BMP/PNG output. The smoke exercises the
controller and hit-test routines used by native handlers, rather than external
mouse/input automation. The native save dialog and full assistive-technology
behavior have not received interactive acceptance testing. Custom charts/buttons
have keyboard shortcuts but incomplete UI Automation exposure. History is session
only; closing a window clears that capture. No hosted, deployment, stakeholder or
business-acceptance claim is made.

The reviewed executable is 442,368 bytes, SHA256
`76a152dda2f86fed29d2e4d8640429df13da8cff998f1b3643fb6a0421ba3d97`.
It was opened locally with the title “Mem Map — Peak RAM and growth”.
Existing preview windows and captures were preserved; no process was stopped.
Rust occasionally emitted incremental cache-finalization Access-is-denied notes;
completed checks exited successfully. Reusable toolchain/build caches and private
ignored artifacts are retained. No unrelated checkout or resource was cleaned.
Reproduction commands and the fast/slow driver are in README.

AC9/AC10 genuine RED logs: artifacts/review-v2/peak-red.log, growth-window-red.log and growth-input-red.log. Full-capacity history is also temporarily swapped into the hidden renderer for 105 redraws, restored before returning, and never exported or shown in a visible window. The final smoke records elapsed redraw time and stable GDI counts.
