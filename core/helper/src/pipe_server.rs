use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use log::error;
use crate::manager::CoreManager;
use crate::path_guard::sanitize_start;
use crate::protocol::{Request, Response, StartBody, StatusData};
#[allow(unused_imports)]
use crate::protocol::PIPE_NAME;
#[allow(unused_imports)]
use crate::token::{load_allowed_roots, pipe_security_descriptor};
#[allow(unused_imports)]
use std::sync::mpsc::Receiver;

/// Listening pipe handle that stop can close to unblock `ConnectNamedPipe`.
pub(crate) struct InterruptibleListen {
    stopped: AtomicBool,
    handle: Mutex<Option<usize>>,
}

impl InterruptibleListen {
    pub(crate) fn new() -> Self {
        Self {
            stopped: AtomicBool::new(false),
            handle: Mutex::new(None),
        }
    }

    pub(crate) fn should_exit(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    pub(crate) fn set_handle(&self, handle: usize) {
        if let Ok(mut slot) = self.handle.lock() {
            *slot = Some(handle);
        }
    }

    pub(crate) fn take_handle(&self) -> Option<usize> {
        self.handle.lock().ok().and_then(|mut slot| slot.take())
    }

    pub(crate) fn request_stop<F>(&self, close: F)
    where
        F: FnOnce(usize),
    {
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(handle) = self.take_handle() {
            close(handle);
        }
    }
}

pub fn dispatch(
    mgr: &CoreManager,
    expected_token: &str,
    req: Request,
    roots: &[PathBuf],
) -> Response {
    let id = req.id.clone();
    if req.token.is_empty() || req.token != expected_token {
        return Response::err(id, "unauthorized");
    }
    match req.method.as_str() {
        "ping" => Response::ok(id, "pong"),
        "core.start" => {
            let body: StartBody = match req.body {
                Some(b) => match serde_json::from_value(b) {
                    Ok(sb) => sb,
                    Err(e) => return Response::err(id, format!("bad body: {}", e)),
                },
                None => return Response::err(id, "missing body"),
            };
            let (core_path, args, work_dir) = match sanitize_start(&body, roots) {
                Ok(v) => v,
                Err(e) => return Response::err(id, e),
            };
            if let Err(e) = mgr.start(&core_path, &args, &work_dir) {
                return Response::err(id, e);
            }
            Response::ok(
                id,
                StatusData {
                    running: true,
                    pid: mgr.status().pid,
                },
            )
        }
        "core.stop" => {
            if let Err(e) = mgr.stop() {
                return Response::err(id, e);
            }
            Response::ok(
                id,
                StatusData {
                    running: false,
                    pid: None,
                },
            )
        }
        "core.status" => Response::ok(id, mgr.status()),
        _ => Response::err(id, format!("unknown method: {}", req.method)),
    }
}

pub fn handle_client<R: BufRead, W: Write>(
    mut reader: R,
    mut writer: W,
    mgr: &CoreManager,
    expected_token: &str,
    roots: &[PathBuf],
) {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let res = match serde_json::from_str::<Request>(trimmed) {
                    Ok(req) => dispatch(mgr, expected_token, req, roots),
                    Err(e) => Response::err("".into(), format!("invalid json: {}", e)),
                };
                if let Ok(json_str) = serde_json::to_string(&res) {
                    let _ = writer.write_all(json_str.as_bytes());
                    let _ = writer.write_all(b"\n");
                    let _ = writer.flush();
                }
            }
            Err(e) => {
                error!("read error: {}", e);
                break;
            }
        }
    }
}

