//! 关于对话框：版本、仓库链接、作者、赞助二维码。
//!
//! 链接是 `SS_NOTIFY` 静态控件 + `SetWindowSubclass`：点击打开浏览器，
//! 悬停变蓝并在 `WM_MOUSELEAVE` 时复原。版本号取自 `CARGO_PKG_VERSION`。

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HANDLE, HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateDIBSection, CreateFontIndirectW, GetDC, GetSysColorBrush, GetTextExtentPoint32W,
    InvalidateRect, ReleaseDC, SelectObject, SetTextColor, BITMAPINFO, BITMAPINFOHEADER,
    COLOR_WINDOW, DIB_RGB_COLORS, HBRUSH, HFONT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, FindWindowW, GetClientRect, GetDlgCtrlID,
    GetPropW, GetWindowRect, RegisterClassW, RemovePropW, SendMessageW, SetPropW, SetWindowPos,
    SystemParametersInfoW, CS_DBLCLKS, HMENU, HWND_TOP, NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS,
    SPI_GETWORKAREA, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_SHOWNORMAL,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CTLCOLORSTATIC,
    WM_DESTROY, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_NCHITTEST, WM_SETFONT, WM_SETICON, WNDCLASSW,
    WS_CAPTION, WS_CHILD, WS_OVERLAPPED, WS_SYSMENU, WS_VISIBLE,
};

use super::panel::KEEP_FONT;

// ---------------------------------------------------------------------------
// 常量
// ---------------------------------------------------------------------------
const IDC_GITEE_LINK: i32 = 4001;
const IDC_GITHUB_LINK: i32 = 4002;
const GITEE_URL: &str = "https://gitee.com/qiuzongman/kiss-light";
const GITHUB_URL: &str = "https://github.com/zongmanqiu/KissLight";

/// 静态文本样式（winuser.h）：居中 / 接收鼠标 / 显示位图。
const SS_CENTER: i32 = 0x01;
const SS_BITMAP: i32 = 0x0E;
const SS_NOTIFY: i32 = 0x0200;
/// `STM_SETIMAGE`：给 `SS_BITMAP` 控件换图。
const STM_SETIMAGE: u32 = 0x0172;

// 二维码 PNG（随源码放在 assets/）
const WECHAT_PNG: &[u8] = include_bytes!("../../assets/WeChatPay.png");
const ALIPAY_PNG: &[u8] = include_bytes!("../../assets/ALiPay.png");

// ---------------------------------------------------------------------------
// 链接的悬停状态与点击
// ---------------------------------------------------------------------------
/// 属性值只当布尔标记用、不解引用；`SetPropW` 接受任意 `HANDLE`。
#[allow(clippy::manual_dangling_ptr)]
fn hover_flag() -> HANDLE {
    HANDLE(1 as *mut std::ffi::c_void)
}

fn set_hover(hwnd: HWND, hovering: bool) {
    let key = w!("hover");
    unsafe {
        if hovering {
            let _ = SetPropW(hwnd, key, hover_flag());
        } else {
            let _ = RemovePropW(hwnd, key);
        }
        let _ = InvalidateRect(hwnd, None, true);
    }
}

fn is_hovering(hwnd: HWND) -> bool {
    unsafe { !GetPropW(hwnd, w!("hover")).0.is_null() }
}

