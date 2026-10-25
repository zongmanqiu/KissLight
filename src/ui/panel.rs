//! 主面板：无边框窗口、强度滑块与退出流程。
//!
//! 面板窗口、托盘窗口和消息循环都在同一个 UI 线程：退出时不存在第二个界面线程
//! 在跨进程调用里被强制终止（见 AGENTS.md「已知平台坑」）。

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, OnceLock};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontIndirectW, CreateFontW, CreatePen, DeleteObject, EndPaint, FillRect,
    GetSysColorBrush, LineTo, MoveToEx, SelectObject, COLOR_WINDOW, HDC, HFONT, PAINTSTRUCT,
    PEN_STYLE,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_STANDARD_CLASSES, ICC_WIN95_CLASSES, INITCOMMONCONTROLSEX,
    TBM_SETPOS, TBM_SETRANGE, TBM_SETTIC,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumChildWindows,
    GetClientRect, GetDlgItem, GetMessageW, KillTimer, LoadImageW, PostQuitMessage, RegisterClassW,
    SendMessageW, SetForegroundWindow, SetTimer, SetWindowPos, SetWindowTextW, ShowWindow,
    SystemParametersInfoW, TranslateMessage, CS_DBLCLKS, HICON, HMENU, HWND_TOP, IMAGE_ICON,
    LR_DEFAULTCOLOR, MSG, NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS, SPI_GETWORKAREA,
    SWP_NOZORDER, SW_HIDE, SW_SHOW, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WA_INACTIVE,
    WINDOW_EX_STYLE, WM_ACTIVATE, WM_CLOSE, WM_CREATE, WM_CTLCOLORSTATIC, WM_DESTROY, WM_HSCROLL,
    WM_PAINT, WM_SETFONT, WM_SETICON, WM_TIMER, WM_UPDATEUISTATE, WNDCLASSW, WS_CHILD, WS_POPUP,
    WS_VISIBLE,
};

use crate::core::gamma;
use crate::core::nightlight::NightlightController;
use crate::core::temp;
use crate::core::util;

// ---------------------------------------------------------------------------
// 常量
// ---------------------------------------------------------------------------
const IDC_INTENSITY_SLIDER: i32 = 1002;
const IDC_VALUE_LABEL: i32 = 1003;

/// 拖动停止后延迟写注册表的节流定时器。
const THROTTLE_TIMER_ID: usize = 2;
const THROTTLE_MS: u32 = 400;
/// 失焦后延迟隐藏，避免拖滑块时面板消失。
const HIDE_TIMER_ID: usize = 3;
const HIDE_DELAY_MS: u32 = 200;

const PANEL_W: i32 = 300;
const PANEL_H: i32 = 60;
/// 注册表里没有存过色温时的面板落点（约 3850K）。
const DEFAULT_INTENSITY: i32 = 50;
/// winuser.h 的 `TBM_GETPOS`（= WM_USER）；windows crate 未导出。
const TBM_GETPOS: u32 = 0x0400;

// ---------------------------------------------------------------------------
// 共享状态
// ---------------------------------------------------------------------------
/// 待写入注册表的色温（-1 = 无待写）。
static PENDING_KELVIN: AtomicI32 = AtomicI32::new(-1);

struct State {
    nightlight: NightlightController,
}

static STATE: OnceLock<Arc<State>> = OnceLock::new();

fn state() -> &'static Arc<State> {
    STATE.get().expect("STATE 未初始化")
}

/// 系统 UI 字体句柄（存地址，避免 HFONT 的 Send/Sync 要求）。关于对话框也用。
pub(crate) static KEEP_FONT: OnceLock<usize> = OnceLock::new();

/// 把最后待写入的色温同步写进注册表。
///
/// 节流定时器到期与退出路径都走这里：退出必须在 UI 线程把写操作做完，否则进程
/// 退出时另一个线程会停在注册表写入途中。
fn flush_pending_kelvin() {
    let kelvin = PENDING_KELVIN.swap(-1, Ordering::Relaxed);
    if kelvin > 0 {
        let _ = state().nightlight.set_color_temperature(kelvin as u16);
    }
}

// ---------------------------------------------------------------------------
// 小工具
// ---------------------------------------------------------------------------
fn rgb(r: u32, g: u32, b: u32) -> COLORREF {
    COLORREF((b << 16) | (g << 8) | r)
}

/// 控件 ID 以 MAKEINTRESOURCE 形式传给 `CreateWindowExW` 的 hMenu 参数。
fn control_id(id: i32) -> HMENU {
    HMENU(id as isize as *mut std::ffi::c_void)
}

/// 资源 ID 当指针是 Win32 的 MAKEINTRESOURCE 约定，不作为指针解引用。
#[allow(clippy::manual_dangling_ptr)]
fn resource_id(id: usize) -> PCWSTR {
    PCWSTR(id as *const u16)
}

