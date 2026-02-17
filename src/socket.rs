use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::controller::FanController;
use crate::error::{Error, Result};

pub const SOCKET_FOLDER_PATH: &str = "/run/fw-fanctrl";
pub const COMMANDS_SOCKET_FILE_PATH: &str = "/run/fw-fanctrl/.fw-fanctrl.commands.sock";

pub type ControllerHandle = Arc<Mutex<FanController>>;

pub async fn start_socket_server(
    controller: ControllerHandle,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    let socket_path = PathBuf::from(COMMANDS_SOCKET_FILE_PATH);
    let folder_path = PathBuf::from(SOCKET_FOLDER_PATH);

    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    if !folder_path.exists() {
        std::fs::create_dir_all(&folder_path)?;
    }

    let listener = UnixListener::bind(&socket_path)
        .map_err(|e| Error::Socket(format!("Failed to bind socket: {}", e)))?;

    listener
        .set_nonblocking(true)
        .map_err(|e| Error::Socket(format!("Failed to set nonblocking: {}", e)))?;

    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o777))
        .map_err(|e| Error::Socket(format!("Failed to set socket permissions: {}", e)))?;

    tracing::info!("Socket server listening on {}", COMMANDS_SOCKET_FILE_PATH);

    let shutdown_check = Arc::clone(&shutdown);
    let accept_task: JoinHandle<Result<()>> = tokio::task::spawn_blocking(move || {
        loop {
            if shutdown_check.load(Ordering::Relaxed) {
                tracing::info!("Socket server received shutdown signal");
                break Ok(());
            }

            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    let controller = Arc::clone(&controller);
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(&mut stream, controller).await {
                            tracing::error!("Error handling connection: {}", e);
                        }
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    tracing::error!("Accept error: {}", e);
                }
            }
        }
    });

    let _ = accept_task.await.map_err(|e| {
        Error::Socket(format!("Socket accept task failed: {}", e))
    })?;

    tracing::info!("Socket server shutting down");

    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    Ok(())
}

async fn handle_connection(
    stream: &mut std::os::unix::net::UnixStream,
    controller: ControllerHandle,
) -> Result<()> {
    let mut buffer = [0u8; 4096];
    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|e| Error::Socket(format!("Failed to read from socket: {}", e)))?;

    if bytes_read == 0 {
        return Ok(());
    }

    let command = String::from_utf8_lossy(&buffer[..bytes_read]);
    let command = command.trim();

    tracing::debug!("Received command: {}", command);

    let response = process_command(command, controller).await?;

    stream
        .write_all(response.as_bytes())
        .map_err(|e| Error::Socket(format!("Failed to write to socket: {}", e)))?;

    Ok(())
}

pub async fn process_command(command: &str, controller: ControllerHandle) -> Result<String> {
    // Filter out arguments starting with -- (e.g., --output-format=JSON)
    let parts: Vec<&str> = command
        .split_whitespace()
        .filter(|s| !s.starts_with("--"))
        .collect();

    if parts.is_empty() {
        return Err(Error::Command("Empty command".into()));
    }

    let mut controller = controller.lock().await;

    match parts[0] {
        "use" => {
            if parts.len() < 2 {
                return Err(Error::Command("Usage: use <strategy>".into()));
            }
            let strategy = parts[1];
            controller.overwrite_strategy(strategy)?;
            Ok(format!(
                "{{\"status\": \"success\", \"strategy\": \"{}\"}}",
                controller.get_current_strategy_name()
            ))
        }
        "reset" => {
            controller.clear_overwritten_strategy();
            Ok(format!(
                "{{\"status\": \"success\", \"strategy\": \"{}\"}}",
                controller.get_current_strategy_name()
            ))
        }
        "reload" => {
            let config =
                crate::config::Config::load(&PathBuf::from("/etc/fw-fanctrl/config.json"))?;
            controller.reload_config(config);
            Ok("{\"status\": \"success\"}".into())
        }
        "pause" => {
            controller.pause()?;
            Ok("{\"status\": \"success\"}".into())
        }
        "resume" => {
            controller.resume()?;
            Ok("{\"status\": \"success\"}".into())
        }
        "print" => {
            let selection = parts.get(1).copied().unwrap_or("all");
            print_selection(selection, &mut controller).await
        }
        _ => Err(Error::Command(format!("Unknown command: {}", parts[0]))),
    }
}

async fn print_selection(selection: &str, controller: &mut FanController) -> Result<String> {
    match selection {
        "all" => {
            let temp = controller.get_actual_temperature()?;
            let strategy = controller.get_current_strategy();
            let moving_avg =
                controller.get_moving_average_temperature(strategy.moving_average_interval);
            let effective =
                controller.get_effective_temperature(temp, strategy.moving_average_interval);

            let response = serde_json::json!({
                "status": "success",
                "strategy": controller.get_current_strategy_name(),
                "default": !controller.is_overwritten(),
                "speed": controller.get_current_speed().to_string(),
                "temperature": temp.to_string(),
                "movingAverageTemperature": moving_avg.to_string(),
                "effectiveTemperature": effective.to_string(),
                "active": controller.is_active(),
                "configuration": controller.get_config()
            });
            Ok(serde_json::to_string(&response).map_err(|e| Error::Config(e.to_string()))?)
        }
        "active" => Ok(serde_json::json!({
            "status": "success",
            "active": controller.is_active()
        })
        .to_string()),
        "current" => Ok(serde_json::json!({
            "status": "success",
            "strategy": controller.get_current_strategy_name(),
            "default": !controller.is_overwritten()
        })
        .to_string()),
        "list" => {
            let strategies: Vec<String> = controller
                .get_config()
                .strategy_names()
                .iter()
                .map(|s| (*s).clone())
                .collect();
            Ok(serde_json::json!({
                "status": "success",
                "strategies": strategies
            })
            .to_string())
        }
        "speed" => Ok(serde_json::json!({
            "status": "success",
            "speed": controller.get_current_speed().to_string()
        })
        .to_string()),
        _ => Err(Error::Command(format!(
            "Unknown print selection: {}",
            selection
        ))),
    }
}
