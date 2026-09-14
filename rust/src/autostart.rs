//! Startup-folder shortcut ("Caffeine Desktop.lnk") management.
//!
//! The in-app toggle manages the very same shortcut the installer's
//! "Windows 시작 시 자동 실행" task creates ({userstartup}\Caffeine Desktop.lnk),
//! so both entry points converge on a single startup entry. It always
//! launches with `--background`: boot-time starts must land in the tray,
//! not open the settings window over the login session.

use std::path::{Path, PathBuf};

use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, BOOL};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, IPersistFile,
};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

use crate::win32::wide_null;

pub const SHORTCUT_NAME: &str = "Caffeine Desktop.lnk";
pub const BACKGROUND_ARG: &str = "--background";

/// Full path of the per-user startup shortcut (Inno's {userstartup}).
pub fn shortcut_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
        .join(SHORTCUT_NAME)
}

/// Whether the startup shortcut exists (created by the installer task
/// or by the in-app toggle — same file).
pub fn is_enabled() -> bool {
    shortcut_path().exists()
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    let path = shortcut_path();
    if on {
        let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
        write_shortcut(&path, &exe, BACKGROUND_ARG)
    } else {
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            // Already gone: the desired state is reached either way.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("remove {}: {e}", path.display())),
        }
    }
}

/// Create (or overwrite) `lnk` -> `exe` with `arguments`.
pub fn write_shortcut(lnk: &Path, exe: &Path, arguments: &str) -> Result<(), String> {
    unsafe {
        // COM is initialized per call and torn down afterwards. The thread
        // may already belong to a different apartment (RPC_E_CHANGED_MODE):
        // then COM is usable as-is and must not be uninitialized here.
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_err() && hr != RPC_E_CHANGED_MODE {
            return Err(format!("CoInitializeEx: {}", hr.0));
        }
        let uninit = hr.is_ok();

        let result = (|| -> windows::core::Result<()> {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            let target = wide_null(&exe.to_string_lossy());
            link.SetPath(PCWSTR::from_raw(target.as_ptr()))?;
            let args = wide_null(arguments);
            link.SetArguments(PCWSTR::from_raw(args.as_ptr()))?;
            let dir = wide_null(
                &exe.parent().unwrap_or(Path::new("")).to_string_lossy(),
            );
            link.SetWorkingDirectory(PCWSTR::from_raw(dir.as_ptr()))?;
            let desc = wide_null("Caffeine Desktop 자동 실행 (백그라운드)");
            link.SetDescription(PCWSTR::from_raw(desc.as_ptr()))?;
            link.SetIconLocation(PCWSTR::from_raw(target.as_ptr()), 0)?;
            let persist: IPersistFile = link.cast()?;
            let lnk_path = wide_null(&lnk.to_string_lossy());
            persist.Save(PCWSTR::from_raw(lnk_path.as_ptr()), BOOL::from(true))?;
            Ok(())
        })();

        if uninit {
            CoUninitialize();
        }
        result.map_err(|e| format!("shortcut {}: {e}", lnk.display()))
    }
}

/// Read back (target, arguments) from a saved shortcut — used by tests.
#[cfg(test)]
pub(crate) fn read_shortcut(lnk: &Path) -> Result<(String, String), String> {
    use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
    use windows::Win32::System::Com::STGM_READ;
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_err() && hr != RPC_E_CHANGED_MODE {
            return Err(format!("CoInitializeEx: {}", hr.0));
        }
        let uninit = hr.is_ok();

        let result = (|| -> windows::core::Result<(String, String)> {
            let persist: IPersistFile =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            let path = wide_null(&lnk.to_string_lossy());
            persist.Load(PCWSTR::from_raw(path.as_ptr()), STGM_READ)?;
            let link: IShellLinkW = persist.cast()?;

            let mut target = [0u16; 1024];
            let mut fd = WIN32_FIND_DATAW::default();
            link.GetPath(&mut target, &mut fd, 0)?;
            let mut args = [0u16; 256];
            link.GetArguments(&mut args)?;
            Ok((wide_to_string(&target), wide_to_string(&args)))
        })();

        if uninit {
            CoUninitialize();
        }
        result.map_err(|e| format!("read {}: {e}", lnk.display()))
    }
}

#[cfg(test)]
fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_path_is_in_user_startup_folder() {
        let p = shortcut_path().to_string_lossy().to_lowercase();
        assert!(p.ends_with(SHORTCUT_NAME.to_lowercase().as_str()), "{p}");
        assert!(
            p.contains(r"microsoft\windows\start menu\programs\startup"),
            "{p}"
        );
    }

    #[test]
    fn shortcut_round_trip_keeps_target_and_background_arg() {
        // Real COM: write a .lnk into a temp dir and read it back — this
        // exercises the exact shell-link path used for the startup entry.
        let exe = std::env::current_exe().expect("test binary path");
        let dir = std::env::temp_dir().join("caffeine-autostart-test");
        std::fs::create_dir_all(&dir).unwrap();
        let lnk = dir.join(SHORTCUT_NAME);
        write_shortcut(&lnk, &exe, BACKGROUND_ARG).expect("write shortcut");
        assert!(lnk.exists());
        let (target, args) = read_shortcut(&lnk).expect("read shortcut");
        assert!(
            target.to_lowercase().contains("caffeine"),
            "target={target}"
        );
        assert!(args.contains(BACKGROUND_ARG), "args={args}");
        std::fs::remove_file(&lnk).unwrap();
    }
}
