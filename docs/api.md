# KissLight API 参考

## core::nightlight — 夜间模式控制

```rust
pub struct NightlightController {
    manager: NightlightManager<RegistryBackend>,
}

impl NightlightController {
    /// 创建新的控制器
    pub fn new() -> Self;

    /// 获取当前色温（1200-6500）
    pub fn get_color_temperature(&self) -> Result<u16>;

    /// 设置色温（1200-6500）
    pub fn set_color_temperature(&self, temperature: u16) -> Result<()>;

    /// 获取夜间模式是否启用
    pub fn is_enabled(&self) -> Result<bool>;

    /// 启用夜间模式
    pub fn enable(&self) -> Result<()>;

    /// 禁用夜间模式
    pub fn disable(&self) -> Result<()>;
}
```

## core::gamma — Gamma 调节

```rust
/// 应用 Gamma 设置（立即生效）
pub fn apply(intensity: i32);

/// 恢复原始 Gamma（退出时调用）
pub fn restore();

/// 保存当前 Gamma（程序启动时调用）
pub fn save_original();
```

## core::config — 配置持久化

```rust
/// 加载配置，不存在则返回默认值
pub fn load() -> Config;

/// 保存配置
pub fn save(config: &Config) -> Result<()>;
```

## ui::window — 主窗口

```rust
/// 运行主窗口（阻塞）
pub fn run();

/// 显示"关于"对话框
pub fn show_about_dialog(parent: HWND);
```

## ui::tray — 系统托盘

```rust
/// 运行托盘线程（非阻塞）
pub fn run_tray_thread();

/// 向主窗口发送消息
pub fn post_to_main(msg: u32);
```

## ui::about — 关于对话框

```rust
/// 显示关于对话框
pub fn show_about_dialog(parent: HWND);
```
