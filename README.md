# KissLight — 轻量 Windows 夜间模式控制器

**[⬇️ 下载 1.1.0 (Gitee)](https://gitee.com/qiuzongman/kiss-light/releases/download/1.1.0/KissLight_x64_1.1.0.zip)**
 ｜ **[⬇️ 下载 1.1.0 (GitHub)](https://github.com/zongmanqiu/KissLight/releases/download/1.1.0/KissLight_x64_1.1.0.zip)**
 ｜ [Gitee主仓](https://gitee.com/qiuzongman/kiss-light)
 ｜ [GitHub镜像](https://github.com/zongmanqiu/KissLight)

纯 Rust + 原生 Win32 API 实现，托盘常驻：双击弹出面板，滑块调色温，退出还原屏幕。
单文件 exe 约 415KB，无运行库依赖、无配置文件。

## 功能

- **托盘常驻**：双击托盘图标弹出面板；右键菜单为 `窗口` / `关于` / `退出`
- **即时生效**：拖动滑块立刻改变屏幕色温（GammaRamp）；松手 400ms 后写回 Windows 夜间模式
- **跟随系统**：色温存在 Windows 夜间模式的注册表里，启动时读回
- **干净退出**：托盘图标先注销再释放，退出还原屏幕原色
- **单实例**：第二次启动只激活已有面板
- **单一产物**：约 415KB，静态链接 MSVC 运行库（不需要 VC++ Redistributable）

## 只写夜灯相关数据

| 写什么 | 位置 | 时机 |
|---|---|---|
| 色温（Windows 夜间模式） | HKCU `...\CloudStore\...\bluelightreduction.settings` | 拖滑块停止 400ms / 退出补写 |
| Gamma 范围解锁（256） | HKLM `...\ICM\GdiICMGammaRange` | 启动一次；已是 256 则不写 |
| 屏幕 GammaRamp（内存态） | 显示驱动 | 调色温；退出还原 |

**不写任何文件**（没有配置文件、日志、缓存），不写开机自启项，不碰夜灯之外的注册表键。
这条规则由 `tests/hygiene.rs` 的静态门禁钉死：源码里出现任何文件写入、
或在 `core/gamma.rs` 之外写注册表，`cargo test` 直接红。

## 构建

要求：Windows 10/11 x64、Rust ≥ 1.95；夜灯核心库以本地路径引用
`../.3rd/win-nightlight-cli-0.5.0/win-nightlight-lib`（构建时需保留该目录）。

```powershell
cd .main
build\check.ps1     # 门禁：cargo fmt --check / clippy -D warnings / cargo test
build\build.ps1     # release 构建 + 部署到 ..\.build\（含许可、版本资源与 MD5 对账）
```

产物：`../.build/KissLight.exe`、`LICENSE`、`LICENSE-win-nightlight-lib`。
编译中间文件在 `../.temp/target`，第三方源码在 `../.3rd`，都不进仓库。

## 目录

```
.main/
├── src/
│   ├── main.rs / lib.rs
│   ├── app.rs              装配：错误模式 / 单实例 / 消息循环
│   ├── core/               纯逻辑：gamma / nightlight / temp / util
│   └── ui/                 界面：panel / tray / about
├── tests/                  smoke / hygiene（写入白名单等）/ docs_consistency
├── assets/                 图标与赞助二维码
├── build/                  build.ps1 / check.ps1
├── licenses/               第三方许可原文
└── README.md / AGENTS.md / HISTORY.md / THIRD-PARTY-NOTICES.md
```

分层规则：`core ← ui ← app`。`core/` 不引用 `windows` crate 与界面代码（可单测）；
托盘、面板与消息循环同属一个 UI 线程。

## 发版检查（约 3 分钟）

1. `build\check.ps1` 全绿；`build\build.ps1` 的版本资源与 MD5 对账通过
2. 托盘右键三项都在；`关于` 里 Gitee / GitHub 两个链接可打开；`窗口` 弹出面板
3. 拖滑块屏幕即时变色；退出后颜色还原、图标消失
4. **睡眠 → 唤醒 → 托盘退出**：不应出现任何 “Unknown Hard Error” 对话框
   （若出现，查 System 日志 `Application Popup` Id=26 与 `Kernel-Power` 130/131 的时间关系）
5. **不写垃圾数据**：`%APPDATA%` 无新增文件/目录，`HKCU\...\Run` 无新增项

## 已知限制

- 唤醒后 GPU 会重置 gamma，需要再动一下滑块（或重开面板）才恢复色温；未做自动重贴
- 面板与关于框是固定像素布局，高 DPI 下不缩放
- 面板只调色温，不替你开关夜间模式（开关交给 Windows 自己的日程/手动）

## License

AGPL-3.0-only，完整许可见 [LICENSE](LICENSE)。第三方组件与许可见
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)。
