//! UI-neutral dashboard state. Rust owns identity, sampling coherence and calculations.
use crate::{
    dashboard::{
        self, Diagnostics, EntitySummary, HistorySeries, Metric, OverviewPlot, Range, Sort,
        SortColumn, TreeRow,
    },
    model::{Grouping, History, Selection, Snapshot},
    preferences::Preferences,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

#[derive(Clone, Debug, Serialize)]
pub struct Reading {
    pub value: Option<String>,
    pub measured: usize,
    pub total: usize,
    pub reason: Option<String>,
    pub enumeration_complete: bool,
}
impl From<&dashboard::Reading> for Reading {
    fn from(r: &dashboard::Reading) -> Self {
        Self {
            value: r.value.map(|n| n.to_string()),
            measured: r.measured,
            total: r.total,
            reason: r.reason.clone(),
            enumeration_complete: r.enumeration_complete,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct ChangeReading {
    pub value: Option<String>,
    pub reason: Option<String>,
}
impl From<&dashboard::ChangeReading> for ChangeReading {
    fn from(r: &dashboard::ChangeReading) -> Self {
        Self {
            value: r.value.map(|n| n.to_string()),
            reason: r.reason.clone(),
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Row {
    pub id: String,
    pub parent_id: Option<String>,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub selected: bool,
    pub name: String,
    pub detail: String,
    pub pid: Option<u32>,
    pub count: usize,
    pub matching_count: usize,
    pub path_count: usize,
    pub lifecycle: String,
    pub exited_at: Option<u32>,
    pub private: Reading,
    pub commit: Reading,
    pub peak: Reading,
    pub change: ChangeReading,
}
#[derive(Clone, Debug, Serialize)]
pub struct Selected {
    pub id: String,
    pub name: String,
    pub paths: Vec<String>,
    pub pid: Option<u32>,
    pub count: usize,
    pub path_count: usize,
    pub paths_complete: bool,
    pub lifecycle: String,
    pub exited_at: Option<u32>,
    pub last_observed: Option<u32>,
    pub recorded: u32,
    pub private: Reading,
    pub commit: Reading,
    pub peak: Reading,
    pub change: ChangeReading,
}
#[derive(Clone, Debug, Serialize)]
pub struct Table {
    pub rows: Vec<Row>,
    pub selected_id: Option<String>,
    pub selection_hidden: bool,
    pub order_held: bool,
    pub total_roots: usize,
    pub matching_roots: usize,
    pub private_scale_bytes: String,
    pub change_scale_bytes: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct OverviewLayer {
    pub name: String,
    pub id: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct OverviewBaseline {
    pub at: u32,
    pub layers: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct OverviewSample {
    pub at: u32,
    pub captured_unix_ms: String,
    pub capacity: String,
    pub available: Option<String>,
    pub used: Option<String>,
    pub layers: Option<Vec<String>>,
    pub baseline: Option<OverviewBaseline>,
    pub measured: usize,
    pub observed: usize,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Overview {
    pub start: u32,
    pub end: u32,
    pub recorded: u32,
    pub capacity: String,
    pub layers: Vec<OverviewLayer>,
    pub samples: Vec<OverviewSample>,
}
#[derive(Clone, Debug, Serialize)]
pub struct HistoryValue {
    pub at: u32,
    pub captured_unix_ms: Option<String>,
    pub reading: Reading,
}
#[derive(Clone, Debug, Serialize)]
pub struct Series {
    pub start: u32,
    pub end: u32,
    pub recorded: u32,
    pub points: Vec<HistoryValue>,
    pub current: Reading,
}
#[derive(Clone, Debug, Serialize)]
pub struct Collection {
    pub status: String,
    pub age_seconds: u64,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SortSetting {
    pub column: String,
    pub descending: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Settings {
    pub grouping: String,
    pub filter: String,
    pub show_exited: bool,
    pub compact: bool,
    pub change_seconds: u32,
    pub sort: SortSetting,
    pub overview_range: Option<u32>,
    pub history_range: Option<u32>,
    pub metric: String,
    pub from_zero: bool,
    pub ui_zoom: u32,
    pub column_widths: [i32; 6],
    pub splitter_permille: u32,
}
#[derive(Clone, Debug, Serialize)]
pub struct HistoryViewport {
    pub follow: bool,
    pub manual_end: Option<u32>,
    pub manual_width: Option<u32>,
    pub reset_enabled: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct DiagnosticView {
    pub observed_session: usize,
    pub current_observed: usize,
    pub current_measured: usize,
    pub current_enumeration_complete: bool,
    pub current_unavailable: usize,
    pub retained_snapshots: usize,
    pub omitted_tracks: usize,
    pub last_success: Option<u32>,
    pub failures: Vec<String>,
    pub own_private: Option<String>,
    pub own_working_set: Option<String>,
}
impl From<Diagnostics> for DiagnosticView {
    fn from(d: Diagnostics) -> Self {
        Self {
            observed_session: d.observed_session,
            current_observed: d.current_observed,
            current_measured: d.current_measured,
            current_enumeration_complete: d.current_enumeration_complete,
            current_unavailable: d.current_unavailable,
            retained_snapshots: d.retained_snapshots,
            omitted_tracks: d.omitted_tracks,
            last_success: d.last_success,
            failures: d.failures,
            own_private: d.own_private.map(|n| n.to_string()),
            own_working_set: d.own_working_set.map(|n| n.to_string()),
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub revision: u64,
    pub displayed_at: u32,
    pub displayed_unix_ms: String,
    pub live_at: u32,
    pub paused: bool,
    pub collection: Collection,
    pub settings: Settings,
    pub table: Table,
    pub selected: Option<Selected>,
    pub overview: Overview,
    pub history: Option<Series>,
    pub history_viewport: HistoryViewport,
    pub diagnostics: DiagnosticView,
    pub notice: String,
    pub reveal_row_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    SetGrouping { value: String },
    SetFilter { value: String },
    SetShowExited { value: bool },
    SetCompact { value: bool },
    SetChangeSeconds { value: u32 },
    SortBy { column: String },
    SelectRow { id: String },
    ToggleExpanded { id: String },
    ShowSelectedProcesses,
    SelectProcessParent,
    SetTableInteractionHeld { value: bool },
    SetPaused { value: bool },
    SetMetric { value: String },
    SetOverviewRange { seconds: Option<u32> },
    SetHistoryRange { seconds: Option<u32> },
    SetFromZero { value: bool },
    SetFollow { value: bool },
    SetHistoryViewport { end: u32, width: u32 },
    ResetHistoryView,
    SetColumnWidths { value: [i32; 6] },
    SetSplitter { value: u32 },
    SetUiZoom { value: u32 },
}

pub struct Application {
    history: History,
    frozen: Option<History>,
    prefs: Preferences,
    selected: Option<Selection>,
    expanded: BTreeSet<Selection>,
    ids: BTreeMap<Selection, String>,
    next_id: u64,
    grouping: Grouping,
    query: String,
    show_exited: bool,
    change_seconds: u32,
    sort: Sort,
    table_held: bool,
    order_held: bool,
    all_rows: Vec<TreeRow>,
    rows: Vec<TreeRow>,
    total_roots: usize,
    metric: Metric,
    follow: bool,
    history_end: Option<u32>,
    history_width: Option<u32>,
    revision: u64,
    received_at: Option<Instant>,
    stopped: bool,
    error: Option<String>,
    notice: String,
    reveal_row_id: Option<String>,
    cached: Option<View>,
}

impl Application {
    pub fn new(mut prefs: Preferences) -> Self {
        let grouping = parse_grouping(&prefs.grouping).unwrap_or(Grouping::Name);
        prefs.grouping = grouping_name(grouping).into();
        prefs.splitter_permille = prefs.splitter_permille.clamp(400, 800);
        prefs.ui_zoom = prefs.ui_zoom.clamp(50, 200);
        if !valid_range(prefs.overview_range) {
            prefs.overview_range = Some(1800);
        }
        if !valid_range(prefs.history_range) {
            prefs.history_range = Some(1800);
        }
        for (width, minimum) in prefs
            .column_widths
            .iter_mut()
            .zip([160, 55, 80, 80, 80, 80])
        {
            *width = (*width).clamp(minimum, 1600);
        }
        let mut app = Self {
            history: History::default(),
            frozen: None,
            prefs,
            selected: None,
            expanded: BTreeSet::new(),
            ids: BTreeMap::new(),
            next_id: 0,
            grouping,
            query: String::new(),
            show_exited: false,
            change_seconds: 300,
            sort: Sort::default(),
            table_held: false,
            order_held: false,
            all_rows: Vec::new(),
            rows: Vec::new(),
            total_roots: 0,
            metric: Metric::PrivateWorkingSet,
            follow: true,
            history_end: None,
            history_width: None,
            revision: 0,
            received_at: None,
            stopped: false,
            error: None,
            notice: String::new(),
            reveal_row_id: None,
            cached: None,
        };
        app.refresh_rows(true);
        app.project();
        app
    }
    fn shown(&self) -> &History {
        self.frozen.as_ref().unwrap_or(&self.history)
    }
    fn protect_selected(&mut self) {
        if let Some(shown) = &self.frozen {
            self.history
                .protect_selection_from(shown, self.selected.as_ref());
        } else {
            self.history.protect_selection(self.selected.as_ref());
        }
    }
    pub fn ingest(&mut self, snapshot: Snapshot) {
        self.protect_selected();
        self.error = snapshot
            .system_error
            .clone()
            .or(snapshot.collection_error.clone());
        self.received_at = Some(Instant::now());
        self.stopped = false;
        self.history.observe(snapshot);
        self.reveal_row_id = None;
        if self.frozen.is_none() {
            self.refresh_rows(false);
        }
        self.revision += 1;
        self.project();
    }
    pub fn mark_collector_stopped(&mut self) {
        self.stopped = true;
        self.revision += 1;
        self.project();
    }
    pub fn view(&self) -> View {
        let mut view = self
            .cached
            .as_ref()
            .expect("projection initialized")
            .clone();
        view.collection = self.collection();
        view
    }
    pub fn preferences(&self) -> Preferences {
        self.prefs.clone()
    }
    pub fn selected_paths(&self) -> Result<Vec<String>, String> {
        let summary = self
            .selected
            .as_ref()
            .and_then(|s| dashboard::summary(self.shown(), s, self.change_seconds))
            .ok_or("Select a process or group first")?;
        if summary.paths.is_empty() {
            return Err("No executable path is available for the selected item".into());
        }
        Ok(summary.paths)
    }
    pub fn export_csv(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        dashboard::export_snapshot(
            self.shown(),
            self.grouping,
            self.show_exited,
            &self.query,
            &mut bytes,
        )
        .map_err(|e| e.to_string())?;
        Ok(bytes)
    }
    fn collection(&self) -> Collection {
        let age_seconds = self.received_at.map_or(0, |at| at.elapsed().as_secs());
        let status = if self.stopped {
            "stopped"
        } else if self.error.is_some() {
            "unavailable"
        } else if self.received_at.is_none() {
            "collecting"
        } else if age_seconds > 6 {
            "delayed"
        } else {
            "live"
        };
        Collection {
            status: status.into(),
            age_seconds,
            error: self.error.clone(),
        }
    }
    fn id_for(&self, selection: &Selection) -> String {
        self.ids
            .get(selection)
            .expect("projected identity registered")
            .clone()
    }
    fn register(&mut self, selection: Selection) {
        if !self.ids.contains_key(&selection) {
            self.next_id += 1;
            self.ids
                .insert(selection, format!("entity-{}", self.next_id));
        }
    }
    fn resolve_id(&self, id: &str) -> Result<Selection, String> {
        self.ids
            .iter()
            .find(|(_, value)| value.as_str() == id)
            .map(|(selection, _)| self.shown().resolve_selection(selection))
            .ok_or_else(|| "Unknown row identity".into())
    }
    fn refresh_rows(&mut self, explicit: bool) {
        let previous_ids = std::mem::take(&mut self.ids);
        for (selection, id) in previous_ids {
            let canonical = self.shown().resolve_selection(&selection);
            self.ids.entry(canonical).or_insert(id);
        }
        self.selected = self
            .selected
            .as_ref()
            .map(|s| self.shown().resolve_selection(s));
        self.expanded = self
            .expanded
            .iter()
            .map(|s| self.shown().resolve_selection(s))
            .collect();
        self.total_roots = dashboard::tree_rows(
            self.shown(),
            self.grouping,
            self.show_exited,
            "",
            self.change_seconds,
            self.sort,
        )
        .iter()
        .filter(|r| r.depth == 0)
        .count();
        let previous_order: BTreeMap<_, _> = self
            .all_rows
            .iter()
            .enumerate()
            .map(|(index, row)| (self.shown().resolve_selection(&row.selection), index))
            .collect();
        let mut next = dashboard::tree_rows(
            self.shown(),
            self.grouping,
            self.show_exited,
            &self.query,
            self.change_seconds,
            self.sort,
        );
        self.order_held = self.table_held && !explicit;
        if self.order_held && !previous_order.is_empty() {
            fn walk(
                parent: Option<&Selection>,
                rows: &[TreeRow],
                previous: &BTreeMap<Selection, usize>,
                output: &mut Vec<TreeRow>,
            ) {
                let mut children: Vec<_> = rows
                    .iter()
                    .filter(|r| r.parent.as_ref() == parent)
                    .collect();
                children.sort_by_key(|r| previous.get(&r.selection).copied().unwrap_or(usize::MAX));
                for row in children {
                    output.push(row.clone());
                    walk(Some(&row.selection), rows, previous, output);
                }
            }
            let mut held = Vec::new();
            walk(None, &next, &previous_order, &mut held);
            next = held;
        }
        for row in &next {
            self.register(row.selection.clone());
        }
        for name in self.shown().named_bands.clone() {
            self.register(Selection::Name(name));
        }
        self.all_rows = next;
        self.rows = self
            .all_rows
            .iter()
            .filter(|row| {
                if !self.query.is_empty() {
                    return true;
                }
                let mut parent = row.parent.as_ref();
                while let Some(p) = parent {
                    if !self.expanded.contains(p) {
                        return false;
                    }
                    parent = self
                        .all_rows
                        .iter()
                        .find(|r| &r.selection == p)
                        .and_then(|r| r.parent.as_ref());
                }
                true
            })
            .cloned()
            .collect();
        if self.selected.is_none() {
            self.selected = self.rows.first().map(|r| r.selection.clone());
        }
        self.protect_selected();
    }
    fn build_series(&self) -> Option<HistorySeries> {
        self.selected.as_ref().map(|selection| {
            let mut series = dashboard::series(self.shown(), selection, self.metric, Range::All);
            let end = if self.follow {
                self.shown().at
            } else {
                self.history_end.unwrap_or(self.shown().at)
            }
            .min(series.end);
            let start = self
                .history_width
                .or(self.prefs.history_range)
                .map_or(series.start, |width| {
                    end.saturating_sub(width).max(series.start)
                })
                .min(end);
            series
                .points
                .retain(|point| point.at >= start && point.at <= end);
            series.start = start;
            series.end = end;
            series.current = series
                .points
                .last()
                .map(|point| point.reading.clone())
                .unwrap_or_default();
            series
        })
    }
    fn project(&mut self) {
        let shown = self.shown();
        let timestamps: BTreeMap<_, _> = shown
            .system
            .iter()
            .map(|sample| (sample.at, sample.captured_unix_ms))
            .collect();
        // Baselines may precede the visible plot. Keep the complete retained
        // decomposition until the projection attaches each sample's context.
        let overview = dashboard::overview(shown, Range::All);
        let selected = self
            .selected
            .as_ref()
            .and_then(|s| dashboard::summary(shown, s, self.change_seconds))
            .map(|s| self.summary_view(s));
        let history = self
            .build_series()
            .map(|series| series_view(series, &timestamps));
        let process_scale = matches!(self.grouping, Grouping::Tree | Grouping::Process);
        let private_scale = self
            .all_rows
            .iter()
            .filter(|r| {
                if process_scale {
                    r.pid.is_some()
                } else {
                    r.depth == 0
                }
            })
            .filter_map(|r| r.private.value)
            .max()
            .unwrap_or(1)
            .max(1);
        let change_scale = self
            .all_rows
            .iter()
            .filter_map(|r| r.change.value.map(i64::unsigned_abs))
            .max()
            .unwrap_or(1)
            .max(1);
        let selected_id = self.selected.as_ref().map(|s| self.id_for(s));
        let selection_hidden = self
            .selected
            .as_ref()
            .is_some_and(|s| !self.rows.iter().any(|r| &r.selection == s));
        let rows = self
            .rows
            .iter()
            .map(|r| Row {
                id: self.id_for(&r.selection),
                parent_id: r.parent.as_ref().map(|p| self.id_for(p)),
                depth: r.depth,
                has_children: r.has_children,
                expanded: r.has_children
                    && (self.expanded.contains(&r.selection) || !self.query.is_empty()),
                selected: self.selected.as_ref() == Some(&r.selection),
                name: r.name.clone(),
                detail: r.detail.clone(),
                pid: r.pid,
                count: r.count,
                matching_count: r.matching_count,
                path_count: r.path_count,
                lifecycle: lifecycle_name(r.lifecycle).into(),
                exited_at: r.exited_at,
                private: (&r.private).into(),
                commit: (&r.commit).into(),
                peak: (&r.peak).into(),
                change: (&r.change).into(),
            })
            .collect();
        let view = View {
            revision: self.revision,
            displayed_at: shown.at,
            displayed_unix_ms: shown
                .system
                .back()
                .map_or(0, |s| s.captured_unix_ms)
                .to_string(),
            live_at: self.history.at,
            paused: self.frozen.is_some(),
            collection: self.collection(),
            settings: Settings {
                grouping: grouping_name(self.grouping).into(),
                filter: self.query.clone(),
                show_exited: self.show_exited,
                compact: self.prefs.compact,
                change_seconds: self.change_seconds,
                sort: SortSetting {
                    column: sort_name(self.sort.column).into(),
                    descending: self.sort.descending,
                },
                overview_range: self.prefs.overview_range,
                history_range: self.prefs.history_range,
                metric: metric_name(self.metric).into(),
                from_zero: self.prefs.from_zero,
                ui_zoom: self.prefs.ui_zoom,
                column_widths: self.prefs.column_widths,
                splitter_permille: self.prefs.splitter_permille,
            },
            table: Table {
                rows,
                selected_id,
                selection_hidden,
                order_held: self.order_held,
                total_roots: self.total_roots,
                matching_roots: self.all_rows.iter().filter(|r| r.depth == 0).count(),
                private_scale_bytes: private_scale.to_string(),
                change_scale_bytes: change_scale.to_string(),
            },
            selected,
            overview: self.overview_view(overview, &timestamps),
            history,
            history_viewport: HistoryViewport {
                follow: self.follow,
                manual_end: self.history_end,
                manual_width: self.history_width,
                reset_enabled: !self.follow
                    || self.history_end.is_some()
                    || self.history_width.is_some(),
            },
            diagnostics: dashboard::diagnostics(shown, std::process::id()).into(),
            notice: self.notice.clone(),
            reveal_row_id: self.reveal_row_id.clone(),
        };
        self.cached = Some(view);
    }
    fn summary_view(&self, s: EntitySummary) -> Selected {
        Selected {
            id: self.id_for(&s.selection),
            name: s.name,
            paths: s.paths,
            pid: s.pid,
            count: s.count,
            path_count: s.path_count,
            paths_complete: s.paths_complete,
            lifecycle: lifecycle_name(s.lifecycle).into(),
            exited_at: s.exited_at,
            last_observed: s.last_observed,
            recorded: s.recorded,
            private: (&s.private).into(),
            commit: (&s.commit).into(),
            peak: (&s.peak).into(),
            change: (&s.change).into(),
        }
    }
    fn overview_view(&self, plot: OverviewPlot, timestamps: &BTreeMap<u32, u64>) -> Overview {
        let start = self.prefs.overview_range.map_or(plot.start, |seconds| {
            plot.end.saturating_sub(seconds).max(plot.start)
        });
        Overview {
            start,
            end: plot.end,
            recorded: plot.recorded,
            capacity: plot.capacity.to_string(),
            layers: plot
                .layers
                .into_iter()
                .map(|layer| OverviewLayer {
                    name: layer.name,
                    id: layer.selection.as_ref().map(|s| self.id_for(s)),
                })
                .collect(),
            samples: plot
                .samples
                .iter()
                .filter(|s| s.at >= start)
                .map(|s| OverviewSample {
                    at: s.at,
                    captured_unix_ms: timestamps
                        .get(&s.at)
                        .expect("overview sample originates in retained system history")
                        .to_string(),
                    capacity: s.capacity.to_string(),
                    available: s.available.map(|n| n.to_string()),
                    used: s.used.map(|n| n.to_string()),
                    layers: s
                        .layers
                        .as_ref()
                        .map(|values| values.iter().map(|n| n.to_string()).collect()),
                    baseline: s.at.checked_sub(self.change_seconds).and_then(|target| {
                        let end = plot.samples.partition_point(|point| point.at <= target);
                        plot.samples[..end]
                            .iter()
                            .rev()
                            .take_while(|point| target - point.at <= 4)
                            .find(|point| point.used.is_some() && point.layers.is_some())
                            .map(|point| OverviewBaseline {
                                at: point.at,
                                layers: point
                                    .layers
                                    .as_ref()
                                    .unwrap()
                                    .iter()
                                    .map(|n| n.to_string())
                                    .collect(),
                            })
                    }),
                    measured: s.measured,
                    observed: s.observed,
                    reason: s.reason.clone(),
                })
                .collect(),
        }
    }
    pub fn dispatch(&mut self, action: Action) -> Result<(), String> {
        // Validate before changing any state, so rejected commands preserve the projection.
        let target = match &action {
            Action::SelectRow { id } => Some(self.resolve_id(id)?),
            Action::ToggleExpanded { id } => {
                let selection = self.resolve_id(id)?;
                if !self
                    .all_rows
                    .iter()
                    .any(|row| row.selection == selection && row.has_children)
                {
                    return Err("This row has no children".into());
                }
                Some(selection)
            }
            Action::ShowSelectedProcesses => {
                let selection = self.selected.clone().ok_or("Select a group first")?;
                if !self
                    .all_rows
                    .iter()
                    .any(|row| row.selection == selection && row.has_children)
                {
                    return Err("The selected item has no children in this view".into());
                }
                Some(selection)
            }
            Action::SelectProcessParent => {
                let child = match self.selected.as_ref() {
                    Some(Selection::Process(key)) => *key,
                    _ => return Err("Select an individual process first".into()),
                };
                Some(Selection::Process(
                    self.shown()
                        .parent_target(child)
                        .ok_or("No parent process is available for the selected instance")?,
                ))
            }
            _ => None,
        };
        match &action {
            Action::SetGrouping { value } => {
                parse_grouping(value)?;
            }
            Action::SetMetric { value }
                if !["private_working_set", "private_commit"].contains(&value.as_str()) =>
            {
                return Err("Unsupported history metric".into());
            }
            Action::SortBy { column } => {
                parse_sort(column)?;
            }
            Action::SetFilter { value } if value.chars().count() > 2048 => {
                return Err("Filter is limited to 2048 characters".into());
            }
            Action::SetChangeSeconds { value } if ![60, 300, 900, 1800].contains(value) => {
                return Err("Unsupported change interval".into());
            }
            Action::SetOverviewRange { seconds } | Action::SetHistoryRange { seconds }
                if !valid_range(*seconds) =>
            {
                return Err("Unsupported history range".into());
            }
            Action::SetHistoryViewport { width, .. } if !(4..=1800).contains(width) => {
                return Err("History width must be between 4 and 1800 seconds".into());
            }
            Action::SetColumnWidths { value }
                if value.iter().any(|width| !(45..=1600).contains(width)) =>
            {
                return Err("Column widths must be between 45 and 1600 pixels".into());
            }
            Action::SetSplitter { value } if !(400..=800).contains(value) => {
                return Err("Splitter must be between 400 and 800 permille".into());
            }
            Action::SetUiZoom { value } if !(50..=200).contains(value) => {
                return Err("UI zoom must be between 50 and 200 percent".into());
            }
            _ => {}
        }
        let mut explicit = matches!(
            &action,
            Action::SetGrouping { .. }
                | Action::SetFilter { .. }
                | Action::SetShowExited { .. }
                | Action::SetChangeSeconds { .. }
                | Action::SortBy { .. }
        );
        self.reveal_row_id = None;
        match action {
            Action::SetGrouping { value } => {
                self.grouping = parse_grouping(&value)?;
                self.prefs.grouping = value;
            }
            Action::SetFilter { value } => self.query = value.trim().into(),
            Action::SetShowExited { value } => self.show_exited = value,
            Action::SetCompact { value } => self.prefs.compact = value,
            Action::SetChangeSeconds { value } => self.change_seconds = value,
            Action::SortBy { column } => {
                let column = parse_sort(&column)?;
                self.sort = Sort {
                    column,
                    descending: if column == self.sort.column {
                        !self.sort.descending
                    } else {
                        !matches!(column, SortColumn::Name | SortColumn::Pid)
                    },
                };
            }
            Action::SelectRow { .. } => {
                self.selected = target;
                explicit = false;
            }
            Action::ToggleExpanded { .. } => {
                let selection = target.expect("row expansion validated");
                if !self.expanded.remove(&selection) {
                    self.expanded.insert(selection);
                }
            }
            Action::ShowSelectedProcesses => {
                let selected = target.expect("group expansion validated");
                self.expanded.insert(selected.clone());
                let children: Vec<_> = self
                    .all_rows
                    .iter()
                    .filter(|r| r.parent.as_ref() == Some(&selected) && r.has_children)
                    .map(|r| r.selection.clone())
                    .collect();
                self.expanded.extend(children);
                self.reveal_row_id = self
                    .all_rows
                    .iter()
                    .find(|r| r.parent.as_ref() == Some(&selected))
                    .map(|r| self.id_for(&r.selection));
            }
            Action::SelectProcessParent => {
                let selection = target.expect("parent navigation validated");
                self.grouping = Grouping::Process;
                self.prefs.grouping = "process".into();
                self.show_exited = true;
                self.query.clear();
                self.selected = Some(selection.clone());
                self.register(selection.clone());
                self.reveal_row_id = Some(self.id_for(&selection));
                explicit = true;
            }
            Action::SetTableInteractionHeld { value } => {
                self.table_held = value;
                explicit = !value;
            }
            Action::SetPaused { value } => {
                self.protect_selected();
                if value && self.frozen.is_none() {
                    self.frozen = Some(self.history.clone());
                } else if !value {
                    self.frozen = None;
                }
                explicit = false;
            }
            Action::SetMetric { value } => {
                self.metric = if value == "private_commit" {
                    Metric::PrivateCommit
                } else {
                    Metric::PrivateWorkingSet
                }
            }
            Action::SetOverviewRange { seconds } => self.prefs.overview_range = seconds,
            Action::SetHistoryRange { seconds } => {
                if !self.follow && self.history_end.is_none() {
                    self.history_end = Some(self.build_series().map_or(self.shown().at, |s| s.end));
                }
                self.prefs.history_range = seconds;
                self.history_width = None;
                if self.follow {
                    self.history_end = None;
                }
            }
            Action::SetFromZero { value } => self.prefs.from_zero = value,
            Action::SetFollow { value } => {
                if self.follow && !value {
                    self.history_end = Some(self.build_series().map_or(self.shown().at, |s| s.end));
                }
                self.follow = value;
                if value {
                    self.history_end = None;
                }
            }
            Action::SetHistoryViewport { end, width } => {
                self.follow = false;
                self.history_end = Some(end.min(self.shown().at));
                self.history_width = Some(width);
            }
            Action::ResetHistoryView => {
                self.follow = true;
                self.history_end = None;
                self.history_width = None;
            }
            Action::SetColumnWidths { value } => self.prefs.column_widths = value,
            Action::SetSplitter { value } => self.prefs.splitter_permille = value,
            Action::SetUiZoom { value } => self.prefs.ui_zoom = value,
        }
        self.refresh_rows(explicit);
        self.revision += 1;
        self.project();
        Ok(())
    }
}

fn series_view(s: HistorySeries, timestamps: &BTreeMap<u32, u64>) -> Series {
    Series {
        start: s.start,
        end: s.end,
        recorded: s.recorded,
        points: s
            .points
            .into_iter()
            .map(|p| HistoryValue {
                at: p.at,
                captured_unix_ms: timestamps.get(&p.at).map(|at| at.to_string()),
                reading: (&p.reading).into(),
            })
            .collect(),
        current: (&s.current).into(),
    }
}
fn lifecycle_name(lifecycle: dashboard::Lifecycle) -> &'static str {
    match lifecycle {
        dashboard::Lifecycle::Live => "live",
        dashboard::Lifecycle::Unavailable => "unavailable",
        dashboard::Lifecycle::Exited => "exited",
    }
}
fn grouping_name(grouping: Grouping) -> &'static str {
    match grouping {
        Grouping::Name => "name",
        Grouping::Executable => "path",
        Grouping::Family => "family",
        Grouping::Process => "process",
        Grouping::Tree => "tree",
    }
}
fn parse_grouping(value: &str) -> Result<Grouping, String> {
    match value {
        "name" => Ok(Grouping::Name),
        "path" => Ok(Grouping::Executable),
        "family" => Ok(Grouping::Family),
        "process" => Ok(Grouping::Process),
        "tree" => Ok(Grouping::Tree),
        _ => Err("Unsupported grouping".into()),
    }
}
fn metric_name(metric: Metric) -> &'static str {
    match metric {
        Metric::PrivateWorkingSet => "private_working_set",
        Metric::PrivateCommit => "private_commit",
    }
}
fn sort_name(column: SortColumn) -> &'static str {
    match column {
        SortColumn::Name => "name",
        SortColumn::Pid => "pid",
        SortColumn::PrivateWorkingSet => "private_working_set",
        SortColumn::Peak => "peak",
        SortColumn::PrivateCommit => "private_commit",
        SortColumn::Change => "change",
    }
}
fn parse_sort(value: &str) -> Result<SortColumn, String> {
    match value {
        "name" => Ok(SortColumn::Name),
        "pid" => Ok(SortColumn::Pid),
        "private_working_set" => Ok(SortColumn::PrivateWorkingSet),
        "peak" => Ok(SortColumn::Peak),
        "private_commit" => Ok(SortColumn::PrivateCommit),
        "change" => Ok(SortColumn::Change),
        _ => Err("Unsupported sort column".into()),
    }
}
fn valid_range(range: Option<u32>) -> bool {
    range.is_none_or(|seconds| [300, 900, 1800].contains(&seconds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProcessKey, ProcessSample};
    fn process(pid: u32, name: &str, private: u64) -> ProcessSample {
        ProcessSample {
            key: ProcessKey {
                pid,
                created: 133_123_456_789_012_345 + u64::from(pid),
                generation: 0,
            },
            name: name.into(),
            image: format!("C:\\Program Files\\{name}"),
            private,
            commit: private * 2,
            working_set: private + 17,
            parent_pid: 0,
        }
    }
    fn frame(at: u32, processes: Vec<ProcessSample>) -> Snapshot {
        Snapshot {
            at,
            total: 32 << 30,
            available: 8 << 30,
            captured_unix_ms: 1_700_000_000_011 + u64::from(at) * 1000,
            processes,
            ..Default::default()
        }
    }
    #[test]
    fn overview_baselines_skip_invalid_attribution_and_never_underflow_startup() {
        let mut app = Application::new(Preferences::default());
        app.ingest(frame(2, vec![process(7, "Worker.exe", 100)]));
        assert!(app.view().overview.samples[0].baseline.is_none());
        app.ingest(frame(298, vec![process(7, "Worker.exe", 100)]));
        let mut invalid = frame(300, vec![process(7, "Worker.exe", 999)]);
        invalid.complete_scan = false;
        app.ingest(invalid);
        app.ingest(frame(601, vec![process(7, "Worker.exe", 150)]));
        let view = app.view();
        assert_eq!(
            view.overview
                .samples
                .last()
                .unwrap()
                .baseline
                .as_ref()
                .unwrap()
                .at,
            298
        );
        app.dispatch(Action::SetChangeSeconds { value: 60 })
            .unwrap();
        assert!(
            app.view()
                .overview
                .samples
                .last()
                .unwrap()
                .baseline
                .is_none()
        );
    }
    #[test]
    fn overview_change_baselines_use_retained_history_outside_the_plot() {
        for baseline_at in [300, 297, 296] {
            let mut app = Application::new(Preferences {
                overview_range: Some(300),
                ..Preferences::default()
            });
            for (at, value) in [(2, 100), (baseline_at, 100), (303, 125), (601, 150)] {
                app.ingest(frame(at, vec![process(7, "Worker.exe", value)]));
            }
            let wire = serde_json::to_value(app.view()).unwrap();
            let overview = &wire["overview"];
            let samples = overview["samples"].as_array().unwrap();
            assert_eq!(
                samples
                    .iter()
                    .map(|s| s["at"].as_u64().unwrap())
                    .collect::<Vec<_>>(),
                [303, 601]
            );
            let band = overview["layers"]
                .as_array()
                .unwrap()
                .iter()
                .position(|l| l["name"] == "Worker.exe")
                .unwrap();
            assert_eq!(samples[0]["baseline"]["at"], 2);
            assert_eq!(samples[0]["baseline"]["layers"][band], "100");
            if baseline_at >= 297 {
                assert_eq!(samples[1]["baseline"]["at"], baseline_at);
                assert_eq!(samples[1]["baseline"]["layers"][band], "100");
            } else {
                assert!(samples[1]["baseline"].is_null());
            }
            app.dispatch(Action::SetPaused { value: true }).unwrap();
            app.ingest(frame(603, vec![process(7, "Worker.exe", 900)]));
            assert_eq!(
                serde_json::to_value(app.view()).unwrap()["overview"],
                *overview
            );
        }
    }
    #[test]
    fn half_scale_survives_initialization_actions_and_invalid_boundaries() {
        let mut app = Application::new(Preferences {
            ui_zoom: 50,
            ..Preferences::default()
        });
        assert_eq!(app.view().settings.ui_zoom, 50);
        app.dispatch(Action::SetUiZoom { value: 100 }).unwrap();
        app.dispatch(Action::SetUiZoom { value: 50 }).unwrap();
        assert_eq!(app.preferences().ui_zoom, 50);
        for value in [0, 49, 201] {
            assert!(app.dispatch(Action::SetUiZoom { value }).is_err());
            assert_eq!(app.view().settings.ui_zoom, 50);
        }
    }
    #[test]
    fn first_zero_second_frame_crosses_the_wire_as_one_exact_coherent_dashboard() {
        let mut app = Application::new(Preferences::default());
        assert_eq!(app.view().collection.status, "collecting");
        let huge = 9_007_199_254_740_993;
        app.ingest(frame(0, vec![process(41, "Worker.exe", huge)]));
        let wire = serde_json::to_value(app.view()).unwrap();
        assert_eq!(wire["collection"]["status"], "live");
        assert_eq!(wire["displayed_at"], 0);
        assert_eq!(wire["displayed_unix_ms"], "1700000000011");
        assert_eq!(
            wire["table"]["rows"][0]["private"]["value"],
            "9007199254740993"
        );
        assert_eq!(wire["selected"]["commit"]["value"], "18014398509481986");
        assert_eq!(wire["overview"]["samples"][0]["used"], "25769803776");
        assert!(
            wire["overview"]["samples"][0]["layers"].is_null(),
            "inconsistent attribution retains a neutral valid system total"
        );
        assert_eq!(
            wire["history"]["points"][0]["reading"]["value"],
            "9007199254740993"
        );
        let csv = String::from_utf8(app.export_csv().unwrap()).unwrap();
        assert!(csv.contains("133123456789012386"));
        assert!(csv.contains("9007199254740993"));
        assert_eq!(wire["table"]["selected_id"], wire["selected"]["id"]);
    }
    #[test]
    fn inspecting_history_keeps_held_table_targets_until_explicit_sort_or_release() {
        let mut app = Application::new(Preferences::default());
        app.ingest(frame(
            0,
            vec![process(1, "Alpha.exe", 100), process(2, "Beta.exe", 80)],
        ));
        let alpha_id = app.view().table.rows[0].id.clone();
        app.dispatch(Action::SetTableInteractionHeld { value: true })
            .unwrap();
        app.ingest(frame(
            2,
            vec![process(1, "Alpha.exe", 50), process(2, "Beta.exe", 200)],
        ));
        assert_eq!(app.view().table.rows[0].id, alpha_id);
        app.dispatch(Action::SetMetric {
            value: "private_commit".into(),
        })
        .unwrap();
        assert_eq!(
            app.view().table.rows[0].id,
            alpha_id,
            "history controls must not reorder a held table"
        );
        assert!(app.view().table.order_held);
        app.dispatch(Action::SortBy {
            column: "private_commit".into(),
        })
        .unwrap();
        assert_eq!(
            app.view().table.rows[0].name,
            "Beta.exe",
            "an explicit header sort applies immediately"
        );
        assert!(!app.view().table.order_held);
        app.ingest(frame(
            4,
            vec![process(1, "Alpha.exe", 500), process(2, "Beta.exe", 20)],
        ));
        assert_eq!(
            app.view().table.rows[0].name,
            "Beta.exe",
            "new values stay at the user's click targets"
        );
        app.dispatch(Action::SetTableInteractionHeld { value: false })
            .unwrap();
        assert_eq!(app.view().table.rows[0].name, "Alpha.exe");
        assert_eq!(
            app.view().table.selected_id.as_deref(),
            Some(alpha_id.as_str())
        );
    }
    #[test]
    fn chart_tooltips_keep_actual_capture_times_when_wall_clock_does_not_match_elapsed_time() {
        let mut app = Application::new(Preferences::default());
        app.ingest(frame(0, vec![process(7, "Worker.exe", 100)]));
        let mut later = frame(2, vec![process(7, "Worker.exe", 150)]);
        later.captured_unix_ms = 1_700_000_009_999;
        app.ingest(later);
        let wire = serde_json::to_value(app.view()).unwrap();
        assert_eq!(
            wire["overview"]["samples"][0]["captured_unix_ms"],
            "1700000000011"
        );
        assert_eq!(
            wire["overview"]["samples"][1]["captured_unix_ms"],
            "1700000009999"
        );
        assert_eq!(
            wire["history"]["points"][0]["captured_unix_ms"],
            "1700000000011"
        );
        assert_eq!(
            wire["history"]["points"][1]["captured_unix_ms"],
            "1700000009999"
        );
        let mut retained = Application::new(Preferences::default());
        retained.ingest(frame(0, vec![process(8, "Exited.exe", 75)]));
        retained
            .dispatch(Action::SetGrouping {
                value: "process".into(),
            })
            .unwrap();
        let id = retained.view().table.rows[0].id.clone();
        retained.dispatch(Action::SelectRow { id }).unwrap();
        retained.ingest(frame(1802, vec![]));
        let retained_view = retained.view();
        let old_point = retained_view
            .history
            .unwrap()
            .points
            .into_iter()
            .find(|point| point.at == 0)
            .unwrap();
        assert_eq!(old_point.reading.value.as_deref(), Some("75"));
        assert_eq!(
            old_point.captured_unix_ms, None,
            "evicted wall-clock metadata must remain unknown"
        );
    }
    #[test]
    fn parent_navigation_uses_the_displayed_instance_even_when_the_live_pid_is_reused() {
        let mut app = Application::new(Preferences::default());
        let parent = process(1, "Parent.exe", 500);
        let mut child = process(2, "Child.exe", 100);
        child.parent_pid = 1;
        app.ingest(frame(0, vec![parent, child.clone()]));
        app.dispatch(Action::SetGrouping {
            value: "process".into(),
        })
        .unwrap();
        let child_id = app
            .view()
            .table
            .rows
            .iter()
            .find(|r| r.pid == Some(2))
            .unwrap()
            .id
            .clone();
        app.dispatch(Action::SelectRow { id: child_id }).unwrap();
        app.dispatch(Action::SetGrouping {
            value: "name".into(),
        })
        .unwrap();
        app.dispatch(Action::SetFilter {
            value: "Child".into(),
        })
        .unwrap();
        app.dispatch(Action::SetPaused { value: true }).unwrap();
        let mut reused = process(1, "Replacement.exe", 900);
        reused.key.created += 1000;
        app.ingest(frame(2, vec![reused, child]));
        let action = serde_json::from_str::<Action>(r#"{"type":"select_process_parent"}"#).unwrap();
        app.dispatch(action).unwrap();
        let view = app.view();
        assert_eq!(view.selected.as_ref().unwrap().pid, Some(1));
        assert_eq!(view.selected.as_ref().unwrap().name, "Parent.exe");
        assert_eq!(view.settings.grouping, "process");
        assert!(view.settings.show_exited);
        assert_eq!(view.settings.filter, "");
        assert_eq!(view.reveal_row_id, view.table.selected_id);
        let parent_id = view.table.selected_id;
        app.dispatch(Action::SetPaused { value: false }).unwrap();
        let resumed = app.view();
        assert_eq!(resumed.table.selected_id, parent_id);
        assert_eq!(resumed.selected.as_ref().unwrap().name, "Parent.exe");
        assert_eq!(resumed.selected.as_ref().unwrap().lifecycle, "exited");
        assert_ne!(
            resumed
                .table
                .rows
                .iter()
                .find(|r| r.name == "Replacement.exe")
                .unwrap()
                .id,
            parent_id.unwrap()
        );
    }
    #[test]
    fn paused_filtering_copy_and_export_use_one_frozen_snapshot_and_keep_hidden_selection() {
        let mut app = Application::new(Preferences::default());
        let one = process(1, "Worker.exe", 100);
        let mut two = process(2, "Worker.exe", 200);
        two.image = "C:\\A distinct path with spaces\\Worker.exe".into();
        let other = process(3, "Other.exe", 500);
        app.ingest(frame(0, vec![one.clone(), two.clone(), other.clone()]));
        let mut later_one = one.clone();
        later_one.private = 150;
        let mut later_two = two.clone();
        later_two.private = 250;
        app.ingest(frame(2, vec![later_one, later_two, other]));
        app.dispatch(Action::SetFilter {
            value: "Worker".into(),
        })
        .unwrap();
        let worker = app
            .view()
            .table
            .rows
            .iter()
            .find(|r| r.depth == 0)
            .unwrap()
            .id
            .clone();
        app.dispatch(Action::SelectRow { id: worker.clone() })
            .unwrap();
        app.dispatch(Action::ShowSelectedProcesses).unwrap();
        assert_eq!(
            app.view()
                .table
                .rows
                .iter()
                .filter(|r| r.pid.is_some())
                .count(),
            2
        );
        assert!(app.view().reveal_row_id.is_some());
        app.dispatch(Action::ToggleExpanded { id: worker.clone() })
            .unwrap();
        app.dispatch(Action::SetPaused { value: true }).unwrap();
        let frozen_used = app.view().overview.samples.last().unwrap().used.clone();
        let mut live = frame(
            4,
            vec![process(1, "Worker.exe", 1000), process(3, "Other.exe", 900)],
        );
        live.available = 16 << 30;
        app.ingest(live);
        let paused = app.view();
        assert_eq!(paused.displayed_at, 2);
        assert_eq!(paused.live_at, 4);
        assert_eq!(
            paused.selected.as_ref().unwrap().private.value.as_deref(),
            Some("400")
        );
        assert_eq!(paused.overview.samples.last().unwrap().used, frozen_used);
        assert_eq!(app.selected_paths().unwrap(), vec![two.image, one.image]);
        let csv = String::from_utf8(app.export_csv().unwrap()).unwrap();
        let lines: Vec<_> = csv.lines().collect();
        assert_eq!(
            lines.len(),
            3,
            "collapsed aggregate and children never duplicate export instances"
        );
        assert!(lines[1].starts_with("2,1,"));
        assert!(lines[2].starts_with("2,2,"));
        assert!(csv.contains(",150,"));
        assert!(csv.contains(",250,"));
        assert!(!csv.contains("Other.exe"));
        app.dispatch(Action::SetFilter {
            value: "Other".into(),
        })
        .unwrap();
        assert!(app.view().table.selection_hidden);
        assert_eq!(
            app.view().table.selected_id.as_deref(),
            Some(worker.as_str())
        );
        assert_eq!(app.view().selected.as_ref().unwrap().name, "Worker.exe");
        assert_eq!(
            String::from_utf8(app.export_csv().unwrap())
                .unwrap()
                .lines()
                .count(),
            2
        );
        app.dispatch(Action::SetPaused { value: false }).unwrap();
        let resumed = app.view();
        assert_eq!(resumed.displayed_at, 4);
        assert_eq!(
            resumed.selected.as_ref().unwrap().private.value.as_deref(),
            Some("1000")
        );
        assert_eq!(resumed.table.selected_id.as_deref(), Some(worker.as_str()));
        assert_ne!(resumed.overview.samples.last().unwrap().used, frozen_used);
    }
    #[test]
    fn history_navigation_latches_follow_and_reset_preserves_metric_range_and_preferences() {
        let mut app = Application::new(Preferences::default());
        for at in (0..=20).step_by(2) {
            app.ingest(frame(
                at,
                vec![process(7, "Worker.exe", 100 + u64::from(at))],
            ));
        }
        app.dispatch(Action::SetMetric {
            value: "private_commit".into(),
        })
        .unwrap();
        app.dispatch(Action::SetFollow { value: false }).unwrap();
        assert_eq!(app.view().history_viewport.manual_end, Some(20));
        app.dispatch(Action::SetHistoryViewport { end: 8, width: 30 })
            .unwrap();
        app.ingest(frame(22, vec![process(7, "Worker.exe", 122)]));
        let old = app.view();
        assert_eq!(old.history.as_ref().unwrap().end, 8);
        assert_eq!(
            old.history.as_ref().unwrap().current.value.as_deref(),
            Some("216")
        );
        assert_eq!(
            old.selected.as_ref().unwrap().commit.value.as_deref(),
            Some("244")
        );
        app.dispatch(Action::SetHistoryRange { seconds: Some(900) })
            .unwrap();
        assert!(!app.view().history_viewport.follow);
        assert_eq!(app.view().history_viewport.manual_width, None);
        assert_eq!(app.view().history.as_ref().unwrap().end, 8);
        app.dispatch(Action::SetOverviewRange { seconds: None })
            .unwrap();
        app.dispatch(Action::SetFromZero { value: false }).unwrap();
        app.dispatch(Action::SetColumnWidths {
            value: [400, 70, 230, 130, 140, 180],
        })
        .unwrap();
        app.dispatch(Action::SetSplitter { value: 720 }).unwrap();
        app.dispatch(Action::SetUiZoom { value: 175 }).unwrap();
        app.dispatch(Action::ResetHistoryView).unwrap();
        let latest = app.view();
        assert!(latest.history_viewport.follow);
        assert!(!latest.history_viewport.reset_enabled);
        assert_eq!(latest.history.as_ref().unwrap().end, 22);
        assert_eq!(latest.settings.metric, "private_commit");
        assert_eq!(latest.settings.history_range, Some(900));
        assert!(!latest.settings.from_zero);
        let prefs = app.preferences();
        assert_eq!(prefs.overview_range, None);
        assert_eq!(prefs.history_range, Some(900));
        assert_eq!(prefs.column_widths, [400, 70, 230, 130, 140, 180]);
        assert_eq!(prefs.splitter_permille, 720);
        assert_eq!(prefs.ui_zoom, 175);
    }
    #[test]
    fn failed_readings_keep_partial_wire_coverage_and_do_not_fabricate_chart_release() {
        use crate::model::ProcessObservation;
        let mut app = Application::new(Preferences::default());
        let one = process(1, "Worker.exe", 100);
        let two = process(2, "Worker.exe", 200);
        app.ingest(frame(0, vec![one, two.clone()]));
        let mut failed = frame(2, vec![process(1, "Worker.exe", 110)]);
        failed.observed.push(ProcessObservation {
            pid: 2,
            key: Some(two.key),
            name: two.name,
            image: Some(two.image),
            parent_pid: 0,
            error: Some("Access denied".into()),
        });
        app.ingest(failed);
        let view = app.view();
        let selected = view.selected.unwrap();
        assert_eq!(selected.private.value.as_deref(), Some("110"));
        assert_eq!(selected.private.measured, 1);
        assert_eq!(selected.private.total, 2);
        assert_eq!(view.diagnostics.current_unavailable, 1);
        assert!(view.overview.samples.last().unwrap().layers.is_none());
        assert_eq!(
            view.overview.samples.last().unwrap().used.as_deref(),
            Some("25769803776")
        );
        assert!(
            view.overview
                .samples
                .last()
                .unwrap()
                .reason
                .as_ref()
                .unwrap()
                .contains("unavailable")
        );
        assert_eq!(
            view.history
                .unwrap()
                .points
                .last()
                .unwrap()
                .reading
                .measured,
            1
        );
        app.ingest(Snapshot {
            at: 4,
            system_error: Some("System reading failed".into()),
            complete_scan: false,
            ..Default::default()
        });
        let unavailable = app.view();
        assert_eq!(unavailable.collection.status, "unavailable");
        assert!(unavailable.overview.samples.last().unwrap().used.is_none());
        app.mark_collector_stopped();
        assert_eq!(app.view().collection.status, "stopped");
        let mut waiting = Application::new(Preferences::default());
        waiting.mark_collector_stopped();
        assert_eq!(waiting.view().collection.status, "stopped");
    }
    #[test]
    fn recovered_identity_keeps_its_opaque_id_but_a_restarted_pid_gets_a_different_one() {
        use crate::model::ProcessObservation;
        let mut app = Application::new(Preferences::default());
        let mut first = frame(0, vec![]);
        first.observed.push(ProcessObservation {
            pid: 42,
            key: None,
            name: "Worker.exe".into(),
            image: None,
            parent_pid: 0,
            error: Some("Identity unavailable".into()),
        });
        app.ingest(first);
        app.dispatch(Action::SetGrouping {
            value: "process".into(),
        })
        .unwrap();
        let original_id = app.view().table.rows[0].id.clone();
        app.dispatch(Action::SelectRow {
            id: original_id.clone(),
        })
        .unwrap();
        let mut recovered = process(42, "Worker.exe", 345);
        recovered.key.created = (1_699_999_999_000 + 11_644_473_600_000) * 10_000;
        app.ingest(frame(2, vec![recovered.clone()]));
        assert_eq!(
            app.view().table.selected_id.as_deref(),
            Some(original_id.as_str())
        );
        assert_eq!(
            app.view()
                .selected
                .as_ref()
                .unwrap()
                .private
                .value
                .as_deref(),
            Some("345")
        );
        assert_eq!(app.view().diagnostics.observed_session, 1);
        app.dispatch(Action::SetShowExited { value: true }).unwrap();
        let mut restarted = recovered;
        restarted.key.created += 1;
        restarted.private = 678;
        app.ingest(frame(4, vec![restarted]));
        let view = app.view();
        let live = view
            .table
            .rows
            .iter()
            .find(|r| r.lifecycle == "live")
            .unwrap();
        assert_ne!(live.id, original_id);
        assert_eq!(
            view.table.selected_id.as_deref(),
            Some(original_id.as_str())
        );
        assert_eq!(view.selected.as_ref().unwrap().lifecycle, "exited");
        assert_eq!(view.diagnostics.observed_session, 2);
    }
    #[test]
    fn invalid_commands_and_unowned_payloads_are_rejected_without_changing_the_dashboard() {
        let mut app = Application::new(Preferences::default());
        app.ingest(frame(0, vec![process(1, "Worker.exe", 10)]));
        app.dispatch(Action::ShowSelectedProcesses).unwrap();
        let reveal = app.view().reveal_row_id;
        assert!(reveal.is_some());
        for command in [
            r#"{"type":"set_grouping","value":"arbitrary"}"#,
            r#"{"type":"set_metric","value":"total_working_set"}"#,
            r#"{"type":"sort_by","column":"arbitrary"}"#,
            r#"{"type":"select_row","id":"not-owned"}"#,
            r#"{"type":"set_history_range","seconds":17}"#,
            r#"{"type":"set_history_viewport","end":0,"width":0}"#,
            r#"{"type":"set_column_widths","value":[400,70,230,130,140,99999]}"#,
            r#"{"type":"set_splitter","value":999}"#,
            r#"{"type":"set_ui_zoom","value":0}"#,
        ] {
            let before = app.view();
            assert!(
                app.dispatch(serde_json::from_str(command).unwrap())
                    .is_err(),
                "{command}"
            );
            let after = app.view();
            assert_eq!(after.revision, before.revision);
            assert_eq!(after.table.selected_id, before.table.selected_id);
            assert_eq!(after.settings.filter, before.settings.filter);
            assert_eq!(
                after.selected.as_ref().unwrap().private.value,
                before.selected.as_ref().unwrap().private.value
            );
        }
        assert!(
            serde_json::from_str::<Action>(
                r#"{"type":"select_row","id":"entity-1","path":"C:\\anything"}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<Action>(r#"{"type":"copy_paths","text":"caller-controlled"}"#)
                .is_err()
        );
        app.mark_collector_stopped();
        assert_eq!(
            app.view().reveal_row_id,
            reveal,
            "rejected commands cannot erase an accepted reveal intent"
        );
    }
}
