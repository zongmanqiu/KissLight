//! 应用装配：系统错误模式 → 单实例仲裁 → 面板与托盘的消息循环。
//!
//! 装配层只负责「把东西接起来」，不放窗口过程或业务逻辑。

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, HANDLE,
};
use windows::Win32::System::Diagnostics::Debug::{
    SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOOPENFILEERRORBOX,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
};

use crate::core::nightlight::NightlightController;
use crate::ui::panel;

/// 进程入口（由 `main.rs` 调用，阻塞到用户退出）。
pub fn run() {
    suppress_system_error_dialogs();

    // 互斥体句柄必须活到进程结束：guard 在 run() 返回时才 drop。
    let Some(_guard) = lock_single_instance() else {
        activate_existing_panel();
        return;
    };

    panel::run(NightlightController::new());
}

/// 持有命名互斥体；drop 时关闭句柄。
struct InstanceGuard(HANDLE);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// 取单实例锁。已有实例（或无权打开）时返回 None。
fn lock_single_instance() -> Option<InstanceGuard> {
    unsafe {
        let handle =
            CreateMutexW(None, false, w!("Local\\KissLightMainInstance")).unwrap_or_default();
        let error = GetLastError().0;
        if error == ERROR_ALREADY_EXISTS.0 || error == ERROR_ACCESS_DENIED.0 {
            None
        } else {
            Some(InstanceGuard(handle))
        }
    }
}

/// 第二次启动时把已有面板叫到前台，而不是再开一个实例。
fn activate_existing_panel() {
    unsafe {
        let found = FindWindowW(panel::class_name(), PCWSTR::null()).unwrap_or_default();
        if found.0.is_null() {
            return;
        }
        let _ = ShowWindow(found, SW_RESTORE);
        let _ = ShowWindow(found, SW_SHOW);
        let _ = SetForegroundWindow(found);
    }
}

/// 抑制系统级「硬错误」模态框（标题形如 “KissLight.exe - 系统错误”）。
///
/// 托盘程序没有前台交互：设备、分页 I/O 一类底层故障被系统转成 critical-error
/// 对话框时，用户只能看到一个无从处理的弹窗。`SetErrorMode` 让系统把这类错误
/// 返回给进程，而不是弹框。见 AGENTS.md「已知平台坑」。
fn suppress_system_error_dialogs() {
    unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX);
    }
}
