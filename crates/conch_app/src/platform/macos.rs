//! macOS environment initialisation.
//!
//! When launched from Finder, the process inherits a minimal launchd
//! environment that lacks `LANG`/`LC_ALL`, `SSH_AUTH_SOCK`, and often a
//! complete `PATH`.  This module detects and repairs those gaps.

use std::ffi::{CStr, CString};
use std::{env, str};

use libc::{LC_ALL, LC_CTYPE, setlocale};
use log::debug;
use objc2::sel;
use objc2_foundation::{NSLocale, NSObjectProtocol};

const FALLBACK_LOCALE: &str = "UTF-8";

/// Entry point — called from `platform::init()`.
pub fn init() {
    set_ssh_auth_sock();
    set_locale();
}

/// Discover `SSH_AUTH_SOCK` from the launchd environment if not already set.
fn set_ssh_auth_sock() {
    if env::var("SSH_AUTH_SOCK").is_ok() {
        return;
    }

    let output = std::process::Command::new("launchctl")
        .args(["getenv", "SSH_AUTH_SOCK"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path.is_empty() {
                debug!("Discovered SSH_AUTH_SOCK from launchd: {path}");
                unsafe { env::set_var("SSH_AUTH_SOCK", &path) };
            }
        }
        _ => {
            debug!("SSH_AUTH_SOCK not available from launchd");
        }
    }
}

/// Set locale via NSLocale when the environment doesn't provide one.
///
/// This mirrors Alacritty's approach: query the system locale via NSLocale and
/// set `LC_ALL` so that child processes (and the process itself) get proper
/// UTF-8 locale settings.
fn set_locale() {
    let env_locale_c = CString::new("").unwrap();
    let env_locale_ptr = unsafe { setlocale(LC_ALL, env_locale_c.as_ptr()) };
    if !env_locale_ptr.is_null() {
        let env_locale = unsafe { CStr::from_ptr(env_locale_ptr).to_string_lossy() };

        // "C" is the default — treat it as unset.
        if env_locale != "C" {
            debug!("Using environment locale: {}", env_locale);
            return;
        }
    }

    let system_locale = system_locale();

    let system_locale_c = CString::new(system_locale.clone()).expect("nul byte in system locale");
    let lc_all = unsafe { setlocale(LC_ALL, system_locale_c.as_ptr()) };

    if lc_all.is_null() {
        debug!("Using fallback locale: {}", FALLBACK_LOCALE);

        let fallback_locale_c = CString::new(FALLBACK_LOCALE).unwrap();
        unsafe { setlocale(LC_CTYPE, fallback_locale_c.as_ptr()) };
        unsafe { env::set_var("LC_CTYPE", FALLBACK_LOCALE) };
    } else {
        debug!("Using system locale: {}", system_locale);
        unsafe { env::set_var("LC_ALL", system_locale) };
    }
}

/// Determine system locale based on language and country code via NSLocale.
fn system_locale() -> String {
    let locale = NSLocale::currentLocale();

    let is_language_code_supported: bool = locale.respondsToSelector(sel!(languageCode));
    let is_country_code_supported: bool = locale.respondsToSelector(sel!(countryCode));
    if is_language_code_supported && is_country_code_supported {
        let language_code = locale.languageCode();
        #[allow(deprecated)]
        if let Some(country_code) = locale.countryCode() {
            format!("{}_{}.UTF-8", language_code, country_code)
        } else {
            "en_US.UTF-8".into()
        }
    } else {
        locale.localeIdentifier().to_string() + ".UTF-8"
    }
}
