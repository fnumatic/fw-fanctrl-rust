# fw-fanctrl-rust

[![Rust](https://img.shields.io/badge/Rust-1.81+-dea584?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Framework_Laptop-FCC624?style=flat)](https://frame.work)

## Why Rust?

- **No external dependencies**: Uses [framework_lib](https://github.com/FrameworkComputer/framework-system/) instead of ectool
- **Single binary**: 2.8MB release build, no Python runtime needed
- **Memory safe**: No GC, lower memory footprint than Python
- **Fast**: Native performance for real-time fan control

## Description

Rust implementation of fw-fanctrl - controls Framework Laptop fan speeds based on configurable temperature/speed curves.

Supports all Framework Laptop models (13" / 16", Intel/AMD).

## Features

- Configurable temperature/speed curves
- Multiple built-in strategies (quiet → performance)
- AC/battery automatic strategy switching
- Unix socket for runtime control
- Compatible with Gnome extension
- systemd service support
- Diagnostic sanity-check command
- Safe shutdown handling (SIGINT/SIGTERM restores EC auto fan mode)

## Requirements

- Framework Laptop (13" / 16", Intel/AMD)
- Rust 1.81+ (for building)
- Root access (EC communication)

## Installation

### From Source

```bash
cargo build --release
sudo cp target/release/fw-fanctrl /usr/local/bin/
```

### Systemd Service

Create `/etc/systemd/system/fw-fanctrl.service`:

```ini
[Unit]
Description=Framework Fan Controller (Rust)
After=multi-user.target

[Service]
Type=simple
Restart=always
RestartSec=5
ExecStart=/usr/local/bin/fw-fanctrl run --config /etc/fw-fanctrl/config.json --silent --no-battery-sensors

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable fw-fanctrl
sudo systemctl start fw-fanctrl
```

**Note:** The service automatically restores EC fan control to automatic mode on shutdown, so no `ExecStopPost` is needed.

## Usage

### Commands

| Command | Description |
|---------|-------------|
| `fw-fanctrl run` | Start the fan control service |
| `fw-fanctrl use <strategy>` | Switch to a specific strategy |
| `fw-fanctrl reset` | Reset to default strategy |
| `fw-fanctrl reload` | Reload configuration file |
| `fw-fanctrl pause` | Pause fan control (EC auto) |
| `fw-fanctrl resume` | Resume fan control |
| `fw-fanctrl print [all\|list\|speed]` | Print status info |
| `fw-fanctrl sanity-check` | Run diagnostic checks |

### Options

| Option | Description |
|--------|-------------|
| `-c, --config <path>` | Config file path (default: `/etc/fw-fanctrl/config.json`) |
| `-s, --silent` | Disable console output |
| `--no-battery-sensors` | Exclude battery temperature sensors |
| `--output-format [natural\|json]` | Output format (default: natural) |

### Examples

```bash
# Start service
sudo fw-fanctrl run

# Start with custom config
sudo fw-fanctrl run -c ./config.json

# Switch strategy
fw-fanctrl use performance

# List strategies
fw-fanctrl print list

# Get current status (JSON for Gnome extension)
fw-fanctrl --output-format json print all

# Run diagnostics
sudo fw-fanctrl sanity-check
```

### Shutdown Safety

When running `fw-fanctrl run`, the service handles both `SIGINT` (Ctrl+C) and `SIGTERM`
(for example from `systemctl stop`). On startup and shutdown it switches EC fan control
back to automatic mode to avoid leaving the fan in manual mode.

You can stop the service safely with:

```bash
sudo systemctl stop fw-fanctrl
```

## Configuration

Configuration file: `/etc/fw-fanctrl/config.json`

```json
{
  "defaultStrategy": "lazy",
  "strategyOnDischarging": "",
  "strategies": {
    "laziest": {
      "fanSpeedUpdateFrequency": 5,
      "movingAverageInterval": 40,
      "speedCurve": [
        { "temp": 0, "speed": 0 },
        { "temp": 45, "speed": 0 },
        { "temp": 65, "speed": 25 },
        { "temp": 85, "speed": 100 }
      ]
    },
    "lazy": {
      "fanSpeedUpdateFrequency": 5,
      "movingAverageInterval": 30,
      "speedCurve": [
        { "temp": 0, "speed": 15 },
        { "temp": 50, "speed": 15 },
        { "temp": 85, "speed": 100 }
      ]
    }
  }
}
```

### Strategy Options

| Field | Description |
|-------|-------------|
| `fanSpeedUpdateFrequency` | How often to update fan speed (seconds) |
| `movingAverageInterval` | Temperature averaging window (seconds) |
| `speedCurve` | Temperature → fan speed mapping |

## Third-party Integrations

### Gnome Shell Extension

[fw-fanctrl-revived-gnome-shell-extension](https://github.com/ghostdevv/fw-fanctrl-revived-gnome-shell-extension) - Control fan profiles from Gnome Quick Settings.

Install from: https://extensions.gnome.org/extension/7864/framework-fan-control/

## Development

```bash
# Build
cargo build              # debug
cargo build --release   # optimized

# Run
cargo run                # debug
cargo run --release     # optimized

# Test
cargo test               # all tests
cargo test <name>       # specific test

# Lint
cargo clippy -- -D warnings
cargo clippy -- -D warnings --fix  # auto-fix

# Format
cargo fmt --check
cargo fmt

# All checks
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

## Release

Releases are automated via GitHub Actions. To create a new release:

```bash
# Install cargo-release (one time)
cargo install cargo-release

# Create a release (bumps version, creates tag, pushes)
cargo release minor --no-publish --allow-dirty --execute
git push --follow-tags
```

This will:
1. Bump the version in Cargo.toml
2. Create a git tag (e.g., v0.2.0)
3. Push to GitHub
4. CI will build and create a GitHub Release with the binary

Use `major`, `minor`, or `patch` as needed.

## Credits

- Original Python implementation: [TamtamHero/fw-fanctrl](https://github.com/TamtamHero/fw-fanctrl/)
- Framework Rust library: [FrameworkComputer/framework-system](https://github.com/FrameworkComputer/framework-system/)
- Gnome extension: [ghostdevv/fw-fanctrl-revived-gnome-shell-extension](https://github.com/ghostdevv/fw-fanctrl-revived-gnome-shell-extension)

## License

MIT
