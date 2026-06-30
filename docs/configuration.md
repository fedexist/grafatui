# Configuration

Grafatui can be configured with CLI options, a TOML configuration file, or both. CLI options override values from the configuration file.

## Common CLI Options

| Option | Description | Default |
|---|---|---|
| `--prometheus-url <URL>` | Prometheus server URL | `http://localhost:9090` |
| `--grafana-json <FILE>` | Grafana dashboard JSON file | none |
| `--validate` | Check the Grafana dashboard import and exit without starting the TUI | `false` |
| `--strict` | Make `--validate` fail when diagnostics contain warnings | `false` |
| `--format <FORMAT>` | Output format for `--validate`: `text` or `json` | `text` |
| `--range <DURATION>` | Time range window, such as `5m`, `1h`, or `24h` | `5m` |
| `--step <DURATION>` | Query step resolution, such as `5s` or `30s` | `5s` |
| `--var <KEY=VALUE>` | Override a dashboard variable | none |
| `--theme <NAME>` | UI theme | `default` |
| `--threshold-marker <MARKER>` | Marker for threshold lines | `dashed` |
| `--autogrid-color <COLOR>` | Color for automatic graph grid lines and labels | `dark-gray` |
| `--export-dir <DIR>` | Directory for exports and recordings | `./grafatui-exports` |
| `--export-format <FORMAT>` | `svg`, `png`, or `both` | `svg` |
| `--record-max-frames <COUNT>` | Maximum changed frames per recording | `300` |
| `--refresh-rate <MS>` | Data fetch interval in milliseconds | `1000` |
| `--config <FILE>` | Configuration file path | none |

Run the full help output with:

```bash
grafatui --help
```

## Configuration File

Create `grafatui.toml` in `~/.config/grafatui/`, or pass a custom path with `--config`.

```toml
prometheus_url = "http://localhost:9090"
refresh_rate = 1000
time_range = "1h"
step = "5s"
theme = "dracula"
threshold_marker = "dashed"
export_dir = "./grafatui-exports"
export_format = "svg"
record_max_frames = 300
autogrid = true
autogrid_color = "dark-gray"
grafana_json = "~/.config/grafatui/my-dashboard.json"

[vars]
job = "node"
instance = "server-01"
```

## Themes

Built-in themes include:

- `default`
- `dracula`
- `monokai`
- `solarized-dark`
- `solarized-light`
- `gruvbox`
- `tokyo-night`
- `catppuccin`

Use a theme from the CLI:

```bash
grafatui --theme tokyo-night
```
