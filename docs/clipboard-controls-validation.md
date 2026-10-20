# Clipboard and compact controls validation (MM-013)

Windows worktree `D:\worktrees\personal\mem-map\system-memory-redesign`, branch
`feat/system-memory-redesign`, extending `c065ad7863c663cff35f1232a1f2bf55ff655bdb`.
Evidence: `C:\Users\DanMarshall\AppData\Local\mem-map-build\evidence\clipboard-controls`.

The header copies the existing CSV snapshot to the clipboard instead of opening
export. Executable paths are selectable static text. Grouping uses named icon
radios with tooltips and the existing commands/keyboard behavior. Rust collection,
model, history and CSV serialization remain unchanged. The native outcome changes
from save dialog/file write to the existing private Windows clipboard helper.

## Verification

- `scripts/check.ps1 all`: exit 0 (`all.log`, `all-exit.json`). Fresh Rust results:
  34 library tests, 1 bundled-origin desktop test, 2 live Windows sampler tests.
  All 59 frontend tests pass; the driver run took 7.077 seconds for that lane.
- Full frontend coverage: 59 pass in 7.871 seconds (`frontend-coverage.log`),
  97.54% lines, 85.71% branches, 81.73% functions across six loaded production
  modules. DOM helpers and transport have 100% executed-line coverage; native
  entry points/FFI are outside this instrumented scope. Command:
  `node --test --experimental-strip-types --experimental-test-coverage
  --test-coverage-include=**/src/*.ts frontend/tests/*.test.ts` from the worktree.
- Real-host browser: 22 headless checks pass in 39.429 seconds. The driver's
  retained report is `../headless-20261020-131849-178/report.json`; it records
  tested asset hashes, commands, geometry and normal host exit/browser/server
  cleanup without forced termination. Exact rebuilt test-host hash:
  `893d131bb5c11fb8529fc6ebd99637d429fbc986b5ca221b1a25a3b57c47ae79`.
- `scripts/build.ps1` rebuilds the optimized native executable and frontend;
  `release.log`, `release-exit.json` and `release-assets.json` retain its final
  result and tested-asset correspondence. A successful build is required before
  independent PASS/publication. Rust formatting, TypeScript, Node syntax and
  Git whitespace checks pass.

## Criteria and test effectiveness

| Criterion | Evidence |
|---|---|
| AC1 snapshot copy | `copy-red.log` fails for the missing header action and missing ordered bridge call; `copy-green.log` passes. Queue tests protect ordering after pause and clipboard-error reporting. Existing Rust CSV cases protect filtered/coherent membership. The actual-host browser parses the copied payload independently, asserting paused timestamp, unique process identities, raw fields, success notice and rejection of the retired export command/untrusted arguments in the private host. |
| AC2 static path | `path-red.log` rejects a textbox. `path-green-coverage.log` and final suite pass static/selectable text, full original accessible description and existing path copy. The real browser verifies the original beginning, protected filename and stable focus layout across window sizes and zoom. |
| AC3 grouping | `group-red.log` rejects missing accessible icon names; the first green attempt exposed a test incorrectly expecting a change event when clicking the already selected radio. The test now cycles through distinct selections, preserving native semantics; final 59-test runs pass distinct icons, names, tooltips and commands. Real-browser clicks cover all five modes and native ArrowRight changes Name to Path while retaining one selected radio. |

Refactoring reuses one SVG creator for buttons and grouping icons. History/axis
radios retain text. The obsolete path input overlay and save-dialog implementation
are removed. The existing private Rust clipboard helper and CSV serializer are
reused; the WebView never supplies arbitrary clipboard text or destination paths.
The exact bundled-origin guard and its test remain in place. The granted command
and handler replace export with snapshot copy. Test effects stay private and do
not touch the OS clipboard. No repeated-scroll or native-window test is added.

Actual OS clipboard ownership/readback is not claimed by headless adapters.
Native compilation and boundary review are separate evidence; no native save
dialog remains. Preview reload is operational verification, not a production
clipboard acceptance test. Generated permission/schema output is build-owned and
ignored; source command declarations and capability grants are reviewed.

Ordinary frontend runs here took about 7-8 seconds versus the earlier roughly
3-second records; no test failed and no lane reclassification is inferred from
this host/session timing. Owning cases run at the cheap DOM/transport layer.
Mutation remains pending: safe CoW storage and calibrated fast/slow attribution
are unavailable. No score is claimed; E2E is excluded. Worktrees, dependencies,
warm build outputs and evidence are retained.

Independent completion review: PASS (exact candidate and finite envelope; retained controls-review-r1 verdict).
