# Grafatui

**Grafatui** is a Terminal User Interface (TUI) for viewing Prometheus metrics, inspired by Grafana. It allows you to visualize time-series data directly in your terminal with a lightweight, keyboard-driven interface.

## Features

- **Prometheus Integration**: Connects directly to your Prometheus instance.
- **Grafana Import**: Import existing Grafana dashboards (JSON) to view your familiar panels in the terminal.
    - Supports `graph`, `timeseries`, `gauge`, `bargauge`, `table`, `stat`, and `heatmap` panels.
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
git clone https://github.com/fedexist/grafatui.git
cd grafatui
cargo install --path .
```

## Quick Demo

Try grafatui in under a minute with the pre-configured demo environment:

```bash
cd examples/demo && docker-compose up -d && sleep 5 && cd ../.. && cargo run -- --grafana-json examples/dashboards/prometheus_demo.json --prometheus http://localhost:19090
```

This starts Prometheus + node-exporter and launches grafatui with a dashboard showcasing all 6 visualization types.
See [`examples/demo/README.md`](examples/demo/README.md) for details.

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

Grafatui supports a configuration file (`config.toml` or `grafatui.toml`) located in your system's standard configuration directory (e.g., `~/.config/grafatui/`), the current directory or the path specified by the `--config` option.

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

**Try the included examples:**
```bash
# Test all visualization types at once
grafatui --grafana-json examples/dashboards/all_visualizations.json --prometheus http://localhost:9090
```

> **Note:** See the `examples/` directory for more sample dashboards demonstrating all supported panel types.


### Keyboard Controls

| Key | Action |
| :--- | :--- |
| `q` | Quit |
| `r` | Force refresh |
| `+` | Zoom out (double range) |
| `-` | Zoom in (halve range) |
| `[` or `Shift+←` | Pan left (backward in time) |
| `]` or `Shift+→` | Pan right (forward in time) |
| `0` | Reset to live mode |
| `↑` / `k` | Select previous panel (auto-scrolls into view) |
| `↓` / `j` | Select next panel (auto-scrolls into view) |
| `PgUp` / `PgDn` | Scroll view vertically (fast) |
| `Home` / `End` | Jump to top / bottom |
| `y` | Toggle Y-axis mode (Auto / Zero-based) |
| `1`..`9` | Toggle visibility of series N |
| `0` | Show all series |
| `f` / `Enter` | Toggle Fullscreen mode |
| `v` | Toggle Value Inspection mode |
| `/` | Search panels |
| `Left` / `Right` | Move cursor (in Inspect mode) |
| `?` | Toggle debug bar |

### Mouse Controls

- **Click**: Select panel (in Normal mode) / Move cursor (in Fullscreen Inspect mode)
- **Drag**: Move cursor (in Fullscreen Inspect mode only)
- **Scroll**: Scroll dashboard vertically

> **Note**: In Normal mode, clicking only selects panels. Press `v` to enable cursor/inspection mode. In Fullscreen mode, clicking activates cursor automatically.

## Contributing

This project uses [Conventional Commits](https://www.conventionalcommits.org/) for automated versioning and changelog generation.

### Commit Message Format

Use the following format for your commits:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types:**
- `feat`: New feature (triggers MINOR version bump)
- `fix`: Bug fix (triggers PATCH version bump)
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Maintenance tasks
- `BREAKING CHANGE`: In footer, triggers MAJOR version bump

**Examples:**
```bash
feat(zoom): add pan left/right with bracket keys
fix(ui): correct color assignment for many series
docs(readme): update keyboard shortcuts table
refactor(app)!: change AppState API

BREAKING CHANGE: AppState constructor now requires theme parameter
```

### Using Commitizen (Optional)

To help write conventional commits, you can install `git-commitizen`:

```bash
cargo install git-commitizen
```

Then use `git cz` instead of `git commit` for an interactive prompt.

### Release Process

Releases are automated via GitHub Actions using [release-plz](https://release-plz.ieni.dev/):

1. Push commits to `main` branch using Conventional Commits format
2. `release-plz` analyzes commits and determines version bump
3. A PR is automatically created with:
   - Updated `Cargo.toml` version
   - Updated `CHANGELOG.md`
   - New Git tag
4. Review and merge the PR
5. GitHub release is created automatically

## License

MIT
