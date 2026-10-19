use mem_map::{
    dashboard::{self, *},
    model::*,
    preferences::{Preferences, preferences_path},
    sampler,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{self, BufWriter, Write},
    mem::{size_of, zeroed},
    path::{Path, PathBuf},
    ptr::{null, null_mut},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{
        DataExchange::*,
        LibraryLoader::GetModuleHandleW,
        Memory::*,
        Threading::{GR_GDIOBJECTS, GetCurrentProcess, GetCurrentProcessId, GetGuiResources},
    },
    UI::{
        Controls::{Dialogs::*, *},
        HiDpi::*,
        Input::KeyboardAndMouse::*,
        Shell::{DefSubclassProc, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};
mod accessibility;
mod draw;
use accessibility::{AccessibilityProvider, AccessibleRow, AccessibleTable};
use draw::*;
use windows::Win32::System::WinRT::{RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize};
use windows_sys::Win32::{
    Storage::FileSystem::FileTimeToLocalFileTime, System::Time::FileTimeToSystemTime,
};
const ID_SYSTEM: i32 = 100;
const ID_PROGRESS: i32 = 101;
const ID_LIVE: i32 = 102;
const ID_PAUSE: i32 = 103;
const ID_EXPORT: i32 = 104;
const ID_GROUP: i32 = 110;
const ID_FILTER: i32 = 120;
const ID_CLEAR: i32 = 121;
const ID_EXITED: i32 = 122;
const ID_COMPACT: i32 = 123;
const ID_CHANGE: i32 = 124;
const ID_TABLE: i32 = 130;
const ID_OVERVIEW_RANGE: i32 = 140;
const ID_LEGEND: i32 = 145;
const ID_OVERVIEW_VALUES: i32 = 151;
const ID_PATH: i32 = 160;
const ID_COPY: i32 = 161;
const ID_SHOW: i32 = 162;
const ID_METRIC: i32 = 163;
const ID_RANGE: i32 = 164;
const ID_ZERO: i32 = 165;
const ID_FIT: i32 = 166;
const ID_RESET: i32 = 167;
const ID_FOLLOW: i32 = 168;
const ID_HISTORY_VALUES: i32 = 169;
const ID_DETAILS: i32 = 175;
const ID_ZOOM: i32 = 176;
const ID_RANGE_SEG: i32 = 180;
const ID_TITLE: i32 = 190;
const ID_COUNTS: i32 = 191;
const ID_SUMMARY: i32 = 192;
const ID_COVERAGE: i32 = 196;
const ID_FOOTER: i32 = 197;
const ID_ORDER: i32 = 201;
const MSG_LAYOUT: u32 = WM_APP + 2;
const MSG_EXPORT: u32 = WM_APP + 3;
const MSG_DETAILS: u32 = WM_APP + 4;
const MSG_ACCESS: u32 = WM_APP + 20;
const MSG_CHART_KEY: u32 = WM_APP + 21;
const MSG_CONTROL_FOCUS: u32 = WM_APP + 22;
const COLLECTION_SECONDS: u32 = 2;
const FILTER_TIMER: usize = 2;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CollectionStatus {
    Collecting,
    Live,
    Unavailable,
    Stopped,
    Delayed,
}
fn collection_status(
    received: bool,
    stopped: bool,
    age: Duration,
    failed: bool,
) -> CollectionStatus {
    if stopped {
        CollectionStatus::Stopped
    } else if failed {
        CollectionStatus::Unavailable
    } else if !received {
        CollectionStatus::Collecting
    } else if age > Duration::from_secs(COLLECTION_SECONDS as u64 * 3) {
        CollectionStatus::Delayed
    } else {
        CollectionStatus::Live
    }
}
fn recording_text(paused_at: Option<&str>, status: CollectionStatus) -> String {
    let state = match status {
        CollectionStatus::Collecting => "Collecting first sample",
        CollectionStatus::Live => {
            if paused_at.is_some() {
                "Recording continues"
            } else {
                "Live · Updating every 2 seconds"
            }
        }
        CollectionStatus::Unavailable => "Collection unavailable",
        CollectionStatus::Stopped => "Collector stopped",
        CollectionStatus::Delayed => "Waiting for collection · Last sample is stale",
    };
    paused_at.map_or_else(
        || state.to_string(),
        |at| format!("Display paused at {at} · {state}"),
    )
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn rect(x: i32, y: i32, w: i32, h: i32) -> RECT {
    RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    }
}
struct OverviewAnnotations {
    used: RECT,
    available_label: RECT,
    available_value: RECT,
}
fn overview_annotations(chart: RECT, used_y: i32) -> OverviewAnnotations {
    let x = chart.right + 6;
    OverviewAnnotations {
        used: rect(x, (used_y - 12).max(chart.top + 44), 82, 24),
        available_label: rect(x + 13, chart.top + 2, 76, 18),
        available_value: rect(x + 13, chart.top + 20, 76, 18),
    }
}
fn contains(r: RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}
fn bytes(n: u64) -> String {
    if n >= 1024 * 1024 * 1024 {
        format!("{:.2} GiB", n as f64 / 1073741824.0)
    } else {
        format!("{:.1} MiB", n as f64 / 1048576.0)
    }
}
fn signed(n: i64) -> String {
    if n.unsigned_abs() < 52429 {
        return "0.0 MiB".into();
    }
    format!(
        "{}{}",
        if n < 0 { "−" } else { "+" },
        bytes(n.unsigned_abs())
    )
}
fn reading(r: &Reading) -> String {
    let value = r.value.map(bytes).unwrap_or_else(|| "—".into());
    if r.partial() {
        format!("{value} · Partial")
    } else {
        value
    }
}
fn complete_history_value(reading: &Reading) -> Option<u64> {
    reading.value.filter(|_| !reading.partial())
}
fn identity_text(row: &TreeRow) -> &str {
    if matches!(row.selection, Selection::App(_)) {
        &row.detail
    } else {
        &row.name
    }
}
fn choose_history_range(
    selected: &mut Option<u32>,
    manual_width: &mut Option<u32>,
    endpoint: &mut Option<u32>,
    follow: bool,
    displayed_end: u32,
    choice: Option<u32>,
) {
    *selected = choice;
    *manual_width = None;
    if follow {
        *endpoint = None;
    } else {
        endpoint.get_or_insert(displayed_end);
    }
}
fn protect_displayed_selection(
    live: &mut History,
    frozen: Option<&History>,
    selected: Option<&Selection>,
) {
    // A frozen choice can refer to an instance already omitted by the live
    // recorder. The model merges the frozen identity idempotently, enriching
    // known paths while retaining any already confirmed live exit time.
    if let Some(shown) = frozen {
        live.protect_selection_from(shown, selected);
    } else {
        live.protect_selection(selected);
    }
}
fn change_reading(r: &ChangeReading) -> String {
    r.value.map(signed).unwrap_or_else(|| "—".into())
}
fn clock(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
fn duration_label(seconds: u32) -> String {
    format!("{}m {:02}s", seconds / 60, seconds % 60)
}
fn range_label(range: Option<u32>) -> String {
    range
        .map(|n| format!("{}m", n / 60))
        .unwrap_or_else(|| "All retained history".into())
}
fn range(value: Option<u32>) -> Range {
    value.map(Range::Last).unwrap_or(Range::All)
}
fn stable_id(selection: &Selection) -> String {
    format!("{selection:?}")
}
fn sample_time(history: &History, at: u32) -> String {
    let wall = history
        .system
        .iter()
        .find(|s| s.at == at)
        .map(|s| wall_time(s.captured_unix_ms));
    wall.map_or_else(
        || format!("{} elapsed", clock(at)),
        |wall| format!("{wall} · {} elapsed", clock(at)),
    )
}
fn legend_row_heights() -> [i32; 3] {
    // Identity and the exact numeric reading each own one readable line.
    [36; 3]
}
fn wall_time(unix_ms: u64) -> String {
    unsafe {
        let ticks = unix_ms
            .saturating_mul(10000)
            .saturating_add(116444736000000000);
        let utc = FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        let mut local = zeroed();
        let mut st: SYSTEMTIME = zeroed();
        if FileTimeToLocalFileTime(&utc, &mut local) != 0
            && FileTimeToSystemTime(&local, &mut st) != 0
        {
            format!("{:02}:{:02}:{:02}", st.wHour, st.wMinute, st.wSecond)
        } else {
            "Unavailable".into()
        }
    }
}
fn open_clipboard(hwnd: HWND) -> io::Result<()> {
    for attempt in 0..5 {
        unsafe {
            if OpenClipboard(hwnd) != 0 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if attempt == 4 {
                return Err(io::Error::new(
                    error.kind(),
                    format!(
                        "OpenClipboard: {error}; open window {:?}; owner {:?}",
                        GetOpenClipboardWindow(),
                        GetClipboardOwner()
                    ),
                ));
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    unreachable!()
}
fn copy_clipboard(hwnd: HWND, text: &str) -> io::Result<()> {
    unsafe {
        open_clipboard(hwnd)?;
        let result = (|| {
            let encoded = wide(text);
            let memory = GlobalAlloc(GMEM_MOVEABLE, encoded.len() * 2);
            if memory.is_null() {
                return Err(io::Error::last_os_error());
            }
            let p = GlobalLock(memory);
            if p.is_null() {
                GlobalFree(memory);
                return Err(io::Error::last_os_error());
            }
            std::ptr::copy_nonoverlapping(encoded.as_ptr(), p.cast(), encoded.len());
            GlobalUnlock(memory);
            if EmptyClipboard() == 0 {
                GlobalFree(memory);
                return Err(io::Error::other(format!(
                    "EmptyClipboard: {}",
                    io::Error::last_os_error()
                )));
            }
            if SetClipboardData(13, memory).is_null() {
                let error = io::Error::last_os_error();
                GlobalFree(memory);
                return Err(io::Error::other(format!("SetClipboardData: {error}")));
            }
            Ok(())
        })();
        CloseClipboard();
        result
    }
}

struct AppClass {
    name: Vec<u16>,
    instance: HINSTANCE,
    large: HICON,
    small: HICON,
    registered: bool,
}
impl AppClass {
    unsafe fn new(instance: HINSTANCE) -> io::Result<Self> {
        unsafe {
            let mut class = Self {
                name: wide("MemMap.Native.Window"),
                instance,
                large: null_mut(),
                small: null_mut(),
                registered: false,
            };
            let dpi = GetDpiForSystem().max(96);
            // Win32 MAKEINTRESOURCE: an ordinal identifier, never dereferenced.
            let icon_resource = std::ptr::without_provenance::<u16>(1);
            // Owned images keep the two requested sizes distinct; LR_SHARED
            // caches by resource name and can return the first size for both.
            class.large = LoadImageW(
                instance,
                icon_resource,
                IMAGE_ICON,
                GetSystemMetricsForDpi(SM_CXICON, dpi),
                GetSystemMetricsForDpi(SM_CYICON, dpi),
                0,
            );
            if class.large.is_null() {
                return Err(io::Error::last_os_error());
            }
            class.small = LoadImageW(
                instance,
                icon_resource,
                IMAGE_ICON,
                GetSystemMetricsForDpi(SM_CXSMICON, dpi),
                GetSystemMetricsForDpi(SM_CYSMICON, dpi),
                0,
            );
            if class.small.is_null() {
                return Err(io::Error::last_os_error());
            }
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                lpszClassName: class.name.as_ptr(),
                hIcon: class.large,
                hIconSm: class.small,
                ..zeroed()
            };
            if RegisterClassExW(&wc) == 0 {
                return Err(io::Error::last_os_error());
            }
            class.registered = true;
            Ok(class)
        }
    }
}
impl Drop for AppClass {
    fn drop(&mut self) {
        unsafe {
            // Never release icons while a registered class still references them.
            if !self.registered || UnregisterClassW(self.name.as_ptr(), self.instance) != 0 {
                if !self.large.is_null() {
                    DestroyIcon(self.large);
                }
                if !self.small.is_null() {
                    DestroyIcon(self.small);
                }
            }
        }
    }
}
#[derive(Clone, Copy)]
struct Control {
    id: i32,
    hwnd: HWND,
}
#[derive(Clone, Copy)]
struct InspectorGeometry {
    width: i32,
    narrow: bool,
    identity_shift: i32,
    summary_top: i32,
    summary_row: i32,
    coverage: i32,
    metric: i32,
    range: i32,
    axis: i32,
    reset: i32,
    chart: i32,
}
#[derive(Clone, Copy, Default)]
struct Layout {
    table: RECT,
    overview: RECT,
    inspector: RECT,
    split: RECT,
    toolbar: RECT,
}
struct App {
    hwnd: HWND,
    inspector: HWND,
    table: HWND,
    controls: Vec<Control>,
    tooltips: HWND,
    tip_text: Vec<Vec<u16>>,
    history: History,
    frozen: Option<History>,
    overview_cache: Option<OverviewPlot>,
    summary_cache: Option<EntitySummary>,
    series_cache: Option<HistorySeries>,
    diagnostics_cache: Diagnostics,
    total_roots: usize,
    rows: Vec<TreeRow>,
    all_rows: Vec<TreeRow>,
    strings: Vec<[Vec<u16>; 6]>,
    selected: Option<Selection>,
    ids: BTreeMap<Selection, String>,
    expanded: BTreeSet<Selection>,
    grouping: Grouping,
    query: String,
    hover_row: Option<usize>,
    pending_query: Option<(String, Instant)>,
    sort: Sort,
    order_held: bool,
    show_exited: bool,
    compact: bool,
    change_seconds: u32,
    metric: Metric,
    follow: bool,
    history_end: Option<u32>,
    history_width: Option<u32>,
    chart_hover: Option<u32>,
    overview_hover: Option<u32>,
    highlighted_band: usize,
    prefs: Preferences,
    prefs_path: Option<PathBuf>,
    fonts: Fonts,
    native_fonts: Fonts,
    font_scale: u32,
    viewport: Viewport,
    layout: Layout,
    legend_height: i32,
    width: i32,
    height: i32,
    minimum_canvas_height: i32,
    dpi: u32,
    text_scale: f64,
    layout_busy: bool,
    syncing: bool,
    ins_scroll: i32,
    ins_height: i32,
    splitter_drag: bool,
    chart_drag: Option<(i32, u32)>,
    running: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    receiver: mpsc::Receiver<Result<Snapshot, String>>,
    launched: Instant,
    received_at: Instant,
    received_frame: bool,
    error: Option<String>,
    notice: String,
    notice_at: Instant,
    buffer: HDC,
    bitmap: HBITMAP,
    original_bitmap: HGDIOBJ,
    buffer_size: (i32, i32),
    row_images: HIMAGELIST,
    accessible: Arc<Mutex<AccessibleTable>>,
    provider: Option<AccessibilityProvider>,
    smoke: Option<PathBuf>,
    smoke_stage: u8,
    smoke_started: Instant,
    smoke_before: u32,
    smoke_display: u32,
    smoke_failures: Vec<String>,
    smoke_clipboard: String,
    smoke_geometry: Vec<String>,
    painted_rows: usize,
    smoke_selection: Option<Selection>,
}
impl App {
    unsafe fn new(smoke: Option<PathBuf>, preference_override: Option<PathBuf>) -> Self {
        let (sender, receiver) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let run = running.clone();
        let launched = Instant::now();
        let worker = thread::spawn(move || {
            while run.load(Ordering::Relaxed) {
                let result =
                    sampler::capture(launched.elapsed().as_secs().min(u32::MAX as u64) as u32)
                        .map_err(|e| e.to_string());
                if sender.send(result).is_err() {
                    break;
                }
                thread::park_timeout(Duration::from_secs(COLLECTION_SECONDS as u64));
            }
        });
        unsafe {
            Self::with_collection(
                smoke,
                preference_override,
                receiver,
                running,
                Some(worker),
                launched,
            )
        }
    }
    unsafe fn with_collection(
        smoke: Option<PathBuf>,
        preference_override: Option<PathBuf>,
        receiver: mpsc::Receiver<Result<Snapshot, String>>,
        running: Arc<AtomicBool>,
        worker: Option<thread::JoinHandle<()>>,
        launched: Instant,
    ) -> Self {
        let prefs_path = preference_override.or_else(|| {
            smoke
                .as_ref()
                .map(|p| p.join("preferences.ini"))
                .or_else(preferences_path)
        });
        let mut prefs = prefs_path
            .as_ref()
            .and_then(|p| Preferences::load(p).ok())
            .unwrap_or_default();
        for (width, minimum) in prefs
            .column_widths
            .iter_mut()
            .zip([160, 55, 80, 80, 80, 80])
        {
            *width = (*width).max(minimum);
        }
        let grouping = match prefs.grouping.as_str() {
            "path" => Grouping::Executable,
            "tree" => Grouping::Tree,
            "process" => Grouping::Process,
            "family" => Grouping::Family,
            _ => Grouping::Name,
        };
        unsafe {
            Self {
                hwnd: null_mut(),
                inspector: null_mut(),
                table: null_mut(),
                controls: Vec::new(),
                tooltips: null_mut(),
                tip_text: Vec::new(),
                history: History::default(),
                frozen: None,
                overview_cache: None,
                summary_cache: None,
                series_cache: None,
                diagnostics_cache: Diagnostics::default(),
                total_roots: 0,
                rows: Vec::new(),
                all_rows: Vec::new(),
                strings: Vec::new(),
                selected: None,
                ids: BTreeMap::new(),
                expanded: BTreeSet::new(),
                grouping,
                query: String::new(),
                hover_row: None,
                pending_query: None,
                sort: Sort::default(),
                order_held: false,
                show_exited: false,
                compact: prefs.compact,
                change_seconds: 300,
                metric: Metric::PrivateWorkingSet,
                follow: true,
                history_end: None,
                history_width: None,
                chart_hover: None,
                overview_hover: None,
                highlighted_band: 0,
                prefs,
                prefs_path,
                fonts: Fonts::new(96),
                native_fonts: Fonts::new(96),
                font_scale: 96,
                viewport: Viewport::new(1560, 1060, 96, 100, 0, 0),
                layout: Layout::default(),
                legend_height: 120,
                width: 1560,
                height: 1060,
                minimum_canvas_height: 600,
                dpi: 96,
                text_scale: 1.0,
                layout_busy: false,
                syncing: false,
                ins_scroll: 0,
                ins_height: 728,
                splitter_drag: false,
                chart_drag: None,
                running,
                worker,
                receiver,
                launched,
                received_at: Instant::now(),
                received_frame: false,
                error: None,
                notice: String::new(),
                notice_at: Instant::now(),
                buffer: null_mut(),
                bitmap: null_mut(),
                original_bitmap: null_mut(),
                buffer_size: (0, 0),
                row_images: 0,
                accessible: Arc::new(Mutex::new(AccessibleTable {
                    rows: Vec::new(),
                    sort: String::new(),
                    focused: None,
                    hwnd: null_mut(),
                })),
                provider: None,
                smoke,
                smoke_stage: 0,
                smoke_started: Instant::now(),
                smoke_before: 0,
                smoke_display: 0,
                smoke_failures: Vec::new(),
                smoke_clipboard: String::new(),
                smoke_geometry: Vec::new(),
                painted_rows: 0,
                smoke_selection: None,
            }
        }
    }
    fn instance_id(&self, s: &Selection) -> String {
        self.ids.get(s).cloned().unwrap_or_else(|| stable_id(s))
    }
    fn shown(&self) -> &History {
        self.frozen.as_ref().unwrap_or(&self.history)
    }
    fn control(&self, id: i32) -> HWND {
        self.controls
            .iter()
            .find(|c| c.id == id)
            .map_or(null_mut(), |c| c.hwnd)
    }
    fn say(&mut self, text: impl Into<String>) {
        self.notice = text.into();
        self.notice_at = Instant::now();
    }
    fn selected_summary(&self) -> Option<EntitySummary> {
        self.summary_cache.clone()
    }
    fn build_series(&self) -> Option<HistorySeries> {
        self.selected.as_ref().map(|s| {
            let mut h = dashboard::series(self.shown(), s, self.metric, Range::All);
            let end = if self.follow {
                self.shown().at
            } else {
                self.history_end.unwrap_or(self.shown().at)
            }
            .min(h.end);
            let start = self
                .history_width
                .or(self.prefs.history_range)
                .map_or(h.start, |r| end.saturating_sub(r).max(h.start))
                .min(end);
            h.points.retain(|p| p.at >= start && p.at <= end);
            h.start = start;
            h.end = end;
            h.current = h
                .points
                .last()
                .map(|p| p.reading.clone())
                .unwrap_or_default();
            h
        })
    }
    fn selected_series(&self) -> Option<HistorySeries> {
        self.series_cache.clone()
    }
    fn choose_history_range(&mut self, choice: Option<u32>) {
        let displayed_end = self
            .series_cache
            .as_ref()
            .map_or(self.shown().at, |series| series.end);
        choose_history_range(
            &mut self.prefs.history_range,
            &mut self.history_width,
            &mut self.history_end,
            self.follow,
            displayed_end,
            choice,
        );
    }
    fn refresh_rows(&mut self, explicit: bool) {
        let old_ids = std::mem::take(&mut self.ids);
        for (old, id) in old_ids {
            let canonical = self.shown().resolve_selection(&old);
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
        let old: BTreeMap<_, _> = self
            .all_rows
            .iter()
            .enumerate()
            .map(|(n, r)| (self.shown().resolve_selection(&r.selection), n))
            .collect();
        let mut next = dashboard::tree_rows(
            self.shown(),
            self.grouping,
            self.show_exited,
            &self.query,
            self.change_seconds,
            self.sort,
        );
        self.order_held =
            unsafe { !explicit && (GetFocus() == self.table || GetCapture() == self.table) };
        if self.order_held && !old.is_empty() {
            fn walk(
                parent: Option<&Selection>,
                rows: &[TreeRow],
                old: &BTreeMap<Selection, usize>,
                out: &mut Vec<TreeRow>,
            ) {
                let mut children: Vec<_> = rows
                    .iter()
                    .filter(|r| r.parent.as_ref() == parent)
                    .collect();
                children.sort_by_key(|r| old.get(&r.selection).copied().unwrap_or(usize::MAX));
                for row in children {
                    out.push(row.clone());
                    walk(Some(&row.selection), rows, old, out);
                }
            }
            let mut ordered = Vec::new();
            walk(None, &next, &old, &mut ordered);
            next = ordered;
        }
        for row in &next {
            self.ids
                .entry(row.selection.clone())
                .or_insert_with(|| stable_id(&row.selection));
        }
        self.all_rows = next;
        self.rows = self
            .all_rows
            .iter()
            .filter(|r| {
                if !self.query.is_empty() {
                    return true;
                }
                let mut parent = r.parent.as_ref();
                while let Some(p) = parent {
                    if !self.expanded.contains(p) {
                        return false;
                    }
                    parent = self
                        .all_rows
                        .iter()
                        .find(|candidate| &candidate.selection == p)
                        .and_then(|r| r.parent.as_ref());
                }
                true
            })
            .cloned()
            .collect();
        if self.selected.is_none() {
            self.selected = self.rows.first().map(|r| r.selection.clone());
        }
        self.strings = self
            .rows
            .iter()
            .map(|r| std::array::from_fn(|column| wide(&self.cell_text(r, column))))
            .collect();
        self.protect_selected();
    }
    fn cell_text(&self, row: &TreeRow, column: usize) -> String {
        match column {
            0 => format!(
                "{}{}{}{}",
                if row.has_children {
                    if self.expanded.contains(&row.selection) || !self.query.is_empty() {
                        "▾ "
                    } else {
                        "▸ "
                    }
                } else {
                    ""
                },
                identity_text(row),
                if row.pid.is_none() {
                    format!(
                        "  {} {}",
                        row.count,
                        if row.count == 1 {
                            "process"
                        } else {
                            "processes"
                        }
                    )
                } else {
                    String::new()
                } + &if row.matching_count < row.count {
                    format!(
                        " · {} matching of {} processes",
                        row.matching_count, row.count
                    )
                } else {
                    String::new()
                },
                if row.lifecycle == Lifecycle::Exited {
                    "  Exited"
                } else {
                    ""
                }
            ),
            1 => row.pid.map_or_else(|| "—".into(), |p| p.to_string()),
            2 => reading(&row.private),
            3 => reading(&row.peak),
            4 => reading(&row.commit),
            _ => change_reading(&row.change),
        }
    }
    fn row_description(&self, row: &TreeRow) -> String {
        let exact_change = format!(
            "{} ({} raw bytes)",
            self.cell_text(row, 5),
            row.change
                .value
                .map_or_else(|| "unavailable".into(), |n| format!("{n:+}"))
        );
        format!(
            "{}; level {}; {}; PID {}; Private working set {}; Private commit {}; Peak private working set last 30 minutes {}; Change over {} minutes {}; {}; {}",
            self.cell_text(row, 0),
            row.depth + 1,
            if row.has_children {
                if self.expanded.contains(&row.selection) {
                    "Expanded"
                } else {
                    "Collapsed"
                }
            } else {
                "Leaf process"
            },
            self.cell_text(row, 1),
            self.cell_text(row, 2),
            self.cell_text(row, 4),
            self.cell_text(row, 3),
            self.change_seconds / 60,
            exact_change,
            row.detail,
            self.coverage_description(row)
        )
    }
    fn coverage_description(&self, row: &TreeRow) -> String {
        let mut s = format!(
            "{} of {} processes measured",
            row.private.measured, row.private.total
        );
        if row.private.partial() {
            s.push_str("; Partial");
        }
        for (label, value) in [
            ("Private commit", &row.commit),
            ("Peak private working set", &row.peak),
        ] {
            if value.partial() {
                s.push_str(&format!(
                    "; {label} Partial: {} of {} processes measured{}",
                    value.measured,
                    value.total,
                    if value.enumeration_complete {
                        ""
                    } else {
                        "; enumeration incomplete"
                    }
                ));
            }
        }
        for reason in [&row.private.reason, &row.peak.reason, &row.change.reason]
            .into_iter()
            .flatten()
        {
            s.push_str("; ");
            s.push_str(reason);
        }
        if row.matching_count < row.count {
            s.push_str(&format!(
                "; {} matching of {} processes",
                row.matching_count, row.count
            ));
        }
        if let Some(at) = row.exited_at {
            s.push_str(&format!(
                "; exited or last observed at {} elapsed",
                clock(at)
            ));
        }
        s
    }
    fn toggle(&mut self, selection: Selection, expand: Option<bool>) {
        let was = self.expanded.contains(&selection);
        if expand.unwrap_or(!was) {
            self.expanded.insert(selection);
        } else {
            self.expanded.remove(&selection);
        }
        self.refresh_rows(true);
    }
    fn select(&mut self, index: usize) {
        if let Some(row) = self.rows.get(index) {
            self.selected = Some(row.selection.clone());
            self.chart_hover = None;
        }
        self.protect_selected();
    }
    fn protect_selected(&mut self) {
        protect_displayed_selection(
            &mut self.history,
            self.frozen.as_ref(),
            self.selected.as_ref(),
        );
    }
    fn pause_display(&mut self) {
        self.protect_selected();
        let entering = self.frozen.take().is_none();
        if entering {
            self.frozen = Some(self.history.clone());
        }
        if !entering {
            self.refresh_rows(false);
        }
    }
    fn save_preferences(&mut self) {
        self.prefs.compact = self.compact;
        self.prefs.grouping = match self.grouping {
            Grouping::Name => "name",
            Grouping::Executable => "path",
            Grouping::Tree => "tree",
            Grouping::Process => "process",
            Grouping::Family => "family",
        }
        .into();
        if let Some(path) = &self.prefs_path
            && let Err(error) = self.prefs.save(path)
        {
            self.say(format!("Preferences: {error}"));
        }
    }
    fn collect(&mut self) -> bool {
        let mut received = false;
        while let Ok(result) = self.receiver.try_recv() {
            received = true;
            self.received_at = Instant::now();
            self.received_frame = true;
            self.protect_selected();
            match result {
                Ok(frame) => {
                    self.error = frame
                        .system_error
                        .clone()
                        .or(frame.collection_error.clone());
                    self.history.observe(frame);
                }
                Err(error) => {
                    self.error = Some(error.clone());
                    let at = self.launched.elapsed().as_secs().min(u32::MAX as u64) as u32;
                    self.history.observe(Snapshot {
                        at,
                        captured_unix_ms: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                        complete_scan: false,
                        system_error: Some(error),
                        ..Snapshot::default()
                    });
                }
            }
            if self.frozen.is_none() {
                self.refresh_rows(false);
            }
        }
        received
    }
}
unsafe fn set_text(hwnd: HWND, text: &str) {
    unsafe {
        if !hwnd.is_null() {
            let mut buffer = vec![0u16; text.encode_utf16().count() + 2];
            let current = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
            if String::from_utf16_lossy(&buffer[..current as usize]) != text {
                SetWindowTextW(hwnd, wide(text).as_ptr());
            }
        }
    }
}
unsafe fn check(hwnd: HWND, value: bool) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        if style & BS_TYPEMASK as u32 == BS_AUTORADIOBUTTON as u32 {
            SetWindowLongPtrW(
                hwnd,
                GWL_STYLE,
                if value {
                    style | WS_TABSTOP
                } else {
                    style & !WS_TABSTOP
                } as isize,
            );
        }
        SendMessageW(
            hwnd,
            BM_SETCHECK,
            if value { BST_CHECKED } else { BST_UNCHECKED } as usize,
            0,
        );
        InvalidateRect(hwnd, null(), 0);
    }
}
unsafe fn create_control(
    ptr: *mut App,
    parent: HWND,
    id: i32,
    class: &str,
    text: &str,
    style: u32,
) -> HWND {
    unsafe {
        if let Some(label) = match id {
            ID_FILTER => Some("Filter:"),
            ID_CHANGE => Some("Change over:"),
            ID_OVERVIEW_RANGE => Some("View last:"),
            ID_PATH => Some("Executable path:"),
            ID_METRIC => Some("Metric"),
            ID_RANGE => Some("View last"),
            ID_OVERVIEW_VALUES => Some("Chart data:"),
            ID_HISTORY_VALUES => Some("History data:"),
            ID_COVERAGE => Some("Info:"),
            _ => None,
        } {
            create_control(ptr, parent, id + 1000, "STATIC", label, 0);
        }
        let style = if class == "BUTTON" && style & BS_TYPEMASK as u32 == BS_PUSHBUTTON as u32 {
            (style & !(BS_TYPEMASK as u32)) | BS_OWNERDRAW as u32
        } else {
            style
        };
        let hwnd = CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            1,
            1,
            parent,
            id as usize as HMENU,
            GetModuleHandleW(null()),
            null(),
        );
        (*ptr).controls.push(Control { id, hwnd });
        SendMessageW(
            hwnd,
            WM_SETFONT,
            if id == ID_TITLE {
                (*ptr).native_fonts.panel
            } else if (ID_SUMMARY..ID_SUMMARY + 4).contains(&id) {
                (*ptr).native_fonts.bold
            } else {
                (*ptr).native_fonts.normal
            } as usize,
            1,
        );
        if class == "BUTTON" {
            SetWindowSubclass(hwnd, Some(native_control_proc), 1, 0);
        }
        if class == "COMBOBOX" {
            SetWindowSubclass(hwnd, Some(combo_proc), 1, 0);
        }
        if class == "EDIT" {
            SetWindowSubclass(hwnd, Some(edit_proc), 1, 0);
        }
        hwnd
    }
}
unsafe fn create_combo(
    ptr: *mut App,
    parent: HWND,
    id: i32,
    labels: &[&str],
    selected: usize,
) -> HWND {
    unsafe {
        let hwnd = create_control(
            ptr,
            parent,
            id,
            "COMBOBOX",
            "",
            WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
        );
        for label in labels {
            SendMessageW(hwnd, CB_ADDSTRING, 0, wide(label).as_ptr() as isize);
        }
        SendMessageW(hwnd, CB_SETCURSEL, selected, 0);
        SendMessageW(
            hwnd,
            CB_SETITEMHEIGHT,
            usize::MAX,
            (28 * (*ptr).viewport.scale / 96) as isize,
        );
        SendMessageW(
            hwnd,
            CB_SETITEMHEIGHT,
            0,
            (30 * (*ptr).viewport.scale / 96) as isize,
        );
        hwnd
    }
}
unsafe fn create_controls(hwnd: HWND, ptr: *mut App) {
    unsafe {
        InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_PROGRESS_CLASS | ICC_STANDARD_CLASSES,
        });
        create_control(
            ptr,
            hwnd,
            ID_SYSTEM,
            "STATIC",
            "Waiting for system memory…",
            0,
        );
        create_control(
            ptr,
            hwnd,
            ID_PROGRESS,
            "msctls_progress32",
            "System RAM used",
            0,
        );
        create_control(ptr, hwnd, ID_LIVE, "STATIC", "Collecting first sample…", 0);
        create_control(
            ptr,
            hwnd,
            ID_PAUSE,
            "BUTTON",
            "Pause display",
            WS_TABSTOP | BS_PUSHBUTTON as u32,
        );
        create_control(
            ptr,
            hwnd,
            ID_EXPORT,
            "BUTTON",
            "Export CSV",
            WS_TABSTOP | BS_PUSHBUTTON as u32,
        );
        for (i, label) in [
            "Name",
            "Path",
            "Process tree",
            "Individual processes",
            "Family",
        ]
        .iter()
        .enumerate()
        {
            create_control(
                ptr,
                hwnd,
                ID_GROUP + i as i32,
                "BUTTON",
                label,
                BS_AUTORADIOBUTTON as u32
                    | BS_PUSHLIKE as u32
                    | if i == 0 { WS_GROUP | WS_TABSTOP } else { 0 },
            );
        }
        let edit = create_control(
            ptr,
            hwnd,
            ID_FILTER,
            "EDIT",
            "",
            WS_TABSTOP | WS_GROUP | ES_AUTOHSCROLL as u32 | WS_BORDER,
        );
        SendMessageW(
            edit,
            EM_SETCUEBANNER,
            1,
            wide("Name, path or PID").as_ptr() as isize,
        );
        SendMessageW(edit, EM_SETLIMITTEXT, 2048, 0);
        create_control(
            ptr,
            hwnd,
            ID_CLEAR,
            "BUTTON",
            "×",
            WS_TABSTOP | BS_PUSHBUTTON as u32,
        );
        create_control(
            ptr,
            hwnd,
            ID_EXITED,
            "BUTTON",
            "Show exited processes",
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
        );
        create_control(
            ptr,
            hwnd,
            ID_COMPACT,
            "BUTTON",
            "Compact rows",
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
        );
        create_combo(
            ptr,
            hwnd,
            ID_CHANGE,
            &["1 minute", "5 minutes", "15 minutes", "30 minutes"],
            1,
        );
        let table = create_control(
            ptr,
            hwnd,
            ID_TABLE,
            "SysListView32",
            "Processes",
            WS_TABSTOP | LVS_REPORT | LVS_OWNERDATA | LVS_SINGLESEL | LVS_SHOWSELALWAYS | WS_BORDER,
        );
        (*ptr).table = table;
        SendMessageW(
            table,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            0,
            (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER | LVS_EX_LABELTIP) as isize,
        );
        SendMessageW(table, LVM_SETBKCOLOR, 0, PANEL as isize);
        SendMessageW(table, LVM_SETTEXTBKCOLOR, 0, PANEL as isize);
        SendMessageW(table, LVM_SETTEXTCOLOR, 0, INK as isize);
        SetWindowTheme(table, wide("").as_ptr(), wide("").as_ptr());
        for (i, name) in [
            "Executable / processes",
            "PID",
            "Private working set",
            "Peak (last 30m)",
            "Private commit",
            "Change (5m)",
        ]
        .iter()
        .enumerate()
        {
            let mut text = wide(name);
            let col = LVCOLUMNW {
                mask: LVCF_TEXT | LVCF_WIDTH | LVCF_FMT,
                fmt: if i == 0 { LVCFMT_LEFT } else { LVCFMT_RIGHT },
                cx: (*ptr).prefs.column_widths[i],
                pszText: text.as_mut_ptr(),
                ..zeroed()
            };
            SendMessageW(
                table,
                LVM_INSERTCOLUMNW,
                i,
                (&col as *const LVCOLUMNW) as isize,
            );
        }
        SetWindowSubclass(table, Some(table_proc), 1, ptr as usize);
        if let Ok(mut state) = (*ptr).accessible.lock() {
            state.hwnd = table;
        }
        (*ptr).provider = Some(accessibility::install(table, (*ptr).accessible.clone()));
        create_combo(
            ptr,
            hwnd,
            ID_OVERVIEW_RANGE,
            &["5m", "15m", "30m", "All retained history"],
            range_index((*ptr).prefs.overview_range),
        );
        for i in 0..5 {
            create_control(
                ptr,
                hwnd,
                ID_LEGEND + i,
                "BUTTON",
                "Waiting for collection",
                WS_TABSTOP | BS_OWNERDRAW as u32,
            );
        }
        create_control(
            ptr,
            hwnd,
            ID_OVERVIEW_VALUES,
            "EDIT",
            "Overview values",
            WS_TABSTOP | ES_READONLY as u32 | ES_MULTILINE as u32,
        );
        let inspector = create_control(
            ptr,
            hwnd,
            0,
            "STATIC",
            "Selected-process inspector",
            WS_CLIPCHILDREN | WS_VSCROLL,
        );
        (*ptr).inspector = inspector;
        SetWindowLongPtrW(inspector, GWL_EXSTYLE, WS_EX_CONTROLPARENT as isize);
        SetWindowSubclass(inspector, Some(inspector_proc), 1, ptr as usize);
        create_control(ptr, inspector, ID_TITLE, "STATIC", "Selected process", 0);
        create_control(ptr, inspector, ID_COUNTS, "STATIC", "", 0);
        for (i, label) in [
            "Private working set",
            "Private commit",
            "Change over 5m",
            "Peak (last 30m)",
        ]
        .into_iter()
        .enumerate()
        {
            create_control(
                ptr,
                inspector,
                ID_SUMMARY + i as i32 + 1000,
                "STATIC",
                label,
                0,
            );
            create_control(ptr, inspector, ID_SUMMARY + i as i32, "STATIC", "", 2);
        }
        create_control(
            ptr,
            inspector,
            ID_COVERAGE,
            "EDIT",
            "",
            WS_TABSTOP | ES_READONLY as u32 | ES_AUTOHSCROLL as u32,
        );
        create_control(
            ptr,
            inspector,
            ID_PATH,
            "EDIT",
            "",
            WS_TABSTOP | ES_READONLY as u32 | ES_AUTOHSCROLL as u32 | WS_BORDER,
        );
        create_control(
            ptr,
            inspector,
            ID_COPY,
            "BUTTON",
            "Copy path",
            WS_TABSTOP | BS_PUSHBUTTON as u32,
        );
        create_control(
            ptr,
            inspector,
            ID_SHOW,
            "BUTTON",
            "Show processes",
            WS_TABSTOP | BS_PUSHBUTTON as u32,
        );
        create_combo(
            ptr,
            inspector,
            ID_METRIC,
            &["Private working set", "Private commit"],
            0,
        );
        create_control(ptr, inspector, ID_RANGE + 1000, "STATIC", "View last", 0);
        for (i, label) in ["5m", "15m", "30m", "All"].into_iter().enumerate() {
            create_control(
                ptr,
                inspector,
                ID_RANGE_SEG + i as i32,
                "BUTTON",
                label,
                BS_AUTORADIOBUTTON as u32
                    | BS_PUSHLIKE as u32
                    | if i == 0 { WS_TABSTOP | WS_GROUP } else { 0 },
            );
        }
        create_control(
            ptr,
            inspector,
            ID_ZERO,
            "BUTTON",
            "From zero",
            WS_TABSTOP | WS_GROUP | BS_AUTORADIOBUTTON as u32 | BS_PUSHLIKE as u32,
        );
        create_control(
            ptr,
            inspector,
            ID_FIT,
            "BUTTON",
            "Fit values",
            BS_AUTORADIOBUTTON as u32 | BS_PUSHLIKE as u32,
        );
        create_control(
            ptr,
            inspector,
            ID_RESET,
            "BUTTON",
            "Reset view",
            WS_GROUP | WS_TABSTOP | BS_PUSHBUTTON as u32,
        );
        create_control(
            ptr,
            inspector,
            ID_FOLLOW,
            "BUTTON",
            "Follow latest",
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
        );
        create_control(
            ptr,
            inspector,
            ID_HISTORY_VALUES,
            "EDIT",
            "Selected-process chart values",
            WS_TABSTOP | ES_READONLY as u32 | ES_MULTILINE as u32,
        );
        for i in 0..4 {
            create_control(ptr, hwnd, ID_FOOTER + i, "STATIC", "", 0);
        }
        create_control(ptr, hwnd, ID_ORDER, "STATIC", "", 0);
        create_control(
            ptr,
            hwnd,
            ID_DETAILS,
            "BUTTON",
            "Details",
            WS_TABSTOP | BS_PUSHBUTTON as u32,
        );
        create_control(
            ptr,
            hwnd,
            ID_ZOOM,
            "BUTTON",
            "View · 100%",
            WS_TABSTOP | BS_PUSHBUTTON as u32,
        );
        let tips = CreateWindowExW(
            WS_EX_TOPMOST,
            wide("tooltips_class32").as_ptr(),
            null(),
            WS_POPUP | TTS_ALWAYSTIP | TTS_NOPREFIX,
            0,
            0,
            0,
            0,
            hwnd,
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        );
        (*ptr).tooltips = tips;
        for (id, message) in [
            (
                ID_EXPORT,
                "Export the displayed snapshot's current filtered process instances, including offscreen rows, once each. Includes raw bytes and timestamp; expansion does not change scope.",
            ),
            (
                ID_PATH,
                "Full executable path. Focus and select this read-only field to inspect or copy its complete value.",
            ),
            (
                ID_GROUP + 4,
                "Family combines descendants up to Windows launcher boundaries. Process tree shows actual ancestry instead.",
            ),
            (
                ID_OVERVIEW_VALUES,
                "Overview values. Tab to a legend band, then use Left/Right to inspect recorded samples.",
            ),
            (
                ID_HISTORY_VALUES,
                "Selected history values. Use Left/Right for earlier/later recorded samples; chart wheel zooms time, Ctrl+wheel scales the application.",
            ),
        ] {
            let text = wide(message);
            (*ptr).tip_text.push(text);
            let text = (*ptr).tip_text.last_mut().unwrap();
            let tool = TTTOOLINFOW {
                cbSize: size_of::<TTTOOLINFOW>() as u32,
                uFlags: TTF_IDISHWND | TTF_SUBCLASS,
                hwnd,
                uId: (*ptr).control(id) as usize,
                lpszText: text.as_mut_ptr(),
                ..zeroed()
            };
            SendMessageW(
                tips,
                TTM_ADDTOOLW,
                0,
                (&tool as *const TTTOOLINFOW) as isize,
            );
        }
    }
}
fn range_index(value: Option<u32>) -> usize {
    match value {
        Some(300) => 0,
        Some(900) => 1,
        Some(1800) => 2,
        _ => 3,
    }
}
fn selected_range(index: isize) -> Option<u32> {
    match index {
        0 => Some(300),
        1 => Some(900),
        2 => Some(1800),
        _ => None,
    }
}
unsafe fn place(ptr: *mut App, id: i32, r: RECT, inspector: bool) {
    unsafe {
        let v = (*ptr).viewport;
        let hwnd = (*ptr).control(id);
        let (x, y) = if inspector {
            (
                r.left * v.scale as i32 / 96,
                (r.top - (*ptr).ins_scroll) * v.scale as i32 / 96,
            )
        } else {
            v.physical(r.left, r.top)
        };
        MoveWindow(
            hwnd,
            x,
            y,
            (r.right - r.left) * v.scale as i32 / 96,
            (r.bottom - r.top) * v.scale as i32 / 96,
            1,
        );
    }
}
unsafe fn table_column_minima(ptr: *mut App) -> [i32; 6] {
    unsafe {
        let dc = GetDC((*ptr).table);
        let old = SelectObject(dc, (*ptr).native_fonts.normal);
        let mut result = [0; 6];
        for (column, header) in [
            "Executable / processes",
            "PID",
            "Private working set",
            "Peak (last 30m)",
            "Private commit",
            "Change (30m)",
        ]
        .into_iter()
        .enumerate()
        {
            let mut extent = zeroed();
            let text = wide(&format!("{header} ↓"));
            GetTextExtentPoint32W(dc, text.as_ptr(), (text.len() - 1) as i32, &mut extent);
            let mut width = if column == 0 {
                160 * (*ptr).viewport.scale as i32 / 96
            } else {
                extent.cx
            };
            if column == 1 {
                GetTextExtentPoint32W(dc, wide("4294967295").as_ptr(), 10, &mut extent);
                width = width.max(extent.cx);
            }
            if column != 0 {
                for row in &(*ptr).rows {
                    let text = wide(&(*ptr).cell_text(row, column));
                    GetTextExtentPoint32W(dc, text.as_ptr(), (text.len() - 1) as i32, &mut extent);
                    width = width.max(extent.cx);
                }
            }
            result[column] = width + 18 * (*ptr).viewport.scale as i32 / 96;
        }
        SelectObject(dc, old);
        ReleaseDC((*ptr).table, dc);
        result
    }
}
unsafe fn fit_table_columns(ptr: *mut App) {
    unsafe {
        let mut client = zeroed();
        GetClientRect((*ptr).table, &mut client);
        let mut widths = table_column_minima(ptr);
        let mut spare = (client.right - widths.iter().sum::<i32>()).max(0);
        // Numeric minima win. Spend remaining space on identity, then bars.
        for column in [0, 2, 5, 3, 4, 1] {
            let desired = (*ptr).prefs.column_widths[column] * (*ptr).viewport.scale as i32 / 96;
            let extra = (desired - widths[column]).max(0).min(spare);
            widths[column] += extra;
            spare -= extra;
        }
        widths[0] += spare;
        for (column, width) in widths.into_iter().enumerate() {
            SendMessageW((*ptr).table, LVM_SETCOLUMNWIDTH, column, width as isize);
        }
    }
}
unsafe fn layout_window(hwnd: HWND, ptr: *mut App) {
    unsafe {
        if (*ptr).layout_busy {
            return;
        }
        (*ptr).layout_busy = true;
        for _ in 0..3 {
            let mut r = zeroed();
            GetClientRect(hwnd, &mut r);
            let requested_y = (*ptr).viewport.y;
            let mut v = Viewport::new(
                r.right,
                r.bottom,
                ((*ptr).dpi as f64 * (*ptr).text_scale).round() as u32,
                (*ptr).prefs.ui_zoom,
                (*ptr).viewport.x,
                (*ptr).viewport.y,
            );
            v.height = v.height.max((*ptr).minimum_canvas_height);
            v.y = requested_y.clamp(0, v.height - v.page_height);
            (*ptr).viewport = v;
            for (bar, length, page, pos) in [
                (SB_HORZ, v.width, v.page_width, v.x),
                (SB_VERT, v.height, v.page_height, v.y),
            ] {
                let info = SCROLLINFO {
                    cbSize: size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
                    nMin: 0,
                    nMax: length - 1,
                    nPage: page as u32,
                    nPos: pos,
                    nTrackPos: 0,
                };
                SetScrollInfo(hwnd, bar, &info, 1);
            }
        }
        let v = (*ptr).viewport;
        (*ptr).width = v.width;
        (*ptr).height = v.height;
        if (*ptr).font_scale != v.scale {
            let old = (*ptr).native_fonts;
            (*ptr).native_fonts = Fonts::new(v.scale);
            (*ptr).font_scale = v.scale;
            let controls = (*ptr).controls.clone();
            for control in controls {
                let font = if control.id == ID_TITLE {
                    (*ptr).native_fonts.panel
                } else if (ID_SUMMARY..ID_SUMMARY + 4).contains(&control.id) {
                    (*ptr).native_fonts.bold
                } else if (ID_SUMMARY + 1000..ID_SUMMARY + 1004).contains(&control.id) {
                    (*ptr).native_fonts.small
                } else {
                    (*ptr).native_fonts.normal
                };
                SendMessageW(control.hwnd, WM_SETFONT, font as usize, 1);
                if [ID_CHANGE, ID_OVERVIEW_RANGE, ID_METRIC].contains(&control.id) {
                    SendMessageW(
                        control.hwnd,
                        CB_SETITEMHEIGHT,
                        usize::MAX,
                        (28 * v.scale / 96) as isize,
                    );
                    SendMessageW(
                        control.hwnd,
                        CB_SETITEMHEIGHT,
                        0,
                        (30 * v.scale / 96) as isize,
                    );
                }
            }
            old.delete();
            for column in 0..6 {
                SendMessageW(
                    (*ptr).table,
                    LVM_SETCOLUMNWIDTH,
                    column,
                    ((*ptr).prefs.column_widths[column] * v.scale as i32 / 96) as isize,
                );
            }
        }
        let w = v.width;
        let h = v.height;
        let available = w - 44;
        let right_minimum = (v.page_width - 32).clamp(240, 300);
        let table_width = (available * (*ptr).prefs.splitter_permille as i32 / 1000)
            .clamp(380, available - right_minimum);
        let right = 16 + table_width + 12;
        let right_width = w - right - 16;
        let workspace_top = if w < 1040 { 128 } else { 80 };
        let footer_height = if w < 1040 { 82 } else { 50 };
        let workspace_height = h - workspace_top - footer_height;
        let legend_height = legend_row_heights().iter().sum::<i32>() + 12;
        (*ptr).legend_height = legend_height;
        let overview_extra = if right_width < 520 { 36 } else { 0 };
        let overview_chrome = 143 + overview_extra + legend_height;
        // This is an extreme-window fallback, not the inspector's full content
        // height. Ordinary windows keep their header/footer inside the client.
        let required_height = workspace_top + overview_chrome + 48 + 10 + 174 + footer_height;
        if (*ptr).minimum_canvas_height != required_height {
            (*ptr).minimum_canvas_height = required_height;
            (*ptr).layout_busy = false;
            layout_window(hwnd, ptr);
            return;
        }
        let overview_height = (workspace_height * 45 / 100)
            .max(130 + overview_chrome)
            .min(workspace_height - 184);
        let overview = rect(right, workspace_top, right_width, overview_height);
        let inspector = rect(
            right,
            overview.bottom + 10,
            right_width,
            h - overview.bottom - footer_height - 10,
        );
        (*ptr).layout = Layout {
            table: rect(16, 272, table_width, (h - 396).max(80)),
            overview,
            inspector,
            split: rect(16 + table_width + 2, workspace_top, 8, workspace_height),
            toolbar: rect(16, workspace_top, table_width, 132),
        };
        place(
            ptr,
            ID_SYSTEM,
            if w < 1040 {
                rect(16, 48, w - 32, 28)
            } else {
                rect(174, 8, 570, 26)
            },
            false,
        );
        place(
            ptr,
            ID_PROGRESS,
            if w < 1040 {
                rect(16, 80, w - 32, 10)
            } else {
                rect(754, 14, (w - 1080).max(110), 12)
            },
            false,
        );
        place(
            ptr,
            ID_LIVE,
            if w < 1040 {
                rect(16, 94, w - 32, 28)
            } else {
                rect(174, 40, w - 500, 34)
            },
            false,
        );
        place(
            ptr,
            ID_PAUSE,
            rect(w - 310, if w < 1040 { 10 } else { 40 }, 158, 34),
            false,
        );
        place(
            ptr,
            ID_EXPORT,
            rect(w - 142, if w < 1040 { 10 } else { 40 }, 126, 34),
            false,
        );
        let widths = [70, 66, 118, 158, 76];
        let mut x = 86;
        let mut group_y = workspace_top + 12;
        for (i, width) in widths.into_iter().enumerate() {
            if x + width > table_width + 4 {
                x = 28;
                group_y += 40;
            }
            place(ptr, ID_GROUP + i as i32, rect(x, group_y, width, 34), false);
            x += width + 2;
        }
        let filter_y = group_y + 44;
        let filter_width = table_width - 94;
        SendMessageW(
            (*ptr).control(ID_FILTER),
            EM_SETMARGINS,
            EC_LEFTMARGIN as usize,
            (28 * v.scale / 96) as isize,
        );
        place(ptr, ID_FILTER + 1000, rect(25, filter_y, 52, 34), false);
        place(
            ptr,
            ID_FILTER,
            rect(78, filter_y, filter_width - 36, 34),
            false,
        );
        place(
            ptr,
            ID_CLEAR,
            rect(78 + filter_width - 30, filter_y, 30, 34),
            false,
        );
        let mut options_y = filter_y + 44;
        x = 24;
        for (id, width) in [(ID_EXITED, 184), (ID_COMPACT, 137), (ID_CHANGE, 202)] {
            if x + width > table_width + 4 {
                x = 24;
                options_y += 38;
            }
            if id == ID_CHANGE {
                place(ptr, ID_CHANGE + 1000, rect(x, options_y, 82, 32), false);
                place(
                    ptr,
                    ID_CHANGE,
                    rect(x + 86, options_y, width - 86, 180),
                    false,
                );
            } else {
                place(ptr, id, rect(x, options_y, width, 32), false);
            }
            x += width + 4;
        }
        (*ptr).layout.toolbar.bottom = options_y + 40;
        (*ptr).layout.table.top = (*ptr).layout.toolbar.bottom + 8;
        (*ptr).layout.table.bottom = (h - footer_height - 62).max((*ptr).layout.table.top + 60);
        place(ptr, ID_TABLE, (*ptr).layout.table, false);
        fit_table_columns(ptr);
        place(
            ptr,
            ID_OVERVIEW_RANGE + 1000,
            rect(
                right + right_width - 188,
                workspace_top + 8 + overview_extra,
                62,
                30,
            ),
            false,
        );
        place(
            ptr,
            ID_OVERVIEW_RANGE,
            rect(
                right + right_width - 124,
                workspace_top + 8 + overview_extra,
                112,
                180,
            ),
            false,
        );
        let mut legend_top = overview.bottom - 40 - legend_height;
        let row_heights = legend_row_heights();
        for i in 0..5 {
            let cell = (right_width - 30) / 2;
            place(
                ptr,
                ID_LEGEND + i,
                rect(
                    right + 12 + (i % 2) * (cell + 6),
                    legend_top,
                    cell,
                    row_heights[(i / 2) as usize] - 2,
                ),
                false,
            );
            if i % 2 == 1 {
                legend_top += row_heights[(i / 2) as usize];
            }
        }
        place(
            ptr,
            ID_OVERVIEW_VALUES + 1000,
            rect(right + 12, overview.bottom - 30, 68, 25),
            false,
        );
        place(
            ptr,
            ID_OVERVIEW_VALUES,
            rect(right + 84, overview.bottom - 30, right_width - 96, 25),
            false,
        );
        let (x, y) = v.physical(inspector.left, inspector.top);
        MoveWindow(
            (*ptr).inspector,
            x,
            y,
            (inspector.right - inspector.left) * v.scale as i32 / 96,
            (inspector.bottom - inspector.top) * v.scale as i32 / 96,
            1,
        );
        layout_inspector(ptr);
        place(
            ptr,
            ID_ORDER,
            rect(18, (*ptr).layout.table.bottom + 2, table_width - 12, 21),
            false,
        );
        if w < 1040 {
            place(ptr, ID_FOOTER, rect(18, h - 80, w / 2 - 30, 22), false);
            place(
                ptr,
                ID_FOOTER + 1,
                rect(w / 2, h - 80, w / 2 - 18, 22),
                false,
            );
            place(ptr, ID_FOOTER + 2, rect(18, h - 50, w - 130, 22), false);
        } else {
            let cell = (w - 140) / 3;
            for i in 0..3 {
                place(
                    ptr,
                    ID_FOOTER + i,
                    rect(18 + i * cell, h - 50, cell - 8, 22),
                    false,
                );
            }
        }
        place(ptr, ID_FOOTER + 3, rect(18, h - 25, w - 180, 22), false);
        place(ptr, ID_DETAILS, rect(w - 98, h - 50, 76, 24), false);
        place(ptr, ID_ZOOM, rect(w - 142, h - 25, 126, 22), false);
        reset_row_height(ptr);
        sync_table(ptr);
        (*ptr).layout_busy = false;
        InvalidateRect(hwnd, null(), 0);
        InvalidateRect((*ptr).inspector, null(), 0);
    }
}
unsafe fn layout_inspector(ptr: *mut App) {
    unsafe {
        let r = (*ptr).layout.inspector;
        let visible = (r.bottom - r.top).max(1);
        (*ptr).ins_height = (*ptr).history_chart().bottom + 128;
        (*ptr).ins_scroll = (*ptr)
            .ins_scroll
            .clamp(0, ((*ptr).ins_height - visible).max(0));
        SetScrollInfo(
            (*ptr).inspector,
            SB_VERT,
            &SCROLLINFO {
                cbSize: size_of::<SCROLLINFO>() as u32,
                fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
                nMin: 0,
                nMax: (*ptr).ins_height - 1,
                nPage: visible as u32,
                nPos: (*ptr).ins_scroll,
                nTrackPos: 0,
            },
            1,
        );
        let mut client = zeroed();
        GetClientRect((*ptr).inspector, &mut client);
        let w = client.right * 96 / (*ptr).viewport.scale as i32;
        let g = (*ptr).inspector_geometry();
        place(ptr, ID_TITLE, rect(12, 8, w - 24, 32), true);
        place(
            ptr,
            ID_COUNTS,
            rect(12, 44, w - if g.narrow { 24 } else { 148 }, 31),
            true,
        );
        for i in 0..4 {
            place(
                ptr,
                ID_SUMMARY + i + 1000,
                rect(
                    12,
                    g.summary_top + i * g.summary_row,
                    w - if g.narrow { 24 } else { 184 },
                    if g.narrow { 18 } else { 23 },
                ),
                true,
            );
            place(
                ptr,
                ID_SUMMARY + i,
                if g.narrow {
                    rect(12, g.summary_top + i * g.summary_row + 18, w - 24, 23)
                } else {
                    rect(w - 168, g.summary_top + i * g.summary_row, 156, 23)
                },
                true,
            );
        }
        place(ptr, ID_COVERAGE + 1000, rect(12, g.coverage, 30, 18), true);
        place(ptr, ID_COVERAGE, rect(46, g.coverage, w - 58, 18), true);
        place(
            ptr,
            ID_SHOW,
            rect(w - 132, if g.narrow { 77 } else { 45 }, 120, 32),
            true,
        );
        place(
            ptr,
            ID_PATH + 1000,
            rect(12, 79 + g.identity_shift, w - 24, 20),
            true,
        );
        place(
            ptr,
            ID_PATH,
            rect(12, 103 + g.identity_shift, w - 170, 34),
            true,
        );
        place(
            ptr,
            ID_COPY,
            rect(w - 156, 103 + g.identity_shift, 144, 34),
            true,
        );
        place(ptr, ID_METRIC + 1000, rect(12, g.metric, 56, 34), true);
        place(ptr, ID_METRIC, rect(68, g.metric, w - 80, 180), true);
        place(ptr, ID_RANGE + 1000, rect(12, g.range, 56, 34), true);
        let range_width = (w - 80) / 4;
        for i in 0..4 {
            place(
                ptr,
                ID_RANGE_SEG + i,
                rect(68 + i * range_width, g.range, range_width - 2, 34),
                true,
            );
        }
        if w < 300 {
            place(ptr, ID_ZERO, rect(68, g.axis, w - 80, 32), true);
            place(ptr, ID_FIT, rect(68, g.axis + 38, w - 80, 32), true);
        } else {
            place(ptr, ID_ZERO, rect(68, g.axis, 108, 32), true);
            place(ptr, ID_FIT, rect(178, g.axis, w - 190, 32), true);
        }
        place(ptr, ID_RESET, rect(12, g.reset, 106, 32), true);
        place(
            ptr,
            ID_FOLLOW,
            if w < 255 {
                rect(12, g.reset + 38, w - 24, 32)
            } else {
                rect(130, g.reset, w - 142, 32)
            },
            true,
        );
        place(
            ptr,
            ID_HISTORY_VALUES + 1000,
            rect(12, (*ptr).history_chart().bottom + 60, w - 24, 18),
            true,
        );
        place(
            ptr,
            ID_HISTORY_VALUES,
            rect(12, (*ptr).history_chart().bottom + 80, w - 24, 38),
            true,
        );
    }
}
unsafe fn reset_row_height(ptr: *mut App) {
    unsafe {
        let height = (if (*ptr).compact { 34 } else { 50 }) * (*ptr).viewport.scale as i32 / 96;
        let image = ImageList_Create(1, height, ILC_COLOR32, 1, 0);
        ImageList_SetImageCount(image, 1);
        let old = SendMessageW(
            (*ptr).table,
            LVM_SETIMAGELIST,
            LVSIL_SMALL as usize,
            image as isize,
        ) as HIMAGELIST;
        if old != 0 {
            ImageList_Destroy(old);
        }
        (*ptr).row_images = image;
    }
}
unsafe fn sync_table(ptr: *mut App) {
    unsafe {
        if (*ptr).table.is_null() {
            return;
        }
        (*ptr).syncing = true;
        let table = (*ptr).table;
        SendMessageW(
            table,
            LVM_SETITEMCOUNT,
            (*ptr).rows.len(),
            (LVSICF_NOSCROLL | LVSICF_NOINVALIDATEALL) as isize,
        );
        let clear = LVITEMW {
            stateMask: LVIS_SELECTED | LVIS_FOCUSED,
            state: 0,
            ..zeroed()
        };
        SendMessageW(
            table,
            LVM_SETITEMSTATE,
            usize::MAX,
            (&clear as *const LVITEMW) as isize,
        );
        if let Some(index) = (*ptr)
            .selected
            .as_ref()
            .and_then(|s| (*ptr).rows.iter().position(|r| &r.selection == s))
        {
            let state = LVITEMW {
                stateMask: LVIS_SELECTED | LVIS_FOCUSED,
                state: LVIS_SELECTED | LVIS_FOCUSED,
                ..zeroed()
            };
            SendMessageW(
                table,
                LVM_SETITEMSTATE,
                index,
                (&state as *const LVITEMW) as isize,
            );
        }
        let labels = [
            "Executable / processes".to_string(),
            "PID".into(),
            "Private working set".into(),
            "Peak (last 30m)".into(),
            "Private commit".into(),
            format!("Change ({}m)", (*ptr).change_seconds / 60),
        ];
        let sortcol = match (*ptr).sort.column {
            SortColumn::Name => 0,
            SortColumn::Pid => 1,
            SortColumn::PrivateWorkingSet => 2,
            SortColumn::Peak => 3,
            SortColumn::PrivateCommit => 4,
            SortColumn::Change => 5,
        };
        for (i, label) in labels.iter().enumerate() {
            let mut text = wide(&format!(
                "{}{}",
                label,
                if i == sortcol {
                    if (*ptr).sort.descending {
                        " ↓"
                    } else {
                        " ↑"
                    }
                } else {
                    ""
                }
            ));
            let column = LVCOLUMNW {
                mask: LVCF_TEXT,
                pszText: text.as_mut_ptr(),
                ..zeroed()
            };
            SendMessageW(
                table,
                LVM_SETCOLUMNW,
                i,
                (&column as *const LVCOLUMNW) as isize,
            );
        }
        fit_table_columns(ptr);
        let mut origin = POINT { x: 0, y: 0 };
        ClientToScreen(table, &mut origin);
        let mut client = zeroed();
        GetClientRect(table, &mut client);
        let rows = (*ptr).rows.clone();
        let mut accessible_rows = Vec::with_capacity(rows.len());
        for (i, row) in rows.iter().enumerate() {
            let mut r = RECT {
                left: LVIR_BOUNDS as i32,
                ..zeroed()
            };
            SendMessageW(table, LVM_GETITEMRECT, i, (&mut r as *mut RECT) as isize);
            if r.bottom < 0 || r.top > client.bottom {
                r = rect(0, 0, 0, 0);
            } else {
                r.left += origin.x;
                r.right += origin.x;
                r.top += origin.y;
                r.bottom += origin.y;
            }
            accessible_rows.push(AccessibleRow {
                id: (*ptr).instance_id(&row.selection),
                parent: row.parent.as_ref().map(|s| (*ptr).instance_id(s)),
                name: (*ptr).cell_text(row, 0),
                value: (*ptr).row_description(row),
                depth: row.depth,
                has_children: row.has_children,
                expanded: (*ptr).expanded.contains(&row.selection)
                    || !String::is_empty(&(*ptr).query),
                selected: (*ptr).selected.as_ref() == Some(&row.selection),
                rect: r,
            });
        }
        let sort = format!(
            "{} {}",
            labels[sortcol],
            if (*ptr).sort.descending {
                "descending"
            } else {
                "ascending"
            }
        );
        let focused = if GetFocus() == table {
            (*ptr).selected.as_ref().map(|s| (*ptr).instance_id(s))
        } else {
            None
        };
        if let Ok(mut state) = (*ptr).accessible.lock() {
            state.rows = accessible_rows;
            state.sort = sort;
            state.focused = focused;
        }
        if let Some(provider) = &(*ptr).provider {
            provider.notify_changed();
        }
        (*ptr).syncing = false;
        InvalidateRect(table, null(), 0);
    }
}
unsafe fn sync_native(ptr: *mut App) {
    unsafe {
        if (*ptr).hwnd.is_null() {
            return;
        }
        let plot = dashboard::overview((*ptr).shown(), range((*ptr).prefs.overview_range));
        (*ptr).overview_cache = Some(plot.clone());
        (*ptr).summary_cache = (*ptr)
            .selected
            .as_ref()
            .and_then(|s| dashboard::summary((*ptr).shown(), s, (*ptr).change_seconds));
        (*ptr).series_cache = (*ptr).build_series();
        (*ptr).diagnostics_cache = dashboard::diagnostics((*ptr).shown(), GetCurrentProcessId());
        let latest = plot.samples.last();
        let header = latest
            .and_then(|s| {
                s.used.zip(s.available).map(|(used, available)| {
                    format!(
                        "System RAM used {} / {} ({:.0}%) · Available {}",
                        bytes(used),
                        bytes(s.capacity),
                        100.0 * used as f64 / s.capacity.max(1) as f64,
                        bytes(available)
                    )
                })
            })
            .unwrap_or_else(|| "System RAM used — · Available — · Collection unavailable".into());
        set_text((*ptr).control(ID_SYSTEM), &header);
        set_text((*ptr).control(ID_PROGRESS), &header);
        SendMessageW((*ptr).control(ID_PROGRESS), PBM_SETRANGE32, 0, 1000);
        SendMessageW(
            (*ptr).control(ID_PROGRESS),
            PBM_SETBARCOLOR,
            0,
            GREEN as isize,
        );
        SendMessageW(
            (*ptr).control(ID_PROGRESS),
            PBM_SETBKCOLOR,
            0,
            INPUT as isize,
        );
        let position = latest
            .and_then(|s| s.used.map(|u| u.saturating_mul(1000) / s.capacity.max(1)))
            .unwrap_or(0);
        SendMessageW(
            (*ptr).control(ID_PROGRESS),
            PBM_SETPOS,
            position as usize,
            0,
        );
        sync_recording_label(ptr);
        set_text(
            (*ptr).control(ID_PAUSE),
            if (*ptr).frozen.is_some() {
                "Resume display"
            } else {
                "Pause display"
            },
        );
        for (i, g) in [
            Grouping::Name,
            Grouping::Executable,
            Grouping::Tree,
            Grouping::Process,
            Grouping::Family,
        ]
        .into_iter()
        .enumerate()
        {
            check((*ptr).control(ID_GROUP + i as i32), (*ptr).grouping == g);
        }
        check((*ptr).control(ID_COMPACT), (*ptr).compact);
        check((*ptr).control(ID_EXITED), (*ptr).show_exited);
        check((*ptr).control(ID_ZERO), (*ptr).prefs.from_zero);
        check((*ptr).control(ID_FIT), !(*ptr).prefs.from_zero);
        check((*ptr).control(ID_FOLLOW), (*ptr).follow);
        for i in 0..4 {
            check(
                (*ptr).control(ID_RANGE_SEG + i),
                range_index((*ptr).prefs.history_range) == i as usize,
            );
        }
        ShowWindow(
            (*ptr).control(ID_CLEAR),
            if String::is_empty(&(*ptr).query) && (*ptr).pending_query.is_none() {
                SW_HIDE
            } else {
                SW_SHOW
            },
        );
        let summary = (*ptr).selected_summary();
        set_text(
            (*ptr).control(ID_TITLE),
            summary
                .as_ref()
                .map_or("Select a process or group", |s| s.name.as_str()),
        );
        let count_text = summary
            .as_ref()
            .map(|s| {
                if let Some(pid) = s.pid {
                    format!(
                        "PID {pid} · {:?}{}",
                        s.lifecycle,
                        s.exited_at
                            .map(|at| format!(" · at {}", clock(at)))
                            .unwrap_or_default()
                    )
                } else {
                    format!(
                        "{} {} · {} {}executable {}",
                        s.count,
                        if s.count == 1 { "process" } else { "processes" },
                        s.path_count,
                        if s.paths_complete { "" } else { "known " },
                        if s.path_count == 1 { "path" } else { "paths" }
                    )
                }
            })
            .unwrap_or_default();
        set_text((*ptr).control(ID_COUNTS), &count_text);
        if let Some(s) = &summary {
            for (i, value) in [
                reading(&s.private),
                reading(&s.commit),
                change_reading(&s.change),
                reading(&s.peak),
            ]
            .into_iter()
            .enumerate()
            {
                set_text((*ptr).control(ID_SUMMARY + i as i32), &value);
            }
        }
        let info = summary
            .as_ref()
            .map(|s| {
                let mut parts = Vec::new();
                if s.private.partial() {
                    parts.push(format!(
                        "Partial · {} of {} measured",
                        s.private.measured, s.private.total
                    ));
                }
                if !s.paths_complete {
                    parts.push("Path coverage incomplete; copying full known paths only".into());
                }
                for (label, value) in [("Commit", &s.commit), ("Peak", &s.peak)] {
                    if value.partial() {
                        parts.push(format!(
                            "{label} Partial · {} of {} measured",
                            value.measured, value.total
                        ));
                    }
                }
                if let Some(reason) = &s.change.reason {
                    parts.push(format!("Change: {reason}"));
                }
                if parts.is_empty() {
                    parts.push(format!(
                        "Peak: private working set · {} recorded",
                        duration_label(s.recorded)
                    ));
                }
                parts.join(" · ")
            })
            .unwrap_or_default();
        set_text((*ptr).control(ID_COVERAGE), &info);
        set_text(
            (*ptr).control(ID_SUMMARY + 2 + 1000),
            &format!("Change over {}m", (*ptr).change_seconds / 60),
        );
        let d = (*ptr).diagnostics_cache.clone();
        let roots = (*ptr).all_rows.iter().filter(|r| r.depth == 0).count();
        let noun = match (*ptr).grouping {
            Grouping::Name => "executable groups",
            Grouping::Executable => "path groups",
            Grouping::Family => "families",
            Grouping::Tree => "process trees",
            Grouping::Process => "processes",
        };
        let counts = if String::is_empty(&(*ptr).query) {
            format!("{} {noun}", (*ptr).total_roots)
        } else {
            format!("{roots} of {} {noun}", (*ptr).total_roots)
        };
        for (i, value) in [
            counts,
            format!("{} processes observed this session", d.observed_session),
            if d.current_enumeration_complete {
                format!("{} with unavailable data", d.current_unavailable)
            } else {
                format!(
                    "{} known unavailable · enumeration incomplete",
                    d.current_unavailable
                )
            },
            "Recording continues when display is paused".into(),
        ]
        .into_iter()
        .enumerate()
        {
            set_text((*ptr).control(ID_FOOTER + i as i32), &value);
        }
        let paths = summary
            .as_ref()
            .map(|s| s.paths.join("\r\n"))
            .unwrap_or_default();
        set_text((*ptr).control(ID_PATH), &paths);
        set_text(
            (*ptr).control(ID_PATH + 1000),
            if summary.as_ref().is_some_and(|s| !s.paths_complete) {
                "Known executable paths (incomplete):"
            } else {
                "Executable path:"
            },
        );
        set_text(
            (*ptr).control(ID_COPY),
            if summary.as_ref().is_some_and(|s| !s.paths_complete) {
                "Copy known paths"
            } else if summary.as_ref().is_some_and(|s| s.path_count > 1) {
                "Copy all paths"
            } else {
                "Copy path"
            },
        );
        EnableWindow((*ptr).control(ID_COPY), i32::from(!paths.is_empty()));
        ShowWindow(
            (*ptr).control(ID_SHOW),
            if summary.as_ref().is_some_and(|s| s.pid.is_none()) {
                SW_SHOW
            } else {
                SW_HIDE
            },
        );
        EnableWindow(
            (*ptr).control(ID_RESET),
            i32::from(
                !(*ptr).follow || (*ptr).history_end.is_some() || (*ptr).history_width.is_some(),
            ),
        );
        for (i, layer) in plot.layers.iter().enumerate().take(5) {
            let value = latest.and_then(|s| s.layers.as_ref().and_then(|v| v.get(i)).copied());
            let label = format!(
                "{}  {}",
                layer.name,
                value.map(bytes).unwrap_or_else(|| "—".into())
            );
            set_text((*ptr).control(ID_LEGEND + i as i32), &label);
        }
        let legend_height = legend_row_heights().iter().sum::<i32>() + 12;
        if legend_height != (*ptr).legend_height && !(*ptr).layout_busy {
            PostMessageW((*ptr).hwnd, MSG_LAYOUT, 0, 0);
        }
        update_chart_values(ptr, &plot);
        let hidden = (*ptr)
            .selected
            .as_ref()
            .is_some_and(|s| !(*ptr).rows.iter().any(|r| &r.selection == s));
        let status = match (hidden, (*ptr).order_held) {
            (true, true) => {
                "Selected item hidden by filter or grouping · Order held while inspecting"
            }
            (true, false) => "Selected item hidden by filter or grouping",
            (false, true) => "Order held while inspecting",
            _ => "",
        };
        set_text((*ptr).control(ID_ORDER), status);
        set_text(
            (*ptr).control(ID_ZOOM),
            &format!("View · {}%", (*ptr).prefs.ui_zoom),
        );
        sync_table(ptr);
        InvalidateRect((*ptr).hwnd, null(), 0);
        InvalidateRect((*ptr).inspector, null(), 0);
    }
}
unsafe fn update_chart_values(ptr: *mut App, plot: &OverviewPlot) {
    unsafe {
        let sample = (*ptr)
            .overview_hover
            .and_then(|at| plot.samples.iter().min_by_key(|s| s.at.abs_diff(at)))
            .or_else(|| plot.samples.last());
        let text = sample
            .map(|s| {
                let band = (*ptr)
                    .highlighted_band
                    .min(plot.layers.len().saturating_sub(1));
                let own = s.layers.as_ref().and_then(|v| v.get(band)).copied();
                format!(
                    "{} · {} {} · Used {} · Available {} · {} of {} measured{}",
                    sample_time((*ptr).shown(), s.at),
                    plot.layers
                        .get(band)
                        .map_or("System usage", |l| l.name.as_str()),
                    own.map(bytes)
                        .unwrap_or_else(|| "attribution unavailable".into()),
                    s.used.map(bytes).unwrap_or_else(|| "—".into()),
                    s.available.map(bytes).unwrap_or_else(|| "—".into()),
                    s.measured,
                    s.observed,
                    s.reason
                        .as_ref()
                        .map(|r| format!(" · {r}"))
                        .unwrap_or_default()
                )
            })
            .unwrap_or_else(|| "No completed system-memory samples".into());
        set_text((*ptr).control(ID_OVERVIEW_VALUES), &text);
        let selected = (*ptr).selected_series();
        let history_text = selected
            .as_ref()
            .map(|s| {
                let point = (*ptr)
                    .chart_hover
                    .and_then(|at| s.points.iter().min_by_key(|p| p.at.abs_diff(at)))
                    .or_else(|| s.points.last());
                point
                    .map(|p| {
                        format!(
                            "{} · {} {}{}",
                            sample_time((*ptr).shown(), p.at),
                            if (*ptr).metric == Metric::PrivateCommit {
                                "Private commit"
                            } else {
                                "Private working set"
                            },
                            reading(&p.reading),
                            p.reading
                                .reason
                                .as_ref()
                                .map(|r| format!(" · {r}"))
                                .unwrap_or_default()
                        )
                    })
                    .unwrap_or_else(|| "No observed selected-process samples in this view".into())
            })
            .unwrap_or_else(|| "Select a process or group to inspect history".into());
        set_text((*ptr).control(ID_HISTORY_VALUES), &history_text);
    }
}
impl App {
    fn inspector_geometry(&self) -> InspectorGeometry {
        let mut client = RECT::default();
        unsafe {
            GetClientRect(self.inspector, &mut client);
        }
        let width = (client.right * 96 / self.viewport.scale as i32).max(200);
        let narrow = width < 330;
        let identity_shift = if narrow { 38 } else { 0 };
        let summary_top = 149 + identity_shift;
        let summary_row = if narrow { 42 } else { 25 };
        let coverage = summary_top + 4 * summary_row + 4;
        let metric = coverage + 22;
        let range = metric + 41;
        let axis = range + 41;
        let reset = axis + if width < 300 { 79 } else { 41 };
        let chart = reset + 62 + if width < 255 { 38 } else { 0 };
        InspectorGeometry {
            width,
            narrow,
            identity_shift,
            summary_top,
            summary_row,
            coverage,
            metric,
            range,
            axis,
            reset,
            chart,
        }
    }
    fn overview_chart(&self) -> RECT {
        let r = self.layout.overview;
        let extra = if r.right - r.left < 520 { 36 } else { 0 };
        rect(
            r.left + 58,
            r.top + 76 + extra,
            (r.right - r.left) - 163,
            (r.bottom - r.top) - 143 - self.legend_height - extra,
        )
    }
    fn overview_caption_bounds(&self) -> RECT {
        let r = self.layout.overview;
        rect(r.left + 12, r.bottom - 52, r.right - r.left - 24, 18)
    }
    fn history_chart(&self) -> RECT {
        let g = self.inspector_geometry();
        let visible = self.layout.inspector.bottom - self.layout.inspector.top;
        rect(64, g.chart, g.width - 90, (visible - 74).clamp(100, 155))
    }
    fn paint(&mut self, c: &Canvas<'_>) {
        let w = self.width;
        let h = self.height;
        c.fill(rect(0, 0, w, h), BG);
        c.text(16, 12, 150, 38, "Mem Map", c.fonts.title, INK, false);
        c.line(
            &[
                POINT {
                    x: 16,
                    y: self.layout.toolbar.top - 4,
                },
                POINT {
                    x: w - 16,
                    y: self.layout.toolbar.top - 4,
                },
            ],
            DIVIDER,
            1,
        );
        c.panel(self.layout.toolbar);
        c.text(
            25,
            self.layout.toolbar.top + 12,
            60,
            34,
            "Group by",
            c.fonts.small,
            MUTED,
            false,
        );
        let split = self.layout.split;
        c.fill(
            rect(split.left + 3, split.top, 1, split.bottom - split.top),
            if self.splitter_drag { ORANGE } else { DIVIDER },
        );
        let strip_y = self.layout.table.bottom + 26;
        c.paragraph(
            rect(20, strip_y, self.layout.table.right - 24, 34),
            if matches!(self.grouping, Grouping::Tree | Grouping::Process) {
                "Memory bars: relative to largest process · Change bars: decrease ← 0 → increase"
            } else {
                "Memory bars: relative to largest group · Change bars: decrease ← 0 → increase"
            },
            MUTED,
        );
        self.paint_overview(c);
        if self.notice_at.elapsed() < Duration::from_secs(7) {
            c.text(
                242,
                h - 31,
                (w - 700).max(200),
                25,
                &self.notice,
                c.fonts.small,
                ORANGE,
                false,
            );
        }
    }
    fn paint_overview(&self, c: &Canvas<'_>) {
        let r = self.layout.overview;
        let extra = if r.right - r.left < 520 { 36 } else { 0 };
        c.panel(r);
        c.text(
            r.left + 12,
            r.top + 8,
            r.right - r.left - if extra == 0 { 204 } else { 24 },
            30,
            "Overall memory usage",
            c.fonts.panel,
            INK,
            false,
        );
        let Some(plot) = self.overview_cache.as_ref() else {
            return;
        };
        let last = plot.samples.last();
        let summary = last
            .and_then(|s| s.used.zip(s.available))
            .map(|(used, available)| {
                format!("{} used · {} available", bytes(used), bytes(available))
            })
            .unwrap_or_else(|| "System memory currently unavailable".into());
        c.text(
            r.left + 12,
            r.top + 40 + extra,
            r.right - r.left - 24,
            23,
            &summary,
            c.fonts.normal,
            INK,
            false,
        );
        let chart = self.overview_chart();
        if chart.bottom <= chart.top {
            return;
        }
        let capacity = plot.capacity.max(1);
        let xy = |at: u32, value: u64| POINT {
            x: chart.left
                + (at.saturating_sub(plot.start) as f64 / (plot.end - plot.start).max(1) as f64
                    * (chart.right - chart.left) as f64) as i32,
            y: chart.bottom
                - (value as f64 / capacity as f64 * (chart.bottom - chart.top) as f64) as i32,
        };
        for fraction in 0..=2 {
            let value = capacity * fraction / 2;
            let p = xy(plot.start, value);
            c.text(
                r.left + 3,
                p.y - 8,
                49,
                17,
                &format!("{:.1}", value as f64 / 1073741824.0),
                c.fonts.small,
                MUTED,
                true,
            );
            if fraction < 2 {
                c.line(
                    &[
                        p,
                        POINT {
                            x: chart.right,
                            y: p.y,
                        },
                    ],
                    DIVIDER,
                    1,
                );
            }
        }
        c.text(
            r.left + 8,
            chart.top - 20,
            40,
            18,
            "GiB",
            c.fonts.small,
            MUTED,
            false,
        );
        c.dashed(chart.top, chart.left, chart.right, BOUNDARY);
        c.text(
            chart.left,
            chart.top - 20,
            chart.right - chart.left + 85,
            18,
            &format!("Capacity: {}", bytes(capacity)),
            c.fonts.small,
            MUTED,
            false,
        );
        let mut start = 0;
        while start < plot.samples.len() {
            if plot.samples[start].used.is_none() {
                start += 1;
                continue;
            }
            let valid = plot.samples[start].layers.is_some();
            let mut end = start + 1;
            while end < plot.samples.len()
                && plot.samples[end].used.is_some()
                && plot.samples[end].layers.is_some() == valid
                && plot.samples[end]
                    .at
                    .saturating_sub(plot.samples[end - 1].at)
                    <= COLLECTION_SECONDS * 2
            {
                end += 1;
            }
            if valid {
                let mut boundaries = Vec::new();
                for band in 0..plot.layers.len() {
                    let upper: Vec<_> = plot.samples[start..end]
                        .iter()
                        .map(|s| xy(s.at, s.layers.as_ref().unwrap().iter().take(band + 1).sum()))
                        .collect();
                    let lower: Vec<_> = plot.samples[start..end]
                        .iter()
                        .rev()
                        .map(|s| xy(s.at, s.layers.as_ref().unwrap().iter().take(band).sum()))
                        .collect();
                    let polygon: Vec<_> = upper.iter().chain(lower.iter()).copied().collect();
                    c.polygon(&polygon, LAYERS[band.min(4)]);
                    if upper.len() == 1 {
                        c.line(&[lower[0], upper[0]], LAYERS[band.min(4)], 2);
                    }
                    boundaries.push(upper);
                }
                // Paint separators after all fills so the next layer's lower
                // edge cannot overwrite the shared cumulative boundary.
                for upper in boundaries {
                    c.line(&upper, CHART_SEPARATOR, 1);
                }
            } else {
                let mut polygon: Vec<_> = plot.samples[start..end]
                    .iter()
                    .map(|s| xy(s.at, s.used.unwrap()))
                    .collect();
                polygon.push(xy(plot.samples[end - 1].at, 0));
                polygon.push(xy(plot.samples[start].at, 0));
                c.polygon(&polygon, LAYERS[0]);
            }
            start = end;
        }
        if let Some(sample) = last
            && let Some(used) = sample.used
        {
            let endpoint = xy(sample.at, used);
            let annotations = overview_annotations(chart, endpoint.y);
            c.circle(endpoint.x, endpoint.y, 3, INK);
            let label_y = annotations.used.top + 12;
            if label_y != endpoint.y {
                c.line(
                    &[
                        POINT {
                            x: chart.right + 2,
                            y: endpoint.y,
                        },
                        POINT {
                            x: chart.right + 2,
                            y: label_y,
                        },
                        POINT {
                            x: annotations.used.left,
                            y: label_y,
                        },
                    ],
                    MUTED,
                    1,
                );
            }
            c.text(
                annotations.used.left,
                annotations.used.top,
                annotations.used.right - annotations.used.left,
                annotations.used.bottom - annotations.used.top,
                &bytes(used),
                c.fonts.bold,
                INK,
                false,
            );
            let bx = chart.right + 6;
            if endpoint.y > chart.top + 8 {
                c.line(
                    &[
                        POINT {
                            x: bx + 2,
                            y: chart.top + 3,
                        },
                        POINT {
                            x: bx + 8,
                            y: chart.top + 3,
                        },
                        POINT {
                            x: bx + 8,
                            y: endpoint.y - 4,
                        },
                        POINT {
                            x: bx + 2,
                            y: endpoint.y - 4,
                        },
                    ],
                    MUTED,
                    1,
                );
            }
            if let Some(available) = sample.available {
                c.text(
                    annotations.available_label.left,
                    annotations.available_label.top,
                    annotations.available_label.right - annotations.available_label.left,
                    annotations.available_label.bottom - annotations.available_label.top,
                    "Available",
                    c.fonts.small,
                    MUTED,
                    false,
                );
                c.text(
                    annotations.available_value.left,
                    annotations.available_value.top,
                    annotations.available_value.right - annotations.available_value.left,
                    annotations.available_value.bottom - annotations.available_value.top,
                    &bytes(available),
                    c.fonts.small,
                    INK,
                    false,
                );
            }
        }
        c.text(
            chart.left,
            chart.bottom + 3,
            70,
            18,
            &clock(plot.start),
            c.fonts.small,
            MUTED,
            false,
        );
        c.text(
            chart.right - 70,
            chart.bottom + 3,
            70,
            18,
            &clock(plot.end),
            c.fonts.small,
            MUTED,
            true,
        );
        let caption = self.overview_caption_bounds();
        c.text(
            caption.left,
            caption.top,
            caption.right - caption.left,
            caption.bottom - caption.top,
            &format!(
                "All processes + other used RAM · {} recorded",
                duration_label(plot.recorded)
            ),
            c.fonts.small,
            MUTED,
            false,
        );
        if let Some(at) = self.overview_hover
            && let Some(sample) = plot.samples.iter().min_by_key(|s| s.at.abs_diff(at))
        {
            let x = xy(sample.at, 0).x;
            c.line(
                &[POINT { x, y: chart.top }, POINT { x, y: chart.bottom }],
                INK,
                1,
            );
        }
    }
}
impl App {
    fn paint_inspector(&self, c: &Canvas<'_>) {
        let mut client = RECT::default();
        unsafe {
            GetClientRect(self.inspector, &mut client);
        }
        let w = client.right * 96 / self.viewport.scale as i32;
        c.fill(rect(0, 0, w, self.ins_height), PANEL);
        let g = self.inspector_geometry();
        c.text(12, g.metric, 56, 34, "Metric", c.fonts.small, MUTED, false);
        c.text(
            12,
            g.range,
            56,
            34,
            "View last",
            c.fonts.small,
            MUTED,
            false,
        );
        c.text(12, g.axis, 56, 32, "Y-axis", c.fonts.small, MUTED, false);
        let chart = self.history_chart();
        let series = self.selected_series();
        let valid: Vec<_> = series
            .as_ref()
            .map(|s| s.points.iter().filter_map(|p| p.reading.value).collect())
            .unwrap_or_default();
        let maximum = valid.iter().copied().max().unwrap_or(0);
        let minimum = valid.iter().copied().min().unwrap_or(0);
        let (low, high) = if self.prefs.from_zero {
            (0, (maximum as f64 * 1.1).max(1048576.0) as u64)
        } else {
            let padding = ((maximum.saturating_sub(minimum)) as f64 * 0.1).max(1048576.0) as u64;
            (
                minimum.saturating_sub(padding),
                maximum.saturating_add(padding),
            )
        };
        if let Some(series) = &series {
            let xy = |at: u32, value: u64| POINT {
                x: chart.left
                    + (at.saturating_sub(series.start) as f64
                        / (series.end - series.start).max(1) as f64
                        * (chart.right - chart.left) as f64) as i32,
                y: chart.bottom
                    - (value.saturating_sub(low) as f64 / (high - low).max(1) as f64
                        * (chart.bottom - chart.top) as f64) as i32,
            };
            for i in 0..=3 {
                let value = low + (high - low) * i / 3;
                let p = xy(series.start, value);
                c.line(
                    &[
                        p,
                        POINT {
                            x: chart.right,
                            y: p.y,
                        },
                    ],
                    DIVIDER,
                    1,
                );
                c.text(
                    1,
                    p.y - 9,
                    57,
                    18,
                    &format!("{:.1}", value as f64 / 1048576.0),
                    c.fonts.small,
                    MUTED,
                    true,
                );
            }
            c.text(
                8,
                chart.top - 22,
                52,
                18,
                "MiB",
                c.fonts.small,
                MUTED,
                false,
            );
            let mut segment = Vec::new();
            let mut prior = None;
            let draw = |points: &Vec<POINT>| {
                if points.is_empty() {
                    return;
                }
                let mut polygon = points.clone();
                polygon.push(POINT {
                    x: points.last().unwrap().x,
                    y: chart.bottom,
                });
                polygon.push(POINT {
                    x: points[0].x,
                    y: chart.bottom,
                });
                c.polygon(&polygon, rgb(48, 75, 61));
                c.line(points, GREEN, 2);
            };
            for p in &series.points {
                if (p.reading.value.is_none() || p.reading.partial())
                    || prior.is_some_and(|at: u32| p.at.saturating_sub(at) > COLLECTION_SECONDS * 2)
                {
                    draw(&segment);
                    segment.clear();
                }
                if let Some(value) = p.reading.value.filter(|_| !p.reading.partial()) {
                    segment.push(xy(p.at, value));
                }
                prior = Some(p.at);
            }
            draw(&segment);
            if let Some(p) = series.points.last()
                && let Some(value) = complete_history_value(&p.reading)
            {
                let endpoint = xy(p.at, value);
                c.circle(endpoint.x, endpoint.y, 3, GREEN);
                c.text(
                    (endpoint.x - 100).max(chart.left),
                    endpoint.y - 22,
                    96,
                    20,
                    &bytes(value),
                    c.fonts.bold,
                    GREEN,
                    true,
                );
            }
            if let Some(at) = self.chart_hover
                && let Some(point) = series.points.iter().min_by_key(|p| p.at.abs_diff(at))
            {
                let x = xy(point.at, 0).x;
                c.line(
                    &[POINT { x, y: chart.top }, POINT { x, y: chart.bottom }],
                    INK,
                    1,
                );
            }
            c.text(
                chart.left,
                chart.bottom + 3,
                72,
                18,
                &clock(series.start),
                c.fonts.small,
                MUTED,
                false,
            );
            c.text(
                chart.right - 72,
                chart.bottom + 3,
                72,
                18,
                &clock(series.end),
                c.fonts.small,
                MUTED,
                true,
            );
            c.text(
                12,
                chart.bottom + 27,
                w - 24,
                20,
                &format!(
                    "Requested: {} · Recorded: {}",
                    range_label(self.prefs.history_range),
                    duration_label(series.recorded)
                ),
                c.fonts.small,
                MUTED,
                false,
            );
        } else {
            c.text(
                chart.left,
                chart.top + 30,
                chart.right - chart.left,
                48,
                "Select a process to see observed history",
                c.fonts.normal,
                MUTED,
                false,
            );
        }
    }
}
unsafe fn paint_table(ptr: *mut App, draw: *mut NMLVCUSTOMDRAW) -> isize {
    unsafe {
        let stage = (*draw).nmcd.dwDrawStage;
        if stage == CDDS_PREPAINT {
            (*ptr).painted_rows = 0;
            return (CDRF_NOTIFYITEMDRAW | CDRF_NOTIFYPOSTPAINT) as isize;
        }
        if stage == CDDS_POSTPAINT {
            let saved = SaveDC((*draw).nmcd.hdc);
            let mut client = zeroed();
            GetClientRect((*ptr).table, &mut client);
            IntersectClipRect((*draw).nmcd.hdc, 0, 0, client.right, client.bottom);
            if (*ptr).rows.is_empty() {
                let fonts = (*ptr).native_fonts;
                let c = Canvas {
                    dc: (*draw).nmcd.hdc,
                    fonts: &fonts,
                };
                let mut r = zeroed();
                GetClientRect((*ptr).table, &mut r);
                c.text(
                    20,
                    50,
                    r.right - 40,
                    40,
                    "No matching processes",
                    fonts.normal,
                    MUTED,
                    false,
                );
            } else {
                // Paint after the native report body has completed. Item-stage
                // rectangles/clip regions vary across common-control versions;
                // the list's own item geometry also handles scrolling/density.
                let first = SendMessageW((*ptr).table, LVM_GETTOPINDEX, 0, 0).max(0) as usize;
                let count =
                    SendMessageW((*ptr).table, LVM_GETCOUNTPERPAGE, 0, 0).max(0) as usize + 2;
                for index in first..(first + count).min((*ptr).rows.len()) {
                    let mut bounds = RECT {
                        left: LVIR_BOUNDS as i32,
                        ..zeroed()
                    };
                    if SendMessageW(
                        (*ptr).table,
                        LVM_GETITEMRECT,
                        index,
                        (&mut bounds as *mut RECT) as isize,
                    ) != 0
                    {
                        draw_table_row(ptr, (*draw).nmcd.hdc, index, bounds);
                    }
                }
            }
            RestoreDC((*draw).nmcd.hdc, saved);
            return CDRF_DODEFAULT as isize;
        }
        if stage == CDDS_ITEMPREPAINT {
            CDRF_SKIPDEFAULT as isize
        } else {
            CDRF_DODEFAULT as isize
        }
    }
}
unsafe fn draw_table_row(ptr: *mut App, dc: HDC, index: usize, bounds: RECT) {
    unsafe {
        let Some(row) = Vec::as_slice(&(*ptr).rows).get(index).cloned() else {
            return;
        };
        let fonts = (*ptr).native_fonts;
        let c = Canvas { dc, fonts: &fonts };
        (*ptr).painted_rows += 1;
        let scale = (*ptr).viewport.scale as i32;
        let factor = |n: i32| n * scale / 96;
        let selected = (*ptr).selected.as_ref() == Some(&row.selection);
        c.fill(
            bounds,
            if selected {
                SELECTED
            } else if (*ptr).hover_row == Some(index) {
                HOVER
            } else {
                PANEL
            },
        );
        if selected {
            c.fill(
                rect(
                    bounds.left,
                    bounds.top,
                    factor(3),
                    bounds.bottom - bounds.top,
                ),
                ORANGE,
            );
        }
        let mut x = -GetScrollPos((*ptr).table, SB_HORZ);
        let denominator = (*ptr)
            .all_rows
            .iter()
            .filter(|r| {
                if matches!((*ptr).grouping, Grouping::Tree | Grouping::Process) {
                    r.pid.is_some()
                } else {
                    r.depth == 0
                }
            })
            .filter_map(|r| r.private.value)
            .max()
            .unwrap_or(1)
            .max(1);
        let changes = (*ptr)
            .all_rows
            .iter()
            .filter_map(|r| r.change.value.map(i64::unsigned_abs))
            .max()
            .unwrap_or(1)
            .max(1);
        for column in 0..6 {
            let width = SendMessageW((*ptr).table, LVM_GETCOLUMNWIDTH, column, 0) as i32;
            let padding = factor(8);
            let mut cell = rect(
                x + padding,
                bounds.top,
                width - 2 * padding,
                bounds.bottom - bounds.top,
            );
            let label = (*ptr).cell_text(&row, column);
            if column == 0 {
                let indent = factor((row.depth * 16) as i32);
                cell.left += indent;
                let line_h = if (*ptr).compact {
                    cell.bottom - cell.top
                } else {
                    factor(28)
                };
                if matches!(row.selection, Selection::App(_)) {
                    let old = SelectObject(c.dc, fonts.normal);
                    SetBkMode(c.dc, TRANSPARENT as i32);
                    SetTextColor(c.dc, INK);
                    let mut bounds = rect(cell.left, cell.top, cell.right - cell.left, line_h);
                    let text = wide(&label);
                    DrawTextW(
                        c.dc,
                        text.as_ptr(),
                        (text.len() - 1) as i32,
                        &mut bounds,
                        DT_SINGLELINE | DT_VCENTER | DT_PATH_ELLIPSIS | DT_NOPREFIX,
                    );
                    SelectObject(c.dc, old);
                } else {
                    c.text(
                        cell.left,
                        cell.top,
                        cell.right - cell.left,
                        line_h,
                        &label,
                        fonts.normal,
                        INK,
                        false,
                    );
                }
                if !(*ptr).compact {
                    c.text(
                        cell.left,
                        cell.top + factor(27),
                        cell.right - cell.left,
                        factor(20),
                        &row.detail,
                        fonts.small,
                        MUTED,
                        false,
                    );
                }
            } else if column == 2 {
                let old_font = SelectObject(c.dc, fonts.normal);
                let mut extent = zeroed();
                let text = wide(&label);
                GetTextExtentPoint32W(c.dc, text.as_ptr(), (text.len() - 1) as i32, &mut extent);
                SelectObject(c.dc, old_font);
                let numeric = factor(88).max(extent.cx).min(cell.right - cell.left);
                let bar_width = (cell.right - cell.left - numeric - factor(8)).max(0);
                if let Some(value) = row.private.value {
                    let filled = (value as f64 / denominator as f64 * bar_width as f64) as i32;
                    c.fill(
                        rect(
                            cell.left,
                            cell.top + (cell.bottom - cell.top) / 2 - factor(4),
                            filled,
                            factor(8),
                        ),
                        GREEN,
                    );
                }
                c.text(
                    cell.right - numeric,
                    cell.top,
                    numeric,
                    cell.bottom - cell.top,
                    &label,
                    fonts.normal,
                    INK,
                    true,
                );
            } else if column == 5 {
                let old_font = SelectObject(c.dc, fonts.normal);
                let mut extent = zeroed();
                let text = wide(&label);
                GetTextExtentPoint32W(c.dc, text.as_ptr(), (text.len() - 1) as i32, &mut extent);
                SelectObject(c.dc, old_font);
                let numeric = factor(96).max(extent.cx).min(cell.right - cell.left);
                let bar_width = (cell.right - cell.left - numeric - factor(6)).max(0);
                let zero = cell.left + bar_width / 2;
                c.line(
                    &[
                        POINT {
                            x: zero,
                            y: cell.top + factor(7),
                        },
                        POINT {
                            x: zero,
                            y: cell.bottom - factor(7),
                        },
                    ],
                    BOUNDARY,
                    1,
                );
                if let Some(value) = row.change.value {
                    let length = (value.unsigned_abs() as f64 / changes as f64
                        * (bar_width / 2) as f64) as i32;
                    c.fill(
                        rect(
                            if value < 0 { zero - length } else { zero },
                            cell.top + (cell.bottom - cell.top) / 2 - factor(4),
                            length,
                            factor(8),
                        ),
                        if value < 0 { GREEN } else { ORANGE },
                    );
                }
                c.text(
                    cell.right - numeric,
                    cell.top,
                    numeric,
                    cell.bottom - cell.top,
                    &label,
                    fonts.normal,
                    INK,
                    true,
                );
            } else {
                c.text(
                    cell.left,
                    cell.top,
                    cell.right - cell.left,
                    cell.bottom - cell.top,
                    &label,
                    fonts.normal,
                    INK,
                    true,
                );
            }
            x += width;
        }
        c.line(
            &[
                POINT {
                    x: bounds.left,
                    y: bounds.bottom - 1,
                },
                POINT {
                    x: bounds.right,
                    y: bounds.bottom - 1,
                },
            ],
            DIVIDER,
            1,
        );
        if selected && GetFocus() == (*ptr).table {
            let mut focus = bounds;
            InflateRect(&mut focus, -factor(4), -factor(3));
            DrawFocusRect(c.dc, &focus);
        }
    }
}
unsafe fn command(hwnd: HWND, ptr: *mut App, id: i32, notification: u32) {
    unsafe {
        if id == ID_FILTER && notification == EN_CHANGE {
            let mut text = vec![0u16; 2049];
            let n = GetWindowTextW((*ptr).control(ID_FILTER), text.as_mut_ptr(), 2049);
            (*ptr).pending_query = Some((
                String::from_utf16_lossy(&text[..n as usize]),
                Instant::now(),
            ));
            SetTimer(hwnd, FILTER_TIMER, 200, None);
            return;
        }
        if id == ID_CHANGE && notification == CBN_SELCHANGE {
            let index = SendMessageW((*ptr).control(id), CB_GETCURSEL, 0, 0);
            (*ptr).change_seconds = match index {
                0 => 60,
                2 => 900,
                3 => 1800,
                _ => 300,
            };
            (*ptr).refresh_rows(true);
        } else if [ID_RANGE, ID_OVERVIEW_RANGE, ID_METRIC].contains(&id)
            && notification == CBN_SELCHANGE
        {
            let index = SendMessageW((*ptr).control(id), CB_GETCURSEL, 0, 0);
            match id {
                ID_RANGE => (*ptr).choose_history_range(selected_range(index)),
                ID_OVERVIEW_RANGE => (*ptr).prefs.overview_range = selected_range(index),
                _ => {
                    (*ptr).metric = if index == 1 {
                        Metric::PrivateCommit
                    } else {
                        Metric::PrivateWorkingSet
                    }
                }
            }
        } else if notification == BN_CLICKED {
            match id {
                ID_PAUSE => (*ptr).pause_display(),
                ID_EXPORT => {
                    PostMessageW(hwnd, MSG_EXPORT, 0, 0);
                }
                ID_CLEAR => {
                    SetWindowTextW((*ptr).control(ID_FILTER), wide("").as_ptr());
                    (*ptr).pending_query = None;
                    KillTimer(hwnd, FILTER_TIMER);
                    (*ptr).query.clear();
                    (*ptr).refresh_rows(true);
                }
                ID_EXITED => {
                    (*ptr).show_exited =
                        SendMessageW((*ptr).control(id), BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
                    (*ptr).refresh_rows(true);
                }
                ID_COMPACT => {
                    let top = SendMessageW((*ptr).table, LVM_GETTOPINDEX, 0, 0);
                    (*ptr).compact =
                        SendMessageW((*ptr).control(id), BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
                    reset_row_height(ptr);
                    let newtop = SendMessageW((*ptr).table, LVM_GETTOPINDEX, 0, 0);
                    SendMessageW(
                        (*ptr).table,
                        LVM_SCROLL,
                        0,
                        ((top - newtop)
                            * (if (*ptr).compact { 34 } else { 50 })
                            * (*ptr).viewport.scale as isize
                            / 96) as isize,
                    );
                }
                ID_COPY => {
                    if let Some(s) = (*ptr).selected_summary() {
                        match copy_clipboard(hwnd, &s.paths.join("\r\n")) {
                            Ok(()) => (*ptr).say(if !s.paths_complete {
                                "Known paths copied · Path coverage incomplete"
                            } else if s.path_count > 1 {
                                "Paths copied"
                            } else {
                                "Path copied"
                            }),
                            Err(e) => (*ptr).say(format!("Copy failed: {e}")),
                        }
                    }
                }
                ID_SHOW => {
                    if let Some(s) = (*ptr).selected.clone() {
                        (*ptr).expanded.insert(s.clone());
                        let children: Vec<_> = (*ptr)
                            .all_rows
                            .iter()
                            .filter(|r| r.parent.as_ref() == Some(&s) && r.has_children)
                            .map(|r| r.selection.clone())
                            .collect();
                        (*ptr).expanded.extend(children);
                        (*ptr).refresh_rows(true);
                        sync_table(ptr);
                        if let Some(index) = (*ptr)
                            .rows
                            .iter()
                            .position(|r| r.parent.as_ref() == Some(&s))
                        {
                            SendMessageW((*ptr).table, LVM_ENSUREVISIBLE, index, 0);
                        }
                        SetFocus((*ptr).table);
                    }
                }
                ID_ZERO => (*ptr).prefs.from_zero = true,
                ID_FIT => (*ptr).prefs.from_zero = false,
                ID_FOLLOW => {
                    let follow =
                        SendMessageW((*ptr).control(id), BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
                    if (*ptr).follow && !follow {
                        (*ptr).history_end = Some(
                            (*ptr)
                                .selected_series()
                                .map_or((*ptr).shown().at, |s| s.end),
                        );
                    }
                    (*ptr).follow = follow;
                    if (*ptr).follow {
                        (*ptr).history_end = None;
                    }
                }
                ID_RESET => {
                    (*ptr).follow = true;
                    (*ptr).history_end = None;
                    (*ptr).history_width = None;
                    (*ptr).chart_hover = None;
                }
                ID_DETAILS => {
                    PostMessageW(hwnd, MSG_DETAILS, 0, 0);
                }
                value if (ID_RANGE_SEG..ID_RANGE_SEG + 4).contains(&value) => {
                    (*ptr).choose_history_range(selected_range((value - ID_RANGE_SEG) as isize));
                }
                ID_ZOOM => {
                    let menu = CreatePopupMenu();
                    for scale in [75, 100, 125, 150, 175, 200] {
                        AppendMenuW(
                            menu,
                            MF_STRING
                                | if scale == (*ptr).prefs.ui_zoom {
                                    MF_CHECKED
                                } else {
                                    0
                                },
                            (2000 + scale) as usize,
                            wide(&format!("{scale}%")).as_ptr(),
                        );
                    }
                    let mut r = zeroed();
                    GetWindowRect((*ptr).control(ID_ZOOM), &mut r);
                    TrackPopupMenu(
                        menu,
                        TPM_RIGHTALIGN | TPM_BOTTOMALIGN,
                        r.right,
                        r.top,
                        0,
                        hwnd,
                        null(),
                    );
                    DestroyMenu(menu);
                }
                value if (ID_GROUP..ID_GROUP + 5).contains(&value) => {
                    (*ptr).grouping = [
                        Grouping::Name,
                        Grouping::Executable,
                        Grouping::Tree,
                        Grouping::Process,
                        Grouping::Family,
                    ][(value - ID_GROUP) as usize];
                    (*ptr).refresh_rows(true);
                }
                value if (ID_LEGEND..ID_LEGEND + 5).contains(&value) => {
                    (*ptr).highlighted_band = (value - ID_LEGEND) as usize;
                }
                value if (2075..=2200).contains(&value) => {
                    (*ptr).prefs.ui_zoom = (value - 2000) as u32;
                    layout_window(hwnd, ptr);
                }
                _ => {}
            }
        }
        (*ptr).save_preferences();
        sync_native(ptr);
    }
}
unsafe fn table_key(ptr: *mut App, key: u16) {
    unsafe {
        let index = (*ptr)
            .selected
            .as_ref()
            .and_then(|s| (*ptr).rows.iter().position(|r| &r.selection == s));
        if let Some(index) = index {
            let row = (&(*ptr).rows)[index].clone();
            match key {
                VK_RIGHT => {
                    if row.has_children && !(*ptr).expanded.contains(&row.selection) {
                        (*ptr).toggle(row.selection, Some(true));
                    } else if Vec::as_slice(&(*ptr).rows)
                        .get(index + 1)
                        .is_some_and(|r| r.depth > row.depth)
                    {
                        (*ptr).select(index + 1);
                    }
                }
                VK_LEFT => {
                    if row.has_children && (*ptr).expanded.contains(&row.selection) {
                        (*ptr).toggle(row.selection, Some(false));
                    } else if let Some(parent) = row.parent {
                        (*ptr).selected = Some(parent);
                    }
                }
                VK_RETURN if row.has_children => {
                    (*ptr).toggle(row.selection, None);
                }
                _ => {}
            }
            sync_native(ptr);
            if let Some(index) = (*ptr)
                .selected
                .as_ref()
                .and_then(|s| (*ptr).rows.iter().position(|r| &r.selection == s))
            {
                SendMessageW((*ptr).table, LVM_ENSUREVISIBLE, index, 0);
            }
        }
    }
}
unsafe extern "system" fn native_control_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    unsafe {
        let control = GetDlgCtrlID(hwnd);
        let parent = GetAncestor(hwnd, GA_ROOT);
        match msg {
            WM_PAINT | WM_PRINTCLIENT => {
                let mut ps = zeroed();
                let dc = if msg == WM_PAINT {
                    BeginPaint(hwnd, &mut ps)
                } else {
                    wp as HDC
                };
                draw_button(hwnd, dc);
                if msg == WM_PAINT {
                    EndPaint(hwnd, &ps);
                }
                0
            }
            WM_ERASEBKGND => 1,
            WM_SETFOCUS | WM_KILLFOCUS => {
                InvalidateRect(hwnd, null(), 0);
                if msg == WM_SETFOCUS {
                    PostMessageW(parent, MSG_CONTROL_FOCUS, control as usize, 0);
                }
                DefSubclassProc(hwnd, msg, wp, lp)
            }
            WM_KEYDOWN
                if (ID_LEGEND..ID_LEGEND + 5).contains(&control)
                    && (wp as u16 == VK_LEFT || wp as u16 == VK_RIGHT) =>
            {
                PostMessageW(
                    parent,
                    MSG_CHART_KEY,
                    control as usize,
                    if wp as u16 == VK_LEFT { -1 } else { 1 },
                );
                0
            }
            BM_SETCHECK => {
                let r = DefSubclassProc(hwnd, msg, wp, lp);
                InvalidateRect(hwnd, null(), 0);
                r
            }
            _ => DefSubclassProc(hwnd, msg, wp, lp),
        }
    }
}
unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    unsafe {
        let id = GetDlgCtrlID(hwnd);
        if id == ID_FILTER && (msg == WM_PAINT || msg == WM_PRINTCLIENT) {
            let result = DefSubclassProc(hwnd, msg, wp, lp);
            let dc = if msg == WM_PAINT {
                GetDC(hwnd)
            } else {
                wp as HDC
            };
            let font = SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT;
            let fonts = Fonts {
                normal: font,
                small: font,
                bold: font,
                title: font,
                panel: font,
            };
            let c = Canvas { dc, fonts: &fonts };
            let mut r = zeroed();
            GetClientRect(hwnd, &mut r);
            if msg == WM_PRINTCLIENT {
                c.fill(r, INPUT);
                c.border(r, BOUNDARY);
                let mut text = vec![0u16; 2049];
                let len = GetWindowTextW(hwnd, text.as_mut_ptr(), 2049);
                let label = if len == 0 {
                    "Name, path or PID".to_string()
                } else {
                    String::from_utf16_lossy(&text[..len as usize])
                };
                c.text(
                    r.bottom,
                    0,
                    r.right - r.bottom - 8,
                    r.bottom,
                    &label,
                    font,
                    if len == 0 { MUTED } else { INK },
                    false,
                );
            }
            let size = r.bottom / 5;
            let x = r.bottom / 3;
            let y = r.bottom / 2;
            c.circle(x, y, size, BOUNDARY);
            c.circle(x, y, (size - 1).max(1), INPUT);
            c.line(
                &[
                    POINT {
                        x: x + size,
                        y: y + size,
                    },
                    POINT {
                        x: x + size + 3,
                        y: y + size + 3,
                    },
                ],
                MUTED,
                1,
            );
            if msg == WM_PAINT {
                ReleaseDC(hwnd, dc);
            }
            return result;
        }
        let parent = GetAncestor(hwnd, GA_ROOT);
        if id == ID_PATH && (msg == WM_PAINT || msg == WM_PRINTCLIENT) && GetFocus() != hwnd {
            let mut ps = zeroed();
            let dc = if msg == WM_PAINT {
                BeginPaint(hwnd, &mut ps)
            } else {
                wp as HDC
            };
            paint_path(hwnd, dc);
            if msg == WM_PAINT {
                EndPaint(hwnd, &ps);
            }
            return 0;
        }
        if id == ID_PATH && (msg == WM_SETFOCUS || msg == WM_KILLFOCUS) {
            InvalidateRect(hwnd, null(), 0);
        }
        if msg == WM_KEYDOWN
            && [ID_OVERVIEW_VALUES, ID_HISTORY_VALUES].contains(&id)
            && (wp as u16 == VK_LEFT || wp as u16 == VK_RIGHT)
        {
            PostMessageW(
                parent,
                MSG_CHART_KEY,
                id as usize,
                if wp as u16 == VK_LEFT { -1 } else { 1 },
            );
            return 0;
        }
        if msg == WM_SETFOCUS {
            PostMessageW(parent, MSG_CONTROL_FOCUS, id as usize, 0);
        }
        DefSubclassProc(hwnd, msg, wp, lp)
    }
}
unsafe extern "system" fn table_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    unsafe {
        let ptr = data as *mut App;
        if msg == WM_MOUSEMOVE {
            let mut hit = LVHITTESTINFO {
                pt: POINT {
                    x: lp as i16 as i32,
                    y: (lp >> 16) as i16 as i32,
                },
                ..zeroed()
            };
            let index = SendMessageW(
                hwnd,
                LVM_HITTEST,
                0,
                (&mut hit as *mut LVHITTESTINFO) as isize,
            );
            let next = if index < 0 {
                None
            } else {
                Some(index as usize)
            };
            if next != (*ptr).hover_row {
                (*ptr).hover_row = next;
                InvalidateRect(hwnd, null(), 0);
            }
            let mut tracking = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tracking);
        }
        if msg == WM_MOUSELEAVE {
            (*ptr).hover_row = None;
            InvalidateRect(hwnd, null(), 0);
        }
        if msg == WM_GETOBJECT
            && let Some(result) = (*ptr)
                .provider
                .as_ref()
                .and_then(|p| p.handle_wm_getobject(wp, lp))
        {
            return result;
        }
        if msg == WM_KEYDOWN && [VK_LEFT, VK_RIGHT, VK_RETURN].contains(&(wp as u16)) {
            table_key(ptr, wp as u16);
            return 0;
        }
        if msg == WM_SETFOCUS || msg == WM_KILLFOCUS {
            let result = DefSubclassProc(hwnd, msg, wp, lp);
            sync_table(ptr);
            return result;
        }
        if msg == WM_NOTIFY {
            let header = &*(lp as *const NMHDR);
            if !(*ptr).layout_busy
                && [HDN_ITEMCHANGINGW, HDN_ITEMCHANGINGA, HDN_TRACKW, HDN_TRACKA]
                    .contains(&header.code)
            {
                let change = &*(lp as *const NMHEADERW);
                if !change.pitem.is_null() && (*change.pitem).mask & HDI_WIDTH != 0 {
                    let min = table_column_minima(ptr)[change.iItem.clamp(0, 5) as usize];
                    if (*change.pitem).cxy < min {
                        return 1;
                    }
                }
            }
            if header.code == HDN_ENDTRACKW || header.code == HDN_ENDTRACKA {
                for column in 0..6 {
                    (*ptr).prefs.column_widths[column] =
                        (SendMessageW(hwnd, LVM_GETCOLUMNWIDTH, column, 0) as i32 * 96
                            / (*ptr).viewport.scale as i32)
                            .max(if column == 0 { 160 } else { 55 });
                }
                (*ptr).save_preferences();
            }
            if header.code == NM_CUSTOMDRAW {
                let draw = &*(lp as *const NMCUSTOMDRAW);
                if draw.dwDrawStage == CDDS_PREPAINT {
                    return CDRF_NOTIFYITEMDRAW as isize;
                }
                if draw.dwDrawStage == CDDS_ITEMPREPAINT {
                    let fonts = (*ptr).native_fonts;
                    let c = Canvas {
                        dc: draw.hdc,
                        fonts: &fonts,
                    };
                    c.fill(draw.rc, INPUT);
                    c.border(draw.rc, DIVIDER);
                    let mut text = vec![0u16; 128];
                    let mut item = HDITEMW {
                        mask: HDI_TEXT,
                        pszText: text.as_mut_ptr(),
                        cchTextMax: 128,
                        ..zeroed()
                    };
                    SendMessageW(
                        header.hwndFrom,
                        HDM_GETITEMW,
                        draw.dwItemSpec,
                        (&mut item as *mut HDITEMW) as isize,
                    );
                    let len = text.iter().position(|c| *c == 0).unwrap_or(text.len());
                    c.text(
                        draw.rc.left + 8,
                        draw.rc.top,
                        draw.rc.right - draw.rc.left - 16,
                        draw.rc.bottom - draw.rc.top,
                        &String::from_utf16_lossy(&text[..len]),
                        fonts.small,
                        MUTED,
                        draw.dwItemSpec > 0,
                    );
                    return CDRF_SKIPDEFAULT as isize;
                }
            }
        }
        DefSubclassProc(hwnd, msg, wp, lp)
    }
}
unsafe fn chart_sample_key(ptr: *mut App, id: i32, direction: isize) {
    unsafe {
        if id == ID_HISTORY_VALUES {
            if let Some(series) = (*ptr).selected_series() {
                let current = (*ptr).chart_hover.unwrap_or(series.end);
                let next = if direction < 0 {
                    series.points.iter().rev().find(|p| p.at < current)
                } else {
                    series.points.iter().find(|p| p.at > current)
                };
                if let Some(p) = next {
                    (*ptr).chart_hover = Some(p.at);
                }
            }
        } else {
            if (ID_LEGEND..ID_LEGEND + 5).contains(&id) {
                (*ptr).highlighted_band = (id - ID_LEGEND) as usize;
            }
            let plot = dashboard::overview((*ptr).shown(), range((*ptr).prefs.overview_range));
            let current = (*ptr).overview_hover.unwrap_or(plot.end);
            let next = if direction < 0 {
                plot.samples.iter().rev().find(|p| p.at < current)
            } else {
                plot.samples.iter().find(|p| p.at > current)
            };
            if let Some(p) = next {
                (*ptr).overview_hover = Some(p.at);
            }
        }
        sync_native(ptr);
    }
}
unsafe fn zoom_history(ptr: *mut App, inward: bool) {
    unsafe {
        let seconds = (*ptr)
            .history_width
            .or((*ptr).prefs.history_range)
            .unwrap_or(HISTORY_SECONDS);
        (*ptr).history_width = Some(zoom_time(seconds, inward));
        if (*ptr).follow {
            (*ptr).history_end = Some((*ptr).shown().at);
        }
        (*ptr).follow = false;
        (*ptr).save_preferences();
        sync_native(ptr);
    }
}
unsafe extern "system" fn inspector_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    unsafe {
        let ptr = data as *mut App;
        let parent = GetParent(hwnd);
        match msg {
            WM_COMMAND | WM_DRAWITEM | WM_NOTIFY | WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC
            | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => SendMessageW(parent, msg, wp, lp),
            WM_ERASEBKGND => 1,
            WM_PAINT | WM_PRINTCLIENT => {
                let mut ps = zeroed();
                let dc = if msg == WM_PAINT {
                    BeginPaint(hwnd, &mut ps)
                } else {
                    wp as HDC
                };
                let saved = SaveDC(dc);
                let mut origin = zeroed();
                GetViewportOrgEx(dc, &mut origin);
                let scale = (*ptr).viewport.scale;
                SetMapMode(dc, MM_ANISOTROPIC);
                SetWindowExtEx(dc, 96, 96, null_mut());
                SetViewportExtEx(dc, scale as i32, scale as i32, null_mut());
                SetViewportOrgEx(
                    dc,
                    origin.x,
                    origin.y - (*ptr).ins_scroll * scale as i32 / 96,
                    null_mut(),
                );
                let fonts = (*ptr).fonts;
                let canvas = Canvas { dc, fonts: &fonts };
                (*ptr).paint_inspector(&canvas);
                RestoreDC(dc, saved);
                if msg == WM_PAINT {
                    EndPaint(hwnd, &ps);
                }
                0
            }
            WM_VSCROLL => {
                let mut info = SCROLLINFO {
                    cbSize: size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_ALL,
                    ..zeroed()
                };
                GetScrollInfo(hwnd, SB_VERT, &mut info);
                (*ptr).ins_scroll = match (wp & 0xffff) as i32 {
                    SB_LINEUP => info.nPos - 32,
                    SB_LINEDOWN => info.nPos + 32,
                    SB_PAGEUP => info.nPos - info.nPage as i32,
                    SB_PAGEDOWN => info.nPos + info.nPage as i32,
                    SB_THUMBTRACK | SB_THUMBPOSITION => info.nTrackPos,
                    SB_TOP => 0,
                    SB_BOTTOM => info.nMax,
                    _ => info.nPos,
                };
                layout_inspector(ptr);
                InvalidateRect(hwnd, null(), 0);
                0
            }
            WM_MOUSEWHEEL => {
                let mut point = POINT {
                    x: lp as i16 as i32,
                    y: (lp >> 16) as i16 as i32,
                };
                ScreenToClient(hwnd, &mut point);
                let x = point.x * 96 / (*ptr).viewport.scale as i32;
                let y = point.y * 96 / (*ptr).viewport.scale as i32 + (*ptr).ins_scroll;
                let delta = ((wp >> 16) as i16) as i32;
                if contains((*ptr).history_chart(), x, y) {
                    zoom_history(ptr, delta > 0);
                } else {
                    (*ptr).ins_scroll -= delta / 120 * 64;
                    layout_inspector(ptr);
                    InvalidateRect(hwnd, null(), 0);
                }
                0
            }
            WM_LBUTTONDOWN => {
                let x = (lp as i16 as i32) * 96 / (*ptr).viewport.scale as i32;
                let y = ((lp >> 16) as i16 as i32) * 96 / (*ptr).viewport.scale as i32
                    + (*ptr).ins_scroll;
                if contains((*ptr).history_chart(), x, y) {
                    let end = if (*ptr).follow {
                        (*ptr).shown().at
                    } else {
                        (*ptr).history_end.unwrap_or((*ptr).shown().at)
                    };
                    (*ptr).chart_drag = Some((x, end));
                    (*ptr).follow = false;
                    (*ptr).history_end = Some(end);
                    SetCapture(hwnd);
                    SetFocus((*ptr).control(ID_HISTORY_VALUES));
                    sync_native(ptr);
                }
                0
            }
            WM_MOUSEMOVE => {
                let x = (lp as i16 as i32) * 96 / (*ptr).viewport.scale as i32;
                let y = ((lp >> 16) as i16 as i32) * 96 / (*ptr).viewport.scale as i32
                    + (*ptr).ins_scroll;
                let chart = (*ptr).history_chart();
                if let Some((origin, end)) = (*ptr).chart_drag {
                    let seconds = (*ptr)
                        .history_width
                        .or((*ptr).prefs.history_range)
                        .unwrap_or(HISTORY_SECONDS);
                    let offset = (origin - x) as i64 * seconds as i64
                        / (chart.right - chart.left).max(1) as i64;
                    (*ptr).history_end =
                        Some((end as i64 + offset).clamp(0, (*ptr).shown().at as i64) as u32);
                    sync_native(ptr);
                } else if contains(chart, x, y)
                    && let Some(series) = (*ptr).selected_series()
                {
                    (*ptr).chart_hover = Some(
                        series.start
                            + ((x - chart.left).max(0) as u64 * (series.end - series.start) as u64
                                / (chart.right - chart.left).max(1) as u64)
                                as u32,
                    );
                    let plot =
                        dashboard::overview((*ptr).shown(), range((*ptr).prefs.overview_range));
                    update_chart_values(ptr, &plot);
                    InvalidateRect(hwnd, null(), 0);
                }
                0
            }
            WM_LBUTTONUP => {
                (*ptr).chart_drag = None;
                ReleaseCapture();
                0
            }
            _ => DefSubclassProc(hwnd, msg, wp, lp),
        }
    }
}
unsafe fn native_notifications(ptr: *mut App, lp: LPARAM) -> Option<LRESULT> {
    unsafe {
        let header = &*(lp as *const NMHDR);
        if header.hwndFrom != (*ptr).table {
            return None;
        }
        match header.code {
            NM_CUSTOMDRAW => Some(paint_table(ptr, lp as *mut NMLVCUSTOMDRAW)),
            LVN_GETDISPINFOW => {
                let info = &mut *(lp as *mut NMLVDISPINFOW);
                if info.item.mask & LVIF_TEXT != 0
                    && let Some(text) = Vec::as_slice(&(*ptr).strings)
                        .get(info.item.iItem.max(0) as usize)
                        .and_then(|s| s.get(info.item.iSubItem.max(0) as usize))
                {
                    info.item.pszText = text.as_ptr() as *mut u16;
                }
                Some(0)
            }
            LVN_GETINFOTIPW => {
                let info = &mut *(lp as *mut NMLVGETINFOTIPW);
                if let Some(row) = Vec::as_slice(&(*ptr).rows).get(info.iItem.max(0) as usize) {
                    let text = wide(&(*ptr).row_description(row));
                    let count = (text.len() - 1).min(info.cchTextMax.saturating_sub(1) as usize);
                    std::ptr::copy_nonoverlapping(text.as_ptr(), info.pszText, count);
                    *info.pszText.add(count) = 0;
                }
                Some(0)
            }
            LVN_ITEMCHANGED => {
                if !(*ptr).syncing {
                    let item = &*(lp as *const NMLISTVIEW);
                    if item.iItem >= 0 && item.uNewState & LVIS_SELECTED != 0 {
                        (*ptr).select(item.iItem as usize);
                        sync_native(ptr);
                    }
                }
                Some(0)
            }
            LVN_COLUMNCLICK => {
                let item = &*(lp as *const NMLISTVIEW);
                let column = match item.iSubItem {
                    0 => SortColumn::Name,
                    1 => SortColumn::Pid,
                    2 => SortColumn::PrivateWorkingSet,
                    3 => SortColumn::Peak,
                    4 => SortColumn::PrivateCommit,
                    _ => SortColumn::Change,
                };
                (*ptr).sort = Sort {
                    column,
                    descending: if column == (*ptr).sort.column {
                        !(*ptr).sort.descending
                    } else {
                        column != SortColumn::Name && column != SortColumn::Pid
                    },
                };
                (*ptr).refresh_rows(true);
                sync_native(ptr);
                Some(0)
            }
            NM_CLICK | NM_DBLCLK => {
                let item = &*(lp as *const NMITEMACTIVATE);
                if item.iItem >= 0 {
                    let index = item.iItem as usize;
                    (*ptr).select(index);
                    if let Some(row) = Vec::as_slice(&(*ptr).rows).get(index).cloned() {
                        let scale = (*ptr).viewport.scale as i32;
                        let chevron = 8 * scale / 96 + row.depth as i32 * 16 * scale / 96
                            - GetScrollPos((*ptr).table, SB_HORZ);
                        if row.has_children
                            && (header.code == NM_DBLCLK
                                || (item.ptAction.x >= chevron
                                    && item.ptAction.x <= chevron + 20 * scale / 96))
                        {
                            (*ptr).toggle(row.selection, None);
                        }
                    }
                    sync_native(ptr);
                }
                Some(0)
            }
            _ => None,
        }
    }
}
unsafe fn export_dialog(hwnd: HWND, ptr: *mut App) {
    unsafe {
        let mut path = vec![0u16; 32768];
        let default = wide("mem-map-snapshot.csv");
        path[..default.len()].copy_from_slice(&default);
        let filter = wide("CSV snapshot (*.csv)\0*.csv\0");
        let extension = wide("csv");
        let mut ofn: OPENFILENAMEW = zeroed();
        ofn.lStructSize = size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = hwnd;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.lpstrFile = path.as_mut_ptr();
        ofn.nMaxFile = path.len() as u32;
        ofn.lpstrDefExt = extension.as_ptr();
        ofn.Flags = OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
        if GetSaveFileNameW(&mut ofn) != 0 {
            let end = path.iter().position(|c| *c == 0).unwrap_or(path.len());
            let path = PathBuf::from(String::from_utf16_lossy(&path[..end]));
            let result = File::create(&path).and_then(|f| {
                let mut out = BufWriter::new(f);
                dashboard::export_snapshot(
                    (*ptr).shown(),
                    (*ptr).grouping,
                    (*ptr).show_exited,
                    &(*ptr).query,
                    &mut out,
                )?;
                out.flush()
            });
            match result {
                Ok(()) => (*ptr).say(format!("Saved {}", path.display())),
                Err(e) => (*ptr).say(format!("Export failed: {e}")),
            }
            sync_native(ptr);
        }
    }
}
unsafe fn details_dialog(hwnd: HWND, ptr: *mut App) {
    unsafe {
        let d = dashboard::diagnostics((*ptr).shown(), GetCurrentProcessId());
        let last = d
            .last_success
            .map(|at| sample_time((*ptr).shown(), at))
            .unwrap_or_else(|| "None".into());
        let mut text = format!(
            "Current unavailable readings: {}\nProcesses observed this session: {}\nLast successful collection: {}\nRetained snapshots: {}\nOmitted tracks: {}\nMem Map private working set: {}\nMem Map total working set: {}\n",
            d.current_unavailable,
            d.observed_session,
            last,
            d.retained_snapshots,
            d.omitted_tracks,
            d.own_private.map(bytes).unwrap_or_else(|| "—".into()),
            d.own_working_set.map(bytes).unwrap_or_else(|| "—".into())
        );
        text.push_str(&format!("Current measurement coverage: {} of {} observed processes measured{}\n\nHistorical collection failures and recorded reasons:\n", d.current_measured, d.current_observed, if d.current_enumeration_complete { "" } else { "; enumeration incomplete, total unknown" }));
        text.push_str(&if d.failures.is_empty() {
            "No recorded collection failures".into()
        } else {
            d.failures.join("\n")
        });
        let window = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            wide("STATIC").as_ptr(),
            wide("Collection details").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            780,
            600,
            hwnd,
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        );
        if window.is_null() {
            (*ptr).say(format!(
                "Details unavailable: {}",
                io::Error::last_os_error()
            ));
            return;
        }
        SetWindowSubclass(window, Some(details_proc), 1, 0);
        let edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            wide("EDIT").as_ptr(),
            wide(&text.replace('\n', "\r\n")).as_ptr(),
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | WS_VSCROLL
                | ES_MULTILINE as u32
                | ES_READONLY as u32
                | ES_AUTOVSCROLL as u32,
            12,
            12,
            740,
            480,
            window,
            1 as HMENU,
            GetModuleHandleW(null()),
            null(),
        );
        SendMessageW(edit, WM_SETFONT, (*ptr).native_fonts.normal as usize, 1);
        let close = CreateWindowExW(
            0,
            wide("BUTTON").as_ptr(),
            wide("Close").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            650,
            504,
            100,
            32,
            window,
            2 as HMENU,
            GetModuleHandleW(null()),
            null(),
        );
        SendMessageW(close, WM_SETFONT, (*ptr).native_fonts.normal as usize, 1);
        SendMessageW(window, WM_SIZE, 0, 0);
        SetFocus(edit);
    }
}
unsafe extern "system" fn details_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    unsafe {
        match msg {
            WM_SIZE => {
                let mut r = zeroed();
                GetClientRect(hwnd, &mut r);
                MoveWindow(
                    GetDlgItem(hwnd, 1),
                    12,
                    12,
                    (r.right - 24).max(1),
                    (r.bottom - 64).max(1),
                    1,
                );
                MoveWindow(
                    GetDlgItem(hwnd, 2),
                    (r.right - 112).max(12),
                    (r.bottom - 44).max(12),
                    100,
                    32,
                    1,
                );
                0
            }
            WM_COMMAND if wp & 0xffff == 2 => {
                DestroyWindow(hwnd);
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            _ => DefSubclassProc(hwnd, msg, wp, lp),
        }
    }
}
impl App {
    unsafe fn ensure_buffer(&mut self, dc: HDC, width: i32, height: i32) -> bool {
        unsafe {
            if self.buffer_size == (width, height) && !self.buffer.is_null() {
                return true;
            }
            self.free_buffer();
            self.buffer = CreateCompatibleDC(dc);
            self.bitmap = CreateCompatibleBitmap(dc, width, height);
            if self.buffer.is_null() || self.bitmap.is_null() {
                self.free_buffer();
                return false;
            }
            self.original_bitmap = SelectObject(self.buffer, self.bitmap);
            self.buffer_size = (width, height);
            true
        }
    }
    unsafe fn free_buffer(&mut self) {
        unsafe {
            if !self.buffer.is_null() {
                if !self.original_bitmap.is_null() {
                    SelectObject(self.buffer, self.original_bitmap);
                }
                DeleteDC(self.buffer);
            }
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap);
            }
            self.buffer = null_mut();
            self.bitmap = null_mut();
            self.original_bitmap = null_mut();
            self.buffer_size = (0, 0);
        }
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
        self.provider.take();
        self.save_preferences();
        unsafe {
            self.free_buffer();
            self.fonts.delete();
            self.native_fonts.delete();
            if self.row_images != 0 {
                ImageList_Destroy(self.row_images);
            }
            if !self.tooltips.is_null() {
                DestroyWindow(self.tooltips);
            }
        }
    }
}
unsafe fn paint_main(hwnd: HWND, ptr: *mut App, dc: HDC) {
    unsafe {
        let saved = SaveDC(dc);
        let v = (*ptr).viewport;
        SetMapMode(dc, MM_ANISOTROPIC);
        SetWindowExtEx(dc, 96, 96, null_mut());
        SetViewportExtEx(dc, v.scale as i32, v.scale as i32, null_mut());
        let (x, y) = v.physical(0, 0);
        SetViewportOrgEx(dc, x, y, null_mut());
        let fonts = (*ptr).fonts;
        let canvas = Canvas { dc, fonts: &fonts };
        (*ptr).paint(&canvas);
        RestoreDC(dc, saved);
        let _ = hwnd;
    }
}
unsafe fn screenshot(hwnd: HWND, path: &Path) -> io::Result<()> {
    unsafe {
        let mut r = zeroed();
        GetClientRect(hwnd, &mut r);
        let (width, height) = (r.right, r.bottom);
        let dc = GetDC(hwnd);
        let mem = CreateCompatibleDC(dc);
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let mut data = null_mut();
        let bmp = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut data, null_mut(), 0);
        if mem.is_null() || bmp.is_null() || data.is_null() {
            if !bmp.is_null() {
                DeleteObject(bmp);
            }
            if !mem.is_null() {
                DeleteDC(mem);
            }
            ReleaseDC(hwnd, dc);
            return Err(io::Error::last_os_error());
        }
        let old = SelectObject(mem, bmp);
        SendMessageW(hwnd, WM_PRINTCLIENT, mem as usize, PRF_CLIENT as isize);
        print_children(hwnd, mem);
        GdiFlush();
        let result = (|| {
            let mut f = BufWriter::new(File::create(path)?);
            let size = (width * height * 4) as u32;
            f.write_all(b"BM")?;
            f.write_all(&(size + 54).to_le_bytes())?;
            f.write_all(&[0; 4])?;
            f.write_all(&54u32.to_le_bytes())?;
            f.write_all(&40u32.to_le_bytes())?;
            f.write_all(&width.to_le_bytes())?;
            f.write_all(&(-height).to_le_bytes())?;
            f.write_all(&1u16.to_le_bytes())?;
            f.write_all(&32u16.to_le_bytes())?;
            f.write_all(&[0; 24])?;
            f.write_all(std::slice::from_raw_parts(data.cast::<u8>(), size as usize))?;
            f.flush()
        })();
        SelectObject(mem, old);
        DeleteObject(bmp);
        DeleteDC(mem);
        ReleaseDC(hwnd, dc);
        result
    }
}
// WM_PRINT's default child traversal does not clip scrolled descendants to
// their parent's viewport. Compose native client surfaces with that same
// clipping explicitly, so exports reflect the visible inspector hierarchy.
unsafe fn print_children(parent: HWND, dc: HDC) {
    unsafe {
        let mut child = GetWindow(parent, GW_CHILD);
        while !child.is_null() {
            if GetWindowLongPtrW(child, GWL_STYLE) as u32 & WS_VISIBLE != 0 {
                let mut bounds = zeroed();
                GetClientRect(child, &mut bounds);
                let mut origin = POINT { x: 0, y: 0 };
                ClientToScreen(child, &mut origin);
                ScreenToClient(parent, &mut origin);
                let saved = SaveDC(dc);
                IntersectClipRect(
                    dc,
                    origin.x,
                    origin.y,
                    origin.x + bounds.right,
                    origin.y + bounds.bottom,
                );
                OffsetViewportOrgEx(dc, origin.x, origin.y, null_mut());
                SendMessageW(child, WM_PRINTCLIENT, dc as usize, PRF_CLIENT as isize);
                print_children(child, dc);
                RestoreDC(dc, saved);
            }
            child = GetWindow(child, GW_HWNDNEXT);
        }
    }
}
unsafe fn refresh_text_scale(ptr: *mut App) {
    unsafe {
        let result = windows::UI::ViewManagement::UISettings::new()
            .and_then(|settings| settings.TextScaleFactor());
        match result {
            Ok(factor) => (*ptr).text_scale = factor.clamp(1.0, 2.25),
            Err(error) => {
                (*ptr).text_scale = 1.0;
                (*ptr).say(format!("Windows text-size setting unavailable: {error}"));
            }
        }
    }
}
unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let create = &*(lp as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            let ptr = create.lpCreateParams as *mut App;
            (*ptr).hwnd = hwnd;
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wp, lp);
        }
        match msg {
            WM_CREATE => {
                (*ptr).dpi = GetDpiForWindow(hwnd);
                refresh_text_scale(ptr);
                create_controls(hwnd, ptr);
                layout_window(hwnd, ptr);
                sync_native(ptr);
                SetTimer(hwnd, 1, 100, None);
                0
            }
            WM_GETMINMAXINFO => {
                let min = &mut *(lp as *mut MINMAXINFO);
                min.ptMinTrackSize = POINT { x: 640, y: 480 };
                0
            }
            WM_SIZE => {
                layout_window(hwnd, ptr);
                0
            }
            WM_SETTINGCHANGE => {
                refresh_text_scale(ptr);
                layout_window(hwnd, ptr);
                0
            }
            WM_DPICHANGED => {
                (*ptr).dpi = (wp & 0xffff) as u32;
                let r = &*(lp as *const RECT);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                layout_window(hwnd, ptr);
                0
            }
            WM_HSCROLL | WM_VSCROLL => {
                let horizontal = msg == WM_HSCROLL;
                let bar = if horizontal { SB_HORZ } else { SB_VERT };
                let mut info = SCROLLINFO {
                    cbSize: size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_ALL,
                    ..zeroed()
                };
                GetScrollInfo(hwnd, bar, &mut info);
                let pos = match (wp & 0xffff) as i32 {
                    SB_LINEUP => info.nPos - 32,
                    SB_LINEDOWN => info.nPos + 32,
                    SB_PAGEUP => info.nPos - info.nPage as i32,
                    SB_PAGEDOWN => info.nPos + info.nPage as i32,
                    SB_THUMBTRACK | SB_THUMBPOSITION => info.nTrackPos,
                    SB_TOP => info.nMin,
                    SB_BOTTOM => info.nMax,
                    _ => info.nPos,
                };
                if horizontal {
                    (*ptr).viewport.x = pos;
                } else {
                    (*ptr).viewport.y = pos;
                }
                layout_window(hwnd, ptr);
                0
            }
            WM_COMMAND => {
                command(
                    hwnd,
                    ptr,
                    (wp & 0xffff) as i32,
                    ((wp >> 16) & 0xffff) as u32,
                );
                0
            }
            WM_DRAWITEM => {
                let draw = &*(lp as *const DRAWITEMSTRUCT);
                if draw.CtlType == ODT_BUTTON {
                    draw_button(draw.hwndItem, draw.hDC);
                    1
                } else {
                    DefWindowProcW(hwnd, msg, wp, lp)
                }
            }
            WM_NOTIFY => {
                native_notifications(ptr, lp).unwrap_or_else(|| DefWindowProcW(hwnd, msg, wp, lp))
            }
            WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN => {
                let dc = wp as HDC;
                let background = if GetParent(lp as HWND) == (*ptr).inspector {
                    PANEL
                } else {
                    BG
                };
                SetBkColor(dc, background);
                SetTextColor(dc, INK);
                SetDCBrushColor(dc, background);
                GetStockObject(DC_BRUSH) as isize
            }
            WM_TIMER => {
                if wp == FILTER_TIMER {
                    KillTimer(hwnd, FILTER_TIMER);
                    if let Some((query, _)) = (*ptr).pending_query.take() {
                        (*ptr).query = query;
                        (*ptr).refresh_rows(true);
                        sync_native(ptr);
                    }
                    return 0;
                }
                if (*ptr).collect() {
                    sync_native(ptr);
                }
                sync_recording_label(ptr);
                if (*ptr).smoke.is_some() {
                    smoke_tick(hwnd, ptr);
                }
                0
            }
            MSG_LAYOUT => {
                layout_window(hwnd, ptr);
                sync_native(ptr);
                0
            }
            MSG_EXPORT => {
                export_dialog(hwnd, ptr);
                0
            }
            MSG_DETAILS => {
                details_dialog(hwnd, ptr);
                0
            }
            MSG_ACCESS => {
                loop {
                    let action = (*ptr).provider.as_ref().and_then(|p| p.take_action());
                    let Some((action, id)) = action else {
                        break;
                    };
                    if let Some(index) = (*ptr)
                        .rows
                        .iter()
                        .position(|r| (*ptr).instance_id(&r.selection) == id)
                    {
                        match action {
                            1 => (*ptr).select(index),
                            2 | 3 => {
                                let selection = (&(*ptr).rows)[index].selection.clone();
                                (*ptr).toggle(selection, Some(action == 2));
                            }
                            _ => {}
                        }
                    }
                }
                sync_native(ptr);
                0
            }
            MSG_CHART_KEY => {
                chart_sample_key(ptr, wp as i32, lp);
                0
            }
            MSG_CONTROL_FOCUS => {
                let id = wp as i32;
                if (ID_LEGEND..ID_LEGEND + 5).contains(&id) {
                    (*ptr).highlighted_band = (id - ID_LEGEND) as usize;
                    sync_native(ptr);
                }
                0
            }
            WM_SETCURSOR => {
                let mut point = zeroed();
                GetCursorPos(&mut point);
                ScreenToClient(hwnd, &mut point);
                let (x, y) = (*ptr).viewport.point(point.x, point.y);
                if contains((*ptr).layout.split, x, y) {
                    SetCursor(LoadCursorW(null_mut(), IDC_SIZEWE));
                    1
                } else {
                    DefWindowProcW(hwnd, msg, wp, lp)
                }
            }
            WM_LBUTTONDOWN => {
                let (x, y) = (*ptr)
                    .viewport
                    .point(lp as i16 as i32, (lp >> 16) as i16 as i32);
                if contains((*ptr).layout.split, x, y) {
                    (*ptr).splitter_drag = true;
                    SetCapture(hwnd);
                }
                0
            }
            WM_MOUSEMOVE => {
                let (x, y) = (*ptr)
                    .viewport
                    .point(lp as i16 as i32, (lp >> 16) as i16 as i32);
                if (*ptr).splitter_drag {
                    (*ptr).prefs.splitter_permille = ((x - 16).max(1) as u32 * 1000
                        / ((*ptr).width - 44) as u32)
                        .clamp(450, 800);
                    layout_window(hwnd, ptr);
                } else {
                    let chart = (*ptr).overview_chart();
                    if contains(chart, x, y) {
                        let plot =
                            dashboard::overview((*ptr).shown(), range((*ptr).prefs.overview_range));
                        let at = plot.start
                            + ((x - chart.left).max(0) as u64 * (plot.end - plot.start) as u64
                                / (chart.right - chart.left).max(1) as u64)
                                as u32;
                        (*ptr).overview_hover = Some(at);
                        if let Some(sample) = plot.samples.iter().min_by_key(|s| s.at.abs_diff(at))
                            && let Some(layers) = &sample.layers
                        {
                            let value = ((chart.bottom - y).max(0) as f64
                                / (chart.bottom - chart.top).max(1) as f64
                                * plot.capacity as f64)
                                as u64;
                            let mut cumulative = 0;
                            for (i, band) in layers.iter().enumerate() {
                                cumulative += band;
                                if value <= cumulative {
                                    (*ptr).highlighted_band = i;
                                    break;
                                }
                            }
                        }
                        update_chart_values(ptr, &plot);
                        InvalidateRect(hwnd, null(), 0);
                    }
                }
                0
            }
            WM_LBUTTONUP => {
                if (*ptr).splitter_drag {
                    (*ptr).splitter_drag = false;
                    ReleaseCapture();
                    (*ptr).save_preferences();
                }
                0
            }
            WM_MOUSEWHEEL => {
                if wp & 0x0008 != 0 {
                    let direction = if (wp >> 16) as i16 > 0 { 1 } else { -1 };
                    (*ptr).prefs.ui_zoom = zoom_interface((*ptr).prefs.ui_zoom, direction);
                    layout_window(hwnd, ptr);
                    (*ptr).save_preferences();
                } else {
                    let mut p = POINT {
                        x: lp as i16 as i32,
                        y: (lp >> 16) as i16 as i32,
                    };
                    ScreenToClient(hwnd, &mut p);
                    let (x, y) = (*ptr).viewport.point(p.x, p.y);
                    if contains((*ptr).layout.inspector, x, y) {
                        SendMessageW((*ptr).inspector, WM_MOUSEWHEEL, wp, lp);
                    }
                }
                0
            }
            WM_KEYDOWN => {
                match wp as u16 {
                    VK_SPACE => {
                        (*ptr).pause_display();
                        sync_native(ptr);
                    }
                    VK_F2 => {
                        (*ptr).grouping = match (*ptr).grouping {
                            Grouping::Name => Grouping::Executable,
                            Grouping::Executable => Grouping::Tree,
                            Grouping::Tree => Grouping::Process,
                            Grouping::Process => Grouping::Family,
                            Grouping::Family => Grouping::Name,
                        };
                        (*ptr).refresh_rows(true);
                        sync_native(ptr);
                    }
                    VK_F3 => {
                        (*ptr).sort = Sort {
                            column: if (*ptr).sort.column == SortColumn::Change {
                                SortColumn::PrivateWorkingSet
                            } else {
                                SortColumn::Change
                            },
                            descending: true,
                        };
                        (*ptr).refresh_rows(true);
                        sync_native(ptr);
                    }
                    VK_F4 => {
                        (*ptr).show_exited = !(*ptr).show_exited;
                        (*ptr).refresh_rows(true);
                        sync_native(ptr);
                    }
                    VK_F5 => {
                        (*ptr).metric = if (*ptr).metric == Metric::PrivateCommit {
                            Metric::PrivateWorkingSet
                        } else {
                            Metric::PrivateCommit
                        };
                        SendMessageW(
                            (*ptr).control(ID_METRIC),
                            CB_SETCURSEL,
                            usize::from((*ptr).metric == Metric::PrivateCommit),
                            0,
                        );
                        sync_native(ptr);
                    }
                    VK_OEM_PLUS | VK_ADD => zoom_history(ptr, true),
                    VK_OEM_MINUS | VK_SUBTRACT => zoom_history(ptr, false),
                    VK_DOWN | VK_UP => {
                        SetFocus((*ptr).table);
                        SendMessageW((*ptr).table, msg, wp, lp);
                    }
                    _ => {}
                }
                0
            }
            WM_ERASEBKGND => 1,
            WM_PRINTCLIENT => {
                paint_main(hwnd, ptr, wp as HDC);
                0
            }
            WM_PAINT => {
                let mut ps = zeroed();
                let dc = BeginPaint(hwnd, &mut ps);
                let mut r = zeroed();
                GetClientRect(hwnd, &mut r);
                if r.right > 0 && r.bottom > 0 && (*ptr).ensure_buffer(dc, r.right, r.bottom) {
                    paint_main(hwnd, ptr, (*ptr).buffer);
                    BitBlt(dc, 0, 0, r.right, r.bottom, (*ptr).buffer, 0, 0, SRCCOPY);
                }
                EndPaint(hwnd, &ps);
                0
            }
            WM_CLOSE => {
                (*ptr).provider.take();
                (*ptr).save_preferences();
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, 1);
                PostQuitMessage(0);
                0
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                DefWindowProcW(hwnd, msg, wp, lp)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}
unsafe fn smoke_assert(ptr: *mut App, condition: bool, criterion: &str) {
    unsafe {
        if !condition && !(*ptr).smoke_failures.iter().any(|s| s == criterion) {
            (*ptr).smoke_failures.push(criterion.into());
        }
    }
}
unsafe fn smoke_tick(hwnd: HWND, ptr: *mut App) {
    unsafe {
        let elapsed = (*ptr).smoke_started.elapsed().as_secs();
        if (*ptr).history.total == 0 && elapsed < 20 {
            return;
        }
        let dir = (*ptr).smoke.clone().unwrap();
        if (*ptr).smoke_stage == 0 && elapsed >= 4 {
            smoke_assert(
                ptr,
                !(*ptr).control(ID_PAUSE).is_null() && !(*ptr).control(ID_EXPORT).is_null(),
                "native header actions",
            );
            smoke_assert(
                ptr,
                GetWindowLongPtrW((*ptr).table, GWL_STYLE) as u32 & LVS_OWNERDATA != 0,
                "virtualized native table",
            );
            smoke_assert(ptr, !(*ptr).rows.is_empty(), "real process rows");
            smoke_assert(ptr, (*ptr).compact, "default compact rows");
            assert_table_render(ptr);
            let font_dc = GetDC(hwnd);
            let old_font = SelectObject(font_dc, (*ptr).native_fonts.normal);
            let mut digit_widths = Vec::new();
            for digit in b'0'..=b'9' {
                let character = [digit as u16];
                let mut size = zeroed();
                GetTextExtentPoint32W(font_dc, character.as_ptr(), 1, &mut size);
                digit_widths.push(size.cx);
            }
            SelectObject(font_dc, old_font);
            ReleaseDC(hwnd, font_dc);
            smoke_assert(
                ptr,
                digit_widths
                    .iter()
                    .all(|width| *width > 0 && *width == digit_widths[0]),
                "platform font uses equal digit advances for aligned numeric columns",
            );
            let before = dashboard::overview((*ptr).shown(), Range::All);
            let frozen_total = before.samples.last().and_then(|s| s.used);
            let pinned: Vec<_> = before.layers.iter().map(|l| l.name.clone()).collect();
            let original = (*ptr).query.clone();
            (*ptr).query = "ChatGPT.exe".into();
            (*ptr).refresh_rows(true);
            let after = dashboard::overview((*ptr).shown(), Range::All);
            smoke_assert(
                ptr,
                after.samples.last().and_then(|s| s.used) == frozen_total
                    && after.layers.iter().map(|l| l.name.clone()).eq(pinned),
                "filter preserves complete system stack and band identities",
            );
            (*ptr).query = original;
            (*ptr).refresh_rows(true);
            if let Some(row) = (*ptr).rows.iter().find(|r| r.has_children).cloned() {
                (*ptr).selected = Some(row.selection.clone());
                (*ptr).toggle(row.selection.clone(), Some(true));
                smoke_assert(
                    ptr,
                    (*ptr)
                        .rows
                        .iter()
                        .any(|r| r.parent.as_ref() == Some(&row.selection)),
                    "expanded group retains child rows",
                );
            }
            // Holding a prior order must survive entering display pause even
            // when its current raw values would sort differently.
            let mut held = (*ptr).all_rows.clone();
            held.reverse();
            let original_all = std::mem::replace(&mut (*ptr).all_rows, held);
            let original_rows = (*ptr).rows.clone();
            (*ptr).rows.reverse();
            let expected_order: Vec<_> = (*ptr).rows.iter().map(|r| r.selection.clone()).collect();
            let old_held = (*ptr).order_held;
            (*ptr).order_held = true;
            (*ptr).pause_display();
            smoke_assert(
                ptr,
                (*ptr)
                    .rows
                    .iter()
                    .map(|r| r.selection.clone())
                    .eq(expected_order),
                "entering display pause preserves held displayed identity order",
            );
            (*ptr).frozen = None;
            (*ptr).all_rows = original_all;
            (*ptr).rows = original_rows;
            (*ptr).order_held = old_held;
            (*ptr).smoke_selection = (*ptr).selected.clone();
            sync_native(ptr);
            let paths = (*ptr)
                .selected_summary()
                .map(|s| s.paths.join("\r\n"))
                .unwrap_or_default();
            if !paths.is_empty() {
                let copy_result = copy_clipboard(hwnd, &paths);
                let readback = clipboard_text(hwnd);
                let copied =
                    copy_result.is_ok() && readback.as_deref().ok() == Some(paths.as_str());
                (*ptr).smoke_clipboard = format!(
                    "clipboard_expected_chars={}\nclipboard_readback_chars={}\nclipboard_write={:?}\nclipboard_read={:?}",
                    paths.encode_utf16().count(),
                    readback.as_ref().map_or(0, |s| s.encode_utf16().count()),
                    copy_result,
                    readback.as_ref().map(|_| "OK")
                );
                smoke_assert(ptr, copied, "full executable paths copied to clipboard");
            }
            let selected_range = (*ptr).prefs.history_range;
            (*ptr).prefs.history_range = Some(1800);
            zoom_history(ptr, true);
            smoke_assert(
                ptr,
                (*ptr).prefs.history_range == Some(1800)
                    && (*ptr).history_width.is_some()
                    && !(*ptr).follow,
                "manual history zoom preserves chosen 30m range and disables Follow latest",
            );
            SendMessageW((*ptr).control(ID_RESET), BM_CLICK, 0, 0);
            smoke_assert(
                ptr,
                (*ptr).prefs.history_range == Some(1800)
                    && (*ptr).history_width.is_none()
                    && (*ptr).history_end.is_none()
                    && (*ptr).follow
                    && SendMessageW((*ptr).control(ID_RANGE_SEG + 2), BM_GETCHECK, 0, 0)
                        == BST_CHECKED as isize,
                "Reset view restores chosen 30m segment and clears manual navigation",
            );
            (*ptr).prefs.history_range = selected_range;
            SendMessageW((*ptr).control(ID_FOLLOW), BM_CLICK, 0, 0);
            let held_end = (*ptr).history_end;
            smoke_assert(
                ptr,
                !(*ptr).follow && held_end.is_some(),
                "unchecking Follow latest latches displayed endpoint",
            );
            zoom_history(ptr, true);
            SendMessageW((*ptr).control(ID_RANGE_SEG), BM_CLICK, 0, 0);
            smoke_assert(
                ptr,
                (*ptr).prefs.history_range == Some(300)
                    && (*ptr).history_width.is_none()
                    && (*ptr).history_end == held_end
                    && !(*ptr).follow,
                "range segment clears manual zoom and preserves Follow-off endpoint",
            );
            (*ptr).choose_history_range(selected_range);
            sync_native(ptr);
            // Stage 1 checks this endpoint after several actual collection cycles.
            (*ptr).smoke_before = (*ptr).history.at;
            SendMessageW((*ptr).control(ID_PAUSE), BM_CLICK, 0, 0);
            (*ptr).smoke_display = (*ptr).shown().at;
            smoke_assert(
                ptr,
                (*ptr).frozen.is_some(),
                "Pause display freezes displayed snapshot",
            );
            (*ptr).smoke_stage = 1;
            (*ptr).smoke_started = Instant::now();
        } else if (*ptr).smoke_stage == 1 && elapsed >= 5 {
            smoke_assert(
                ptr,
                (*ptr).history.at > (*ptr).smoke_before
                    && (*ptr).shown().at == (*ptr).smoke_display,
                "recording advances while all displayed readings stay frozen",
            );
            smoke_assert(
                ptr,
                (*ptr).selected == (*ptr).smoke_selection,
                "stable selection while paused",
            );
            smoke_assert(
                ptr,
                (*ptr)
                    .selected_series()
                    .is_none_or(|s| Some(s.end) == (*ptr).history_end),
                "Follow latest off preserves selected endpoint while recording advances",
            );
            let exported = File::create(dir.join("paused-snapshot.csv")).and_then(|f| {
                dashboard::export_snapshot(
                    (*ptr).shown(),
                    (*ptr).grouping,
                    (*ptr).show_exited,
                    &(*ptr).query,
                    f,
                )
            });
            smoke_assert(ptr, exported.is_ok(), "paused coherent snapshot export");
            let capture = screenshot(hwnd, &dir.join("paused-dashboard.bmp"));
            smoke_assert(
                ptr,
                capture.is_ok(),
                "native controls included in paused dashboard capture",
            );
            SendMessageW((*ptr).control(ID_PAUSE), BM_CLICK, 0, 0);
            smoke_assert(
                ptr,
                (*ptr).frozen.is_none() && (*ptr).shown().at == (*ptr).history.at,
                "resume advances to latest completed snapshot",
            );
            smoke_assert(
                ptr,
                (*ptr)
                    .selected_series()
                    .is_none_or(|s| Some(s.end) == (*ptr).history_end),
                "Follow latest off preserves endpoint after resume",
            );
            SendMessageW((*ptr).control(ID_FOLLOW), BM_CLICK, 0, 0);
            smoke_assert(
                ptr,
                (*ptr).follow && (*ptr).history_end.is_none(),
                "Follow latest re-enabled returns to newest displayed sample",
            );
            (*ptr).smoke_stage = 2;
            (*ptr).smoke_started = Instant::now();
        } else if (*ptr).smoke_stage == 2 && elapsed >= 2 {
            sync_native(ptr);
            let capture = screenshot(hwnd, &dir.join("dashboard.bmp"));
            smoke_assert(ptr, capture.is_ok(), "reference native dashboard capture");
            assert_button_labels(ptr);
            assert_overview_layout(ptr);
            assert_table_render(ptr);
            let original_zoom = (*ptr).prefs.ui_zoom;
            let original_selection = (*ptr).selected.clone();
            if let Some(row) = (*ptr)
                .rows
                .iter()
                .find(|row| {
                    row.depth == 0
                        && row.name.eq_ignore_ascii_case("codex.exe")
                        && complete_history_value(&row.private).is_some()
                })
                .cloned()
            {
                (*ptr).selected = Some(row.selection);
            }
            for scale in [150, 200] {
                (*ptr).prefs.ui_zoom = scale;
                layout_window(hwnd, ptr);
                sync_native(ptr);
                let result = screenshot(hwnd, &dir.join(format!("dashboard-{scale}.bmp")));
                smoke_assert(ptr, result.is_ok(), "scaled native dashboard capture");
                assert_button_labels(ptr);
                assert_overview_layout(ptr);
                SendMessageW(hwnd, WM_HSCROLL, SB_BOTTOM as usize, 0);
                SendMessageW(hwnd, WM_VSCROLL, SB_BOTTOM as usize, 0);
                // Put the plot itself in view; scrolling to the last field can
                // hide a missing custom canvas behind correct native children.
                (*ptr).ins_scroll = (*ptr).history_chart().top - 24;
                layout_inspector(ptr);
                let right = screenshot(hwnd, &dir.join(format!("dashboard-{scale}-inspector.bmp")));
                smoke_assert(
                    ptr,
                    right.is_ok(),
                    "scaled inspector and exact values reachable with scrolling",
                );
                assert_history_render(hwnd, ptr);
                (*ptr).viewport.x = 0;
                (*ptr).viewport.y = 0;
                (*ptr).ins_scroll = 0;
                smoke_assert(
                    ptr,
                    (*ptr).viewport.scale >= scale,
                    "application scaling preserved",
                );
            }
            (*ptr).selected = original_selection;
            sync_native(ptr);
            (*ptr).prefs.ui_zoom = original_zoom;
            let actual_text_scale = (*ptr).text_scale;
            (*ptr).text_scale = 1.5;
            layout_window(hwnd, ptr);
            smoke_assert(
                ptr,
                (*ptr).viewport.scale
                    == (((*ptr).dpi as f64 * 1.5).round() as u32 * original_zoom / 100),
                "Windows text-scale setting affects shared font/control/canvas geometry",
            );
            let text_scaled = screenshot(hwnd, &dir.join("dashboard-text150.bmp"));
            smoke_assert(
                ptr,
                text_scaled.is_ok(),
                "injected text-size capture without changing Windows settings",
            );
            (*ptr).text_scale = actual_text_scale;
            (*ptr).viewport.x = 0;
            (*ptr).viewport.y = 0;
            (*ptr).ins_scroll = 0;
            layout_window(hwnd, ptr);
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                640,
                480,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            layout_window(hwnd, ptr);
            let narrow = screenshot(hwnd, &dir.join("dashboard-narrow.bmp"));
            SendMessageW(hwnd, WM_HSCROLL, SB_BOTTOM as usize, 0);
            SendMessageW(hwnd, WM_VSCROLL, SB_BOTTOM as usize, 0);
            SendMessageW((*ptr).inspector, WM_VSCROLL, SB_BOTTOM as usize, 0);
            let mut client = zeroed();
            GetClientRect(hwnd, &mut client);
            let reachable = [ID_ZOOM, ID_HISTORY_VALUES].into_iter().all(|id| {
                let mut r = zeroed();
                GetWindowRect((*ptr).control(id), &mut r);
                MapWindowPoints(null_mut(), hwnd, (&mut r as *mut RECT).cast(), 2);
                r.left >= 0 && r.top >= 0 && r.right <= client.right && r.bottom <= client.bottom
            });
            let narrow_values = screenshot(hwnd, &dir.join("dashboard-narrow-values.bmp"));
            smoke_assert(
                ptr,
                narrow.is_ok() && narrow_values.is_ok() && reachable,
                "extreme window overflow keeps final history data and zoom inside actual client after scrolling",
            );
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                1560,
                1060,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            layout_window(hwnd, ptr);
            sync_native(ptr);
            let gdi_before = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
            let dc = GetDC(hwnd);
            let mut client = zeroed();
            GetClientRect(hwnd, &mut client);
            let allocated = (*ptr).ensure_buffer(dc, client.right, client.bottom);
            smoke_assert(ptr, allocated, "persistent GDI buffer allocated");
            let before = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
            let started = Instant::now();
            for _ in 0..100 {
                paint_main(hwnd, ptr, (*ptr).buffer);
            }
            let render_ms = started.elapsed().as_millis();
            let after = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
            ReleaseDC(hwnd, dc);
            smoke_assert(
                ptr,
                before == after,
                "100 redraws keep GDI object count stable",
            );
            let full_retention = stress_full_retention(hwnd, ptr, &dir);
            smoke_assert(
                ptr,
                full_retention.is_ok(),
                "full-retention model/cache construction and 100 redraw resource invariant",
            );
            let retention_report =
                full_retention.unwrap_or_else(|error| format!("diagnostic_error={error}"));
            let diagnostics = dashboard::diagnostics((*ptr).shown(), GetCurrentProcessId());
            let result = if (*ptr).smoke_failures.is_empty() {
                "PASS"
            } else {
                "FAIL"
            };
            let report = format!(
                "result={result}\nobserved_processes={}\nretained_samples={}\ncurrent_unavailable={}\nprivate_bytes={}\nworking_set_bytes={}\ngdi_objects_before={}\ngdi_objects_after={}\ninitial_gdi={}\n100_render_ms={}\ntext_scale_factor={}\n{}\n{}\nhistory_geometry={:?}\nfailures={:?}\n",
                diagnostics.observed_session,
                diagnostics.retained_snapshots,
                diagnostics.current_unavailable,
                diagnostics.own_private.unwrap_or(0),
                diagnostics.own_working_set.unwrap_or(0),
                before,
                after,
                gdi_before,
                render_ms,
                (*ptr).text_scale,
                retention_report,
                (*ptr).smoke_clipboard,
                (*ptr).smoke_geometry,
                (*ptr).smoke_failures
            );
            let _ = std::fs::write(dir.join("smoke.txt"), report);
            (*ptr).smoke_stage = 3;
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        } else if (*ptr).smoke_stage == 0 && elapsed >= 20 {
            let _ = std::fs::write(
                dir.join("smoke.txt"),
                format!("result=FAIL\nerror={:?}", (*ptr).error),
            );
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }
}
pub fn run() -> io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next();
    let is_smoke = mode.as_ref().is_some_and(|s| s == "--smoke-test");
    let probe = mode.as_ref().is_some_and(|s| s == "--probe-test");
    let directory = if is_smoke || probe {
        let path = PathBuf::from(
            args.next()
                .ok_or_else(|| io::Error::other("test launch requires an output directory"))?,
        );
        std::fs::create_dir_all(&path)?;
        Some(path)
    } else {
        None
    };
    let smoke = if is_smoke { directory.clone() } else { None };
    let preference_override = directory.map(|dir| dir.join("preferences.ini"));
    unsafe {
        let initialized = RoInitialize(RO_INIT_SINGLETHREADED).is_ok();
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(null());
        let class = AppClass::new(instance)?;
        let mut app = Box::new(App::new(smoke, preference_override));
        let dpi = GetDpiForSystem().max(96) as i32;
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.name.as_ptr(),
            wide("Mem Map").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN | WS_HSCROLL | WS_VSCROLL,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1560 * dpi / 96,
            1060 * dpi / 96,
            null_mut(),
            null_mut(),
            instance,
            (&mut *app as *mut App).cast(),
        );
        if hwnd.is_null() {
            return Err(io::Error::last_os_error());
        }
        if app.smoke.is_none() && !probe {
            ShowWindow(hwnd, SW_SHOWNORMAL);
            UpdateWindow(hwnd);
        }
        let mut message: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result == 0 {
                break;
            }
            if result < 0 {
                let error = io::Error::last_os_error();
                DestroyWindow(hwnd);
                return Err(error);
            }
            if message.message == WM_KEYDOWN {
                let key = message.wParam as u16;
                let control = GetKeyState(VK_CONTROL as i32) < 0;
                if control {
                    let direction = match key {
                        VK_OEM_PLUS | VK_ADD => Some(1),
                        VK_OEM_MINUS | VK_SUBTRACT => Some(-1),
                        0x30 | VK_NUMPAD0 => Some(0),
                        _ => None,
                    };
                    if let Some(direction) = direction {
                        app.prefs.ui_zoom = zoom_interface(app.prefs.ui_zoom, direction);
                        layout_window(hwnd, &mut *app);
                        app.save_preferences();
                        continue;
                    }
                    if key == 0x46 {
                        SetFocus(app.control(ID_FILTER));
                        SendMessageW(app.control(ID_FILTER), EM_SETSEL, 0, -1);
                        continue;
                    }
                }
                if key == VK_ESCAPE && GetFocus() == app.control(ID_FILTER) {
                    SetWindowTextW(app.control(ID_FILTER), wide("").as_ptr());
                    app.pending_query = None;
                    app.query.clear();
                    app.refresh_rows(true);
                    sync_native(&mut *app);
                    SetFocus(app.table);
                    continue;
                }
                let mut class_name = [0u16; 64];
                let class_len = GetClassNameW(message.hwnd, class_name.as_mut_ptr(), 64);
                let class_name = String::from_utf16_lossy(&class_name[..class_len as usize]);
                if !control && class_name != "Edit" && class_name != "ComboBox" {
                    match key {
                        VK_OEM_PLUS | VK_ADD => {
                            zoom_history(&mut *app, true);
                            continue;
                        }
                        VK_OEM_MINUS | VK_SUBTRACT => {
                            zoom_history(&mut *app, false);
                            continue;
                        }
                        0x31..=0x33 => {
                            app.choose_history_range(Some([300, 900, 1800][(key - 0x31) as usize]));
                            sync_native(&mut *app);
                            app.save_preferences();
                            continue;
                        }
                        _ => {}
                    }
                }
                if (VK_F2..=VK_F5).contains(&key) {
                    SendMessageW(hwnd, WM_KEYDOWN, message.wParam, message.lParam);
                    continue;
                }
                if key == VK_F8 {
                    if let Some(Selection::Process(child)) = app.selected.clone()
                        && let Some(parent) = app.shown().parent_target(child)
                    {
                        app.grouping = Grouping::Process;
                        app.show_exited = true;
                        SetWindowTextW(app.control(ID_FILTER), wide("").as_ptr());
                        app.query.clear();
                        app.pending_query = None;
                        app.selected = Some(Selection::Process(parent));
                        app.refresh_rows(true);
                        sync_native(&mut *app);
                        if let Some(index) = app
                            .rows
                            .iter()
                            .position(|r| r.selection == Selection::Process(parent))
                        {
                            SendMessageW(app.table, LVM_ENSUREVISIBLE, index, 0);
                        }
                    }
                    continue;
                }
            }
            if message.message == WM_MOUSEWHEEL && message.wParam & 0x0008 != 0 {
                SendMessageW(hwnd, WM_MOUSEWHEEL, message.wParam, message.lParam);
                continue;
            }
            let root = GetAncestor(message.hwnd, GA_ROOT);
            let dialog = if root != hwnd && GetWindow(root, GW_OWNER) == hwnd {
                root
            } else {
                hwnd
            };
            if IsDialogMessageW(dialog, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        drop(app);
        if initialized {
            RoUninitialize();
        }
        Ok(())
    }
}
unsafe fn paint_path(hwnd: HWND, dc: HDC) {
    unsafe {
        let mut bounds = zeroed();
        GetClientRect(hwnd, &mut bounds);
        let font = SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT;
        let fonts = Fonts {
            normal: font,
            small: font,
            bold: font,
            title: font,
            panel: font,
        };
        let c = Canvas { dc, fonts: &fonts };
        c.fill(bounds, INPUT);
        c.border(bounds, BOUNDARY);
        let len = GetWindowTextLengthW(hwnd);
        let mut buffer = vec![0u16; len as usize + 1];
        GetWindowTextW(hwnd, buffer.as_mut_ptr(), len + 1);
        let full = String::from_utf16_lossy(&buffer[..len as usize]);
        let root = GetAncestor(hwnd, GA_ROOT);
        let app = GetWindowLongPtrW(root, GWLP_USERDATA) as *const App;
        let incomplete = !app.is_null()
            && (*app)
                .summary_cache
                .as_ref()
                .is_some_and(|s| !s.paths_complete);
        let text = if full.contains('\n') {
            format!(
                "{} {}executable paths",
                full.lines().count(),
                if incomplete { "known " } else { "" }
            )
        } else {
            let chars: Vec<_> = full.chars().collect();
            let maxwidth = (bounds.right - 16).max(1);
            let measure = |text: &str| {
                let text = wide(text);
                let old = SelectObject(dc, font);
                let mut size = zeroed();
                GetTextExtentPoint32W(dc, text.as_ptr(), (text.len() - 1) as i32, &mut size);
                SelectObject(dc, old);
                size.cx
            };
            if measure(&full) <= maxwidth {
                full
            } else {
                let suffix = full.rsplit(['\\', '/']).next().unwrap_or(&full);
                let mut prefix = chars.len().saturating_sub(suffix.chars().count()).min(36);
                loop {
                    let text = format!(
                        "{}…\\{}",
                        chars.iter().take(prefix).collect::<String>(),
                        suffix
                    );
                    if measure(&text) <= maxwidth || prefix <= 3 {
                        break text;
                    }
                    prefix -= 1;
                }
            }
        };
        c.text(
            8,
            0,
            bounds.right - 16,
            bounds.bottom,
            &text,
            font,
            INK,
            false,
        );
    }
}
unsafe extern "system" fn combo_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT | WM_PRINTCLIENT => {
                let mut ps = zeroed();
                let dc = if msg == WM_PAINT {
                    BeginPaint(hwnd, &mut ps)
                } else {
                    wp as HDC
                };
                let mut bounds = zeroed();
                GetClientRect(hwnd, &mut bounds);
                let font = SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT;
                let fonts = Fonts {
                    normal: font,
                    small: font,
                    bold: font,
                    title: font,
                    panel: font,
                };
                let c = Canvas { dc, fonts: &fonts };
                c.fill(bounds, INPUT);
                c.border(bounds, BOUNDARY);
                let mut text = vec![0u16; 256];
                let count = GetWindowTextW(hwnd, text.as_mut_ptr(), 256);
                let arrow = (bounds.bottom / 4).max(4);
                c.text(
                    8,
                    0,
                    bounds.right - bounds.bottom - 8,
                    bounds.bottom,
                    &String::from_utf16_lossy(&text[..count as usize]),
                    font,
                    INK,
                    false,
                );
                let x = bounds.right - bounds.bottom / 2;
                let y = bounds.bottom / 2;
                c.line(
                    &[
                        POINT {
                            x: x - arrow,
                            y: y - arrow / 2,
                        },
                        POINT {
                            x,
                            y: y + arrow / 2,
                        },
                        POINT {
                            x: x + arrow,
                            y: y - arrow / 2,
                        },
                    ],
                    MUTED,
                    1,
                );
                if GetFocus() == hwnd {
                    let mut focus = bounds;
                    InflateRect(&mut focus, -3, -3);
                    DrawFocusRect(dc, &focus);
                }
                if msg == WM_PAINT {
                    EndPaint(hwnd, &ps);
                }
                0
            }
            WM_ERASEBKGND => 1,
            WM_SETFOCUS | WM_KILLFOCUS => {
                InvalidateRect(hwnd, null(), 0);
                DefSubclassProc(hwnd, msg, wp, lp)
            }
            _ => DefSubclassProc(hwnd, msg, wp, lp),
        }
    }
}
// Diagnostic-only repeated values exercise the full retention window. This
// clone is never observed by the live recorder or passed to the CSV exporter.
unsafe fn stress_full_retention(hwnd: HWND, ptr: *mut App, dir: &Path) -> io::Result<String> {
    unsafe {
        let mut diagnostic = (*ptr).history.clone();
        let end = HISTORY_SECONDS;
        let template =
            diagnostic.system.back().cloned().ok_or_else(|| {
                io::Error::other("No completed system frame for retention stress")
            })?;
        diagnostic.times = (0..MAX_POINTS)
            .map(|index| index as u32 * COLLECTION_SECONDS)
            .collect();
        diagnostic.system = (0..MAX_POINTS)
            .map(|index| {
                let mut sample = template.clone();
                sample.at = index as u32 * COLLECTION_SECONDS;
                sample.captured_unix_ms = template
                    .captured_unix_ms
                    .saturating_sub((end - sample.at) as u64 * 1000);
                sample
            })
            .collect();
        diagnostic.at = end;
        diagnostic.latest.at = end;
        for track in diagnostic.tracks.values_mut() {
            if track.exited_at.is_none() {
                track.first_observed = 0;
                track.last_observed = end;
                if let Some(point) = track.current {
                    track.points = (0..MAX_POINTS)
                        .map(|index| Point {
                            at: index as u32 * COLLECTION_SECONDS,
                            ..point
                        })
                        .collect();
                    track.current = track.points.back().copied();
                    track.unavailable.clear();
                }
            }
        }
        let tracks = diagnostic.tracks.len();
        let snapshots = diagnostic.times.len();
        let old_frozen = (*ptr).frozen.replace(diagnostic);
        let old_grouping = (*ptr).grouping;
        let old_query = std::mem::take(&mut (*ptr).query);
        (*ptr).grouping = Grouping::Name;
        let refresh_started = Instant::now();
        (*ptr).refresh_rows(true);
        let rows_ms = refresh_started.elapsed().as_millis();
        let buffer_started = Instant::now();
        sync_native(ptr);
        let buffers_ms = buffer_started.elapsed().as_millis();
        let dc = GetDC(hwnd);
        let mut client = zeroed();
        GetClientRect(hwnd, &mut client);
        if !(*ptr).ensure_buffer(dc, client.right, client.bottom) {
            ReleaseDC(hwnd, dc);
            (*ptr).frozen = old_frozen;
            (*ptr).grouping = old_grouping;
            (*ptr).query = old_query;
            return Err(io::Error::last_os_error());
        }
        let before = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
        let render_started = Instant::now();
        for _ in 0..100 {
            paint_main(hwnd, ptr, (*ptr).buffer);
            SendMessageW(
                (*ptr).inspector,
                WM_PRINTCLIENT,
                (*ptr).buffer as usize,
                PRF_CLIENT as isize,
            );
        }
        let render_ms = render_started.elapsed().as_millis();
        let after = GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS);
        ReleaseDC(hwnd, dc);
        let mut counters: windows_sys::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS_EX2 =
            zeroed();
        counters.cb = std::mem::size_of_val(&counters) as u32;
        let measured = windows_sys::Win32::System::ProcessStatus::K32GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&mut counters
                as *mut windows_sys::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS_EX2)
                .cast(),
            counters.cb,
        ) != 0;
        let image = screenshot(hwnd, &dir.join("diagnostic-full-retention.bmp"));
        (*ptr).frozen = old_frozen;
        (*ptr).grouping = old_grouping;
        (*ptr).query = old_query;
        (*ptr).refresh_rows(true);
        sync_native(ptr);
        image?;
        if before == 0 || before != after {
            return Err(io::Error::other(format!(
                "Full-retention redraw GDI resources changed: {before} -> {after}"
            )));
        }
        Ok(format!(
            "diagnostic_tracks={tracks}\ndiagnostic_snapshots={snapshots}\ndiagnostic_name_refresh_ms={rows_ms}\ndiagnostic_chart_buffers_ms={buffers_ms}\ndiagnostic_100_redraw_ms={render_ms}\ndiagnostic_gdi_before={before}\ndiagnostic_gdi_after={after}\ndiagnostic_memory_measured={measured}\ndiagnostic_private_commit_bytes={}\ndiagnostic_working_set_bytes={}",
            counters.PrivateUsage, counters.WorkingSetSize
        ))
    }
}
unsafe fn sync_recording_label(ptr: *mut App) {
    unsafe {
        let status = collection_status(
            (*ptr).received_frame,
            (*ptr)
                .worker
                .as_ref()
                .is_some_and(|worker| worker.is_finished()),
            (*ptr).received_at.elapsed(),
            (*ptr).error.is_some(),
        );
        let paused = (*ptr).frozen.as_ref().map(|h| {
            h.system
                .back()
                .map(|sample| wall_time(sample.captured_unix_ms))
                .unwrap_or_else(|| "—".into())
        });
        set_text(
            (*ptr).control(ID_LIVE),
            &recording_text(paused.as_deref(), status),
        );
    }
}
unsafe fn clipboard_text(hwnd: HWND) -> io::Result<String> {
    unsafe {
        open_clipboard(hwnd)?;
        let memory = GetClipboardData(13);
        let result = if memory.is_null() {
            Err(io::Error::other(format!(
                "GetClipboardData: {}",
                io::Error::last_os_error()
            )))
        } else {
            let data = GlobalLock(memory);
            if data.is_null() {
                Err(io::Error::other(format!(
                    "GlobalLock: {}",
                    io::Error::last_os_error()
                )))
            } else {
                let length = (GlobalSize(memory) / 2).min(1024 * 1024);
                let text = std::slice::from_raw_parts(data.cast::<u16>(), length);
                let end = text.iter().position(|c| *c == 0).unwrap_or(text.len());
                let text = String::from_utf16_lossy(&text[..end]);
                GlobalUnlock(memory);
                Ok(text)
            }
        };
        CloseClipboard();
        result
    }
}
unsafe fn assert_button_labels(ptr: *mut App) {
    unsafe {
        let buttons = (*ptr).controls.clone();
        for control in buttons {
            let mut class = [0u16; 32];
            let count = GetClassNameW(control.hwnd, class.as_mut_ptr(), 32);
            if String::from_utf16_lossy(&class[..count as usize]) != "Button" {
                continue;
            }
            let mut text = vec![0u16; 512];
            let count = GetWindowTextW(control.hwnd, text.as_mut_ptr(), 512);
            let dc = GetDC(control.hwnd);
            let font = SendMessageW(control.hwnd, WM_GETFONT, 0, 0) as HFONT;
            let old = SelectObject(dc, font);
            let mut size = zeroed();
            GetTextExtentPoint32W(dc, text.as_ptr(), count, &mut size);
            let mut bounds = zeroed();
            GetClientRect(control.hwnd, &mut bounds);
            if (ID_LEGEND..ID_LEGEND + 5).contains(&control.id) {
                let factor = (*ptr).viewport.scale as i32;
                let label = String::from_utf16_lossy(&text[..count as usize]);
                let (_, value) = label.rsplit_once("  ").unwrap_or((&label, ""));
                let value = wide(value);
                let mut measured = zeroed();
                GetTextExtentPoint32W(dc, value.as_ptr(), (value.len() - 1) as i32, &mut measured);
                SelectObject(dc, old);
                ReleaseDC(control.hwnd, dc);
                smoke_assert(
                    ptr,
                    measured.cx <= bounds.right - 28 * factor / 96
                        && bounds.bottom >= 33 * factor / 96,
                    &format!(
                        "legend {} exact numeric value owns a complete readable line at {}%",
                        control.id,
                        (*ptr).prefs.ui_zoom
                    ),
                );
                continue;
            }
            SelectObject(dc, old);
            ReleaseDC(control.hwnd, dc);
            let style = GetWindowLongPtrW(control.hwnd, GWL_STYLE) as u32;
            let padding = if style & BS_TYPEMASK as u32 == BS_AUTOCHECKBOX as u32 {
                38
            } else {
                16
            };
            let available = bounds.right - padding * (*ptr).viewport.scale as i32 / 96;
            smoke_assert(
                ptr,
                size.cx <= available,
                &format!(
                    "native button {} exact label fits at {}%",
                    control.id,
                    (*ptr).prefs.ui_zoom
                ),
            );
        }
    }
}
unsafe fn assert_overview_layout(ptr: *mut App) {
    unsafe {
        let v = (*ptr).viewport;
        let physical = |r: RECT| {
            let (x, y) = v.physical(r.left, r.top);
            let (right, bottom) = v.physical(r.right, r.bottom);
            RECT {
                left: x,
                top: y,
                right,
                bottom,
            }
        };
        let native = |id| {
            let mut r = zeroed();
            GetWindowRect((*ptr).control(id), &mut r);
            let mut a = POINT {
                x: r.left,
                y: r.top,
            };
            let mut b = POINT {
                x: r.right,
                y: r.bottom,
            };
            ScreenToClient((*ptr).hwnd, &mut a);
            ScreenToClient((*ptr).hwnd, &mut b);
            RECT {
                left: a.x,
                top: a.y,
                right: b.x,
                bottom: b.y,
            }
        };
        let chart = (*ptr).overview_chart();
        let axes = physical(rect(
            chart.left,
            chart.bottom + 3,
            chart.right - chart.left,
            18,
        ));
        let caption = physical((*ptr).overview_caption_bounds());
        let legends: Vec<_> = (0..5).map(|i| native(ID_LEGEND + i)).collect();
        let legend_top = legends.iter().map(|r| r.top).min().unwrap();
        let legend_bottom = legends.iter().map(|r| r.bottom).max().unwrap();
        let data = native(ID_OVERVIEW_VALUES);
        let panel = physical((*ptr).layout.overview);
        let label = format!(
            "All processes + other used RAM · {} recorded",
            duration_label((*ptr).overview_cache.as_ref().map_or(0, |p| p.recorded))
        );
        let text = wide(&label);
        let dc = GetDC((*ptr).hwnd);
        let old = SelectObject(dc, (*ptr).native_fonts.small);
        let mut size = zeroed();
        GetTextExtentPoint32W(dc, text.as_ptr(), (text.len() - 1) as i32, &mut size);
        let mut annotations_fit = true;
        let mut annotation_geometry = Vec::new();
        if let Some(plot) = &(*ptr).overview_cache
            && let Some(sample) = plot.samples.last()
            && let Some((used, available)) = sample.used.zip(sample.available)
        {
            let endpoint = chart.bottom
                - (used as f64 / plot.capacity.max(1) as f64 * (chart.bottom - chart.top) as f64)
                    as i32;
            let labels = overview_annotations(chart, endpoint);
            for (bounds, label, font) in [
                (labels.used, bytes(used), (*ptr).native_fonts.bold),
                (
                    labels.available_label,
                    "Available".into(),
                    (*ptr).native_fonts.small,
                ),
                (
                    labels.available_value,
                    bytes(available),
                    (*ptr).native_fonts.small,
                ),
            ] {
                SelectObject(dc, font);
                let label = wide(&label);
                let mut measured = zeroed();
                GetTextExtentPoint32W(dc, label.as_ptr(), (label.len() - 1) as i32, &mut measured);
                let bounds = physical(bounds);
                annotation_geometry.push(format!(
                    "text{}x{} box{}x{} right{}/{} bottom{}/{}",
                    measured.cx,
                    measured.cy,
                    bounds.right - bounds.left,
                    bounds.bottom - bounds.top,
                    bounds.right,
                    panel.right,
                    bounds.bottom,
                    legend_top
                ));
                annotations_fit &= measured.cx <= bounds.right - bounds.left
                    && measured.cy <= bounds.bottom - bounds.top
                    && bounds.right <= panel.right
                    && bounds.bottom <= legend_top;
            }
            annotations_fit &= labels.available_value.bottom + 6 <= labels.used.top;
        }
        SelectObject(dc, old);
        ReleaseDC((*ptr).hwnd, dc);
        smoke_assert(
            ptr,
            chart.bottom - chart.top >= 48
                && (chart.bottom - chart.top >= 130
                    || (*ptr).layout.inspector.bottom - (*ptr).layout.inspector.top <= 175)
                && axes.bottom <= legend_top
                && legend_bottom <= caption.top
                && caption.bottom <= data.top
                && data.bottom <= panel.bottom
                && caption.left >= panel.left
                && caption.right <= panel.right
                && size.cx <= caption.right - caption.left
                && size.cy <= caption.bottom - caption.top
                && annotations_fit,
            &format!(
                "overview exact Used/Available annotations, axes, legend, caption and data occupy separate visible rows at {}% (plot{}, inspector{}, annotations_fit{}, {:?})",
                (*ptr).prefs.ui_zoom,
                chart.bottom - chart.top,
                (*ptr).layout.inspector.bottom - (*ptr).layout.inspector.top,
                annotations_fit,
                annotation_geometry
            ),
        );
    }
}
unsafe fn assert_history_render(hwnd: HWND, ptr: *mut App) {
    unsafe {
        let complete_points = (*ptr).selected_series().map_or(0, |series| {
            series
                .points
                .iter()
                .filter(|point| complete_history_value(&point.reading).is_some())
                .count()
        });
        smoke_assert(
            ptr,
            complete_points >= 2,
            "history image oracle has at least two real complete selected samples",
        );
        let mut client = zeroed();
        GetClientRect(hwnd, &mut client);
        let source = GetDC(hwnd);
        let dc = CreateCompatibleDC(source);
        let bitmap = CreateCompatibleBitmap(source, client.right, client.bottom);
        if dc.is_null() || bitmap.is_null() {
            if !dc.is_null() {
                DeleteDC(dc);
            }
            if !bitmap.is_null() {
                DeleteObject(bitmap);
            }
            ReleaseDC(hwnd, source);
            smoke_assert(
                ptr,
                false,
                "history image oracle allocated native capture surface",
            );
            return;
        }
        let old = SelectObject(dc, bitmap);
        SendMessageW(hwnd, WM_PRINTCLIENT, dc as usize, PRF_CLIENT as isize);
        print_children(hwnd, dc);
        let mut origin = POINT { x: 0, y: 0 };
        ClientToScreen((*ptr).inspector, &mut origin);
        ScreenToClient(hwnd, &mut origin);
        let mut inspector = zeroed();
        GetClientRect((*ptr).inspector, &mut inspector);
        let chart = (*ptr).history_chart();
        let scale = (*ptr).viewport.scale as i32;
        let bounds = RECT {
            left: origin.x + chart.left * scale / 96,
            top: origin.y + (chart.top - (*ptr).ins_scroll) * scale / 96,
            right: origin.x + chart.right * scale / 96,
            bottom: origin.y + (chart.bottom - (*ptr).ins_scroll) * scale / 96,
        };
        let complete_top = origin.y + (chart.top - 22 - (*ptr).ins_scroll) * scale / 96;
        let complete_bottom = origin.y + (chart.bottom + 47 - (*ptr).ins_scroll) * scale / 96;
        let visible = bounds.left >= origin.x.max(0)
            && complete_top >= origin.y.max(0)
            && bounds.right <= (origin.x + inspector.right).min(client.right)
            && complete_bottom <= (origin.y + inspector.bottom).min(client.bottom);
        let geometry = format!(
            "zoom{} scale{} canvas{}x{} minimum{} parent{}x{} origin{},{} inspector{}x{} scroll{} plot[{},{}..{},{}]",
            (*ptr).prefs.ui_zoom,
            scale,
            (*ptr).width,
            (*ptr).height,
            (*ptr).minimum_canvas_height,
            client.right,
            client.bottom,
            origin.x,
            origin.y,
            inspector.right,
            inspector.bottom,
            (*ptr).ins_scroll,
            bounds.left,
            bounds.top,
            bounds.right,
            bounds.bottom
        );
        (*ptr).smoke_geometry.push(geometry.clone());
        let mut green_pixels = 0;
        'pixels: for y in bounds.top.max(0)..bounds.bottom.min(client.bottom) {
            for x in bounds.left.max(0)..bounds.right.min(client.right) {
                if GetPixel(dc, x, y) == GREEN {
                    green_pixels += 1;
                }
                if green_pixels >= 64 {
                    break 'pixels;
                }
            }
        }
        SelectObject(dc, old);
        DeleteObject(bitmap);
        DeleteDC(dc);
        ReleaseDC(hwnd, source);
        smoke_assert(
            ptr,
            visible && green_pixels >= 20,
            &format!(
                "scrolled history plot paints green inside mapped inspector at {}% (visible={visible}, green_pixels={green_pixels}; {geometry})",
                (*ptr).prefs.ui_zoom
            ),
        );
    }
}
unsafe fn assert_table_render(ptr: *mut App) {
    unsafe {
        let table = (*ptr).table;
        let mut client = zeroed();
        GetClientRect(table, &mut client);
        let dc = GetDC(table);
        let memory = CreateCompatibleDC(dc);
        let bitmap = CreateCompatibleBitmap(dc, client.right, client.bottom);
        let old = SelectObject(memory, bitmap);
        SendMessageW(
            table,
            WM_PRINT,
            memory as usize,
            (PRF_CLIENT | PRF_CHILDREN | PRF_ERASEBKGND) as isize,
        );
        let top = SendMessageW(table, LVM_GETTOPINDEX, 0, 0).max(0) as usize;
        let mut row = RECT {
            left: LVIR_BOUNDS as i32,
            ..zeroed()
        };
        SendMessageW(
            table,
            LVM_GETITEMRECT,
            top,
            (&mut row as *mut RECT) as isize,
        );
        let mut text_pixels = 0;
        for y in row.top.max(0)..(row.top + 200).min(client.bottom) {
            for x in 8..client.right.min(800) {
                let pixel = GetPixel(memory, x, y);
                if pixel & 255 > 170 && (pixel >> 8) & 255 > 170 && (pixel >> 16) & 255 > 170 {
                    text_pixels += 1;
                }
            }
        }
        let density = row.bottom - row.top;
        SelectObject(memory, old);
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(table, dc);
        smoke_assert(
            ptr,
            (*ptr).rows.is_empty() || ((*ptr).painted_rows > 0 && text_pixels > 20),
            &format!(
                "native table body renders real row text (painted={}, text_pixels={text_pixels})",
                (*ptr).painted_rows
            ),
        );
        let expected = (if (*ptr).compact { 34 } else { 50 }) * (*ptr).viewport.scale as i32 / 96;
        smoke_assert(
            ptr,
            (density - expected).abs() <= 3,
            &format!(
                "native row density follows shared scale (actual={density}, expected={expected})"
            ),
        );
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;
    #[test]
    fn overview_used_and_available_labels_remain_separate_for_short_and_full_plots() {
        for height in [48, 100, 130] {
            let chart = rect(58, 112, 180, height);
            for used_percent in [0, 56, 99, 100] {
                let endpoint = chart.bottom - height * used_percent / 100;
                let labels = overview_annotations(chart, endpoint);
                assert!(
                    labels.available_label.bottom <= labels.available_value.top,
                    "Available name/value own separate rows"
                );
                assert!(
                    labels.available_value.bottom + 6 <= labels.used.top
                        || labels.used.bottom + 6 <= labels.available_label.top,
                    "Used and Available annotations overlap: plot{height} used{used_percent}%"
                );
                assert!(
                    labels.used.top >= chart.top && labels.used.bottom <= chart.bottom + 27,
                    "Used callout remains between capacity and legend rows"
                );
            }
        }
    }
    unsafe fn native_dashboard_fit_failures(hwnd: HWND, app: &mut App) -> Vec<String> {
        unsafe {
            app.history.observe(Snapshot {
                at: 0,
                total: 32 * 1073741824,
                available: 8 * 1073741824,
                processes: vec![ProcessSample {
                    key: ProcessKey {
                        pid: u32::MAX,
                        created: 17,
                        generation: 0,
                    },
                    name: "WindowFit.exe".into(),
                    image: "C:\\Original executable paths\\WindowFit.exe".into(),
                    private: 10 * 1073741824,
                    commit: 20 * 1073741824,
                    working_set: 10 * 1073741824,
                    parent_pid: 0,
                }],
                ..Snapshot::default()
            });
            app.refresh_rows(true);
            sync_native(&mut *app);
            let mut failures = Vec::new();
            for (width, height, dpi, zoom, text, all_headers) in [
                (1300, 800, 96, 100, 1.0, true),
                (1300, 700, 96, 100, 1.0, true),
                (1560, 1060, 96, 100, 1.0, true),
                (2275, 1400, 168, 100, 1.0, true),
                (1950, 1200, 96, 150, 1.0, true),
                (1950, 1200, 96, 100, 1.5, true),
                (1000, 900, 96, 100, 1.0, false),
                (900, 800, 96, 100, 1.0, false),
            ] {
                let before_case = failures.len();
                app.dpi = dpi;
                app.prefs.ui_zoom = zoom;
                app.text_scale = text;
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    0,
                    0,
                    width,
                    height,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
                layout_window(hwnd, &mut *app);
                let before_overview = app.smoke_failures.len();
                assert_overview_layout(app);
                for issue in &app.smoke_failures[before_overview..] {
                    failures.push(issue.clone());
                }
                let mut client = zeroed();
                GetClientRect(hwnd, &mut client);
                let case = format!("{width}x{height} dpi{dpi} zoom{zoom} text{text}");
                for bar in [SB_HORZ, SB_VERT] {
                    let mut info = SCROLLINFO {
                        cbSize: size_of::<SCROLLINFO>() as u32,
                        fMask: SIF_ALL,
                        ..zeroed()
                    };
                    if GetScrollInfo(hwnd, bar, &mut info) != 0
                        && info.nMax - info.nMin + 1 > info.nPage as i32
                    {
                        failures.push(format!(
                            "{case}: main scrollbar {bar} range{} page{}",
                            info.nMax - info.nMin + 1,
                            info.nPage
                        ));
                    }
                }
                for control in &app.controls {
                    if GetParent(control.hwnd) != hwnd
                        || GetWindowLongPtrW(control.hwnd, GWL_STYLE) as u32 & WS_VISIBLE == 0
                    {
                        continue;
                    }
                    let mut bounds = zeroed();
                    GetWindowRect(control.hwnd, &mut bounds);
                    MapWindowPoints(null_mut(), hwnd, (&mut bounds as *mut RECT).cast(), 2);
                    if bounds.left < 0
                        || bounds.top < 0
                        || bounds.right > client.right
                        || bounds.bottom > client.bottom
                    {
                        failures.push(format!(
                            "{case}: control{} outside actual{}x{} [{},{}..{},{}]",
                            control.id,
                            client.right,
                            client.bottom,
                            bounds.left,
                            bounds.top,
                            bounds.right,
                            bounds.bottom
                        ));
                    }
                }
                let mut inspector_client = zeroed();
                GetClientRect(app.inspector, &mut inspector_client);
                for control in &app.controls {
                    if GetParent(control.hwnd) != app.inspector {
                        continue;
                    }
                    let mut bounds = zeroed();
                    GetWindowRect(control.hwnd, &mut bounds);
                    MapWindowPoints(
                        null_mut(),
                        app.inspector,
                        (&mut bounds as *mut RECT).cast(),
                        2,
                    );
                    if bounds.left < 0 || bounds.right > inspector_client.right {
                        failures.push(format!(
                            "{case}: inspector control{} horizontal clip {}..{} client{}",
                            control.id, bounds.left, bounds.right, inspector_client.right
                        ));
                    }
                }
                let mut table_client = zeroed();
                GetClientRect(app.table, &mut table_client);
                let columns: i32 = (0..6)
                    .map(|i| SendMessageW(app.table, LVM_GETCOLUMNWIDTH, i, 0) as i32)
                    .sum();
                if all_headers && columns > table_client.right {
                    failures.push(format!(
                        "{case}: six headers exceed table client ({columns}>{})",
                        table_client.right
                    ));
                }
                let dc = GetDC(app.table);
                let old = SelectObject(dc, app.native_fonts.normal);
                for column in 1..6 {
                    let mut label = vec![0u16; 256];
                    let mut native_column = LVCOLUMNW {
                        mask: LVCF_TEXT,
                        pszText: label.as_mut_ptr(),
                        cchTextMax: label.len() as i32,
                        ..zeroed()
                    };
                    SendMessageW(
                        app.table,
                        LVM_GETCOLUMNW,
                        column,
                        (&mut native_column as *mut LVCOLUMNW) as isize,
                    );
                    let end = label.iter().position(|c| *c == 0).unwrap();
                    let mut extent = zeroed();
                    GetTextExtentPoint32W(dc, label.as_ptr(), end as i32, &mut extent);
                    let column_width =
                        SendMessageW(app.table, LVM_GETCOLUMNWIDTH, column, 0) as i32;
                    if extent.cx + 16 * app.viewport.scale as i32 / 96 > column_width {
                        failures.push(format!("{case}: header{column} clips exact text"));
                    }
                    for row in &app.rows {
                        let value = wide(&app.cell_text(row, column));
                        GetTextExtentPoint32W(
                            dc,
                            value.as_ptr(),
                            (value.len() - 1) as i32,
                            &mut extent,
                        );
                        if extent.cx + 16 * app.viewport.scale as i32 / 96 > column_width {
                            failures
                                .push(format!("{case}: numeric column{column} clips exact value"));
                        }
                    }
                }
                SelectObject(dc, old);
                ReleaseDC(app.table, dc);
                eprintln!(
                    "native_fit_case={case} actual_client={}x{} six_columns={columns} table_client={} result={}",
                    client.right,
                    client.bottom,
                    table_client.right,
                    failures.len() == before_case
                );
            }
            app.dpi = 96;
            app.prefs.ui_zoom = 200;
            app.text_scale = 1.0;
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                640,
                480,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            layout_window(hwnd, app);
            assert!(
                app.viewport.page_width < 760
                    && app.viewport.page_height < app.minimum_canvas_height,
                "extreme overflow is limited to below the readable viewport minimum"
            );
            SendMessageW(hwnd, WM_HSCROLL, SB_BOTTOM as usize, 0);
            SendMessageW(hwnd, WM_VSCROLL, SB_BOTTOM as usize, 0);
            SendMessageW(app.inspector, WM_VSCROLL, SB_BOTTOM as usize, 0);
            let mut client = zeroed();
            GetClientRect(hwnd, &mut client);
            for id in [ID_ZOOM, ID_HISTORY_VALUES] {
                let mut bounds = zeroed();
                GetWindowRect(app.control(id), &mut bounds);
                MapWindowPoints(null_mut(), hwnd, (&mut bounds as *mut RECT).cast(), 2);
                assert!(
                    bounds.left >= 0
                        && bounds.top >= 0
                        && bounds.right <= client.right
                        && bounds.bottom <= client.bottom,
                    "extreme overflow preserves keyboard/history value reachability: control{id}"
                );
            }
            eprintln!(
                "native_extreme_fallback=640x480 zoom200 only_below_readable_minimum=true last_history_value_and_zoom_reachable=true"
            );
            app.prefs.ui_zoom = 100;
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                1300,
                800,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            app.viewport.x = 0;
            app.viewport.y = 0;
            app.ins_scroll = 0;
            layout_window(hwnd, app);
            failures
        }
    }
    #[test]
    fn first_zero_second_snapshot_publishes_native_dashboard_and_survives_producer_close() {
        #[derive(Debug, PartialEq)]
        struct Published {
            header: String,
            rows: isize,
            title: String,
            private: String,
            overview_text: String,
            progress: isize,
            overview_used: Option<u64>,
            history_value: Option<u64>,
        }
        unsafe fn text(hwnd: HWND) -> String {
            unsafe {
                let len = GetWindowTextLengthW(hwnd);
                let mut buffer = vec![0; len as usize + 1];
                GetWindowTextW(hwnd, buffer.as_mut_ptr(), len + 1);
                String::from_utf16_lossy(&buffer[..len as usize])
            }
        }
        unsafe fn published(app: &App) -> Published {
            unsafe {
                Published {
                    header: text(app.control(ID_SYSTEM)),
                    rows: SendMessageW(app.table, LVM_GETITEMCOUNT, 0, 0),
                    title: text(app.control(ID_TITLE)),
                    private: text(app.control(ID_SUMMARY)),
                    overview_text: text(app.control(ID_OVERVIEW_VALUES)),
                    progress: SendMessageW(app.control(ID_PROGRESS), PBM_GETPOS, 0, 0),
                    overview_used: app
                        .overview_cache
                        .as_ref()
                        .and_then(|plot| plot.samples.last())
                        .and_then(|sample| sample.used),
                    history_value: app
                        .series_cache
                        .as_ref()
                        .and_then(|series| series.points.last())
                        .and_then(|point| point.reading.value),
                }
            }
        }
        let (first, closed) = unsafe {
            let initialized = RoInitialize(RO_INIT_SINGLETHREADED).is_ok();
            let instance = GetModuleHandleW(null());
            let class_name = wide("MemMap.Native.FirstZeroSnapshot.Test");
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class_name.as_ptr(),
                ..zeroed()
            };
            assert_ne!(RegisterClassW(&class), 0, "native regression window class");
            let (sender, receiver) = mpsc::channel();
            let mut app = Box::new(App::with_collection(
                None,
                Some(PathBuf::new()),
                receiver,
                Arc::new(AtomicBool::new(false)),
                None,
                Instant::now(),
            ));
            app.prefs_path = None;
            let hwnd = CreateWindowExW(
                WS_EX_CONTROLPARENT,
                class_name.as_ptr(),
                wide("MemMap zero-frame regression").as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN | WS_HSCROLL | WS_VSCROLL,
                0,
                0,
                1280,
                900,
                null_mut(),
                null_mut(),
                instance,
                (&mut *app as *mut App).cast(),
            );
            assert!(!hwnd.is_null(), "native regression window");
            KillTimer(hwnd, 1);
            sender
                .send(Ok(Snapshot {
                    at: 0,
                    total: 8 * 1073741824,
                    available: 3 * 1073741824,
                    captured_unix_ms: 1700000000000,
                    processes: vec![ProcessSample {
                        key: ProcessKey {
                            pid: 77,
                            created: 123,
                            generation: 0,
                        },
                        name: "FirstAtZero.exe".into(),
                        image: "C:\\Original Path\\FirstAtZero.exe".into(),
                        private: 256 * 1048576,
                        commit: 512 * 1048576,
                        working_set: 256 * 1048576,
                        parent_pid: 0,
                    }],
                    ..Snapshot::default()
                }))
                .unwrap();
            SendMessageW(hwnd, WM_TIMER, 1, 0);
            let first = published(&app);
            drop(sender);
            SendMessageW(hwnd, WM_TIMER, 1, 0);
            let closed = published(&app);
            // Keep the WinRT/native UI fixture on its original STA thread.
            // Re-entering UISettings through a process-cached activation factory
            // after another test's STA has ended is not a valid client lifetime.
            let fit_failures = native_dashboard_fit_failures(hwnd, &mut app);
            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(hwnd);
            RedrawWindow(
                hwnd,
                null(),
                null_mut(),
                RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW,
            );
            let mut button_colors = Vec::new();
            for id in [ID_LEGEND, ID_PAUSE, ID_COPY] {
                let button = app.control(id);
                SetWindowTextW(button, wide("Updated native button").as_ptr());
                let dc = GetDC(button);
                let mut client = zeroed();
                GetClientRect(button, &mut client);
                let color = GetPixel(dc, client.right - 8, 8);
                ReleaseDC(button, dc);
                eprintln!(
                    "native_screen_button{id}_after_settext={color:#08x} expected_panel={PANEL:#08x}"
                );
                button_colors.push((id, color));
            }
            DestroyWindow(hwnd);
            let mut message = zeroed();
            PeekMessageW(&mut message, null_mut(), WM_QUIT, WM_QUIT, PM_REMOVE);
            drop(app);
            UnregisterClassW(class_name.as_ptr(), instance);
            if initialized {
                RoUninitialize();
            }
            for (id, color) in button_colors {
                assert_eq!(
                    color, PANEL,
                    "real screen button{id} stays dark after native WM_SETTEXT (not only WM_PRINT)"
                );
            }
            assert!(
                fit_failures.is_empty(),
                "actual native client fit:\n{}",
                fit_failures.join("\n")
            );
            (first, closed)
        };
        assert!(
            first.header.contains("5.00 GiB / 8.00 GiB"),
            "first at0 header must publish before a second sample: {first:?}"
        );
        assert_eq!(first.rows, 1, "first at0 native table row");
        assert_eq!(first.title, "FirstAtZero.exe");
        assert_eq!(first.private, "256.0 MiB");
        assert!(first.overview_text.contains("5.00 GiB"));
        assert_eq!(first.progress, 625);
        assert_eq!(first.overview_used, Some(5 * 1073741824));
        assert_eq!(first.history_value, Some(256 * 1048576));
        assert_eq!(
            closed, first,
            "completed data remains published after its producer closes"
        );
    }
    #[test]
    fn range_choices_clear_manual_zoom_without_unlatching_follow_off() {
        let mut selected = Some(1800);
        let mut manual = Some(120);
        let mut end = Some(42);
        for choice in [Some(300), Some(900), Some(1800), None] {
            manual = Some(120);
            choose_history_range(&mut selected, &mut manual, &mut end, false, 99, choice);
            assert_eq!(selected, choice);
            assert_eq!(manual, None);
            assert_eq!(end, Some(42));
        }
        choose_history_range(&mut selected, &mut manual, &mut end, true, 99, Some(300));
        assert_eq!(end, None);
        assert_eq!(selected, Some(300));
    }
    #[test]
    fn compact_path_rows_distinguish_same_named_executables_by_original_path() {
        let sample = |pid, path: &str| ProcessSample {
            key: ProcessKey {
                pid,
                created: pid as u64,
                generation: 0,
            },
            name: "same.exe".into(),
            image: path.into(),
            private: 100,
            commit: 200,
            working_set: 100,
            parent_pid: 0,
        };
        let mut history = History::default();
        history.observe(Snapshot {
            at: 0,
            total: 10000,
            available: 5000,
            processes: vec![sample(1, "C:\\A\\same.exe"), sample(2, "D:\\B\\same.exe")],
            ..Snapshot::default()
        });
        let rows = dashboard::tree_rows(
            &history,
            Grouping::Executable,
            false,
            "",
            300,
            Sort::default(),
        );
        let mut labels: Vec<_> = rows
            .iter()
            .filter(|r| r.depth == 0)
            .map(identity_text)
            .collect();
        labels.sort();
        assert_eq!(labels, vec!["C:\\A\\same.exe", "D:\\B\\same.exe"]);
        let names = dashboard::tree_rows(&history, Grouping::Name, false, "", 300, Sort::default());
        assert_eq!(identity_text(&names[0]), "same.exe");
    }
    #[test]
    fn frozen_choice_is_protected_before_resume_and_keeps_its_confirmed_exit() {
        let mut live = History::default();
        let processes: Vec<_> = (1..=MAX_TRACKS as u32 + 1)
            .map(|pid| ProcessSample {
                key: ProcessKey {
                    pid,
                    created: pid as u64,
                    generation: 0,
                },
                name: format!("p{pid}.exe"),
                image: format!("D:\\Full Path\\p{pid}.exe"),
                private: if pid > MAX_TRACKS as u32 { 17 } else { 1000 },
                commit: 2000,
                working_set: 1000,
                parent_pid: 0,
            })
            .collect();
        let selected = Selection::Process(processes.last().unwrap().key);
        live.observe(Snapshot {
            at: 0,
            total: 10000000,
            available: 5000000,
            processes,
            ..Snapshot::default()
        });
        let frozen = live.clone();
        live.observe(Snapshot {
            at: 2,
            ..Snapshot::default()
        });
        live.observe(Snapshot {
            at: 6,
            ..Snapshot::default()
        });
        assert!(dashboard::summary(&live, &selected, 300).is_none());
        protect_displayed_selection(&mut live, Some(&frozen), Some(&selected));
        let retained = dashboard::summary(&live, &selected, 300).unwrap();
        assert_eq!(retained.lifecycle, Lifecycle::Exited);
        assert_eq!(
            retained.paths,
            vec![format!("D:\\Full Path\\p{}.exe", MAX_TRACKS + 1)]
        );
        assert_eq!(retained.exited_at, Some(6));
        live.observe(Snapshot {
            at: 8,
            ..Snapshot::default()
        });
        // Both another paused collection and resume use the same controller.
        protect_displayed_selection(&mut live, Some(&frozen), Some(&selected));
        protect_displayed_selection(&mut live, None, Some(&selected));
        let resumed = dashboard::summary(&live, &selected, 300).unwrap();
        assert_eq!(resumed.exited_at, Some(6));
        assert_eq!(resumed.private.value, None);
        assert_eq!(resumed.paths, retained.paths);
        let history = dashboard::series(&live, &selected, Metric::PrivateWorkingSet, Range::All);
        assert_eq!(history.points.len(), 1);
        assert_eq!(history.current.value, Some(17));
    }
    #[test]
    fn partial_group_keeps_coverage_and_has_no_normal_history_endpoint() {
        let key = |pid| ProcessKey {
            pid,
            created: pid as u64,
            generation: 0,
        };
        let process = |pid, value| ProcessSample {
            key: key(pid),
            name: "partial.exe".into(),
            image: "C:\\partial.exe".into(),
            private: value,
            commit: value * 2,
            working_set: value,
            parent_pid: 0,
        };
        let mut h = History::default();
        h.observe(Snapshot {
            at: 0,
            total: 10000,
            available: 5000,
            processes: vec![process(1, 100), process(2, 200)],
            ..Snapshot::default()
        });
        h.observe(Snapshot {
            at: 2,
            total: 10000,
            available: 5000,
            processes: vec![process(1, 150)],
            observed: vec![
                ProcessObservation {
                    pid: 1,
                    key: Some(key(1)),
                    name: "partial.exe".into(),
                    image: Some("C:\\partial.exe".into()),
                    parent_pid: 0,
                    error: None,
                },
                ProcessObservation {
                    pid: 2,
                    key: Some(key(2)),
                    name: "partial.exe".into(),
                    image: Some("C:\\partial.exe".into()),
                    parent_pid: 0,
                    error: Some("counter denied".into()),
                },
            ],
            ..Snapshot::default()
        });
        let selected = Selection::Name("partial.exe".into());
        let summary = dashboard::summary(&h, &selected, 300).unwrap();
        let series = dashboard::series(&h, &selected, Metric::PrivateWorkingSet, Range::All);
        assert!(summary.private.partial());
        assert_eq!((summary.private.measured, summary.private.total), (1, 2));
        assert!(reading(&summary.private).contains("Partial"));
        assert_eq!(series.current.value, summary.private.value);
        assert_eq!((series.current.measured, series.current.total), (1, 2));
        assert_eq!(complete_history_value(&series.current), None);
        assert_eq!(complete_history_value(&series.points[0].reading), Some(300));
    }
    #[test]
    fn stopped_and_stale_recorders_never_claim_live_or_recording_continues() {
        for paused in [None, Some("12:34:56")] {
            let stopped = collection_status(true, true, Duration::ZERO, false);
            let text = recording_text(paused, stopped);
            assert!(text.contains("Collector stopped"));
            assert!(!text.contains("Live") && !text.contains("Recording continues"));
            let stale = collection_status(true, false, Duration::from_secs(7), false);
            assert_eq!(stale, CollectionStatus::Delayed);
            assert!(recording_text(paused, stale).contains("stale"));
        }
        assert_eq!(
            collection_status(false, false, Duration::ZERO, false),
            CollectionStatus::Collecting
        );
        assert_eq!(
            collection_status(true, false, Duration::ZERO, true),
            CollectionStatus::Unavailable
        );
        assert_eq!(
            recording_text(
                Some("12:34:56"),
                collection_status(true, false, Duration::from_secs(2), false)
            ),
            "Display paused at 12:34:56 · Recording continues"
        );
    }
}
