# MM-005: Mem Map app icon

status: Resolved

User scope: make an icon for Mem Map. Provide an editable vector master and PNG/
Windows ICO exports, and embed it in the native app. Use the existing teal palette
and a simple memory-chip/usage-chart motif that remains clear at small sizes.

- AC1: Transparent icon assets include sizes from 16 through 256 pixels, inspected
  at native small size and as a large preview. The SVG remains editable.
- AC2: Built Windows executables contain the icon; both small and large window
  icons use the embedded resource. Normal app builds do not need an image renderer.
- AC3: Preserve bounded runtime overhead and existing process/history behavior.
  Verify the actual resource and native window icon handles plus the existing smoke.

Windows worktree: D:\worktrees\personal\mem-map\app-icon, feat/app-icon, starting
from freshly fetched main at 0ac30df9595b990deb345e8c7e9697b4951efaf3. Prior preview,
worktree, captures and warm caches are retained. Publish a separate feature branch;
merging this new change is separate from the earlier PR #1 authorization.

## Resolution

AC1-AC3 pass local engineering validation in docs/icon-validation.md. SVG, PNG
and multi-size ICO are checked in; the executable and native window class use the
icon. Native resource extraction, icon-handle RED/GREEN, 22 tests, smoke, formatting,
Clippy and release pass. Independent completion review: PASS, retained privately
in artifacts/icon/review/verdict.md. Existing hosted/manual/coverage gaps remain.
This publishes a separate feature branch and PR; merging it remains separate.