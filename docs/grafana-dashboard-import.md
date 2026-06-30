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

## Import Diagnostics

Grafatui prints import warnings before starting the TUI when a dashboard uses
important Grafana features that are skipped or ignored. Diagnostics include
unsupported panel types, value mappings, reduce options, unresolved variables,
and unsupported variable modifiers such as `${var:regex}`.

Run a non-interactive check with:

```bash
grafatui --validate --grafana-json ./dash.json
```

Warnings do not make validation fail. A dashboard that can be parsed and
imported exits successfully even if diagnostics are printed.

Use `--strict` to make warnings fail validation, or `--format json` to emit a
machine-readable summary:

```bash
grafatui --validate --strict --grafana-json ./dash.json
grafatui --validate --format json --grafana-json ./dash.json
```

## Hidden Targets

Grafatui honors `targets[].hide` by skipping hidden targets during import.
Panels with a mix of hidden and visible targets render only the visible target
queries.

## Query Modes

Grafatui honors `targets[].instant` from Grafana dashboard JSON. Targets marked
as instant use the Prometheus instant `query` endpoint, while range targets use
`query_range`.

If a target does not specify `instant`, Gauge, Bar Gauge, and Table panels
default to instant queries. Graph, Timeseries, Stat, and Heatmap panels default
to range queries.

## Field Configuration

Grafatui applies selected `fieldConfig.defaults` values where they map cleanly
to terminal rendering:

- `min` and `max` set explicit Graph y-axis bounds and Gauge limits.
- `thresholds` render graph threshold lines and drive dynamic coloring for Stat,
  Gauge, and Bar Gauge panels.
- `unit`, `decimals`, and `noValue` affect supported panel values, axes,
  legends, and exports.
- `custom.axisGridShow` controls per-panel graph guide lines.

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
