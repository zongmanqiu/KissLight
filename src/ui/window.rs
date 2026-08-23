//! 主窗口 —— UI 创建、窗口过程、控件、自绘进度条。
//!
//! 从 main.rs 提取，遵循 Pecia 的 core/ui 分离架构。

#![allow(non_snake_case)]

use std::env;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    COLORREF, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM, BOOL,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, CreateFontIndirectW, CreatePen, DeleteObject, DrawTextW, EndPaint, FillRect,
    GetDC, GetStockObject, GetSysColorBrush, GetTextExtentPoint32W, InvalidateRect, LineTo, MoveToEx, NULL_BRUSH, PAINTSTRUCT, ReleaseDC,
    SelectObject, SetBkMode, SetTextColor, COLOR_WINDOW,
    HBRUSH, HFONT, HPEN, DT_CENTER, DT_SINGLELINE, DT_VCENTER, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetPriorityClass, SetPriorityClass, PROCESS_CREATION_FLAGS,
    BELOW_NORMAL_PRIORITY_CLASS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_GROUPBOX,
    CreateWindowExW, DefWindowProcW,
    DispatchMessageW, EnumChildWindows, GetClientRect, GetDlgItem, GetMessageW, GetWindowLongPtrW,
    GetWindowTextLengthW, GetWindowTextW, RegisterClassW, SendMessageW,
    SetForegroundWindow, SetTimer, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, SystemParametersInfoW,
    TranslateMessage, CS_DBLCLKS, ES_NUMBER, GWLP_USERDATA, HWND_TOP, MSG,
    NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS, SPI_GETWORKAREA, SW_HIDE, SW_SHOW,
    SWP_NOSIZE, SWP_NOZORDER, SWP_NOMOVE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND,
    WM_CREATE, WM_CTLCOLORSTATIC, WM_DESTROY, WM_PAINT, WM_SETICON, WM_SETFONT, WM_TIMER,
    WNDCLASSW, WS_CAPTION, WS_CHILD, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_POPUP,
    WS_SYSMENU, WS_VISIBLE, WS_EX_CLIENTEDGE, HICON, HMENU,
};

use crate::core::config::AppConfig;
use crate::core::nightlight::NightlightController;

// ---------------------------------------------------------------------------
// 控件 ID
// ---------------------------------------------------------------------------
const IDC_TOGGLE_BTN: i32 = 1001;
const IDC_TEMP_SLIDER: i32 = 1002;
const IDC_VALUE_LABEL: i32 = 1003;

const IDC_BAR_PHYS: i32 = 3001;
const IDC_BAR_PAGE: i32 = 3002;
const IDC_BAR_CACHE: i32 = 3003;
const IDC_INFO_PHYS: i32 = 3011;
const IDC_INFO_PAGE: i32 = 3012;
const IDC_INFO_CACHE: i32 = 3013;

const IDC_INTERVAL_EDIT: i32 = 2101;
const IDC_COMBO_LEVEL: i32 = 2102;

const IDC_AUTOSTART: i32 = 2007;
const IDC_FSAVOID: i32 = 2008;

const WIN_W: i32 = 300; // 最小宽度，运行时自动调整
const WIN_H: i32 = 202;
const MARGIN: i32 = 14; // 四周统一边缘间隔
const TIMER_ID: usize = 1;
const TIMER_MS: u32 = 1000;
// 注册表写入节流定时器：停止拖动 400ms 后在后台线程写入
const TEMP_TIMER_ID: usize = 2;
const TEMP_TIMER_MS: u32 = 400;

// 待写入的色温值（原子共享，节流定时器到期后后台线程读取写入）
static PENDING_TEMP: AtomicI32 = AtomicI32::new(-1);

