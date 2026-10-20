# Tauri interface contract

The user's 2026-10-20 instruction replaces the entire Win32 UI with Tauri.
The existing Rust sampler, history, dashboard calculations, CSV and presentation
preferences remain authoritative. The original system-memory specification owns
product behavior; its native painting mechanism is superseded.

## Ownership and wire format

`src/application.rs` owns the UI-neutral controller and serializable projections.
`src/desktop.rs` owns Tauri, collection and native copy/save boundaries.
`frontend/` owns DOM, CSS, SVG, keyboard/pointer input and local scroll positions.
Rust owns selection, expansion, grouping, sorting, filtering, paused history and
chart viewport state. No UI calculations may replace domain calculations.

Commands are `get_view() -> View`, `dispatch(action: Action) -> View`,
`copy_paths() -> string` and `export_csv() -> string`. Copy/export resolve the
current displayed selection/snapshot inside Rust; callers supply no text or path.
Only bundled local main-window content may invoke commands. The frontend polls
get_view, serializes actions, applies complete revisions and ignores stale ones.
No generic shell, filesystem or clipboard capability is exposed.

All field names and action tags use snake_case. Exact byte values and identity
integers are decimal strings. Elapsed seconds, counts, revision, PID, percentages
and pixel widths are JSON numbers. Selection IDs are opaque Rust-managed strings.
Unknown readings remain null with reason/coverage, never converted to zero.

## View

`View` has `revision`, `displayed_at`, `displayed_unix_ms` (decimal string),
`live_at`, `paused`, `collection` { `status`, `age_seconds`, `error` },
`settings`, `table`, `selected`, `overview`, `history`, `history_viewport`,
`diagnostics`, `notice` and `reveal_row_id` (nullable string).

`settings`: `grouping` (name/path/tree/process/family), `filter`, `show_exited`,
`compact`, `change_seconds`, `sort` { `column`, `descending` },
`overview_range`, `history_range` (nullable seconds), `metric`
(private_working_set/private_commit), `from_zero`, `ui_zoom`, `column_widths`
(six numbers), `splitter_permille`.

`table`: `rows`, `selected_id`, `selection_hidden`, `order_held`, `total_roots`,
`matching_roots`, `private_scale_bytes`, `change_scale_bytes` (decimal strings).
Rows contain `id`, `parent_id`, `depth`, `has_children`, `expanded`, `selected`,
`name`, `detail`, `pid`, `count`, `matching_count`, `path_count`, `lifecycle`
(live/unavailable/exited), `exited_at`, `private`, `commit`, `peak`, `change`.

`Reading`: `value` (nullable decimal string), `measured`, `total`, `reason`,
`enumeration_complete`. `ChangeReading`: `value` (nullable signed decimal string),
`reason`. `selected` is nullable and contains `id`, `name`, `paths`, `pid`,
`count`, `path_count`, `paths_complete`, `lifecycle`, `exited_at`,
`last_observed`, `recorded`, `private`, `commit`, `peak`, `change`.

`overview`: existing OverviewPlot shape, with layers { `name`, `id` (nullable) },
samples { `at`, `capacity`, `available`, `used`, `layers`, `measured`, `observed`,
`reason`, `captured_unix_ms` }. Byte values are decimal strings; unknown layers remain null.
`history`: nullable existing HistorySeries shape, points { `at`, `reading`, `captured_unix_ms` }.
Sample timestamps are exact decimal strings from retained Rust observations.
`history_viewport`: `follow`, `manual_end`, `manual_width`, `reset_enabled`.
`diagnostics`: existing Diagnostics shape with exact byte strings.

## Actions

Actions are tagged with `type`, with the listed additional fields:

- `set_grouping {value}`, `set_filter {value}`, `set_show_exited {value}`,
  `set_compact {value}`, `set_change_seconds {value}`, `sort_by {column}`.
- `select_row {id}`, `toggle_expanded {id}`, `show_selected_processes`,
  `select_process_parent` (F8), `set_table_interaction_held {value}`, `set_paused {value}`.
- `set_metric {value}`, `set_overview_range {seconds}`, `set_history_range {seconds}`,
  `set_from_zero {value}`, `set_follow {value}`,
  `set_history_viewport {end, width}`, `reset_history_view`.
- `set_column_widths {value}`, `set_splitter {value}`, `set_ui_zoom {value}`.

Actions validate bounded values and resolve IDs against known controller state.
Explicit sort/filter/group changes apply immediately even during order hold.
Follow-off latches the endpoint; changing range clears manual zoom and retains
Follow-off; reset clears manual viewport and restores Follow. The frontend consumes
`reveal_row_id` only from an accepted Show processes or F8 action response. Polls
never replay scrolling, and repeating the same explicit request reveals it again.

## Rendering acceptance

Persistent keyed DOM owns normal flow. The inspector and table each have one
scroll container; collection never changes focus or scroll. Root viewport has
no horizontal overflow at ordinary supported sizes. Sticky headers and reserved
scrollbar gutters protect content. All controls use native HTML semantics;
treegrid and charts provide keyboard navigation and textual readings.

Desktop evidence includes actual Tauri/WebView2 collection, pause/resume,
selection, filtering, hierarchy,
resize/zoom, clipboard readback and CSV. Browser fixture tests alone cannot prove
the desktop rendering boundary. Legacy GDI production UI and HWND-only smoke
checks are removed after replacement is verified.

At constrained window sizes or enlarged text, regions stack in normal document
flow with a usable table viewport and a bounded inspector. Ordinary desktop
sizes retain the split workspace and visible header/footer. The user explicitly
removed the repeated-scroll regression on 2026-10-20; it is not a maintained test
or a release gate for this migration.
