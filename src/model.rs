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
                    ),
                )
            })
            .collect();
        let families: BTreeMap<_, _> = snapshot
            .processes
            .iter()
            .map(|p| {
                let mut root = p.key;
                while let Some((_, parent_pid, _)) = parents.get(&root.pid) {
                    if let Some((parent, _, _)) = parents
                        .get(parent_pid)
                        .filter(|(parent, _, launcher)| parent.created < root.created && !launcher)
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
            });
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
                            || t.key.pid.to_string().contains(&query))
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
