#[allow(unused_imports)]
use log::{error, info};
#[allow(unused_imports)]
use std::ffi::OsString;
#[allow(unused_imports)]
use std::sync::mpsc;
#[allow(unused_imports)]
use std::time::Duration;

#[allow(unused_imports)]
use crate::manager::CoreManager;
#[allow(unused_imports)]
use crate::pipe_server::serve_pipe;
#[allow(unused_imports)]
use crate::protocol::{SERVICE_DESC, SERVICE_DISPLAY, SERVICE_NAME};
#[allow(unused_imports)]
use crate::token::{
    generate_and_save_token, load_token, restrict_existing_token_acl, save_allowed_roots,
    save_owner_sid,
};

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn should_restart_after_policy_write(was_running: bool) -> bool {
    was_running
}

#[cfg(windows)]
pub fn run_service() -> Result<(), String> {
    use windows_service::service_dispatcher;

    let _ = load_token().map_err(|e| format!("load token: {}", e))?;
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
        .map_err(|e| format!("service dispatcher: {}", e))
}

#[cfg(windows)]
extern "system" fn ffi_service_main(_arguments: u32, _raw_arguments: *mut *mut u16) {
    if let Err(e) = run_service_internal() {
        error!("service main failed: {}", e);
    }
}

#[cfg(windows)]
fn run_service_internal() -> Result<(), String> {
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

    let tok = load_token().map_err(|e| format!("load token: {}", e))?;
    let mgr = CoreManager::new();
    let mgr_stop = mgr.clone();

    let (stop_tx, stop_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = event_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .map_err(|e| format!("register service handler: {}", e))?;

    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::NO_ERROR,
        checkpoint: 0,
        wait_hint: Duration::from_secs(2),
        process_id: None,
    });

    let tok_clone = tok.clone();
    let mgr_clone = mgr.clone();
    let (pipe_done_tx, pipe_done_rx) = mpsc::channel();
    std::thread::spawn(move || {
        if let Err(e) = serve_pipe(mgr_clone, tok_clone, stop_rx) {
            error!("pipe server stopped: {}", e);
        }
        let _ = pipe_done_tx.send(());
    });

    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::NO_ERROR,
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    });

    // Wait for stop or shutdown signal
    let _ = event_rx.recv();

    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StopPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::NO_ERROR,
        checkpoint: 0,
        wait_hint: Duration::from_secs(5),
        process_id: None,
    });

    let _ = mgr_stop.stop();
    let _ = stop_tx.send(());
    // Wait for ConnectNamedPipe to unwind so SCM stop/reinstall does not leave a ghost.
    let _ = pipe_done_rx.recv_timeout(Duration::from_secs(4));

    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::NO_ERROR,
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    });

    Ok(())
}

#[cfg(windows)]
pub fn install_service() -> Result<(), String> {
    use windows_service::service::{
        ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType,
    };
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    let exe = std::env::current_exe()
        .map_err(|e| format!("current exe: {}", e))?
        .canonicalize()
        .map_err(|e| format!("canonicalize exe: {}", e))?;

    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .map_err(|e| format!("scm connect (need admin): {}", e))?;

    if let Ok(service) = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::QUERY_STATUS
            | ServiceAccess::START
            | ServiceAccess::STOP
            | ServiceAccess::CHANGE_CONFIG,
    ) {
        repair_existing_service(&service)?;
        return wait_service_running(&service);
    }

    generate_and_save_token().map_err(|e| format!("token: {e}"))?;
    save_owner_sid().map_err(|e| format!("owner: {e}"))?;
    save_allowed_roots().map_err(|e| format!("allow list: {e}"))?;

    let service_info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe,
        launch_arguments: vec![OsString::from("service")],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };

    let service = manager
        .create_service(&service_info, ServiceAccess::START)
        .map_err(|e| format!("create service: {}", e))?;

    service
        .start(&[OsString::from("service")])
        .map_err(|e| format!("start service: {}", e))?;

    wait_service_running(&service)
}

