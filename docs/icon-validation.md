# App icon validation - MM-005

Windows, 2026-10-19, Rust 1.99.1 x86_64-pc-windows-gnu, starting from main
0ac30df9595b990deb345e8c7e9697b4951efaf3. Independent completion review: PASS; private verdict retained in artifacts/icon/review/verdict.md.

The editable SVG depicts a teal memory chip with a three-part usage chart. PNG
and ICO exports retain transparency. The ICO includes 16, 20, 24, 32, 40, 48, 64,
128 and 256px PNG frames. Windows WPF exports the path artwork offline; the icon
and PNG are checked in, so ordinary app builds do not invoke WPF or rasterization.
Repeated exports on this machine produce identical file hashes. Unsupported SVG
elements or path attributes are rejected instead of silently omitted.

The build-only embed-resource 3.0.12 dependency compiles the resource for the
mem-map executable. Native small and large icons load separately at system DPI;
their owning window-class guard unregisters the class before releasing them.
Partial creation failures release owned handles. If unregistering a still-live
class fails, its icons stay alive for process teardown rather than leaving the
registered class with invalid handles. No sampler or history layout changed.

AC1: Inspected the master preview and actual small/large icons extracted from
the executable by Windows Shell. A private resource probe decodes every ICO frame,
checks the declared dimensions and verifies transparent corners/opaque artwork.
Shell extraction succeeds for both sizes (28 and 56px on this display).

AC2: Retained genuine native RED fails on embedded small and large window icons
before resource/class integration. GREEN checks both non-null class icon handles,
in addition to the real PE extraction. The resource is embedded; no external icon
file needs to accompany the executable. GNU resource compilation is verified;
the dependency's MSVC support is not independently exercised here.

AC3: Twenty fast tests and two real Windows tests pass, along with formatting,
locked denied-warning Clippy, locked release and the full existing native smoke.
The full-history diagnostic run keeps GDI counts at 28 to 28 across 105 redraws.
Observed working set was 14,954,496 bytes and 19,894,272 with full diagnostic history;
private RAM was 2,580,480 and 7,512,064 respectively. These are workload measurements,
not exact icon allocation accounting or a long-duration soak. Icon handles are
loaded once per application, and redraws reuse them.

Release size: 489,984 bytes. SHA256:
`953bf7b613c64145bc5523006f5ae3143786f638acc2ddc62d2a702c8508de02`.
The ICO is 14,752 bytes. Private checks, probes, native RED/GREEN logs and Shell
images are retained in ignored artifacts/icon. Existing GNU coverage, mutation,
external input, physical monitor transition and hosted CI gaps remain disclosed
in the earlier validation files; no new coverage or acceptance score is claimed.

The prior running preview, feature worktree, captures and warm caches are retained.
Publication uses a separate feature branch/PR. A merge of this icon change requires
separate authorization from the earlier merge of PR #1.
