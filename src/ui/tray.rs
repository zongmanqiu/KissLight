//! 原生 Win32 系统托盘（与主窗口共用同一个消息循环）。
//!
//! 托盘窗口、托盘图标和主窗口都归 UI 线程所有：退出时先注销图标再销毁窗口，
//! 进程不会在另一个界面线程仍停在 `Shell_NotifyIcon` 跨进程调用途中被终止
//! （休眠唤醒后资源管理器繁忙时，这种终止会让系统弹出 “Unknown Hard Error”）。
//!
//! 托盘动作通过 `PostMessage` 发给主窗口，避免在菜单回调内部销毁窗口。

use std::mem::size_of;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::AttachThreadInput;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW, GetWindowThreadProcessId,
    LoadIconW, PostMessageW, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow,
    SetWindowLongPtrW, TrackPopupMenu, CS_DBLCLKS, GWLP_USERDATA, HICON, IDI_APPLICATION,
    MF_SEPARATOR, MF_STRING, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_APP, WM_COMMAND, WM_DESTROY, WM_LBUTTONDBLCLK, WM_NULL, WM_RBUTTONUP,
    WNDCLASSW,
};

/// 托盘动作对应的自定义消息（发给主面板）。
pub const MSG_TRAY_SHOW: u32 = WM_APP + 1;
pub const MSG_TRAY_QUIT: u32 = WM_APP + 3;
pub const MSG_TRAY_ABOUT: u32 = WM_APP + 4;

const WM_TRAY: u32 = WM_APP + 10;
const ID_SHOW: usize = 2001;
const ID_ABOUT: usize = 2006;
const ID_QUIT: usize = 2005;
const TRAY_UID: u32 = 1;

/// 托盘窗口句柄（由 `init` 记录，供 `shutdown` 使用）。
static TRAY_HWND: OnceLock<AtomicUsize> = OnceLock::new();
/// 主窗口句柄（托盘动作转发目标）。
static MAIN_HWND: OnceLock<AtomicUsize> = OnceLock::new();
/// Explorer 重启时 Windows 广播的注册消息 ID。
static WM_TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();
/// 菜单命令先暂存，等菜单关闭后再投递，避免在菜单回调里销毁窗口。
static PENDING_COMMAND: AtomicUsize = AtomicUsize::new(0);

struct TrayState {
    icon: HICON,
}

fn tray_hwnd() -> HWND {
    let v = TRAY_HWND
        .get()
        .map(|a| a.load(Ordering::Relaxed))
        .unwrap_or(0);
    HWND(v as *mut _)
}

fn main_hwnd() -> HWND {
    let v = MAIN_HWND
        .get()
        .map(|a| a.load(Ordering::Relaxed))
        .unwrap_or(0);
    HWND(v as *mut _)
}

/// 创建托盘窗口与图标。失败时返回 false（主窗口仍可运行）。
pub fn init(main: HWND) -> bool {
    MAIN_HWND
        .get_or_init(|| AtomicUsize::new(0))
        .store(main.0 as usize, Ordering::Relaxed);

    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("KissLightTrayWindow");
        let _ = RegisterClassW(&WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        });
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("KissLightTray"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            hinstance,
            None,
        ) else {
            return false;
        };
        if hwnd.0.is_null() {
            return false;
        }
        TRAY_HWND
            .get_or_init(|| AtomicUsize::new(0))
            .store(hwnd.0 as usize, Ordering::Relaxed);

        let initial_icon = super::panel::load_logo_icon();
        let state = Box::new(TrayState { icon: initial_icon });
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);

        let _ = WM_TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));

        add_tray_icon(hwnd)
    }
}

/// 退出前调用：注销托盘图标 → 释放图标句柄 → 销毁托盘窗口。
/// 全部在 UI 线程完成，调用返回后进程不再持有任何窗口或图标。
pub fn shutdown() {
    let hwnd = tray_hwnd();
    if hwnd.0.is_null() {
        return;
    }
    unsafe {
        // 先从任务栏注销，图标句柄才可以释放。
        delete_tray_icon(hwnd);
        let st = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
        if !st.is_null() {
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            // SAFETY: st 来自 init() 里的 Box::into_raw，且这里先把 GWLP_USERDATA
            // 置零——此后没有代码能再拿到这个指针，只回收一次。
            let state = Box::from_raw(st);
            if !state.icon.is_invalid() {
                let _ = DestroyIcon(state.icon);
            }
        }
        let _ = DestroyWindow(hwnd);
    }
}

