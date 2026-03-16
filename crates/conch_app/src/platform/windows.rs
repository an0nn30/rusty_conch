//! Windows environment initialisation and native window integration.
//!
//! Windows processes inherit the full system environment by default, and the
//! SSH agent uses a named pipe rather than `SSH_AUTH_SOCK`.  This module
//! provides a hook for any future fixups plus dark title-bar support.

use std::ffi::c_void;

/// `DwmSetWindowAttribute` from dwmapi.dll.
#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: isize,
        dw_attribute: u32,
        pv_attribute: *const c_void,
        cb_attribute: u32,
    ) -> i32;
}

/// `EnumWindows` and `GetWindowThreadProcessId` from user32.dll.
#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(callback: unsafe extern "system" fn(isize, isize) -> i32, lparam: isize) -> i32;
    fn GetWindowThreadProcessId(hwnd: isize, lpdw_process_id: *mut u32) -> u32;
}

/// <https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute>
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

/// Entry point — called from `platform::init()`.
pub(crate) fn init() {
    // Nothing to patch up for now.
}

/// Set the dark/light title-bar attribute on all windows owned by this process.
///
/// This calls `DwmSetWindowAttribute` with `DWMWA_USE_IMMERSIVE_DARK_MODE`
/// which is required on Windows 10 1809+ and Windows 11 to get a dark
/// caption bar / title bar that matches the app's dark theme.
pub(crate) fn set_dark_title_bar(dark: bool) {
    let value: u32 = if dark { 1 } else { 0 };
    let pid = std::process::id();

    unsafe extern "system" fn enum_callback(hwnd: isize, lparam: isize) -> i32 {
        // lparam encodes both the target PID and the dark-mode value.
        let target_pid = (lparam >> 32) as u32;
        let value = (lparam & 0xFFFF_FFFF) as u32;

        let mut window_pid: u32 = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut window_pid) };
        if window_pid == target_pid {
            unsafe {
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_USE_IMMERSIVE_DARK_MODE,
                    &value as *const u32 as *const c_void,
                    std::mem::size_of::<u32>() as u32,
                );
            }
        }
        1 // continue enumeration
    }

    let lparam = ((pid as isize) << 32) | (value as isize);
    unsafe {
        EnumWindows(enum_callback, lparam);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn lparam_encoding_round_trips() {
        let pid: u32 = 12345;
        let value: u32 = 1;
        let lparam = ((pid as isize) << 32) | (value as isize);

        let decoded_pid = (lparam >> 32) as u32;
        let decoded_value = (lparam & 0xFFFF_FFFF) as u32;

        assert_eq!(decoded_pid, pid, "PID should round-trip through lparam");
        assert_eq!(decoded_value, value, "Value should round-trip through lparam");
    }

    #[test]
    fn lparam_encoding_dark_false() {
        let pid: u32 = 99999;
        let value: u32 = 0;
        let lparam = ((pid as isize) << 32) | (value as isize);

        let decoded_pid = (lparam >> 32) as u32;
        let decoded_value = (lparam & 0xFFFF_FFFF) as u32;

        assert_eq!(decoded_pid, pid);
        assert_eq!(decoded_value, 0, "dark=false should encode as 0");
    }
}
