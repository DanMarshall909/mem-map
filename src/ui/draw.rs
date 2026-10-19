use super::*;

pub(super) const BG: u32 = rgb(32, 35, 39);
pub(super) const PANEL: u32 = rgb(40, 45, 50);
pub(super) const INPUT: u32 = BG;
pub(super) const DIVIDER: u32 = rgb(66, 73, 79);
pub(super) const BOUNDARY: u32 = rgb(114, 124, 132);
pub(super) const INK: u32 = rgb(236, 239, 241);
pub(super) const MUTED: u32 = rgb(184, 192, 198);
pub(super) const ORANGE: u32 = rgb(242, 161, 74);
pub(super) const GREEN: u32 = rgb(108, 188, 139);
pub(super) const SELECTED: u32 = rgb(61, 53, 44);
pub(super) const HOVER: u32 = rgb(52, 58, 64);
pub(super) const CHART_SEPARATOR: u32 = rgb(16, 18, 20);
pub(super) const LAYERS: [u32; 5] = [
    rgb(115, 123, 130),
    rgb(52, 112, 77),
    GREEN,
    ORANGE,
    rgb(215, 130, 51),
];
pub(super) const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}
#[derive(Clone, Copy)]
pub(super) struct Fonts {
    pub normal: HFONT,
    pub small: HFONT,
    pub bold: HFONT,
    pub title: HFONT,
    pub panel: HFONT,
}
impl Fonts {
    pub unsafe fn new(scale: u32) -> Self {
        unsafe fn font(height: i32, weight: u32, scale: u32) -> HFONT {
            unsafe {
                CreateFontW(
                    -height * scale as i32 / 96,
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
                    DEFAULT_PITCH as u32 | FF_DONTCARE as u32,
                    wide("Segoe UI").as_ptr(),
                )
            }
        }
        unsafe {
            Self {
                normal: font(14, FW_NORMAL, scale),
                small: font(12, FW_NORMAL, scale),
                bold: font(14, FW_SEMIBOLD, scale),
                title: font(27, FW_SEMIBOLD, scale),
                panel: font(19, FW_SEMIBOLD, scale),
            }
        }
    }
    pub unsafe fn delete(self) {
        unsafe {
            for font in [self.normal, self.small, self.bold, self.title, self.panel] {
                if !font.is_null() {
                    DeleteObject(font);
                }
            }
        }
    }
}

pub(super) struct Canvas<'a> {
    pub dc: HDC,
    pub fonts: &'a Fonts,
}
impl Canvas<'_> {
    pub fn fill(&self, r: RECT, color: u32) {
        unsafe {
            let brush = CreateSolidBrush(color);
            FillRect(self.dc, &r, brush);
            DeleteObject(brush);
        }
    }
    pub fn border(&self, r: RECT, color: u32) {
        unsafe {
            let brush = CreateSolidBrush(color);
            FrameRect(self.dc, &r, brush);
            DeleteObject(brush);
        }
    }
    pub fn panel(&self, r: RECT) {
        self.fill(r, PANEL);
        self.border(r, DIVIDER);
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "Explicit drawing geometry, font and alignment"
    )]
    pub fn text(
        &self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        s: &str,
        font: HFONT,
        color: u32,
        right: bool,
    ) {
        if w <= 0 || h <= 0 {
            return;
        }
        unsafe {
            let old = SelectObject(self.dc, font);
            SetBkMode(self.dc, TRANSPARENT as i32);
            SetTextColor(self.dc, color);
            let mut bounds = rect(x, y, w, h);
            let text = wide(s);
            DrawTextW(
                self.dc,
                text.as_ptr(),
                (text.len() - 1) as i32,
                &mut bounds,
                DT_SINGLELINE
                    | DT_VCENTER
                    | DT_END_ELLIPSIS
                    | DT_NOPREFIX
                    | if right { DT_RIGHT } else { DT_LEFT },
            );
            SelectObject(self.dc, old);
        }
    }
    pub fn paragraph(&self, r: RECT, s: &str, color: u32) {
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
                DT_WORDBREAK | DT_NOPREFIX,
            );
            SelectObject(self.dc, old);
        }
    }
    pub fn line(&self, points: &[POINT], color: u32, width: i32) {
        if points.len() < 2 {
            return;
        }
        unsafe {
            let pen = CreatePen(PS_SOLID, width, color);
            let old = SelectObject(self.dc, pen);
            Polyline(self.dc, points.as_ptr(), points.len() as i32);
            SelectObject(self.dc, old);
            DeleteObject(pen);
        }
    }
    pub fn dashed(&self, y: i32, left: i32, right: i32, color: u32) {
        let mut x = left;
        while x < right {
            self.line(
                &[
                    POINT { x, y },
                    POINT {
                        x: (x + 5).min(right),
                        y,
                    },
                ],
                color,
                1,
            );
            x += 9;
        }
    }
    pub fn circle(&self, x: i32, y: i32, radius: i32, color: u32) {
        unsafe {
            let brush = CreateSolidBrush(color);
            let old = SelectObject(self.dc, brush);
            let pen = SelectObject(self.dc, GetStockObject(NULL_PEN));
            Ellipse(self.dc, x - radius, y - radius, x + radius, y + radius);
            SelectObject(self.dc, pen);
            SelectObject(self.dc, old);
            DeleteObject(brush);
        }
    }
    pub fn polygon(&self, points: &[POINT], color: u32) {
        if points.len() < 3 {
            return;
        }
        unsafe {
            let brush = CreateSolidBrush(color);
            let old = SelectObject(self.dc, brush);
            let pen = SelectObject(self.dc, GetStockObject(NULL_PEN));
            Polygon(self.dc, points.as_ptr(), points.len() as i32);
            SelectObject(self.dc, pen);
            SelectObject(self.dc, old);
            DeleteObject(brush);
        }
    }
}

