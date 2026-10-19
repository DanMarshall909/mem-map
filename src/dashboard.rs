//! Snapshot-coherent presentation data. All byte values are unformatted.
use crate::model::{Grouping, History, ProcessKey, Selection};
use std::io::{self, Write};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Metric {
    #[default]
    PrivateWorkingSet,
    PrivateCommit,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Range {
    Last(u32),
    #[default]
    All,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortColumn {
    Name,
    Pid,
    #[default]
    PrivateWorkingSet,
    Peak,
    PrivateCommit,
    Change,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub column: SortColumn,
    pub descending: bool,
}
impl Default for Sort {
    fn default() -> Self {
        Self {
            column: SortColumn::PrivateWorkingSet,
            descending: true,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lifecycle {
    #[default]
    Live,
    Unavailable,
    Exited,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    pub value: Option<u64>,
    pub measured: usize,
    pub total: usize,
    pub reason: Option<String>,
    pub enumeration_complete: bool,
}
impl Default for Reading {
    fn default() -> Self {
        Self {
            value: None,
            measured: 0,
            total: 0,
            reason: None,
            enumeration_complete: true,
        }
    }
}
impl Reading {
    pub fn partial(&self) -> bool {
        self.value.is_some() && (self.measured < self.total || !self.enumeration_complete)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChangeReading {
    pub value: Option<i64>,
    pub reason: Option<String>,
}
#[derive(Clone, Debug)]
pub struct TreeRow {
    pub selection: Selection,
    pub parent: Option<Selection>,
    pub depth: usize,
    pub has_children: bool,
    pub name: String,
    pub detail: String,
    pub pid: Option<u32>,
    pub count: usize,
    pub matching_count: usize,
    pub path_count: usize,
    pub lifecycle: Lifecycle,
    pub exited_at: Option<u32>,
    pub private: Reading,
    pub commit: Reading,
    pub peak: Reading,
    pub change: ChangeReading,
}
#[derive(Clone, Debug)]
pub struct EntitySummary {
    pub selection: Selection,
    pub name: String,
    pub paths: Vec<String>,
    pub pid: Option<u32>,
    pub count: usize,
    pub path_count: usize,
    pub paths_complete: bool,
    pub lifecycle: Lifecycle,
    pub exited_at: Option<u32>,
    pub last_observed: Option<u32>,
    pub recorded: u32,
    pub private: Reading,
    pub commit: Reading,
    pub peak: Reading,
    pub change: ChangeReading,
    pub members: Vec<ProcessKey>,
}
#[derive(Clone, Debug)]
pub struct HistoryValue {
    pub at: u32,
    pub reading: Reading,
}
#[derive(Clone, Debug)]
pub struct HistorySeries {
    pub start: u32,
    pub end: u32,
    pub recorded: u32,
    pub points: Vec<HistoryValue>,
    pub current: Reading,
}
#[derive(Clone, Debug)]
pub struct OverviewLayer {
    pub name: String,
    pub selection: Option<Selection>,
}
#[derive(Clone, Debug)]
pub struct OverviewSample {
    pub at: u32,
    pub capacity: u64,
    pub available: Option<u64>,
    pub used: Option<u64>,
    /// Layer thicknesses in legend order. None retains a neutral system total.
    pub layers: Option<Vec<u64>>,
    pub measured: usize,
    pub observed: usize,
    pub reason: Option<String>,
}
#[derive(Clone, Debug)]
pub struct OverviewPlot {
    pub start: u32,
    pub end: u32,
    pub recorded: u32,
    pub capacity: u64,
    pub layers: Vec<OverviewLayer>,
    pub samples: Vec<OverviewSample>,
}
#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    pub observed_session: usize,
    pub current_observed: usize,
    pub current_measured: usize,
    pub current_enumeration_complete: bool,
    pub current_unavailable: usize,
    pub retained_snapshots: usize,
    pub omitted_tracks: usize,
    pub last_success: Option<u32>,
    pub failures: Vec<String>,
    pub own_private: Option<u64>,
    pub own_working_set: Option<u64>,
}

fn query_matches(track: &crate::model::Track, query: &str) -> bool {
    query.is_empty()
        || track.name.to_lowercase().contains(query)
        || track.display_image.to_lowercase().contains(query)
        || track.key.pid.to_string().contains(query)
        || track
            .parent_name
            .as_ref()
            .is_some_and(|name| name.to_lowercase().contains(query))
}

fn all_tracks(history: &History) -> Vec<std::borrow::Cow<'_, crate::model::Track>> {
    use std::borrow::Cow;
    let mut tracks: Vec<_> = history.tracks.values().map(Cow::Borrowed).collect();
    for observed in &history.latest.observed {
        if let Some(key) = observed.key
            && !history.tracks.contains_key(&key)
            && let Some(track) = history.current_track(key)
        {
            tracks.push(Cow::Owned(track));
        }
    }
    tracks
}

pub fn tree_rows(
    history: &History,
    grouping: Grouping,
    show_exited: bool,
    query: &str,
    change_seconds: u32,
    sort: Sort,
) -> Vec<TreeRow> {
    use std::collections::{BTreeMap, BTreeSet};
    let query = query.trim().to_lowercase();
    let all = all_tracks(history);
    let lookup: BTreeMap<_, _> = all.iter().map(|t| (t.key, t.as_ref())).collect();
    let mut eligible: BTreeSet<_> = all
        .iter()
        .filter(|t| show_exited || t.exited_at.is_none())
        .map(|t| t.key)
        .collect();
    if grouping == Grouping::Tree {
        let mut pending: Vec<_> = eligible.iter().copied().collect();
        while let Some(key) = pending.pop() {
            if let Some(parent) = lookup
                .get(&key)
                .and_then(|t| t.parent_key)
                .filter(|key| lookup.contains_key(key))
                && eligible.insert(parent)
            {
                pending.push(parent);
            }
        }
    }
    let tracks: Vec<_> = eligible
        .iter()
        .filter_map(|key| lookup.get(key).copied())
        .collect();
    let mut entities: BTreeMap<Selection, Option<Selection>> = BTreeMap::new();
    for track in &tracks {
        let leaf = Selection::Process(track.key);
        match grouping {
            Grouping::Name => {
                let root = Selection::Name(track.name.to_lowercase());
                entities.entry(root.clone()).or_insert(None);
                let paths: BTreeSet<_> = tracks
                    .iter()
                    .filter(|t| {
                        t.name.eq_ignore_ascii_case(&track.name)
                            || t.name.to_lowercase() == track.name.to_lowercase()
                    })
                    .map(|t| t.image.as_str())
                    .collect();
                if paths.len() > 1 && !track.image.is_empty() {
                    let path = Selection::App(track.image.clone());
                    entities.entry(path.clone()).or_insert(Some(root));
                    entities.insert(leaf, Some(path));
                } else {
                    entities.insert(leaf, Some(root));
                }
            }
            Grouping::Executable => {
                if track.image.is_empty() {
                    entities.insert(leaf, None);
                } else {
                    let root = Selection::App(track.image.clone());
                    entities.entry(root.clone()).or_insert(None);
                    entities.insert(leaf, Some(root));
                }
            }
            Grouping::Family => {
                let root = Selection::Family(track.family);
                entities.entry(root.clone()).or_insert(None);
                entities.insert(leaf, Some(root));
            }
            Grouping::Process => {
                entities.insert(leaf, None);
            }
            Grouping::Tree => {
                entities.insert(
                    leaf,
                    track
                        .parent_key
                        .filter(|k| eligible.contains(k))
                        .map(Selection::Process),
                );
            }
        }
    }
    let mut keep: BTreeSet<Selection> = entities
        .keys()
        .filter(|selection| {
            tracks
                .iter()
                .any(|t| history.matches(t, selection) && query_matches(t, &query))
        })
        .cloned()
        .collect();
    let mut pending: Vec<_> = keep.iter().cloned().collect();
    while let Some(key) = pending.pop() {
        if let Some(Some(parent)) = entities.get(&key)
            && keep.insert(parent.clone())
        {
            pending.push(parent.clone());
        }
    }
    let mut rows: BTreeMap<Selection, TreeRow> = BTreeMap::new();
    for (selection, parent) in entities
        .into_iter()
        .filter(|(selection, _)| keep.contains(selection))
    {
        let Some(entity) = summary(history, &selection, change_seconds) else {
            continue;
        };
        let matching_count = tracks
            .iter()
            .filter(|t| history.matches(t, &selection) && query_matches(t, &query))
            .count();
        let detail = if entity.paths.len() == 1 {
            entity.paths[0].clone()
        } else if entity.paths.is_empty() {
            "Executable path unavailable".into()
        } else {
            format!("{} executable paths", entity.paths.len())
        };
        rows.insert(
            selection.clone(),
            TreeRow {
                selection,
                parent,
                depth: 0,
                has_children: false,
                name: entity.name,
                detail,
                pid: entity.pid,
                count: entity.count,
                matching_count,
                path_count: entity.path_count,
                lifecycle: entity.lifecycle,
                exited_at: entity.exited_at,
                private: entity.private,
                commit: entity.commit,
                peak: entity.peak,
                change: entity.change,
            },
        );
    }
    fn compare(a: &TreeRow, b: &TreeRow, sort: Sort) -> std::cmp::Ordering {
        fn values<T: Ord>(a: Option<T>, b: Option<T>, descending: bool) -> std::cmp::Ordering {
            match (a, b) {
                (Some(a), Some(b)) => {
                    if descending {
                        b.cmp(&a)
                    } else {
                        a.cmp(&b)
                    }
                }
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        }
        let order = match sort.column {
            SortColumn::Name => {
                if sort.descending {
                    b.name.to_lowercase().cmp(&a.name.to_lowercase())
                } else {
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                }
            }
            SortColumn::Pid => values(a.pid, b.pid, sort.descending),
            SortColumn::PrivateWorkingSet => {
                values(a.private.value, b.private.value, sort.descending)
            }
            SortColumn::Peak => values(a.peak.value, b.peak.value, sort.descending),
            SortColumn::PrivateCommit => values(a.commit.value, b.commit.value, sort.descending),
            SortColumn::Change => values(a.change.value, b.change.value, sort.descending),
        };
        order.then_with(|| a.selection.cmp(&b.selection))
    }
    fn append(
        parent: Option<Selection>,
        depth: usize,
        rows: &mut BTreeMap<Selection, TreeRow>,
        out: &mut Vec<TreeRow>,
        sort: Sort,
    ) {
        let mut keys: Vec<_> = rows
            .values()
            .filter(|r| r.parent == parent)
            .map(|r| r.selection.clone())
            .collect();
        keys.sort_by(|a, b| compare(&rows[a], &rows[b], sort));
        for key in keys {
            let mut row = rows.remove(&key).unwrap();
            row.depth = depth;
            row.has_children = rows.values().any(|r| r.parent.as_ref() == Some(&key));
            out.push(row);
            append(Some(key), depth + 1, rows, out, sort);
        }
    }
    let mut result = Vec::new();
    append(None, 0, &mut rows, &mut result, sort);
    result
}
fn members<'a>(
    history: &'a History,
    selection: &Selection,
) -> Vec<std::borrow::Cow<'a, crate::model::Track>> {
    use std::borrow::Cow;
    if let Selection::Process(key) = selection {
        let key = history.resolve_key(*key);
        if let Some(track) = history.tracks.get(&key) {
            vec![Cow::Borrowed(track)]
        } else {
            history
                .current_track(key)
                .map(Cow::Owned)
                .into_iter()
                .collect()
        }
    } else {
        all_tracks(history)
            .into_iter()
            .filter(|track| history.matches(track, selection))
            .collect()
    }
}

fn retained_span(
    history: &History,
    selection: &Selection,
    tracks: &[std::borrow::Cow<'_, crate::model::Track>],
) -> (u32, u32) {
    let key = match selection {
        Selection::Name(key) | Selection::App(key) => Some(key.to_lowercase()),
        Selection::Family(_) => Some(String::new()),
        _ => None,
    };
    let group_span = key.and_then(|key| {
        let mut samples = history.system.iter().filter(|frame| match selection {
            Selection::Name(_) => frame.groups.contains_key(&key),
            Selection::App(_) => frame.paths.contains_key(&key),
            Selection::Family(key) => frame
                .families
                .keys()
                .any(|family| history.resolve_key(*family) == history.resolve_key(*key)),
            _ => unreachable!(),
        });
        let first = samples.next()?.at;
        Some((first, samples.next_back().map_or(first, |frame| frame.at)))
    });
    let cached = history.selected_identity_for(selection);
    let first = group_span
        .map(|span| span.0)
        .unwrap_or_else(|| {
            tracks
                .iter()
                .map(|track| track.first_observed)
                .min()
                .or_else(|| {
                    cached.and_then(|identity| identity.last_leaf_point.map(|point| point.at))
                })
                .unwrap_or(history.at)
        })
        .max(history.times.front().copied().unwrap_or(history.at));
    let end = if cached.map_or_else(
        || tracks.iter().all(|track| track.exited_at.is_some()),
        |identity| identity.exited_at.is_some(),
    ) {
        group_span.map(|span| span.1).unwrap_or_else(|| {
            tracks
                .iter()
                .map(|track| track.last_observed)
                .max()
                .or_else(|| cached.map(|identity| identity.last_observed))
                .unwrap_or(history.at)
        })
    } else {
        history.at
    };
    (first, end)
}

fn reading_at(history: &History, selection: &Selection, metric: Metric, at: u32) -> Reading {
    if matches!(selection, Selection::Process(_))
        && let Some(cached) = history.selected_identity_for(selection)
        && let Some(point) = cached.last_leaf_point.filter(|point| point.at == at)
        && (at < history.at || cached.exited_at.is_none())
    {
        return Reading {
            value: Some(match metric {
                Metric::PrivateWorkingSet => point.private_bytes,
                Metric::PrivateCommit => point.commit_bytes,
            }),
            measured: 1,
            total: 1,
            ..Default::default()
        };
    }
    if matches!(
        selection,
        Selection::Name(_) | Selection::App(_) | Selection::Family(_)
    ) && let Ok(index) = history.system.binary_search_by_key(&at, |frame| frame.at)
        && history.system[index].complete_scan
    {
        let frame = &history.system[index];
        let group = match selection {
            Selection::Name(name) => frame.groups.get(&name.to_lowercase()),
            Selection::App(path) => frame.paths.get(&path.to_lowercase()),
            Selection::Family(key) => {
                frame.families.get(&history.resolve_key(*key)).or_else(|| {
                    frame
                        .families
                        .iter()
                        .find(|(family, _)| {
                            history.resolve_key(**family) == history.resolve_key(*key)
                        })
                        .map(|(_, group)| group)
                })
            }
            _ => unreachable!(),
        };
        if let Some(group) = group {
            return Reading {
                value: (group.measured > 0).then_some(match metric {
                    Metric::PrivateWorkingSet => group.bytes,
                    Metric::PrivateCommit => group.commit,
                }),
                measured: group.measured,
                total: group.observed,
                reason: (group.measured < group.observed)
                    .then(|| "Some process measurements are unavailable".into()),
                enumeration_complete: true,
            };
        }
        return Reading {
            value: Some(0),
            ..Default::default()
        };
    }
    let mut result = Reading::default();
    for track in members(history, selection) {
        if at < track.first_observed {
            if at < history.at
                && !matches!(selection, Selection::Process(_))
                && !history.tracks.contains_key(&track.key)
            {
                result.total += 1;
                result.reason = Some("Detailed history omitted for some group members".into());
            }
            continue;
        }
        if track.exited_at.is_some_and(|exit| at >= exit) {
            continue;
        }
        result.total += 1;
        if let Ok(index) = track.points.binary_search_by_key(&at, |point| point.at) {
            let point = &track.points[index];
            let bytes = match metric {
                Metric::PrivateWorkingSet => point.private_bytes,
                Metric::PrivateCommit => point.commit_bytes,
            };
            result.value = Some(result.value.unwrap_or(0).saturating_add(bytes));
            result.measured += 1;
        } else {
            result.reason = Some(
                track
                    .unavailable
                    .iter()
                    .find(|(time, _)| *time == at)
                    .map(|(_, reason)| reason.clone())
                    .unwrap_or_else(|| "Measurement unavailable".into()),
            );
        }
    }
    if result.total == 0 {
        if !matches!(selection, Selection::Process(_))
            && history.system.iter().any(|s| s.at == at && s.complete_scan)
        {
            result.value = Some(0);
        } else {
            result.reason = Some("No observation at this time".into());
        }
    }
    if !matches!(selection, Selection::Process(_))
        && history
            .system
            .iter()
            .any(|s| s.at == at && !s.complete_scan)
    {
        result.enumeration_complete = false;
        result.reason =
            Some("Process enumeration incomplete; coverage includes only known members".into());
    }
    result
}

fn calculations(
    history: &History,
    selection: &Selection,
    seconds: u32,
) -> (Reading, ChangeReading) {
    let mut peak = Reading::default();
    for at in history
        .times
        .iter()
        .copied()
        .filter(|at| *at >= history.at.saturating_sub(crate::model::HISTORY_SECONDS))
    {
        let value = reading_at(history, selection, Metric::PrivateWorkingSet, at);
        if value
            .value
            .is_some_and(|v| peak.value.is_none_or(|p| v > p))
        {
            peak = value;
        }
    }
    let (first, end) = retained_span(history, selection, &members(history, selection));
    peak.reason = Some(format!(
        "Observed private working set peak; {} seconds recorded of a 30-minute window",
        end.saturating_sub(first)
    ));
    let unavailable = |reason: String| ChangeReading {
        value: None,
        reason: Some(reason),
    };
    let need = || {
        unavailable(if seconds == 300 {
            "Need five minutes of history".into()
        } else {
            format!("Need {} seconds of history", seconds)
        })
    };
    let Some(target) = history.at.checked_sub(seconds) else {
        return (peak, need());
    };
    if history.times.front().is_none_or(|at| *at > target) {
        return (peak, need());
    }
    let Some((baseline_at, baseline)) = history
        .times
        .iter()
        .rev()
        .copied()
        .filter(|at| *at <= target)
        .take_while(|at| target.saturating_sub(*at) <= history.interval.max(2) * 2)
        .map(|at| {
            (
                at,
                reading_at(history, selection, Metric::PrivateWorkingSet, at),
            )
        })
        .find(|(_, value)| value.value.is_some() && !value.partial())
    else {
        return (
            peak,
            unavailable(
                "No complete valid baseline within two collection intervals before the target"
                    .into(),
            ),
        );
    };
    let current = reading_at(history, selection, Metric::PrivateWorkingSet, history.at);
    if current.value.is_none()
        || baseline.value.is_none()
        || current.partial()
        || baseline.partial()
    {
        return (
            peak,
            unavailable("Change unavailable: incomplete endpoint measurement coverage".into()),
        );
    }
    let change = i128::from(current.value.unwrap()) - i128::from(baseline.value.unwrap());
    (
        peak,
        ChangeReading {
            value: Some(change.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64),
            reason: Some(format!(
                "Baseline: last valid sample at or before target, at {baseline_at}s (maximum age: two collection intervals)"
            )),
        },
    )
}

pub fn summary(
    history: &History,
    selection: &Selection,
    change_seconds: u32,
) -> Option<EntitySummary> {
    let tracks = members(history, selection);
    let cached = history.selected_identity_for(selection);
    if tracks.is_empty() && cached.is_none() {
        return None;
    }
    let first = tracks.first();
    let live: Vec<_> = tracks.iter().filter(|t| t.exited_at.is_none()).collect();
    let lifecycle = if cached.is_some_and(|identity| identity.exited_at.is_some())
        || cached.is_none() && live.is_empty()
    {
        Lifecycle::Exited
    } else if live.iter().any(|t| t.current.is_some()) {
        Lifecycle::Live
    } else {
        Lifecycle::Unavailable
    };
    let identity_tracks: Vec<_> = tracks
        .iter()
        .filter(|t| live.is_empty() || t.exited_at.is_none())
        .map(|t| t.as_ref())
        .collect();
    let mut paths: std::collections::BTreeMap<_, _> = identity_tracks
        .iter()
        .filter(|t| !t.display_image.is_empty())
        .map(|t| (t.image.clone(), t.display_image.clone()))
        .collect();
    if let Some(cached) = cached {
        paths = cached.paths.clone();
    }
    let mut private = reading_at(history, selection, Metric::PrivateWorkingSet, history.at);
    let mut commit = reading_at(history, selection, Metric::PrivateCommit, history.at);
    if lifecycle == Lifecycle::Exited {
        private.value = None;
        private.reason = Some("Exited; values in history are historical".into());
        commit.value = None;
        commit.reason = private.reason.clone();
    }
    let (peak, change) = calculations(history, selection, change_seconds);
    let (recorded_first, recorded_end) = retained_span(history, selection, &tracks);
    let count = if let Some(cached) = cached {
        cached.count
    } else if lifecycle == Lifecycle::Exited {
        tracks.len()
    } else if matches!(selection, Selection::Name(_) | Selection::App(_)) {
        private.total
    } else {
        live.len()
    };
    Some(EntitySummary {
        selection: selection.clone(),
        name: if let Some(cached) = cached {
            cached.name.clone()
        } else if let Selection::Family(key) = selection {
            history
                .tracks
                .get(key)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| first.map(|track| track.name.clone()).unwrap_or_default())
        } else {
            first.map(|track| track.name.clone()).unwrap_or_default()
        },
        paths: paths.values().cloned().collect(),
        pid: if let Selection::Process(key) = selection {
            Some(key.pid)
        } else {
            None
        },
        count,
        path_count: paths.len(),
        paths_complete: cached.map_or_else(
            || {
                identity_tracks
                    .iter()
                    .all(|track| !track.display_image.is_empty())
                    && identity_tracks.len() == count
            },
            |cached| cached.paths_complete,
        ),
        lifecycle,
        exited_at: if lifecycle == Lifecycle::Exited {
            cached
                .and_then(|cached| cached.exited_at)
                .or_else(|| tracks.iter().filter_map(|t| t.exited_at).max())
        } else {
            None
        },
        last_observed: cached
            .map(|cached| cached.last_observed)
            .or_else(|| tracks.iter().map(|t| t.last_observed).max()),
        recorded: recorded_end.saturating_sub(recorded_first),
        private,
        commit,
        peak,
        change,
        members: tracks.iter().map(|t| t.key).collect(),
    })
}

pub fn series(
    history: &History,
    selection: &Selection,
    metric: Metric,
    range: Range,
) -> HistorySeries {
    let tracks = members(history, selection);
    let (mut first, mut end) = retained_span(history, selection, &tracks);
    let cached_point = matches!(selection, Selection::Process(_))
        .then(|| history.selected_identity_for(selection))
        .flatten()
        .and_then(|identity| identity.last_leaf_point);
    if let Some(point) = cached_point {
        first = first.min(point.at);
        end = end.max(point.at);
    }
    let start = match range {
        Range::Last(seconds) => end.saturating_sub(seconds).max(first),
        Range::All => first,
    };
    let mut times: std::collections::BTreeSet<_> = history.times.iter().copied().collect();
    if let Some(point) = cached_point {
        times.insert(point.at);
    }
    let points: Vec<_> = times
        .into_iter()
        .filter(|at| *at >= start && *at <= end)
        .map(|at| HistoryValue {
            at,
            reading: reading_at(history, selection, metric, at),
        })
        .collect();
    let current = points.last().map(|p| p.reading.clone()).unwrap_or_default();
    HistorySeries {
        start,
        end,
        recorded: end.saturating_sub(first),
        points,
        current,
    }
}
pub fn overview(history: &History, range: Range) -> OverviewPlot {
    let end = history.at;
    let first = history.system.front().map(|s| s.at).unwrap_or(end);
    let start = match range {
        Range::Last(seconds) => end.saturating_sub(seconds).max(first),
        Range::All => first,
    };
    let mut layers = vec![
        OverviewLayer {
            name: "Other used RAM".into(),
            selection: None,
        },
        OverviewLayer {
            name: "Other processes".into(),
            selection: None,
        },
    ];
    for name in &history.named_bands {
        let label = history
            .tracks
            .values()
            .find(|t| t.name.to_lowercase() == *name)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| name.clone());
        layers.push(OverviewLayer {
            name: label,
            selection: Some(Selection::Name(name.clone())),
        });
    }
    let samples=history.system.iter().filter(|s|s.at>=start).map(|s| {
        let system_valid=s.error.is_none() && s.total>0 && s.available<=s.total;
        let used=system_valid.then(||s.total-s.available);
        let mut reason=s.error.clone();
        if !system_valid && reason.is_none() {reason=Some("System physical memory reading unavailable".into());}
        let mut thicknesses=None;
        if let Some(used)=used {
            if s.measured_private>used {reason=Some("Decomposition unavailable: measured private working sets exceed system used RAM".into());}
            else if !s.complete_scan {reason=Some("Decomposition unavailable: process enumeration incomplete".into());}
            else if history.named_bands.iter().any(|name|s.groups.get(name).is_some_and(|g|g.measured<g.observed)) {
                reason=Some("Attribution gap: a named executable group has unavailable measurements".into());
            } else {
                let named:Vec<_>=history.named_bands.iter().map(|name|s.groups.get(name).map(|g|g.bytes).unwrap_or(0)).collect();
                let named_total=named.iter().copied().sum::<u64>();
                let mut values=vec![used-s.measured_private,s.measured_private-named_total];
                values.extend(named);thicknesses=Some(values);
                if s.measured<s.observed {reason=Some(format!("{} of {} processes measured; unavailable allocations remain in Other used RAM",s.measured,s.observed));}
            }
        }
        OverviewSample{at:s.at,capacity:s.total,available:system_valid.then_some(s.available),used,layers:thicknesses,measured:s.measured,observed:s.observed,reason}
    }).collect();
    OverviewPlot {
        start,
        end,
        recorded: end.saturating_sub(first),
        capacity: history
            .system
            .iter()
            .rev()
            .find(|s| s.error.is_none() && s.total > 0)
            .map(|s| s.total)
            .unwrap_or(0),
        layers,
        samples,
    }
}
pub fn diagnostics(history: &History, own_pid: u32) -> Diagnostics {
    let latest = history.system.back();
    let tracks = all_tracks(history);
    let own = tracks
        .iter()
        .find(|t| t.key.pid == own_pid && t.exited_at.is_none() && t.current.is_some());
    Diagnostics {
        observed_session: history.observed_session.len(),
        current_observed: latest.map_or(0, |s| s.observed),
        current_measured: latest.map_or(0, |s| s.measured),
        current_enumeration_complete: latest.is_some_and(|s| s.complete_scan),
        current_unavailable: latest.map_or(0, |s| s.observed.saturating_sub(s.measured)),
        retained_snapshots: history.times.len(),
        omitted_tracks: history.omitted,
        last_success: history
            .system
            .iter()
            .rev()
            .find(|s| s.error.is_none() && s.complete_scan && s.total > 0)
            .map(|s| s.at),
        failures: history
            .failures
            .iter()
            .map(|(at, error)| format!("{at}s: {error}"))
            .collect(),
        own_private: own.and_then(|t| t.current.map(|p| p.private_bytes)),
        own_working_set: own.map(|t| t.working_set),
    }
}
/// One row per matching process instance at the displayed snapshot; never aggregates.
pub fn export_snapshot(
    history: &History,
    grouping: Grouping,
    show_exited: bool,
    query: &str,
    mut out: impl Write,
) -> io::Result<()> {
    out.write_all(b"elapsed_seconds,pid,creation_filetime,name,image,private_ram_kib,private_commit_kib,private_working_set_bytes,private_commit_bytes,snapshot_unix_ms,state,measurement_reason,provisional_generation,history_omitted\r\n")?;
    let keys: std::collections::BTreeSet<_> =
        tree_rows(history, grouping, show_exited, query, 300, Sort::default())
            .into_iter()
            .filter_map(|r| {
                if let Selection::Process(key) = r.selection {
                    Some(key)
                } else {
                    None
                }
            })
            .collect();
    let timestamp = history
        .system
        .back()
        .map(|s| s.captured_unix_ms)
        .unwrap_or(0);
    for key in keys {
        let tracks = members(history, &Selection::Process(key));
        let Some(track) = tracks.first() else {
            continue;
        };
        let name = track.name.replace('"', "\"\"");
        let image = track.display_image.replace('"', "\"\"");
        write!(out, "{},{},", history.at, key.pid)?;
        if key.generation == 0 {
            write!(out, "{}", key.created)?;
        }
        write!(out, ",\"{name}\",\"{image}\",")?;
        if let Some(point) = track.current {
            write!(
                out,
                "{},{},{},{},",
                point.private_bytes / 1024,
                point.commit_bytes / 1024,
                point.private_bytes,
                point.commit_bytes
            )?;
        } else {
            out.write_all(b",,,,")?;
        }
        let state = if track.exited_at.is_some() {
            "Exited"
        } else if track.current.is_some() {
            "Live"
        } else {
            "Unavailable"
        };
        let reason = reading_at(
            history,
            &Selection::Process(key),
            Metric::PrivateWorkingSet,
            history.at,
        )
        .reason
        .unwrap_or_default()
        .replace('"', "\"\"");
        writeln!(
            out,
            "{timestamp},{state},\"{reason}\",{},{}\r",
            key.generation,
            !history.tracks.contains_key(&key)
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProcessObservation, ProcessSample, Snapshot};
    fn process(pid: u32, name: &str, private: u64) -> ProcessSample {
        ProcessSample {
            key: ProcessKey {
                pid,
                created: u64::from(pid),
                generation: 0,
            },
            name: name.into(),
            image: format!("C:\\Apps\\{name}"),
            private,
            commit: private * 6,
            working_set: private * 2,
            parent_pid: 0,
        }
    }
    fn frame(at: u32, processes: Vec<ProcessSample>) -> Snapshot {
        Snapshot {
            at,
            total: 32 << 30,
            available: 8 << 30,
            processes,
            captured_unix_ms: 1_000_000 + u64::from(at) * 1000,
            ..Default::default()
        }
    }
    #[test]
    fn failed_measurement_stays_unavailable_until_complete_enumeration_confirms_exit() {
        let mut h = History::default();
        let p = process(4, "Worker.exe", 1024);
        let selected = Selection::Process(p.key);
        h.observe(frame(0, vec![p.clone()]));
        let mut failed = frame(2, vec![]);
        failed.observed.push(ProcessObservation {
            pid: 4,
            key: Some(p.key),
            name: p.name,
            image: Some(p.image),
            parent_pid: 0,
            error: Some("Access denied".into()),
        });
        h.observe(failed);
        let current = summary(&h, &selected, 60).unwrap();
        assert_eq!(current.lifecycle, Lifecycle::Unavailable);
        assert_eq!(current.private.value, None);
        assert_eq!(current.exited_at, None);
        h.observe(frame(4, vec![]));
        let exited = summary(&h, &selected, 60).unwrap();
        assert_eq!(exited.lifecycle, Lifecycle::Exited);
        assert_eq!(exited.exited_at, Some(4));
        assert_eq!(
            exited.private.value, None,
            "Historical values cannot become current allocation"
        );
        assert_eq!(
            series(&h, &selected, Metric::PrivateWorkingSet, Range::All).points[0]
                .reading
                .value,
            Some(1024)
        );
    }
    #[test]
    fn change_requires_the_full_requested_interval_and_peak_uses_simultaneous_group_totals() {
        let mut h = History::default();
        h.observe(frame(
            0,
            vec![process(1, "Worker.exe", 100), process(2, "Worker.exe", 300)],
        ));
        h.observe(frame(
            2,
            vec![process(1, "Worker.exe", 300), process(2, "Worker.exe", 100)],
        ));
        for at in (4..=180).step_by(2) {
            h.observe(frame(
                at,
                vec![process(1, "Worker.exe", 150), process(2, "Worker.exe", 200)],
            ));
        }
        let selected = Selection::Name("worker.exe".into());
        let early = summary(&h, &selected, 300).unwrap();
        assert_eq!(early.change.value, None);
        assert!(early.change.reason.unwrap().contains("five minutes"));
        assert_eq!(
            early.peak.value,
            Some(400),
            "Independent peaks total 600 but observed group peak is 400"
        );
        for at in (182..=300).step_by(2) {
            h.observe(frame(at, vec![process(1, "Worker.exe", 160)]));
        }
        let complete = summary(&h, &selected, 300).unwrap();
        assert_eq!(
            complete.change.value,
            Some(-240),
            "Confirmed exits participate in group net change"
        );
        h.observe(frame(302, vec![process(1, "Worker.exe", 170)]));
        assert_eq!(
            summary(&h, &selected, 301).unwrap().change.value,
            Some(-230),
            "Baseline is before target, never after it"
        );
        let mut failed = frame(304, vec![]);
        failed.observed.push(ProcessObservation {
            pid: 1,
            key: Some(ProcessKey {
                pid: 1,
                created: 1,
                generation: 0,
            }),
            name: "Worker.exe".into(),
            image: None,
            parent_pid: 0,
            error: Some("Denied".into()),
        });
        h.observe(failed);
        assert_eq!(
            summary(&h, &selected, 300).unwrap().change.value,
            None,
            "Unknown endpoint cannot appear as a release"
        );
    }
    #[test]
    fn overview_uses_unique_private_resident_bytes_and_retains_system_total_when_attribution_fails()
    {
        let gib = 1u64 << 30;
        let mut h = History::default();
        let a = process(1, "A.exe", 3 * gib);
        let b = process(2, "B.exe", 2 * gib);
        let c = process(3, "C.exe", gib);
        let d = process(4, "D.exe", gib);
        let e = process(5, "E.exe", gib);
        let f = process(6, "F.exe", gib);
        h.observe(frame(0, vec![a.clone(), a.clone(), b, c, d, e, f]));
        let plot = overview(&h, Range::All);
        assert_eq!(plot.capacity, 32 * gib);
        assert_eq!(plot.samples[0].used, Some(24 * gib));
        assert_eq!(plot.samples[0].available, Some(8 * gib));
        assert_eq!(
            plot.samples[0].layers,
            Some(vec![15 * gib, 3 * gib, 3 * gib, 2 * gib, gib])
        );
        assert_eq!(
            plot.samples[0].measured, 6,
            "Duplicate process identity contributes once"
        );
        let pinned: Vec<_> = plot.layers.iter().map(|l| l.name.clone()).collect();
        let mut coverage = h.clone();
        let mut unavailable = frame(
            2,
            vec![
                process(2, "B.exe", 2 * gib),
                process(3, "C.exe", gib),
                process(4, "D.exe", gib),
                process(5, "E.exe", gib),
                process(6, "F.exe", gib),
            ],
        );
        unavailable.observed.push(ProcessObservation {
            pid: a.key.pid,
            key: Some(a.key),
            name: a.name.clone(),
            image: Some(a.image.clone()),
            parent_pid: a.parent_pid,
            error: Some("Private working set unavailable".into()),
        });
        coverage.observe(unavailable);
        let unknown = overview(&coverage, Range::All);
        let unknown = unknown.samples.last().unwrap();
        assert_eq!(unknown.used, Some(24 * gib));
        assert_eq!(unknown.available, Some(8 * gib));
        assert_eq!(
            unknown.layers, None,
            "A failed named-band measurement preserves the system total as neutral attribution, rather than fabricating a release"
        );
        assert_eq!((unknown.measured, unknown.observed), (5, 6));
        assert!(
            unknown
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("unavailable")),
            "The neutral attribution explains unavailable named-band coverage"
        );
        let mut incomplete = frame(4, vec![a.clone()]);
        incomplete.complete_scan = false;
        incomplete.collection_error = Some("Process enumeration interrupted".into());
        coverage.observe(incomplete);
        let partial = overview(&coverage, Range::All);
        let partial = partial.samples.last().unwrap();
        assert_eq!(partial.used, Some(24 * gib));
        assert_eq!(partial.available, Some(8 * gib));
        assert_eq!(
            partial.layers, None,
            "Incomplete enumeration cannot attribute valid system RAM to an apparently complete set of processes"
        );
        assert_eq!((partial.measured, partial.observed), (1, 1));
        assert!(
            partial
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("incomplete"))
        );
        h.observe(frame(2, vec![process(9, "New.exe", 20 * gib)]));
        assert_eq!(
            overview(&h, Range::All)
                .layers
                .iter()
                .map(|l| l.name.clone())
                .collect::<Vec<_>>(),
            pinned
        );
        h.observe(frame(4, vec![process(9, "New.exe", 30 * gib)]));
        let inconsistent = overview(&h, Range::All).samples.last().unwrap().clone();
        assert_eq!(inconsistent.used, Some(24 * gib));
        assert_eq!(
            inconsistent.layers, None,
            "Never clamp or rescale inconsistent measured allocation"
        );
        let mut failed = frame(6, vec![a]);
        failed.system_error = Some("System memory unavailable".into());
        h.observe(failed);
        assert_eq!(
            overview(&h, Range::All).samples.last().unwrap().used,
            None,
            "Failed system collection creates a gap"
        );
    }
    #[test]
    fn filtered_hierarchy_keeps_ancestors_full_totals_and_real_process_tree_own_values() {
        let mut h = History::default();
        let a = process(1, "Worker.exe", 100);
        let mut b = process(2, "Worker.exe", 300);
        b.image = "D:\\Other\\Worker.exe".into();
        b.parent_pid = 1;
        let mut c = process(3, "Child.exe", 20);
        c.parent_pid = 2;
        h.observe(frame(0, vec![a, b, c]));
        let rows = tree_rows(&h, Grouping::Name, false, "D:\\Other", 300, Sort::default());
        assert_eq!(
            rows.len(),
            3,
            "Name group, matching path and matching instance remain visible"
        );
        assert_eq!(rows[0].private.value, Some(400));
        assert_eq!((rows[0].matching_count, rows[0].count), (1, 2));
        assert_eq!(rows[1].parent, Some(rows[0].selection.clone()));
        assert_eq!(rows[2].pid, Some(2));
        let tree = tree_rows(&h, Grouping::Tree, false, "3", 300, Sort::default());
        assert_eq!(
            tree.iter().map(|r| r.pid).collect::<Vec<_>>(),
            vec![Some(1), Some(2), Some(3)]
        );
        assert_eq!(
            tree.iter().map(|r| r.private.value).collect::<Vec<_>>(),
            vec![Some(100), Some(300), Some(20)]
        );
        assert_eq!(
            tree.iter().map(|r| r.depth).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(
            tree_rows(
                &h,
                Grouping::Name,
                false,
                "no such executable",
                300,
                Sort::default()
            )
            .is_empty()
        );
        assert_eq!(
            overview(&h, Range::All).samples[0].used,
            Some(24 << 30),
            "Filtering cannot change system accounting"
        );
    }
    #[test]
    fn csv_exports_one_displayed_filtered_instance_with_exact_bytes_and_original_full_path() {
        let mut h = History::default();
        let mut p = process(7, "say\"hi,there.exe", 12345);
        p.image = "C:\\Program Files\\Long\\say\"hi,there.exe".into();
        h.observe(frame(0, vec![p.clone(), process(8, "other.exe", 900)]));
        let frozen = h.clone();
        h.observe(frame(2, vec![process(7, "say\"hi,there.exe", 45678)]));
        let mut csv = Vec::new();
        export_snapshot(&frozen, Grouping::Name, false, "12345", &mut csv).unwrap();
        assert_eq!(
            String::from_utf8(csv).unwrap().lines().count(),
            1,
            "Filter does not match arbitrary memory values"
        );
        let mut csv = Vec::new();
        export_snapshot(&frozen, Grouping::Name, false, "7", &mut csv).unwrap();
        let text = String::from_utf8(csv).unwrap();
        assert_eq!(
            text.lines().count(),
            2,
            "Aggregate rows cannot duplicate instance values"
        );
        assert!(text.contains("0,7,7,\"say\"\"hi,there.exe\",\"C:\\Program Files\\Long\\say\"\"hi,there.exe\",12,72,12345,74070,1000000,Live"));
        let diagnostic = diagnostics(&h, 7);
        assert_eq!(diagnostic.observed_session, 2);
        assert_eq!(diagnostic.current_unavailable, 0);
        assert_eq!(diagnostic.retained_snapshots, 2);
        assert_eq!(diagnostic.own_private, Some(45678));
    }
    #[test]
    fn change_uses_a_recent_valid_baseline_and_exit_history_never_resurrects_on_unknown_pid_reuse()
    {
        let mut h = History::default();
        let selected = Selection::Name("worker.exe".into());
        h.observe(frame(0, vec![process(7, "Worker.exe", 100)]));
        h.observe(frame(2, vec![process(7, "Worker.exe", 150)]));
        let mut failed = frame(4, vec![]);
        failed.observed.push(ProcessObservation {
            pid: 7,
            key: Some(ProcessKey {
                pid: 7,
                created: 7,
                generation: 0,
            }),
            name: "Worker.exe".into(),
            image: None,
            parent_pid: 0,
            error: Some("Denied".into()),
        });
        h.observe(failed);
        h.observe(frame(304, vec![process(7, "Worker.exe", 300)]));
        assert_eq!(
            summary(&h, &selected, 300).unwrap().change.value,
            Some(150),
            "A valid baseline two seconds before the target is usable"
        );
        h.observe(frame(306, vec![]));
        let old = Selection::Process(ProcessKey {
            pid: 7,
            created: 7,
            generation: 0,
        });
        assert_eq!(
            summary(&h, &old, 300).unwrap().recorded,
            304,
            "Retained process history ends at last observation"
        );
        let mut unknown = frame(308, vec![]);
        unknown.observed.push(ProcessObservation {
            pid: 7,
            key: None,
            name: "Other.exe".into(),
            image: None,
            parent_pid: 0,
            error: Some("Access denied".into()),
        });
        h.observe(unknown);
        assert_eq!(summary(&h, &old, 300).unwrap().lifecycle, Lifecycle::Exited);
        assert_eq!(summary(&h, &old, 300).unwrap().exited_at, Some(306));
        let mut restarted = process(7, "Other.exe", 40);
        restarted.key.created = (11_644_473_600_000u64 + 1_307_000) * 10_000;
        h.observe(frame(310, vec![restarted]));
        assert_eq!(summary(&h, &old, 300).unwrap().lifecycle, Lifecycle::Exited);
        assert_eq!(diagnostics(&h, 7).observed_session, 2);
    }
    #[test]
    fn full_scan_totals_and_counts_survive_bounded_track_retention() {
        let mut h = History::default();
        let count = crate::model::MAX_TRACKS + 30;
        h.observe(frame(
            0,
            (1..=count as u32)
                .map(|pid| process(pid, "Many.exe", 1025))
                .collect(),
        ));
        let selected = Selection::Name("many.exe".into());
        let value = summary(&h, &selected, 300).unwrap();
        assert_eq!(value.private.value, Some(count as u64 * 1025));
        assert_eq!(value.count, count);
        assert_eq!(
            (value.private.measured, value.private.total),
            (count, count)
        );
        let path = summary(&h, &Selection::App("c:\\apps\\many.exe".into()), 300).unwrap();
        assert_eq!(path.private.value, Some(count as u64 * 1025));
        assert_eq!(path.count, count);
        assert_eq!(
            overview(&h, Range::All).samples[0]
                .layers
                .as_ref()
                .unwrap()
                .last(),
            Some(&(count as u64 * 1025))
        );
        assert_eq!(diagnostics(&h, 0).omitted_tracks, 30);
    }
    #[test]
    fn current_table_and_csv_include_an_entire_group_outside_the_history_track_cap() {
        let mut h = History::default();
        let limit = crate::model::MAX_TRACKS as u32;
        let mut all: Vec<_> = (1..=limit)
            .map(|pid| process(pid, "Large.exe", 1000))
            .collect();
        all.extend((limit + 1..=limit + 40).map(|pid| process(pid, "Small.exe", 10)));
        h.observe(frame(0, all.clone()));
        let small = summary(&h, &Selection::Name("small.exe".into()), 300)
            .expect("Current group remains available outside history cap");
        assert_eq!((small.count, small.private.value), (40, Some(400)));
        let rows = tree_rows(&h, Grouping::Name, false, "Small.exe", 300, Sort::default());
        assert_eq!(rows.len(), 41);
        assert_eq!(rows[0].matching_count, 40);
        let mut csv = Vec::new();
        export_snapshot(&h, Grouping::Name, false, "Small.exe", &mut csv).unwrap();
        assert_eq!(String::from_utf8(csv).unwrap().lines().count(), 41);
        assert_eq!(
            series(
                &h,
                &Selection::Process(ProcessKey {
                    pid: limit + 40,
                    created: u64::from(limit + 40),
                    generation: 0
                }),
                Metric::PrivateWorkingSet,
                Range::All
            )
            .recorded,
            0
        );
        for at in [2, 4] {
            let mut next = all.clone();
            for process in next.iter_mut().filter(|p| p.name == "Small.exe") {
                process.private += u64::from(at);
            }
            h.observe(frame(at, next));
        }
        for selected in [
            Selection::Name("small.exe".into()),
            Selection::App("c:\\apps\\small.exe".into()),
        ] {
            let values = series(&h, &selected, Metric::PrivateWorkingSet, Range::All);
            assert_eq!(
                values
                    .points
                    .iter()
                    .map(|p| (p.at, p.reading.value))
                    .collect::<Vec<_>>(),
                vec![(0, Some(400)), (2, Some(480)), (4, Some(560))],
                "Retained group samples remain inspectable independently of omitted leaf histories"
            );
            assert_eq!(values.recorded, 4);
            assert_eq!(summary(&h, &selected, 4).unwrap().recorded, 4);
        }
        for selected in [
            Selection::Name("small.exe".into()),
            Selection::App("c:\\apps\\small.exe".into()),
            Selection::Process(ProcessKey {
                pid: limit + 40,
                created: u64::from(limit + 40),
                generation: 0,
            }),
        ] {
            let mut watched = h.clone();
            watched.protect_selection(Some(&selected));
            watched.observe(frame(
                6,
                all.iter()
                    .filter(|p| p.name == "Large.exe")
                    .cloned()
                    .collect(),
            ));
            let exited = summary(&watched, &selected, 4)
                .expect("A selected outside-cap entity remains inspectable after its immediate next-sample exit");
            assert_eq!(exited.lifecycle, Lifecycle::Exited);
            assert_eq!(exited.exited_at, Some(6));
            assert_eq!(exited.private.value, None);
            assert_eq!(
                exited.count,
                if matches!(selected, Selection::Process(_)) {
                    1
                } else {
                    40
                }
            );
            assert_eq!(exited.paths, vec!["C:\\Apps\\Small.exe"]);
            let retained = series(&watched, &selected, Metric::PrivateWorkingSet, Range::All);
            assert_eq!(retained.end, 4);
            assert_eq!(
                retained.current.value,
                Some(if matches!(selected, Selection::Process(_)) {
                    14
                } else {
                    560
                })
            );
            assert!(watched.tracks.len() <= crate::model::MAX_TRACKS);
        }
    }
    #[test]
    fn selected_group_identity_keeps_every_original_path_and_member_after_exceeding_history_cap() {
        let limit = crate::model::MAX_TRACKS as u32;
        let mut all: Vec<_> = (1..=limit + 1)
            .map(|pid| {
                let mut p = process(pid, "Shared.exe", 1000);
                p.parent_pid = if pid == 1 { 0 } else { 1 };
                p
            })
            .collect();
        let last = all.last_mut().unwrap();
        last.private = 1;
        last.image = "D:\\Other\\Shared.exe".into();
        let root = all[0].key;
        for selected in [
            Selection::Name("shared.exe".into()),
            Selection::Family(root),
        ] {
            let mut h = History::default();
            h.observe(frame(0, all.clone()));
            h.observe(frame(2, all.clone()));
            h.protect_selection(Some(&selected));
            let mut unavailable = frame(3, vec![]);
            unavailable.complete_scan = false;
            h.observe(unavailable);
            let uncertain = summary(&h, &selected, 2).unwrap();
            assert_ne!(
                uncertain.lifecycle,
                Lifecycle::Exited,
                "An incomplete scan cannot establish selected-group exit"
            );
            assert_eq!(uncertain.exited_at, None);
            assert_eq!(uncertain.count, limit as usize + 1);
            assert_eq!(
                uncertain.paths,
                vec!["C:\\Apps\\Shared.exe", "D:\\Other\\Shared.exe"]
            );
            h.observe(frame(4, vec![]));
            let exited = summary(&h, &selected, 2)
                .expect("Selected identity outlives detailed-track retention");
            assert_eq!(exited.name, "Shared.exe");
            assert_eq!(exited.lifecycle, Lifecycle::Exited);
            assert_eq!(exited.exited_at, Some(4));
            assert_eq!((exited.count, exited.path_count), (limit as usize + 1, 2));
            assert_eq!(
                exited.paths,
                vec!["C:\\Apps\\Shared.exe", "D:\\Other\\Shared.exe"]
            );
            assert_eq!(exited.last_observed, Some(2));
            assert_eq!(exited.private.value, None);
            assert_eq!(exited.commit.value, None);
            let values = series(&h, &selected, Metric::PrivateWorkingSet, Range::All);
            assert_eq!(values.end, 2);
            assert_eq!(values.current.value, Some(u64::from(limit) * 1000 + 1));
            assert!(h.tracks.len() <= crate::model::MAX_TRACKS);
            assert!(
                h.tracks
                    .values()
                    .all(|track| track.points.len() <= crate::model::MAX_POINTS)
            );
        }
    }
    #[test]
    fn frozen_selection_recovers_omitted_leaf_identity_and_one_historical_point_after_live_exit() {
        let limit = crate::model::MAX_TRACKS as u32;
        let mut values: Vec<_> = (1..=limit)
            .map(|pid| process(pid, "Large.exe", 1000))
            .collect();
        let mut leaf = process(limit + 1, "Last.exe", 17);
        leaf.image = "D:\\Original Path\\Last.exe".into();
        let selected = Selection::Process(leaf.key);
        values.push(leaf);
        let mut live = History::default();
        live.observe(frame(0, values));
        let frozen = live.clone();
        live.observe(frame(2, vec![]));
        live.observe(frame(6, vec![]));
        live.protect_selection_from(&frozen, Some(&selected));
        for at in [6, 8] {
            if at > live.at {
                live.observe(frame(at, vec![]));
                live.protect_selection(Some(&selected));
            }
            live.protect_selection_from(&frozen, Some(&selected));
            let exited = summary(&live, &selected, 300)
                .expect("A paused selection still has its full displayed identity");
            assert_eq!(exited.name, "Last.exe");
            assert_eq!((exited.count, exited.path_count), (1, 1));
            assert_eq!(exited.paths, vec!["D:\\Original Path\\Last.exe"]);
            assert_eq!(exited.lifecycle, Lifecycle::Exited);
            assert_eq!(exited.exited_at, Some(6));
            assert_eq!(exited.last_observed, Some(0));
            assert_eq!(exited.private.value, None);
            assert_eq!(exited.commit.value, None);
            let history = series(&live, &selected, Metric::PrivateWorkingSet, Range::All);
            assert_eq!(
                history
                    .points
                    .iter()
                    .map(|p| (p.at, p.reading.value))
                    .collect::<Vec<_>>(),
                vec![(0, Some(17))]
            );
            assert_eq!(
                series(&live, &selected, Metric::PrivateCommit, Range::All)
                    .current
                    .value,
                Some(102)
            );
            assert!(live.tracks.len() <= crate::model::MAX_TRACKS);
        }
        live.protect_selection(None);
        assert!(
            summary(&live, &selected, 300).is_none(),
            "The lightweight cache belongs only to the current selection"
        );
    }
    #[test]
    fn selected_identity_updates_complete_paths_and_marks_unknown_path_coverage_explicitly() {
        let mut h = History::default();
        let selected = Selection::Name("worker.exe".into());
        let first = process(1, "Worker.exe", 10);
        let mut second = process(2, "Worker.exe", 20);
        second.image = "D:\\Old\\Worker.exe".into();
        h.observe(frame(0, vec![first.clone(), second]));
        h.protect_selection(Some(&selected));
        let mut replacement = process(3, "Worker.exe", 30);
        replacement.image = "E:\\New\\Worker.exe".into();
        h.observe(frame(2, vec![first.clone(), replacement.clone()]));
        let changed = summary(&h, &selected, 300).unwrap();
        assert!(changed.paths_complete);
        assert_eq!(
            changed.paths,
            vec!["C:\\Apps\\Worker.exe", "E:\\New\\Worker.exe"]
        );
        let mut failed = frame(4, vec![first, replacement]);
        failed.observed.push(ProcessObservation {
            pid: 4,
            key: Some(ProcessKey {
                pid: 4,
                created: 4,
                generation: 0,
            }),
            name: "Worker.exe".into(),
            image: None,
            parent_pid: 0,
            error: Some("Path and counters unavailable".into()),
        });
        h.observe(failed);
        let partial = summary(&h, &selected, 300).unwrap();
        assert_eq!(partial.count, 3);
        assert!(!partial.paths_complete);
        assert_eq!(partial.path_count, 2);
        h.observe(frame(6, vec![]));
        let exited = summary(&h, &selected, 300).unwrap();
        assert_eq!(exited.count, 3);
        assert!(!exited.paths_complete);
        assert_eq!(
            exited.paths,
            vec!["C:\\Apps\\Worker.exe", "E:\\New\\Worker.exe"]
        );
        h.protect_selection(None);
        let mut incomplete = frame(8, vec![process(8, "Worker.exe", 40)]);
        incomplete.complete_scan = false;
        h.observe(incomplete);
        h.protect_selection(Some(&selected));
        assert!(
            !summary(&h, &selected, 300).unwrap().paths_complete,
            "Known current paths cannot establish full path coverage when the first selected scan is incomplete"
        );
    }
    #[test]
    fn family_history_counts_omitted_members_and_their_exits_without_fabricating_growth() {
        let mut h = History::default();
        let all: Vec<_> = (1..=crate::model::MAX_TRACKS as u32 + 1)
            .map(|pid| {
                let mut p = process(pid, "Worker.exe", 100);
                if pid > 1 {
                    p.parent_pid = 1;
                }
                p
            })
            .collect();
        let selected = Selection::Family(all[0].key);
        h.observe(frame(0, all.clone()));
        h.observe(frame(2, all.clone()));
        assert_eq!(
            summary(&h, &selected, 2).unwrap().change.value,
            Some(0),
            "Omitted member history must use complete aggregate observations"
        );
        let values = series(&h, &selected, Metric::PrivateWorkingSet, Range::All);
        assert_eq!(values.points[0].reading.value, Some(102500));
        assert!(!values.points[0].reading.partial());
        assert!(!values.points.last().unwrap().reading.partial());
        h.observe(frame(
            4,
            all.into_iter().take(crate::model::MAX_TRACKS).collect(),
        ));
        assert_eq!(
            summary(&h, &selected, 4).unwrap().change.value,
            Some(-100),
            "An omitted member's confirmed exit remains in the family's net change"
        );
    }
    #[test]
    fn known_paths_survive_read_failures_and_incomplete_enumeration_never_makes_a_complete_aggregate()
     {
        let mut h = History::default();
        let p = process(7, "Worker.exe", 100);
        let path = p.image.clone();
        h.observe(frame(0, vec![p.clone()]));
        let mut failed = frame(2, vec![]);
        failed.observed.push(ProcessObservation {
            pid: 7,
            key: Some(p.key),
            name: p.name.clone(),
            image: None,
            parent_pid: 0,
            error: Some("Denied".into()),
        });
        h.observe(failed);
        let selected = Selection::App(path.to_lowercase());
        let missing = summary(&h, &selected, 2).unwrap();
        assert_eq!(missing.private.value, None);
        assert_eq!(missing.change.value, None);
        assert_eq!(missing.paths, vec![path.clone()]);
        let mut recovered = p.clone();
        recovered.image.clear();
        recovered.private = 150;
        h.observe(frame(4, vec![recovered]));
        assert_eq!(summary(&h, &selected, 2).unwrap().private.value, Some(150));
        assert_eq!(summary(&h, &selected, 2).unwrap().paths, vec![path]);
        let mut partial = frame(6, vec![p]);
        partial.complete_scan = false;
        h.observe(partial);
        let unknown = summary(&h, &Selection::Name("worker.exe".into()), 2).unwrap();
        assert!(
            unknown.private.partial(),
            "Enumeration failure means unknown group membership, even when all known members measured"
        );
        assert_eq!(unknown.change.value, None);
    }
    #[test]
    fn diagnostics_count_current_observed_failures_without_guessing_missing_enumeration_members() {
        let mut h = History::default();
        h.observe(frame(
            0,
            vec![process(7, "Worker.exe", 100), process(8, "Other.exe", 200)],
        ));
        let mut incomplete = frame(2, vec![process(7, "Worker.exe", 150)]);
        incomplete.complete_scan = false;
        incomplete.collection_error = Some("Process enumeration failed".into());
        h.observe(incomplete);
        let coverage = diagnostics(&h, 7);
        assert_eq!(
            coverage.current_unavailable, 0,
            "An unobserved process is unknown, not a current observed counter failure"
        );
        assert_eq!(
            (coverage.current_measured, coverage.current_observed),
            (1, 1)
        );
        assert!(!coverage.current_enumeration_complete);
        assert!(
            coverage
                .failures
                .iter()
                .any(|reason| reason.contains("Process enumeration failed"))
        );
        let mut denied = frame(4, vec![process(7, "Worker.exe", 160)]);
        denied.observed.push(ProcessObservation {
            pid: 8,
            key: Some(process(8, "Other.exe", 200).key),
            name: "Other.exe".into(),
            image: None,
            parent_pid: 0,
            error: Some("Access denied".into()),
        });
        h.observe(denied);
        let coverage = diagnostics(&h, 7);
        assert_eq!(coverage.current_unavailable, 1);
        assert_eq!(
            (coverage.current_measured, coverage.current_observed),
            (1, 2)
        );
        assert!(coverage.current_enumeration_complete);
    }
    #[test]
    fn provisional_lifetimes_are_distinct_after_absence_and_recovery_preserves_proven_identity() {
        let mut h = History::default();
        let denied = |at| {
            let mut s = frame(at, vec![]);
            s.observed.push(ProcessObservation {
                pid: 7,
                key: None,
                name: "Worker.exe".into(),
                image: None,
                parent_pid: 0,
                error: Some("Denied".into()),
            });
            s
        };
        h.observe(denied(0));
        let old = tree_rows(&h, Grouping::Process, false, "7", 300, Sort::default())[0]
            .selection
            .clone();
        h.observe(frame(2, vec![]));
        h.observe(denied(4));
        let current = tree_rows(&h, Grouping::Process, false, "7", 300, Sort::default());
        assert_eq!(current.len(), 1);
        let pending = current[0].selection.clone();
        h.protect_selection(Some(&pending));
        assert_ne!(pending, old);
        assert_eq!(summary(&h, &old, 300).unwrap().lifecycle, Lifecycle::Exited);
        let mut recovered = process(7, "Worker.exe", 100);
        recovered.key.created = (11_644_473_600_000u64 + 1_003_000) * 10_000;
        let real = Selection::Process(recovered.key);
        h.observe(frame(6, vec![recovered]));
        assert_eq!(h.resolve_selection(&pending), real);
        assert_eq!(summary(&h, &pending, 300).unwrap().private.value, Some(100));
        assert_eq!(
            summary(&h, &real, 300).unwrap().paths,
            vec!["C:\\Apps\\Worker.exe"]
        );
        h.observe(frame(8, vec![]));
        let exited = summary(&h, &pending, 300).unwrap();
        assert_eq!(exited.lifecycle, Lifecycle::Exited);
        assert_eq!(exited.exited_at, Some(8));
        assert_eq!(exited.last_observed, Some(6));
        assert_eq!(diagnostics(&h, 7).observed_session, 2);
    }
    #[test]
    fn unavailable_parent_identity_and_selected_exited_history_survive_collection_and_cap_pressure()
    {
        let mut h = History::default();
        let parent = process(10, "Editor.exe", 100);
        let mut child = process(20, "Child.exe", 50);
        child.parent_pid = 10;
        let mut s = frame(0, vec![child.clone()]);
        s.observed.push(ProcessObservation {
            pid: 10,
            key: Some(parent.key),
            name: parent.name,
            image: Some(parent.image),
            parent_pid: 0,
            error: Some("Counter denied".into()),
        });
        h.observe(s);
        assert_eq!(h.parent_target(child.key), Some(parent.key));
        let mut h = History::default();
        let limit = crate::model::MAX_TRACKS as u32;
        h.observe(frame(
            0,
            (1..=limit)
                .map(|pid| process(pid, "Many.exe", 100))
                .collect(),
        ));
        let selected = Selection::Process(ProcessKey {
            pid: 1,
            created: 1,
            generation: 0,
        });
        h.protect_selection(Some(&selected));
        let mut next: Vec<_> = (2..=limit)
            .map(|pid| process(pid, "Many.exe", 100))
            .collect();
        next.push(process(limit + 1, "New.exe", 200));
        h.observe(frame(2, next));
        let retained = summary(&h, &selected, 300)
            .expect("Selected exited history remains inspectable under pressure");
        assert_eq!(retained.lifecycle, Lifecycle::Exited);
        assert_eq!(
            series(&h, &selected, Metric::PrivateWorkingSet, Range::All).points[0]
                .reading
                .value,
            Some(100)
        );
        assert_eq!(diagnostics(&h, 0).omitted_tracks, 1);
    }
    #[test]
    fn observed_inaccessible_history_is_preserved_and_selected_omitted_process_is_admitted() {
        let limit = crate::model::MAX_TRACKS as u32;
        let mut h = History::default();
        h.observe(frame(
            0,
            (1..=limit)
                .map(|pid| process(pid, "Many.exe", 100))
                .collect(),
        ));
        let make = |at, newvalue| {
            let mut values: Vec<_> = (2..=limit)
                .map(|pid| process(pid, "Many.exe", 100))
                .collect();
            values.push(process(limit + 1, "New.exe", newvalue));
            let mut s = frame(at, values);
            s.observed.push(ProcessObservation {
                pid: 1,
                key: Some(ProcessKey {
                    pid: 1,
                    created: 1,
                    generation: 0,
                }),
                name: "Many.exe".into(),
                image: None,
                parent_pid: 0,
                error: Some("Denied".into()),
            });
            s
        };
        h.observe(make(2, 200));
        let inaccessible = Selection::Process(ProcessKey {
            pid: 1,
            created: 1,
            generation: 0,
        });
        assert_eq!(
            series(&h, &inaccessible, Metric::PrivateWorkingSet, Range::All).points[0]
                .reading
                .value,
            Some(100)
        );
        assert_eq!(
            summary(&h, &inaccessible, 300).unwrap().lifecycle,
            Lifecycle::Unavailable
        );
        let selected = Selection::Process(ProcessKey {
            pid: limit + 1,
            created: u64::from(limit + 1),
            generation: 0,
        });
        h.protect_selection(Some(&selected));
        h.observe(make(4, 210));
        h.observe(make(6, 240));
        let values = series(&h, &selected, Metric::PrivateWorkingSet, Range::All);
        assert_eq!(
            values
                .points
                .iter()
                .map(|p| (p.at, p.reading.value))
                .collect::<Vec<_>>(),
            vec![(2, Some(200)), (4, Some(210)), (6, Some(240))]
        );
        let mut failures = History::default();
        let mut s = frame(0, vec![]);
        s.observed = (1..=limit + 1)
            .map(|pid| ProcessObservation {
                pid,
                key: Some(ProcessKey {
                    pid,
                    created: u64::from(pid),
                    generation: 0,
                }),
                name: "Denied.exe".into(),
                image: None,
                parent_pid: 0,
                error: Some("Denied".into()),
            })
            .collect();
        failures.observe(s);
        assert_eq!(diagnostics(&failures, 0).omitted_tracks, 1);
        assert_eq!(
            tree_rows(
                &failures,
                Grouping::Process,
                false,
                "",
                300,
                Sort::default()
            )
            .len(),
            limit as usize + 1
        );
    }
}
