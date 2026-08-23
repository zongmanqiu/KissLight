# KissLight 架构总览

> 本文档描述源码的分层架构与数据流。

- 语言：Rust (edition 2021)
- UI：原生 Win32 API（无框架依赖）
- 构建：Cargo + winres（manifest/图标嵌入）
- 平台：仅 Windows x64

---

## 一、分层与依赖方向

```
core/  ←  ui/
```

- **`core/`（纯逻辑层）**：夜间模式控制、Gamma 调节、配置持久化。禁止引用任何 UI 代码。
- **`ui/`（界面层）**：主窗口、系统托盘、关于对话框。唯一可以创建窗口 / 弹对话框的层。

### 依赖规则
- `core/` 禁止 `use crate::ui::*`
- `ui/` 可以 `use crate::core::*`
- `main.rs` 仅调用 `core::config::load()` 和 `ui::window::run()`

---

## 二、模块职责

### core/ — 纯逻辑层

| 模块 | 职责 |
|------|------|
| `nightlight.rs` | 夜间模式控制：调用 win-nightlight-lib 读写注册表 |
| `gamma.rs` | Gamma 调节：SetDeviceGammaRamp 应用色温 |
| `config.rs` | 配置持久化：JSON 读写、默认值 |
| `util.rs` | 工具函数：UTF-8/UTF-16 转换 |

### ui/ — 界面层

| 模块 | 职责 |
|------|------|
| `window.rs` | 主窗口：UI 创建、窗口过程、滑块控制 |
| `tray.rs` | 系统托盘：托盘图标、右键菜单 |
| `about.rs` | 关于对话框：版本、作者、仓库链接、赞助二维码 |

### main.rs — 入口

精简入口，仅负责：
1. 单实例互斥（Mutex）
2. 启动 UI 线程

---

## 三、数据流

```
用户双击托盘图标
    ↓
position_near_tray() 定位到屏幕右下角
    ↓
ShowWindow() 显示调节面板
    ↓
用户拖动滑块
    ↓
WM_HSCROLL → 计算色温 → SetDeviceGammaRamp()
    ↓
用户释放滑块
    ↓
WM_TIMER（节流）→ NightlightController::set_color_temperature()
    ↓
写入注册表 → 夜间模式生效
```

---

## 四、关键设计

1. **单实例**：通过命名互斥体 `Local\KissLightMainInstance` 保证
2. **无边框窗口**：WS_POPUP 风格，自绘 2px 边框
3. **节流写入**：拖动时仅应用 Gamma，停止后 100ms 才写注册表
4. **退出恢复**：WM_DESTROY 时调用 gamma::restore() 恢复原始 Gamma
