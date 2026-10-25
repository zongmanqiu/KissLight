# KissLight 更新历史

## v1.1.0 (2026-10-26)

### 修复

- **休眠唤醒后退出程序弹 “Unknown Hard Error”**：托盘窗口此前跑在独立线程上，退出时主线程先结束进程，托盘线程往往还停在 `Shell_NotifyIcon` 的跨进程调用里；休眠唤醒后资源管理器繁忙，这段时间被拉长，系统清理被强制终止的界面线程时弹出 hard error 对话框。现将托盘并入主窗口的消息循环，退出按「补写色温 → 注销托盘图标 → 释放图标句柄 → 销毁托盘窗口 → 销毁主窗口」顺序在同一线程完成，进程退出时不再有第二个界面线程。
- **托盘图标先于注销被销毁**：`NIM_DELETE` 与 `DestroyIcon` 顺序颠倒，资源管理器可能继续引用已释放的图标句柄；现改为先注销再释放。
- **退出时丢失最后一次色温调整**：拖动滑块后 400ms 内退出，待写入的色温会随进程一起丢掉；退出路径现在会补写。
- **抑制系统硬错误弹窗**：启动时调用 `SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX)`，底层设备/分页 I/O 故障不再以 “<exe> - 系统错误” 模态框的形式打断退出。

### 变更

- **核心升级到 win-nightlight-lib 0.5.0**（本地 `.3rd/win-nightlight-cli-0.5.0`）：读写改为 CloudStore 中的 Bond CompactBinary 载荷，只改色温字段，保留 Windows 写入的未知字段；写入时间戳单调递增，避免 explorer 忽略回退的写入。已在本机 Windows 11 26100 上验证读取与写入回读。
- 全进程共用一个 `Nightlight` 句柄（库的要求），注册表写入改在 UI 线程同步执行，退出时没有后台写线程。
- 去掉每秒一次的托盘 `NIM_MODIFY` 刷新（图标内容从未变化，纯空转）。
- 托盘右键菜单文案：显示窗口 → 窗口，完全退出 → 退出。
- **不再写任何文件**：移除 `%APPDATA%\memclean` 下的配置读写（那是同机另一个程序的路径）与 HKCU Run 自启项；本项目没有配置文件。
- **目录与代码重构**：`src/{app,core,ui}` 三层，`window.rs`→`panel.rs`，`image/`→`assets/`，删除 memclean 时期死代码（约 1500 行），clippy 零警告、rustfmt 统一格式。
- **新增门禁**：`tests/hygiene.rs`（不写文件 / 注册表写入仅 `core/gamma.rs` / `core` 纯净 / 绝对路径 / 调试打印 / 指针 `SAFETY`）与 `tests/docs_consistency.rs`（文档预算、版本号一致、许可、发布目录）。
- **静态链接 MSVC 运行库**（`+crt-static`）：不再依赖 `VCRUNTIME140.dll`；exe 360KB → 415KB（424,448 字节）。
- 移除未使用依赖 `serde` / `serde_json` / `chrono`。
- 文档只保留 4 份：`README.md`、`HISTORY.md`、`AGENTS.md`、`THIRD-PARTY-NOTICES.md`（外加 `licenses/` 原文）；文档预算由 `tests/docs_consistency.rs` 核对。
- 程序产物固定叫 **`KissLight.exe`**（不再带架构/版本后缀）。
- 关于对话框重做：Gitee / GitHub 双仓库链接（新增 https://github.com/zongmanqiu/KissLight）、
  修复链接控件 ID 缺失（此前点击全部打开同一地址且不变色）、补窗口背景刷、
  链接常态蓝色、悬停红色并在鼠标离开时复原、版本号取自 `CARGO_PKG_VERSION`、去掉致谢链接。

### 技术

- 构建产物：`KissLight.exe`（约 415KB）
- 构建需要 Rust ≥ 1.95（依赖库使用 edition 2024）

## v1.0.2 (2026-08-30)

### 修复

- **托盘右键菜单被任务栏遮挡**：菜单 Z 序跟随前台窗口，托盘是后台隐藏窗口，`SetForegroundWindow` 因前台权限限制失败时菜单会以低于任务栏的 Z 序弹出。现增加 `AttachThreadInput` 兜底强制切换前台，并在 `TrackPopupMenu` 后补发 `WM_NULL` 收尾消息，菜单点外部取消时不再残留。

## v1.0.1 (2026-08-25)

### 修复

- **资源管理器重启后托盘图标消失**：处理 Windows `TaskbarCreated` 广播消息，Explorer 重启后自动重新添加托盘图标并刷新状态。这是 Windows 托盘程序的标准做法（QQ、微信、Steam、Everything 等均采用此机制）。

## v1.0.0 (2026-08-23)

### 新增

- 系统托盘常驻，双击弹出调节面板
- 滑块实时调节色温（1200K - 6500K）
- 开机自动启用夜间模式
- 退出时恢复原始 Gamma 设置
- 单实例运行，避免重复启动
- 关于对话框：版本、作者、仓库链接、赞助二维码
- 无边框窗口，2px 黑色边框
- 窗口固定在屏幕右下角

### 技术

- 基于 Rust + 原生 Win32 API
- 使用 win-nightlight-lib 控制夜间模式
- 构建产物：KissLight_x64_1.0.0.exe（约 0.4MB）
