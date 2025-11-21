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
| `↑` / `↓` | Scroll vertically |
| `PgUp` / `PgDn` | Scroll fast |
| `Home` / `End` | Jump to top / bottom |
| `?` | Toggle debug bar |

## License

MIT