#[cfg(windows)]
fn repair_existing_service(service: &windows_service::service::Service) -> Result<(), String> {
    use windows_service::service::ServiceState;

    if load_token().is_err() {
        if let Ok(st) = service.query_status() {
            if st.current_state == ServiceState::Running {
                return Err(
                    "token missing while service is running; stop it or uninstall+install"
                        .to_string(),
                );
            }
        }
        generate_and_save_token().map_err(|e| format!("token: {e}"))?;
    } else {
        restrict_existing_token_acl().map_err(|e| format!("token acl: {e}"))?;
    }

    save_owner_sid().map_err(|e| format!("owner: {e}"))?;
    save_allowed_roots().map_err(|e| format!("allow list: {e}"))?;

    let running = service
        .query_status()
        .ok()
        .map(|st| {
            st.current_state == ServiceState::Running
                || st.current_state == ServiceState::StartPending
        })
        .unwrap_or(false);

    if should_restart_after_policy_write(running) {
        return restart_service(service);
    }

    service
        .start(&[OsString::from("service")])
        .map_err(|e| format!("start existing service: {e}"))?;
    Ok(())
}

#[cfg(windows)]
fn restart_service(service: &windows_service::service::Service) -> Result<(), String> {
    use windows_service::service::ServiceState;

    let _ = service.stop();
    for _ in 0..50 {
        if let Ok(st) = service.query_status() {
            if st.current_state == ServiceState::Stopped {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    service
        .start(&[OsString::from("service")])
        .map_err(|e| format!("restart service: {e}"))?;
    wait_service_running(service)
}

#[cfg(windows)]
fn wait_service_running(service: &windows_service::service::Service) -> Result<(), String> {
    use windows_service::service::ServiceState;

    for _ in 0..20 {
        if let Ok(st) = service.query_status() {
            if st.current_state == ServiceState::Running {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

#[cfg(windows)]
pub fn uninstall_service() -> Result<(), String> {
    use windows_service::service::{ServiceAccess, ServiceState};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|e| format!("scm connect (need admin): {}", e))?;

    let service = manager
        .open_service(
            SERVICE_NAME,
            ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
        )
        .map_err(|e| format!("service not installed: {}", e))?;

    let _ = service.stop();
    for _ in 0..30 {
        if let Ok(st) = service.query_status() {
            if st.current_state == ServiceState::Stopped {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    service.delete().map_err(|e| format!("delete service: {}", e))
}

#[cfg(windows)]
pub fn start_scm() -> Result<(), String> {
    use windows_service::service::ServiceAccess;
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|e| format!("scm connect: {}", e))?;

    let service = manager
        .open_service(SERVICE_NAME, ServiceAccess::START)
        .map_err(|e| format!("open service: {}", e))?;

    service
        .start(&[OsString::from("service")])
        .map_err(|e| format!("start service: {}", e))
}

#[cfg(windows)]
pub fn stop_scm() -> Result<(), String> {
    use windows_service::service::ServiceAccess;
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|e| format!("scm connect: {}", e))?;

    let service = manager
        .open_service(SERVICE_NAME, ServiceAccess::STOP)
        .map_err(|e| format!("open service: {}", e))?;

    let _ = service.stop();
    Ok(())
}

#[cfg(not(windows))]
pub fn run_service() -> Result<(), String> {
    Err("Windows service is only supported on Windows".to_string())
}

#[cfg(not(windows))]
pub fn install_service() -> Result<(), String> {
    Err("Windows service is only supported on Windows".to_string())
}

#[cfg(not(windows))]
pub fn uninstall_service() -> Result<(), String> {
    Err("Windows service is only supported on Windows".to_string())
}

#[cfg(not(windows))]
pub fn start_scm() -> Result<(), String> {
    Err("Windows service is only supported on Windows".to_string())
}

#[cfg(not(windows))]
pub fn stop_scm() -> Result<(), String> {
    Err("Windows service is only supported on Windows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_restarts_running_service_after_writing_allow() {
        assert!(should_restart_after_policy_write(true));
        assert!(!should_restart_after_policy_write(false));
    }
}