#[cfg(windows)]
pub fn serve_pipe(
    mgr: CoreManager,
    expected_token: String,
    stop_rx: Receiver<()>,
) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::io::BufReader;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;
    use std::sync::Arc;
    use log::info;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, LocalFree, HLOCAL, INVALID_HANDLE_VALUE};
    use windows::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows::Win32::Security::SECURITY_ATTRIBUTES;
    use windows::Win32::Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX,
    };
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
        PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };

    const SDDL_REVISION_1: u32 = 1;

    info!("helper listening on {}", PIPE_NAME);
    let pipe_name_wide: Vec<u16> = OsStr::new(PIPE_NAME)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let sddl = pipe_security_descriptor();
    let sddl_wide: Vec<u16> = OsStr::new(&sddl)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut sec_desc = windows::Win32::Security::PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl_wide.as_ptr()),
            SDDL_REVISION_1,
            &mut sec_desc,
            None,
        )
        .map_err(|e| format!("security descriptor: {}", e))?;
    }

    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sec_desc.0,
        bInheritHandle: false.into(),
    };

    let listen = Arc::new(InterruptibleListen::new());
    let listen_stop = Arc::clone(&listen);

    std::thread::spawn(move || {
        let _ = stop_rx.recv();
        listen_stop.request_stop(|raw| {
            unsafe {
                let _ = CloseHandle(windows::Win32::Foundation::HANDLE(raw as _));
            }
            // Wake a blocking ConnectNamedPipe if CloseHandle raced with accept.
            let _ = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(PIPE_NAME);
        });
    });

    let mut first_instance = true;
    while !listen.should_exit() {
        let mut open_mode = PIPE_ACCESS_DUPLEX;
        if first_instance {
            open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }

        let pipe_handle = unsafe {
            CreateNamedPipeW(
                PCWSTR(pipe_name_wide.as_ptr()),
                open_mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                PIPE_UNLIMITED_INSTANCES,
                65536,
                65536,
                0,
                Some(&sa),
            )
        };

        first_instance = false;

        if pipe_handle == INVALID_HANDLE_VALUE {
            if listen.should_exit() {
                break;
            }
            error!("CreateNamedPipeW failed");
            std::thread::sleep(std::time::Duration::from_millis(500));
            continue;
        }

        listen.set_handle(pipe_handle.0 as usize);
        if listen.should_exit() {
            if listen.take_handle().is_some() {
                unsafe { let _ = CloseHandle(pipe_handle); }
            }
            break;
        }

        let connected = unsafe { ConnectNamedPipe(pipe_handle, None) };
        let owned = listen.take_handle();
        if listen.should_exit() {
            if owned.is_some() {
                unsafe { let _ = CloseHandle(pipe_handle); }
            }
            break;
        }

        if let Err(e) = connected {
            // ERROR_PIPE_CONNECTED (535) is ok
            if e.code().0 != -2147024361 && e.code().0 != 535 {
                if owned.is_some() {
                    unsafe { let _ = CloseHandle(pipe_handle); };
                }
                continue;
            }
        }

        let mgr_clone = mgr.clone();
        let tok_clone = expected_token.clone();
        // Reload allow-list per connection so install/upgrade writes take effect
        // without waiting for a service restart.
        let roots_clone = load_allowed_roots();
        let std_file = unsafe { std::fs::File::from_raw_handle(pipe_handle.0 as _) };
        let raw_handle = pipe_handle.0 as usize;

        std::thread::spawn(move || {
            let writer = match std_file.try_clone() {
                Ok(w) => w,
                Err(e) => {
                    error!("clone pipe handle file: {}", e);
                    unsafe {
                        let _ = DisconnectNamedPipe(windows::Win32::Foundation::HANDLE(
                            raw_handle as _,
                        ));
                    }
                    return;
                }
            };
            let reader = BufReader::new(std_file);
            handle_client(reader, writer, &mgr_clone, &tok_clone, &roots_clone);
            unsafe {
                let _ = DisconnectNamedPipe(windows::Win32::Foundation::HANDLE(raw_handle as _));
            }
        });
    }

    unsafe {
        if !sec_desc.0.is_null() {
            let _ = LocalFree(HLOCAL(sec_desc.0 as _));
        }
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn serve_pipe(
    _mgr: CoreManager,
    _expected_token: String,
    _stop_rx: Receiver<()>,
) -> Result<(), String> {
    Err("Named pipe server is only supported on Windows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_handle_client_ping_unauthorized() {
        let mgr = CoreManager::new();
        let input = b"{\"version\":1,\"id\":\"1\",\"method\":\"ping\",\"token\":\"wrong\"}\n";
        let mut output = Vec::new();
        let roots = Vec::new();
        handle_client(Cursor::new(input), &mut output, &mgr, "correct", &roots);
        let out_str = String::from_utf8(output).unwrap();
        assert!(out_str.contains("\"ok\":false"));
        assert!(out_str.contains("\"unauthorized\""));
    }

    #[test]
    fn test_handle_client_ping_authorized() {
        let mgr = CoreManager::new();
        let input = b"{\"version\":1,\"id\":\"1\",\"method\":\"ping\",\"token\":\"correct\"}\n";
        let mut output = Vec::new();
        let roots = Vec::new();
        handle_client(Cursor::new(input), &mut output, &mgr, "correct", &roots);
        let out_str = String::from_utf8(output).unwrap();
        assert!(out_str.contains("\"ok\":true"));
        assert!(out_str.contains("\"pong\""));
    }

    #[test]
    fn request_stop_closes_listen_handle() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let listen = InterruptibleListen::new();
        listen.set_handle(42);
        let closed = Arc::new(AtomicUsize::new(0));
        let closed_c = Arc::clone(&closed);
        listen.request_stop(move |h| {
            closed_c.store(h, Ordering::SeqCst);
        });
        assert!(listen.should_exit());
        assert_eq!(closed.load(Ordering::SeqCst), 42);
        assert!(listen.take_handle().is_none());
    }
}
