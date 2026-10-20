# MM-008: Replace the entire UI with Tauri

status: Resolved

## Authority and scope

The user reports scrolling corrupts the native inspector and explicitly requests
replacement of the entire UI with Tauri while keeping the backend (2026-10-20).
Continue `feat/system-memory-redesign` in the existing Windows worktree
`D:\worktrees\personal\mem-map\system-memory-redesign`, based on published
`b2adc8ba05c4446054d26ce91b9faf2cfb05a6ca`. No merge or deployment is requested.
The [system-memory specification](../../../system-memory-spec.md) retains product
authority; the user's latest instruction supersedes its native UI mechanism.
The [interface contract](../../../tauri-contract.md) records implementation decisions.

## Acceptance criteria

- AC1: All dashboard regions and interaction controls run in Tauri with no legacy
  GDI production interface; the existing Rust backend remains authoritative.
- AC2: Browser layout preserves readable controls, chart and selection, with
  fitting responsive panels. The user explicitly removed the repeated-scroll
  regression test on 2026-10-20 because the reported corruption was a native
  Windows UI issue. That test is excluded from the maintained suite and gate.
- AC3: Group/filter/tree/sort/hold, frozen pause with continued recording, history
  metric/range/pan/zoom/follow, unknown values and identity precision retain the
  original dashboard semantics across the new IPC boundary.
- AC4: Full-path copy, displayed filtered CSV, presentation-only preferences,
  keyboard and accessibility work through the actual desktop boundary.
- AC5: Appropriate fast/backend and real WebView2 checks pass; the complete
  candidate receives independent review before ordinary feature-branch push.

## Plan and ownership

Root owns build configuration, Tauri shell, integration, real renderer checks,
tracker and Git. Bounded backend and frontend writers own nonoverlapping files
under the shared contract. Build/test/runtime slots are serialized by root.
UI-neutral extraction and IPC/browser rendering are novel boundaries (tracers);
existing domain tests/calculations remain the reference for retained behavior.

## Evidence and decisions

- Discovery: dashboard/model/sampler/preferences are already reusable. Replace
  mixed GDI/native-control rendering with plain TypeScript, CSS and SVG.
- Windows has WebView2 155.0.4283.62. Existing Visual Studio lacks the C++ tools;
  first validate the existing isolated Rust GNU/LLVM toolchain with Tauri.
- Build-spike bound: prove local Tauri/WebView2 launch using existing GNU/LLVM,
  one compiler slot, dependencies/outputs on C. If unsupported, revise compiler
  setup before product implementation; no cache deletion or security changes.
- Shared-rules sync preflight reports checkout/upstream drift; global rules are
  unchanged. No relevant memory entries were found.
- The complete GDI/control interface is replaced by Tauri. The sampler,
  dashboard calculations and preference storage remain Rust-owned; obsolete
  native-only viewport/zoom helpers are removed from the model.
- Final local verification: 37 Rust tests, 24 frontend tests, format/Clippy,
  optimized build and 13 real Tauri/WebView2 checks pass. The actual release
  proves native copy/CSV/UIA, pause/recording, group/filter and responsive
  resize/zoom. The user-removed scrolling test is absent from the maintained
  desktop runner. See [validation](../../../tauri-validation.md) for exact
  binary/assets, evidence locations and coverage/mutation limitations.
- Independent whole-candidate completion review: PASS for the frozen candidate
  and exact finalization envelope. Ordinary feature-branch publication follows
  that envelope. No merge or deployment is included.

- R1 independent review found one remaining visible-path defect. Protected
  directory/filename presentation and document-level global shortcuts are now
  repaired. All 24 frontend and 13 actual desktop checks pass against the fresh
  release, with genuine prior-source/native RED evidence. The R2 packet includes
  complete source/evidence and a bounded finalization envelope. The requested
  live browser preview is open and intentionally retained.
