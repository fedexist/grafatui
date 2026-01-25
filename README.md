# Grafatui

[![CI](https://github.com/fedexist/grafatui/workflows/CI/badge.svg)](https://github.com/fedexist/grafatui/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/grafatui.svg)](https://crates.io/crates/grafatui)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust Version](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![docs.rs](https://img.shields.io/docsrs/grafatui)](https://docs.rs/grafatui)

**Grafatui** is a terminal-based user interface (TUI) for Prometheus, inspired by Grafana. It allows you to visualize time-series data directly in your terminal with a lightweight, keyboard-driven interface.

[![asciicast](https://asciinema.org/a/vMRNEjG0FEDKGP31.svg)](https://asciinema.org/a/vMRNEjG0FEDKGP31)

## Why Grafatui?

- 🚀 **Lightweight**: No browser, no Electron, just your terminal
- ⚡ **Fast**: Sub-second startup, minimal resource usage
- 📊 **Familiar**: Import your existing Grafana dashboards
- ⌨️ **Keyboard-first**: Navigate and explore without touching your mouse
- 🎨 **Customizable**: Multiple themes and configuration options

## Quick Start

If you already have Prometheus running:

```bash
# Install from crates.io
cargo install grafatui

# Run with your Prometheus instance
grafatui --prometheus-url http://localhost:9090
```

Or try the included demo with zero setup:

```bash
git clone https://github.com/fedexist/grafatui.git
cd grafatui
cd examples/demo && docker-compose up -d && sleep 5 && cd ../..
cargo run -- --grafana-json examples/dashboards/prometheus_demo.json --prometheus-url http://localhost:19090
```

This demo showcases all 7 visualization types (graph, stat, gauge, bar gauge, table, heatmap) using a local Prometheus instance.

## Features

### Prometheus Integration
- **Direct connection** to any Prometheus instance
- **Real-time updates** with configurable refresh rates
- **Optimized queries** with caching and request deduplication

### Grafana Dashboard Import
- **Import existing dashboards** from JSON files
- **Supported panels**: graph, timeseries, gauge, bargauge, table, stat, heatmap
- **Template variables** with CLI overrides
- **Legend formatting** (`{{label}}` syntax)
- **Grid layouts** using `gridPos`

### Interactive TUI
- **Time controls**: Zoom, pan, and jump to live updates
- **Panel navigation**: Arrow keys, vim-style, or mouse
- **Search**: Quickly find panels by name
- **Fullscreen mode**: Focus on a single panel
- **Value inspection**: Cursor-based point-in-time data exploration
- **Series toggling**: Show/hide individual metrics

### Customization
- **8 themes**: default, dracula, monokai, solarized (dark/light), gruvbox, tokyo-night, catppuccin
- **Config file support** (TOML)
- **Flexible CLI** with sensible defaults

## Installation

### From Crates.io

```bash
cargo install grafatui
```

### From Source

Ensure you have Rust installed (1.85+ recommended).

```bash
git clone https://github.com/fedexist/grafatui.git
cd grafatui
cargo install --path .
```

### Prebuilt Binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/fedexist/grafatui/releases):

- **Linux** (x86_64, aarch64)
- **macOS** (x86_64, aarch64)
- **Windows** (x86_64)

### Package Managers

**Homebrew** (coming soon):
```bash
brew install grafatui
```

## Shell Completions

Grafatui can generate shell completions for Bash, Zsh, Fish, PowerShell, and Elvish.

**Bash:**
```bash
# Add to .bashrc
source <(grafatui completions bash)
```

**Zsh:**
```zsh
# Add to .zshrc
source <(grafatui completions zsh)
```

**Fish:**
```fish
grafatui completions fish | source
```

## Usage

```bash
grafatui [OPTIONS]
```

### Common Options

| Option | Description | Default |
|--------|-------------|---------|
| `--prometheus-url <URL>` | Prometheus server URL | `http://localhost:9090` |
| `--grafana-json <FILE>` | Import Grafana dashboard JSON | - |
| `--range <DURATION>` | Time range window (e.g., `5m`, `1h`, `24h`) | `5m` |
| `--step <DURATION>` | Query step resolution (e.g., `5s`, `30s`) | `5s` |
| `--var <KEY=VALUE>` | Override dashboard variables | - |
| `--theme <NAME>` | UI theme | `default` |
| `--refresh-rate <MS>` | Data fetch interval (milliseconds) | `1000` |
| `--config <FILE>` | Custom config file path | - |

Run `grafatui --help` for the full list of options.

### Configuration File

Create a `grafatui.toml` in `~/.config/grafatui/` (or use `--config`):

```toml
prometheus_url = "http://localhost:9090"
refresh_rate = 1000
time_range = "1h"
theme = "dracula"
grafana_json = "~/.config/grafatui/my-dashboard.json"
```

### Examples

**Basic usage:**
```bash
grafatui --prometheus-url http://localhost:9090
```

**Import a Grafana dashboard:**
```bash
grafatui --prometheus-url http://prod-prom:9090 --grafana-json ./node-exporter.json
```

**Override template variables:**
```bash
grafatui --grafana-json ./dash.json --var job=node --var instance=server-01
```

**Use a theme:**
```bash
grafatui --theme tokyo-night
```

**Try the included examples:**
```bash
# All visualization types
grafatui --grafana-json examples/dashboards/all_visualizations.json --prometheus-url http://localhost:9090

# vLLM monitoring (mock demo)
cd examples/demo && docker-compose up -d && cd ../..
grafatui --config examples/demo/grafatui.toml
```

> **Tip**: See the `examples/` directory for more sample dashboards.

## Keyboard Controls

| Key | Action |
|-----|--------|
| `q` | Quit |
| `r` | Force refresh |
| `+` / `-` | Zoom out / in |
| `[` / `]` | Pan left / right (time) |
| `0` | Reset to live mode |
| `↑` / `↓` or `k` / `j` | Select previous/next panel |
| `PgUp` / `PgDn` | Scroll vertically |
| `Home` / `End` | Jump to top / bottom |
| `y` | Toggle Y-axis mode |
| `1`..`9` | Toggle series visibility |
| `f` / `Enter` | Fullscreen mode |
| `v` | Value inspection mode |
| `/` | Search panels |
| `←` / `→` | Move cursor (inspect mode) |
| `?` | Toggle debug info |

## Mouse Support

- **Click**: Select panel (or move cursor in fullscreen inspect mode)
- **Drag**: Move cursor (fullscreen inspect mode only)
- **Scroll**: Scroll dashboard vertically

> **Note**: In normal mode, clicking selects panels. Press `v` or `f` to enable cursor interaction.

## Comparison with Grafana

| Feature | Grafatui | Grafana |
|---------|----------|---------|
| **Resource usage** | ~10 MB RAM | ~500+ MB RAM |
| **Startup time** | <1 second | 5-10 seconds |
| **Interface** | Terminal (TUI) | Web browser |
| **Dashboard format** | Import Grafana JSON | Native |
| **Ideal for** | Quick debugging, SSH sessions, minimal environments | Production monitoring, complex dashboards, sharing |

**Use Grafatui when:**
- You're SSH'd into a server and want quick metrics
- You need fast, low-overhead monitoring
- You live in the terminal
- You want keyboard-first workflow

**Use Grafana when:**
- You need advanced features (alerts, annotations, user management)
- You're building dashboards for a team
- You need rich visualizations and plugins

## Performance

Grafatui is designed to be fast and resource-efficient:

- **Minimal footprint**: ~10 MB memory usage
- **Smart caching**: Deduplicates identical queries across panels
- **Client-side downsampling**: Adapts data points to terminal width
- **Async architecture**: Non-blocking UI using Tokio

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

**Quick tips:**
- Use [Conventional Commits](https://www.conventionalcommits.org/) for your commit messages
- Run `cargo fmt` and `cargo clippy` before submitting
- Add tests for new features

## Acknowledgments

Grafatui is built with amazing open-source libraries:

- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal manipulation
- [tokio](https://github.com/tokio-rs/tokio) - Async runtime
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client

Inspired by [Grafana](https://grafana.com/) and the TUI ecosystem.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

Copyright 2025 Federico D'Ambrosio
