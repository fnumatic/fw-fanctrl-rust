# AGENTS.md - fw-fanctrl-rust

## Project Overview

fw-fanctrl-rust is a Rust CLI tool for controlling Framework Laptop fan speeds based on
configurable temperature/speed curves. It communicates with the EC via ectool or the
framework_lib crate.

## Build Commands

```bash
# Build
cargo build              # debug build
cargo build --release   # optimized build

# Run
cargo run                # debug run
cargo run --release      # optimized run
cargo run -- --help      # show CLI help

# Test
cargo test               # run all tests
cargo test <name>       # run specific test by name
cargo test -- --nocapture  # show println! output during tests

# Lint
cargo clippy -- -D warnings    # fail on warnings
cargo clippy -- -D warnings --fix  # auto-fix clippy suggestions

# Format check
cargo fmt --check

# Format fix
cargo fmt

# Doc check (fail on warnings)
RUSTDOCFLAGS="-Dwarnings" cargo doc

# All CI checks
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

## Code Style Guidelines

### Formatting
- Use `cargo fmt` for all formatting (rustfmt)
- Maximum line length: 100 characters
- 4 spaces for indentation (no tabs)
- Trailing commas in multi-line lists
- Single space after block-opening brace, before closing

### Naming Conventions
- **Variables/functions**: snake_case (e.g., `fan_speed`, `get_temperature`)
- **Types/enums**: CamelCase (e.g., `FanController`, `StrategyConfig`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `DEFAULT_SOCKET_PATH`)
- **Files**: snake_case (e.g., `socket.rs`, `hardware_controller.rs`)
- **Modules**: snake_case (e.g., `mod hardware;`)

### Imports Organization
Order imports with blank lines between groups:
1. Standard library (`std::`, `core::`)
2. External crates (`tokio::`, `serde::`, `clap::`)
3. Local modules (`crate::`, `super::`)

```rust
use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use clap::Parser;

use crate::config::Config;
use crate::hardware::EctoolController;
```

### Error Handling
- Use `std::result::Result<T, E>` with custom error types
- Define error enums with `thiserror` or manually for better error messages
- Use the `?` operator extensively; avoid `unwrap()` except in tests
- Add context to errors with `map_err(|e| format!("context: {}", e))`

```rust
// Example error handling pattern
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Config error: {0}")]
    Config(String),
    
    #[error("Ectool error: {0}")]
    Ectool(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

### Async and Tokio
- Use tokio for async runtime (already in dependencies)
- Prefer `async fn` over manual futures
- Use `tokio::select!` for concurrent operations
- Use `tokio::sync::Mutex<T>` for shared state
- Avoid blocking calls; use tokio's async alternatives

### Logging
- Use `tracing` crate for structured logging (already in dependencies)
- Use appropriate log levels: `error!`, `warn!`, `info!`, `debug!`, `trace!`
- Include context in log messages

```rust
tracing::info!("Fan speed updated to {}%", speed);
tracing::debug!("Temperature: {}°C, curve point: {:?}", temp, point);
tracing::warn!("Failed to read temperature, using fallback");
```

### CLI with Clap
- Use clap derive macros (`#[derive(Parser)]`)
- Group related options with `#[clap(flatten)]`
- Add help text to all arguments

```rust
#[derive(Parser, Debug)]
#[clap(author, version, about)]
pub struct Cli {
    #[clap(short, long, default_value = "/etc/fw-fanctrl/config.json")]
    pub config: PathBuf,
    
    #[clap(short, long)]
    pub strategy: Option<String>,
    
    #[clap(short, long)]
    pub silent: bool,
}
```

### Config and Serialization
- Use serde with JSON for configuration
- Validate config at startup; fail fast on invalid config
- Provide sensible defaults where appropriate

### Testing Guidelines
- Unit tests: place in same file with `#[cfg(test)]` module
- Integration tests: place in `tests/` directory
- Use `#[tokio::test]` for async tests
- Mock external dependencies (ectool) for unit tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_curve_interpolation() {
        let curve = vec![(0, 0), (50, 50), (100, 100)];
        assert_eq!(interpolate(&curve, 25), 25);
    }
}
```

## Project Structure

```
src/
├── main.rs          # Entry point, CLI parsing, command dispatch
├── config.rs        # Configuration loading and validation
├── curve.rs         # Temperature-to-speed curve interpolation
├── controller.rs    # Main fan control loop and state management
├── hardware.rs      # Hardware interface (ectool or framework_lib)
└── socket.rs        # Unix socket server for runtime control

tests/               # Integration tests
config.json          # Default configuration with strategies
Cargo.toml           # Dependencies and build config
```

## Reference Implementations

### Python Reference
The Python implementation at https://github.com/TamtamHero/fw-fanctrl/
serves as the reference for:
- Fan control algorithm (moving average, curve interpolation)
- Command protocol over Unix socket
- Strategy management (AC vs battery)
- ectool command patterns

### Rust Framework Library
The official Framework Computer Rust library at https://github.com/FrameworkComputer/framework-system/
provides:
- Direct EC communication without ectool dependency
- Fan control: `fan_set_duty()`, `fan_set_rpm()`, `autofanctrl()`
- Temperature reading via EC memory map
- Power/AC status via battery flags
- MSRV: 1.81

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| tokio | Async runtime |
| serde | Serialization |
| clap | CLI parsing |
| tracing | Logging |
| thiserror | Error types (add as needed) |
| framework_lib | Alternative: Direct EC communication (optional) |

## Common Tasks

### Running the service
```bash
sudo cargo run --release -- -c /etc/fw-fanctrl/config.json
```

### Controlling via socket
```bash
echo "print current" | nc -U /run/fw-fanctrl/commands.sock
# or
fw-fanctrl-ctl use performance
```

### Adding a new strategy
Edit `config.json` and add a new strategy under `strategies`:
```json
"quiet": {
    "fanSpeedUpdateFrequency": 5,
    "movingAverageInterval": 60,
    "speedCurve": [
        {"temp": 0, "speed": 0},
        {"temp": 50, "speed": 0},
        {"temp": 70, "speed": 30},
        {"temp": 85, "speed": 100}
    ]
}
```
