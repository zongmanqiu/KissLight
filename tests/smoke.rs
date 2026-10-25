//! 冒烟测试 —— 核心模块可加载、工具函数正确。
//!
//! 纯逻辑的单测跟着模块走（`core::temp` / `core::gamma`）；这里只做能整体加载的冒烟。
//! 文档与卫生门禁见 tests/docs_consistency.rs 与 tests/hygiene.rs。

use kisslight::core::util;

/// 核心模块在编译期链接进来、可被调用。
#[test]
fn smoke_core_modules_load() {
    let _ = util::wide("hello");
    let _ = util::wide_to_string(&[0x0068, 0x0065, 0x006C, 0x006C, 0x006F, 0x0000]);
    assert_eq!(kisslight::core::temp::intensity_to_kelvin(100), 1200);
}

/// `wide` 以 NUL 结尾，且往返一致。
#[test]
fn smoke_util_roundtrip() {
    let original = "Hello, 世界!";
    let wide = util::wide(original);
    assert_eq!(*wide.last().unwrap(), 0);
    let back = util::wide_to_string(&wide[..wide.len() - 1]);
    assert_eq!(back, original);
}

#[test]
fn smoke_util_empty() {
    let wide = util::wide("");
    assert_eq!(wide.len(), 1);
    assert_eq!(wide[0], 0);
}

#[test]
fn smoke_util_ascii() {
    let wide = util::wide("abc");
    assert_eq!(wide.len(), 4);
    assert_eq!(wide[0], 'a' as u16);
    assert_eq!(wide[3], 0);
}

#[test]
fn smoke_util_chinese() {
    let wide = util::wide("你好");
    assert_eq!(wide.len(), 3);
    assert!(wide[0] > 0x7F);
    assert!(wide[1] > 0x7F);
}
