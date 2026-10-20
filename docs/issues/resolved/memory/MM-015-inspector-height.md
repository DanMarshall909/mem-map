# MM-015: Give the inspector available height

status: Resolved

Cosmetic continuation of the Tauri UI work from d65715f. Size the sidebar
overview to its content and give the inspector the remaining workspace height;
retain overflow scrolling when content genuinely cannot fit. Expanded overview
and narrow-window reflow retain their existing rules.

Validation: live browser at2082x1399 measured inspector clientHeight1015 and
scrollHeight1015 (no vertical overflow), overview260px. Screenshot retained at
LOCALAPPDATA/mem-map-build/evidence/inspector-fit.png. Frontend typecheck/build
passed; existing23 headless checks passed in headless-20261020-135756-228.
No new tests or RED cycle: the user's global cosmetic-UI exception applies.
The native executable was not rebuilt for this CSS-only browser-preview change.
