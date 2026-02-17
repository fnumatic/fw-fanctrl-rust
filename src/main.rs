use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use fw_fanctrl::config::{Config, DEFAULT_CONFIG_PATH};
use fw_fanctrl::controller::FanController;
use fw_fanctrl::error::Result;
use fw_fanctrl::hardware::HardwareController;
use fw_fanctrl::socket::{start_socket_server, ControllerHandle};

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

        #[clap(long)]
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
    SanityCheck {
        #[clap(long)]
        fan: bool,

        #[clap(long)]
        temp: bool,

        #[clap(long, default_value = "true")]
        all: bool,
    },
}

fn run_socket_command(cmd: &str, args: Option<&str>, format: OutputFormat) -> Result<()> {
    let full_cmd = match args {
        Some(a) => format!("{} {}", cmd, a),
        None => cmd.to_string(),
    };
    let result = send_command(&full_cmd)?;
    print_result(&result, format);
    Ok(())
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
            run_socket_command("use", Some(&strategy), cli.output_format)?;
        }
        Some(Command::Reset) => {
            run_socket_command("reset", None, cli.output_format)?;
        }
        Some(Command::Reload) => {
            run_socket_command("reload", None, cli.output_format)?;
        }
        Some(Command::Pause) => {
            run_socket_command("pause", None, cli.output_format)?;
        }
        Some(Command::Resume) => {
            run_socket_command("resume", None, cli.output_format)?;
        }
        Some(Command::Print { selection }) => {
            let args = selection.unwrap_or_else(|| "all".to_string());
            run_socket_command("print", Some(&args), cli.output_format)?;
        }
        Some(Command::SanityCheck { fan, temp, all }) => {
            let check_all = all || (!fan && !temp);
            run_sanity_check(check_all, fan, temp)?;
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

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async move {
        {
            let ctrl = controller_handle.lock().await;
            ctrl.enable_auto_fan()?;
        }

        let shutdown = Arc::new(AtomicBool::new(false));
        let server_handle = Arc::clone(&controller_handle);
        let shutdown_clone = Arc::clone(&shutdown);
        let socket_task = tokio::spawn(async move {
            if let Err(e) = start_socket_server(server_handle, shutdown_clone).await {
                tracing::error!("Socket server error: {}", e);
            }
        });

        if !silent {
            println!(
                "{:<15} {:<10} {:<10} {:<10}",
                "Strategy", "Temp", "Speed", "Active"
            );
        }

        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = std::pin::pin!(tokio::signal::ctrl_c());

        loop {
            tokio::select! {
                _ = &mut sigint => {
                    tracing::info!("Received SIGINT, switching fan to auto mode before exit");
                    break;
                }
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM, switching fan to auto mode before exit");
                    break;
                }
                _ = sleep(Duration::from_secs(1)) => {
                    let mut ctrl = controller_handle.lock().await;
                    match ctrl.step() {
                        Ok(temp) => {
                            if !silent {
                                let strategy_name = ctrl.get_current_strategy_name();
                                let speed = ctrl.get_current_speed();
                                let active = ctrl.is_active();
                                println!(
                                    "{:<15} {:<10.1} {:<10} {:<10}",
                                    strategy_name,
                                    temp,
                                    speed,
                                    active
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error in control loop: {}", e);
                        }
                    }
                }
            }
        }

        tracing::info!("Shutting down socket server...");
        shutdown.store(true, Ordering::Relaxed);
        let _ = socket_task.await;
        tracing::info!("Socket server shut down");

        let cleanup_result = {
            let ctrl = controller_handle.lock().await;
            ctrl.enable_auto_fan()
        };

        if let Err(e) = cleanup_result {
            tracing::error!("Failed to restore auto fan control on shutdown: {}", e);
            return Err(e);
        }

        Ok(())
    })
}

fn send_command(command: &str) -> Result<String> {
    use std::io::{Read, Write};
    use std::net::Shutdown;
    use std::os::unix::net::UnixStream;

    let socket_path = fw_fanctrl::socket::COMMANDS_SOCKET_FILE_PATH;

    let mut stream = UnixStream::connect(socket_path)
        .map_err(|e| fw_fanctrl::error::Error::Socket(format!("Failed to connect: {}", e)))?;

    stream
        .write_all(command.as_bytes())
        .map_err(|e| fw_fanctrl::error::Error::Socket(format!("Failed to send: {}", e)))?;

    stream
        .shutdown(Shutdown::Write)
        .map_err(|e| fw_fanctrl::error::Error::Socket(format!("Failed to shutdown: {}", e)))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| fw_fanctrl::error::Error::Socket(format!("Failed to read: {}", e)))?;

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

fn print_check_result<T>(name: &str, result: Result<T>, print_ok: impl FnOnce(&T)) {
    match result {
        Ok(val) => print_ok(&val),
        Err(e) => {
            println!("{}: FAILED", name);
            eprintln!("  Error: {}", e);
        }
    }
}

fn run_sanity_check(check_all: bool, check_fan: bool, check_temp: bool) -> Result<()> {
    let hw = HardwareController::new(false)?;

    println!("=== Sanity Check ===\n");

    // Temperature check
    if check_all || check_temp {
        print_check_result("Temperature", hw.check_temperature(), |t| {
            println!("Temperature: {:>5.1}°C - OK", t)
        });
    }

    // Power check
    print_check_result("Power", hw.is_on_ac(), |on_ac| {
        if *on_ac {
            println!("Power:       AC connected - OK")
        } else {
            println!("Power:       Battery - OK")
        }
    });

    // Fan check
    if check_all || check_fan {
        println!("\nTesting fan control...");
        match hw.test_fan_control(4) {
            Ok(results) => {
                println!("{:>6}  {:>6}", "Speed%", "RPM");
                for (speed, rpm) in results {
                    println!("{:>6}  {:>6}", speed, rpm);
                }
                println!("Fan control: OK (auto-restored)");
            }
            Err(e) => {
                println!("Fan control: FAILED");
                eprintln!("  Error: {}", e);
            }
        }
    }

    // Always restore auto fan mode at the end
    print_check_result("Fan mode", hw.enable_auto_fan(), |_| {
        println!("Fan mode: Auto")
    });

    println!("\n=== Done ===");
    Ok(())
}
