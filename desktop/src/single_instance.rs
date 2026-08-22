//! One GPUI process at a time. A second launch focuses the first window and exits.
//! Two instances would fight over singpanel-host, mixed/Clash ports, and TUN.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::host::app_root;

const LOCK_NAME: &str = "gpui.instance.lock";

#[derive(Debug)]
pub struct AlreadyRunning;

pub struct InstanceGuard {
    _file: File,
}

/// Take the process-wide lock. Keep the returned guard (or `hold`) for the process lifetime.
pub fn try_lock_path(path: &Path) -> Result<InstanceGuard, AlreadyRunning> {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|_| AlreadyRunning)?;
    if !exclusive_nonblock(&file) {
        return Err(AlreadyRunning);
    }
    let _ = file.set_len(0);
    let _ = writeln!(file, "{}", std::process::id());
    let _ = file.flush();
    Ok(InstanceGuard { _file: file })
}

pub fn default_lock_path() -> PathBuf {
    app_root().join("runtime").join(LOCK_NAME)
}

static HELD: std::sync::Mutex<Option<InstanceGuard>> = std::sync::Mutex::new(None);

/// Take the process-wide lock and keep it until process exit.
pub fn acquire() -> Result<(), AlreadyRunning> {
    let guard = try_lock_path(&default_lock_path())?;
    *HELD.lock().unwrap_or_else(|e| e.into_inner()) = Some(guard);
    Ok(())
}

/// Best-effort: bring the already-running SingPanel window to the front.
pub fn focus_existing() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .args(["-b", "app.singplane.gpui"])
            .status();
        let _ = std::process::Command::new("osascript")
            .args([
                "-e",
                r#"tell application "System Events" to set frontmost of the first process whose name is "SingPanel" to true"#,
            ])
            .status();
    }
    #[cfg(windows)]
    {
        focus_existing_windows();
    }
}

#[cfg(unix)]
fn exclusive_nonblock(file: &File) -> bool {
    use std::os::unix::io::AsRawFd;
    const LOCK_EX: i32 = 2;
    const LOCK_NB: i32 = 4;
    extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) == 0 }
}

#[cfg(windows)]
fn exclusive_nonblock(file: &File) -> bool {
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        event: *mut core::ffi::c_void,
    }
    const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x0000_0001;
    const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x0000_0002;
    extern "system" {
        fn LockFileEx(
            file: *mut core::ffi::c_void,
            flags: u32,
            reserved: u32,
            nbytes_low: u32,
            nbytes_high: u32,
            overlapped: *mut Overlapped,
        ) -> i32;
    }
    let mut ov = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: 0,
        offset_high: 0,
        event: core::ptr::null_mut(),
    };
    unsafe {
        LockFileEx(
            file.as_raw_handle() as *mut core::ffi::c_void,
            LOCKFILE_FAIL_IMMEDIATELY | LOCKFILE_EXCLUSIVE_LOCK,
            0,
            1,
            0,
            &mut ov,
        ) != 0
    }
}

#[cfg(windows)]
fn focus_existing_windows() {
    type Handle = *mut core::ffi::c_void;
    extern "system" {
        fn FindWindowW(class: *const u16, window: *const u16) -> Handle;
        fn ShowWindow(hwnd: Handle, cmd: i32) -> i32;
        fn SetForegroundWindow(hwnd: Handle) -> i32;
    }
    const SW_RESTORE: i32 = 9;
    let title: Vec<u16> = "SingPanel\0".encode_utf16().collect();
    unsafe {
        let hwnd = FindWindowW(core::ptr::null(), title.as_ptr());
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_lock() -> PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("singpanel-instance-{n}.lock"))
    }

    #[test]
    fn second_lock_fails_until_first_drops() {
        let path = temp_lock();
        let first = try_lock_path(&path).expect("first lock");
        assert!(try_lock_path(&path).is_err());
        drop(first);
        let second = try_lock_path(&path).expect("lock after drop");
        drop(second);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn default_lock_lives_under_runtime() {
        let p = default_lock_path();
        assert!(p.ends_with("gpui.instance.lock"));
        assert!(p.to_string_lossy().contains("runtime"));
    }
}
