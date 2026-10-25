# KissLight 项目规则（AGENTS.md）

本文件对仓库根（`.main/`）及全部子目录生效。项目根（上一级）的 `AGENTS.md` 只写工作区路径。
KissLight 是一个小工具：**代码与文档都保持克制**，能并进现有文件的就不要新开文件。

## 一、目录边界

- 仓库只放开手写源码（`src/`）、测试（`tests/`）、构建脚本（`build.rs` + `build/`）、
  资源（`assets/`）、许可（`licenses/`）、以及 4 份文档（`README.md` / `HISTORY.md` /
  `AGENTS.md` / `THIRD-PARTY-NOTICES.md`）。
- 编译中间产物去 `../.temp/`，发布产物去 `../.build/`，第三方源码在 `../.3rd/`。
- **文档预算：只许这 4 份 markdown**（`tests/docs_consistency.rs` 会拦新增）。
  新增文档前先问「它比 README 多解决了什么」；一次性草稿放 `../.temp/`，不进仓库。

## 二、数据写入红线

KissLight **只写夜灯相关数据**：

| 目标 | 内容 | 时机 | 代码位置 |
|---|---|---|---|
| HKCU CloudStore 夜灯设置/状态值 | 色温、开关 | 拖滑块（节流 400ms）/ 启动只读 | `src/core/nightlight.rs`（经 win-nightlight-lib） |
| HKLM `...\ICM\GdiICMGammaRange` = 256 | 解锁 gamma 范围 | 启动；**已是 256 则不写** | `src/core/gamma.rs` |
| 显示 GammaRamp（内存态） | 当前色温 | 调色温 / 退出还原 | `src/core/gamma.rs` |

**禁止**：任何文件写入（本项目没有配置文件/日志/缓存）、开机自启项、夜灯之外的注册表键。
门禁：`tests/hygiene.rs`（`no_file_writes_anywhere`、`registry_writes_only_in_gamma`、
`nightlight_paths_come_from_the_library`）。

## 三、分层

```
core ← ui ← app
```

- `core/` 是纯逻辑：不得 `use windows::`、`use crate::ui`、`use crate::app`。
- `ui/` 是唯一创建窗口/托盘/对话框的层；全部窗口归 UI 线程。
- `app/` 只做装配（错误模式、单实例、启动消息循环）。
- 门禁：`tests/hygiene.rs::core_stays_pure`。

## 四、编码纪律

1. `core/` 禁止 `unwrap` / `expect` / `panic!`；返回错误而不是中止。
2. 指针类 `unsafe`（`from_raw` / `from_raw_parts` / `transmute` / `get_unchecked` /
   `copy_nonoverlapping`）必须写 `// SAFETY:`，说明越界/对齐/生命周期/别名为何成立。
3. 注释写**为什么**；区间/常量单点定义（`core/temp.rs`）；新增 >40 行逻辑带单测。
4. 加依赖先算体积与许可成本；新增写入先过第二节红线。
5. 门禁：`tests/hygiene.rs`（绝对路径、调试打印、未完成标记、本机身份指纹）。

## 五、已知平台坑（实测，别再踩）

1. **休眠唤醒后退出弹 “Unknown Hard Error”**：这是系统 hard-error 弹窗（System 日志
   `Application Popup` Id=26，紧跟 `Kernel-Power` 130/131），不是崩溃、也没有 WER 记录。
   退出必须保持单 UI 线程 + 顺序拆除：补写色温 → `NIM_DELETE` → `DestroyIcon` →
   `DestroyWindow(托盘)` → `DestroyWindow(面板)`；否则主线程会强杀仍在 `Shell_NotifyIcon`
   跨进程调用里的托盘线程。
2. `NIM_DELETE` 必须先于 `DestroyIcon`（任务栏仍持有图标句柄）。
3. `GetMessageW` 返回 `-1` 是出错，不能当「有消息」（`BOOL::as_bool()` 会误判）。
4. `PCWSTR(wide(s).as_ptr())` 会在语句结束处悬垂——先 `let` 绑定，或改用 `w!("...")` 静态串。
5. 自绘静态控件**必须给控制 ID**：ID 为 0 时链接既不会变色、点击也全落到同一个 URL。
6. 托盘每秒一次的 `NIM_MODIFY` 是纯空转，已删；不要再加周期定时器。
7. `+crt-static`：exe 约 415KB，不依赖 `VCRUNTIME140.dll`；验收可用
   `objdump -p KissLight.exe | grep "DLL Name"` 确认。
8. 单实例互斥 `Local\KissLightMainInstance`；第二次启动只激活已有面板。

## 六、交付前

1. `build\check.ps1`：`cargo fmt --check` → `clippy --all-targets -- -D warnings` → `cargo test`
2. `build\build.ps1`：release 构建 + 部署到 `../.build/`，校验 `FileVersion` 与 MD5/大小
3. 真机验收：按 `README.md` §发版检查（含睡眠唤醒退出与「不写垃圾数据」）
4. 改动落地前可快照到项目根 `../.bf/<MMDD-HHMM> <说明>/`