// ---------------------------------------------------------------------------
// 共享状态
// ---------------------------------------------------------------------------
struct SharedState {
    cfg: Arc<RwLock<AppConfig>>,
    quit: AtomicBool,
    hwnd: AtomicUsize,
}
impl SharedState {
    fn new(cfg: AppConfig) -> Arc<Self> {
        Arc::new(Self {
            cfg: Arc::new(RwLock::new(cfg)),
            quit: AtomicBool::new(false),
            hwnd: AtomicUsize::new(0),
        })
    }
    fn set_hwnd(&self, h: HWND) {
        self.hwnd.store(h.0 as usize, Ordering::Relaxed);
    }
}
static SHARED: OnceLock<Arc<SharedState>> = OnceLock::new();
/// 保持系统 UI 字体句柄存活（存地址，Avoid HFONT 的 Send/Sync 要求）。
pub(crate) static KEEP_FONT: OnceLock<usize> = OnceLock::new();
fn shared() -> &'static Arc<SharedState> {
    SHARED.get().expect("SHARED not initialized")
}

// ---------------------------------------------------------------------------
// 自绘颜色条
// ---------------------------------------------------------------------------
fn RGB(r: u32, g: u32, b: u32) -> u32 {
    (b << 16) | (g << 8) | r
}
fn bar_color(pct: u32) -> u32 {
    if pct > 66 {
        RGB(0xD0, 0x30, 0x30)
    } else if pct > 33 {
        RGB(0xE0, 0xA0, 0x10)
    } else {
        RGB(0x30, 0xA0, 0x50)
    }
}
unsafe extern "system" fn bar_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_PAINT {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let w = rc.right - rc.left;
        let bg = CreateSolidBrush(COLORREF(RGB(0x28, 0x28, 0x28)));
        let _ = FillRect(hdc, &rc, bg);
        let _ = DeleteObject(bg);
        let pct = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as u32;
        let fill_w = (w as u64 * pct as u64 / 100) as i32;
        if fill_w > 0 {
            let fr = RECT {
                left: rc.left,
                top: rc.top,
                right: rc.left + fill_w,
                bottom: rc.bottom,
            };
            let fg = CreateSolidBrush(COLORREF(bar_color(pct)));
            let _ = FillRect(hdc, &fr, fg);
            let _ = DeleteObject(fg);
        }
        let pct = pct.min(100);
        if let Some(fh) = KEEP_FONT.get() {
            let _ = SelectObject(hdc, HFONT(*fh as *mut _));
        }
        let _ = SetTextColor(hdc, COLORREF(RGB(0xFF, 0xFF, 0xFF)));
        let _ = SetBkMode(hdc, TRANSPARENT);
        let mut txt = crate::core::util::wide(&format!("{}%", pct));
        let _ = DrawTextW(hdc, &mut txt, &mut rc, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
        let _ = EndPaint(hwnd, &ps);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
fn register_bar_class(hinst: windows::Win32::Foundation::HINSTANCE) {
    unsafe {
        let wc = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(bar_wndproc),
            hInstance: hinst.into(),
            lpszClassName: w!("MemCleanBar"),
            hbrBackground: HBRUSH(
                CreateSolidBrush(COLORREF(RGB(0x28, 0x28, 0x28))).0 as isize as *mut _,
            ),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);
    }
}
fn make_bar(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) -> HWND {
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("MemCleanBar"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE,
            x,
            y,
            w,
            h,
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize as *mut _),
            None,
            None,
        )
        .unwrap_or_default()
    }
}
fn bar_set(hwnd: HWND, pct: u32) {
    unsafe {
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, pct as isize);
        let _ = InvalidateRect(hwnd, None, true);
    }
}

// ---------------------------------------------------------------------------
// 工具
// ---------------------------------------------------------------------------
fn ws(v: i32) -> WINDOW_STYLE {
    WINDOW_STYLE(v as u32)
}
pub(crate) fn dlg(hwnd: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(hwnd, id).unwrap_or_default() }
}
fn make_child(
    hwnd: HWND,
    class: PCWSTR,
    text: PCWSTR,
    style: WINDOW_STYLE,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> HWND {
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            text,
            style | WS_CHILD | WS_VISIBLE,
            x,
            y,
            w,
            h,
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize as *mut _),
            None,
            None,
        )
        .unwrap_or_default()
    }
}
fn lbl(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) -> HWND {
    make_child(hwnd, w!("STATIC"), PCWSTR::null(), ws(0x200), id, x, y, w, h)
}
fn chk(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32, text: &str) -> HWND {
    let t = crate::core::util::wide(text);
    make_child(hwnd, w!("BUTTON"), PCWSTR(t.as_ptr()), ws(BS_AUTOCHECKBOX), id, x, y, w, h)
}
fn groupbox(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32, text: &str) -> HWND {
    let t = crate::core::util::wide(text);
    make_child(hwnd, w!("BUTTON"), PCWSTR(t.as_ptr()), ws(BS_GROUPBOX), id, x, y, w, h)
}
fn edt(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) -> HWND {
    unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR::null(),
            ws(ES_NUMBER) | WS_CHILD | WS_VISIBLE,
            x,
            y,
            w,
            h,
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::HMENU(id as isize as *mut _),
            None,
            None,
        )
        .unwrap_or_default()
    }
}
fn combo(hwnd: HWND, id: i32, x: i32, y: i32, w: i32) -> HWND {
    make_child(hwnd, w!("COMBOBOX"), PCWSTR::null(), ws(0x3), id, x, y, w, 120)
}