/// 取子控件句柄；不存在时返回空句柄。
pub(crate) fn child(hwnd: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(hwnd, id).unwrap_or_default() }
}

fn set_value_label(hwnd: HWND, intensity: i32) {
    let text = util::wide(&format!(" {intensity:>2}"));
    unsafe {
        let _ = SetWindowTextW(child(hwnd, IDC_VALUE_LABEL), PCWSTR(text.as_ptr()));
    }
}

fn set_slider_position(hwnd: HWND, intensity: i32) {
    unsafe {
        let _ = SendMessageW(
            child(hwnd, IDC_INTENSITY_SLIDER),
            TBM_SETPOS,
            WPARAM(1),
            LPARAM(intensity as isize),
        );
    }
}

/// 主面板窗口类名（app 层用它找回已有实例）。
pub(crate) fn class_name() -> PCWSTR {
    w!("KissLightMainWindow")
}

pub(crate) fn load_logo_icon() -> HICON {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        match LoadImageW(hinstance, resource_id(1), IMAGE_ICON, 0, 0, LR_DEFAULTCOLOR) {
            Ok(handle) => HICON(handle.0),
            Err(_) => HICON::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// 界面创建
// ---------------------------------------------------------------------------
fn create_controls(hwnd: HWND) {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        // 强度数值标签
        let initial = util::wide(&format!(" {DEFAULT_INTENSITY:>2}"));
        let label = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            PCWSTR(initial.as_ptr()),
            WS_CHILD | WS_VISIBLE,
            14,
            18,
            30,
            24,
            hwnd,
            control_id(IDC_VALUE_LABEL),
            hinstance,
            None,
        )
        .unwrap_or_default();
        let label_font = CreateFontW(18, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"));
        let _ = SendMessageW(label, WM_SETFONT, WPARAM(label_font.0 as usize), LPARAM(1));

        // 强度滑块（0-100）
        let slider = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("msctls_trackbar32"),
            w!(""),
            WS_CHILD | WS_VISIBLE,
            48,
            15,
            246,
            30,
            hwnd,
            control_id(IDC_INTENSITY_SLIDER),
            hinstance,
            None,
        )
        .unwrap_or_default();
        let _ = SendMessageW(
            slider,
            TBM_SETRANGE,
            WPARAM(1),
            LPARAM(((temp::MAX_INTENSITY as isize) << 16) | temp::MIN_INTENSITY as isize),
        );
        let _ = SendMessageW(slider, TBM_SETPOS, WPARAM(1), LPARAM(0));
        // 去掉刻度线：只留 0 号刻度并关闭自动刻度
        let no_ticks = [0isize];
        let _ = SendMessageW(
            slider,
            TBM_SETTIC,
            WPARAM(0),
            LPARAM(no_ticks.as_ptr() as isize),
        );
    }
}

fn init_common_controls() {
    unsafe {
        let settings = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES | ICC_WIN95_CLASSES,
        };
        let _ = InitCommonControlsEx(&settings);
    }
}