pub(super) unsafe fn draw_button(hwnd: HWND, dc: HDC) {
    unsafe {
        let mut r = zeroed();
        GetClientRect(hwnd, &mut r);
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let kind = style & BS_TYPEMASK as u32;
        let check = SendMessageW(hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
        let focused = GetFocus() == hwnd;
        let id = GetDlgCtrlID(hwnd);
        let accent = id == ID_EXPORT;
        let selected = check || accent;
        let font = SendMessageW(hwnd, WM_GETFONT, 0, 0) as HFONT;
        let mut attributes: LOGFONTW = zeroed();
        GetObjectW(
            font,
            size_of::<LOGFONTW>() as i32,
            (&mut attributes as *mut LOGFONTW).cast(),
        );
        let scaled = |n: i32| n * attributes.lfHeight.abs().max(1) / 14;
        let fonts = Fonts {
            normal: font,
            small: font,
            bold: font,
            title: font,
            panel: font,
        };
        let c = Canvas { dc, fonts: &fonts };
        c.fill(
            r,
            if selected {
                if accent { GREEN } else { ORANGE }
            } else {
                PANEL
            },
        );
        c.border(r, if id == ID_PAUSE { ORANGE } else { BOUNDARY });
        let mut text = vec![0u16; 1024];
        let len = GetWindowTextW(hwnd, text.as_mut_ptr(), 1024);
        let text = String::from_utf16_lossy(&text[..len as usize]);
        let checkbox = kind == BS_AUTOCHECKBOX as u32 || kind == BS_CHECKBOX as u32;
        if checkbox {
            let s = (r.bottom - r.top).min(scaled(16));
            let boxr = rect(scaled(9), (r.bottom - s) / 2, s, s);
            c.fill(r, PANEL);
            c.border(boxr, if check { ORANGE } else { BOUNDARY });
            if check {
                c.line(
                    &[
                        POINT {
                            x: boxr.left + scaled(3),
                            y: boxr.top + scaled(8),
                        },
                        POINT {
                            x: boxr.left + scaled(6),
                            y: boxr.top + scaled(11),
                        },
                        POINT {
                            x: boxr.right - scaled(3),
                            y: boxr.top + scaled(4),
                        },
                    ],
                    ORANGE,
                    2,
                );
            }
            c.text(
                scaled(32),
                0,
                r.right - scaled(38),
                r.bottom,
                &text,
                font,
                INK,
                false,
            );
        } else if (ID_LEGEND..ID_LEGEND + 5).contains(&id) {
            c.fill(
                rect(
                    scaled(7),
                    scaled(8),
                    scaled(8),
                    (r.bottom - scaled(16)).max(scaled(6)),
                ),
                LAYERS[(id - ID_LEGEND) as usize],
            );
            let (name, value) = text.rsplit_once("  ").unwrap_or((&text, ""));
            c.text(
                scaled(22),
                scaled(1),
                r.right - scaled(28),
                scaled(16),
                name,
                font,
                INK,
                false,
            );
            c.text(
                scaled(22),
                scaled(17),
                r.right - scaled(28),
                scaled(16),
                value,
                font,
                INK,
                false,
            );
        } else {
            c.text(
                8,
                0,
                r.right - 16,
                r.bottom,
                &text,
                font,
                if IsWindowEnabled(hwnd) == 0 {
                    MUTED
                } else if selected {
                    BG
                } else {
                    if id == ID_DETAILS { ORANGE } else { INK }
                },
                false,
            );
        }
        if id == ID_DETAILS {
            let mut extent = zeroed();
            let text = wide(&text);
            let old = SelectObject(dc, font);
            GetTextExtentPoint32W(dc, text.as_ptr(), (text.len() - 1) as i32, &mut extent);
            SelectObject(dc, old);
            c.line(
                &[
                    POINT {
                        x: 8,
                        y: r.bottom / 2 + extent.cy / 2 - 1,
                    },
                    POINT {
                        x: (8 + extent.cx).min(r.right - 8),
                        y: r.bottom / 2 + extent.cy / 2 - 1,
                    },
                ],
                ORANGE,
                1,
            );
        }
        if focused {
            let mut focus = r;
            InflateRect(&mut focus, -3, -3);
            DrawFocusRect(dc, &focus);
        }
    }
}
