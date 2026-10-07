# Configuration

Grafatui can be configured with CLI options, a TOML configuration file, or both. CLI options override values from the configuration file.

## Common CLI Options

| Option | Description | Default |
|---|---|---|
| `--prometheus-url <URL>` | Prometheus server URL | `http://localhost:9090` |
| `--grafana-json <FILE>` | Grafana dashboard file: Classic JSON, or V2 resource JSON or YAML | none |
| `--annotations-file <FILE>` | Read-only external JSONL point-event file | none |
| `--annotations-command <PROGRAM>` | Read-only executable annotation provider | none |
| `--annotations-command-arg <ARG>` | Argument for `--annotations-command`; repeat to preserve order | none |
| `--annotations-command-timeout <DURATION>` | Maximum command-provider runtime | `10s` |
| `--validate` | Check the Grafana dashboard import and exit without starting the TUI | `false` |
| `--strict` | Make `--validate` fail when diagnostics contain warnings | `false` |
| `--format <FORMAT>` | Output format for `--validate`: `text` or `json` | `text` |
| `--range <DURATION>` | Time range window, such as `5m`, `1h`, or `24h` | `5m` |
| `--step <DURATION>` | Exact query step, such as `5s` or `30s`; omit it for automatic range-aware resolution | automatic, minimum `5s` |
| `--scrape-interval <DURATION>` | Prometheus scrape interval, used for `$__rate_interval` | `15s` |
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
scrape_interval = "15s"
theme = "dracula"
threshold_marker = "dashed"
export_dir = "./grafatui-exports"
export_format = "svg"
record_max_frames = 300
autogrid = true
autogrid_color = "dark-gray"
grafana_json = "~/.config/grafatui/my-dashboard.json"
annotations_file = "./events.jsonl"

[vars]
job = "node"
instance = "server-01"
```

## Query Resolution

When `step`/`--step` is absent, range queries choose their step the way
Grafana's Prometheus datasource does:

1. Divide the time range by the panel's `maxDataPoints`, or by 1000 when it is
   unset.
2. Round to a Grafana interval, such as `20s`, `1m`, or `10m`.
3. Use the min interval instead when it is larger. The min interval is the
   target's `interval`, then the panel's `interval`, then `5s`.
4. Coarsen the step if the query would exceed Prometheus's limit of 11,000
points per series.

An explicit `--step` or configured `step` is used exactly. It is not rounded,
coarsened for the time range, or overridden by imported panel/target intervals.

`$__interval` is the step a query runs with. `$__rate_interval` is
`max($__interval + scrape interval, 4 × scrape interval)`. The scrape interval
is the query's min interval in automatic mode when the dashboard sets one, and
`--scrape-interval` otherwise. Explicit-step mode always uses
`--scrape-interval` for this calculation.

With the defaults, a `5m` or `1h` range queries every `5s`, `24h` every `1m`,
and `7d` every `10m`. The title bar shows the step for panels without their own
query options.

## External Annotation Sources

Select one read-only annotation source: `annotations_file` or the nested
`[annotations_command]` table. The two TOML forms conflict. The
`--annotations-file` CLI flag conflicts with every command-source CLI flag;
CLI source selection still replaces the complete TOML annotation source.

```toml
[annotations_command]
program = "./target/debug/examples/git_annotation_provider"
args = ["."]
timeout = "10s"
```

`program` is required; `args` defaults to an empty list and `timeout` defaults
to `10s`. The matching CLI source is:

```bash
grafatui \
  --annotations-command ./target/debug/examples/git_annotation_provider \
  --annotations-command-arg=. \
  --annotations-command-timeout 10s
```

`--annotations-command-arg` and `--annotations-command-timeout` require
`--annotations-command`; repeat the argument flag to retain argument order.
`--annotations-file` and `--annotations-command` cannot be combined. A CLI file
or command has whole-source precedence over TOML: it replaces the configured
file or complete command configuration rather than merging individual fields.

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
