//! Presentation preferences only. Process identities, paths and history are never persisted.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub grouping: String,
    pub compact: bool,
    pub column_widths: [i32; 6],
    pub splitter_permille: u32,
    pub overview_range: Option<u32>,
    pub history_range: Option<u32>,
    pub from_zero: bool,
    pub metric: String,
    pub overview_metric: String,
    pub overview_from_zero: bool,
    pub ui_zoom: u32,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            grouping: "name".into(),
            compact: true,
            column_widths: [300, 55, 190, 125, 125, 165],
            splitter_permille: 680,
            overview_range: Some(1800),
            history_range: Some(1800),
            from_zero: true,
            metric: "private_working_set".into(),
            overview_metric: "private_working_set".into(),
            overview_from_zero: true,
            ui_zoom: 100,
        }
    }
}

pub fn preferences_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("MemMap").join("preferences.ini"))
}

impl Preferences {
    pub fn load(path: &Path) -> io::Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error),
        };
        let mut result = Self::default();
        if !text.lines().any(|line| line.trim() == "version=1") {
            return Ok(result);
        }
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "grouping" if ["name", "path", "tree", "process", "family"].contains(&value) => {
                    result.grouping = value.into()
                }
                "metric" | "overview_metric"
                    if ["private_working_set", "private_commit"].contains(&value) =>
                {
                    if key.trim() == "metric" {
                        result.metric = value.into();
                    } else {
                        result.overview_metric = value.into();
                    }
                }
                "compact" => {
                    if let Ok(v) = value.parse() {
                        result.compact = v;
                    }
                }
                "overview_from_zero" => {
                    if let Ok(v) = value.parse() {
                        result.overview_from_zero = v;
                    }
                }
                "from_zero" => {
                    if let Ok(v) = value.parse() {
                        result.from_zero = v;
                    }
                }
                "splitter_permille" => {
                    if let Ok(v @ 400..=800) = value.parse() {
                        result.splitter_permille = v;
                    }
                }
                "ui_zoom" => {
                    if let Ok(v @ 50..=200) = value.parse() {
                        result.ui_zoom = v;
                    }
                }
                "overview_range" | "history_range" => {
                    let range = match value {
                        "all" => Some(None),
                        "300" => Some(Some(300)),
                        "900" => Some(Some(900)),
                        "1800" => Some(Some(1800)),
                        _ => None,
                    };
                    if let Some(range) = range {
                        if key.trim() == "overview_range" {
                            result.overview_range = range;
                        } else {
                            result.history_range = range;
                        }
                    }
                }
                "column_widths" => {
                    let widths: Result<Vec<i32>, _> = value.split(',').map(str::parse).collect();
                    if let Ok(widths) = widths
                        && widths.len() == 6
                        && widths.iter().all(|w| (45..=1600).contains(w))
                    {
                        result.column_widths.copy_from_slice(&widths);
                    }
                }
                _ => {}
            }
        }
        Ok(result)
    }
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let range = |r: Option<u32>| r.map(|n| n.to_string()).unwrap_or_else(|| "all".into());
        let text = format!(
            "version=1\ngrouping={}\ncompact={}\ncolumn_widths={}\nsplitter_permille={}\noverview_range={}\nhistory_range={}\nfrom_zero={}\nui_zoom={}\nmetric={}\noverview_metric={}\noverview_from_zero={}\n",
            self.grouping,
            self.compact,
            self.column_widths.map(|w| w.to_string()).join(","),
            self.splitter_permille,
            range(self.overview_range),
            range(self.history_range),
            self.from_zero,
            self.ui_zoom,
            self.metric,
            self.overview_metric,
            self.overview_from_zero
        );
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        // A same-directory rename publishes the complete file in one filesystem operation.
        fs::write(&temporary, text)?;
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presentation_round_trip_preserves_supported_preferences_and_rejects_invalid_fields() {
        let dir = std::env::temp_dir().join(format!("memmap-preferences-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("preferences.ini");
        let wanted = Preferences {
            grouping: "tree".into(),
            compact: false,
            column_widths: [410, 65, 225, 140, 145, 180],
            splitter_permille: 720,
            overview_range: None,
            history_range: Some(900),
            from_zero: false,
            metric: "private_commit".into(),
            overview_metric: "private_commit".into(),
            overview_from_zero: false,
            ui_zoom: 50,
        };
        wanted.save(&path).unwrap();
        assert_eq!(Preferences::load(&path).unwrap(), wanted);
        // Saving again must atomically replace an existing preference file on Windows.
        let replacement = Preferences {
            compact: true,
            ..wanted.clone()
        };
        replacement.save(&path).unwrap();
        assert_eq!(Preferences::load(&path).unwrap(), replacement);
        fs::write(&path, "version=1\ncompact=no\nsplitter_permille=0\nui_zoom=999\ngrouping=unknown\nhistory_range=17\noverview_range=all\ncolumn_widths=9,1\n").unwrap();
        let actual = Preferences::load(&path).unwrap();
        assert_eq!(
            actual,
            Preferences {
                overview_range: None,
                ..Preferences::default()
            }
        );
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&dir).unwrap();
        assert_eq!(Preferences::load(&path).unwrap(), Preferences::default());
    }
}
