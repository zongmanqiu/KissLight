//! 夜间模式控制模块
//! 使用 win-nightlight-lib 0.5 读写 Windows 夜间模式的 CloudStore 数据
//! （Bond CompactBinary 载荷，库负责时间戳单调递增，避免 explorer 忽略写入）。
//!
//! 库要求整个进程只保留一个 `Nightlight` 句柄并跨线程共享，
//! 否则各句柄各自记录时间戳，写入可能倒退。

use anyhow::Result;
use win_nightlight_lib::{ColorTemperature, Nightlight};

// ---------------------------------------------------------------------------
// 夜间模式管理器
// ---------------------------------------------------------------------------
pub struct NightlightController {
    nightlight: Nightlight,
}

impl NightlightController {
    /// 创建新的控制器
    pub fn new() -> Self {
        Self {
            nightlight: Nightlight::new(),
        }
    }

    /// 获取当前色温；未设置或存储值超出 1200-6500K 时返回 None
    pub fn color_temperature(&self) -> Result<Option<u16>> {
        let settings = self.nightlight.read_settings()?;
        Ok(settings
            .color_temperature()
            .and_then(|kelvin| u16::try_from(kelvin).ok())
            .and_then(|kelvin| ColorTemperature::new(kelvin).ok())
            .map(ColorTemperature::kelvin))
    }

    /// 设置色温（写入注册表，Windows 夜间模式随后按此值生效）
    pub fn set_color_temperature(&self, temperature: u16) -> Result<()> {
        let kelvin = temperature.clamp(
            ColorTemperature::MIN.kelvin(),
            ColorTemperature::MAX.kelvin(),
        );
        self.nightlight
            .set_color_temperature(ColorTemperature::new(kelvin)?)?;
        Ok(())
    }

    /// 检查夜间模式是否启用
    pub fn is_enabled(&self) -> Result<bool> {
        Ok(self.nightlight.read_state()?.is_active())
    }

    /// 启用夜间模式
    pub fn enable(&self) -> Result<()> {
        self.nightlight.set_active(true)?;
        Ok(())
    }

    /// 禁用夜间模式
    pub fn disable(&self) -> Result<()> {
        self.nightlight.set_active(false)?;
        Ok(())
    }

    /// 切换夜间模式
    pub fn toggle(&self) -> Result<bool> {
        Ok(self.nightlight.toggle()?)
    }
}

impl Default for NightlightController {
    fn default() -> Self {
        Self::new()
    }
}