fn set_text(hwnd: HWND, id: i32, text: &str) {
    unsafe {
        let h = dlg(hwnd, id);
        if !h.0.is_null() {
            let _ = SetWindowTextW(h, PCWSTR(crate::core::util::wide(text).as_ptr()));
        }
    }
}
fn text_of(hwnd: HWND, id: i32) -> String {
    unsafe {
        let h = dlg(hwnd, id);
        let len = GetWindowTextLengthW(h);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        GetWindowTextW(h, &mut buf);
        crate::core::util::wide_to_string(&buf)
    }
}
fn checked(hwnd: HWND, id: i32) -> bool {
    unsafe {
        let h = dlg(hwnd, id);
        SendMessageW(h, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 != 0
    }
}
fn set_checked(hwnd: HWND, id: i32, on: bool) {
    unsafe {
        let h = dlg(hwnd, id);
        let _ = SendMessageW(h, BM_SETCHECK, WPARAM(if on { 1 } else { 0 }), LPARAM(0));
    }
}
fn set_edit(hwnd: HWND, id: i32, v: u32) {
    let s = format!("{v}");
    unsafe {
        let h = dlg(hwnd, id);
        let _ = SetWindowTextW(h, PCWSTR(crate::core::util::wide(&s).as_ptr()));
    }
}
fn edit_val(hwnd: HWND, id: i32) -> Option<u32> {
    text_of(hwnd, id).trim().parse::<u32>().ok()
}

// ---------------------------------------------------------------------------
// Logo 加载（从嵌入的 ICO 资源加载）
// ---------------------------------------------------------------------------
pub fn load_logo_icon() -> HICON {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{LoadImageW, IMAGE_ICON, LR_DEFAULTCOLOR};
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let hinst: windows::Win32::Foundation::HINSTANCE =
            windows::Win32::Foundation::HINSTANCE(hinst.0);
        let icon = LoadImageW(
            hinst,
            PCWSTR(1 as *const u16),
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTCOLOR,
        );
        match icon {
            Ok(h) => HICON(h.0 as *mut _),
            Err(_) => HICON::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// 刷新信息（空函数，因为我们不再需要刷新内存信息）
// ---------------------------------------------------------------------------
fn refresh_info(_hwnd: HWND) {
    // 不再需要刷新内存信息
}

// ---------------------------------------------------------------------------
// 清理（空函数，因为我们不再需要清理内存）
// ---------------------------------------------------------------------------
fn run_clean_level(_level: u32) {
    // 不再需要清理内存
}

fn lower_self_priority() -> PROCESS_CREATION_FLAGS {
    unsafe {
        let h = GetCurrentProcess();
        let prev = GetPriorityClass(h);
        let _ = SetPriorityClass(h, BELOW_NORMAL_PRIORITY_CLASS);
        PROCESS_CREATION_FLAGS(prev)
    }
}

fn restore_self_priority(prev: PROCESS_CREATION_FLAGS) {
    unsafe {
        let h = GetCurrentProcess();
        let _ = SetPriorityClass(h, prev);
    }
}
fn run_manual() {
    let level = shared().cfg.read().map(|g| g.level).unwrap_or(2);
    run_clean_level(level);
}

// ---------------------------------------------------------------------------
// 开机启动
// ---------------------------------------------------------------------------
fn set_autostart(enable: bool) {
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_OPEN_CREATE_OPTIONS, REG_SZ,
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW,
    };
    let key = crate::core::util::wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let value = crate::core::util::wide("memclean");
    unsafe {
        let mut hkey = HKEY::default();
        let res = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            0,
            None,
            REG_OPEN_CREATE_OPTIONS(0),
            KEY_READ | KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if res.is_ok() {
            if enable {
                if let Ok(exe) = std::env::current_exe() {
                    let mut data = crate::core::util::wide(&format!("\"{}\" silent", exe.display()));
                    data.push(0);
                    let bytes: &[u8] =
                        std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2);
                    let _ = RegSetValueExW(hkey, PCWSTR(value.as_ptr()), 0, REG_SZ, Some(bytes));
                }
            } else {
                let _ = RegDeleteValueW(hkey, PCWSTR(value.as_ptr()));
            }
            let _ = RegCloseKey(hkey);
        }
    }
}

// ---------------------------------------------------------------------------
// 界面同步 / 保存
// ---------------------------------------------------------------------------
fn sync_ui(hwnd: HWND, c: &AppConfig) {
    set_edit(hwnd, IDC_INTERVAL_EDIT, c.interval_minutes);
    combo_set_level(hwnd, c.level);
    set_checked(hwnd, IDC_AUTOSTART, c.autostart);
    set_checked(hwnd, IDC_FSAVOID, c.fullscreen_avoid);
}

fn save_from_ui(hwnd: HWND) {
    let mut c = shared().cfg.read().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(v) = edit_val(hwnd, IDC_INTERVAL_EDIT) {
        c.interval_minutes = v;
    }
    c.level = combo_level(hwnd);
    c.autostart = checked(hwnd, IDC_AUTOSTART);
    c.fullscreen_avoid = checked(hwnd, IDC_FSAVOID);
    c.sanitize();
    *shared().cfg.write().unwrap_or_else(|e| e.into_inner()) = c.clone();
    crate::core::config::save(&c);
    sync_ui(hwnd, &c);
}

// ---- 清理强度下拉框 ----
fn combo_add_string(hwnd: HWND, id: i32, item: &str) {
    unsafe {
        let h = dlg(hwnd, id);
        let text = crate::core::util::wide(item);
        let _ = SendMessageW(
            h,
            windows::Win32::UI::WindowsAndMessaging::CB_ADDSTRING,
            WPARAM(0),
            LPARAM(text.as_ptr() as isize),
        );
    }
}
fn combo_sel(hwnd: HWND, id: i32) -> u32 {
    unsafe {
        let h = dlg(hwnd, id);
        SendMessageW(h, 0x0147, WPARAM(0), LPARAM(0)).0 as u32
    }
}
fn combo_set_sel(hwnd: HWND, id: i32, idx: usize) {
    unsafe {
        let h = dlg(hwnd, id);
        let _ = SendMessageW(h, 0x014E, WPARAM(idx), LPARAM(0));
    }
}
fn combo_set_level(hwnd: HWND, level: u32) {
    let idx = (level.saturating_sub(1)).min(1) as usize;
    combo_set_sel(hwnd, IDC_COMBO_LEVEL, idx);
}
fn combo_level(hwnd: HWND) -> u32 {
    let sel = combo_sel(hwnd, IDC_COMBO_LEVEL);
    if sel == u32::MAX {
        1
    } else {
        (sel + 1).min(2)
    }
}

// ---------------------------------------------------------------------------
// 窗口过程
// ---------------------------------------------------------------------------
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            shared().set_hwnd(hwnd);
            crate::core::gamma::save_original();
            create_ui(hwnd);
            // 启动时自动开启夜灯
            let controller = NightlightController::new();
            let intensity = controller.get_color_temperature()
                .map(|t| temp_to_intensity(t))
                .unwrap_or(50);
            let temp = intensity_to_temp(intensity);
            crate::core::gamma::apply_temp(temp);
            if let Ok(h_slider) = windows::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_TEMP_SLIDER) {
                SendMessageW(h_slider, 0x0405, WPARAM(1), LPARAM(intensity as isize));
            }
            // 更新数值标签
            let text = format!(" {:>2}", intensity);
            let wide = crate::core::util::wide(&text);
            if let Ok(h_label) = windows::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_VALUE_LABEL) {
                let _ = SetWindowTextW(h_label, windows::core::PCWSTR(wide.as_ptr()));
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            // 上边框 2px
            let pen2 = windows::Win32::Graphics::Gdi::CreatePen(windows::Win32::Graphics::Gdi::PEN_STYLE(0), 2, COLORREF(RGB(0, 0, 0)));
            let old_pen = SelectObject(hdc, pen2);
            let _ = windows::Win32::Graphics::Gdi::MoveToEx(hdc, 0, 0, None);
            let _ = windows::Win32::Graphics::Gdi::LineTo(hdc, rc.right, 0);
            // 左边框 2px
            let _ = windows::Win32::Graphics::Gdi::MoveToEx(hdc, 0, 0, None);
            let _ = windows::Win32::Graphics::Gdi::LineTo(hdc, 0, rc.bottom);
            let _ = SelectObject(hdc, old_pen);
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(pen2);
            // 下边框 1px
            let pen1 = windows::Win32::Graphics::Gdi::CreatePen(windows::Win32::Graphics::Gdi::PEN_STYLE(0), 1, COLORREF(RGB(0, 0, 0)));
            let old_pen = SelectObject(hdc, pen1);
            let _ = windows::Win32::Graphics::Gdi::MoveToEx(hdc, 0, rc.bottom - 1, None);
            let _ = windows::Win32::Graphics::Gdi::LineTo(hdc, rc.right, rc.bottom - 1);
            // 右边框 1px
            let _ = windows::Win32::Graphics::Gdi::MoveToEx(hdc, rc.right - 1, 0, None);
            let _ = windows::Win32::Graphics::Gdi::LineTo(hdc, rc.right - 1, rc.bottom);
            let _ = SelectObject(hdc, old_pen);
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(pen1);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == TEMP_TIMER_ID {
                // 节流到期：用户已停止拖动，后台线程写入注册表（不阻塞 UI）
                windows::Win32::UI::WindowsAndMessaging::KillTimer(hwnd, TEMP_TIMER_ID);
                let temp = PENDING_TEMP.load(Ordering::Relaxed);
                if temp > 0 {
                    std::thread::spawn(move || {
                        let controller = NightlightController::new();
                        let _ = controller.set_color_temperature(temp as u16);
                    });
                }
            } else if wparam.0 == 3 {
                // 延迟隐藏定时器：检查窗口是否仍然未激活
                windows::Win32::UI::WindowsAndMessaging::KillTimer(hwnd, 3);
                let _ = ShowWindow(hwnd, SW_HIDE);
            } else if wparam.0 == TIMER_ID {
                refresh_info(hwnd);
            }
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let br = GetSysColorBrush(COLOR_WINDOW);
            LRESULT(br.0 as isize)
        }
        WM_COMMAND => {
            LRESULT(0)
        }
        0x0114 /* WM_HSCROLL */ => {
            let h_slider = HWND(lparam.0 as *mut _);
            let id = windows::Win32::UI::WindowsAndMessaging::GetDlgCtrlID(h_slider);
            let pos = SendMessageW(h_slider, 0x0400, WPARAM(0), LPARAM(0)).0 as i32;

            if id == IDC_TEMP_SLIDER {
                let temp = intensity_to_temp(pos);
                crate::core::gamma::apply_temp(temp);
                // 更新数值标签
                let text = format!(" {:>2}", pos);
                let wide = crate::core::util::wide(&text);
                if let Ok(h_label) = windows::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, IDC_VALUE_LABEL) {
                    let _ = SetWindowTextW(h_label, windows::core::PCWSTR(wide.as_ptr()));
                }
                PENDING_TEMP.store(temp as i32, Ordering::Relaxed);
                let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(hwnd, TEMP_TIMER_ID);
                windows::Win32::UI::WindowsAndMessaging::SetTimer(hwnd, TEMP_TIMER_ID, TEMP_TIMER_MS, None);
            }
            LRESULT(0)
        }
        0x0128 /* WM_UPDATEUISTATE */ => {
            // 隐藏焦点矩形
            LRESULT(0)
        }
        0x0006 /* WM_ACTIVATE */ => {
            if (wparam.0 & 0xffff) == 0 /* WA_INACTIVE */ {
                // 延迟隐藏，避免拖动滑块时窗口消失
                windows::Win32::UI::WindowsAndMessaging::SetTimer(hwnd, 3, 200, None);
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        super::tray::MSG_TRAY_SHOW => {
            position_near_tray(hwnd);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            LRESULT(0)
        }
        super::tray::MSG_TRAY_CLEAN => {
            let lv = wparam.0 as u32;
            if (1..=3).contains(&lv) {
                run_clean_level(lv);
            } else {
                run_manual();
            }
            LRESULT(0)
        }
        super::tray::MSG_TRAY_QUIT => {
            // 恢复 GammaRamp（注册表设置保留，系统稍后按注册表应用）
            crate::core::gamma::restore();
            shared().quit.store(true, Ordering::Relaxed);
            // 必须调用 DestroyWindow 触发 WM_DESTROY，否则窗口句柄泄漏导致进程残留
            windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
            LRESULT(0)
        }
        super::tray::MSG_TRAY_ABOUT => {
            super::about::show_about_dialog(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            // 恢复 GammaRamp
            crate::core::gamma::restore();
            windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ---------------------------------------------------------------------------
// 文本宽度测量（用于自动计算窗口宽度）
// ---------------------------------------------------------------------------
fn measure_text_width(hwnd: HWND, text: &str) -> i32 {
    unsafe {
        let hdc = GetDC(hwnd);
        if let Some(&fh) = KEEP_FONT.get() {
            let _ = SelectObject(hdc, HFONT(fh as *mut _));
        }
        let wide = crate::core::util::wide(text);
        let mut size = SIZE::default();
        let _ = GetTextExtentPoint32W(hdc, &wide, &mut size);
        let _ = ReleaseDC(hwnd, hdc);
        size.cx
    }
}

// ---------------------------------------------------------------------------
// 强度 ↔ 色温映射（微软 Night Light 风格）
// 强度 0   = 6500K（不变）
// 强度 100 = 1200K（最暖）
// ---------------------------------------------------------------------------
fn intensity_to_temp(intensity: i32) -> u16 {
    (6500 - (6500 - 1200) * intensity / 100) as u16
}

fn temp_to_intensity(temp: u16) -> i32 {
    let t = temp.clamp(1200, 6500) as i32;
    (6500 - t) * 100 / (6500 - 1200)
}

// ---------------------------------------------------------------------------
// 界面创建
// ---------------------------------------------------------------------------
fn create_ui(hwnd: HWND) {
    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();

        // 数值标签（居中于左边框和滑块之间）
        let val_text = crate::core::util::wide(" 0");
        let h_val = CreateWindowExW(
            WINDOW_EX_STYLE(0), w!("STATIC"), windows::core::PCWSTR(val_text.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            14, 18, 30, 24,
            hwnd, HMENU(IDC_VALUE_LABEL as _), hinstance, None,
        ).unwrap_or_default();
        // 设置较大字体
        let hfont = windows::Win32::Graphics::Gdi::CreateFontW(
            18, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
        );
        let _ = SendMessageW(h_val, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));

        // 强度滑块（0-100）
        let h_slider = CreateWindowExW(
            WINDOW_EX_STYLE(0), w!("msctls_trackbar32"), w!(""),
            WS_CHILD | WS_VISIBLE,
            48, 15, 246, 30,
            hwnd, HMENU(IDC_TEMP_SLIDER as _), hinstance, None,
        ).unwrap_or_default();
        SendMessageW(h_slider, 0x0406, WPARAM(1), LPARAM((100 << 16) | 0));
        SendMessageW(h_slider, 0x0405, WPARAM(1), LPARAM(0));
        // 去掉刻度线（TBM_SETTICS = 0x0404）
        let no_ticks: Vec<isize> = vec![0];
        SendMessageW(h_slider, 0x0404, WPARAM(0), LPARAM(no_ticks.as_ptr() as isize));
    }
}

// ---------------------------------------------------------------------------
// 窗口位置
// ---------------------------------------------------------------------------
fn position_bottom_right(hwnd: HWND) {
    unsafe {
        let mut rect = RECT::default();
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut _ as *mut _),
            windows::Win32::UI::WindowsAndMessaging::SPIF_SENDCHANGE,
        );
        let wa_left = rect.left as i32;
        let wa_top = rect.top as i32;
        let wa_w = rect.right - rect.left;
        let wa_h = rect.bottom - rect.top;
        let mut wrect = RECT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut wrect);
        let w_w = wrect.right - wrect.left;
        let w_h = wrect.bottom - wrect.top;
        let m = 10i32;
        let x = wa_left + wa_w - w_w - m;
        let y = wa_top + wa_h - w_h - m;
        let _ = SetWindowPos(hwnd, HWND_TOP, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
    }
}

fn position_near_tray(hwnd: HWND) {
    unsafe {
        let mut rect = RECT::default();
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut _ as *mut _),
            windows::Win32::UI::WindowsAndMessaging::SPIF_SENDCHANGE,
        );
        let wa_right = rect.right as i32;
        let wa_bottom = rect.bottom as i32;
        let w = 300i32;
        let h = 60i32;
        let x = wa_right - w - 20;
        let y = wa_bottom - h - 20;
        let _ = SetWindowPos(hwnd, HWND_TOP, x, y, w, h, SWP_NOZORDER);
    }
}

