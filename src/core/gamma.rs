//! GammaRamp：把色温即时投到屏幕，并在退出时还原。
//!
//! 本模块是 KissLight **唯一的注册表写入点**（`unlock_gamma_range` 写 HKLM 的
//! `GdiICMGammaRange`，且已是目标值时不重复写）。夜灯数据由 `core::nightlight`
//! 经 win-nightlight-lib 写入 HKCU。全部写入清单见 README「只写夜灯相关数据」。

use std::sync::Mutex;

use crate::core::temp;
use crate::core::util;

extern "system" {
    fn GetDC(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn ReleaseDC(hwnd: *mut std::ffi::c_void, hdc: *mut std::ffi::c_void) -> i32;
    fn SetDeviceGammaRamp(hdc: *mut std::ffi::c_void, ramp: *const std::ffi::c_void) -> i32;
    fn GetDeviceGammaRamp(hdc: *mut std::ffi::c_void, ramp: *mut std::ffi::c_void) -> i32;
}

/// 启动时读到的原始 ramp；退出时写回（内存态，不落盘）。
static SAVED_RAMP: Mutex<Option<[u16; 768]>> = Mutex::new(None);

/// 取屏幕 DC；调用方负责 `ReleaseDC`。
fn screen_dc() -> *mut std::ffi::c_void {
    unsafe { GetDC(std::ptr::null_mut()) }
}

/// 释放屏幕 DC。
fn release_screen_dc(hdc: *mut std::ffi::c_void) {
    unsafe {
        ReleaseDC(std::ptr::null_mut(), hdc);
    }
}

/// 记录当前 GammaRamp 作为「原始值」（程序启动时调用一次）。
pub fn save_original() {
    let hdc = screen_dc();
    if hdc.is_null() {
        return;
    }
    let mut ramp = [0u16; 768];
    let captured = unsafe { GetDeviceGammaRamp(hdc, ramp.as_mut_ptr().cast()) } != 0;
    release_screen_dc(hdc);
    if captured {
        match SAVED_RAMP.lock() {
            Ok(mut saved) => *saved = Some(ramp),
            Err(poisoned) => *poisoned.into_inner() = Some(ramp),
        }
    }
}

/// 恢复原始 GammaRamp（WM_DESTROY 时调用）。
pub fn restore() {
    let saved = match SAVED_RAMP.lock() {
        Ok(guard) => *guard,
        Err(poisoned) => *poisoned.into_inner(),
    };
    let Some(ramp) = saved else {
        return;
    };
    let hdc = screen_dc();
    if hdc.is_null() {
        return;
    }
    unsafe {
        SetDeviceGammaRamp(hdc, ramp.as_ptr().cast());
    }
    release_screen_dc(hdc);
}

/// 立即应用色温；返回驱动是否接受。
pub fn apply_temp(kelvin: u16) -> bool {
    let ramp = build_ramp(kelvin);
    let hdc = screen_dc();
    if hdc.is_null() {
        return false;
    }
    let applied = unsafe { SetDeviceGammaRamp(hdc, ramp.as_ptr().cast()) } != 0;
    release_screen_dc(hdc);
    applied
}

/// 写 `HKLM\...\ICM\GdiICMGammaRange = 256`，解锁完整 GammaRamp 范围。
///
/// Windows 默认把第三方 gamma ramp 的白点限制在约 50%，不改这项就调不出夜间模式
/// 那样的深红（与 LightBulb / redshift 的做法一致）。已是 256 时直接返回，不重复写。
pub fn unlock_gamma_range() -> bool {
    const HKEY_LOCAL_MACHINE: usize = 0x8000_0002;
    const KEY_QUERY_VALUE: u32 = 0x0001;
    const KEY_SET_VALUE: u32 = 0x0002;
    const REG_DWORD: u32 = 4;
    const ERROR_SUCCESS: i32 = 0;
    const UNLOCKED: u32 = 256;

    unsafe {
        extern "system" {
            fn RegOpenKeyExW(
                hkey: usize,
                subkey: *const u16,
                options: u32,
                desired: u32,
                result: *mut usize,
            ) -> i32;
            fn RegQueryValueExW(
                hkey: usize,
                name: *const u16,
                reserved: *mut u32,
                kind: *mut u32,
                data: *mut u8,
                size: *mut u32,
            ) -> i32;
            fn RegSetValueExW(
                hkey: usize,
                name: *const u16,
                reserved: u32,
                kind: u32,
                data: *const u8,
                size: u32,
            ) -> i32;
            fn RegCloseKey(hkey: usize) -> i32;
        }

        let subkey = util::wide(r"Software\Microsoft\Windows NT\CurrentVersion\ICM");
        let value_name = util::wide("GdiICMGammaRange");

        let mut key: usize = 0;
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            0,
            KEY_QUERY_VALUE | KEY_SET_VALUE,
            &mut key,
        ) != ERROR_SUCCESS
        {
            return false;
        }

        let mut current: u32 = 0;
        let mut size = 4u32;
        let mut kind = 0u32;
        let queried = RegQueryValueExW(
            key,
            value_name.as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            (&mut current as *mut u32).cast(),
            &mut size,
        );
        if queried == ERROR_SUCCESS && kind == REG_DWORD && size == 4 && current == UNLOCKED {
            RegCloseKey(key);
            return true;
        }

        let unlocked = UNLOCKED.to_le();
        let written = RegSetValueExW(
            key,
            value_name.as_ptr(),
            0,
            REG_DWORD,
            (&unlocked as *const u32).cast(),
            4,
        );
        RegCloseKey(key);
        written == ERROR_SUCCESS
    }
}

