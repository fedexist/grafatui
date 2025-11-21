# Grafatui

**Grafatui** is a Terminal User Interface (TUI) for viewing Prometheus metrics, inspired by Grafana. It allows you to visualize time-series data directly in your terminal with a lightweight, keyboard-driven interface.

## Features

- **Prometheus Integration**: Connects directly to your Prometheus instance.
- **Grafana Import**: Import existing Grafana dashboards (JSON) to view your familiar panels in the terminal.
    - Supports `graph`, `timeseries`, and `stat` panels.
    - Parses variables (`templating.list`) and `legendFormat`.
    - Approximates grid layouts (`gridPos`).
- **Interactive TUI**:
    - Real-time updates.
    - Zoom in/out and pan through time ranges.
    - Keyboard navigation (vim-style or arrow keys).
- **Lightweight**: Built with Rust, `ratatui`, and `tokio`.

## Installation

### From Source

Ensure you have Rust installed (1.70+ recommended).

```bash
git clone https://github.com/yourusername/grafatui.git
cd grafatui
cargo install --path .
```

## Usage

```bash
grafatui [OPTIONS]
```

### Options

- `--prometheus <URL>`: Prometheus base URL (default: `http://localhost:9090`).
- `--range <DURATION>`: Initial time range window, e.g., `5m`, `1h` (default: `5m`).
- `--step <DURATION>`: Query step resolution, e.g., `5s` (default: `5s`).
- `--grafana-json <PATH>`: Path to a Grafana dashboard JSON file to import.
- `--var <KEY=VALUE>`: Override or set dashboard variables (can be used multiple times).
- `--tick-rate <MS>`: UI refresh rate in milliseconds (default: `250`).
- `--refresh-rate <MS>`: Data fetch interval in milliseconds (default: `1000`).
- `--theme <NAME>`: UI theme (default: `default`). Supported: `default`, `dracula`, `monokai`.

## Configuration

Grafatui supports a configuration file (`config.toml` or `grafatui.toml`) located in your system's standard configuration directory (e.g., `~/.config/grafatui/`) or the current directory.

**Example `config.toml`:**
```toml
prometheus_url = "http://localhost:9090"
refresh_rate = 1000
time_range = "1h"
theme = "dracula"
```

## Themes

You can customize the look and feel using the `--theme` argument or the `theme` configuration option.

**Available Themes:**
- `default`: Standard terminal colors.
- `dracula`: Dracula color scheme.
- `monokai`: Monokai color scheme.
- `solarized-dark`: Solarized Dark.
- `solarized-light`: Solarized Light.
- `gruvbox`: Gruvbox Dark.
- `tokyo-night`: Tokyo Night.
- `catppuccin`: Catppuccin Mocha.

### Examples

**Basic usage:**
```bash
grafatui --prometheus http://localhost:9090
```

**Import a Grafana dashboard:**
```bash
grafatui --prometheus http://prod-prom:9090 --grafana-json ./dashboards/node-exporter.json
```

**Override variables:**
```bash
grafatui --grafana-json ./dash.json --var job=node --var instance=server-01
```

### Keyboard Controls

| Key | Action |
| :--- | :--- |
| `q` | Quit |
| `r` | Force refresh |
| `+` | Zoom out (double range) |
| `-` | Zoom in (halve range) |
| `↑` / `k` | Select previous panel |
| `↓` / `j` | Select next panel |
| `PgUp` / `PgDn` | Scroll view vertically |
| `Home` / `End` | Jump to top / bottom |
| `y` | Toggle Y-axis mode (Auto / Zero-based) |
| `1`..`9` | Toggle visibility of series N |
| `0` | Show all series |
| `?` | Toggle debug bar |

## License

MIT
