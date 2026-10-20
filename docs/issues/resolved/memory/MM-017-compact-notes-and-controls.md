# MM-017: Simplify notes and match history controls

status: Resolved

Cosmetic continuation from9eb8737. Hide routine table footnotes from the visible
layout (retain accessible context), remove the routine recording footer note,
and move the unchanged Details info action directly beside Pause. Keep explicit
paused/error feedback and action notices. Process-history range uses the same
compact5m/15m/30m/All radio toggles as overall memory, retaining its group name,
commands, keyboard behavior and selected state. Axis controls retain their layout.

Frontend typecheck/build and63 existing frontend tests passed;23 existing headless
checks passed in headless-20261020-140334-577, including modal close/focus and
responsive layout. Live browser verifies Pause/Details/Copy order, no visible
footnotes, compact range and fitted inspector; screenshot retained at
LOCALAPPDATA/mem-map-build/evidence/compact-notes.png. No new tests or TDD cycle
under the global cosmetic exception. Native executable was not rebuilt; browser
preview uses the live frontend and existing Rust backend.
