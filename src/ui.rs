use mem_map::{model::*, sampler};
use std::{
    f64::consts::TAU,
    fs::File,
    io::{self, BufWriter, Write},
    mem::{size_of, zeroed},
    path::PathBuf,
    ptr::{null, null_mut},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{
        LibraryLoader::GetModuleHandleW,
        Threading::{
            GR_GDIOBJECTS, GetCurrentProcess, GetCurrentProcessId, GetGuiResources, GetProcessTimes,
        },
    },
    UI::{
        Controls::{Dialogs::*, EM_SETCUEBANNER, EM_SETLIMITTEXT, EM_SETSEL, SetScrollInfo},
        HiDpi::*,
        Input::KeyboardAndMouse::*,
        WindowsAndMessaging::*,
    },
};

const BG: u32 = rgb(244, 246, 248);
const INK: u32 = rgb(26, 39, 48);
const MUTED: u32 = rgb(103, 117, 129);
const BORDER: u32 = rgb(222, 228, 233);
const ACCENT: u32 = rgb(0, 124, 104);
const RED: u32 = rgb(185, 72, 50);
const COLORS: [u32; 8] = [
    rgb(0, 150, 125),
    rgb(80, 127, 218),
    rgb(162, 119, 211),
    rgb(229, 167, 69),
    rgb(215, 111, 100),
    rgb(120, 156, 173),
    rgb(157, 169, 180),
    rgb(226, 234, 239),
];
const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn bytes(n: u64) -> String {
    if n >= 1024 * 1024 * 1024 {
        format!("{:.2} GiB", n as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{:.1} MiB", n as f64 / (1024.0 * 1024.0))
    }
}
fn delta(n: i64) -> String {
    format!(
        "{}{}",
        if n >= 0 { "+" } else { "−" },
        bytes(n.unsigned_abs())
    )
}
fn clock(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
fn window_label(window: Option<u32>) -> String {
    match window {
        Some(seconds) if seconds % 60 == 0 => format!("{}m", seconds / 60),
        Some(seconds) => format!("{seconds}s"),
        None => "Session".into(),
    }
}
fn rect(x: i32, y: i32, w: i32, h: i32) -> RECT {
    RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    }
}
fn contains(r: RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

#[derive(Clone, Copy)]
struct Fonts {
    normal: HFONT,
    small: HFONT,
    bold: HFONT,
    title: HFONT,
    number: HFONT,
}
unsafe fn search_font(dpi: u32) -> HFONT {
    unsafe {
        CreateFontW(
            -14 * dpi as i32 / 96,
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            FIXED_PITCH as u32 | FF_MODERN as u32,
            wide("Consolas").as_ptr(),
        )
    }
}
impl Fonts {
    unsafe fn new() -> Self {
        unsafe fn font(height: i32, weight: u32) -> HFONT {
            // SAFETY: valid font attributes and terminated face name; OS owns no input pointers.
            unsafe {
                CreateFontW(
                    -height,
                    0,
                    0,
                    0,
                    weight as i32,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32,
                    CLEARTYPE_QUALITY as u32,
                    FIXED_PITCH as u32 | FF_MODERN as u32,
                    wide("Consolas").as_ptr(),
                )
            }
        }
        unsafe {
            Self {
                normal: font(14, FW_NORMAL),
                small: font(12, FW_NORMAL),
                bold: font(14, FW_SEMIBOLD),
                title: font(24, FW_SEMIBOLD),
                number: font(30, FW_SEMIBOLD),
            }
        }
    }
}

struct Canvas<'a> {
    dc: HDC,
    fonts: &'a Fonts,
}
impl Canvas<'_> {
    fn paragraph(&self, r: RECT, s: &str, color: u32) {
        unsafe {
            let old = SelectObject(self.dc, self.fonts.small);
            SetBkMode(self.dc, TRANSPARENT as i32);
            SetTextColor(self.dc, color);
            let mut bounds = r;
            let text = wide(s);
            DrawTextW(
                self.dc,
                text.as_ptr(),
                (text.len() - 1) as i32,
                &mut bounds,
                DT_WORDBREAK | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
            SelectObject(self.dc, old);
        }
    }
    fn fill(&self, r: RECT, color: u32) {
        unsafe {
            let brush = CreateSolidBrush(color);
            FillRect(self.dc, &r, brush);
            DeleteObject(brush);
        }
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "Drawing calls keep geometry, font and alignment explicit"
    )]
    fn text(&self, x: i32, y: i32, w: i32, h: i32, s: &str, font: HFONT, color: u32, right: bool) {
        unsafe {
            let old = SelectObject(self.dc, font);
            SetBkMode(self.dc, TRANSPARENT as i32);
            SetTextColor(self.dc, color);
            let mut r = rect(x, y, w, h);
            let text = wide(s);
            DrawTextW(
                self.dc,
                text.as_ptr(),
                (text.len() - 1) as i32,
                &mut r,
                DT_SINGLELINE
                    | DT_VCENTER
                    | DT_END_ELLIPSIS
                    | DT_NOPREFIX
                    | if right { DT_RIGHT } else { DT_LEFT },
            );
            SelectObject(self.dc, old);
        }
    }
    fn line(&self, points: &[POINT], color: u32, width: i32) {
        unsafe {
            if points.len() < 2 {
                return;
            }
            let pen = CreatePen(PS_SOLID, width, color);
            let old = SelectObject(self.dc, pen);
            Polyline(self.dc, points.as_ptr(), points.len() as i32);
            SelectObject(self.dc, old);
            DeleteObject(pen);
        }
    }
    fn circle(&self, x: i32, y: i32, radius: i32, color: u32) {
        unsafe {
            let brush = CreateSolidBrush(color);
            let old = SelectObject(self.dc, brush);
            let oldpen = SelectObject(self.dc, GetStockObject(NULL_PEN));
            Ellipse(self.dc, x - radius, y - radius, x + radius, y + radius);
            SelectObject(self.dc, oldpen);
            SelectObject(self.dc, old);
            DeleteObject(brush);
        }
    }
    fn polygon(&self, points: &[POINT], color: u32) {
        unsafe {
            let brush = CreateSolidBrush(color);
            let old = SelectObject(self.dc, brush);
            let oldpen = SelectObject(self.dc, GetStockObject(NULL_PEN));
            Polygon(self.dc, points.as_ptr(), points.len() as i32);
            SelectObject(self.dc, oldpen);
            SelectObject(self.dc, old);
            DeleteObject(brush);
        }
    }
    fn panel(&self, r: RECT) {
        self.fill(r, BORDER);
        self.fill(
            rect(
                r.left + 1,
                r.top + 1,
                r.right - r.left - 2,
                r.bottom - r.top - 2,
            ),
            rgb(255, 255, 255),
        );
    }
    fn button(&self, r: RECT, text: &str, selected: bool) {
        self.fill(r, if selected { ACCENT } else { rgb(233, 238, 241) });
        let compact = r.right - r.left < 90;
        let padding = if compact { 5 } else { 10 };
        self.text(
            r.left + padding,
            r.top,
            r.right - r.left - 2 * padding,
            r.bottom - r.top,
            text,
            if compact {
                self.fonts.small
            } else {
                self.fonts.bold
            },
            if selected { rgb(255, 255, 255) } else { INK },
            false,
        );
    }
}

#[derive(Clone, Copy)]
enum Action {
    Pause,
    Export,
    Group(Grouping),
    Drill,
    Back,
    Pie(usize),
    Overview(bool),
    RefreshComparison,
    Compare(usize),
    Growth,
    Inactive,
    Metric,
    Range(u32),
    Select(usize),
    Parent(ProcessKey),
    TimeZoom(bool),
    UiZoom(i32),
}

struct App {
    history: History,
    comparison: Comparison,
    comparison_ready: bool,
    overview: bool,
    fonts: Fonts,
    grouping: Grouping,
    scope: Option<Selection>,
    growth: bool,
    inactive: bool,
    commit: bool,
    range: u32,
    selection: Option<Selection>,
    rows: Vec<Row>,
    scroll: usize,
    query: String,
    search: HWND,
    growth_input: HWND,
    growth_window: Option<u32>,
    growth_error: Option<String>,
    search_font: HFONT,
    pause: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    receiver: mpsc::Receiver<Result<Snapshot, String>>,
    error: Option<String>,
    notice: String,
    own_ram: u64,
    own_ws: u64,
    width: i32,
    height: i32,
    dpi: u32,
    ui_zoom: u32,
    viewport: Viewport,
    layout_busy: bool,
    font_scale: u32,
    wheel_delta: i32,
    wheel_mode: u8,
    hits: Vec<(RECT, Action)>,
    graph_rect: RECT,
    comparison_rect: RECT,
    pie: Vec<(f64, f64, Option<Selection>)>,
    pie_circle: (i32, i32, i32),
    hover: Option<(i32, i32)>,
    smoke: Option<PathBuf>,
    smoke_step: u8,
    smoke_paused_points: usize,
    smoke_paused_at: u32,
    smoke_failure: Option<String>,
    smoke_native_done: bool,
    launched: Instant,
    buffer: HDC,
    bitmap: HBITMAP,
    original_bitmap: HGDIOBJ,
    buffer_size: (i32, i32),
}

