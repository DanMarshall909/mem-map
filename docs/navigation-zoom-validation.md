# Navigation, monospaced display and zoom validation - MM-004

Windows, 2026-10-19, Rust 1.99.1 x86_64-pc-windows-gnu. This extends feature
commit 75a4d43273917d5b1eaf7f91f03429c155b88a76 on feat/top-five-overlay.
Main remains at 15c7d0c18190a57de7dfd5bfb3d74769066b4ee3.
Independent completion review: PASS; private verdict retained in artifacts/mm004/review/verdict.md.

| Criterion | Local evidence |
| --- | --- |
| AC1 Parent navigation | Genuine model RED returned no target where the captured parent was expected. GREEN asserts the exact PID/creation-time key, retention after exit, safe refusal for unobserved/expired parents, and no redirect to a reused PID. Highlighted parent lines in process rows and selected details share the navigation action; F8 also invokes it. Native smoke sends a click with conflicting search/scope/inactive filters, processes the deferred native search clear, then asserts exact selection, cleared filters and a visible row. Its retained RED fails on parent row scrolled into view; GREEN preserves navigation scrolling across the reentrant edit notification. |
| AC2 Monospaced display | Consolas is used for all drawing and native edit/combo controls. Native GDI measurements assert nonzero equal widths for iiii and WWWW in every display font and the edit font. Inspected 100/125/200 percent renders show aligned numbers and readable metric controls. Axis labels and controls were widened or rearranged where the wider font would truncate their contents. |
| AC3 Chart zoom | Model RED then GREEN verifies bounded time steps, including non-preset values and limits. Wheel over either chart and plus/minus controls/keys select 30s, 1m, 2m, 5m, 10m, 15m or 30m. Existing 5/15/30m presets remain. Native window-message smoke asserts chart wheel zoom, unchanged observation count, growth duration and pinned identities. |
| AC4 Interface zoom | Model tests cover 75-200 percent bounds/reset, virtual-canvas dimensions, overflow clamps, 125 percent at 144 DPI, zero-size windows and coordinate round trips. Native smoke drives Ctrl+wheel, layout at 75/125/200/100 percent, scrollbar end positions, search-control bounds and 144/192 DPI messages, then restores DPI. Text, drawing, native controls and hit coordinates use the same viewport scale and pan. Ctrl+0 or clicking the percentage resets scale. |

Twenty fast tests pass in 0.04s excluding compilation. Two real Windows tests pass
in 1.00s. Lane membership remains appropriate: deterministic model cases are fast;
real process/allocation/handle fixtures stay slow. Formatting, locked Clippy with
denied warnings, locked release, and native smoke pass. Private RED/GREEN logs,
check outputs, screenshots and captures remain in ignored artifacts/mm004.

Release size: 473,600 bytes. SHA256:
`f6c0e0378cdd43b267e1badf42a4d675129871e2506b70151b54d0b76ef79989`.
The asserted run observed 498 process instances. Private RAM was 2,637,824 bytes
and working set 14,204,928 before diagnostic stress. With 490 tracks by 901 points,
private RAM was 10,592,256 and working set 22,171,648 bytes. The 105 full-history
redraws took 4,865ms, with GDI counts stable at 22 to 22. These are workload-specific
process measurements, not total GPU/kernel accounting or a long-duration soak.
Diagnostic repeated samples remain separate from real displayed/exported history.

Parent navigation stores one bounded optional process key per track, reusing the
existing capture. History caps, 12-byte points and CSV schema are unchanged.
Interface zoom scales the existing GDI buffer to the client area and exposes
overflow through native scrollbars; it adds no UI framework or dependency.
Layout uses the owning raw pointer without holding an App borrow across native
calls that can synchronously reenter the window procedure.

Coverage retains the previously verified Windows GNU profiler_builtins blocker.
Mutation is pending, with no source-bound lane map or isolated CoW harness; no
coverage or mutation score is claimed. Native smoke is in-process message and
shared-renderer evidence. External mouse/keyboard input, actual monitor moves,
full UI Automation, hosted CI and user acceptance remain unproved.

Independent review, publication and integration are distinct. The intended update
is the same public feature branch and open PR #1. No merge or cleanup is included;
the primary checkout, earlier app instances, captures and build caches are retained.
