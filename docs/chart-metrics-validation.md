# MM-018 chart metric and axis validation

Continuation of487b1cc in the existing Windows system-memory-redesign worktree.

AC1: Rust tests independently assert distinct Other/A/B/C commit values31/19/23/29
and baselines17/7/11/13, filter independence,50GiB commit above32GiB physical RAM,
physical header unchanged, partial measured totals/excluded unknown allocations,
incomplete attribution and frozen paused data. No system commit limit is sampled
or claimed. Both metrics use matching retained baselines. Physical header now has
its own snapshot projection. Existing sampler/model retention remains authoritative.

AC2: Rust checks independent axis/metric settings, constructor/preference recovery
and rejected invalid overview metric without partial state. Frontend asserts real
positive fitted lower bounds, zero restoration, plot clipping and cursor placement.
Fitted physical bounds stay within RAM capacity; commit scales to data. New saved
metric values load only from a bounded enum; legacy preferences keep defaults.

AC3: Both charts reuse icon-capable native radio groups with distinct RAM/page and
zero/fit SVGs, option names, titles, checked state and focus behavior. Headless
journey clicks every metric/axis control, checks independence and saved preferences,
uses native keyboard arrows, verifies physical header/commit labels and restores
normal settings. No native-window or OS clipboard automation is used.

Genuine RED logs are external under LOCALAPPDATA/mem-map-build/evidence/chart-metrics.
They reject the missing overview metric/action, missing fitted bounds and missing
icon controls. The initial cursor test targeted a button SVG and later omitted
expansion; those harness/setup failures were corrected and are not behavioral RED
claims. A real fit-bound failure is retained separately. An intermittent real-browser failure exposed a poll arriving while an action was pending. A deterministic transport RED reproduced the old render; pending-action suppression fixes it without weakening the headless radio checks. Equal-revision health refresh and queue recovery remain tested. Both production and tests
were refactored: reuse the physical overview layer builder, independent physical
header, retained aggregate commit, native icon radios and shared clipped plot area.
Tests derive expected values from distinguishable inputs rather than another
production output. Partial commit can retain known bands when named attribution
is complete; otherwise the measured total remains neutral with explicit coverage.

Final scripts/check.ps1 all: PASS (41 library +1 desktop-origin +2 live sampler Rust tests,67 frontend tests,24 real-host headless checks).
Frontend coverage:67 passed;97.65% lines,85.81% branches,84.50% functions over six modules.
Native scripts/build.ps1 and tested-asset correspondence: PASS (retained release log and artifact hashes).
Independent completion review: PASS (exact candidate and finite envelope; retained chart-metrics-review-r1 verdict).

All automated tests remain headless. Rust formal source coverage is not configured;
frontend coverage reports execution of six modules separately. No mutation score
is claimed: safe CoW storage and calibrated fast/slow source attribution remain
unavailable. No lane moves follow single noisy runs. Keep reusable builds/caches,
worktrees and evidence. Global skills drift previously surfaced is preserved.

Local lesson: chart metric and physical summary are distinct owners; changing the
plotted counter must not relabel physical RAM or turn measured commit into a
system-total claim. Owning projection tests and wire documentation carry this;
no new standing rule or memory update is needed.
