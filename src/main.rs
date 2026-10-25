//! KissLight 入口：装配在 `app`，业务逻辑在 `core` / `ui`。
//!
//! 本文件保持极薄，初始化顺序在 `app` 里一眼看全。

#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    kisslight::app::run();
}
