//! Session-only display rules. Sampling and unfiltered history stay authoritative.
use crate::{
    dashboard::{self, OverviewPlot, TreeRow},
    model::{History, ProcessKey, Selection},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Rule {
    pub kind: String,
    pub value: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Exclusions {
    pub rules: Vec<Rule>,
    pub other_ram: bool,
    pub other_processes: bool,
    pub show_excluded: bool,
    #[serde(skip)]
    tree_members: BTreeMap<String, BTreeSet<ProcessKey>>,
}
impl Default for Exclusions {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            other_ram: false,
            other_processes: false,
            show_excluded: true,
            tree_members: BTreeMap::new(),
        }
    }
}
fn normalized(value: &str) -> String {
    value.trim().replace('/', "\\").to_lowercase()
}
impl Exclusions {
    pub fn active(&self) -> bool {
        !self.rules.is_empty() || self.other_ram || self.other_processes
    }
    pub fn validate(kind: &str, value: &str) -> Result<(), String> {
        if !["process", "tree", "path"].contains(&kind) {
            return Err("Unsupported exclusion kind".into());
        }
        if value.trim().is_empty() || value.chars().count() > 2048 {
            return Err("Exclusions require 1 to 2048 characters".into());
        }
        Ok(())
    }
    pub fn add(&mut self, kind: String, value: String) {
        let rule = Rule {
            kind,
            value: normalized(&value),
        };
        if !self.rules.contains(&rule) {
            self.rules.push(rule);
        }
    }
    pub fn remove(&mut self, kind: &str, value: &str) {
        self.rules
            .retain(|r| r.kind != kind || r.value != normalized(value));
    }
    pub fn refresh(&mut self, history: &History, frozen: Option<&History>) {
        if !self.rules.iter().any(|r| r.kind == "tree") {
            self.tree_members.clear();
            return;
        }
        let mut tracks = dashboard::all_tracks(history);
        if let Some(frozen) = frozen {
            tracks.extend(dashboard::all_tracks(frozen));
        }
        self.tree_members.retain(|value, _| {
            self.rules
                .iter()
                .any(|r| r.kind == "tree" && &r.value == value)
        });
        for rule in self.rules.iter().filter(|r| r.kind == "tree") {
            let members = self.tree_members.entry(rule.value.clone()).or_default();
            *members = members
                .iter()
                .map(|key| history.resolve_key(*key))
                .filter(|key| tracks.iter().any(|t| t.key == *key))
                .collect();
            members.extend(
                tracks
                    .iter()
                    .filter(|t| t.name.to_lowercase().contains(&rule.value))
                    .map(|t| t.key),
            );
            loop {
                let previous = members.len();
                for t in &tracks {
                    if t.parent_key
                        .is_some_and(|key| members.contains(&history.resolve_key(key)))
                    {
                        members.insert(t.key);
                    }
                }
                if previous == members.len() {
                    break;
                }
            }
        }
    }
    fn name_matches(&self, name: &str) -> bool {
        self.rules
            .iter()
            .any(|r| r.kind == "process" && name.to_lowercase().contains(&r.value))
    }
    pub fn keys(&self, history: &History) -> BTreeSet<ProcessKey> {
        if self.rules.is_empty() {
            return BTreeSet::new();
        }
        let tracks = dashboard::all_tracks(history);
        let mut trees: BTreeSet<_> = tracks
            .iter()
            .filter(|t| {
                self.rules
                    .iter()
                    .any(|r| r.kind == "tree" && t.name.to_lowercase().contains(&r.value))
            })
            .map(|t| t.key)
            .collect();
        trees.extend(self.tree_members.values().flatten().copied());
        loop {
            let previous = trees.len();
            for t in &tracks {
                if t.parent_key
                    .is_some_and(|key| trees.contains(&history.resolve_key(key)))
                {
                    trees.insert(t.key);
                }
            }
            if previous == trees.len() {
                break;
            }
        }
        tracks
            .iter()
            .filter(|t| {
                trees.contains(&t.key)
                    || self.name_matches(&t.name)
                    || self.rules.iter().any(|r| {
                        r.kind == "path"
                            && !t.image.is_empty()
                            && normalized(&t.image).contains(&r.value)
                    })
            })
            .map(|t| t.key)
            .collect()
    }
    pub fn rows(&self, history: &History, rows: Vec<TreeRow>) -> Vec<TreeRow> {
        if self.rules.is_empty() {
            return rows;
        }
        let keys = self.keys(history);
        let tracks = dashboard::all_tracks(history);
        let visible_keys: BTreeSet<_> = rows
            .iter()
            .filter_map(|r| match r.selection {
                Selection::Process(key) if !keys.contains(&key) => Some(key),
                _ => None,
            })
            .collect();
        let parents: BTreeMap<_, _> = rows
            .iter()
            .map(|r| (r.selection.clone(), r.parent.clone()))
            .collect();
        let mut rows: Vec<_> = rows
            .into_iter()
            .filter(|row| {
                tracks
                    .iter()
                    .any(|t| visible_keys.contains(&t.key) && history.matches(t, &row.selection))
            })
            .collect();
        let kept: BTreeSet<_> = rows.iter().map(|r| r.selection.clone()).collect();
        for row in &mut rows {
            let mut visited = BTreeSet::new();
            while let Some(parent) = row.parent.clone() {
                if kept.contains(&parent) {
                    break;
                }
                if !visited.insert(parent.clone()) {
                    row.parent = None;
                    break;
                }
                row.parent = parents.get(&parent).cloned().flatten();
            }
        }
        let parents: BTreeMap<_, _> = rows
            .iter()
            .map(|r| (r.selection.clone(), r.parent.clone()))
            .collect();
        for row in &mut rows {
            row.has_children = parents.values().any(|p| p.as_ref() == Some(&row.selection));
            let mut parent = row.parent.as_ref();
            row.depth = 0;
            while let Some(p) = parent {
                row.depth += 1;
                parent = parents.get(p).and_then(|p| p.as_ref());
            }
        }
        rows
    }
    pub fn overview(
        &self,
        history: &History,
        mut plot: OverviewPlot,
        commit: bool,
    ) -> OverviewPlot {
        if !self.active() {
            return plot;
        }
        let keys = self.keys(history);
        let tracks = dashboard::all_tracks(history);
        let detailed = self.rules.iter().any(|r| r.kind != "process");
        let affected: BTreeSet<_> = tracks
            .iter()
            .filter(|t| keys.contains(&t.key))
            .map(|t| t.name.to_lowercase())
            .collect();
        let mut by_name: BTreeMap<String, Vec<_>> = BTreeMap::new();
        for track in &tracks {
            by_name
                .entry(track.name.to_lowercase())
                .or_default()
                .push(track.as_ref());
        }
        let named_offset = if commit { 1 } else { 2 };
        for (sample, frame) in plot
            .samples
            .iter_mut()
            .zip(history.system.iter().filter(|s| s.at >= plot.start))
        {
            let Some(values) = &mut sample.layers else {
                sample.used = None;
                sample.reason = Some(format!(
                    "Filtered attribution unavailable: {}",
                    sample
                        .reason
                        .as_deref()
                        .unwrap_or("allocation detail unavailable")
                ));
                continue;
            };
            let mut removed = 0_u64;
            let mut gap = false;
            for (name, group) in &frame.groups {
                if group.measured == 0 {
                    continue;
                }
                let whole = self.name_matches(name);
                let members = by_name
                    .get(name)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|t| t.first_observed <= frame.at && t.last_observed >= frame.at);
                if !whole
                    && detailed
                    && members
                        .clone()
                        .filter(|t| t.points.binary_search_by_key(&frame.at, |p| p.at).is_ok())
                        .count()
                        != group.measured
                {
                    gap = true;
                    break;
                }
                let excluded = if whole {
                    if commit { group.commit } else { group.bytes }
                } else {
                    let mut bytes = 0;
                    for track in members.filter(|t| keys.contains(&t.key)) {
                        if let Ok(index) = track.points.binary_search_by_key(&frame.at, |p| p.at) {
                            let point = &track.points[index];
                            bytes += if commit {
                                point.commit_bytes
                            } else {
                                point.private_bytes
                            };
                        }
                    }
                    bytes
                };
                let index = history
                    .named_bands
                    .iter()
                    .position(|n| n == name)
                    .map_or(named_offset - 1, |i| i + named_offset);
                if excluded > values[index] {
                    gap = true;
                    break;
                }
                values[index] -= excluded;
                removed += excluded;
            }
            if gap {
                sample.layers = None;
                sample.used = None;
                sample.reason = Some("Filtered attribution unavailable: complete process detail is not retained for this sample".into());
                continue;
            }
            if !commit && self.other_ram {
                removed += values[0];
                values[0] = 0;
            }
            if self.other_processes {
                removed += values[named_offset - 1];
                values[named_offset - 1] = 0;
            }
            if self.show_excluded {
                values.push(removed);
            }
            sample.used = Some(values.iter().sum());
        }
        if self.show_excluded {
            plot.layers.push(dashboard::OverviewLayer {
                name: "Excluded".into(),
                selection: None,
            });
        }
        let keep: Vec<_> = plot.layers.iter().enumerate().filter_map(|(i, layer)| {
            let hidden = (layer.name == "Other used RAM" && self.other_ram) || (layer.name == "Other processes" && self.other_processes)
                || matches!(&layer.selection, Some(Selection::Name(name)) if self.name_matches(name))
                || (matches!(&layer.selection, Some(Selection::Name(name)) if affected.contains(name)) && !plot.samples.is_empty() && plot.samples.iter().all(|s| s.layers.as_ref().is_some_and(|v| v[i] == 0)));
            (!hidden).then_some(i)
        }).collect();
        plot.layers = keep.iter().map(|i| plot.layers[*i].clone()).collect();
        for sample in &mut plot.samples {
            if let Some(values) = &sample.layers {
                sample.layers = Some(keep.iter().map(|i| values[*i]).collect());
            }
        }
        plot
    }
}