impl App {
    unsafe fn new(smoke: Option<PathBuf>) -> Self {
        let (sender, receiver) = mpsc::sync_channel(1);
        let running = Arc::new(AtomicBool::new(true));
        let pause = Arc::new(AtomicBool::new(false));
        let run = running.clone();
        let paused = pause.clone();
        let launched = Instant::now();
        let worker = thread::spawn(move || {
            while run.load(Ordering::Relaxed) {
                if !paused.load(Ordering::Relaxed) {
                    let sample =
                        sampler::capture(launched.elapsed().as_secs().min(u32::MAX as u64) as u32)
                            .map_err(|e| e.to_string());
                    if sender.try_send(sample).is_err() && !run.load(Ordering::Relaxed) {
                        break;
                    }
                }
                thread::park_timeout(Duration::from_secs(2));
            }
        });
        unsafe {
            Self {
                history: History::default(),
                comparison: Comparison::default(),
                comparison_ready: false,
                overview: true,
                fonts: Fonts::new(),
                grouping: Grouping::Name,
                scope: None,
                growth: false,
                inactive: false,
                commit: false,
                range: 1800,
                selection: None,
                rows: Vec::new(),
                scroll: 0,
                query: String::new(),
                search: null_mut(),
                growth_input: null_mut(),
                growth_window: Some(300),
                growth_error: None,
                search_font: null_mut(),
                pause,
                running,
                worker: Some(worker),
                receiver,
                error: None,
                notice: String::new(),
                own_ram: 0,
                own_ws: 0,
                width: 1260,
                height: 890,
                dpi: 96,
                ui_zoom: 100,
                viewport: Viewport::new(1260, 890, 96, 100, 0, 0),
                layout_busy: false,
                font_scale: 96,
                wheel_delta: 0,
                wheel_mode: 0,
                hits: Vec::new(),
                graph_rect: rect(0, 0, 0, 0),
                comparison_rect: rect(0, 0, 0, 0),
                pie: Vec::new(),
                pie_circle: (0, 0, 0),
                hover: None,
                smoke,
                smoke_step: 0,
                smoke_paused_points: 0,
                smoke_paused_at: 0,
                smoke_failure: None,
                smoke_native_done: false,
                launched,
                buffer: null_mut(),
                bitmap: null_mut(),
                original_bitmap: null_mut(),
                buffer_size: (0, 0),
            }
        }
    }
    fn refresh_rows(&mut self) {
        if !self.comparison_ready && self.history.total > 0 {
            self.comparison
                .refresh(&self.history, self.grouping, self.commit);
            self.comparison_ready = true;
        }
        self.rows = self.history.rows_with_window(
            if self.scope.is_some() {
                Grouping::Process
            } else {
                self.grouping
            },
            self.inactive,
            &self.query,
            self.growth,
            self.growth_window,
        );
        if let Some(scope) = &self.scope {
            self.rows.retain(|r|matches!(&r.selection,Selection::Process(key) if self.history.tracks.get(key).is_some_and(|t|self.history.matches(t,scope))));
        }
        self.scroll = self
            .scroll
            .min(self.rows.len().saturating_sub(self.visible_rows()));
        if self.selection.is_none() {
            self.selection = self.rows.first().map(|r| r.selection.clone());
        }
    }
    fn visible_rows(&self) -> usize {
        ((self.height - 174).max(32) / 32) as usize
    }
    fn set_growth_window(&mut self, text: &str) -> bool {
        match parse_growth_window(text) {
            Ok(window) => {
                self.growth_window = window;
                self.growth_error = None;
                self.scroll = 0;
                self.refresh_rows();
                true
            }
            Err(error) => {
                self.growth_error = Some(error.into());
                false
            }
        }
    }
    fn pie_hit(&self, x: i32, y: i32) -> Option<usize> {
        if self.overview {
            return None;
        }
        let (cx, cy, r) = self.pie_circle;
        let dx = x - cx;
        let dy = y - cy;
        if dx * dx + dy * dy > r * r || dx * dx + dy * dy < (r / 2) * (r / 2) {
            return None;
        }
        let angle = ((dy as f64).atan2(dx as f64) + TAU / 4.0).rem_euclid(TAU);
        self.pie
            .iter()
            .position(|(start, end, target)| target.is_some() && angle >= *start && angle < *end)
    }
    fn action(&mut self, hwnd: HWND, action: Action) {
        let mut reveal = None;
        match action {
            Action::Pause => {
                let paused = !self.pause.load(Ordering::Relaxed);
                self.pause.store(paused, Ordering::Relaxed);
                if !paused && let Some(worker) = &self.worker {
                    worker.thread().unpark();
                }
            }
            Action::Export => unsafe {
                PostMessageW(hwnd, WM_APP + 1, 0, 0);
            },
            Action::Group(grouping) => {
                self.grouping = grouping;
                self.comparison_ready = false;
                self.scope = None;
                self.selection = None;
                self.scroll = 0;
            }
            Action::Drill => {
                if let Some(selected) = self
                    .selection
                    .clone()
                    .filter(|s| !matches!(s, Selection::Process(_)))
                {
                    self.scope = Some(selected);
                    self.scroll = 0;
                }
            }
            Action::Back => {
                self.selection = self.scope.take();
                self.scroll = 0;
            }
            Action::Pie(index) => {
                if let Some(selection) = self.pie.get(index).and_then(|slice| slice.2.clone()) {
                    self.selection = Some(selection.clone());
                    self.scope = Some(selection);
                    self.scroll = 0;
                }
            }
            Action::Overview(top_five) => self.overview = top_five,
            Action::RefreshComparison => self.comparison_ready = false,
            Action::Compare(index) => {
                if let Some(member) = self.comparison.members.get(index) {
                    self.selection = Some(member.selection.clone());
                    self.scope = Some(member.selection.clone());
                    self.scroll = 0;
                }
            }
            Action::Growth => {
                self.growth = !self.growth;
                self.scroll = 0;
            }
            Action::Inactive => {
                self.inactive = !self.inactive;
                self.scroll = 0;
            }
            Action::Metric => {
                self.commit = !self.commit;
                self.comparison_ready = false;
            }
            Action::Range(range) => self.range = range,
            Action::TimeZoom(inward) => self.range = zoom_time(self.range, inward),
            Action::UiZoom(direction) => {
                self.ui_zoom = zoom_interface(self.ui_zoom, direction);
                self.hover = None;
                unsafe {
                    PostMessageW(hwnd, WM_APP + 2, 0, 0);
                }
            }
            Action::Select(index) => {
                if let Some(row) = self.rows.get(index) {
                    self.selection = Some(row.selection.clone());
                }
            }
            Action::Parent(child) => {
                if let Some(parent) = self.history.parent_target(child) {
                    self.grouping = Grouping::Process;
                    self.comparison_ready = false;
                    self.scope = None;
                    self.query.clear();
                    self.inactive = true;
                    self.selection = Some(Selection::Process(parent));
                    reveal = Some(parent);
                    // Clear the native edit after this borrow ends: SetWindowTextW
                    // synchronously reenters the window's WM_COMMAND handler.
                    unsafe {
                        PostMessageW(hwnd, WM_APP + 3, 0, 0);
                    }
                } else {
                    self.notice = "Parent is outside retained history or unavailable".into();
                }
            }
        }
        self.refresh_rows();
        if let Some(parent) = reveal
            && let Some(index) = self
                .rows
                .iter()
                .position(|row| row.selection == Selection::Process(parent))
        {
            self.scroll = index
                .saturating_sub(self.visible_rows() / 2)
                .min(self.rows.len().saturating_sub(self.visible_rows()));
            let row_y = 142 + (index - self.scroll) as i32 * 32;
            self.viewport.x = 0;
            self.viewport.y = (row_y - self.viewport.page_height / 2)
                .clamp(0, self.height - self.viewport.page_height);
            unsafe {
                PostMessageW(hwnd, WM_APP + 2, 0, 0);
            }
        }
        unsafe {
            InvalidateRect(hwnd, null(), 0);
        }
    }
    fn export_to(&self, path: &std::path::Path) -> io::Result<()> {
        let mut file = BufWriter::new(File::create(path)?);
        self.history.export(&mut file)?;
        file.flush()
    }
    fn tick(&mut self, hwnd: HWND) {
        let mut changed = false;
        while let Ok(result) = self.receiver.try_recv() {
            if self.pause.load(Ordering::Relaxed) {
                continue;
            }
            match result {
                Ok(frame) => {
                    if let Some(own) = frame
                        .processes
                        .iter()
                        .find(|p| p.key.pid == unsafe { GetCurrentProcessId() })
                    {
                        self.own_ram = own.private;
                        self.own_ws = own.working_set;
                    }
                    self.history.observe(frame);
                    self.error = None;
                    self.refresh_rows();
                }
                Err(error) => self.error = Some(error),
            }
            changed = true;
        }
        if changed {
            // A hidden diagnostic window must keep the same persistent drawing
            // buffer as a visible one, or its reported footprint would be too low.
            if self.smoke.is_some() {
                unsafe {
                    let dc = GetDC(hwnd);
                    if self.ensure_buffer(
                        dc,
                        self.width * self.dpi as i32 / 96,
                        self.height * self.dpi as i32 / 96,
                    ) {
                        self.render(self.buffer);
                    }
                    ReleaseDC(hwnd, dc);
                }
            }
            unsafe {
                InvalidateRect(hwnd, null(), 0);
            }
        }
        if self.smoke.is_some() {
            self.smoke_tick(hwnd);
        }
    }
    fn smoke_assert(&mut self, condition: bool, code: &str) {
        if !condition && self.smoke_failure.is_none() {
            self.smoke_failure = Some(code.into());
        }
    }
    fn smoke_tick(&mut self, hwnd: HWND) {
        let elapsed = self.launched.elapsed().as_secs();
        let dir = self.smoke.clone().unwrap();
        if self.history.total == 0 && elapsed < 20 {
            return;
        }
        if elapsed >= 4 && self.smoke_step == 0 {
            self.smoke_assert(
                self.overview && self.comparison.members.len() == 5,
                "initial top-five overview",
            );
            self.smoke_assert(
                self.hits
                    .iter()
                    .any(|(_, action)| matches!(action, Action::Overview(false))),
                "pie switch control",
            );
            self.smoke_assert(
                self.hits
                    .iter()
                    .any(|(_, action)| matches!(action, Action::RefreshComparison)),
                "refresh control",
            );
            self.comparison.members.clear();
            self.action(hwnd, Action::RefreshComparison);
            self.smoke_assert(self.comparison.members.len() == 5, "refresh pins five");
            let pinned: Vec<_> = self
                .comparison
                .members
                .iter()
                .map(|m| m.selection.clone())
                .collect();
            self.action(hwnd, Action::Range(900));
            self.smoke_assert(
                self.comparison
                    .members
                    .iter()
                    .map(|m| &m.selection)
                    .eq(pinned.iter()),
                "range keeps colour identities",
            );
            let screenshot = unsafe { self.screenshot(&dir.join("top-five.bmp")) };
            self.smoke_assert(screenshot.is_ok(), "top-five screenshot");
            if let Some(expected) = self.comparison.members.first().map(|m| m.selection.clone()) {
                self.smoke_assert(
                    self.hits
                        .iter()
                        .any(|(_, action)| matches!(action, Action::Compare(0))),
                    "clickable top-five legend",
                );
                self.action(hwnd, Action::Compare(0));
                self.smoke_assert(
                    self.scope.as_ref() == Some(&expected)
                        && self.selection.as_ref() == Some(&expected),
                    "comparison legend exact selection",
                );
                self.smoke_assert(!self.rows.is_empty() && self.rows.iter().all(|r| matches!(&r.selection, Selection::Process(key) if self.history.tracks.get(key).is_some_and(|t| self.history.matches(t, &expected)))), "comparison legend member filtering");
                self.action(hwnd, Action::Back);
            }
            self.action(hwnd, Action::Overview(false));
            self.smoke_assert(!self.overview, "pie mode action");
            self.action(hwnd, Action::Group(Grouping::Process));
            self.smoke_assert(self.grouping == Grouping::Process, "group action");
            self.smoke_assert(
                self.comparison
                    .members
                    .iter()
                    .all(|m| matches!(m.selection, Selection::Process(_))),
                "group action refreshes comparison identities",
            );
            let parent_row = self.rows.iter().position(|row| matches!(&row.selection,
                Selection::Process(key) if self.history.tracks.get(key).is_some_and(|t|t.parent_name.as_deref()==Some("ChatGPT.exe"))))
                .or_else(|| self.rows.iter().position(|row| matches!(&row.selection,
                    Selection::Process(key) if self.history.tracks.get(key).is_some_and(|t|t.parent_name.is_some()))));
            if let Some(index) = parent_row {
                self.action(hwnd, Action::Select(index));
                let screenshot = unsafe { self.screenshot(&dir.join("parent-detail.bmp")) };
                self.smoke_assert(screenshot.is_ok(), "parent detail screenshot");
            } else {
                self.smoke_assert(false, "no observed parent for detail inspection");
            }
            self.action(hwnd, Action::Growth);
            self.smoke_assert(self.growth, "growth action");
            self.action(hwnd, Action::Metric);
            self.smoke_assert(self.commit, "metric action");
            self.action(hwnd, Action::Range(300));
            self.smoke_assert(self.range == 300, "range action");
            let accepted = self.set_growth_window("90s");
            self.smoke_assert(
                accepted && self.growth_window == Some(90) && self.range == 300,
                "custom growth independent from graph window",
            );
            let rejected = !self.set_growth_window("999m");
            self.smoke_assert(
                rejected && self.growth_window == Some(90),
                "invalid growth preserves prior window",
            );
            self.set_growth_window("5 min");
            self.smoke_assert(
                self.rows.iter().all(|r| r.peak >= r.ram),
                "peak RAM includes the current observation",
            );
            self.action(hwnd, Action::Inactive);
            self.smoke_assert(self.inactive, "inactive action");
            if self.rows.len() > 1 {
                let expected = self.rows[1].selection.clone();
                self.action(hwnd, Action::Select(1));
                self.smoke_assert(self.selection == Some(expected), "row selection action");
            } else {
                self.smoke_assert(false, "insufficient selection rows");
            }
            self.action(hwnd, Action::Group(Grouping::Name));
            let screenshot = unsafe { self.screenshot(&dir.join("pie.bmp")) };
            self.smoke_assert(screenshot.is_ok(), "pie screenshot");
            let pie_index=self.pie.iter().position(|(_,_,target)|matches!(target,Some(Selection::Name(name)) if name=="node.exe")).unwrap_or(0);
            if let Some((start, end, Some(expected))) = self.pie.get(pie_index).cloned() {
                let a = (start + end) / 2.0 - TAU / 4.0;
                let (cx, cy, r) = self.pie_circle;
                let hit = self.pie_hit(
                    cx + (r as f64 * 0.8 * a.cos()) as i32,
                    cy + (r as f64 * 0.8 * a.sin()) as i32,
                );
                self.smoke_assert(hit == Some(pie_index), "pie arc hit testing");
                self.action(hwnd, Action::Pie(pie_index));
                self.smoke_assert(
                    self.scope.as_ref() == Some(&expected)
                        && self.selection.as_ref() == Some(&expected),
                    "pie drill selection",
                );
                self.smoke_assert(!self.rows.is_empty()&&self.rows.iter().all(|row|matches!(&row.selection,Selection::Process(key) if self.history.tracks.get(key).is_some_and(|t|self.history.matches(t,&expected)))),"pie member filtering");
                let screenshot = unsafe { self.screenshot(&dir.join("group-detail.bmp")) };
                self.smoke_assert(screenshot.is_ok(), "group screenshot");
                self.action(hwnd, Action::Select(0));
                let screenshot = unsafe { self.screenshot(&dir.join("process-detail.bmp")) };
                self.smoke_assert(screenshot.is_ok(), "process screenshot");
                self.action(hwnd, Action::Back);
                self.smoke_assert(self.scope.is_none(), "drill back action");
            } else {
                self.smoke_assert(false, "missing clickable app pie slice");
            }
            self.action(hwnd, Action::Overview(true));
            self.smoke_assert(self.overview, "return to top-five mode");
            self.action(hwnd, Action::Pause);
            self.smoke_assert(self.pause.load(Ordering::Relaxed), "pause flag");
            self.smoke_paused_points = self.history.times.len();
            self.smoke_paused_at = self.history.at;
            self.launched = Instant::now();
            self.smoke_step = 1;
        } else if elapsed >= 4 && self.smoke_step == 1 {
            self.smoke_assert(
                self.history.times.len() == self.smoke_paused_points
                    && self.history.at == self.smoke_paused_at,
                "pause must freeze observations across two sampling intervals",
            );
            self.action(hwnd, Action::Pause);
            self.smoke_assert(!self.pause.load(Ordering::Relaxed), "resume flag");
            self.action(hwnd, Action::Inactive);
            self.action(hwnd, Action::Growth);
            self.action(hwnd, Action::Metric);
            self.smoke_step = 2;
        } else if elapsed >= 8 && self.smoke_step == 2 {
            self.smoke_assert(
                self.history.at > self.smoke_paused_at
                    && self.history.times.len() > self.smoke_paused_points,
                "resume must capture a new observation",
            );
            self.smoke_assert(
                self.history
                    .times
                    .iter()
                    .zip(self.history.times.iter().skip(1))
                    .any(|(a, b)| b - a > 3),
                "resume must preserve the time gap",
            );
            let result = self
                .export_to(&dir.join("history.csv"))
                .and_then(|_| unsafe { self.screenshot(&dir.join("dashboard.bmp")) });
            let diagnostics = self.stress_diagnostics();
            let error = result
                .err()
                .map(|e| e.to_string())
                .or(self.error.clone())
                .or(self.smoke_failure.clone())
                .or_else(|| diagnostics.as_ref().err().map(|e| e.to_string()));
            let report = format!(
                "result={}\nobserved_processes={}\nprivate_bytes={}\nworking_set_bytes={}\nretained_samples={}\ninaccessible={}\naction_assertions={}\n{}\nerror={:?}\n",
                if error.is_none() && self.history.total > 0 {
                    "PASS"
                } else {
                    "FAIL"
                },
                self.history.tracks.len(),
                self.own_ram,
                self.own_ws,
                self.history.times.len(),
                self.history.inaccessible,
                if self.smoke_failure.is_none() {
                    "PASS"
                } else {
                    "FAIL"
                },
                diagnostics.as_deref().unwrap_or("diagnostic stress failed"),
                error
            );
            let _ = std::fs::write(dir.join("smoke.txt"), report);
            self.smoke_step = 3;
            unsafe {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        } else if elapsed >= 20 && self.smoke_step == 0 {
            let _ = std::fs::write(dir.join("smoke.txt"), format!("FAIL: {:?}", self.error));
            unsafe {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
    }
    fn stress_diagnostics(&mut self) -> io::Result<String> {
        // Diagnostic-only load: repeated real counter values exercise the full ring.
        // They are kept separate from displayed/exported, genuinely observed history.
        let cpu_before_stress = unsafe {
            let mut creation = zeroed();
            let mut exit = zeroed();
            let mut kernel: FILETIME = zeroed();
            let mut user: FILETIME = zeroed();
            if GetProcessTimes(
                GetCurrentProcess(),
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            (((kernel.dwHighDateTime as u64) << 32 | kernel.dwLowDateTime as u64)
                + ((user.dwHighDateTime as u64) << 32 | user.dwLowDateTime as u64))
                / 10000
        };
        let frame = sampler::capture(0)?;
        let mut full = History::default();
        for index in 0..MAX_POINTS + 10 {
            full.observe(Snapshot {
                at: index as u32 * 2,
                total: frame.total,
                available: frame.available,
                processes: frame.processes.clone(),
                inaccessible: frame.inaccessible,
            });
        }
        let measured = sampler::capture(0)?;
        let own = measured
            .processes
            .iter()
            .find(|p| p.key.pid == std::process::id())
            .ok_or_else(|| io::Error::other("Cannot measure diagnostic footprint"))?;
        let mut cpu_ms = 0;
        let mut gdi_before = 0;
        let mut gdi_after = 0;
        std::mem::swap(&mut self.history, &mut full);
        self.refresh_rows();
        let render_started = Instant::now();
        unsafe {
            let process = GetCurrentProcess();
            let mut creation = zeroed();
            let mut exit = zeroed();
            let mut kernel: FILETIME = zeroed();
            let mut user: FILETIME = zeroed();
            if GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) != 0 {
                cpu_ms = (((kernel.dwHighDateTime as u64) << 32 | kernel.dwLowDateTime as u64)
                    + ((user.dwHighDateTime as u64) << 32 | user.dwLowDateTime as u64))
                    / 10000;
            }
            let dc = GetDC(null_mut());
            if self.ensure_buffer(dc, self.width, self.height) {
                for _ in 0..5 {
                    self.render(self.buffer);
                }
                gdi_before = GetGuiResources(process, GR_GDIOBJECTS);
                for _ in 0..100 {
                    self.render(self.buffer);
                }
                gdi_after = GetGuiResources(process, GR_GDIOBJECTS);
            }
            ReleaseDC(null_mut(), dc);
        }
        let full_render_ms = render_started.elapsed().as_millis();
        std::mem::swap(&mut self.history, &mut full);
        self.refresh_rows();
        std::hint::black_box(&full);
        if gdi_before == 0 || gdi_after != gdi_before {
            return Err(io::Error::other(format!(
                "GDI resources changed during 100 redraws: {gdi_before} -> {gdi_after}"
            )));
        }
        Ok(format!(
            "full_history_tracks={}\nfull_history_points_per_track={}\nfull_history_private_bytes={}\nfull_history_working_set_bytes={}\ncpu_ms_before_history_stress={}\ncpu_ms_including_history_stress={}\nfull_history_105_render_ms={}\ngdi_objects_before={}\ngdi_objects_after={}",
            full.tracks.len(),
            full.times.len(),
            own.private,
            own.working_set,
            cpu_before_stress,
            cpu_ms,
            full_render_ms,
            gdi_before,
            gdi_after
        ))
    }
    fn hit(&mut self, canvas: &Canvas<'_>, r: RECT, text: &str, selected: bool, action: Action) {
        canvas.button(r, text, selected);
        self.hits.push((r, action));
    }
    fn render(&mut self, dc: HDC) {
        // Font handles are independent from state and remain owned by self until after rendering.
        let fonts = self.fonts;
        let c = Canvas { dc, fonts: &fonts };
        self.paint(&c);
    }
    fn paint_comparison(&mut self, c: &Canvas<'_>, rx: i32, rw: i32) {
        let plot = self.comparison.plot(&self.history, self.commit, self.range);
        c.text(
            rx + 12,
            79,
            rw - 216,
            24,
            if self.commit {
                "Top 5 · Commit"
            } else {
                "Top 5 · RAM"
            },
            c.fonts.bold,
            INK,
            false,
        );
        let chart = rect(rx + 88, 113, rw - 104, 68);
        self.comparison_rect = chart;
        let xy = |at: u32, value: u64| POINT {
            x: chart.left
                + (at.saturating_sub(plot.start) as f64 / (plot.end - plot.start).max(1) as f64
                    * (chart.right - chart.left) as f64) as i32,
            y: chart.bottom
                - (value as f64 / plot.maximum as f64 * (chart.bottom - chart.top) as f64) as i32,
        };
        for i in 0..=2 {
            let y = chart.top + (chart.bottom - chart.top) * i / 2;
            c.line(
                &[POINT { x: chart.left, y }, POINT { x: chart.right, y }],
                BORDER,
                1,
            );
            c.text(
                rx + 2,
                y - 8,
                80,
                17,
                &bytes(plot.maximum * (2 - i) as u64 / 2),
                c.fonts.small,
                MUTED,
                true,
            );
        }
        for (index, series) in plot.series.iter().enumerate() {
            for segment in &series.segments {
                let points: Vec<_> = segment.iter().map(|(at, value)| xy(*at, *value)).collect();
                c.line(&points, COLORS[index], 2);
                if let Some(last) = points.last() {
                    c.circle(last.x, last.y, 2, COLORS[index]);
                }
            }
            let member = &self.comparison.members[index];
            let label = match &member.selection {
                Selection::Process(key) => format!("{} · PID {}", member.name, key.pid),
                Selection::App(image) => format!("{} · {}", member.name, image),
                Selection::Family(key) => format!("{} · family {}", member.name, key.pid),
                _ => member.name.clone(),
            };
            let y = 202 + index as i32 * 14;
            c.circle(rx + 16, y + 7, 3, COLORS[index]);
            c.text(rx + 26, y, rw - 132, 14, &label, c.fonts.small, INK, false);
            c.text(
                rx + rw - 102,
                y,
                90,
                14,
                &series
                    .current
                    .map(bytes)
                    .unwrap_or_else(|| "Unavailable".into()),
                c.fonts.small,
                MUTED,
                true,
            );
            self.hits
                .push((rect(rx + 10, y, rw - 20, 14), Action::Compare(index)));
        }
        c.text(
            chart.left,
            183,
            75,
            17,
            &clock(plot.start),
            c.fonts.small,
            MUTED,
            false,
        );
        c.text(
            chart.right - 75,
            183,
            75,
            17,
            &clock(plot.end),
            c.fonts.small,
            MUTED,
            true,
        );
        if plot.series.is_empty() {
            c.text(
                chart.left + 4,
                130,
                chart.right - chart.left - 8,
                24,
                "Waiting for accessible processes…",
                c.fonts.small,
                MUTED,
                false,
            );
        }
        c.text(
            rx + 12,
            272,
            rw - 24,
            14,
            &format!(
                "Pinned · Refresh · {} window",
                window_label(Some(self.range))
            ),
            c.fonts.small,
            MUTED,
            false,
        );
    }
    fn paint(&mut self, c: &Canvas<'_>) {
        let w = self.width;
        let h = self.height;
        let f = c.fonts;
        let table_right = w - 398;
        let rx = table_right + 12;
        let rw = w - rx - 16;
        self.hits.clear();
        self.pie.clear();
        self.comparison_rect = rect(0, 0, 0, 0);
        c.fill(rect(0, 0, w, h), BG);
        c.text(18, 7, 145, 30, "Mem Map", f.title, INK, false);
        let used = self.history.total.saturating_sub(self.history.available);
        c.text(
            177,
            10,
            w - 640,
            25,
            &format!(
                "RAM {} / {} ({:.0}%) · Free {}",
                bytes(used),
                bytes(self.history.total),
                used as f64 * 100.0 / self.history.total.max(1) as f64,
                bytes(self.history.available)
            ),
            f.small,
            INK,
            false,
        );
        c.text(
            19,
            39,
            w - 425,
            20,
            &format!(
                "Self: {} private / {} working set  ·  {} processes observed  ·  {} unavailable",
                bytes(self.own_ram),
                bytes(self.own_ws),
                self.history
                    .tracks
                    .values()
                    .filter(|t| t.current.is_some())
                    .count(),
                self.history.inaccessible
            ),
            f.small,
            MUTED,
            false,
        );
        let paused = self.pause.load(Ordering::Relaxed);
        c.text(w - 451, 14, 30, 25, "UI", f.small, MUTED, false);
        for (x, width, label, direction) in [
            (w - 419, 30, "−".to_owned(), -1),
            (w - 385, 65, format!("{}%", self.ui_zoom), 0),
            (w - 316, 30, "+".to_owned(), 1),
        ] {
            self.hit(
                c,
                rect(x, 14, width, 25),
                &label,
                false,
                Action::UiZoom(direction),
            );
        }
        c.text(
            w - 385,
            44,
            112,
            22,
            "Growth over",
            f.small,
            if self.growth_error.is_some() {
                RED
            } else {
                MUTED
            },
            false,
        );
        c.panel(rect(w - 265, 44, 150, 23));
        c.text(
            w - 259,
            44,
            138,
            23,
            &window_label(self.growth_window),
            f.small,
            INK,
            false,
        );
        self.hit(
            c,
            rect(w - 229, 14, 92, 30),
            if paused { "Resume" } else { "Pause" },
            paused,
            Action::Pause,
        );
        self.hit(
            c,
            rect(w - 127, 14, 110, 30),
            "Export CSV",
            true,
            Action::Export,
        );
        c.text(18, 72, 43, 24, "Group", f.small, MUTED, false);
        for (i, (mode, label)) in [
            (Grouping::Name, "Name"),
            (Grouping::Executable, "Path"),
            (Grouping::Family, "Family"),
            (Grouping::Process, "PID"),
        ]
        .iter()
        .enumerate()
        {
            self.hit(
                c,
                rect(65 + i as i32 * 68, 70, 63, 28),
                label,
                self.scope.is_none() && self.grouping == *mode,
                Action::Group(*mode),
            );
        }
        self.hit(
            c,
            rect(341, 70, 89, 28),
            if self.growth { "Growth ↓" } else { "RAM ↓" },
            self.growth,
            Action::Growth,
        );
        self.hit(
            c,
            rect(436, 70, 96, 28),
            "Inactive",
            self.inactive,
            Action::Inactive,
        );
        c.panel(rect(540, 71, (w - 948).max(120), 25));
        c.text(
            545,
            72,
            (w - 958).max(110),
            22,
            if self.query.is_empty() {
                "Search name / parent / path / PID"
            } else {
                &self.query
            },
            f.small,
            MUTED,
            false,
        );
        c.panel(rect(16, 110, table_right - 16, h - 146));
        c.fill(rect(17, 111, table_right - 18, 29), rgb(234, 239, 242));
        c.text(
            28,
            113,
            table_right - 545,
            25,
            "NAME / EXECUTABLE PATH",
            f.small,
            MUTED,
            false,
        );
        c.text(
            table_right - 505,
            113,
            60,
            25,
            "PID / #",
            f.small,
            MUTED,
            true,
        );
        c.text(
            table_right - 435,
            113,
            100,
            25,
            "PRIVATE RAM",
            f.small,
            MUTED,
            true,
        );
        c.text(
            table_right - 325,
            113,
            100,
            25,
            "PEAK RAM",
            f.small,
            MUTED,
            true,
        );
        c.text(
            table_right - 215,
            113,
            100,
            25,
            "COMMIT",
            f.small,
            MUTED,
            true,
        );
        c.text(
            table_right - 105,
            113,
            92,
            25,
            &format!("Δ RAM {}", window_label(self.growth_window)),
            f.small,
            MUTED,
            true,
        );
        let end = (self.scroll + self.visible_rows()).min(self.rows.len());
        for index in self.scroll..end {
            let row = &self.rows[index];
            let y = 142 + (index - self.scroll) as i32 * 32;
            if self.selection.as_ref() == Some(&row.selection) {
                c.fill(rect(17, y, table_right - 18, 31), rgb(224, 241, 235));
                c.fill(rect(17, y, 3, 31), ACCENT);
            }
            let name = if matches!(row.selection, Selection::Process(_)) {
                row.name.clone()
            } else {
                format!("› {}", row.name)
            };
            c.text(
                28,
                y,
                table_right - 545,
                18,
                &name,
                f.bold,
                if row.active { INK } else { MUTED },
                false,
            );
            let path = if let Selection::Process(key) = &row.selection {
                self.history
                    .tracks
                    .get(key)
                    .map(|track| format!("Parent: {} · {}", track.parent_description(), row.detail))
                    .unwrap_or_else(|| row.detail.clone())
            } else if matches!(row.selection, Selection::Name(_)) {
                let paths: std::collections::BTreeSet<_> = self
                    .history
                    .tracks
                    .values()
                    .filter(|t| self.history.matches(t, &row.selection))
                    .map(|t| t.image.as_str())
                    .collect();
                if paths.len() > 1 {
                    format!("{} paths · expand to inspect", paths.len())
                } else {
                    row.detail.clone()
                }
            } else {
                row.detail.clone()
            };
            c.text(
                28,
                y + 17,
                table_right - 545,
                13,
                &path,
                f.small,
                if matches!(row.selection, Selection::Process(key) if self.history.parent_target(key).is_some()) { ACCENT } else { MUTED },
                false,
            );
            let pid = if let Selection::Process(key) = &row.selection {
                key.pid.to_string()
            } else {
                row.count.to_string()
            };
            c.text(table_right - 505, y, 60, 30, &pid, f.small, MUTED, true);
            c.text(
                table_right - 435,
                y,
                100,
                30,
                &if row.active {
                    bytes(row.ram)
                } else {
                    "Not observed".into()
                },
                f.bold,
                INK,
                true,
            );
            c.text(
                table_right - 325,
                y,
                100,
                30,
                &bytes(row.peak),
                f.normal,
                INK,
                true,
            );
            c.text(
                table_right - 215,
                y,
                100,
                30,
                &if row.active {
                    bytes(row.commit)
                } else {
                    "—".into()
                },
                f.normal,
                MUTED,
                true,
            );
            c.text(
                table_right - 105,
                y,
                92,
                30,
                &if row.active {
                    delta(row.growth)
                } else {
                    "—".into()
                },
                f.small,
                if row.growth > 8 * 1024 * 1024 {
                    RED
                } else {
                    MUTED
                },
                true,
            );
            c.fill(rect(27, y + 31, table_right - 40, 1), rgb(237, 241, 243));
            self.hits
                .push((rect(17, y, table_right - 18, 32), Action::Select(index)));
            if let Selection::Process(child) = row.selection
                && self.history.parent_target(child).is_some()
            {
                self.hits.push((
                    rect(28, y + 17, table_right - 545, 13),
                    Action::Parent(child),
                ));
            }
        }
        if self.rows.is_empty() {
            c.text(
                28,
                158,
                table_right - 46,
                35,
                if self.history.total == 0 {
                    "Reading memory counters…"
                } else {
                    "No matches. Clear the search or include inactive."
                },
                f.normal,
                MUTED,
                false,
            );
        }
        if self.rows.len() > self.visible_rows() {
            let track = self.visible_rows() as i32 * 32;
            let thumb = (track * self.visible_rows() as i32 / self.rows.len() as i32).max(15);
            let top = (track - thumb) * self.scroll as i32
                / (self.rows.len() - self.visible_rows()).max(1) as i32;
            c.fill(rect(table_right - 6, 142, 3, track), BORDER);
            c.fill(rect(table_right - 6, 142 + top, 3, thumb), MUTED);
        }
        c.panel(rect(rx, 70, rw, 218));
        self.hit(
            c,
            rect(rx + rw - 196, 79, 48, 24),
            "Pie",
            !self.overview,
            Action::Overview(false),
        );
        self.hit(
            c,
            rect(rx + rw - 144, 79, 52, 24),
            "Top 5",
            self.overview,
            Action::Overview(true),
        );
        if self.overview {
            self.hit(
                c,
                rect(rx + rw - 88, 79, 76, 24),
                "Refresh",
                false,
                Action::RefreshComparison,
            );
            self.paint_comparison(c, rx, rw);
        } else {
            c.text(
                rx + 12,
                79,
                rw - 210,
                24,
                "Physical RAM",
                f.bold,
                INK,
                false,
            );
            let apps = self.history.rows(self.grouping, false, "", false);
            let mut values: Vec<_> = apps
                .iter()
                .take(4)
                .map(|r| (r.name.clone(), r.ram))
                .collect();
            let rest = apps.iter().skip(4).map(|r| r.ram).sum();
            if rest > 0 {
                values.push(("Other apps".into(), rest));
            }
            let slices = partition(self.history.total, self.history.available, &values);
            let cx = rx + 76;
            let cy = 177;
            let radius = 59;
            self.pie_circle = (cx, cy, radius);
            let mut angle = 0.0;
            for (index, slice) in slices.iter().enumerate() {
                let arc = TAU * slice.bytes as f64 / self.history.total.max(1) as f64;
                let target = if index < apps.len().min(4) {
                    Some(apps[index].selection.clone())
                } else {
                    None
                };
                self.pie.push((angle, angle + arc, target.clone()));
                let steps = (arc * 36.0).ceil().max(1.0) as usize;
                let color = if slice.label == "Available" {
                    COLORS[7]
                } else if slice.label == "Other in use" {
                    COLORS[6]
                } else {
                    COLORS[index.min(5)]
                };
                if slice.bytes > 0 {
                    let mut polygon = vec![POINT { x: cx, y: cy }];
                    for step in 0..=steps {
                        let a = angle + arc * step as f64 / steps as f64 - TAU / 4.0;
                        polygon.push(POINT {
                            x: cx + (radius as f64 * a.cos()) as i32,
                            y: cy + (radius as f64 * a.sin()) as i32,
                        });
                    }
                    c.polygon(&polygon, color);
                }
                angle += arc;
                let y = 109 + index as i32 * 22;
                c.circle(rx + 151, y + 10, 3, color);
                c.text(rx + 162, y, rw - 241, 20, &slice.label, f.small, INK, false);
                c.text(
                    rx + rw - 79,
                    y,
                    67,
                    20,
                    &bytes(slice.bytes),
                    f.small,
                    MUTED,
                    true,
                );
                if target.is_some() {
                    self.hits
                        .push((rect(rx + 144, y, rw - 154, 22), Action::Pie(index)));
                }
            }
            c.circle(cx, cy, 36, rgb(255, 255, 255));
            c.text(
                cx - 33,
                cy - 16,
                66,
                31,
                &format!(
                    "{:.0}%",
                    used as f64 * 100.0 / self.history.total.max(1) as f64
                ),
                f.title,
                INK,
                false,
            );
            c.text(
                rx + 12,
                265,
                rw - 24,
                17,
                "Other in use: system, shared & unmeasured.",
                f.small,
                MUTED,
                false,
            );
        }
        c.panel(rect(rx, 300, rw, h - 336));
        let selected = self.selection.as_ref().and_then(|s| {
            self.history
                .rows_with_window(
                    match s {
                        Selection::Name(_) => Grouping::Name,
                        Selection::App(_) => Grouping::Executable,
                        Selection::Family(_) => Grouping::Family,
                        Selection::Process(_) => Grouping::Process,
                    },
                    true,
                    "",
                    false,
                    self.growth_window,
                )
                .into_iter()
                .find(|r| &r.selection == s)
        });
        let title = selected
            .as_ref()
            .map(|r| r.name.as_str())
            .unwrap_or("Select an app or process");
        c.text(rx + 12, 309, rw - 24, 25, title, f.bold, INK, false);
        let members: Vec<_> = self
            .selection
            .as_ref()
            .map(|s| {
                self.history
                    .tracks
                    .values()
                    .filter(|t| self.history.matches(t, s))
                    .collect()
            })
            .unwrap_or_default();
        let observed = members.iter().filter(|t| t.current.is_some()).count();
        let paths: std::collections::BTreeSet<_> =
            members.iter().map(|t| t.image.as_str()).collect();
        c.text(
            rx + 12,
            336,
            rw - 150,
            24,
            &format!("{observed} processes · {} paths", paths.len()),
            f.small,
            MUTED,
            false,
        );
        if self.scope.is_some() {
            let r = rect(rx + rw - 132, 335, 120, 25);
            c.button(r, "← All groups", false);
            self.hits.push((r, Action::Back));
        } else if selected
            .as_ref()
            .is_some_and(|r| !matches!(r.selection, Selection::Process(_)))
        {
            let r = rect(rx + rw - 132, 335, 120, 25);
            c.button(r, "Expand ↗", false);
            self.hits.push((r, Action::Drill));
        }
        let path = paths.iter().copied().collect::<Vec<_>>().join(" | ");
        let single = if members.len() == 1 {
            Some(members[0])
        } else {
            None
        };
        c.paragraph(
            rect(
                rx + 12,
                365,
                rw - 24,
                if single.is_some() { 32 } else { 46 },
            ),
            &path,
            MUTED,
        );
        if let Some(track) = single {
            let navigable = self.history.parent_target(track.key).is_some();
            c.text(
                rx + 12,
                399,
                rw - 24,
                16,
                &format!("Parent: {}", track.parent_description()),
                f.small,
                if navigable { ACCENT } else { MUTED },
                false,
            );
            if navigable {
                self.hits
                    .push((rect(rx + 12, 399, rw - 24, 16), Action::Parent(track.key)));
            }
        }
        let points = self
            .selection
            .as_ref()
            .map(|s| self.history.graph(s, self.commit, self.range))
            .unwrap_or_default();
        let peak = points.iter().filter_map(|p| p.1).max().unwrap_or(0);
        let measured = selected.as_ref().is_some_and(|r| r.active);
        let ram = selected.as_ref().map(|r| r.ram).unwrap_or(0);
        let commit = selected.as_ref().map(|r| r.commit).unwrap_or(0);
        let growth = selected.as_ref().map(|r| r.growth).unwrap_or(0);
        let growth_label = format!("RAM growth · {}", window_label(self.growth_window));
        let fields = [
            (
                "Private RAM",
                if measured {
                    bytes(ram)
                } else {
                    "Not observed".into()
                },
            ),
            (
                "Private commit",
                if measured {
                    bytes(commit)
                } else {
                    "Not observed".into()
                },
            ),
            (
                growth_label.as_str(),
                if measured {
                    delta(growth)
                } else {
                    "—".into()
                },
            ),
            (
                if single.is_some() {
                    "Working set"
                } else {
                    "Executable paths"
                },
                single
                    .map(|t| {
                        if t.current.is_some() {
                            bytes(t.working_set)
                        } else {
                            "Not observed".into()
                        }
                    })
                    .unwrap_or(paths.len().to_string()),
            ),
            (
                "Peak RAM (30m)",
                bytes(selected.as_ref().map(|r| r.peak).unwrap_or(0)),
            ),
        ];
        for (i, (label, value)) in fields.iter().enumerate() {
            let y = 417 + i as i32 * 21;
            c.text(rx + 12, y, 145, 20, label, f.small, MUTED, false);
            c.text(rx + 157, y, rw - 169, 20, value, f.bold, INK, true);
        }
        if let Some(t) = single {
            c.text(
                rx + 12,
                523,
                rw - 24,
                19,
                &format!(
                    "PID {} · Parent PID {} · {} samples",
                    t.key.pid,
                    t.parent_pid,
                    t.points.len()
                ),
                f.small,
                MUTED,
                false,
            );
        }
        self.hit(
            c,
            rect(rx + 12, 547, 81, 24),
            if self.commit { "Commit" } else { "RAM" },
            self.commit,
            Action::Metric,
        );
        self.hit(
            c,
            rect(rx + 98, 547, 33, 24),
            "−",
            false,
            Action::TimeZoom(false),
        );
        self.hit(
            c,
            rect(rx + 135, 547, 33, 24),
            "+",
            false,
            Action::TimeZoom(true),
        );
        c.text(
            rx + 172,
            547,
            38,
            24,
            &window_label(Some(self.range)),
            f.small,
            MUTED,
            false,
        );
        for (i, (secs, label)) in [(300, "5m"), (900, "15m"), (1800, "30m")]
            .iter()
            .enumerate()
        {
            self.hit(
                c,
                rect(rx + rw - 156 + i as i32 * 48, 547, 43, 24),
                label,
                self.range == *secs,
                Action::Range(*secs),
            );
        }
        let chart = rect(rx + 88, 592, rw - 104, h - 668);
        self.graph_rect = chart;
        let max = (peak as f64 * 1.1).max(10.0 * 1024.0 * 1024.0) as u64;
        let end_time = self.history.at.max(1);
        let start = end_time.saturating_sub(self.range);
        let xy = |at: u32, value: u64| POINT {
            x: chart.left
                + (at.saturating_sub(start) as f64 / (end_time - start).max(1) as f64
                    * (chart.right - chart.left) as f64) as i32,
            y: chart.bottom
                - (value as f64 / max as f64 * (chart.bottom - chart.top) as f64) as i32,
        };
        for i in 0..=3 {
            let y = chart.top + (chart.bottom - chart.top) * i / 3;
            c.line(
                &[POINT { x: chart.left, y }, POINT { x: chart.right, y }],
                BORDER,
                1,
            );
            c.text(
                rx + 2,
                y - 8,
                80,
                17,
                &bytes(max * (3 - i) as u64 / 3),
                f.small,
                MUTED,
                true,
            );
        }
        let mut segment = Vec::new();
        let mut prior = None;
        for (at, value) in &points {
            if value.is_none() || prior.is_some_and(|prev| at.saturating_sub(prev) > 3) {
                c.line(&segment, ACCENT, 2);
                segment.clear();
            }
            if let Some(value) = value {
                segment.push(xy(*at, *value));
            }
            prior = Some(*at);
        }
        c.line(&segment, ACCENT, 2);
        if let Some(last) = segment.last() {
            c.circle(last.x, last.y, 2, ACCENT);
        }
        if points.iter().filter(|p| p.1.is_some()).count() < 2 {
            c.text(
                chart.left + 4,
                chart.top + 15,
                chart.right - chart.left - 8,
                25,
                "Collecting history…",
                f.small,
                MUTED,
                false,
            );
        }
        if let Some((x, y)) = self.hover.filter(|(x, y)| {
            *x >= chart.left && *x <= chart.right && *y >= chart.top && *y <= chart.bottom
        }) {
            let _ = y;
            if let Some((at, Some(value))) =
                points.iter().min_by_key(|(at, _)| (xy(*at, 0).x - x).abs())
            {
                let p = xy(*at, *value);
                c.line(
                    &[
                        POINT {
                            x: p.x,
                            y: chart.top,
                        },
                        POINT {
                            x: p.x,
                            y: chart.bottom,
                        },
                    ],
                    COLORS[5],
                    1,
                );
                c.text(
                    chart.left + 4,
                    chart.top + 2,
                    chart.right - chart.left - 8,
                    20,
                    &format!("{} · {}", clock(*at), bytes(*value)),
                    f.small,
                    INK,
                    false,
                );
            }
        }
        c.text(
            chart.left,
            chart.bottom + 4,
            80,
            18,
            &clock(start),
            f.small,
            MUTED,
            false,
        );
        c.text(
            chart.right - 90,
            chart.bottom + 4,
            90,
            18,
            &clock(end_time),
            f.small,
            MUTED,
            true,
        );
        c.text(
            rx + 12,
            h - 53,
            rw - 24,
            16,
            "Wheel: time zoom · Ctrl+wheel: UI zoom",
            f.small,
            MUTED,
            false,
        );
        let status = if let Some(error) = &self.error {
            format!("Sampling failed: {error}")
        } else if let Some(error) = &self.growth_error {
            format!("Growth window: {error}")
        } else if !self.notice.is_empty() {
            self.notice.clone()
        } else {
            format!(
                "{}–{} / {} rows · {} snapshots · {} tracks omitted · {}",
                self.scroll + usize::from(!self.rows.is_empty()),
                end,
                self.rows.len(),
                self.history.times.len(),
                self.history.omitted,
                if paused { "PAUSED" } else { "Live · 2 sec" }
            )
        };
        c.text(
            18,
            h - 27,
            w - 36,
            22,
            &status,
            f.small,
            if self.error.is_some() || self.growth_error.is_some() {
                RED
            } else {
                MUTED
            },
            false,
        );
    }
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
    unsafe fn screenshot(&mut self, path: &std::path::Path) -> io::Result<()> {
        unsafe { self.screenshot_view(path, Viewport::new(1260, 890, 96, 100, 0, 0)) }
    }
    unsafe fn screenshot_view(&mut self, path: &std::path::Path, v: Viewport) -> io::Result<()> {
        unsafe {
            let width = 1260;
            let height = 890;
            let dc = GetDC(null_mut());
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
                ReleaseDC(null_mut(), dc);
                return Err(io::Error::last_os_error());
            }
            let old = SelectObject(mem, bmp);
            let old_size = (self.width, self.height);
            self.width = v.width;
            self.height = v.height;
            let saved = SaveDC(mem);
            SetMapMode(mem, MM_ANISOTROPIC);
            SetWindowExtEx(mem, 96, 96, null_mut());
            SetViewportExtEx(mem, v.scale as i32, v.scale as i32, null_mut());
            let (x, y) = v.physical(0, 0);
            SetViewportOrgEx(mem, x, y, null_mut());
            self.render(mem);
            RestoreDC(mem, saved);
            GdiFlush();
            let result = (|| {
                let mut file = BufWriter::new(File::create(path)?);
                let size = (width * height * 4) as u32;
                file.write_all(b"BM")?;
                file.write_all(&(size + 54).to_le_bytes())?;
                file.write_all(&[0; 4])?;
                file.write_all(&54u32.to_le_bytes())?;
                file.write_all(&40u32.to_le_bytes())?;
                file.write_all(&width.to_le_bytes())?;
                file.write_all(&(-height).to_le_bytes())?;
                file.write_all(&1u16.to_le_bytes())?;
                file.write_all(&32u16.to_le_bytes())?;
                file.write_all(&[0; 24])?;
                file.write_all(std::slice::from_raw_parts(data.cast::<u8>(), size as usize))?;
                file.flush()
            })();
            (self.width, self.height) = old_size;
            SelectObject(mem, old);
            DeleteObject(bmp);
            DeleteDC(mem);
            ReleaseDC(null_mut(), dc);
            result
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
        unsafe {
            self.free_buffer();
        }
        unsafe {
            for font in [
                self.fonts.normal,
                self.fonts.small,
                self.fonts.bold,
                self.fonts.title,
                self.fonts.number,
                self.search_font,
            ] {
                DeleteObject(font);
            }
        }
    }
}

// A modal dialog runs a nested message loop. Hold no Rust borrow of App while it
// is open: timer/paint callbacks must remain free to borrow the window state.
unsafe fn export_dialog(hwnd: HWND, ptr: *mut App) {
    unsafe {
        let mut path = vec![0u16; 32768];
        let default = wide("mem-map-history.csv");
        path[..default.len()].copy_from_slice(&default);
        let filter = wide("CSV history (*.csv)\0*.csv\0");
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
            let result = (*ptr).export_to(&path);
            match result {
                Ok(()) => (*ptr).notice = format!("Saved {}", path.display()),
                Err(e) => {
                    MessageBoxW(
                        hwnd,
                        wide(&e.to_string()).as_ptr(),
                        wide("Export failed").as_ptr(),
                        MB_ICONERROR,
                    );
                }
            }
            InvalidateRect(hwnd, null(), 0);
        } else if CommDlgExtendedError() != 0 {
            (*ptr).notice = "Could not open the save dialog".into();
            InvalidateRect(hwnd, null(), 0);
        }
    }
}

// Layout APIs synchronously dispatch window messages. Use the owning raw pointer,
// with no App borrow held across those calls, and suppress nested layout.
unsafe fn layout_window(hwnd: HWND, ptr: *mut App) {
    unsafe {
        if (*ptr).layout_busy {
            return;
        }
        (*ptr).layout_busy = true;
        for _ in 0..3 {
            let mut r: RECT = zeroed();
            GetClientRect(hwnd, &mut r);
            let v = Viewport::new(
                r.right,
                r.bottom,
                (*ptr).dpi,
                (*ptr).ui_zoom,
                (*ptr).viewport.x,
                (*ptr).viewport.y,
            );
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
        if (*ptr).font_scale != v.scale || (*ptr).search_font.is_null() {
            let old = (*ptr).search_font;
            (*ptr).search_font = search_font(v.scale);
            (*ptr).font_scale = v.scale;
            SendMessageW((*ptr).search, WM_SETFONT, (*ptr).search_font as usize, 1);
            SendMessageW(
                (*ptr).growth_input,
                WM_SETFONT,
                (*ptr).search_font as usize,
                1,
            );
            DeleteObject(old);
        }
        for (control, x, y, w, h) in [
            ((*ptr).search, 540, 71, (v.width - 948).max(120), 25),
            ((*ptr).growth_input, v.width - 265, 44, 150, 180),
        ] {
            let (x, y) = v.physical(x, y);
            MoveWindow(
                control,
                x,
                y,
                w * v.scale as i32 / 96,
                h * v.scale as i32 / 96,
                1,
            );
        }
        (*ptr).refresh_rows();
        (*ptr).layout_busy = false;
        InvalidateRect(hwnd, null(), 0);
    }
}

// In-process native contract exercise, outside an App borrow: drive the actual
// window procedure and child controls. This is not external mouse/keyboard QA.
unsafe fn smoke_native(hwnd: HWND, ptr: *mut App) {
    unsafe {
        if (*ptr).smoke.is_none() || (*ptr).smoke_step != 1 || (*ptr).smoke_native_done {
            return;
        }
        (*ptr).smoke_native_done = true;
        let dir = (*ptr).smoke.clone().unwrap();
        let dc = GetDC(hwnd);
        let mut monospaced = !dc.is_null();
        if !dc.is_null() {
            for font in [
                (*ptr).fonts.normal,
                (*ptr).fonts.small,
                (*ptr).fonts.bold,
                (*ptr).fonts.title,
                (*ptr).fonts.number,
                (*ptr).search_font,
            ] {
                let old = SelectObject(dc, font);
                let mut narrow: SIZE = zeroed();
                let mut broad: SIZE = zeroed();
                monospaced &= !font.is_null()
                    && GetTextExtentPoint32W(dc, wide("iiii").as_ptr(), 4, &mut narrow) != 0
                    && GetTextExtentPoint32W(dc, wide("WWWW").as_ptr(), 4, &mut broad) != 0
                    && narrow.cx > 0
                    && narrow.cx == broad.cx;
                SelectObject(dc, old);
            }
        }
        (*ptr).smoke_assert(monospaced, "actual equal-width display and edit fonts");
        let original = (
            (*ptr).grouping,
            (*ptr).scope.clone(),
            (*ptr).selection.clone(),
            (*ptr).inactive,
            (*ptr).overview,
        );
        (*ptr).action(hwnd, Action::Group(Grouping::Process));
        let candidate = (*ptr).history.tracks.keys().find_map(|child| {
            (*ptr)
                .history
                .parent_target(*child)
                .map(|parent| (*child, parent))
        });
        if let Some((child, parent)) = candidate {
            (*ptr).selection = Some(Selection::Process(child));
            let index = (*ptr)
                .rows
                .iter()
                .position(|r| r.selection == Selection::Process(child))
                .unwrap();
            (*ptr).scroll = index.min((*ptr).rows.len().saturating_sub((*ptr).visible_rows()));
            if (*ptr).ensure_buffer(dc, 1260, 890) {
                (*ptr).render((*ptr).buffer);
            }
            let link = (*ptr).hits.iter().rev().find_map(|(r, a)| match a {
                Action::Parent(k) if *k == child => Some(*r),
                _ => None,
            });
            (*ptr).smoke_assert(link.is_some(), "parent is a rendered clickable control");
            if let Some(r) = link {
                // Force conflicting filters after hit construction; navigation must
                // reveal the exact captured parent and scroll its row into view.
                SetWindowTextW((*ptr).search, wide("no process can match this").as_ptr());
                (*ptr).scope = Some(Selection::Name("no scope".into()));
                (*ptr).inactive = false;
                let (x, y) = (*ptr)
                    .viewport
                    .physical((r.left + r.right) / 2, (r.top + r.bottom) / 2);
                SendMessageW(
                    hwnd,
                    WM_LBUTTONDOWN,
                    0,
                    ((y as u32 & 0xffff) << 16 | (x as u32 & 0xffff)) as isize,
                );
                SendMessageW(hwnd, WM_APP + 3, 0, 0);
                (*ptr).smoke_assert(
                    (*ptr).selection == Some(Selection::Process(parent))
                        && (*ptr).grouping == Grouping::Process
                        && String::is_empty(&(*ptr).query)
                        && (*ptr).scope.is_none()
                        && (*ptr).inactive,
                    "native parent click reveals exact instance through filters",
                );
                let visible = (*ptr)
                    .rows
                    .iter()
                    .position(|r| r.selection == Selection::Process(parent))
                    .is_some_and(|i| {
                        i >= (*ptr).scroll && i < (*ptr).scroll + (*ptr).visible_rows()
                    });
                (*ptr).smoke_assert(visible, "parent row scrolled into view");
                let result = (*ptr).screenshot(&dir.join("parent-navigation.bmp"));
                (*ptr).smoke_assert(result.is_ok(), "parent navigation screenshot");
            }
        } else {
            (*ptr).smoke_assert(
                false,
                "native parent navigation requires an observed parent",
            );
        }
        let points = (*ptr).history.times.len();
        let growth = (*ptr).growth_window;
        let pinned: Vec<_> = (*ptr)
            .comparison
            .members
            .iter()
            .map(|m| m.selection.clone())
            .collect();
        (*ptr).action(hwnd, Action::Range(1800));
        (*ptr).render((*ptr).buffer);
        let chart = (*ptr).graph_rect;
        let (x, y) = (*ptr).viewport.physical(chart.left + 10, chart.top + 10);
        let mut pt = POINT { x, y };
        ClientToScreen(hwnd, &mut pt);
        let wheel_point = ((pt.y as u32 & 0xffff) << 16 | (pt.x as u32 & 0xffff)) as isize;
        SendMessageW(hwnd, WM_MOUSEWHEEL, 120 << 16, wheel_point);
        (*ptr).smoke_assert((*ptr).range == 900, "native chart wheel zoom");
        SendMessageW(hwnd, WM_MOUSEWHEEL, (120 << 16) | 8, wheel_point);
        SendMessageW(hwnd, WM_APP + 2, 0, 0);
        (*ptr).smoke_assert(
            (*ptr).ui_zoom == 125 && (*ptr).viewport.scale == (*ptr).dpi.max(96) * 125 / 100,
            "native Ctrl wheel UI zoom",
        );
        for zoom in [75, 125, 200, 100] {
            (*ptr).ui_zoom = zoom;
            layout_window(hwnd, ptr);
            if zoom == 200 {
                SendMessageW(hwnd, WM_HSCROLL, SB_BOTTOM as usize, 0);
                SendMessageW(hwnd, WM_VSCROLL, SB_BOTTOM as usize, 0);
                (*ptr).smoke_assert(
                    (*ptr).viewport.x > 0 && (*ptr).viewport.y > 0,
                    "enlarged interface overflow scrollbars",
                );
            }
            let v = (*ptr).viewport;
            let mut client: RECT = zeroed();
            GetClientRect(hwnd, &mut client);
            let expected = Viewport::new(client.right, client.bottom, (*ptr).dpi, zoom, v.x, v.y);
            (*ptr).smoke_assert(
                v.page_width == expected.page_width && v.page_height == expected.page_height,
                "viewport matches final client bounds",
            );
            let mut control: RECT = zeroed();
            GetWindowRect((*ptr).search, &mut control);
            let mut point = POINT {
                x: control.left,
                y: control.top,
            };
            ScreenToClient(hwnd, &mut point);
            let (x, y) = v.physical(540, 71);
            (*ptr).smoke_assert(
                point.x == x
                    && point.y == y
                    && control.right - control.left
                        == (v.width - 948).max(120) * v.scale as i32 / 96,
                "native edit matches scaled and scrolled canvas",
            );
            let view = Viewport::new(1260, 890, 96, zoom, v.x, v.y);
            let result = (*ptr).screenshot_view(&dir.join(format!("zoom-{zoom}.bmp")), view);
            (*ptr).smoke_assert(result.is_ok(), "scaled renderer screenshot");
        }
        let original_dpi = (*ptr).dpi;
        let mut window: RECT = zeroed();
        GetWindowRect(hwnd, &mut window);
        for dpi in [144, 192, original_dpi] {
            SendMessageW(
                hwnd,
                WM_DPICHANGED,
                ((dpi << 16) | dpi) as usize,
                (&window as *const RECT) as isize,
            );
            let v = (*ptr).viewport;
            let mut control: RECT = zeroed();
            GetWindowRect((*ptr).growth_input, &mut control);
            let mut point = POINT {
                x: control.left,
                y: control.top,
            };
            ScreenToClient(hwnd, &mut point);
            let expected = v.physical(v.width - 265, 44);
            (*ptr).smoke_assert(
                v.scale == dpi
                    && point.x == expected.0
                    && point.y == expected.1
                    && control.right - control.left == 150 * dpi as i32 / 96,
                "DPI message rescales native controls with canvas",
            );
        }
        (*ptr).smoke_assert(
            (*ptr).history.times.len() == points
                && (*ptr).growth_window == growth
                && (*ptr)
                    .comparison
                    .members
                    .iter()
                    .map(|m| &m.selection)
                    .eq(pinned.iter()),
            "zoom preserves observations, growth and pinned identities",
        );
        (
            (*ptr).grouping,
            (*ptr).scope,
            (*ptr).selection,
            (*ptr).inactive,
            (*ptr).overview,
        ) = original;
        (*ptr).refresh_rows();
        (*ptr).render((*ptr).buffer);
        ReleaseDC(hwnd, dc);
    }
}

// SAFETY: Win32 invokes this on the creating thread. App lives in a Box from before
// CreateWindowEx until after the message loop, and the pointer is installed at WM_NCCREATE.
unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let create = &*(lp as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wp, lp);
        }
        match msg {
            m if m == WM_APP + 1 => {
                export_dialog(hwnd, ptr);
                0
            }
            m if m == WM_APP + 2 => {
                layout_window(hwnd, ptr);
                0
            }
            m if m == WM_APP + 3 => {
                // EN_CHANGE resets list scrolling even when the controller has
                // already cleared its query. Preserve the revealed parent row.
                let scroll = (*ptr).scroll;
                SetWindowTextW((*ptr).search, wide("").as_ptr());
                (*ptr).scroll = scroll.min((*ptr).rows.len().saturating_sub((*ptr).visible_rows()));
                0
            }
            WM_CREATE => {
                (*ptr).dpi = GetDpiForWindow(hwnd);
                let search = CreateWindowExW(
                    WS_EX_CLIENTEDGE,
                    wide("EDIT").as_ptr(),
                    wide("").as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                    0,
                    0,
                    1,
                    1,
                    hwnd,
                    100usize as HMENU,
                    GetModuleHandleW(null()),
                    null(),
                );
                (*ptr).search = search;
                (*ptr).search_font = search_font((*ptr).dpi);
                SendMessageW(search, WM_SETFONT, (*ptr).search_font as usize, 1);
                SendMessageW(search, EM_SETLIMITTEXT, 256, 0);
                SendMessageW(
                    search,
                    EM_SETCUEBANNER,
                    1,
                    wide("Search name / parent / path / PID").as_ptr() as isize,
                );
                let growth = CreateWindowExW(
                    0,
                    wide("COMBOBOX").as_ptr(),
                    wide("").as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWN as u32,
                    0,
                    0,
                    1,
                    180,
                    hwnd,
                    101usize as HMENU,
                    GetModuleHandleW(null()),
                    null(),
                );
                (*ptr).growth_input = growth;
                SendMessageW(growth, WM_SETFONT, (*ptr).search_font as usize, 1);
                for label in ["1 min", "5 min", "15 min", "30 min", "Session"] {
                    SendMessageW(growth, CB_ADDSTRING, 0, wide(label).as_ptr() as isize);
                }
                SendMessageW(growth, CB_SETCURSEL, 1, 0);
                SendMessageW(growth, CB_LIMITTEXT, 32, 0);
                SetTimer(hwnd, 1, 250, None);
                layout_window(hwnd, ptr);
                0
            }
            WM_GETMINMAXINFO => {
                let info = &mut *(lp as *mut MINMAXINFO);
                info.ptMinTrackSize = POINT { x: 640, y: 480 };
                0
            }
            WM_SIZE => {
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
                let mut info: SCROLLINFO = zeroed();
                info.cbSize = size_of::<SCROLLINFO>() as u32;
                info.fMask = SIF_ALL;
                if GetScrollInfo(hwnd, bar, &mut info) != 0 {
                    let next = match (wp & 0xffff) as i32 {
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
                        (*ptr).viewport.x = next;
                    } else {
                        (*ptr).viewport.y = next;
                    }
                    layout_window(hwnd, ptr);
                }
                0
            }
            WM_COMMAND => {
                if wp & 0xffff == 100 && (wp >> 16) & 0xffff == EN_CHANGE as usize {
                    let mut text = [0u16; 257];
                    let len = GetWindowTextW((*ptr).search, text.as_mut_ptr(), 257);
                    let app = &mut *ptr;
                    app.query = String::from_utf16_lossy(&text[..len as usize]);
                    app.scroll = 0;
                    app.refresh_rows();
                    InvalidateRect(hwnd, null(), 0);
                }
                if wp & 0xffff == 101
                    && [CBN_SELCHANGE as usize, CBN_EDITCHANGE as usize]
                        .contains(&((wp >> 16) & 0xffff))
                {
                    let mut text = [0u16; 64];
                    let len = if (wp >> 16) & 0xffff == CBN_SELCHANGE as usize {
                        let index = SendMessageW((*ptr).growth_input, CB_GETCURSEL, 0, 0);
                        if index < 0 {
                            0
                        } else {
                            SendMessageW(
                                (*ptr).growth_input,
                                CB_GETLBTEXT,
                                index as usize,
                                text.as_mut_ptr() as isize,
                            ) as i32
                        }
                    } else {
                        GetWindowTextW((*ptr).growth_input, text.as_mut_ptr(), 64)
                    };
                    if len >= 0 {
                        (*ptr).set_growth_window(&String::from_utf16_lossy(&text[..len as usize]));
                    }
                    InvalidateRect(hwnd, null(), 0);
                }
                0
            }
            WM_TIMER => {
                (*ptr).tick(hwnd);
                smoke_native(hwnd, ptr);
                0
            }
            WM_LBUTTONDOWN => {
                let (x, y) = (*ptr)
                    .viewport
                    .point(lp as i16 as i32, (lp >> 16) as i16 as i32);
                SetFocus(hwnd);
                let app = &mut *ptr;
                if let Some((_, action)) = app
                    .hits
                    .iter()
                    .rev()
                    .find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
                    .copied()
                {
                    app.action(hwnd, action);
                } else if let Some(index) = app.pie_hit(x, y) {
                    app.action(hwnd, Action::Pie(index));
                }
                0
            }
            WM_LBUTTONDBLCLK => {
                let (x, y) = (*ptr)
                    .viewport
                    .point(lp as i16 as i32, (lp >> 16) as i16 as i32);
                let app = &mut *ptr;
                if let Some((_, action)) = app
                    .hits
                    .iter()
                    .rev()
                    .find(|(r, _)| x >= r.left && x < r.right && y >= r.top && y < r.bottom)
                    .copied()
                {
                    if matches!(action, Action::Parent(_)) {
                        return 0;
                    }
                    app.action(
                        hwnd,
                        if matches!(action, Action::Select(_)) {
                            Action::Drill
                        } else {
                            action
                        },
                    );
                }
                0
            }
            WM_MOUSEWHEEL => {
                let mut point = POINT {
                    x: lp as i16 as i32,
                    y: (lp >> 16) as i16 as i32,
                };
                ScreenToClient(hwnd, &mut point);
                let app = &mut *ptr;
                let (x, y) = app.viewport.point(point.x, point.y);
                let mode = if wp & 0x0008 != 0 {
                    2
                } else if contains(app.graph_rect, x, y) || contains(app.comparison_rect, x, y) {
                    1
                } else {
                    0
                };
                if mode != app.wheel_mode {
                    app.wheel_delta = 0;
                    app.wheel_mode = mode;
                }
                app.wheel_delta += (wp >> 16) as i16 as i32;
                while app.wheel_delta.abs() >= 120 {
                    let step = app.wheel_delta.signum();
                    app.wheel_delta -= step * 120;
                    match mode {
                        2 => app.action(hwnd, Action::UiZoom(step)),
                        1 => app.action(hwnd, Action::TimeZoom(step > 0)),
                        _ if step > 0 => app.scroll = app.scroll.saturating_sub(3),
                        _ => {
                            app.scroll = (app.scroll + 3)
                                .min(app.rows.len().saturating_sub(app.visible_rows()))
                        }
                    }
                }
                InvalidateRect(hwnd, null(), 0);
                0
            }
            WM_MOUSEMOVE => {
                let app = &mut *ptr;
                let next = app
                    .viewport
                    .point(lp as i16 as i32, (lp >> 16) as i16 as i32);
                let inside = |(x, y): (i32, i32)| {
                    x >= app.graph_rect.left
                        && x <= app.graph_rect.right
                        && y >= app.graph_rect.top
                        && y <= app.graph_rect.bottom
                };
                let dirty = inside(next) || app.hover.is_some_and(inside);
                app.hover = if inside(next) { Some(next) } else { None };
                if dirty {
                    InvalidateRect(hwnd, null(), 0);
                }
                0
            }
            WM_KEYDOWN => {
                let app = &mut *ptr;
                let action = match wp as u16 {
                    VK_SPACE => Some(Action::Pause),
                    VK_F2 => Some(Action::Group(match app.grouping {
                        Grouping::Name => Grouping::Executable,
                        Grouping::Executable => Grouping::Family,
                        Grouping::Family => Grouping::Process,
                        Grouping::Process => Grouping::Name,
                    })),
                    VK_F3 => Some(Action::Growth),
                    VK_F4 => Some(Action::Inactive),
                    VK_F5 => Some(Action::Metric),
                    VK_F6 => Some(Action::Overview(!app.overview)),
                    VK_F7 => Some(Action::RefreshComparison),
                    VK_F8 => match app.selection {
                        Some(Selection::Process(key)) => Some(Action::Parent(key)),
                        _ => None,
                    },
                    VK_OEM_PLUS | VK_ADD => Some(Action::TimeZoom(true)),
                    VK_OEM_MINUS | VK_SUBTRACT => Some(Action::TimeZoom(false)),
                    VK_RETURN => Some(Action::Drill),
                    VK_BACK => Some(Action::Back),
                    0x31 => Some(Action::Range(300)),
                    0x32 => Some(Action::Range(900)),
                    0x33 => Some(Action::Range(1800)),
                    _ => None,
                };
                if let Some(action) = action {
                    app.action(hwnd, action);
                } else if wp as u16 == VK_DOWN || wp as u16 == VK_UP {
                    let selected = app
                        .selection
                        .as_ref()
                        .and_then(|s| app.rows.iter().position(|r| &r.selection == s))
                        .unwrap_or(0);
                    let next = if wp as u16 == VK_DOWN {
                        (selected + 1).min(app.rows.len().saturating_sub(1))
                    } else {
                        selected.saturating_sub(1)
                    };
                    if next < app.scroll {
                        app.scroll = next;
                    } else if next >= app.scroll + app.visible_rows() {
                        app.scroll = next + 1 - app.visible_rows();
                    }
                    app.action(hwnd, Action::Select(next));
                }
                0
            }
            WM_ERASEBKGND => 1,
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = zeroed();
                let dc = BeginPaint(hwnd, &mut ps);
                let mut r: RECT = zeroed();
                GetClientRect(hwnd, &mut r);
                let app = &mut *ptr;
                if r.right > 0 && r.bottom > 0 && app.ensure_buffer(dc, r.right, r.bottom) {
                    let saved = SaveDC(app.buffer);
                    SetMapMode(app.buffer, MM_ANISOTROPIC);
                    SetWindowExtEx(app.buffer, 96, 96, null_mut());
                    let v = app.viewport;
                    SetViewportExtEx(app.buffer, v.scale as i32, v.scale as i32, null_mut());
                    let (x, y) = v.physical(0, 0);
                    SetViewportOrgEx(app.buffer, x, y, null_mut());
                    app.render(app.buffer);
                    RestoreDC(app.buffer, saved);
                    BitBlt(dc, 0, 0, r.right, r.bottom, app.buffer, 0, 0, SRCCOPY);
                }
                EndPaint(hwnd, &ps);
                0
            }
            WM_CLOSE => {
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

pub fn run() -> io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let smoke = if args.next().is_some_and(|a| a == "--smoke-test") {
        let dir = PathBuf::from(
            args.next()
                .ok_or_else(|| io::Error::other("--smoke-test requires an output directory"))?,
        );
        std::fs::create_dir_all(&dir)?;
        Some(dir)
    } else {
        None
    };
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(null());
        let class = wide("MemMap.Native.Window");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            lpszClassName: class.as_ptr(),
            ..zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut app = Box::new(App::new(smoke));
        let dpi = GetDpiForSystem().max(96) as i32;
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("Mem Map — Navigation and zoom").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN | WS_HSCROLL | WS_VSCROLL,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1260 * dpi / 96,
            900 * dpi / 96,
            null_mut(),
            null_mut(),
            instance,
            (&mut *app as *mut App).cast(),
        );
        if hwnd.is_null() {
            return Err(io::Error::last_os_error());
        }
        if app.smoke.is_none() {
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
                return Err(io::Error::last_os_error());
            }
            // Native edits/combos can consume wheel input while focused. Route
            // interface zoom globally, and ordinary wheel input outside the
            // growth field/list, through the same canvas hit coordinates.
            if message.message == WM_MOUSEWHEEL && message.hwnd != hwnd {
                let x = message.lParam as i16 as i32;
                let y = (message.lParam >> 16) as i16 as i32;
                let mut growth: RECT = zeroed();
                GetWindowRect(app.growth_input, &mut growth);
                if message.wParam & 0x0008 != 0
                    || (SendMessageW(app.growth_input, CB_GETDROPPEDSTATE, 0, 0) == 0
                        && !contains(growth, x, y))
                {
                    SendMessageW(hwnd, WM_MOUSEWHEEL, message.wParam, message.lParam);
                    continue;
                }
            }
            if message.message == WM_KEYDOWN {
                if GetKeyState(VK_CONTROL as i32) < 0 {
                    let direction = match message.wParam as u16 {
                        VK_OEM_PLUS | VK_ADD => Some(1),
                        VK_OEM_MINUS | VK_SUBTRACT => Some(-1),
                        0x30 | VK_NUMPAD0 => Some(0),
                        _ => None,
                    };
                    if let Some(direction) = direction {
                        app.action(hwnd, Action::UiZoom(direction));
                        continue;
                    }
                }
                if message.wParam == 0x46 && GetKeyState(VK_CONTROL as i32) < 0 {
                    SetFocus(app.search);
                    SendMessageW(app.search, EM_SETSEL, 0, -1);
                    continue;
                }
                if message.wParam == VK_ESCAPE as usize {
                    SetWindowTextW(app.search, wide("").as_ptr());
                    SetFocus(hwnd);
                    continue;
                }
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        Ok(())
    }
}
