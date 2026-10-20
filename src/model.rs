use std::collections::{BTreeMap, VecDeque};

pub const HISTORY_SECONDS: u32 = 30 * 60;
pub const MAX_POINTS: usize = 901;
pub const MAX_TRACKS: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessKey {
    pub pid: u32,
    pub created: u64,
    /// Nonzero only for a locally observed lifetime whose creation time is unknown.
    pub generation: u32,
}

#[derive(Clone, Debug)]
pub struct ProcessSample {
    pub key: ProcessKey,
    pub name: String,
    pub image: String,
    pub private: u64,
    pub commit: u64,
    pub working_set: u64,
    pub parent_pid: u32,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub at: u32,
    pub total: u64,
    pub available: u64,
    pub inaccessible: usize,
    pub processes: Vec<ProcessSample>,
    /// Enumeration survives failed counter/path reads so inaccessible never means exited.
    pub observed: Vec<ProcessObservation>,
    pub complete_scan: bool,
    pub system_error: Option<String>,
    pub collection_error: Option<String>,
    pub captured_unix_ms: u64,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            at: 0,
            total: 0,
            available: 0,
            inaccessible: 0,
            processes: Vec::new(),
            observed: Vec::new(),
            complete_scan: true,
            system_error: None,
            collection_error: None,
            captured_unix_ms: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProcessObservation {
    pub pid: u32,
    pub key: Option<ProcessKey>,
    pub name: String,
    pub image: Option<String>,
    pub parent_pid: u32,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct GroupMeasurement {
    pub bytes: u64,
    pub commit: u64,
    pub measured: usize,
    pub observed: usize,
}

#[derive(Clone, Debug)]
pub struct SystemSample {
    pub at: u32,
    pub captured_unix_ms: u64,
    pub total: u64,
    pub available: u64,
    pub error: Option<String>,
    pub measured_private: u64,
    pub measured: usize,
    pub observed: usize,
    pub groups: BTreeMap<String, GroupMeasurement>,
    pub paths: BTreeMap<String, GroupMeasurement>,
    pub families: BTreeMap<ProcessKey, GroupMeasurement>,
    pub complete_scan: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub at: u32,
    pub private_bytes: u64,
    pub commit_bytes: u64,
}

#[derive(Clone)]
pub struct Track {
    pub key: ProcessKey,
    pub name: String,
    pub image: String,
    pub display_image: String,
    pub points: VecDeque<Point>,
    pub current: Option<Point>,
    pub family: ProcessKey,
    pub parent_pid: u32,
    pub working_set: u64,
    pub parent_name: Option<String>,
    pub parent_key: Option<ProcessKey>,
    pub first_observed: u32,
    pub first_observed_unix_ms: u64,
    pub last_observed: u32,
    pub exited_at: Option<u32>,
    pub unavailable: VecDeque<(u32, String)>,
}

impl Track {
    pub fn parent_description(&self) -> String {
        if self.parent_pid == 0 {
            "None".into()
        } else {
            format!(
                "{} (PID {})",
                self.parent_name.as_deref().unwrap_or("Unavailable"),
                self.parent_pid
            )
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Selection {
    Name(String),
    App(String),
    Family(ProcessKey),
    Process(ProcessKey),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grouping {
    Name,
    Executable,
    Family,
    Process,
    Tree,
}

/// One selected entity's identity survives bounded detailed-track retention.
/// Groups retain aggregate samples elsewhere; only a selected leaf keeps one point here.
#[derive(Clone, Debug)]
pub struct SelectedIdentity {
    pub selection: Selection,
    pub name: String,
    pub paths: BTreeMap<String, String>,
    pub paths_complete: bool,
    pub count: usize,
    pub last_observed: u32,
    pub exited_at: Option<u32>,
    pub last_leaf_point: Option<Point>,
}

#[derive(Clone, Default)]
pub struct History {
    pub tracks: BTreeMap<ProcessKey, Track>,
    pub times: VecDeque<u32>,
    pub at: u32,
    pub total: u64,
    pub available: u64,
    pub inaccessible: usize,
    pub omitted: usize,
    pub system: VecDeque<SystemSample>,
    pub named_bands: Vec<String>,
    pub observed_session: std::collections::BTreeSet<ProcessKey>,
    pub failures: VecDeque<(u32, String)>,
    pub interval: u32,
    pub latest: Snapshot,
    pub latest_families: BTreeMap<ProcessKey, ProcessKey>,
    pub key_aliases: BTreeMap<ProcessKey, ProcessKey>,
    pub next_generation: u32,
    pub protected_keys: std::collections::BTreeSet<ProcessKey>,
    pub provisional_first: BTreeMap<ProcessKey, (u32, u64)>,
    pub selected_identity: Option<SelectedIdentity>,
}

impl History {
    pub fn resolve_key(&self, mut key: ProcessKey) -> ProcessKey {
        while let Some(resolved) = self.key_aliases.get(&key) {
            key = *resolved;
        }
        key
    }
    pub fn resolve_selection(&self, selection: &Selection) -> Selection {
        match selection {
            Selection::Process(key) => Selection::Process(self.resolve_key(*key)),
            Selection::Family(key) => Selection::Family(self.resolve_key(*key)),
            other => other.clone(),
        }
    }
    pub fn selected_identity_for(&self, selection: &Selection) -> Option<&SelectedIdentity> {
        let resolved = self.resolve_selection(selection);
        self.selected_identity
            .as_ref()
            .filter(|cached| self.resolve_selection(&cached.selection) == resolved)
    }
    fn selection_identity(
        &self,
        selection: &Selection,
        previous: Option<&SelectedIdentity>,
    ) -> Option<SelectedIdentity> {
        let selection = self.resolve_selection(selection);
        let current: Vec<_> = self
            .latest
            .observed
            .iter()
            .filter_map(|observed| observed.key)
            .filter_map(|key| self.current_track(key))
            .filter(|track| self.matches(track, &selection))
            .collect();
        let historical: Vec<_> = self
            .tracks
            .values()
            .filter(|track| self.matches(track, &selection))
            .collect();
        let previous =
            previous.filter(|cached| self.resolve_selection(&cached.selection) == selection);
        let first = current.first().or_else(|| historical.first().copied());
        let mut cached = if let Some(previous) = previous {
            previous.clone()
        } else {
            let first = first?;
            let matching: Vec<_> = if current.is_empty() {
                historical.clone()
            } else {
                current.iter().collect()
            };
            let group = self
                .system
                .iter()
                .rev()
                .find_map(|sample| match &selection {
                    Selection::Name(name) => sample.groups.get(&name.to_lowercase()),
                    Selection::App(path) => sample.paths.get(&path.to_lowercase()),
                    Selection::Family(key) => sample
                        .families
                        .iter()
                        .find(|(family, _)| self.resolve_key(**family) == self.resolve_key(*key))
                        .map(|(_, value)| value),
                    Selection::Process(_) => None,
                });
            let count = if current.is_empty() {
                group.map_or(matching.len(), |group| group.observed)
            } else {
                current.len()
            };
            let paths: BTreeMap<_, _> = matching
                .iter()
                .filter(|track| !track.display_image.is_empty())
                .map(|track| (track.image.clone(), track.display_image.clone()))
                .collect();
            let name = if let Selection::Family(key) = &selection {
                self.tracks
                    .get(&self.resolve_key(*key))
                    .map(|track| track.name.clone())
                    .or_else(|| {
                        self.current_track(self.resolve_key(*key))
                            .map(|track| track.name)
                    })
                    .unwrap_or_else(|| first.name.clone())
            } else {
                first.name.clone()
            };
            SelectedIdentity {
                selection: selection.clone(),
                name,
                paths,
                paths_complete: (current.is_empty() || self.latest.complete_scan)
                    && matching.len() == count
                    && matching.iter().all(|track| !track.display_image.is_empty()),
                count,
                last_observed: matching
                    .iter()
                    .map(|track| track.last_observed)
                    .max()
                    .unwrap_or(self.at),
                exited_at: None,
                last_leaf_point: if matches!(selection, Selection::Process(_)) {
                    matching
                        .iter()
                        .filter_map(|track| track.points.back().copied())
                        .max_by_key(|point| point.at)
                } else {
                    None
                },
            }
        };
        cached.selection = selection.clone();
        if !current.is_empty() {
            if self.latest.complete_scan {
                cached.count = current.len();
                let paths: BTreeMap<_, _> = current
                    .iter()
                    .filter(|track| !track.display_image.is_empty())
                    .map(|track| (track.image.clone(), track.display_image.clone()))
                    .collect();
                let paths_complete = current.iter().all(|track| !track.display_image.is_empty());
                if paths_complete {
                    cached.paths = paths;
                } else {
                    cached.paths.extend(paths);
                }
                cached.paths_complete = paths_complete;
            }
            cached.last_observed = self.at;
            cached.exited_at = None;
            if matches!(selection, Selection::Process(_))
                && let Some(point) = current
                    .iter()
                    .filter_map(|track| track.current)
                    .max_by_key(|point| point.at)
            {
                cached.last_leaf_point = Some(point);
            }
        } else if self.latest.complete_scan
            && !self.times.is_empty()
            && self.at >= cached.last_observed
        {
            // An unreadable same-name executable might still belong to this path.
            let membership_known = if let Selection::App(path) = &selection {
                let name = path.rsplit('\\').next().unwrap_or(path).to_lowercase();
                !self.latest.observed.iter().any(|observed| {
                    observed.image.is_none() && observed.name.to_lowercase() == name
                })
            } else {
                true
            };
            if membership_known && cached.exited_at.is_none() {
                cached.exited_at = Some(self.at);
            }
        }
        Some(cached)
    }
    fn refresh_selected_identity(&mut self) {
        if let Some(previous) = self.selected_identity.take() {
            self.selected_identity = self.selection_identity(&previous.selection, Some(&previous));
        }
    }
    pub fn protect_selection(&mut self, selection: Option<&Selection>) {
        self.selected_identity = selection.and_then(|selection| {
            self.selection_identity(selection, self.selected_identity.as_ref())
        });
        self.protected_keys.clear();
        if let Some(selection) = selection {
            let resolved = self.resolve_selection(selection);
            let incoming: Vec<_> = self
                .latest
                .observed
                .iter()
                .filter_map(|observed| observed.key)
                .filter(|key| !self.tracks.contains_key(key))
                .filter_map(|key| self.current_track(key))
                .filter(|track| self.matches(track, &resolved))
                .collect();
            let keys: Vec<_> = self
                .tracks
                .values()
                .filter(|t| self.matches(t, &resolved))
                .map(|t| t.key)
                .collect();
            self.protected_keys.extend(keys);
            self.protected_keys
                .extend(incoming.iter().map(|track| track.key));
            if let Selection::Process(key) | Selection::Family(key) = resolved {
                self.protected_keys.insert(key);
            }
            // Admit selected observations before another scan can confirm their exit.
            for track in incoming {
                if self.tracks.len() >= MAX_TRACKS {
                    let victim = self
                        .tracks
                        .iter()
                        .filter(|(key, _)| !self.protected_keys.contains(key))
                        .min_by_key(|(key, track)| {
                            (
                                track.exited_at.is_none(),
                                track.current.map_or(0, |p| p.private_bytes),
                                **key,
                            )
                        })
                        .map(|(key, _)| *key);
                    if let Some(key) = victim {
                        self.tracks.remove(&key);
                    } else {
                        continue;
                    }
                }
                self.tracks.insert(track.key, track);
            }
            self.omitted = self
                .latest
                .observed
                .iter()
                .filter_map(|p| p.key)
                .filter(|key| !self.tracks.contains_key(key))
                .count();
        }
    }
    pub fn protect_selection_from(&mut self, shown: &History, selection: Option<&Selection>) {
        let frozen = selection.and_then(|selection| {
            shown.selection_identity(selection, shown.selected_identity_for(selection))
        });
        let existing = selection
            .and_then(|selection| self.selected_identity_for(selection))
            .cloned();
        self.selected_identity = match (existing, frozen) {
            (Some(mut existing), Some(mut frozen)) => {
                if frozen.last_observed > existing.last_observed {
                    frozen.exited_at = existing.exited_at.filter(|at| *at >= frozen.last_observed);
                    Some(frozen)
                } else {
                    if !existing.paths_complete {
                        if frozen.last_observed == existing.last_observed && frozen.paths_complete {
                            existing.name = frozen.name;
                            existing.paths = frozen.paths;
                            existing.paths_complete = true;
                            existing.count = frozen.count;
                        } else {
                            existing.paths.extend(frozen.paths);
                        }
                    }
                    existing.last_leaf_point = existing
                        .last_leaf_point
                        .into_iter()
                        .chain(frozen.last_leaf_point)
                        .max_by_key(|point| point.at);
                    Some(existing)
                }
            }
            (existing, frozen) => existing.or(frozen),
        };
        self.refresh_selected_identity();
        self.protect_selection(selection);
    }
    pub fn current_track(&self, key: ProcessKey) -> Option<Track> {
        let observed = self.latest.observed.iter().find(|p| p.key == Some(key))?;
        let sample = self.latest.processes.iter().find(|p| p.key == key);
        let point = sample.map(|p| Point {
            at: self.at,
            private_bytes: p.private,
            commit_bytes: p.commit,
        });
        let image = observed.image.clone().unwrap_or_default();
        let parent = self.latest.observed.iter().find(|p| {
            p.pid == observed.parent_pid
                && p.key.is_some_and(|parent| {
                    parent.generation == 0 && key.generation == 0 && parent.created < key.created
                })
        });
        Some(Track {
            key,
            name: observed.name.clone(),
            image: image.to_lowercase(),
            display_image: image,
            points: point.into_iter().collect(),
            current: point,
            family: self.latest_families.get(&key).copied().unwrap_or(key),
            parent_pid: observed.parent_pid,
            working_set: sample.map_or(0, |p| p.working_set),
            parent_name: parent.map(|p| p.name.clone()),
            parent_key: parent.and_then(|p| p.key),
            first_observed: self.at,
            first_observed_unix_ms: self.latest.captured_unix_ms,
            last_observed: self.at,
            exited_at: None,
            unavailable: observed
                .error
                .as_ref()
                .map(|reason| VecDeque::from([(self.at, reason.clone())]))
                .unwrap_or_default(),
        })
    }
    pub fn parent_target(&self, child: ProcessKey) -> Option<ProcessKey> {
        let child = self.resolve_key(child);
        self.tracks
            .get(&child)
            .and_then(|track| track.parent_key)
            .or_else(|| {
                self.latest
                    .observed
                    .iter()
                    .find(|p| p.key == Some(child))
                    .and_then(|child| {
                        self.latest
                            .observed
                            .iter()
                            .find(|p| {
                                p.pid == child.parent_pid
                                    && p.key.is_some_and(|parent| {
                                        parent.generation == 0
                                            && child.key.is_some_and(|key| {
                                                key.generation == 0 && parent.created < key.created
                                            })
                                    })
                            })
                            .and_then(|p| p.key)
                    })
            })
            .filter(|parent| {
                self.tracks.contains_key(parent)
                    || self.latest.observed.iter().any(|p| p.key == Some(*parent))
            })
    }
    pub fn observe(&mut self, mut snapshot: Snapshot) {
        if self.times.back().is_some_and(|last| snapshot.at <= *last) {
            return;
        }
        self.at = snapshot.at;
        self.total = snapshot.total;
        self.available = snapshot.available;
        self.inaccessible = snapshot.inaccessible;
        self.omitted = 0;
        self.interval = if self.interval == 0 { 2 } else { self.interval };
        // Count and attribute the complete scan before bounded per-instance retention.
        for process in &mut snapshot.processes {
            if process.image.is_empty() {
                process.image = self
                    .tracks
                    .get(&process.key)
                    .map(|t| t.display_image.clone())
                    .or_else(|| {
                        self.latest
                            .processes
                            .iter()
                            .find(|p| p.key == process.key)
                            .map(|p| p.image.clone())
                    })
                    .unwrap_or_default();
            }
        }
        snapshot.processes.sort_by_key(|p| p.key);
        snapshot.processes.dedup_by_key(|p| p.key);
        for process in &snapshot.processes {
            if !snapshot.observed.iter().any(|p| p.pid == process.key.pid) {
                snapshot.observed.push(ProcessObservation {
                    pid: process.key.pid,
                    key: Some(process.key),
                    name: process.name.clone(),
                    image: Some(process.image.clone()),
                    parent_pid: process.parent_pid,
                    error: None,
                });
            }
        }
        snapshot.observed.sort_by_key(|p| p.pid);
        snapshot.observed.dedup_by_key(|p| p.pid);
        // A provisional lifetime can be linked only when the recovered OS creation
        // predates its first observation and no complete scan established an exit.
        for observed in &snapshot.observed {
            if let Some(key) = observed.key
                && key.generation == 0
                && !self.tracks.contains_key(&key)
            {
                let provisional = self
                    .tracks
                    .values()
                    .find(|t| {
                        t.key.pid == key.pid
                            && t.key.generation > 0
                            && t.exited_at.is_none()
                            && t.first_observed_unix_ms > 0
                            && key
                                .created
                                .checked_div(10_000)
                                .and_then(|ms| ms.checked_sub(11_644_473_600_000))
                                .is_some_and(|ms| ms <= t.first_observed_unix_ms)
                    })
                    .map(|t| t.key);
                let provisional = provisional.or_else(|| {
                    self.latest
                        .observed
                        .iter()
                        .filter_map(|p| p.key)
                        .find(|previous| {
                            previous.pid == key.pid
                                && previous.generation > 0
                                && self
                                    .provisional_first
                                    .get(previous)
                                    .is_some_and(|(_, first)| {
                                        *first > 0
                                            && key
                                                .created
                                                .checked_div(10_000)
                                                .and_then(|ms| ms.checked_sub(11_644_473_600_000))
                                                .is_some_and(|ms| ms <= *first)
                                    })
                        })
                });
                if let Some(previous) = provisional {
                    if let Some(mut track) = self.tracks.remove(&previous) {
                        track.key = key;
                        if track.family == previous {
                            track.family = key;
                        }
                        self.tracks.insert(key, track);
                    }
                    self.key_aliases.insert(previous, key);
                    self.observed_session.remove(&previous);
                    if self.protected_keys.remove(&previous) {
                        self.protected_keys.insert(key);
                    }
                }
            }
        }
        for observed in &mut snapshot.observed {
            if observed.key.is_none() {
                observed.key = Some(
                    self.tracks
                        .values()
                        .find(|t| t.key.pid == observed.pid && t.exited_at.is_none())
                        .map(|t| t.key)
                        .or_else(|| {
                            self.latest
                                .observed
                                .iter()
                                .find(|p| p.pid == observed.pid)
                                .and_then(|p| p.key)
                        })
                        .unwrap_or_else(|| {
                            self.next_generation = self
                                .next_generation
                                .checked_add(1)
                                .expect("Provisional identity generations exhausted");
                            ProcessKey {
                                pid: observed.pid,
                                created: 0,
                                generation: self.next_generation,
                            }
                        }),
                );
            }
            if let Some(key) = observed.key
                && key.generation > 0
            {
                self.provisional_first
                    .entry(key)
                    .or_insert((snapshot.at, snapshot.captured_unix_ms));
            }
            if observed.image.is_none() {
                observed.image = observed
                    .key
                    .and_then(|key| {
                        self.tracks
                            .get(&key)
                            .filter(|t| !t.display_image.is_empty())
                            .map(|t| t.display_image.clone())
                    })
                    .or_else(|| {
                        observed.key.and_then(|key| {
                            self.latest
                                .processes
                                .iter()
                                .find(|p| p.key == key && !p.image.is_empty())
                                .map(|p| p.image.clone())
                        })
                    });
            }
            if observed.image.is_none()
                && let Some(process) = snapshot
                    .processes
                    .iter()
                    .find(|p| Some(p.key) == observed.key && !p.image.is_empty())
            {
                observed.image = Some(process.image.clone());
            }
        }
        let incoming: std::collections::BTreeSet<_> =
            snapshot.observed.iter().filter_map(|p| p.key).collect();
        let mut groups: BTreeMap<String, GroupMeasurement> = BTreeMap::new();
        let mut paths: BTreeMap<String, GroupMeasurement> = BTreeMap::new();
        for observed in &snapshot.observed {
            groups
                .entry(observed.name.to_lowercase())
                .or_default()
                .observed += 1;
            if let Some(image) = &observed.image {
                paths.entry(image.to_lowercase()).or_default().observed += 1;
            }
        }
        let mut measured_private = 0u64;
        for process in &snapshot.processes {
            let group = groups.entry(process.name.to_lowercase()).or_default();
            group.bytes = group.bytes.saturating_add(process.private);
            group.commit = group.commit.saturating_add(process.commit);
            group.measured += 1;
            if !process.image.is_empty() {
                let path = paths.entry(process.image.to_lowercase()).or_default();
                path.bytes = path.bytes.saturating_add(process.private);
                path.commit = path.commit.saturating_add(process.commit);
                path.measured += 1;
            }
            measured_private = measured_private.saturating_add(process.private);
        }
        if self.named_bands.is_empty()
            && !groups.is_empty()
            && snapshot.system_error.is_none()
            && snapshot.complete_scan
        {
            let mut prominent: Vec<_> = groups.iter().filter(|(_, g)| g.measured > 0).collect();
            prominent.sort_by(|(an, a), (bn, b)| b.bytes.cmp(&a.bytes).then_with(|| an.cmp(bn)));
            self.named_bands = prominent
                .into_iter()
                .take(3)
                .map(|(name, _)| name.clone())
                .collect();
        }
        self.system.push_back(SystemSample {
            at: snapshot.at,
            captured_unix_ms: snapshot.captured_unix_ms,
            total: snapshot.total,
            available: snapshot.available,
            error: snapshot.system_error.clone(),
            measured_private,
            measured: snapshot.processes.len(),
            observed: snapshot
                .observed
                .len()
                .max(snapshot.processes.len() + snapshot.inaccessible),
            groups,
            paths,
            families: BTreeMap::new(),
            complete_scan: snapshot.complete_scan,
        });
        if let Some(error) = &snapshot.system_error {
            self.failures.push_back((self.at, error.clone()));
        }
        if let Some(error) = &snapshot.collection_error {
            self.failures.push_back((self.at, error.clone()));
        }
        self.times.push_back(self.at);
        while self.times.len() > MAX_POINTS
            || self
                .times
                .front()
                .is_some_and(|at| self.at.saturating_sub(*at) > HISTORY_SECONDS)
        {
            self.times.pop_front();
        }
        let oldest = *self.times.front().unwrap();
        while self.system.front().is_some_and(|s| s.at < oldest) {
            self.system.pop_front();
        }
        while self.failures.front().is_some_and(|(at, _)| *at < oldest) {
            self.failures.pop_front();
        }
        self.tracks.retain(|key, track| {
            self.protected_keys.contains(key)
                || track.exited_at.is_none()
                || track.last_observed >= oldest
        });
        for track in self.tracks.values_mut() {
            track.current = None;
            let observed = snapshot.observed.iter().find(|p| {
                p.pid == track.key.pid
                    && p.key
                        .map(|key| key == track.key)
                        .unwrap_or(track.exited_at.is_none())
            });
            if let Some(observed) = observed {
                track.last_observed = self.at;
                track.exited_at = None;
                if let Some(error) = &observed.error {
                    track.unavailable.push_back((self.at, error.clone()));
                }
            } else if snapshot.complete_scan {
                if track.exited_at.is_none() {
                    track.exited_at = Some(self.at);
                }
            } else if track.exited_at.is_none() {
                track
                    .unavailable
                    .push_back((self.at, "Process enumeration unavailable".into()));
            }
            while track
                .unavailable
                .front()
                .is_some_and(|(at, _)| *at < oldest)
            {
                track.unavailable.pop_front();
            }
            while track.points.front().is_some_and(|p| p.at < oldest) {
                track.points.pop_front();
            }
        }
        self.tracks.retain(|key, track| {
            self.protected_keys.contains(key)
                || track.exited_at.is_none()
                || track.last_observed >= oldest
        });
        for observed in &snapshot.observed {
            let key = observed
                .key
                .or_else(|| {
                    self.tracks
                        .values()
                        .find(|track| track.key.pid == observed.pid && track.exited_at.is_none())
                        .map(|track| track.key)
                })
                .unwrap_or(ProcessKey {
                    pid: observed.pid,
                    created: 0,
                    generation: 0,
                });
            self.observed_session.insert(key);
            if let Some(error) = &observed.error {
                self.failures.push_back((
                    self.at,
                    format!("{} (PID {}): {error}", observed.name, observed.pid),
                ));
            }
            if observed.error.is_some()
                && !self.tracks.contains_key(&key)
                && self.tracks.len() >= MAX_TRACKS
                && self.protected_keys.contains(&key)
            {
                let victim = self
                    .tracks
                    .iter()
                    .filter(|(key, _)| !self.protected_keys.contains(key))
                    .min_by_key(|(key, t)| {
                        (
                            t.exited_at.is_none(),
                            t.points.back().map(|p| p.private_bytes).unwrap_or(0),
                            **key,
                        )
                    })
                    .map(|(key, _)| *key);
                if let Some(victim) = victim {
                    self.tracks.remove(&victim);
                }
            }
            if observed.error.is_some()
                && !self.tracks.contains_key(&key)
                && self.tracks.len() < MAX_TRACKS
            {
                let image = observed.image.clone().unwrap_or_default();
                self.tracks.insert(
                    key,
                    Track {
                        key,
                        name: observed.name.clone(),
                        image: image.to_lowercase(),
                        display_image: image,
                        points: VecDeque::new(),
                        current: None,
                        family: key,
                        parent_pid: observed.parent_pid,
                        working_set: 0,
                        parent_name: None,
                        parent_key: None,
                        first_observed: self
                            .provisional_first
                            .get(&key)
                            .map(|p| p.0)
                            .unwrap_or(self.at),
                        last_observed: self.at,
                        exited_at: None,
                        first_observed_unix_ms: self
                            .provisional_first
                            .get(&key)
                            .map(|p| p.1)
                            .unwrap_or(snapshot.captured_unix_ms),
                        unavailable: VecDeque::from([(self.at, observed.error.clone().unwrap())]),
                    },
                );
            } else if observed.error.is_some() && !self.tracks.contains_key(&key) {
                self.omitted += 1;
            }
        }
        let parents: BTreeMap<_, _> = snapshot
            .observed
            .iter()
            .filter_map(|p| {
                p.key.filter(|key| key.generation == 0).map(|key| {
                    (
                        key.pid,
                        (
                            key,
                            p.parent_pid,
                            matches!(
                                p.name.to_ascii_lowercase().as_str(),
                                "explorer.exe"
                                    | "services.exe"
                                    | "wininit.exe"
                                    | "winlogon.exe"
                                    | "smss.exe"
                                    | "csrss.exe"
                                    | "cmd.exe"
                                    | "powershell.exe"
                                    | "pwsh.exe"
                                    | "windowsterminal.exe"
                                    | "conhost.exe"
                                    | "taskmgr.exe"
                            ),
                            p.name.clone(),
                        ),
                    )
                })
            })
            .collect();
        let families: BTreeMap<_, _> = snapshot
            .observed
            .iter()
            .filter_map(|p| {
                p.key.map(|key| {
                    let mut root = key;
                    while let Some((_, parent_pid, _, _)) = parents.get(&root.pid) {
                        if let Some((parent, _, _, _)) =
                            parents.get(parent_pid).filter(|(parent, _, launcher, _)| {
                                parent.created < root.created && !launcher
                            })
                        {
                            root = *parent;
                        } else {
                            if let Some(previous) = self.tracks.get(&root) {
                                root = previous.family;
                            } else if let Some(previous) = self.latest_families.get(&root) {
                                root = *previous;
                            }
                            break;
                        }
                    }
                    (key, root)
                })
            })
            .collect();
        let aggregate = &mut self.system.back_mut().unwrap().families;
        for observed in &snapshot.observed {
            if let Some(family) = observed.key.and_then(|key| families.get(&key)) {
                aggregate.entry(*family).or_default().observed += 1;
            }
        }
        for process in &snapshot.processes {
            let group = aggregate.entry(families[&process.key]).or_default();
            group.bytes = group.bytes.saturating_add(process.private);
            group.commit = group.commit.saturating_add(process.commit);
            group.measured += 1;
        }
        self.latest_families = families.clone();
        self.latest = snapshot.clone();
        snapshot
            .processes
            .sort_by_key(|p| std::cmp::Reverse(p.private));
        for process in snapshot.processes {
            if !self.tracks.contains_key(&process.key) && self.tracks.len() >= MAX_TRACKS {
                let victim = self
                    .tracks
                    .iter()
                    .filter(|(key, track)| {
                        !incoming.contains(key)
                            && track.exited_at.is_some()
                            && !self.protected_keys.contains(key)
                    })
                    .min_by_key(|(_, t)| t.points.back().map(|p| p.at))
                    .map(|(key, _)| *key)
                    .or_else(|| {
                        self.protected_keys
                            .contains(&process.key)
                            .then(|| {
                                self.tracks
                                    .iter()
                                    .filter(|(key, _)| !self.protected_keys.contains(key))
                                    .min_by_key(|(key, t)| {
                                        (
                                            t.points.back().map(|p| p.private_bytes).unwrap_or(0),
                                            **key,
                                        )
                                    })
                                    .map(|(key, _)| *key)
                            })
                            .flatten()
                    });
                if let Some(key) = victim {
                    self.tracks.remove(&key);
                } else {
                    self.omitted += 1;
                    continue;
                }
            }
            let point = Point {
                at: self.at,
                private_bytes: process.private,
                commit_bytes: process.commit,
            };
            let track = self.tracks.entry(process.key).or_insert_with(|| Track {
                key: process.key,
                name: process.name.clone(),
                image: process.image.to_lowercase(),
                display_image: process.image.clone(),
                points: VecDeque::new(),
                current: None,
                family: families[&process.key],
                parent_pid: process.parent_pid,
                working_set: process.working_set,
                parent_name: None,
                parent_key: None,
                first_observed: self.at,
                first_observed_unix_ms: snapshot.captured_unix_ms,
                last_observed: self.at,
                exited_at: None,
                unavailable: VecDeque::new(),
            });
            if track.parent_name.is_none()
                && let Some((key, _, _, name)) = parents
                    .get(&track.parent_pid)
                    .filter(|(parent, _, _, _)| parent.created < track.key.created)
            {
                track.parent_name = Some(name.clone());
                track.parent_key = Some(*key);
            }
            track.points.push_back(point);
            while track.points.len() > MAX_POINTS
                || track.points.front().is_some_and(|p| p.at < oldest)
            {
                track.points.pop_front();
            }
            track.current = Some(point);
            track.last_observed = self.at;
            track.exited_at = None;
            track.name = process.name.clone();
            track.image = process.image.to_lowercase();
            track.display_image = process.image.clone();
            track.working_set = process.working_set;
            track.family = families[&process.key];
        }
        for track in self.tracks.values_mut() {
            if track.parent_name.is_none()
                && track.key.generation == 0
                && let Some((parent, _, _, name)) = parents
                    .get(&track.parent_pid)
                    .filter(|(parent, _, _, _)| parent.created < track.key.created)
            {
                track.parent_name = Some(name.clone());
                track.parent_key = Some(*parent);
            }
            if let Some(family) = families.get(&track.key) {
                track.family = *family;
            }
        }
        self.omitted = self
            .latest
            .observed
            .iter()
            .filter_map(|p| p.key)
            .filter(|key| !self.tracks.contains_key(key))
            .count();
        self.refresh_selected_identity();
    }
    pub fn matches(&self, track: &Track, selection: &Selection) -> bool {
        match selection {
            Selection::Name(name) => {
                track.name.eq_ignore_ascii_case(name) || track.name.to_lowercase() == *name
            }
            Selection::App(image) => &track.image == image,
            Selection::Family(key) => track.family == self.resolve_key(*key),
            Selection::Process(key) => track.key == self.resolve_key(*key),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn roots(history: &History, grouping: Grouping, query: &str) -> Vec<crate::dashboard::TreeRow> {
        crate::dashboard::tree_rows(
            history,
            grouping,
            false,
            query,
            300,
            crate::dashboard::Sort::default(),
        )
        .into_iter()
        .filter(|r| r.depth == 0)
        .collect()
    }
    fn graph(
        history: &History,
        selected: &Selection,
        commit: bool,
        seconds: u32,
    ) -> Vec<(u32, Option<u64>)> {
        crate::dashboard::series(
            history,
            selected,
            if commit {
                crate::dashboard::Metric::PrivateCommit
            } else {
                crate::dashboard::Metric::PrivateWorkingSet
            },
            crate::dashboard::Range::Last(seconds),
        )
        .points
        .into_iter()
        .map(|p| (p.at, p.reading.value))
        .collect()
    }
    #[test]
    fn parent_names_are_retained_after_exit_and_never_replaced_by_reused_pids() {
        let mut parent = sample(10, 100, 500);
        parent.name = "ChatGPT.exe".into();
        let mut child = sample(20, 200, 100);
        child.name = "codex.exe".into();
        child.parent_pid = 10;
        let mut h = History::default();
        h.observe(frame(0, vec![parent, child.clone()]));
        assert_eq!(
            h.tracks[&child.key].parent_description(),
            "ChatGPT.exe (PID 10)"
        );
        h.observe(frame(2, vec![child.clone()]));
        assert_eq!(
            h.tracks[&child.key].parent_description(),
            "ChatGPT.exe (PID 10)"
        );
        let mut reused = sample(10, 300, 900);
        reused.name = "unrelated.exe".into();
        let mut unknown = sample(30, 250, 200);
        unknown.name = "worker.exe".into();
        unknown.parent_pid = 10;
        h.observe(frame(
            4,
            vec![reused, child.clone(), unknown.clone(), sample(40, 400, 50)],
        ));
        assert_eq!(
            h.tracks[&child.key].parent_description(),
            "ChatGPT.exe (PID 10)"
        );
        assert_eq!(
            h.tracks[&unknown.key].parent_description(),
            "Unavailable (PID 10)"
        );
        assert_eq!(
            h.tracks[&ProcessKey {
                pid: 40,
                created: 400,
                generation: 0
            }]
                .parent_description(),
            "None"
        );
        assert_eq!(
            crate::dashboard::tree_rows(
                &h,
                Grouping::Process,
                false,
                "chatgpt",
                300,
                crate::dashboard::Sort::default()
            )
            .iter()
            .map(|r| r.selection.clone())
            .collect::<Vec<_>>(),
            vec![Selection::Process(child.key)]
        );
    }
    #[test]
    fn parent_navigation_targets_the_retained_instance_and_never_a_reused_pid() {
        let parent = sample(10, 100, 300);
        let mut child = sample(20, 200, 200);
        child.parent_pid = 10;
        let mut unknown = sample(30, 250, 100);
        unknown.parent_pid = 10;
        let mut h = History::default();
        h.observe(frame(0, vec![parent.clone(), child.clone()]));
        assert_eq!(h.parent_target(child.key), Some(parent.key));
        h.observe(frame(
            2,
            vec![sample(10, 300, 400), child.clone(), unknown.clone()],
        ));
        assert_eq!(
            h.parent_target(child.key),
            Some(parent.key),
            "Exited original parent remains the target"
        );
        assert_eq!(
            h.parent_target(unknown.key),
            None,
            "A younger reused PID is not its parent"
        );
        assert_eq!(h.parent_target(parent.key), None);
        h.observe(frame(2000, vec![child.clone(), sample(10, 300, 400)]));
        assert_eq!(
            h.parent_target(child.key),
            None,
            "Expired parent must not redirect to the current PID owner"
        );
        assert_eq!(
            h.parent_target(ProcessKey {
                pid: 999,
                created: 1,
                generation: 0
            }),
            None
        );
    }
    #[test]
    fn families_follow_nested_parents_and_do_not_attach_to_a_reused_parent_pid() {
        let mut root = sample(10, 10, 500);
        root.name = "editor.exe".into();
        let mut child = sample(20, 20, 300);
        child.name = "node.exe".into();
        child.parent_pid = 10;
        let mut grandchild = sample(30, 30, 100);
        grandchild.name = "worker.exe".into();
        grandchild.parent_pid = 20;
        let mut orphan = sample(40, 5, 50);
        orphan.parent_pid = 10;
        let mut h = History::default();
        h.observe(frame(
            0,
            vec![root, child.clone(), grandchild.clone(), orphan],
        ));
        let rows = roots(&h, Grouping::Family, "");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (
                rows[0].name.as_str(),
                rows[0].private.value.unwrap(),
                rows[0].count
            ),
            ("editor.exe", 900 * 1024, 3)
        );
        h.observe(frame(2, vec![child, grandchild]));
        let rows = roots(&h, Grouping::Family, "");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (
                rows[0].name.as_str(),
                rows[0].private.value.unwrap(),
                rows[0].count
            ),
            ("editor.exe", 400 * 1024, 2)
        );
    }
    #[test]
    fn family_grouping_does_not_combine_independent_apps_under_windows_launchers() {
        let mut launcher = sample(1, 1, 40);
        launcher.name = "explorer.exe".into();
        let mut editor = sample(2, 2, 300);
        editor.name = "editor.exe".into();
        editor.parent_pid = 1;
        let mut browser = sample(3, 3, 700);
        browser.name = "browser.exe".into();
        browser.parent_pid = 1;
        let mut worker = sample(4, 4, 200);
        worker.parent_pid = 2;
        let mut h = History::default();
        h.observe(frame(0, vec![launcher, editor, browser, worker]));
        let rows = roots(&h, Grouping::Family, "");
        assert_eq!(
            rows.iter()
                .map(|r| (r.name.as_str(), r.private.value.unwrap(), r.count))
                .collect::<Vec<_>>(),
            vec![
                ("browser.exe", 700 * 1024, 1),
                ("editor.exe", 500 * 1024, 2),
                ("explorer.exe", 40 * 1024, 1)
            ]
        );
    }
    #[test]
    fn process_name_group_combines_node_across_paths_and_can_expand_to_members() {
        let mut h = History::default();
        let mut a = sample(10, 1, 300);
        a.name = "node.exe".into();
        a.image = "C:\\Node\\node.exe".into();
        let mut b = sample(11, 2, 700);
        b.name = "node.exe".into();
        b.image = "C:\\Editor\\node.exe".into();
        h.observe(frame(0, vec![a, b, sample(12, 3, 200)]));
        let groups = roots(&h, Grouping::Name, "node");
        assert_eq!(groups.len(), 1);
        assert_eq!(
            (groups[0].private.value.unwrap(), groups[0].count),
            (1000 * 1024, 2)
        );
        assert_eq!(
            graph(&h, &Selection::Name("node.exe".into()), false, 1800),
            vec![(0, Some(1000 * 1024))]
        );
        assert_eq!(roots(&h, Grouping::Executable, "node").len(), 2);
    }
    #[test]
    fn name_group_graphs_work_for_unicode_executable_names() {
        let mut process = sample(1, 1, 123);
        process.name = "NÖDE.exe".into();
        let mut h = History::default();
        h.observe(frame(0, vec![process]));
        assert_eq!(
            graph(&h, &Selection::Name("nöde.exe".into()), false, 1800),
            vec![(0, Some(123 * 1024))]
        );
    }
    #[test]
    fn history_has_hard_caps_and_evicts_inactive_instances_before_live_ones() {
        let mut h = History::default();
        for at in 0..=2000 {
            h.observe(frame(at, vec![sample(1, 1, at)]));
        }
        let graph = graph(
            &h,
            &Selection::Process(ProcessKey {
                pid: 1,
                created: 1,
                generation: 0,
            }),
            false,
            1800,
        );
        assert_eq!(graph.len(), MAX_POINTS);
        assert_eq!(graph.first(), Some(&(1100, Some(1100 * 1024))));
        assert_eq!(graph.last(), Some(&(2000, Some(2000 * 1024))));
        assert_eq!(
            crate::dashboard::summary(
                &h,
                &Selection::Process(ProcessKey {
                    pid: 1,
                    created: 1,
                    generation: 0
                }),
                300
            )
            .unwrap()
            .change
            .value,
            Some(300 * 1024)
        );
        h.observe(frame(4000, vec![sample(2, 2, 300)]));
        assert_eq!(h.tracks.len(), 1);

        let mut h = History::default();
        let processes = (0..MAX_TRACKS as u32 + 30)
            .map(|pid| sample(pid, 1, 100))
            .collect();
        h.observe(frame(0, processes));
        assert_eq!(h.tracks.len(), MAX_TRACKS);
        assert_eq!(h.omitted, 30);
        h.observe(frame(2, vec![sample(9000, 2, 200)]));
        assert!(h.tracks.contains_key(&ProcessKey {
            pid: 9000,
            created: 2,
            generation: 0
        }));
        assert!(h.tracks.len() <= MAX_TRACKS);
    }
    fn sample(pid: u32, created: u64, ram_kib: u32) -> ProcessSample {
        ProcessSample {
            key: ProcessKey {
                pid,
                created,
                generation: 0,
            },
            name: "browser.exe".into(),
            image: "C:\\Apps\\browser.exe".into(),
            private: ram_kib as u64 * 1024,
            commit: ram_kib as u64 * 2048,
            working_set: ram_kib as u64 * 4096,
            parent_pid: 0,
        }
    }
    fn frame(at: u32, processes: Vec<ProcessSample>) -> Snapshot {
        Snapshot {
            at,
            total: 1 << 30,
            available: 1 << 28,
            processes,
            ..Default::default()
        }
    }
}