fn post_to_main(msg: u32) {
    unsafe {
        let _ = PostMessageW(main_hwnd(), msg, WPARAM(0), LPARAM(0));
    }
}

fn add_tray_icon(hwnd: HWND) -> bool {
    unsafe {
        let st = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
        let icon = if st.is_null() {
            LoadIconW(None, IDI_APPLICATION).unwrap_or_default()
        } else {
            (*st).icon
        };
        let tip = "KissLight - 夜间模式控制器".to_string();
        let tip_wide = crate::core::util::wide(&tip);
        let mut nid = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_UID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: icon,
            ..Default::default()
        };
        let copy_len = tip_wide.len().min(127);
        nid.szTip[..copy_len].copy_from_slice(&tip_wide[..copy_len]);
        nid.szTip[copy_len] = 0;
        Shell_NotifyIconW(NIM_ADD, &nid).as_bool()
    }
}

fn delete_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        ..Default::default()
    };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY => {
            let event = lparam.0 as u32;
            if event == WM_RBUTTONUP {
                show_menu(hwnd);
            } else if event == WM_LBUTTONDBLCLK {
                post_to_main(MSG_TRAY_SHOW);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            match wparam.0 & 0xffff {
                ID_SHOW => PENDING_COMMAND.store(MSG_TRAY_SHOW as usize, Ordering::Relaxed),
                ID_ABOUT => PENDING_COMMAND.store(MSG_TRAY_ABOUT as usize, Ordering::Relaxed),
                ID_QUIT => PENDING_COMMAND.store(MSG_TRAY_QUIT as usize, Ordering::Relaxed),
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // 不在这里 PostQuitMessage：消息循环由主窗口的 WM_DESTROY 结束。
            LRESULT(0)
        }
        _ => {
            // Explorer 重启后重新添加托盘图标
            if let Some(&tb_msg) = WM_TASKBAR_CREATED.get() {
                if msg == tb_msg {
                    let _ = add_tray_icon(hwnd);
                    return LRESULT(0);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let menu = CreatePopupMenu().unwrap_or_default();
    let show = crate::core::util::wide("窗口");
    let about = crate::core::util::wide("关于");
    let quit = crate::core::util::wide("退出");
    let _ = AppendMenuW(
        menu,
        MF_STRING,
        ID_SHOW,
        windows::core::PCWSTR(show.as_ptr()),
    );
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
    let _ = AppendMenuW(
        menu,
        MF_STRING,
        ID_ABOUT,
        windows::core::PCWSTR(about.as_ptr()),
    );
    let _ = AppendMenuW(
        menu,
        MF_STRING,
        ID_QUIT,
        windows::core::PCWSTR(quit.as_ptr()),
    );
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    // TrackPopupMenu 的菜单 Z 序跟随前台窗口；托盘是后台隐藏窗口，
    // 若 SetForegroundWindow 因前台权限限制失败，菜单会被任务栏遮挡。
    if !SetForegroundWindow(hwnd).as_bool() {
        force_foreground(hwnd);
    }
    let _ = TrackPopupMenu(
        menu,
        TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
        pt.x,
        pt.y,
        0,
        hwnd,
        None,
    );
    // 收尾消息：否则点击菜单外部取消时菜单可能不消失
    let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
    let _ = DestroyMenu(menu);

    let command = PENDING_COMMAND.swap(0, Ordering::Relaxed);
    if command != 0 {
        post_to_main(command as u32);
    }
}

/// 通过 AttachThreadInput 借用前台线程的输入队列，强制切换前台。
unsafe fn force_foreground(hwnd: HWND) {
    let fg = GetForegroundWindow();
    if fg.is_invalid() {
        return;
    }
    let cur_tid = GetWindowThreadProcessId(hwnd, None);
    let fg_tid = GetWindowThreadProcessId(fg, None);
    if cur_tid == 0 || fg_tid == 0 || cur_tid == fg_tid {
        return;
    }
    if AttachThreadInput(cur_tid, fg_tid, true).as_bool() {
        let _ = SetForegroundWindow(hwnd);
        let _ = AttachThreadInput(cur_tid, fg_tid, false);
    }
}
