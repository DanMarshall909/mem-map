# Mem Map frontend

Plain TypeScript, normal-flow CSS and SVG. Rust owns domain state; the renderer
uses only the four commands in `docs/tauri-contract.md`. No fixture data is bundled
with the application. Test fixtures live under `tests/`.

`npm run test:fast` runs DOM/IPC-ordering tests. `npm run build` checks TypeScript
and produces `dist/` for the Tauri build. `npm run test:slow -- <executable> <output>`
delegates to the repository's actual Tauri/WebView2 acceptance runner; build the
desktop executable first. Browser fixtures do not establish native rendering,
Windows UIA, clipboard or CSV correctness.

Dependencies are pinned in `package-lock.json`. On this low-space Windows
worktree, `node_modules` is a junction to
`C:\Users\DanMarshall\AppData\Local\mem-map-build\frontend-tauri-node_modules\node_modules`.
The installation container and npm cache live on C:. Install into that container
with the same package/lock files and retain the junction; npm can replace a
junction if asked to install directly into this worktree.

The table virtualizes rows, preserves an identity-based scroll anchor and uses
roving row focus. Inspector controls and its one scroll container persist across
updates. History navigation changes Rust's time viewport, not inspector scroll.
Collection updates never explicitly focus a control or reveal the selection;
only an accepted Show processes/F8 response or user navigation may reveal a row.
The same `reveal_row_id` in a later poll never replays navigation.
