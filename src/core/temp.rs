//! 面板强度 ↔ 色温映射（纯函数）。
//!
//! 强度 0 = 6500K（不加滤镜），100 = 1200K（最暖），与 Windows 夜间模式的
//! 取值区间一致。区间常量在此单点定义，滑块、GammaRamp 与核心库共用。

/// 最暖色温（Windows 夜间模式下限）。
pub const MIN_KELVIN: u16 = 1200;
/// 最冷色温（等于不加滤镜）。
pub const MAX_KELVIN: u16 = 6500;
/// 面板滑块下限。
pub const MIN_INTENSITY: i32 = 0;
/// 面板滑块上限。
pub const MAX_INTENSITY: i32 = 100;

/// 强度 → 色温；越界强度按端点处理。
pub fn intensity_to_kelvin(intensity: i32) -> u16 {
    let intensity = intensity.clamp(MIN_INTENSITY, MAX_INTENSITY);
    let span = i32::from(MAX_KELVIN - MIN_KELVIN);
    (i32::from(MAX_KELVIN) - span * intensity / MAX_INTENSITY) as u16
}

/// 色温 → 强度；越界色温按端点处理。
pub fn kelvin_to_intensity(kelvin: u16) -> i32 {
    let kelvin = kelvin.clamp(MIN_KELVIN, MAX_KELVIN);
    let span = i32::from(MAX_KELVIN - MIN_KELVIN);
    (i32::from(MAX_KELVIN) - i32::from(kelvin)) * MAX_INTENSITY / span
}

#[cfg(test)]
mod tests {
    use super::*;
    use win_nightlight_lib::ColorTemperature;

    #[test]
    fn endpoints_map_to_the_night_light_range() {
        assert_eq!(intensity_to_kelvin(0), 6500);
        assert_eq!(intensity_to_kelvin(100), 1200);
        assert_eq!(kelvin_to_intensity(6500), 0);
        assert_eq!(kelvin_to_intensity(1200), 100);
    }

    #[test]
    fn mapping_is_monotonic() {
        let mut previous = intensity_to_kelvin(MIN_INTENSITY);
        for intensity in (MIN_INTENSITY + 1)..=MAX_INTENSITY {
            let kelvin = intensity_to_kelvin(intensity);
            assert!(kelvin <= previous, "强度 {intensity} 的色温不应高于前一档");
            previous = kelvin;
        }
    }

    #[test]
    fn out_of_range_input_clamps_to_endpoints() {
        assert_eq!(intensity_to_kelvin(-5), 6500);
        assert_eq!(intensity_to_kelvin(500), 1200);
        assert_eq!(kelvin_to_intensity(0), 100);
        assert_eq!(kelvin_to_intensity(9000), 0);
    }

    /// 映射区间必须与核心库接受的色温区间一致，否则滑块端点会被库拒绝。
    #[test]
    fn kelvin_range_matches_the_night_light_library() {
        assert_eq!(MAX_KELVIN, ColorTemperature::MAX.kelvin());
        assert_eq!(MIN_KELVIN, ColorTemperature::MIN.kelvin());
    }
}
