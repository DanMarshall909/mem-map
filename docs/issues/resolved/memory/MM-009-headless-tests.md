# MM-009: Run every automated test headlessly

status: Resolved

## Authority and scope

The user requests "make all tests headless" (2026-10-20). This continues the
Tauri migration in `feat/system-memory-redesign`, based on published `2fcdcb3`,
in `D:\worktrees\personal\mem-map\system-memory-redesign`. Keep the existing
live browser preview. No merge, deployment, or unrelated worktree cleanup.

## Acceptance criteria

- AC1: Every maintained automated entrypoint runs without showing an application,
  browser, or dialog, moving OS focus, or modifying the OS clipboard.
- AC2: The slow browser suite still exercises the production frontend with the
  real Rust controller and Windows sampler: pause/recording, copy/export data,
  filtering/grouping, keyboard/pointer interactions, responsive layout and
  accessibility semantics. The previously removed scroll test stays removed.
- AC3: Test processes and servers are isolated, close on success and failure,
  and leave the live editing preview untouched. Missing prerequisites fail clearly.
- AC4: Normal fast/slow/all drivers select the complete suite. Documentation
  distinguishes headless browser/controller evidence from native-shell evidence.

## Plan and evidence

- Rust library/origin tests, Windows live sampler tests and HappyDOM tests already
  run headlessly. Replace the visible native smoke lane rather than hiding a
  Tauri window. Native clipboard/save-dialog/UIA checks become historical evidence.
- Use a test-only Rust stdio host with the unchanged application/sampler and an
  isolated preference path. Launch headless Playwright Edge against built assets
  on an ephemeral loopback port, with private copy/export adapters.
- Retain the existing feature worktree as the same Tauri follow-up; its clean
  branch was fetched and fast-forward checked. The primary checkout remains main.
- Local verification: `check.ps1 all` passes all 37 Rust tests, 24 fast frontend
  tests and 16 headless browser checks. Format, all-target Clippy, TypeScript/Vite
  build and script syntax checks pass. See `docs/headless-validation.md` for
  commands, artifacts, boundary limitations and the headless-mode regression probe.
- Failure-path verification: a nonexistent browser fails clearly and closes the
  owned Rust host normally and its ephemeral server. A failed browser journey
  also closed both without forced cleanup. The live preview remains untouched.
- Independent completion review: PASS for the frozen candidate and exact finalization envelope. Normal feature-branch publication follows; no merge or deployment is included.
