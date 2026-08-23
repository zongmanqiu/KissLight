//! GammaRamp 实时色温应用模块
//! 用于在写注册表后立即呈现色温效果，避免 CloudStore 应用延迟

use std::sync::Mutex;

// ---------------------------------------------------------------------------
// 外部函数声明
// ---------------------------------------------------------------------------
extern "system" {
    fn GetDC(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn ReleaseDC(hwnd: *mut std::ffi::c_void, hdc: *mut std::ffi::c_void) -> i32;
    fn SetDeviceGammaRamp(hdc: *mut std::ffi::c_void, lpRamp: *const std::ffi::c_void) -> i32;
    fn GetDeviceGammaRamp(hdc: *mut std::ffi::c_void, lpRamp: *mut std::ffi::c_void) -> i32;
}

// ---------------------------------------------------------------------------
// 保存的原始 GammaRamp（程序启动时读取）
// ---------------------------------------------------------------------------
static SAVED_RAMP: Mutex<Option<[u16; 768]>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// 保存当前 GammaRamp 作为原始值
// ---------------------------------------------------------------------------
pub fn save_original() {
    unsafe {
        let mut ramp = [0u16; 768];
        let hdc = GetDC(std::ptr::null_mut());
        if !hdc.is_null() {
            if GetDeviceGammaRamp(hdc, ramp.as_mut_ptr() as *mut std::ffi::c_void) != 0 {
                if let Ok(mut guard) = SAVED_RAMP.lock() {
                    *guard = Some(ramp);
                }
            }
            ReleaseDC(std::ptr::null_mut(), hdc);
        }
    }
}

// ---------------------------------------------------------------------------
// 恢复原始 GammaRamp
// ---------------------------------------------------------------------------
pub fn restore() {
    unsafe {
        let ramp = if let Ok(guard) = SAVED_RAMP.lock() {
            guard.clone()
        } else {
            None
        };

        if let Some(ramp) = ramp {
            let hdc = GetDC(std::ptr::null_mut());
            if !hdc.is_null() {
                SetDeviceGammaRamp(hdc, ramp.as_ptr() as *const std::ffi::c_void);
                ReleaseDC(std::ptr::null_mut(), hdc);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 解锁 GammaRamp 完整范围
// Windows 默认限制第三方 gamma ramp 每个通道白点不低于 ~50%，
// 通过注册表 GdiICMGammaRange = 256 解锁完整范围（与 LightBulb/redshift 一致）
// ---------------------------------------------------------------------------
pub fn unlock_gamma_range() -> bool {
    unsafe {
        extern "system" {
            fn RegOpenKeyExW(
                hkey: usize,
                subkey: *const u16,
                options: u32,
                desired: u32,
                result: *mut usize,
            ) -> i32;
            fn RegSetValueExW(
                hkey: usize,
                name: *const u16,
                reserved: u32,
                typ: u32,
                data: *const u8,
                size: u32,
            ) -> i32;
            fn RegCloseKey(hkey: usize) -> i32;
        }

        const HKEY_LOCAL_MACHINE: usize = 0x80000002;
        const KEY_SET_VALUE: u32 = 0x0002;
        const REG_DWORD: u32 = 4;
        const ERROR_SUCCESS: i32 = 0;

        let subkey: Vec<u16> = "Software\\Microsoft\\Windows NT\\CurrentVersion\\ICM\0"
            .encode_utf16()
            .collect();
        let name: Vec<u16> = "GdiICMGammaRange\0".encode_utf16().collect();
        let value: u32 = 256u32.to_le(); // 解锁完整范围

        let mut key: usize = 0;
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut key,
        ) != ERROR_SUCCESS
        {
            return false;
        }

        let result = RegSetValueExW(
            key,
            name.as_ptr(),
            0,
            REG_DWORD,
            (&value as *const u32) as *const u8,
            4,
        );
        RegCloseKey(key);
        result == ERROR_SUCCESS
    }
}

// ---------------------------------------------------------------------------
// Tanner Helland 色温转 RGB 乘数
// ---------------------------------------------------------------------------
fn get_red(temperature: f64) -> f64 {
    if temperature > 6600.0 {
        (329.698727446 * (temperature / 100.0 - 60.0).powf(-0.1332047592) / 255.0)
            .clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn get_green(temperature: f64) -> f64 {
    if temperature > 6600.0 {
        (288.1221695283 * (temperature / 100.0 - 60.0).powf(-0.0755148492) / 255.0)
            .clamp(0.0, 1.0)
    } else {
        ((99.4708025861 * (temperature / 100.0).ln() - 161.1195681661) / 255.0).clamp(0.0, 1.0)
    }
}

fn get_blue(temperature: f64) -> f64 {
    if temperature >= 6600.0 {
        1.0
    } else if temperature <= 1900.0 {
        0.0
    } else {
        ((138.5177312231 * (temperature / 100.0 - 10.0).ln() - 305.0447927307) / 255.0)
            .clamp(0.0, 1.0)
    }
}

// ---------------------------------------------------------------------------
// 立即应用色温（实时 GammaRamp）
// ---------------------------------------------------------------------------
pub fn apply_temp(temperature: u16) -> bool {
    unsafe {
        let temp = temperature.clamp(1200, 6500) as f64;
        let r = get_red(temp) as f32;
        let g = get_green(temp) as f32;
        let b = get_blue(temp) as f32;

        let hdc = GetDC(std::ptr::null_mut());
        if hdc.is_null() {
            return false;
        }

        let mut ramp = [0u16; 768];
        for i in 0..256 {
            let value = (i as f32 / 255.0 * 65535.0) as u16;
            ramp[i] = ((value as f32 * r) as u32).min(65535) as u16;
            ramp[256 + i] = ((value as f32 * g) as u32).min(65535) as u16;
            ramp[512 + i] = ((value as f32 * b) as u32).min(65535) as u16;
        }

        let result = SetDeviceGammaRamp(hdc, ramp.as_ptr() as *const std::ffi::c_void);
        ReleaseDC(std::ptr::null_mut(), hdc);
        result != 0
    }
}