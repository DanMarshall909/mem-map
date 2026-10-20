# MM-013: Copy snapshots, static paths and grouping icons

status: Resolved

Continue the same Tauri UI follow-up on `feat/system-memory-redesign` in
`D:\worktrees\personal\mem-map\system-memory-redesign`, extending `c065ad7`.
The user requests clipboard copying instead of export, a non-editable executable
path and grouping icons. Keep the Rust collection/model, snapshot semantics,
existing path-copy action and all automated tests headless. No merge, cleanup,
deployment or new arbitrary browser clipboard/file payload capability.

- AC1: The header offers Copy snapshot to clipboard instead of Export CSV. It
  copies the existing CSV serialization of the displayed, filtered snapshot,
  including rows outside the viewport. Paused data stays frozen. No save dialog
  or file destination appears. Copy success/failure uses the existing notice.
- AC2: Executable path is static selectable text, with full path available to
  assistive technology and the path-copy action. Long directories yield to the
  executable filename across supported sizes; no editable/read-only textbox.
- AC3: All five grouping options use distinct icons, accessible radio names,
  descriptive tooltips, selected/focus states and existing keyboard/commands.

Plan: owning DOM/transport RED-GREEN, existing real-controller headless copy and
layout journey updates, Rust/frontend checks, native rebuild, frozen independent
review and exact specification/task finalization before normal feature push.
Clipboard payload stays Rust-owned; only the desktop clipboard outcome changes.
Private headless adapters never write the real OS clipboard.

Independent completion review: PASS (exact candidate and finite envelope; retained controls-review-r1 verdict).
