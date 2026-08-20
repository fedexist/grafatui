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

Export a Grafana dashboard using the **Classic** JSON model, then pass it to
Grafatui. In Grafana 13, choose **Export as code → Advanced options → Model:
Classic**:

```bash
grafatui --prometheus-url http://localhost:9090 --grafana-json ./dashboard.json
```

V1 Resource, V2 Resource, and YAML dashboard files are not currently
supported. See [Grafana Dashboard Import](grafana-dashboard-import.md) for the
full export procedure.

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
| `f` / `Enter` | Fullscreen selected panel |
| `v` | Inspect values |
| `/` | Search panels |
