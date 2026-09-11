# Quick Start

## Connect to Prometheus

If Prometheus is already running locally:

```bash
grafatui --prometheus-url http://localhost:9090
```

Point Grafatui at another Prometheus server with the same option:

```bash
grafatui --prometheus-url http://prometheus.example.com:9090
```

## Import a Grafana Dashboard

Grafatui imports either a Classic JSON dashboard or an exact
`dashboard.grafana.app/v2` JSON resource that uses a `GridLayout` or nested
`RowsLayout` containers:

```bash
grafatui --prometheus-url http://localhost:9090 --grafana-json ./dashboard.json
```

For V2 dashboards that use tabs, auto-grid, repeat, conditional rendering,
nested non-empty row variables, or library panels, use the Classic export
fallback: open **Export as code → Advanced options**, set **Model** to
**Classic**, then download or copy the JSON. V1 Resource and Resource YAML
files are unsupported.
See [Grafana Dashboard Import](grafana-dashboard-import.md) for the full format
requirements.

Override dashboard variables with repeated `--var` options:

```bash
grafatui --grafana-json ./dash.json --var job=node --var instance=server-01
```

## Run the Demo

The repository includes a Prometheus demo stack and sample dashboards:

```bash
git clone https://github.com/fedexist/grafatui.git
cd grafatui
cd examples/demo && docker-compose up -d && sleep 5 && cd ../..
cargo run -- --grafana-json examples/dashboards/prometheus_demo.json --prometheus-url http://localhost:19090
```

When finished:

```bash
cd examples/demo
docker-compose down -v
```

## Useful First Keys

| Key | Action |
|---|---|
| `q` | Quit |
| `r` | Force refresh |
| `+` / `-` | Zoom out / in |
| `[` / `]` | Pan left / right |
| `f` | Fullscreen selected panel |
| `Enter` / `Space` on a row | Toggle the row |
| `Left` / `Right` on a row | Collapse / expand the row |
| `v` | Inspect values |
| `/` | Search visible rows and panels |
