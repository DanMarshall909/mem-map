# Headless test validation (2026-10-20)

Local verification and independent completion review: PASS.
Owning task: MM-009. Source base: `2fcdcb39937081e48d451076b23f6b528ea720e6`.
Windows worktree: `D:\worktrees\personal\mem-map\system-memory-redesign`.

## Maintained test lanes

| Driver | Evidence |
| --- | --- |
| `scripts/check.ps1 fast` | 34 Rust library, 1 origin-policy and 24 HappyDOM tests; no UI |
| `scripts/check.ps1 slow` | 2 live Windows sampler tests and 16 headless browser checks |
| `scripts/check.ps1 all` | Both lanes; final combined run passed |
| `scripts/smoke.ps1` | Builds test-only Rust host and frontend; runs headless browser checks |
| Frontend `npm run test:fast`, `test:slow`, `test:all` | Explicit fast, slow and combined frontend lanes |

Every browser launch explicitly uses `headless: true`; a runtime CDP gate checks
the engine's headless user-agent marker. No headed fallback or browser download
occurs. The default installed Edge reported `Edg/155.0.4283.62`. Configured browser
channels/executables must be compatible with the pinned Playwright version.

The browser consumes built production assets on an owned ephemeral loopback port.
A test-only NDJSON Rust example uses the unchanged `Application`, sampler and
presentation preference implementation. Its sampling is demand-driven at a
two-second cadence while the browser polls, including when the display is paused.
It does not exercise production's collector thread or Tauri runtime. No production
Rust/frontend source, capability, CSP, or command authorization policy changes.

## Results and reproducibility

Evidence root: `%LOCALAPPDATA%\mem-map-build\evidence\headless-tests`.

- `all-headless.log`: `scripts/check.ps1 all`, exit 0. Rust library 0.64s,
  origin 0.00s, Windows live 0.76s and frontend 1.98s, excluding compilation.
- Final browser artifacts:
  `%LOCALAPPDATA%\mem-map-build\evidence\headless-20261020-104700-022`.
  `report.json` records all 16 checks, host SHA256 before/after, built asset
  SHA256 values, command, host PID, ephemeral URL and cleanup. PASS in 30.60s;
  browser/server closed, Rust host exit 0, no forced cleanup. A prior headless
  run (`headless-tests/run-2`) passed 15 checks in 17.87s before adding the
  isolated-preference assertion and asset metadata. This is a boundary/slow lane;
  no test moved lanes based on these differently loaded runs.
- `clippy.log`: all targets with `--locked --offline -- -D warnings`, exit 0.
  Format, Node/PowerShell syntax and full whitespace checks recognizing Windows
  CRLF also pass. TypeScript/Vite build is part of each smoke invocation.
- `headless-red.log`: the same runtime gate rejects the already-running native
  WebView2 carrier without launching or changing a visible window.
  `headless-green.log`: the gate accepts a real explicitly headless Edge launch.
  Initial import setup and branded-product-label probe failures are preserved
  separately as diagnostics; neither is counted as behavior RED.
- `run-1/report.json`: a grouping test's immediate checkbox confirmation raced
  the asynchronous controller response. The test now clicks and waits for both
  Rust grouping and checked DOM state. This failed run still records normal
  host exit and browser/server cleanup. It is not a passing acceptance run.
- `negative-path-proof.json` and `missing-browser/report.json`: a nonexistent
  executable is rejected with the expected missing-browser diagnostic, the host
  exits 0, the server closes, and no headed fallback occurs.

Browser journeys retain live sampling, frozen pause with continued recording,
full path/copy data, frozen CSV timestamp and distinct process identities, filter
and all five groupings, four viewports (1280x800, 980x740, 760x540, 1440x900),
125% pointer column resizing, 200% reflow/path visibility, global zoom shortcuts,
Chromium selected/expanded treegrid semantics, invalid-command rejection,
isolated presentation preferences and absence of renderer exceptions.
The removed repeated-scroll test remains absent.

## Evidence limits and test design

Private copy/export adapters consume the real controller's original paths and
CSV bytes. An independent CSV consumer verifies exported records. These checks
never alter the OS clipboard or open save dialogs. Chromium accessibility is
checked instead of Windows UIA. Native Tauri IPC, Windows compositor/DPI, save
dialog, clipboard and UIA outcomes remain historical evidence in
`tauri-validation.md`; this suite makes no fresh native-shell claim.

Coverage review maps the retained assertions to real controller/sampler calls,
the production browser bundle, strict command rejection and failure cleanup.
No new numeric production coverage score is claimed. Existing core production
code and fast tests are unchanged. Rust numeric instrumentation remains unavailable
on the configured GNU toolchain. No mutation score is claimed: E2E is excluded
from mutation, and the previously documented safe-CoW/lane-attribution limitation
remains pending. This test infrastructure change does not alter that limitation.

Production refactoring: none needed; production behavior is unchanged.
Test refactoring: central headless launch/runtime gate, private effect adapters,
owned lifecycle cleanup and asynchronous state acknowledgement replace the old
focus-dependent native runner. The former native helper retains only the
already-running manual preview's exact-PID Close utility; no test calls it.
The preview at `http://127.0.0.1:1420/` is intentionally retained.
