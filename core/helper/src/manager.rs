use std::fs::OpenOptions;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use log::{error, info};
use crate::protocol::StatusData;

pub const CORE_LOG_FILE_NAME: &str = "sing-box.core.log";

#[derive(Default)]
struct CoreManagerState {
    child: Option<Child>,
    pid: Option<u32>,
    running: bool,
}

#[derive(Clone, Default)]
pub struct CoreManager {
    state: Arc<Mutex<CoreManagerState>>,
}

impl CoreManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CoreManagerState::default())),
        }
    }

    pub fn start(&self, path: &Path, args: &[String], work_dir: &Path) -> Result<(), String> {
        let mut st = self.state.lock().unwrap();
        if st.running {
            let _ = Self::kill_locked(&mut st);
        }

        let log_path = if work_dir.as_os_str().is_empty() {
            std::env::temp_dir().join(CORE_LOG_FILE_NAME)
        } else {
            work_dir.join(CORE_LOG_FILE_NAME)
        };

        let file_out = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
            .map_err(|e| format!("open core log: {}", e))?;

        let file_err = file_out
            .try_clone()
            .map_err(|e| format!("clone log file handle: {}", e))?;

        let mut cmd = Command::new(path);
        cmd.args(args);
        if !work_dir.as_os_str().is_empty() {
            cmd.current_dir(work_dir);
        }
        cmd.stdout(Stdio::from(file_out));
        cmd.stderr(Stdio::from(file_err));

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let child = cmd.spawn().map_err(|e| format!("start core: {}", e))?;
        let pid = child.id();
        st.pid = Some(pid);
        st.running = true;

        info!("core started pid={} path={:?} log={:?}", pid, path, log_path);

        let state_clone = Arc::clone(&self.state);
        // Note: We don't move `child` into thread directly if we want `try_wait` in `status()`.
        // Instead, we store `child` in state and monitor via background thread or try_wait.
        st.child = Some(child);

        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let mut st = state_clone.lock().unwrap();
                if let Some(ref mut child) = st.child {
                    match child.try_wait() {
                        Ok(Some(exit_status)) => {
                            info!("core exited pid={} status={:?}", pid, exit_status);
                            st.running = false;
                            st.child = None;
                            st.pid = None;
                            break;
                        }
                        Ok(None) => {
                            // still running
                        }
                        Err(e) => {
                            error!("core try_wait error pid={}: {}", pid, e);
                            st.running = false;
                            st.child = None;
                            st.pid = None;
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
        });

        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut st = self.state.lock().unwrap();
        Self::kill_locked(&mut st)
    }

    fn kill_locked(st: &mut CoreManagerState) -> Result<(), String> {
        if !st.running {
            st.child = None;
            st.pid = None;
            return Ok(());
        }

        let pid = st.pid;
        if let Some(pid_val) = pid {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                let _ = Command::new("taskkill")
                    .args(["/PID", &pid_val.to_string(), "/T", "/F"])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
            }
            #[cfg(not(windows))]
            {
                let _ = pid_val;
            }
        }

        if let Some(mut child) = st.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        st.running = false;
        st.pid = None;
        info!("core stopped pid={:?}", pid);
        Ok(())
    }

    pub fn status(&self) -> StatusData {
        let mut st = self.state.lock().unwrap();
        if st.running {
            if let Some(ref mut child) = st.child {
                match child.try_wait() {
                    Ok(None) => return StatusData {
                        running: true,
                        pid: st.pid,
                    },
                    Ok(Some(_)) | Err(_) => {
                        st.running = false;
                        st.child = None;
                        st.pid = None;
                    }
                }
            }
        }
        StatusData {
            running: false,
            pid: None,
        }
    }
}
