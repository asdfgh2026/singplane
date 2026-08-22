mod ctl;
mod manager;
mod path_guard;
mod pipe_server;
mod protocol;
mod service;
mod token;

use std::env;
use std::process::exit;
use std::sync::mpsc;

use manager::CoreManager;
use pipe_server::serve_pipe;
use service::{install_service, run_service, start_scm, stop_scm, uninstall_service};
use token::{generate_and_save_token, load_token};

fn print_usage() {
    eprintln!(
        r#"singpanel-helper — elevated core host for SingPanel (Windows)

  install     Install & start Windows service (requires Administrator)
  uninstall   Stop & remove service (requires Administrator)
  start|stop  Control service via SCM (requires Administrator)
  service     SCM entrypoint (do not run manually)
  run         Foreground pipe server (debug)
  ctl ...     Client: ping | status | start | stop

Client examples:
  singpanel-helper ctl ping
  singpanel-helper ctl start --core C:\path\sing-box.exe --config C:\path\config.json
  singpanel-helper ctl stop"#
    );
}

fn main() {
    let _ = env_logger::try_init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        exit(2);
    }

    match args[1].as_str() {
        "install" => {
            if let Err(e) = install_service() {
                eprintln!("install failed: {}", e);
                exit(1);
            }
            println!("SingPanel Helper installed and started");
        }
        "uninstall" => {
            if let Err(e) = uninstall_service() {
                eprintln!("uninstall failed: {}", e);
                exit(1);
            }
            println!("SingPanel Helper uninstalled");
        }
        "start" => {
            if let Err(e) = start_scm() {
                eprintln!("{}", e);
                exit(1);
            }
            println!("service started");
        }
        "stop" => {
            if let Err(e) = stop_scm() {
                eprintln!("{}", e);
                exit(1);
            }
            println!("service stop requested");
        }
        "service" => {
            if let Err(e) = run_service() {
                eprintln!("service error: {}", e);
                exit(1);
            }
        }
        "run" => {
            let tok = match load_token() {
                Ok(t) => t,
                Err(_) => match generate_and_save_token() {
                    Ok(t) => {
                        println!("generated token");
                        t
                    }
                    Err(e) => {
                        eprintln!("generate token error: {}", e);
                        exit(1);
                    }
                },
            };

            let (stop_tx, stop_rx) = mpsc::channel();
            let mgr = CoreManager::new();

            ctrlc_handler(stop_tx);

            println!("console mode — pipe server");
            if let Err(e) = serve_pipe(mgr, tok, stop_rx) {
                eprintln!("serve pipe error: {}", e);
                exit(1);
            }
        }
        "ctl" => {
            let code = ctl::run_ctl(&args[2..]);
            exit(code);
        }
        "ping" => {
            let code = ctl::run_ctl(&["ping".to_string()]);
            exit(code);
        }
        _ => {
            print_usage();
            exit(2);
        }
    }
}

fn ctrlc_handler(stop_tx: mpsc::Sender<()>) {
    ctrlc::set_handler(move || {
        let _ = stop_tx.send(());
    })
    .expect("failed to set Ctrl+C handler");
}
