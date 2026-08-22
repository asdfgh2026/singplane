use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[cfg(windows)]
use std::time::Instant;

#[allow(unused_imports)]
use crate::protocol::PIPE_NAME;
use crate::protocol::{Request, Response, StartBody, PROTOCOL_VER};
use crate::token::load_token;

#[cfg_attr(not(windows), allow(dead_code))]
pub const CTL_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
pub const CTL_IO_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(windows)]
fn ctl_dial() -> Result<std::fs::File, String> {
    use std::fs::OpenOptions;
    use std::os::windows::ffi::OsStrExt;
    use std::ffi::OsStr;
    use windows::core::PCWSTR;
    use windows::Win32::System::Pipes::WaitNamedPipeW;

    let pipe_wide: Vec<u16> = OsStr::new(PIPE_NAME)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let deadline = Instant::now() + CTL_CONNECT_TIMEOUT;

    loop {
        match OpenOptions::new().read(true).write(true).open(PIPE_NAME) {
            Ok(f) => return Ok(f),
            Err(e) => {
                let last_err = format!("connect helper: {e} (service running?)");
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(format!("{last_err} (timed out)"));
                }
                let wait_ms = remaining.as_millis().min(u32::MAX as u128) as u32;
                let _ = unsafe { WaitNamedPipeW(PCWSTR(pipe_wide.as_ptr()), wait_ms) };
                if Instant::now() >= deadline {
                    return Err(format!("{last_err} (timed out)"));
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn ctl_dial() -> Result<std::fs::File, String> {
    Err("Named pipe client is only supported on Windows".to_string())
}

pub fn ctl_call(method: &str, body: Option<serde_json::Value>) -> Result<Response, String> {
    let tok = load_token().map_err(|e| format!("load token: {} (is helper installed?)", e))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let req = Request {
        version: PROTOCOL_VER,
        id: format!("{}", now),
        method: method.to_string(),
        token: tok,
        body,
    };

    let pipe = ctl_dial()?;
    let req_json = serde_json::to_string(&req).map_err(|e| format!("serialize request: {}", e))?;
    let line = write_and_read_timeout(pipe, &req_json, CTL_IO_TIMEOUT)?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err("empty response from helper".to_string());
    }

    serde_json::from_str::<Response>(trimmed).map_err(|e| format!("parse response: {}", e))
}

fn write_and_read_timeout(
    pipe: std::fs::File,
    req_json: &str,
    timeout: Duration,
) -> Result<String, String> {
    let payload = req_json.to_string();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| {
            let mut writer = pipe
                .try_clone()
                .map_err(|e| format!("clone pipe stream: {e}"))?;
            let reader = BufReader::new(pipe);
            writer
                .write_all(payload.as_bytes())
                .map_err(|e| format!("write pipe: {e}"))?;
            writer
                .write_all(b"\n")
                .map_err(|e| format!("write pipe newline: {e}"))?;
            writer.flush().map_err(|e| format!("flush pipe: {e}"))?;
            read_line_with_timeout(reader, timeout)
        })();
        let _ = tx.send(result);
    });
    match rx.recv_timeout(timeout) {
        Ok(r) => r,
        Err(_) => Err("helper timed out".to_string()),
    }
}

pub(crate) fn read_line_with_timeout<R: BufRead + Send + 'static>(
    mut reader: R,
    timeout: Duration,
) -> Result<String, String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let res = reader.read_line(&mut line).map(|_| line);
        let _ = tx.send(res);
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(line)) => Ok(line),
        Ok(Err(e)) => Err(format!("read response: {e}")),
        Err(_) => Err("read helper: timed out".to_string()),
    }
}

pub fn run_ctl(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("usage: singpanel-helper ctl <ping|start|stop|status> ...");
        return 2;
    }

    match args[0].as_str() {
        "ping" => match ctl_call("ping", None) {
            Ok(res) => {
                if res.ok {
                    println!("pong");
                    0
                } else {
                    eprintln!("{}", res.error.unwrap_or_else(|| "unknown error".into()));
                    1
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        },
        "status" => match ctl_call("core.status", None) {
            Ok(res) => {
                let s = serde_json::to_string(&res).unwrap_or_default();
                println!("{}", s);
                if res.ok {
                    0
                } else {
                    1
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        },
        "stop" => match ctl_call("core.stop", None) {
            Ok(res) => {
                if res.ok {
                    println!("stopped");
                    0
                } else {
                    eprintln!("{}", res.error.unwrap_or_else(|| "unknown error".into()));
                    1
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                1
            }
        },
        "start" => {
            let mut core = String::new();
            let mut config = String::new();
            let mut workdir = String::new();

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--core" => {
                        i += 1;
                        if i < args.len() {
                            core = args[i].clone();
                        }
                    }
                    "--config" => {
                        i += 1;
                        if i < args.len() {
                            config = args[i].clone();
                        }
                    }
                    "--workdir" => {
                        i += 1;
                        if i < args.len() {
                            workdir = args[i].clone();
                        }
                    }
                    _ => {}
                }
                i += 1;
            }

            if core.is_empty() || config.is_empty() {
                eprintln!("usage: ctl start --core PATH --config PATH [--workdir DIR]");
                return 2;
            }

            if workdir.is_empty() {
                workdir = Path::new(&config)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".into());
            }

            let body = StartBody {
                path: core,
                config: Some(config.clone()),
                args: vec!["run".into(), "-c".into(), config],
                work_dir: Some(workdir),
            };

            match ctl_call("core.start", Some(serde_json::to_value(body).unwrap())) {
                Ok(res) => {
                    if res.ok {
                        let data_str = serde_json::to_string(&res.data).unwrap_or_default();
                        println!("{}", data_str);
                        0
                    } else {
                        eprintln!("{}", res.error.unwrap_or_else(|| "unknown error".into()));
                        1
                    }
                }
                Err(e) => {
                    eprintln!("{}", e);
                    1
                }
            }
        }
        _ => {
            eprintln!("unknown ctl command: {}", args[0]);
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor, Read};
    use std::time::Duration;

    struct NeverRead;

    impl Read for NeverRead {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        }
    }

    #[test]
    fn read_line_times_out() {
        let err = read_line_with_timeout(BufReader::new(NeverRead), Duration::from_millis(80))
            .expect_err("blocked helper must time out");
        assert!(
            err.to_ascii_lowercase().contains("time"),
            "expected timeout error, got {err}"
        );
    }

    #[test]
    fn read_line_returns_available_line() {
        let line = read_line_with_timeout(BufReader::new(Cursor::new(b"pong\n")), Duration::from_secs(1))
            .unwrap();
        assert_eq!(line.trim(), "pong");
    }
}
