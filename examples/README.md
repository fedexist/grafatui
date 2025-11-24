# Grafatui Examples

This directory contains example Grafana dashboards and a demo environment for testing grafatui.

## Quick Demo

Want to try grafatui instantly? Use the pre-configured demo environment:

```bash
cd demo
docker-compose up -d && sleep 5 && cd ../.. && cargo run -- --grafana-json examples/dashboards/prometheus_demo.json --prometheus http://localhost:10001
```

See [`demo/README.md`](demo/README.md) for details.

## Dashboards


### `prometheus_demo.json`
**Recommended for first-time users!** A comprehensive dashboard designed for the included demo environment.
Shows all 6 visualization types with real metrics from Prometheus monitoring itself:
- Graph, Gauge, Stat, Bar Gauge, Table, Heatmap
- Works immediately with `demo/docker-compose.yml`

### `all_visualizations.json`
Demonstrates all supported panel types in a single dashboard:
- **Graph**: Line chart showing CPU usage over time
- **Gauge**: Progress bar for memory usage
- **Stat**: Big value display with sparkline for uptime
- **Bar Gauge**: Vertical bars comparing request rates
- **Table**: Tabular view of series
- **Heatmap**: Color-coded time-series intensity

### Usage

```bash
# Test with local Prometheus (default port 9090)
cargo run -- --grafana-json examples/dashboards/all_visualizations.json

# Or with custom Prometheus URL
cargo run -- --grafana-json examples/dashboards/all_visualizations.json --prometheus http://prometheus.example.com:9090

# Override variables
cargo run -- --grafana-json examples/dashboards/all_visualizations.json --var instance=localhost:9090
```

## Creating Your Own

You can export any Grafana dashboard as JSON and use it with grafatui:
1. In Grafana, go to Dashboard Settings → JSON Model
2. Copy the JSON
3. Save it to a file
4. Run: `grafatui --grafana-json your-dashboard.json`

## Supported Panel Types

- ✅ `graph` / `timeseries` - Line charts
- ✅ `gauge` - Progress bars
- ✅ `bargauge` - Bar charts
- ✅ `table` - Data tables
- ✅ `stat` - Single value + sparkline
- ✅ `heatmap` - Color intensity grid