fn draw_border(hdc: HDC, rect: &RECT) {
    unsafe {
        // 上/左边框 2px，下/右边框 1px：无边框窗口的视觉边界。
        let thick = CreatePen(PEN_STYLE(0), 2, rgb(0, 0, 0));
        let old = SelectObject(hdc, thick);
        let _ = MoveToEx(hdc, rect.left, rect.top, None);
        let _ = LineTo(hdc, rect.right, rect.top);
        let _ = MoveToEx(hdc, rect.left, rect.top, None);
        let _ = LineTo(hdc, rect.left, rect.bottom);
        let _ = SelectObject(hdc, old);
        let _ = DeleteObject(thick);

        let thin = CreatePen(PEN_STYLE(0), 1, rgb(0, 0, 0));
        let old = SelectObject(hdc, thin);
        let _ = MoveToEx(hdc, rect.left, rect.bottom - 1, None);
        let _ = LineTo(hdc, rect.right, rect.bottom - 1);
        let _ = MoveToEx(hdc, rect.right - 1, rect.top, None);
        let _ = LineTo(hdc, rect.right - 1, rect.bottom);
        let _ = SelectObject(hdc, old);
        let _ = DeleteObject(thin);
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
            gamma::save_original();
            create_controls(hwnd);
            let intensity = state()
                .nightlight
                .color_temperature()
                .ok()
                .flatten()
                .map(temp::kelvin_to_intensity)
                .unwrap_or(DEFAULT_INTENSITY);
            gamma::apply_temp(temp::intensity_to_kelvin(intensity));
            set_slider_position(hwnd, intensity);
            set_value_label(hwnd, intensity);
            LRESULT(0)
        }
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut paint);
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            let background = GetSysColorBrush(COLOR_WINDOW);
            let _ = FillRect(hdc, &rect, background);
            draw_border(hdc, &rect);
            let _ = EndPaint(hwnd, &paint);
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let background = GetSysColorBrush(COLOR_WINDOW);
            LRESULT(background.0 as isize)
        }
        WM_HSCROLL => {
            let slider = HWND(lparam.0 as *mut std::ffi::c_void);
            if slider == child(hwnd, IDC_INTENSITY_SLIDER) {
                let intensity = SendMessageW(slider, TBM_GETPOS, WPARAM(0), LPARAM(0)).0 as i32;
                let kelvin = temp::intensity_to_kelvin(intensity);
                gamma::apply_temp(kelvin);
                set_value_label(hwnd, intensity);
                PENDING_KELVIN.store(i32::from(kelvin), Ordering::Relaxed);
                let _ = KillTimer(hwnd, THROTTLE_TIMER_ID);
                SetTimer(hwnd, THROTTLE_TIMER_ID, THROTTLE_MS, None);
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == THROTTLE_TIMER_ID => {
            // 拖动已停止：把节流期间的最后一次色温落到注册表。
            let _ = KillTimer(hwnd, THROTTLE_TIMER_ID);
            flush_pending_kelvin();
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == HIDE_TIMER_ID => {
            let _ = KillTimer(hwnd, HIDE_TIMER_ID);
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_ACTIVATE => {
            if (wparam.0 & 0xffff) as u32 == WA_INACTIVE {
                SetTimer(hwnd, HIDE_TIMER_ID, HIDE_DELAY_MS, None);
            }
            LRESULT(0)
        }
        // 隐藏焦点虚线框
        WM_UPDATEUISTATE => LRESULT(0),
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
        super::tray::MSG_TRAY_ABOUT => {
            super::about::show_about_dialog(hwnd);
            LRESULT(0)
        }
        super::tray::MSG_TRAY_QUIT => {
            // 退出顺序：补写色温 → 注销托盘图标并销毁托盘窗口 → 销毁主窗口。
            // 图标先注销再释放句柄、窗口由拥有它的线程销毁，进程退出时不再有
            // 半拆除的窗口或仍被系统引用的图标。
            let _ = KillTimer(hwnd, THROTTLE_TIMER_ID);
            let _ = KillTimer(hwnd, HIDE_TIMER_ID);
            flush_pending_kelvin();
            super::tray::shutdown();
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            gamma::restore();
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ---------------------------------------------------------------------------
// 窗口位置
// ---------------------------------------------------------------------------
fn position_near_tray(hwnd: HWND) {
    unsafe {
        let mut work = RECT::default();
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut work as *mut _ as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let x = work.right - PANEL_W - 20;
        let y = work.bottom - PANEL_H - 20;
        let _ = SetWindowPos(hwnd, HWND_TOP, x, y, PANEL_W, PANEL_H, SWP_NOZORDER);
    }
}

// ---------------------------------------------------------------------------
// 系统 UI 字体
// ---------------------------------------------------------------------------
fn apply_ui_font(hwnd: HWND) {
    unsafe {
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        if SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            Some(&mut metrics as *mut _ as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_err()
        {
            return;
        }
        let font = CreateFontIndirectW(&metrics.lfMessageFont);
        if font.is_invalid() {
            return;
        }
        let _ = KEEP_FONT.set(font.0 as usize);
        let _ = SendMessageW(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        let _ = EnumChildWindows(hwnd, Some(enum_child_set_font), LPARAM(font.0 as isize));
    }
}

unsafe extern "system" fn enum_child_set_font(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let font = HFONT(lparam.0 as *mut std::ffi::c_void);
    let _ = SendMessageW(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    BOOL(1)
}

// ---------------------------------------------------------------------------
// 入口（由 app 调用）
// ---------------------------------------------------------------------------
/// 创建面板与托盘，进入消息循环，直到用户退出。
pub fn run(nightlight: NightlightController) {
    let shared = Arc::new(State { nightlight });
    let _ = STATE.set(shared);

    init_common_controls();

    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class = class_name();
        let window_class = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wnd_proc),
            hInstance: HINSTANCE(hinstance.0),
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassW(&window_class) == 0 {
            return;
        }
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!(""),
            WS_POPUP,
            -100,
            -100,
            PANEL_W,
            PANEL_H,
            None,
            None,
            hinstance,
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

        super::tray::init(hwnd);

        let mut message = MSG::default();
        loop {
            // GetMessageW 返回 -1 表示出错；BOOL::as_bool() 会把它当“有消息”，
            // 于是用零值 MSG 死循环（见 AGENTS.md「已知平台坑」）。
            let result = GetMessageW(&mut message, None, 0, 0);
            if result.0 == 0 || result.0 == -1 {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
