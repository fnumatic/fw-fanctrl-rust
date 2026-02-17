use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

mod config;
mod controller;
mod curve;
mod error;
mod hardware;
mod socket;

use config::{Config, DEFAULT_CONFIG_PATH};
use controller::FanController;
use error::Result;
use hardware::HardwareController;
use socket::{start_socket_server, ControllerHandle};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(long, default_value = "unix")]
    socket_controller: String,

    #[clap(long, value_enum, default_value = "natural")]
    output_format: OutputFormat,

    #[clap(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
enum OutputFormat {
    Natural,
    Json,
}

#[derive(Subcommand, Debug)]
enum Command {
    Run {
        #[clap(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: PathBuf,

        #[clap(short, long)]
        strategy: Option<String>,

        #[clap(short, long)]
        silent: bool,

        #[clap(long)]
        no_battery_sensors: bool,
    },
    Use {
        strategy: String,
    },
    Reset,
    Reload,
    Pause,
    Resume,
    Print {
        selection: Option<String>,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Some(Command::Run {
            config,
            strategy,
            silent,
            no_battery_sensors,
        }) => {
            run_service(config, strategy, silent, no_battery_sensors)?;
        }
        Some(Command::Use { strategy }) => {
            let result = send_command(&format!("use {}", strategy))?;
            print_result(&result, cli.output_format);
        }
        Some(Command::Reset) => {
            let result = send_command("reset")?;
            print_result(&result, cli.output_format);
        }
        Some(Command::Reload) => {
            let result = send_command("reload")?;
            print_result(&result, cli.output_format);
        }
        Some(Command::Pause) => {
            let result = send_command("pause")?;
            print_result(&result, cli.output_format);
        }
        Some(Command::Resume) => {
            let result = send_command("resume")?;
            print_result(&result, cli.output_format);
        }
        Some(Command::Print { selection }) => {
            let selection = selection.unwrap_or_else(|| "all".to_string());
            let result = send_command(&format!("print {}", selection))?;
            print_result(&result, cli.output_format);
        }
        None => {
            eprintln!("Error: No command provided. Use --help for usage information.");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn run_service(
    config_path: PathBuf,
    strategy: Option<String>,
    silent: bool,
    no_battery_sensors: bool,
) -> Result<()> {
    let config = Config::load(&config_path)?;

    let hw = HardwareController::new(no_battery_sensors)?;

    let controller = FanController::new(hw, config, strategy);

    let controller_handle: ControllerHandle = Arc::new(Mutex::new(controller));

    let server_handle = Arc::clone(&controller_handle);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            if let Err(e) = start_socket_server(server_handle).await {
                tracing::error!("Socket server error: {}", e);
            }
        });
    });

    if !silent {
        println!(
            "{:<15} {:<10} {:<10} {:<10}",
            "Strategy", "Temp", "Speed", "Active"
        );
    }

    loop {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let mut ctrl = controller_handle.lock().await;
            ctrl.step()
        });

        match result {
            Ok(temp) => {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let ctrl = controller_handle.lock().await;
                    if !silent {
                        let strategy_name = ctrl
                            .get_current_strategy()
                            .fan_speed_update_frequency
                            .to_string();
                        println!(
                            "{:<15} {:<10.1} {:<10} {:<10}",
                            strategy_name,
                            temp,
                            ctrl.get_current_speed(),
                            ctrl.is_active()
                        );
                    }
                });
            }
            Err(e) => {
                tracing::error!("Error in control loop: {}", e);
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn send_command(command: &str) -> Result<String> {
    use std::io::{Read, Write};
    use std::net::Shutdown;
    use std::os::unix::net::UnixStream;

    let socket_path = socket::COMMANDS_SOCKET_FILE_PATH;

    let mut stream = UnixStream::connect(socket_path)
        .map_err(|e| error::Error::Socket(format!("Failed to connect: {}", e)))?;

    stream
        .write_all(command.as_bytes())
        .map_err(|e| error::Error::Socket(format!("Failed to send: {}", e)))?;

    stream
        .shutdown(Shutdown::Write)
        .map_err(|e| error::Error::Socket(format!("Failed to shutdown: {}", e)))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| error::Error::Socket(format!("Failed to read: {}", e)))?;

    Ok(response)
}

fn print_result(result: &str, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", result);
        }
        OutputFormat::Natural => {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result) {
                if let Some(status) = parsed.get("status") {
                    if status == "success" {
                        if let Some(strategies) = parsed.get("strategies") {
                            println!("Strategy list:");
                            if let Some(arr) = strategies.as_array() {
                                for s in arr {
                                    println!("  - {}", s);
                                }
                            }
                        } else if let Some(speed) = parsed.get("speed") {
                            println!("Fan speed: {}%", speed);
                        } else if let Some(active) = parsed.get("active") {
                            println!("Active: {}", active);
                        } else if let Some(strategy) = parsed.get("strategy") {
                            println!("Current strategy: {}", strategy);
                        }
                    } else {
                        eprintln!("Error: {:?}", parsed.get("reason"));
                    }
                }
            } else {
                println!("{}", result);
            }
        }
    }
}
