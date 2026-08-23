# KissLight

轻量级 Windows 夜间模式控制器，基于 Rust + 原生 Win32 API。

> 仅支持 Windows 10/11 64 位。极简设计，托盘常驻，滑块调节色温。

## 功能

- **托盘常驻**：双击托盘图标弹出调节面板
- **滑块调节**：实时调节色温（1200K - 6500K）
- **开机自启**：自动启用夜间模式，退出时恢复原始设置
- **单实例**：第二次启动自动激活已有窗口
- **极小体积**：exe 约 0.4MB，运行内存 2~5MB

## 架构

```
core/  ←  ui/       依赖方向（强制）
```

| 层 | 职责 |
|----|------|
| `core/` | 纯逻辑：夜间模式控制、Gamma 调节、配置（禁止 UI 代码） |
| `ui/` | 界面：主窗口、托盘、关于对话框 |

详见 [docs/architecture.md](docs/architecture.md)。

## 快速开始

### 环境要求

- Windows 10/11 x64
- [Rust 工具链](https://rustup.rs/)（≥ 1.70）

### 构建

```powershell
cd .main

# 方式一：构建脚本（推荐）
.\build\build.ps1            # Release
.\build\build.ps1 -Debug     # Debug
.\build\build.ps1 -Clean     # 清理缓存

# 方式二：手动
$env:CARGO_HOME = "..\.build\cargo"
$env:CARGO_TARGET_DIR = "..\.build\target"
cargo build --release
```

产物：`../.build/KissLight_x64_1.0.0.exe`（约 0.4MB）

### 测试

```powershell
cargo test                 # 运行全部测试
```

## 目录结构

```
.main/
├── src/
│   ├── main.rs          入口（单实例 + 启动 UI）
│   ├── lib.rs           库入口（集成测试用）
│   ├── core/            纯逻辑层
│   │   ├── mod.rs       模块聚合
│   │   ├── nightlight.rs 夜间模式控制
│   │   ├── gamma.rs     Gamma 调节
│   │   ├── config.rs    配置持久化
│   │   └── util.rs      工具函数
│   └── ui/              界面层
│       ├── mod.rs       模块聚合
│       ├── window.rs    主窗口
│       ├── tray.rs      系统托盘
│       └── about.rs     关于对话框
├── tests/               测试
├── build/build.ps1      构建脚本
├── image/               图片资源
├── docs/                项目文档
├── reference/           参考项目说明
├── Cargo.toml
├── build.rs             构建脚本（ICO + manifest）
└── app.manifest         Windows 清单
```

## 文档

| 文件 | 说明 |
|------|------|
| [docs/architecture.md](docs/architecture.md) | 架构总览 |
| [docs/structure.md](docs/structure.md) | 目录结构说明 |
| [docs/api.md](docs/api.md) | API 参考 |
| [docs/testing.md](docs/testing.md) | 测试指南 |
| [docs/changes.md](docs/changes.md) | 更新日志 |
| [DOCS_MANIFEST.md](DOCS_MANIFEST.md) | 文档清单 |

## 技术栈

| 组件 | 说明 |
|------|------|
| Rust | 主语言（edition 2021） |
| Win32 API | 原生 UI（via `windows` crate） |
| win-nightlight-lib | 夜间模式控制（MIT 许可证） |
| winres | Windows 资源嵌入（manifest + 图标） |

## 致谢

本项目的夜间模式控制功能基于 [kvnxiao/win-nightlight-cli](https://github.com/kvnxiao/win-nightlight-cli) 项目，采用 MIT 许可证。

## 许可证

[AGPL-3.0](LICENSE)

Copyright (C) 2026 邱宗满
