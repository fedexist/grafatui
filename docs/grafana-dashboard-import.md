# Grafana Dashboard Import

Grafatui imports supported Grafana dashboard JSON files and renders supported
panels in the terminal.

| Format | Status | Requirements |
|---|---|---|
| Classic JSON | ✅ Supported | Non-resource object with fields such as `title`, `panels`, and `templating` |
| V2 Resource JSON | 🔶 Partial | Exact `apiVersion: dashboard.grafana.app/v2` and recursive grid, row, tab, or AutoGrid containers |
| V2 Resource YAML | 🔶 Partial | The same V2 subset, read from a `.yaml` or `.yml` file |
| V1 Resource JSON | ❌ Unsupported | The `dashboard.grafana.app/v1` resource envelope is not accepted |

`--grafana-json` (alias `--grafana-dashboard`) reads `.json` files as JSON and
`.yaml`/`.yml` files as YAML. Files with any other extension are parsed as JSON
first and then as YAML.

The supported V2 subset maps inline `Panel` elements, Prometheus `PanelQuery`
queries, top-level variables, `timeSettings.autoRefresh`, supported field
configuration, fixed-grid positions, AutoGrid placement, and nested `RowsLayout`/`TabsLayout` containers to the
same Grafatui behavior as Classic JSON.

Rows and tabs may recursively contain `GridLayout`, `RowsLayout`, `TabsLayout`,
or `AutoGridLayout`. AutoGrid adapts columns to terminal width and supports
fixed row heights, `fillScreen` growth, and table content fitting with minimum/maximum heights, row matching, and body scrolling. See [Static AutoGrid sizing](grafana-compatibility.md#static-autogrid-sizing)
for supported sizing options and the runnable example.

AutoGrid items also support repeats and variable/data/time-range conditions,
with dashboard and containing row/tab variable scopes. See [the dynamic example](../examples/dashboards/grafana_v2_autogrid_dynamic.json).

Grafana's resource API writes empty lists and objects as `null` (for example
`links`, `transformations`, `options`, and `variables`), and its exporter may
omit them entirely. Grafatui treats both the same as an empty value, so
dashboards exported from Grafana 13 import unchanged. Exports made with
**Share dashboard with another instance** enabled also work: their queries carry
no datasource and run against the Prometheus server given by `--prometheus-url`.

Library panels are exported as a reference to the library panel's uid, without
the panel itself, so Grafatui skips them with an import diagnostic. Enable
**Share dashboard with another instance** when exporting to inline library
panels into the dashboard.

Repeated rows/tabs, row/tab conditions, and library panels remain unsupported; unsupported V2 layouts
and fields are fatal import errors. Repeated grid items are also rejected rather
than silently changing the dashboard.

## Export From Grafana

Grafana 13 exports dashboards in the V2 Resource model by default:

1. Open the dashboard in Grafana.
2. In the toolbar, open **Export** and select **Export as code**.
3. Choose JSON or YAML.
4. Download the file, or copy it into a local `.json` or `.yaml` file.
5. Run Grafatui with `--grafana-json`.

```bash
grafatui --prometheus-url http://localhost:9090 --grafana-json ./node-exporter.yaml
```

For dashboards that use V2 features Grafatui does not support yet, export the
Classic model instead: under **Export as code**, expand **Advanced options**,
set **Model** to **Classic**, and save the JSON. Grafana documents the available
models and export controls in
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