/// 生成 3×256 的 GammaRamp（红、绿、蓝各 256 项，单调不减）。
pub(crate) fn build_ramp(kelvin: u16) -> [u16; 768] {
    let temperature = f64::from(kelvin.clamp(temp::MIN_KELVIN, temp::MAX_KELVIN));
    let (red, green, blue) = (
        red_factor(temperature),
        green_factor(temperature),
        blue_factor(temperature),
    );
    let mut ramp = [0u16; 768];
    for i in 0..256 {
        let level = (i as f32 / 255.0 * 65535.0) as u16;
        ramp[i] = scale(level, red);
        ramp[256 + i] = scale(level, green);
        ramp[512 + i] = scale(level, blue);
    }
    ramp
}

fn scale(level: u16, factor: f64) -> u16 {
    (f64::from(level) * factor).min(65535.0) as u16
}

// Tanner Helland 色温 → RGB 乘数（Windows 夜间模式惯用近似）。
fn red_factor(temperature: f64) -> f64 {
    if temperature > 6600.0 {
        (329.698_727_446 * (temperature / 100.0 - 60.0).powf(-0.133_204_759_2) / 255.0)
            .clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn green_factor(temperature: f64) -> f64 {
    if temperature > 6600.0 {
        (288.122_169_528_3 * (temperature / 100.0 - 60.0).powf(-0.075_514_849_2) / 255.0)
            .clamp(0.0, 1.0)
    } else {
        ((99.470_802_586_1 * (temperature / 100.0).ln() - 161.119_568_166_1) / 255.0)
            .clamp(0.0, 1.0)
    }
}

fn blue_factor(temperature: f64) -> f64 {
    if temperature >= 6600.0 {
        1.0
    } else if temperature <= 1900.0 {
        0.0
    } else {
        ((138.517_731_223_1 * (temperature / 100.0 - 10.0).ln() - 305.044_792_730_7) / 255.0)
            .clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(index: usize) -> u16 {
        (index as f32 / 255.0 * 65535.0) as u16
    }

    #[test]
    fn ramp_is_monotonic_per_channel() {
        let ramp = build_ramp(3400);
        for channel in 0..3 {
            for i in 1..256 {
                assert!(
                    ramp[channel * 256 + i] >= ramp[channel * 256 + i - 1],
                    "通道 {channel} 第 {i} 项低于前一项"
                );
            }
        }
    }

    #[test]
    fn coolest_temperature_stays_close_to_identity() {
        let ramp = build_ramp(temp::MAX_KELVIN);
        for i in 0..256 {
            for channel in 0..3 {
                let value = ramp[channel * 256 + i];
                assert!(value <= level(i));
                assert!(
                    u32::from(value) * 100 >= u32::from(level(i)) * 97,
                    "6500K 应在 3% 以内接近原色"
                );
            }
        }
    }

    #[test]
    fn warmest_temperature_removes_blue() {
        let ramp = build_ramp(temp::MIN_KELVIN);
        assert!(ramp[512..].iter().all(|&value| value == 0));
        assert!(ramp[255] > ramp[0]);
    }

    #[test]
    fn out_of_range_temperatures_clamp() {
        assert_eq!(build_ramp(0), build_ramp(temp::MIN_KELVIN));
        assert_eq!(build_ramp(9000), build_ramp(temp::MAX_KELVIN));
    }
}
