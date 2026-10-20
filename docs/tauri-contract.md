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

Historical migration evidence includes actual Tauri/WebView2, native resizing,
clipboard readback, CSV save dialog and Windows UIA. At the user's subsequent
request, all maintained automated tests now run headlessly. A test-only stdio
host uses the unchanged Rust application/sampler; Playwright renders the built
frontend in a headless browser and checks selection, filtering, hierarchy,
pause/recording, resize/zoom, copy/export data and Chromium accessibility.
Copy/export adapters are private to the test runner. They do not establish
native clipboard/dialog correctness, Tauri command authorization or Windows UIA;
the existing Rust origin test still covers its explicit origin policy. No
production API or Tauri permission is added for testing.

At constrained window sizes or enlarged text, regions stack in normal document
flow with a usable table viewport and a bounded inspector. Ordinary desktop
sizes retain the split workspace and visible header/footer. The user explicitly
removed the repeated-scroll regression on 2026-10-20; it is not a maintained test
or a release gate for this migration.

## Compact presentation (MM-010)

Working-set proportions use a subtle full-row background with exact numeric
cells. Percentage change is presentation over the existing coherent byte delta:
baseline = current private bytes minus delta. BigInt arithmetic preserves large
integers; partial/unknown endpoints stay unavailable, growth from zero is New,
exact zero is white, and a valid confirmed exit uses zero current memory.
Red/green text strength increases with absolute percentage, saturating at 100%.
The palette retains readable contrast over the row fill. Raw bytes/reasons remain
in tooltip/accessibility text and CSV. Sorting still uses the backend byte delta;
the change header describes that distinction.

Chart compactness follows its own width divided by interface scale. Compact
labels use a manual HTML popover, shown by hover/focus or an info-button tap.
The popup sits beside the plot when possible and stays in the viewport; it adds
no layout height or label gutter and does not change sampling/drag coordinates.
Escape, outside pointer input and leaving the chart by keyboard dismiss it.
Normal desktop charts retain inline labels. Flat surfaces retain input underlines,
selection/focus accents, column resizing and forced-color control boundaries.

## Main overview (MM-011)

A local view switch promotes the existing overview chart into the whole workspace.
The process panel, splitter and inspector become hidden, including their keyboard
navigation targets. The same chart, range control and shared header/footer remain.
Chart height follows the available overview panel rather than the sidebar cap;
expanded layout overrides normal reflow flex containers at narrow breakpoints.

Returning refreshes the keyed table from the latest controller view and restores
the existing process context. Collector updates never reset the local view choice.
Escape returns to the dashboard, after a compact chart's labels have had their
own Escape; dialogs retain their modal Escape. Ctrl+F and existing process/history
shortcuts first reveal the relevant controls. No new IPC action, persisted setting,
Tauri permission or backend behavior is introduced.

## Expanded chart detail (MM-012)

Expanded overview tick density follows plot width/height, capped at eight
intervals without duplicate labels on short timelines. Both charts label age
relative to their displayed right endpoint, ending at 0s even when paused/panned.

The expanded overview alone overlays a legend in the plot's top-left corner.
Each band is one line, with aligned name, bytes, share of physical RAM and change
percentage columns. Names use optional `program_name` metadata when supplied,
otherwise the layer's existing name, truncated to 20 Unicode characters including
an ellipsis. Full names and values remain in help. The current Rust projection
supplies executable names; this frontend does not guess friendly names or add
metadata collection, IPC, persistence or permissions.

All legend values and the cursor use the same nearest actual sample. Physical
shares use exact BigInt bytes and that sample's capacity. Band change uses the
selected change interval and the last valid attributed overview sample at or
before the target, no older than four seconds (two collection intervals). Missing
baselines/attribution stay unavailable; zero baselines use the table's existing
New/zero convention. Red indicates growth, green decline, white no change. These
changes describe the represented bands, including residual RAM, and do not reuse
filtered table rows. A shorter displayed range may lack the requested baseline.

Pointer departure restores the latest displayed sample regardless of focus.
Keyboard arrows still provide deliberate sample/band inspection. A pointer-
transparent cursor marks the inspected time and known used-memory point without
reflow. SVG clipping protects column boundaries; labels never remove stack bands.

The shared header owns the used/capacity/available summary. The plot keeps numeric
memory axes and its dashed capacity line, with no repeated summary, endpoint
bracket, below-plot legend or visible Chart data disclosure. Precise descriptions
remain available to assistive technology and through hover help. Compact axis
annotations retain hover/focus/tap behavior. Icon actions retain accessible names,
tooltips and existing commands. The backend and headless-only test policy remain.
