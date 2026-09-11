# Grafana Dashboard Import

Grafatui imports supported Grafana dashboard JSON files and renders supported
panels in the terminal.

| Format | Status | Requirements |
|---|---|---|
| Classic JSON | ✅ Supported | Non-resource object with fields such as `title`, `panels`, and `templating` |
| V2 Resource JSON | 🔶 Partial | JSON only, exact `apiVersion: dashboard.grafana.app/v2`, and `GridLayout` or nested `RowsLayout` containers |
| V1 Resource JSON | ❌ Unsupported | The `dashboard.grafana.app/v1` resource envelope is not accepted |
| Resource YAML | ❌ Unsupported | `--grafana-json` accepts JSON only |

The supported V2 subset maps inline `Panel` elements, Prometheus `PanelQuery`
queries, top-level variables, `timeSettings.autoRefresh`, supported field
configuration, fixed-grid positions, and nested `RowsLayout` containers to the
same Grafatui behavior as Classic JSON.

`RowsLayout` rows may contain a `GridLayout` or another `RowsLayout`. Tabs,
auto-grid, repeat, conditional rendering, nested non-empty row variables,
library panels, and Resource YAML remain unsupported; unsupported V2 layouts
and fields are fatal import errors. Repeated grid items are also rejected rather
than silently changing the dashboard.

## Export From Grafana

1. Open the dashboard in Grafana.
2. In the toolbar, open **Export** and select **Export as code**.
3. Expand **Advanced options**.
4. Set **Model** to **Classic**.
5. Download the file, or copy the JSON into a local `.json` file.
6. Run Grafatui with `--grafana-json`.

```bash
grafatui --prometheus-url http://localhost:9090 --grafana-json ./node-exporter.json
```

Grafana 13 defaults to the V2 Resource model. Its fixed-grid and supported rows
JSON resources can be imported directly. For tabs, auto-grid, or other deferred
V2 features, use this Classic export path as the fallback. Grafana documents the available models and export controls in
[Export a dashboard as code](https://grafana.com/docs/grafana/latest/visualizations/dashboards/share-dashboards-panels/#export-a-dashboard-as-code).

## Supported Panel Types

Grafatui currently supports:

- `graph`
- `timeseries`
- `stat`
- `gauge`
- `bargauge`
- `table`
- `heatmap`

Classic row headers and their collapsed state are rendered and interactive.
Collapsed descendants are excluded from navigation, search, refresh, and
export. Hidden-header rows are transparent: they consume no header and their
children remain visible.

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
unsupported V2 datasources, and unsupported variable modifiers such as
`${var:regex}`. V2 diagnostics retain their `spec.*` source paths.

Run a non-interactive check with:

```bash
grafatui --validate --grafana-json ./dash.json
```

Warnings do not make validation fail. A dashboard that can be parsed and
imported exits successfully even if diagnostics are printed.

Use `--strict` to make warnings fail validation, or `--format json` to emit a
machine-readable summary. Fatal V2 layout and repeat errors fail validation in
all modes; `--strict` additionally fails when import diagnostics are present:

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
