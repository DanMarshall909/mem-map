# MM-016: Grow the overview into spare vertical space

status: Resolved

Cosmetic follow-up from a989e95: let the overview graph fill the height left by
content-sized process details. Keep a usable overview minimum, inspector overflow
for genuinely short windows, existing narrow reflow and expanded-overview sizing.

The right pane now gives its flexible row to the overview and sizes the inspector
to content. The SVG uses its panel height instead of a capped sidebar height.
At2082x1399 the overview is530px with a467px plot; inspector content/clientHeight
are both745px. Live screenshot: LOCALAPPDATA/mem-map-build/evidence/overview-height.png.

Frontend typecheck/build passed. Existing63 frontend tests and23 headless checks
passed (headless-20261020-140056-489). Two existing layout expectations were updated
for panel-owned sizing; no new tests or TDD cycle were added under the global
cosmetic exception. Native executable was not rebuilt; the browser preview uses
live frontend sources with the existing Rust backend.
