import type { Row, View } from '../src/contract.ts';
export function fixture(): View {
  const r = { value: '1073741824', measured: 2, total: 2, reason: null, enumeration_complete: true };
  const root: Row = { id: 'name:worker', parent_id: null, depth: 0, has_children: true, expanded: true, selected: true,
    name: 'worker.exe', detail: 'C:\\A\\worker.exe', pid: null, count: 2, matching_count: 2, path_count: 1,
    lifecycle: 'live', exited_at: null, private: r, peak: r, commit: { ...r, value: '2147483648' }, change: { value: '-1048576', reason: null } };
  return { revision: 1, displayed_at: 600, displayed_unix_ms: '1800000000000', live_at: 600, paused: false,
    collection: { status: 'live', age_seconds: 0, error: null },
    system_memory: { capacity: '34359738368', used: '25769803776', available: '8589934592' },
    settings: { grouping: 'name', filter: '', show_exited: false, compact: true, change_seconds: 300,
      sort: { column: 'private_working_set', descending: true }, overview_range: 1800, history_range: 1800,
      metric: 'private_working_set', from_zero: true, overview_metric: 'private_working_set', overview_from_zero: true, ui_zoom: 100, column_widths: [300, 55, 190, 125, 125, 165], splitter_permille: 680 },
    table: { rows: [root, { ...root, id: 'process:123:18000000000000000', parent_id: root.id, depth: 1, has_children: false, expanded: false, selected: false, pid: 123, count: 1 }],
      selected_id: root.id, selection_hidden: false, order_held: false, total_roots: 1, matching_roots: 1, private_scale_bytes: '1073741824', change_scale_bytes: '1048576' },
    selected: { ...root, paths: ['C:\\A\\worker.exe'], paths_complete: true, last_observed: 600, recorded: 600 },
    overview: { start: 0, end: 600, recorded: 600, capacity: '34359738368', layers: [{ name: 'Other used RAM', id: null }, { name: 'worker.exe', id: root.id }],
      samples: [0, 300, 600].map(at => ({ at, captured_unix_ms: String(1800000000000 + at * 1000), capacity: '34359738368', used: '25769803776', available: '8589934592', layers: ['24696061952', '1073741824'], baseline: at < 300 ? null : { at: at - 300, layers: ['24696061952', '1073741824'] }, measured: 2, observed: 2, reason: null })) },
    history: { start: 0, end: 600, recorded: 600, current: r, points: [0, 300, 600].map(at => ({ at, captured_unix_ms: String(1800000000000 + at * 1000), reading: r })) },
    history_viewport: { follow: true, manual_end: null, manual_width: null, reset_enabled: false },
    diagnostics: { observed_session: 2, current_observed: 2, current_measured: 2, current_enumeration_complete: true, current_unavailable: 0, retained_snapshots: 301, omitted_tracks: 0, last_success: 600, failures: [], own_private: '10485760', own_working_set: '20971520' },
    notice: '', reveal_row_id: null };
}
