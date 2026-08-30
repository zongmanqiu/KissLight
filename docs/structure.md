# KissLight 目录结构说明

> 接手项目必读。定义每个目录/文件的职责与规则。

## 一、目录总览

```
.main/
├── src/                源码
│   ├── main.rs         程序入口（精简，仅初始化+启动）
│   ├── lib.rs          库入口（集成测试用）
│   ├── core/           核心纯逻辑层（禁止 UI 代码）
│   │   ├── mod.rs      模块聚合
│   │   ├── nightlight.rs 夜间模式控制
│   │   ├── gamma.rs    Gamma 调节
│   │   ├── config.rs   配置持久化
│   │   └── util.rs     工具函数
│   └── ui/             界面层（窗口/控件/托盘）
│       ├── mod.rs      模块聚合
│       ├── window.rs   主窗口
│       ├── tray.rs     系统托盘
│       └── about.rs    关于对话框
├── build/              构建脚本（源码，非产物）
│   └── build.ps1       PowerShell 构建脚本
├── image/              ← 图片资源（logo + 二维码）
├── docs/               ← 项目文档
├── reference/          ← 参考项目说明
├── tests/              ← 集成测试
├── Cargo.toml          ← 项目清单
├── Cargo.lock          ← 依赖锁定
├── build.rs            ← 构建脚本（ICO 生成 + manifest 嵌入）
├── app.manifest        ← Windows 清单（DPI/权限）
└── README.md           ← 项目说明
```

## 二、各目录职责

### src/core/ — 纯逻辑层（Model）

**规则：禁止包含任何 UI 代码。** 这是可测试性的根基。

| 文件 | 职责 |
|------|------|
| `nightlight.rs` | 夜间模式控制：调用 win-nightlight-lib 读写注册表 |
| `gamma.rs` | Gamma 调节：SetDeviceGammaRamp 应用色温 |
| `config.rs` | 配置：JSON 持久化、默认值 |
| `util.rs` | 工具：UTF-8 ↔ UTF-16 转换 |

### src/ui/ — 界面层（View + Controller）

| 文件 | 职责 |
|------|------|
| `window.rs` | 主窗口：UI 创建、窗口过程、滑块控制、自绘边框 |
| `tray.rs` | 系统托盘：托盘图标、右键菜单 |
| `about.rs` | 关于对话框：版本、作者、仓库链接、赞助二维码 |

### build/ — 构建脚本

| 文件 | 职责 |
|------|------|
| `build.ps1` | PowerShell 构建脚本：Release/Debug 构建、清理缓存、复制产物 |

### image/ — 图片资源

| 文件 | 职责 |
|------|------|
| `KissLight.ico` | 应用图标 |
| `WeChatPay.png` | 微信赞助二维码 |
| `ALiPay.png` | 支付宝赞助二维码 |

### docs/ — 项目文档

| 文件 | 职责 |
|------|------|
| `architecture.md` | 架构总览 |
| `structure.md` | 本文档 |
| `api.md` | API 参考 |
| `testing.md` | 测试指南 |
| `changes.md` | 更新日志 |

### reference/ — 参考项目

| 文件 | 职责 |
|------|------|
| `README.md` | 参考项目说明（win-nightlight-cli） |

## 三、构建产物

产物输出到 `../.build/` 目录：

```
../.build/
├── KissLight_x64_1.0.2.exe    ← 发布版可执行文件
└── target/                     ← Cargo 构建目录
```

## 四、依赖关系

```
main.rs
  ├── core::config::load()
  └── ui::window::run()
        ├── ui::tray::run_tray_thread()
        └── ui::about::show_about_dialog()

core::nightlight
  └── win-nightlight-lib（外部 MIT 库）

core::gamma
  └── windows::Win32::Graphics::Gdi::SetDeviceGammaRamp
```