fn open_url(url: &str) {
    let target = crate::core::util::wide(url);
    unsafe {
        let _ = ShellExecuteW(
            HWND::default(),
            w!("open"),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// 链接控件 ID → 仓库地址（控件 ID 缺失时这里会退化成同一个 URL，见 AGENTS.md 平台坑 5）。
fn link_url(id: i32) -> &'static str {
    if id == IDC_GITHUB_LINK {
        GITHUB_URL
    } else {
        GITEE_URL
    }
}

fn open_link(id: i32) {
    open_url(link_url(id));
}

/// 链接控件的窗口过程：接管点击与悬停，其余交给 `DefSubclassProc`。
unsafe extern "system" fn link_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    _ref_data: usize,
) -> LRESULT {
    match msg {
        // 让静态文本把鼠标消息当客户区处理
        WM_NCHITTEST => LRESULT(1 /* HTCLIENT */),
        WM_MOUSEMOVE => {
            if !is_hovering(hwnd) {
                set_hover(hwnd, true);
            }
            // 鼠标离开控件时补一条 WM_MOUSELEAVE，否则悬停色不会复原
            let mut track = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            let _ = TrackMouseEvent(&mut track);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_MOUSELEAVE => {
            set_hover(hwnd, false);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            open_link(GetDlgCtrlID(hwnd));
            LRESULT(0)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

// ---------------------------------------------------------------------------
// 窗口过程
// ---------------------------------------------------------------------------
unsafe extern "system" fn about_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CTLCOLORSTATIC => {
            let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut std::ffi::c_void);
            let control = HWND(lparam.0 as *mut std::ffi::c_void);
            let id = GetDlgCtrlID(control);
            if id == IDC_GITEE_LINK || id == IDC_GITHUB_LINK {
                let color = if is_hovering(control) {
                    COLORREF(0x0000_00FF) // 悬停：红
                } else {
                    COLORREF(0x00FF_0000) // 常态：蓝
                };
                let _ = SetTextColor(hdc, color);
            }
            let background = GetSysColorBrush(COLOR_WINDOW);
            LRESULT(background.0 as isize)
        }
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ---------------------------------------------------------------------------
// 辅助
// ---------------------------------------------------------------------------
fn control_id(id: i32) -> HMENU {
    HMENU(id as isize as *mut std::ffi::c_void)
}

/// 窗口类背景刷：系统颜色索引 +1 是 Win32 的约定（不是指针）。
#[allow(clippy::manual_dangling_ptr)]
fn system_color_brush(index: i32) -> HBRUSH {
    HBRUSH((index + 1) as isize as *mut std::ffi::c_void)
}

fn static_control(
    hwnd: HWND,
    id: i32,
    text: &str,
    (x, y, width, height): (i32, i32, i32, i32),
    style: i32,
    font: HFONT,
) -> HWND {
    let wide = crate::core::util::wide(text);
    let control = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            PCWSTR(wide.as_ptr()),
            WINDOW_STYLE(style as u32) | WS_CHILD | WS_VISIBLE,
            x,
            y,
            width,
            height,
            hwnd,
            control_id(id),
            None,
            None,
        )
        .unwrap_or_default()
    };
    unsafe {
        let _ = SendMessageW(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    }
    control
}

fn centered_static(hwnd: HWND, text: &str, rect: (i32, i32, i32, i32), font: HFONT) -> HWND {
    static_control(hwnd, 0, text, rect, SS_CENTER, font)
}

/// 链接控件：居中、可点击、悬停变色。
fn link_static(hwnd: HWND, id: i32, text: &str, rect: (i32, i32, i32, i32), font: HFONT) -> HWND {
    let control = static_control(hwnd, id, text, rect, SS_CENTER | SS_NOTIFY, font);
    unsafe {
        let _ = SetWindowSubclass(control, Some(link_proc), 1, 0);
    }
    control
}

fn text_width(hwnd: HWND, text: &str) -> i32 {
    let wide = crate::core::util::wide(text);
    let mut size = SIZE::default();
    unsafe {
        let hdc = GetDC(hwnd);
        if let Some(&font) = KEEP_FONT.get() {
            let _ = SelectObject(hdc, HFONT(font as *mut std::ffi::c_void));
        }
        let _ = GetTextExtentPoint32W(hdc, &wide, &mut size);
        let _ = ReleaseDC(hwnd, hdc);
    }
    size.cx
}

/// 从 PNG 字节创建 HBITMAP（32bpp 顶行优先 DIB）。
fn decode_png_to_bitmap(
    png_data: &[u8],
) -> Option<(windows::Win32::Graphics::Gdi::HBITMAP, i32, i32)> {
    unsafe {
        let decoder = png::Decoder::new(std::io::Cursor::new(png_data));
        let mut reader = decoder.read_info().ok()?;
        let mut buffer = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buffer).ok()?;
        let raw = &buffer[..info.buffer_size()];
        let width = info.width as i32;
        let height = info.height as i32;
        let hdc = GetDC(HWND::default());
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib = CreateDIBSection(hdc, &bitmap_info, DIB_RGB_COLORS, &mut bits, None, 0)
            .unwrap_or_default();
        if dib.is_invalid() || bits.is_null() {
            let _ = ReleaseDC(HWND::default(), hdc);
            return None;
        }
        // SAFETY: CreateDIBSection 给了 width*height 个 32bpp 像素（bits 非空、对齐要求
        // 由 DIB 保证），这里按同一个尺寸构造切片并逐像素写入。
        let destination =
            std::slice::from_raw_parts_mut(bits as *mut u32, (width * height) as usize);
        for (pixel, source) in destination.iter_mut().zip(raw.chunks_exact(4)) {
            *pixel = u32::from(source[2])
                | u32::from(source[1]) << 8
                | u32::from(source[0]) << 16
                | u32::from(source[3]) << 24;
        }
        let _ = ReleaseDC(HWND::default(), hdc);
        Some((dib, width, height))
    }
}

fn create_qr(parent: HWND, x: i32, y: i32, size: i32, png: &[u8]) {
    unsafe {
        let control = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            PCWSTR::null(),
            WINDOW_STYLE(SS_BITMAP as u32) | WS_CHILD | WS_VISIBLE,
            x,
            y,
            size,
            size,
            parent,
            HMENU::default(),
            None,
            None,
        )
        .unwrap_or_default();
        if let Some((bitmap, _, _)) = decode_png_to_bitmap(png) {
            let _ = SendMessageW(control, STM_SETIMAGE, WPARAM(0), LPARAM(bitmap.0 as isize));
        }
    }
}

// ---------------------------------------------------------------------------
// 公开接口
// ---------------------------------------------------------------------------
/// 显示关于对话框；已存在时先关掉旧的。
pub fn show_about_dialog(parent: HWND) {
    unsafe {
        let class_name = w!("KissLightAbout");
        let existing = FindWindowW(class_name, PCWSTR::null()).unwrap_or_default();
        if !existing.0.is_null() {
            let _ = DestroyWindow(existing);
        }

        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let window_class = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(about_wndproc),
            hInstance: windows::Win32::Foundation::HINSTANCE(hinstance.0),
            lpszClassName: class_name,
            hbrBackground: system_color_brush(COLOR_WINDOW.0),
            ..Default::default()
        };
        RegisterClassW(&window_class);

        let style = WINDOW_STYLE(WS_OVERLAPPED.0 | WS_CAPTION.0 | WS_SYSMENU.0);
        let qr_size = 200i32;
        let qr_gap = 40i32;
        let content_width = qr_size * 2 + qr_gap;
        let dialog_width = content_width + 40;
        let dialog_height = 500i32;

        // 先按工作区居中，窗口高度等布局完成后再收紧
        let mut work = RECT::default();
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut work as *mut _ as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let x = (work.left + work.right - dialog_width) / 2;
        let y = (work.top + work.bottom - dialog_height) / 2;

        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!("关于"),
            style | WS_VISIBLE,
            x,
            y,
            dialog_width,
            dialog_height,
            parent,
            None,
            hinstance,
            None,
        ) else {
            return;
        };

        let logo = super::panel::load_logo_icon();
        let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(0), LPARAM(logo.0 as isize));
        let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(1), LPARAM(logo.0 as isize));

        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);
        let client_width = client.right - client.left;

        let mut metrics = NONCLIENTMETRICSW {
            cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        let _ = SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            Some(&mut metrics as *mut _ as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let font = CreateFontIndirectW(&metrics.lfMessageFont);

        let mut title_face = metrics.lfMessageFont;
        title_face.lfHeight = title_face.lfHeight * 16 / 10;
        title_face.lfWeight = 700;
        let title_font = CreateFontIndirectW(&title_face);

        let mut link_face = metrics.lfMessageFont;
        link_face.lfUnderline = 1;
        let link_font = CreateFontIndirectW(&link_face);

        let margin = 20i32;
        let width = client_width - margin * 2;
        let mut cursor = 16i32;

        // 标题（版本号来自 Cargo.toml，不手写）
        let title = format!("KissLight {}", env!("CARGO_PKG_VERSION"));
        centered_static(hwnd, &title, (margin, cursor, width, 30), title_font);
        cursor += 34;

        // 仓库链接：Gitee仓库 | GitHub仓库
        let gitee_text = "Gitee仓库";
        let github_text = "GitHub仓库";
        let separator = "|";
        let gitee_width = text_width(hwnd, gitee_text) + 4;
        let github_width = text_width(hwnd, github_text) + 4;
        let separator_width = text_width(hwnd, separator);
        let gap = 12i32;
        let row_width = gitee_width + gap + separator_width + gap + github_width;
        let row_x = margin + (width - row_width) / 2;
        link_static(
            hwnd,
            IDC_GITEE_LINK,
            gitee_text,
            (row_x, cursor, gitee_width, 20),
            link_font,
        );
        centered_static(
            hwnd,
            separator,
            (row_x + gitee_width + gap, cursor, separator_width, 20),
            font,
        );
        link_static(
            hwnd,
            IDC_GITHUB_LINK,
            github_text,
            (
                row_x + gitee_width + gap + separator_width + gap,
                cursor,
                github_width,
                20,
            ),
            link_font,
        );
        cursor += 44;

        // 作者 / 邮箱 / 许可
        centered_static(
            hwnd,
            "Copyright \u{00A9} 2026 邱宗满",
            (margin, cursor, width, 20),
            font,
        );
        cursor += 22;
        centered_static(
            hwnd,
            "qiuzongman@foxmail.com",
            (margin, cursor, width, 20),
            font,
        );
        cursor += 22;
        centered_static(hwnd, "License: AGPL-3.0", (margin, cursor, width, 20), font);
        cursor += 44;

        // 赞助二维码
        centered_static(hwnd, "赞助", (margin, cursor, width, 20), font);
        cursor += 24;
        let qr_total = qr_size * 2 + qr_gap;
        let qr_start = (client_width - qr_total) / 2;

        centered_static(hwnd, "微信", (qr_start, cursor, qr_size, 18), font);
        cursor += 20;
        create_qr(hwnd, qr_start, cursor, qr_size, WECHAT_PNG);
        centered_static(
            hwnd,
            "支付宝",
            (qr_start + qr_size + qr_gap, cursor - 20, qr_size, 18),
            font,
        );
        create_qr(
            hwnd,
            qr_start + qr_size + qr_gap,
            cursor,
            qr_size,
            ALIPAY_PNG,
        );
        cursor += qr_size + 8;

        // 收紧高度并重新居中
        let mut window_rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut window_rect);
        let non_client_height =
            (window_rect.bottom - window_rect.top) - (client.bottom - client.top);
        let final_height = cursor + non_client_height;
        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            0,
            0,
            dialog_width,
            final_height,
            SWP_NOMOVE | SWP_NOZORDER,
        );
        let _ = GetWindowRect(hwnd, &mut window_rect);
        let width = window_rect.right - window_rect.left;
        let height = window_rect.bottom - window_rect.top;
        let x = (work.right - work.left - width) / 2 + work.left;
        let y = (work.bottom - work.top - height) / 2 + work.top;
        let _ = SetWindowPos(hwnd, HWND_TOP, x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_link_opens_its_own_repository() {
        assert_eq!(link_url(IDC_GITEE_LINK), GITEE_URL);
        assert_eq!(link_url(IDC_GITHUB_LINK), GITHUB_URL);
        assert_ne!(GITEE_URL, GITHUB_URL);
        assert_ne!(IDC_GITEE_LINK, IDC_GITHUB_LINK);
    }
}
