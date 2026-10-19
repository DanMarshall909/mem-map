# Parent-name validation — MM-003

Windows, 2026-10-19, Rust 1.99.1 x86_64-pc-windows-gnu. This extends the current
feature branch at ab575327730c322f4243eafb7b8735de840d26af; main remains at
15c7d0c18190a57de7dfd5bfb3d74769066b4ee3. Independent review: PASS; private verdict retained in artifacts/mm003/review/verdict.md.

| Criterion | Evidence |
| --- | --- |
| AC1 Parent identity | Genuine model RED expects ChatGPT.exe (PID 10), while the original formatter reports Unavailable. GREEN verifies the observed name, retention after exit, rejection of a younger reused parent PID for an unknown child, preservation of the original cached name, and None for no recorded parent. |
| AC2 Visible/searchable name | The same regression searches by the retained parent name and expects exactly the child process. Native smoke selects an actually observed child and writes parent-detail.bmp. The inspected renderer shows parent name/PID in process rows and selected details, alongside all memory fields and the dense 22-row table. |
| AC3 Boundary/overhead | A real test launches a hidden, owned cmd.exe child blocked on stdin; capture must report this test process's independently known PID and executable filename as its parent. EOF releases the child, including during assertion failure. The two native tests share a fixture lock because handle counts are process-global. No production OS API, sampler or history-point layout was added. |

Checks: 18 fast tests pass in 0.04s excluding compilation; two real Windows tests
pass in 0.81s. Existing fast/slow lanes remain unchanged. Formatting, locked Clippy
with denied warnings, locked release and asserted native smoke pass. Private logs
and screenshots are retained under artifacts/mm003 and excluded from public source.

Release size: 461,824 bytes; SHA256
`5da738b37cff55da7e413fd2181264216c8b04617c832746587b70708d978253`.
Native smoke observed 411 measured instances. Working set was 11,587,584 bytes
before diagnostic stress and 18,165,760 with 407 tracks by 901 points. Private bytes
were 2,170,880 and 8,732,672 respectively. The 105 full-history redraws took 4,755 ms;
GDI object counts stayed 13→13. These are workload-specific process counters, not
total kernel/GPU/GDI accounting. Diagnostic repeated samples remain separate from
visible and exported observations.

Parent names are copied from the existing snapshot after creation-time validation
and retained once associated with that child instance. Unobserved/protected/exited
parents may have an unavailable name. Metadata remains bounded by MAX_TRACKS, while
each compact history point remains 12 bytes. CSV schema is unchanged.

Coverage retains the verified Windows GNU profiler_builtins blocker; mutation has
no established coverage/lane map or isolated CoW harness. Neither has a passing
score. Native external input, full UI Automation, hosted CI and business acceptance
remain unproved. Renderer inspection and controller smoke have narrower scope.

The primary checkout, running previews, captures and reusable build/tool caches
are retained. Publication updates the same feature branch and PR #1; it does not
authorize or establish a merge into main.
