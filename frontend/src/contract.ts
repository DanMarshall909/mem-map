export type Decimal = string;
export type Grouping = 'name' | 'path' | 'tree' | 'process' | 'family';
export type Metric = 'private_working_set' | 'private_commit';
export type SortColumn = 'name' | 'pid' | 'private_working_set' | 'peak' | 'private_commit' | 'change';
export interface Reading { value: Decimal | null; measured: number; total: number; reason: string | null; enumeration_complete: boolean }
export interface ChangeReading { value: Decimal | null; reason: string | null }
export interface Row {
  id: string; parent_id: string | null; depth: number; has_children: boolean; expanded: boolean; selected: boolean;
  name: string; detail: string; pid: number | null; count: number; matching_count: number; path_count: number;
  lifecycle: 'live' | 'unavailable' | 'exited'; exited_at: number | null;
  private: Reading; commit: Reading; peak: Reading; change: ChangeReading;
}
export interface Selected {
  id: string; name: string; paths: string[]; pid: number | null; count: number; path_count: number;
  paths_complete: boolean; lifecycle: Row['lifecycle']; exited_at: number | null; last_observed: number | null;
  recorded: number; private: Reading; commit: Reading; peak: Reading; change: ChangeReading;
}
export interface Settings {
  grouping: Grouping; filter: string; show_exited: boolean; compact: boolean; change_seconds: number;
  sort: { column: SortColumn; descending: boolean }; overview_range: number | null; history_range: number | null;
  metric: Metric; from_zero: boolean; ui_zoom: number; column_widths: number[]; splitter_permille: number;
}
export interface OverviewSample {
  at: number; captured_unix_ms: Decimal; capacity: Decimal; available: Decimal | null; used: Decimal | null;
  layers: Decimal[] | null; measured: number; observed: number; reason: string | null;
}
export interface Overview {
  start: number; end: number; recorded: number; capacity: Decimal;
  layers: { name: string; program_name?: string | null; id: string | null }[]; samples: OverviewSample[];
}
export interface HistorySeries { start: number; end: number; recorded: number; current: Reading; points: { at: number; captured_unix_ms: Decimal | null; reading: Reading }[] }
export interface Diagnostics {
  observed_session: number; current_observed: number; current_measured: number; current_enumeration_complete: boolean;
  current_unavailable: number; retained_snapshots: number; omitted_tracks: number; last_success: number | null;
  failures: string[]; own_private: Decimal | null; own_working_set: Decimal | null;
}
export interface View {
  revision: number; displayed_at: number; displayed_unix_ms: Decimal; live_at: number; paused: boolean;
  collection: { status: string; age_seconds: number; error: string | null };
  settings: Settings;
  table: { rows: Row[]; selected_id: string | null; selection_hidden: boolean; order_held: boolean;
    total_roots: number; matching_roots: number; private_scale_bytes: Decimal; change_scale_bytes: Decimal };
  selected: Selected | null; overview: Overview; history: HistorySeries | null;
  history_viewport: { follow: boolean; manual_end: number | null; manual_width: number | null; reset_enabled: boolean };
  diagnostics: Diagnostics; notice: string; reveal_row_id: string | null;
}
export type Action =
  | { type: 'set_grouping'; value: Grouping }
  | { type: 'set_filter'; value: string }
  | { type: 'set_show_exited' | 'set_compact' | 'set_table_interaction_held' | 'set_paused' | 'set_from_zero' | 'set_follow'; value: boolean }
  | { type: 'set_change_seconds' | 'set_splitter' | 'set_ui_zoom'; value: number }
  | { type: 'sort_by'; column: SortColumn }
  | { type: 'select_row' | 'toggle_expanded'; id: string }
  | { type: 'show_selected_processes' | 'reset_history_view' | 'select_process_parent' }
  | { type: 'set_metric'; value: Metric }
  | { type: 'set_overview_range' | 'set_history_range'; seconds: number | null }
  | { type: 'set_history_viewport'; end: number; width: number }
  | { type: 'set_column_widths'; value: number[] };
