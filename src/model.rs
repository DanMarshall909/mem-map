#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slice {
    pub label: String,
    pub bytes: u64,
}

pub fn partition(total: u64, available: u64, apps: &[(String, u64)]) -> Vec<Slice> {
    let available = available.min(total);
    let mut remaining = total - available;
    let mut slices = Vec::new();
    for (label, bytes) in apps {
        let bytes = (*bytes).min(remaining);
        slices.push(Slice {
            label: label.clone(),
            bytes,
        });
        remaining -= bytes;
    }
    slices.push(Slice {
        label: "Other in use".into(),
        bytes: remaining,
    });
    slices.push(Slice {
        label: "Available".into(),
        bytes: available,
    });
    slices
}

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Write};

pub const HISTORY_SECONDS: u32 = 30 * 60;
pub const MAX_POINTS: usize = 901;
pub const MAX_TRACKS: usize = 1024;

pub fn zoom_time(seconds: u32, inward: bool) -> u32 {
    const STEPS: [u32; 7] = [30, 60, 120, 300, 600, 900, HISTORY_SECONDS];
    if inward {
        STEPS.into_iter().rev().find(|s| *s < seconds).unwrap_or(30)
    } else {
        STEPS
            .into_iter()
            .find(|s| *s > seconds)
            .unwrap_or(HISTORY_SECONDS)
    }
}
pub fn zoom_interface(percent: u32, direction: i32) -> u32 {
    if direction == 0 {
        100
    } else {
        (percent as i32 + direction.signum() * 25).clamp(75, 200) as u32
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub scale: u32,
    pub width: i32,
    pub height: i32,
    pub page_width: i32,
    pub page_height: i32,
    pub x: i32,
    pub y: i32,
}
impl Viewport {
    pub fn new(width: i32, height: i32, dpi: u32, zoom: u32, x: i32, y: i32) -> Self {
        let scale = dpi.max(96) * zoom.clamp(75, 200) / 100;
        let page_width = (width.max(0) * 96 / scale as i32).max(1);
        let page_height = (height.max(0) * 96 / scale as i32).max(1);
        let width = page_width.max(1100);
        let height = page_height.max(790);
        Self {
            scale,
            width,
            height,
            page_width,
            page_height,
            x: x.clamp(0, width - page_width),
            y: y.clamp(0, height - page_height),
        }
    }
    pub fn point(&self, x: i32, y: i32) -> (i32, i32) {
        (
            x * 96 / self.scale as i32 + self.x,
            y * 96 / self.scale as i32 + self.y,
        )
    }
    pub fn physical(&self, x: i32, y: i32) -> (i32, i32) {
        (
            (x - self.x) * self.scale as i32 / 96,
            (y - self.y) * self.scale as i32 / 96,
        )
    }
}

pub fn parse_growth_window(text: &str) -> Result<Option<u32>, &'static str> {
    let error = "Use 1s–30m or Session";
    let text: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    if text == "session" {
        return Ok(None);
    }
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let amount = text[..split].parse::<u32>().map_err(|_| error)?;
    let multiplier = match &text[split..] {
        "" | "m" | "min" | "mins" | "minute" | "minutes" => 60,
        "s" | "sec" | "secs" | "second" | "seconds" => 1,
        _ => return Err(error),
    };
    let seconds = amount
        .checked_mul(multiplier)
        .filter(|s| (1..=HISTORY_SECONDS).contains(s))
        .ok_or(error)?;
    Ok(Some(seconds))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessKey {
    pub pid: u32,
    pub created: u64,
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

#[derive(Default, Debug)]
pub struct Snapshot {
    pub at: u32,
    pub total: u64,
    pub available: u64,
    pub inaccessible: usize,
    pub processes: Vec<ProcessSample>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub at: u32,
    pub ram_kib: u32,
    pub commit_kib: u32,
}

pub struct Track {
    pub key: ProcessKey,
    pub name: String,
    pub image: String,
    pub points: VecDeque<Point>,
    pub current: Option<Point>,
    pub family: ProcessKey,
    pub parent_pid: u32,
    pub working_set: u64,
    pub parent_name: Option<String>,
    pub parent_key: Option<ProcessKey>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
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
}

pub struct Row {
    pub selection: Selection,
    pub name: String,
    pub detail: String,
    pub ram: u64,
    pub commit: u64,
    pub peak: u64,
    pub growth: i64,
    pub count: usize,
    pub active: bool,
}

pub struct ComparisonMember {
    pub selection: Selection,
    pub name: String,
}

#[derive(Default)]
pub struct Comparison {
    pub members: Vec<ComparisonMember>,
}

pub struct ComparisonSeries {
    pub segments: Vec<Vec<(u32, u64)>>,
    pub current: Option<u64>,
}

pub struct ComparisonPlot {
    pub start: u32,
    pub end: u32,
    pub maximum: u64,
    pub series: Vec<ComparisonSeries>,
}

impl Comparison {
    pub fn plot(&self, history: &History, commit: bool, seconds: u32) -> ComparisonPlot {
        let series: Vec<_> = self
            .members
            .iter()
            .map(|member| {
                let points = history.graph(&member.selection, commit, seconds);
                let current = points.last().and_then(|(_, value)| *value);
                let mut segments: Vec<Vec<(u32, u64)>> = Vec::new();
                let mut previous = None;
                let mut new_segment = true;
                for (at, value) in points {
                    if previous.is_some_and(|last| at.saturating_sub(last) > 3) {
                        new_segment = true;
                    }
                    if let Some(value) = value {
                        if new_segment {
                            segments.push(Vec::new());
                        }
                        segments.last_mut().unwrap().push((at, value));
                        new_segment = false;
                    } else {
                        new_segment = true;
                    }
                    previous = Some(at);
                }
                ComparisonSeries { segments, current }
            })
            .collect();
        let peak = series
            .iter()
            .flat_map(|s| &s.segments)
            .flatten()
            .map(|(_, value)| *value)
            .max()
            .unwrap_or(0);
        ComparisonPlot {
            start: history.at.saturating_sub(seconds),
            end: history.at.max(1),
            maximum: peak.saturating_add(peak / 10).max(10 * 1024 * 1024),
            series,
        }
    }
    pub fn refresh(&mut self, history: &History, grouping: Grouping, commit: bool) {
        let mut rows = history.rows(grouping, false, "", false);
        rows.sort_by(|a, b| {
            let value = |row: &Row| if commit { row.commit } else { row.ram };
            value(b)
                .cmp(&value(a))
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.detail.cmp(&b.detail))
        });
        self.members = rows
            .into_iter()
            .take(5)
            .map(|row| ComparisonMember {
                selection: row.selection,
                name: row.name,
            })
            .collect();
    }
}

#[derive(Default)]
pub struct History {
    pub tracks: BTreeMap<ProcessKey, Track>,
    pub times: VecDeque<u32>,
    pub at: u32,
    pub total: u64,
    pub available: u64,
    pub inaccessible: usize,
    pub omitted: usize,
}

impl History {
    pub fn parent_target(&self, child: ProcessKey) -> Option<ProcessKey> {
        self.tracks
            .get(&child)
            .and_then(|track| track.parent_key)
            .filter(|parent| self.tracks.contains_key(parent))
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
        self.tracks
            .retain(|_, track| track.points.back().is_some_and(|p| p.at >= oldest));
        for track in self.tracks.values_mut() {
            track.current = None;
            while track.points.front().is_some_and(|p| p.at < oldest) {
                track.points.pop_front();
            }
        }
        let incoming: std::collections::BTreeSet<_> =
            snapshot.processes.iter().map(|p| p.key).collect();
        let parents: BTreeMap<_, _> = snapshot
            .processes
            .iter()
            .map(|p| {
                (
                    p.key.pid,
                    (
                        p.key,
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
            .collect();
        let families: BTreeMap<_, _> = snapshot
            .processes
            .iter()
            .map(|p| {
                let mut root = p.key;
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
                        }
                        break;
                    }
                }
                (p.key, root)
            })
            .collect();
        snapshot
            .processes
            .sort_by_key(|p| std::cmp::Reverse(p.private));
        for process in snapshot.processes {
            if !self.tracks.contains_key(&process.key) && self.tracks.len() >= MAX_TRACKS {
                let victim = self
                    .tracks
                    .iter()
                    .filter(|(key, _)| !incoming.contains(key))
                    .min_by_key(|(_, t)| t.points.back().map(|p| p.at))
                    .map(|(key, _)| *key);
                if let Some(key) = victim {
                    self.tracks.remove(&key);
                } else {
                    self.omitted += 1;
                    continue;
                }
            }
            let point = Point {
                at: self.at,
                ram_kib: (process.private / 1024).min(u32::MAX as u64) as u32,
                commit_kib: (process.commit / 1024).min(u32::MAX as u64) as u32,
            };
            let track = self.tracks.entry(process.key).or_insert_with(|| Track {
                key: process.key,
                name: process.name,
                image: process.image.to_lowercase(),
                points: VecDeque::new(),
                current: None,
                family: families[&process.key],
                parent_pid: process.parent_pid,
                working_set: process.working_set,
                parent_name: None,
                parent_key: None,
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
            track.working_set = process.working_set;
            track.family = families[&process.key];
        }
    }
    pub fn rows(&self, grouping: Grouping, inactive: bool, query: &str, growth: bool) -> Vec<Row> {
        self.rows_with_window(grouping, inactive, query, growth, None)
    }
    pub fn rows_with_window(
        &self,
        grouping: Grouping,
        inactive: bool,
        query: &str,
        growth: bool,
        window: Option<u32>,
    ) -> Vec<Row> {
        let query = query.to_lowercase();
        let mut rows: Vec<Row> = Vec::new();
        for track in self.tracks.values() {
            let active = track.current.is_some();
            if !active && !inactive {
                continue;
            }
            let selection = match grouping {
                Grouping::Name => Selection::Name(track.name.to_lowercase()),
                Grouping::Executable => Selection::App(track.image.clone()),
                Grouping::Family => Selection::Family(track.family),
                Grouping::Process => Selection::Process(track.key),
            };
            let row = if let Some(index) = rows.iter().position(|row| row.selection == selection) {
                &mut rows[index]
            } else {
                rows.push(Row {
                    selection,
                    name: if grouping == Grouping::Family {
                        self.tracks
                            .get(&track.family)
                            .map(|root| root.name.clone())
                            .unwrap_or_else(|| format!("Family PID {}", track.family.pid))
                    } else {
                        track.name.clone()
                    },
                    detail: if grouping == Grouping::Family {
                        self.tracks
                            .get(&track.family)
                            .map(|root| root.image.clone())
                            .unwrap_or_else(|| track.image.clone())
                    } else {
                        track.image.clone()
                    },
                    ram: 0,
                    commit: 0,
                    peak: 0,
                    growth: 0,
                    count: 0,
                    active: false,
                });
                rows.last_mut().unwrap()
            };
            row.active |= active;
            row.count += usize::from(active);
            if let Some(point) = track.current {
                row.ram += point.ram_kib as u64 * 1024;
                row.commit += point.commit_kib as u64 * 1024;
            }
        }
        rows.retain(|r| {
            query.is_empty()
                || self.tracks.values().any(|t| {
                    self.matches(t, &r.selection)
                        && (t.name.to_lowercase().contains(&query)
                            || t.image.contains(&query)
                            || t.key.pid.to_string().contains(&query)
                            || t.parent_name
                                .as_ref()
                                .is_some_and(|name| name.to_lowercase().contains(&query)))
                })
        });
        for row in &mut rows {
            let points = self.graph(&row.selection, false, HISTORY_SECONDS);
            row.peak = points.iter().filter_map(|p| p.1).max().unwrap_or(0);
            if row.active
                && let Some((at, Some(current))) = points.last()
            {
                let mut baseline = *current;
                let mut previous = *at;
                let cutoff = self.at.saturating_sub(window.unwrap_or(HISTORY_SECONDS));
                for (at, value) in points.iter().rev().skip(1) {
                    if *at < cutoff || previous.saturating_sub(*at) > 3 {
                        break;
                    }
                    let Some(value) = value else {
                        break;
                    };
                    baseline = *value;
                    previous = *at;
                }
                row.growth = *current as i64 - baseline as i64;
            }
        }
        rows.sort_by(|a, b| {
            b.active
                .cmp(&a.active)
                .then_with(|| {
                    if growth {
                        b.growth.cmp(&a.growth)
                    } else {
                        b.ram.cmp(&a.ram)
                    }
                })
                .then_with(|| a.name.cmp(&b.name))
        });
        rows
    }
    pub fn matches(&self, track: &Track, selection: &Selection) -> bool {
        match selection {
            Selection::Name(name) => {
                track.name.eq_ignore_ascii_case(name) || track.name.to_lowercase() == *name
            }
            Selection::App(image) => &track.image == image,
            Selection::Family(key) => &track.family == key,
            Selection::Process(key) => &track.key == key,
        }
    }
    pub fn graph(
        &self,
        selection: &Selection,
        commit: bool,
        seconds: u32,
    ) -> Vec<(u32, Option<u64>)> {
        let mut values: Vec<(u32, Option<u64>)> = self
            .times
            .iter()
            .copied()
            .filter(|at| *at >= self.at.saturating_sub(seconds))
            .map(|at| (at, None))
            .collect();
        for track in self.tracks.values().filter(|t| self.matches(t, selection)) {
            let mut index = 0;
            for point in &track.points {
                while index < values.len() && values[index].0 < point.at {
                    index += 1;
                }
                if index < values.len() && values[index].0 == point.at {
                    let bytes = u64::from(if commit {
                        point.commit_kib
                    } else {
                        point.ram_kib
                    }) * 1024;
                    values[index].1 = Some(values[index].1.unwrap_or(0) + bytes);
                }
            }
        }
        values
    }
    pub fn export(&self, mut out: impl Write) -> io::Result<()> {
        out.write_all(b"elapsed_seconds,pid,creation_filetime,name,image,private_ram_kib,private_commit_kib\r\n")?;
        for track in self.tracks.values() {
            let name = track.name.replace('"', "\"\"");
            let image = track.image.replace('"', "\"\"");
            let mut points = track.points.iter().peekable();
            let first = track.points.front().map(|p| p.at).unwrap_or(self.at);
            for at in self.times.iter().filter(|at| **at >= first) {
                write!(
                    out,
                    "{at},{},{},\"{name}\",\"{image}\",",
                    track.key.pid, track.key.created
                )?;
                if points.peek().is_some_and(|point| point.at == *at) {
                    let point = points.next().unwrap();
                    writeln!(out, "{},{}\r", point.ram_kib, point.commit_kib)?;
                } else {
                    out.write_all(b",\r\n")?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_is_bounded_and_scaled_scrolling_preserves_coordinate_identity() {
        assert_eq!(zoom_time(1800, true), 900);
        assert_eq!(zoom_time(900, true), 600);
        assert_eq!(zoom_time(500, true), 300);
        assert_eq!(zoom_time(500, false), 600);
        assert_eq!(zoom_time(30, true), 30);
        assert_eq!(zoom_time(1800, false), 1800);
        assert_eq!(zoom_interface(100, 1), 125);
        assert_eq!(zoom_interface(200, 1), 200);
        assert_eq!(zoom_interface(75, -1), 75);
        assert_eq!(zoom_interface(175, 0), 100);
        let normal = Viewport::new(1260, 890, 96, 100, 999, 999);
        assert_eq!(
            (normal.width, normal.height, normal.x, normal.y),
            (1260, 890, 0, 0)
        );
        let enlarged = Viewport::new(1260, 890, 96, 200, 999, 999);
        assert_eq!(
            (
                enlarged.scale,
                enlarged.width,
                enlarged.height,
                enlarged.page_width,
                enlarged.page_height,
                enlarged.x,
                enlarged.y
            ),
            (192, 1100, 790, 630, 445, 470, 345)
        );
        assert_eq!(enlarged.point(100, 50), (520, 370));
        assert_eq!(enlarged.physical(520, 370), (100, 50));
        let high_dpi = Viewport::new(1260, 890, 144, 125, 999, 999);
        assert_eq!(high_dpi.scale, 180);
        let physical = high_dpi.physical(500, 400);
        let logical = high_dpi.point(physical.0, physical.1);
        assert!((logical.0 - 500).abs() <= 1 && (logical.1 - 400).abs() <= 1);
        let zero = Viewport::new(0, 0, 96, 100, -100, -100);
        assert!(zero.page_width > 0 && zero.page_height > 0 && zero.x == 0 && zero.y == 0);
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
                created: 400
            }]
                .parent_description(),
            "None"
        );
        assert_eq!(
            h.rows(Grouping::Process, false, "chatgpt", false)
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
                created: 1
            }),
            None
        );
    }
    #[test]
    fn comparison_uses_one_scale_and_time_window_with_gaps_and_no_false_zeroes() {
        let mut h = History::default();
        h.observe(frame(
            0,
            vec![sample(1, 1, 100 * 1024), sample(2, 2, 50 * 1024)],
        ));
        let mut comparison = Comparison::default();
        comparison.refresh(&h, Grouping::Process, false);
        h.observe(frame(
            2,
            vec![sample(1, 1, 200 * 1024), sample(2, 2, 80 * 1024)],
        ));
        h.observe(frame(10, vec![sample(1, 1, 300 * 1024)]));
        h.observe(frame(12, vec![sample(2, 2, 90 * 1024)]));
        h.observe(frame(
            14,
            vec![sample(1, 1, 400 * 1024), sample(2, 2, 100 * 1024)],
        ));
        let mib = 1024 * 1024;
        let plot = comparison.plot(&h, false, 300);
        assert_eq!((plot.start, plot.end, plot.maximum), (0, 14, 440 * mib));
        assert_eq!(plot.series.len(), 2);
        assert_eq!(
            plot.series[0].segments,
            vec![
                vec![(0, 100 * mib), (2, 200 * mib)],
                vec![(10, 300 * mib)],
                vec![(14, 400 * mib)]
            ]
        );
        assert_eq!(
            plot.series[1].segments,
            vec![
                vec![(0, 50 * mib), (2, 80 * mib)],
                vec![(12, 90 * mib), (14, 100 * mib)]
            ]
        );
        assert_eq!(plot.series[0].current, Some(400 * mib));
        let zoom = comparison.plot(&h, true, 3);
        assert_eq!((zoom.start, zoom.end, zoom.maximum), (11, 14, 880 * mib));
        assert_eq!(zoom.series[0].segments, vec![vec![(14, 800 * mib)]]);
        assert_eq!(
            zoom.series[1].segments,
            vec![vec![(12, 180 * mib), (14, 200 * mib)]]
        );
        h.observe(frame(16, vec![]));
        assert!(
            comparison
                .plot(&h, false, 300)
                .series
                .iter()
                .all(|s| s.current.is_none())
        );
        let empty = Comparison::default().plot(&History::default(), false, 300);
        assert!(empty.series.is_empty());
        assert!(empty.end > empty.start && empty.maximum > 0);
    }
    #[test]
    fn comparison_pins_five_current_groups_and_refreshes_by_metric_without_identity_reuse() {
        let mut history = History::default();
        let processes: Vec<_> = (1..=7)
            .map(|pid| {
                let mut p = sample(pid, u64::from(pid), pid * 100);
                p.name = format!("app{pid}.exe");
                p.image = format!("C:\\Apps\\app{pid}.exe");
                p.commit = u64::from(8 - pid) * 1000 * 1024;
                p
            })
            .collect();
        history.observe(frame(0, processes.clone()));
        let mut comparison = Comparison::default();
        comparison.refresh(&history, Grouping::Name, false);
        assert_eq!(
            comparison
                .members
                .iter()
                .map(|m| m.name.as_str())
                .collect::<Vec<_>>(),
            vec!["app7.exe", "app6.exe", "app5.exe", "app4.exe", "app3.exe"]
        );
        history.observe(frame(2, vec![processes[0].clone()]));
        assert_eq!(
            comparison.members[0].selection,
            Selection::Name("app7.exe".into())
        );
        assert_eq!(
            history.graph(&comparison.members[0].selection, false, 300),
            vec![(0, Some(700 * 1024)), (2, None)]
        );
        comparison.refresh(&history, Grouping::Name, true);
        assert_eq!(comparison.members.len(), 1);
        assert_eq!(
            comparison.members[0].selection,
            Selection::Name("app1.exe".into())
        );
        history.observe(frame(4, processes.clone()));
        comparison.refresh(&history, Grouping::Process, true);
        assert_eq!(
            comparison
                .members
                .iter()
                .map(|m| m.selection.clone())
                .collect::<Vec<_>>(),
            (1..=5)
                .map(|pid| Selection::Process(ProcessKey {
                    pid,
                    created: u64::from(pid)
                }))
                .collect::<Vec<_>>()
        );
        let mut reused = sample(1, 1000, 9999);
        reused.name = "new.exe".into();
        history.observe(frame(6, vec![reused]));
        assert_eq!(
            history
                .graph(&comparison.members[0].selection, true, 300)
                .last(),
            Some(&(6, None))
        );
        comparison.refresh(&history, Grouping::Process, false);
        assert_eq!(
            comparison.members[0].selection,
            Selection::Process(ProcessKey {
                pid: 1,
                created: 1000
            })
        );
        history.observe(frame(8, vec![]));
        comparison.refresh(&history, Grouping::Name, false);
        assert!(comparison.members.is_empty());
    }

    #[test]
    fn configurable_growth_accepts_presets_custom_durations_and_rejects_outside_retention() {
        for (text, expected) in [
            ("1 min", Some(60)),
            ("1s", Some(1)),
            ("1800s", Some(1800)),
            ("90s", Some(90)),
            ("7m", Some(420)),
            ("15", Some(900)),
            ("30 minutes", Some(1800)),
            ("Session", None),
        ] {
            assert_eq!(parse_growth_window(text), Ok(expected), "{text}");
        }
        for text in [
            "",
            "0s",
            "31m",
            "1801s",
            "-1m",
            "1.5m",
            "99999999999999m",
            "4294967295m",
            "nonsense",
        ] {
            assert!(parse_growth_window(text).is_err(), "{text}");
        }
    }
    #[test]
    fn growth_uses_the_configured_lookback_and_concurrent_group_values_including_exits() {
        let mut h = History::default();
        for (at, a, b) in [
            (0, 100, 200),
            (2, 200, 300),
            (4, 250, 250),
            (6, 300, 200),
            (8, 350, 100),
        ] {
            h.observe(frame(at, vec![sample(1, 1, a), sample(2, 2, b)]));
        }
        let rows = h.rows_with_window(Grouping::Process, false, "", false, Some(4));
        assert_eq!((rows[0].growth, rows[1].growth), (100 * 1024, -150 * 1024));
        assert_eq!(
            h.rows_with_window(Grouping::Process, false, "1", false, Some(3))[0].growth,
            50 * 1024,
            "Use the first captured point inside a non-aligned lookback"
        );
        assert_eq!(
            h.rows_with_window(Grouping::Name, false, "", false, Some(4))[0].growth,
            -50 * 1024
        );
        assert_eq!(
            h.rows_with_window(Grouping::Name, false, "", false, None)[0].growth,
            150 * 1024
        );
        h.observe(frame(10, vec![sample(1, 1, 450)]));
        assert_eq!(
            h.rows_with_window(Grouping::Name, false, "", false, Some(4))[0].growth,
            -50 * 1024,
            "Group change includes a departing instance"
        );
        h.observe(frame(30, vec![sample(1, 1, 700)]));
        assert_eq!(
            h.rows_with_window(Grouping::Name, false, "", false, Some(300))[0].growth,
            0,
            "A capture gap resets the endpoint baseline"
        );
    }
    #[test]
    fn peak_ram_is_the_highest_concurrent_group_total_not_a_sum_of_individual_peaks() {
        let mut h = History::default();
        h.observe(frame(0, vec![sample(1, 1, 100), sample(2, 2, 300)]));
        h.observe(frame(2, vec![sample(1, 1, 300), sample(2, 2, 100)]));
        h.observe(frame(4, vec![sample(1, 1, 100), sample(2, 2, 300)]));
        let group = h.rows(Grouping::Name, false, "", false);
        assert_eq!(group[0].peak, 400 * 1024);
        assert!(
            h.rows(Grouping::Process, false, "", false)
                .iter()
                .all(|row| row.peak == 300 * 1024)
        );
        h.observe(frame(6, vec![sample(1, 1, 50)]));
        assert_eq!(
            h.rows(Grouping::Name, false, "", false)[0].peak,
            400 * 1024,
            "Exited instances remain in group peak history"
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
        let groups = h.rows(Grouping::Name, false, "node", false);
        assert_eq!(groups.len(), 1);
        assert_eq!((groups[0].ram, groups[0].count), (1000 * 1024, 2));
        assert_eq!(
            h.graph(&Selection::Name("node.exe".into()), false, 1800),
            vec![(0, Some(1000 * 1024))]
        );
        assert_eq!(h.rows(Grouping::Executable, false, "node", false).len(), 2);
    }
    #[test]
    fn name_group_graphs_work_for_unicode_executable_names() {
        let mut process = sample(1, 1, 123);
        process.name = "NÖDE.exe".into();
        let mut h = History::default();
        h.observe(frame(0, vec![process]));
        assert_eq!(
            h.graph(&Selection::Name("nöde.exe".into()), false, 1800),
            vec![(0, Some(123 * 1024))]
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
        let rows = h.rows(Grouping::Family, false, "", false);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0].name.as_str(), rows[0].ram, rows[0].count),
            ("editor.exe", 900 * 1024, 3)
        );
        h.observe(frame(2, vec![child, grandchild]));
        let rows = h.rows(Grouping::Family, false, "", false);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (rows[0].name.as_str(), rows[0].ram, rows[0].count),
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
        let rows = h.rows(Grouping::Family, false, "", false);
        assert_eq!(
            rows.iter()
                .map(|r| (r.name.as_str(), r.ram, r.count))
                .collect::<Vec<_>>(),
            vec![
                ("browser.exe", 700 * 1024, 1),
                ("editor.exe", 500 * 1024, 2),
                ("explorer.exe", 40 * 1024, 1)
            ]
        );
    }

    #[test]
    fn pie_keeps_app_slots_stable_when_zero_or_capped_so_clicks_cannot_shift_identity() {
        let slices = partition(100, 100, &[("zero".into(), 0), ("capped".into(), 10)]);
        assert_eq!(
            slices
                .iter()
                .map(|s| (s.label.as_str(), s.bytes))
                .collect::<Vec<_>>(),
            vec![
                ("zero", 0),
                ("capped", 0),
                ("Other in use", 0),
                ("Available", 100)
            ]
        );
    }
    #[test]
    fn ram_and_growth_sorts_have_different_numeric_orders_with_inactive_last() {
        let mut h = History::default();
        h.observe(frame(
            0,
            vec![sample(1, 1, 1000), sample(2, 2, 100), sample(3, 3, 5000)],
        ));
        h.observe(frame(2, vec![sample(1, 1, 1010), sample(2, 2, 400)]));
        let order = |rows: Vec<Row>| rows.into_iter().map(|r| r.selection).collect::<Vec<_>>();
        let a = Selection::Process(ProcessKey { pid: 1, created: 1 });
        let b = Selection::Process(ProcessKey { pid: 2, created: 2 });
        let dead = Selection::Process(ProcessKey { pid: 3, created: 3 });
        assert_eq!(
            order(h.rows(Grouping::Process, true, "", false)),
            vec![a.clone(), b.clone(), dead.clone()]
        );
        assert_eq!(
            order(h.rows(Grouping::Process, true, "", true)),
            vec![b, a, dead]
        );
    }

    #[test]
    fn pie_accounts_for_private_ram_other_memory_and_available_without_overlap() {
        let slices = partition(
            1000,
            250,
            &[("browser".into(), 400), ("editor".into(), 100)],
        );
        assert_eq!(
            slices,
            vec![
                Slice {
                    label: "browser".into(),
                    bytes: 400
                },
                Slice {
                    label: "editor".into(),
                    bytes: 100
                },
                Slice {
                    label: "Other in use".into(),
                    bytes: 250
                },
                Slice {
                    label: "Available".into(),
                    bytes: 250
                },
            ]
        );
        assert_eq!(slices.iter().map(|s| s.bytes).sum::<u64>(), 1000);
    }

    fn sample(pid: u32, created: u64, ram_kib: u32) -> ProcessSample {
        ProcessSample {
            key: ProcessKey { pid, created },
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

    #[test]
    fn app_groups_instances_but_pid_reuse_keeps_distinct_process_histories() {
        let mut h = History::default();
        h.observe(frame(0, vec![sample(7, 10, 100), sample(8, 11, 200)]));
        h.observe(frame(2, vec![sample(7, 20, 50), sample(8, 11, 250)]));
        let apps = h.rows(Grouping::Executable, false, "BROWSER", false);
        assert_eq!(apps.len(), 1);
        assert_eq!((apps[0].ram, apps[0].count), (300 * 1024, 2));
        let process = Selection::Process(ProcessKey {
            pid: 7,
            created: 10,
        });
        assert_eq!(
            h.graph(&process, false, 1800),
            vec![(0, Some(100 * 1024)), (2, None)]
        );
        assert_eq!(h.rows(Grouping::Process, true, "", false).len(), 3);
        assert_eq!(h.rows(Grouping::Process, false, "", false).len(), 2);
        assert_eq!(
            h.graph(&Selection::App("c:\\apps\\browser.exe".into()), true, 1800),
            vec![(0, Some(600 * 1024)), (2, Some(600 * 1024))]
        );
    }

    #[test]
    fn history_has_hard_caps_and_evicts_inactive_instances_before_live_ones() {
        let mut h = History::default();
        for at in 0..=2000 {
            h.observe(frame(at, vec![sample(1, 1, at)]));
        }
        let graph = h.graph(
            &Selection::Process(ProcessKey { pid: 1, created: 1 }),
            false,
            1800,
        );
        assert_eq!(graph.len(), MAX_POINTS);
        assert_eq!(graph.first(), Some(&(1100, Some(1100 * 1024))));
        assert_eq!(graph.last(), Some(&(2000, Some(2000 * 1024))));
        assert_eq!(
            h.rows(Grouping::Process, false, "", true)[0].growth,
            900 * 1024
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
            created: 2
        }));
        assert!(h.tracks.len() <= MAX_TRACKS);
    }

    #[test]
    fn csv_exports_instances_units_gaps_and_quoted_paths() {
        let mut h = History::default();
        let mut process = sample(7, 99, 123);
        process.name = "say\"hi,there.exe".into();
        h.observe(frame(0, vec![process]));
        h.observe(frame(2, vec![]));
        let mut csv = Vec::new();
        h.export(&mut csv).unwrap();
        assert_eq!(
            String::from_utf8(csv).unwrap(),
            concat!(
                "elapsed_seconds,pid,creation_filetime,name,image,private_ram_kib,private_commit_kib\r\n",
                "0,7,99,\"say\"\"hi,there.exe\",\"c:\\apps\\browser.exe\",123,246\r\n",
                "2,7,99,\"say\"\"hi,there.exe\",\"c:\\apps\\browser.exe\",,\r\n"
            )
        );
    }

    #[test]
    fn observation_gaps_reset_growth_and_export_keeps_retained_inactive_points() {
        let mut h = History::default();
        h.observe(frame(0, vec![sample(7, 1, 100), sample(8, 1, 50)]));
        h.observe(frame(2, vec![sample(7, 1, 130)]));
        h.observe(frame(20, vec![sample(7, 1, 200)]));
        assert_eq!(
            h.rows(Grouping::Process, false, "7", true)[0].growth,
            0,
            "Pause must reset growth baseline"
        );
        h.observe(frame(22, vec![sample(7, 1, 240)]));
        assert_eq!(
            h.rows(Grouping::Process, false, "7", true)[0].growth,
            40 * 1024
        );
        h.observe(frame(21, vec![]));
        assert_eq!(h.at, 22, "Out-of-order observations cannot rewrite history");
        for at in (24..1820).step_by(2) {
            h.observe(frame(at, vec![sample(7, 1, 240)]));
        }
        assert!(!h.tracks.contains_key(&ProcessKey { pid: 8, created: 1 }));
        let mut csv = Vec::new();
        h.export(&mut csv).unwrap();
        let text = String::from_utf8(csv).unwrap();
        assert!(text.contains("20,7,1,\"browser.exe\",\"c:\\apps\\browser.exe\",200,400\r\n"));
    }

    #[test]
    fn pie_remains_a_partition_during_sampling_races_and_zero_memory() {
        for (total, available, apps) in [
            (100, 20, vec![("a".into(), 90), ("b".into(), 40)]),
            (100, 200, vec![("a".into(), 10)]),
            (0, 0, vec![]),
        ] {
            assert_eq!(
                partition(total, available, &apps)
                    .iter()
                    .map(|s| s.bytes)
                    .sum::<u64>(),
                total
            );
        }
    }
}
