# MM-020: Compact table toolbar icons

status: Resolved

The user requested that Show exited processes, Compact rows and Change over move
onto the Group by row and use icons. The filter remains underneath. The first
row wraps at small widths; the selected comparison duration remains visible.

Cosmetic-only presentation change: native checkbox/select controls, accessible
names, option values, commands, state and keyboard behavior are retained. The
two checkbox icons keep checked/focus styling, tooltips and forced-color states.
No backend, calculation, preference or domain command changed. The global
cosmetic UI exception applies; no failing-first or new CSS/markup tests required.

Verification: frontend TypeScript/Vite build PASS; 70 existing frontend tests
PASS; 25 existing real-host headless checks PASS. Inspected the rendered toolbar
in the live wide preview and at 760x540, plus the existing smaller/zoom journeys.
Manually toggled compact rows off/on through the new icon and restored its state.
Evidence: LOCALAPPDATA/mem-map-build/evidence/toolbar-icons/live-preview.png and
headless-20261020-144053-314. Native backend was unchanged; the live preview uses
the updated frontend. No fresh native executable/release claim for this cosmetic
slice. All automated checks remain headless. No merge or worktree cleanup.