// ---------------------------------------------------------------------------
// UI 字体
// ---------------------------------------------------------------------------
fn apply_ui_font(hwnd: HWND) {
    unsafe {
        let mut ncm = NONCLIENTMETRICSW::default();
        ncm.cbSize = size_of::<NONCLIENTMETRICSW>() as u32;
        if SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            ncm.cbSize,
            Some(&mut ncm as *mut _ as *mut _),
            windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_err()
        {
            return;
        }
        let font = CreateFontIndirectW(&ncm.lfMessageFont);
        if font.is_invalid() {
            return;
        }
        let _ = KEEP_FONT.set(font.0 as usize);
        let _ = SendMessageW(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        let _ = EnumChildWindows(hwnd, Some(enum_child_setfont), LPARAM(font.0 as isize));
    }
}

unsafe extern "system" fn enum_child_setfont(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let font = HFONT(lparam.0 as *mut _);
    let _ = SendMessageW(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    BOOL(1)
}

fn init_common_controls() {
    unsafe {
        let icc = windows::Win32::UI::Controls::INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<windows::Win32::UI::Controls::INITCOMMONCONTROLSEX>()
                as u32,
            dwICC: windows::Win32::UI::Controls::ICC_STANDARD_CLASSES
                | windows::Win32::UI::Controls::ICC_WIN95_CLASSES,
        };
        let _ = windows::Win32::UI::Controls::InitCommonControlsEx(&icc);
    }
}

// ---------------------------------------------------------------------------
// 入口（由 main.rs 调用）
// ---------------------------------------------------------------------------
pub fn run(cfg: AppConfig) {
    let shared = SharedState::new(cfg);
    let _ = SHARED.set(shared.clone());

    init_common_controls();

    unsafe {
        let hinst = GetModuleHandleW(None).unwrap();
        register_bar_class(hinst.into());

        let class_name = w!("KissLightMainWindow");
        let wc = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            lpszClassName: class_name,
            hbrBackground: HBRUSH((COLOR_WINDOW.0 as i32 + 1) as isize as *mut _),
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return;
        }
        let style: WINDOW_STYLE = WS_POPUP;
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!(""),
            style,
            -100,
            -100,
            300,
            60,
            None,
            None,
            hinst,
            None,
        ) else {
            return;
        };
        apply_ui_font(hwnd);

        let logo = load_logo_icon();
        if !logo.is_invalid() {
            let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(0), LPARAM(logo.0 as isize));
            let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(1), LPARAM(logo.0 as isize));
        }
        let _ = ShowWindow(hwnd, SW_HIDE);

        let silent = env::args().any(|a| a == "silent" || a == "slient");
        if silent {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }

        super::tray::spawn(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
