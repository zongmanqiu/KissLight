//! 夜间模式控制模块
//! 使用 win-nightlight-lib 控制 Windows 夜间模式

use anyhow::Result;
use win_nightlight_lib::{NightlightManager, RegistryBackend, nightlight_settings::ScheduleMode};

// ---------------------------------------------------------------------------
// 夜间模式管理器
// ---------------------------------------------------------------------------
pub struct NightlightController {
    manager: NightlightManager<RegistryBackend>,
}

impl NightlightController {
    /// 创建新的控制器
    pub fn new() -> Self {
        Self {
            manager: NightlightManager::new(RegistryBackend),
        }
    }

    /// 获取当前色温
    pub fn get_color_temperature(&self) -> Result<u16> {
        let settings = self.manager.get_settings()?;
        Ok(settings.color_temperature)
    }

    /// 设置色温
    pub fn set_color_temperature(&self, temperature: u16) -> Result<()> {
        self.manager.set_color_temperature(temperature)?;
        Ok(())
    }

    /// 检查夜间模式是否启用
    pub fn is_enabled(&self) -> Result<bool> {
        let state = self.manager.get_state()?;
        Ok(state.is_enabled)
    }

    /// 启用夜间模式
    pub fn enable(&self) -> Result<()> {
        self.manager.enable()?;
        Ok(())
    }

    /// 禁用夜间模式
    pub fn disable(&self) -> Result<()> {
        self.manager.disable()?;
        Ok(())
    }

    /// 切换夜间模式
    pub fn toggle(&self) -> Result<bool> {
        let enabled = self.is_enabled()?;
        if enabled {
            self.disable()?;
            Ok(false)
        } else {
            self.enable()?;
            Ok(true)
        }
    }
}