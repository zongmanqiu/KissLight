# 第三方许可与致谢

KissLight 以 **AGPL-3.0-only** 发布（见 `LICENSE`）。本项目衍生于下面这个开源项目，
并按许可证要求随包分发其许可原文。

## 一、win-nightlight-cli / win-nightlight-lib（核心）

| 项 | 内容 |
|---|---|
| 项目 | win-nightlight-cli（Windows 夜间模式 CLI 与库） |
| 作者 | Kevin Xiao（kvnxiao） |
| 仓库 | <https://github.com/kvnxiao/win-nightlight-cli> |
| 版本 | 0.5.0（源码位于项目根 `.3rd/win-nightlight-cli-0.5.0/`，`Cargo.toml` 以 path 依赖引用） |
| 许可 | MIT License，原文见 `licenses/win-nightlight-lib-MIT.txt` |

KissLight 使用其 `win-nightlight-lib` 读写 Windows 11 夜间模式的 CloudStore 数据
（Bond CompactBinary 载荷、时间戳单调递增、未知字段保留）。本项目的界面、GammaRamp、
托盘与构建/发布流程为自研。

发布目录 `.build/` 里同时提供 `LICENSE-win-nightlight-lib`（该 MIT 原文的副本），
`tests/docs_consistency.rs` 会核对两者逐字一致。

## 二、Rust 依赖

| crate | 用途 | 许可 |
|---|---|---|
| `windows` | Win32 API 绑定 | MIT OR Apache-2.0 |
| `png` | 解码关于对话框的二维码 | MIT OR Apache-2.0 |
| `anyhow` | 错误上下文 | MIT OR Apache-2.0 |
| `winres`、`image`、`ico`（构建期） | 生成 ICO / 写 VERSIONINFO | MIT 等宽松许可 |

完整依赖树见 `Cargo.lock`；各 crate 的许可原文随 crates.io 包分发。

## 三、运行期组件

exe **静态链接** MSVC 运行库（`-C target-feature=+crt-static`），因此不依赖
VC++ Redistributable，也不额外分发 DLL。导入表只包含系统 DLL：
kernel32 / user32 / gdi32 / shell32 / comctl32 / advapi32 / oleaut32 / ntdll。
