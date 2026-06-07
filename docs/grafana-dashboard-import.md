# Grafana Dashboard Import

Grafatui can import Grafana dashboard JSON files and render supported panels in the terminal.

## Export From Grafana

1. Open the dashboard in Grafana.
2. Open dashboard settings.
3. Choose the JSON model view.
4. Copy the JSON into a local file.
5. Run Grafatui with `--grafana-json`.

```bash
grafatui --prometheus-url http://localhost:9090 --grafana-json ./node-exporter.json
```

## Supported Panel Types

Grafatui currently supports:

- `graph`
- `timeseries`
- `stat`
- `gauge`
- `bargauge`
- `table`
- `heatmap`

Row panels are traversed so nested panels can be imported, but row headers and collapsed row behavior are not rendered.

## Variables

Grafatui reads dashboard variables from `templating.list` and expands `$var` and `${var}` in PromQL expressions.

Defaults come from the dashboard JSON. Override them from the CLI:

```bash
grafatui --grafana-json ./dash.json --var job=node --var instance=server-01
```

Prometheus query variables such as `label_values(up, instance)` and `query_result(...)` are resolved before panel queries run.

## Built-In PromQL Variables

Grafatui expands the following Grafana-style variables:

- `$__interval`
- `$__interval_ms`
- `$__range`
- `$__range_s`
- `$__range_ms`
- `$__rate_interval`
- `$__rate_interval_ms`

## Compatibility Details

See the [Grafana compatibility matrix](grafana-compatibility.md) for field-by-field support details.
