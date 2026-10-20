# MM-014 overview controls validation

Candidate continues 5f72182 in the existing Windows task worktree
D:\worktrees\personal\mem-map\system-memory-redesign. No merge/cleanup.

## Behavior evidence

- AC1: Rust projection retains a valid baseline outside the visible plot. The
  independent fixture uses at2=100, at300=100, at303=125 and at601=150 bytes;
  with a 300-second range only303/601 remain plotted, while baselines2/300
  yield +25%/+50%. Four-second freshness boundary, invalid attribution fallback,
  startup underflow, interval changes and frozen paused context are asserted.
  Frontend uses those exact decimal baseline bytes; unknown/zero/New conventions
  and sample-relative pointer/keyboard/latest behavior remain covered.
- AC2: Range radios issue existing actions, retain selected state on live updates
  and support native keyboard arrows in the real browser.
- AC3: Modal Close belongs to a fixed header. Only the body scrolls. Real headless
  420x300 journey checks close hit testing/viewport bounds, overflowing content,
  click close, Escape close, opener focus restoration and retained main view.
- AC4: Rust accepts50 through startup/actions/persistence, rejects49/201, and
  preserves valid preferences. DOM tests assert option/action, keyboard/wheel
  lower bound and7px base font; real browser checks50 and its saved preference.
- AC5: DOM/headless checks omit title/counts/sample buttons, preserve status for
  pause/errors and keyboard chart inspection, and compare copy/back icon color.

Genuine RED evidence is retained outside the repository under
LOCALAPPDATA/mem-map-build/evidence/overview-controls. The first combined chrome
RED exposed costly element-diff rendering; the repaired boolean-presence oracle
fails precisely for the visible title. Other REDs reject absent range toggles,
old modal structure, old75% clamp, and missing out-of-window baseline context.

Refactor: production reuses native radio controls and separates scrollable modal
content from its heading, removes unused arrow glyphs, and keeps
baseline selection in the Rust projection. Tests reuse fixtures and domain
commands; serialized baseline assertions avoid coupling to private storage.
Existing file line endings were preserved to keep the diff narrow.

## Final gates

scripts/check.ps1 all: PASS,40 Rust (37 library +1 desktop +2 live sampler),
63 frontend and23 real-host headless checks. Frontend coverage:63 passing;97.54% lines,
85.41% branches,83.94% functions over six modules in the final full run.
Native scripts/build.ps1 and tested-asset correspondence: PASS (retained release log and artifact hashes).
Independent completion review: PASS (exact candidate and finite envelope; retained overview-controls-review-r1 verdict).

All automated browser tests remain headless; no OS clipboard/native-window test,
repeated-scroll test, external write, deployment or runtime clipboard claim.
Mutation remains pending: no safe CoW writable layer/calibrated lane attribution
is available; no mutation score is claimed. Fast/slow timing is reviewed without
reclassifying single noisy runs. Existing source files and reusable build caches
are retained. Global-skills preflight drift previously surfaced remains untouched.

Local lesson: visible chart data is not sufficient comparison context. The owning
wire contract and retained-history regression now carry the invariant; no new
standing rule or memory update is needed.
