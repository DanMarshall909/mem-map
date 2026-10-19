# MM-004: Parent navigation, monospaced display and zoom

status: Resolved

User scope: push changes, make the parent clickable to navigate to that process,
use a monospaced font, and allow zoom. Continue the same app feature branch/PR #1.
The user confirmed both chart time range and interface scaling. Preserve existing
captures and history bounds; no merge.

- AC1: Clicking an available parent in a process row or details selects its exact
  captured instance. Reveal it despite search, grouping, scope or inactive filters.
  Retained exited parents remain inspectable. Missing/expired/unobserved parents
  are not links to a different process reusing the PID.
- AC2: Use a monospaced face for the native display and edit controls; verify actual
  equal-width glyph measurements and readable aligned numeric columns.
- AC3: Allow bounded chart time zoom without changing stored history, growth window
  or pinned comparison identities. Provide discoverable controls and keyboard access.
- AC4: Allow bounded interface scaling while keeping content, controls and hit
  targets aligned at normal and high DPI. Provide reset and access to overflow.

Validation: behavior-first parent identity/navigation and zoom tests, native smoke
with equal-glyph and rendered-control evidence, inspected images, fast/Windows lanes,
format, Clippy, release and independent completion review. Existing instrumentation
and external input/hosted limitations remain disclosed.

Workspace: Windows D:\worktrees\personal\mem-map\top-five-overlay,
feat/top-five-overlay at 75a4d43273917d5b1eaf7f91f03429c155b88a76, tracking the same
origin feature branch. Primary main and earlier previews remain preserved.

Local evidence: docs/navigation-zoom-validation.md. Engineering checks and independent completion review PASS; private verdict is retained
in artifacts/mm004/review/verdict.md.

## Resolution

AC1-AC4 have local engineering validation in docs/navigation-zoom-validation.md.
Parent links target captured process instances, reveal them through filters and
preserve scrolling after the native search field clears. Consolas and both chart
and interface zoom are implemented. Twenty fast tests, two Windows tests, native
message/render smoke, formatting, Clippy, release and independent review pass.
Existing coverage, mutation, external-input, real monitor-transition and hosted
acceptance gaps remain. This publishes to the same feature branch and PR #1;
integration into main is separate. Existing previews and captures are preserved.