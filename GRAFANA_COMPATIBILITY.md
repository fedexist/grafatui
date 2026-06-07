# Grafana Dashboard JSON Compatibility

This document provides a comprehensive feature-parity table between the [Grafana Dashboard JSON Model](https://grafana.com/docs/grafana/latest/dashboards/build-dashboards/json-model/) and what Grafatui currently supports.

> **Snapshot**: Grafatui v0.1.7. The roadmap prioritizes Grafana parity first,
> then user-visible product value. See [ROADMAP.md](ROADMAP.md) for milestone
> slices built from this compatibility ladder.

**Legend**:
- ✅ **Supported** — Fully implemented and working
- 🔶 **Partial** — Partially implemented or with limitations
- ❌ **Not Implemented** — Recognized but not yet functional
- ⛔ **Not Applicable** — Cannot be implemented in a TUI context (e.g., browser-only features)

---

## Dashboard-Level Properties

| JSON Field | Status | Notes |
|---|---|---|
| `title` | ✅ Supported | Displayed in the title bar |
| `uid` | ❌ Not Implemented | Not used (not needed for local JSON import) |
| `id` | ❌ Not Implemented | Not used |
| `version` | ❌ Not Implemented | Not used |
| `tags` | ❌ Not Implemented | Ignored |
| `timezone` | ❌ Not Implemented | All timestamps displayed in UTC |
| `editable` | ⛔ Not Applicable | Grafatui is read-only |
| `style` | ⛔ Not Applicable | TUI has its own theme system |
| `schemaVersion` | ❌ Not Implemented | Not validated |
| `refresh` | ✅ Supported | Used as the default data refresh interval; overridden by config or `--refresh-rate` |
| `time` | ❌ Not Implemented | Uses `--range` CLI option instead |
| `time.from` / `time.to` | ❌ Not Implemented | Uses `--range` CLI option instead |
| `fiscalYearStartMonth` | ⛔ Not Applicable | |
| `liveNow` | ❌ Not Implemented | Uses `0` key to reset to live instead |
| `weekStart` | ⛔ Not Applicable | |

---

## Panels

### Panel Types

| Panel Type | Status | Notes |
|---|---|---|
| `graph` (legacy) | ✅ Supported | Rendered as a line chart (Braille markers) |
| `timeseries` | ✅ Supported | Mapped to graph renderer |
| `stat` | ✅ Supported | Big value + sparkline |
| `gauge` | ✅ Supported | Horizontal gauge bar |
| `bargauge` | ✅ Supported | Vertical bar chart |
| `table` | ✅ Supported | Two-column table (Series, Value) |
| `heatmap` | ✅ Supported | Character-based block heatmap |
| `row` | 🔶 Partial | Row panels are traversed for nested panels, but row headers/collapse are not rendered |
| `text` | ❌ Not Implemented | Skipped during import |
| `dashlist` | ❌ Not Implemented | Skipped during import |
| `alertlist` | ❌ Not Implemented | Skipped during import |
| `news` | ⛔ Not Applicable | |
| `annolist` | ❌ Not Implemented | |
| `barchart` | ❌ Not Implemented | Skipped (distinct from `bargauge`) |
| `candlestick` | ❌ Not Implemented | |
| `canvas` | ⛔ Not Applicable | Interactive canvas not feasible in TUI |
| `datagrid` | ❌ Not Implemented | |
| `debug` | ⛔ Not Applicable | |
| `geomap` | ⛔ Not Applicable | Map visualization not feasible in TUI |
| `histogram` | ❌ Not Implemented | |
| `logs` | ❌ Not Implemented | |
| `nodeGraph` | ⛔ Not Applicable | |
| `piechart` | ❌ Not Implemented | |
| `state-timeline` | ❌ Not Implemented | |
| `status-history` | ❌ Not Implemented | |
| `trend` | ❌ Not Implemented | |
| `xychart` | ❌ Not Implemented | |

### Panel Common Fields

| JSON Field | Status | Notes |
|---|---|---|
| `title` | ✅ Supported | Displayed as the panel border title |
| `type` | ✅ Supported | Used to select the renderer |
| `gridPos` | ✅ Supported | 24-column grid layout fully supported |
| `gridPos.x` | ✅ Supported | |
| `gridPos.y` | ✅ Supported | |
| `gridPos.w` | ✅ Supported | |
| `gridPos.h` | ✅ Supported | |
| `id` | ❌ Not Implemented | Not used |
| `description` | ❌ Not Implemented | Not displayed |
| `transparent` | ⛔ Not Applicable | TUI panels always have borders |
| `links` | ⛔ Not Applicable | No browser navigation |
| `repeat` | ❌ Not Implemented | Template repeat not supported |
| `repeatDirection` | ❌ Not Implemented | |
| `maxPerRow` | ❌ Not Implemented | |
| `collapsed` (row) | ❌ Not Implemented | Rows are always expanded |
| `panels` (nested in row) | ✅ Supported | Nested panels are extracted recursively |

---

## Targets (Queries)

| JSON Field | Status | Notes |
|---|---|---|
| `targets` (array) | ✅ Supported | Multiple targets per panel supported |
| `targets[].expr` | ✅ Supported | PromQL expression |
| `targets[].legendFormat` | ✅ Supported | `{{label}}` syntax for legend formatting |
| `targets[].refId` | ❌ Not Implemented | Not used |
| `targets[].datasource` | ❌ Not Implemented | Only Prometheus datasource is supported |
| `targets[].interval` | ❌ Not Implemented | Uses global `--step` instead |
| `targets[].intervalFactor` | ❌ Not Implemented | |
| `targets[].instant` | ✅ Supported | Uses Prometheus instant `query` when true; Gauge, BarGauge, and Table default to instant |
| `targets[].format` | ❌ Not Implemented | Always treated as time_series |
| `targets[].hide` | ❌ Not Implemented | All targets are visible |
| `targets[].exemplar` | ❌ Not Implemented | |
| `targets[].editorMode` | ⛔ Not Applicable | UI-only setting |

### PromQL Special Variables

| Variable | Status | Notes |
|---|---|---|
| `$__rate_interval` | ✅ Supported | Computed as `max(step × 4, 60s)` |
| `$__rate_interval_ms` | ✅ Supported | Millisecond form of `$__rate_interval` |
| `$__interval` | ✅ Supported | Computed from the current range and panel resolution, bounded by `--step` |
| `$__interval_ms` | ✅ Supported | Millisecond form of `$__interval` |
| `$__range` | ✅ Supported | Current dashboard time range |
| `$__range_s` | ✅ Supported | Current dashboard time range in seconds |
| `$__range_ms` | ✅ Supported | Current dashboard time range in milliseconds |

---

## Templating (Variables)

| JSON Field | Status | Notes |
|---|---|---|
| `templating.list` | ✅ Supported | Variables extracted from dashboard |
| `templating.list[].name` | ✅ Supported | Used as `$var` or `${var}` in queries |
| `templating.list[].current.value` | ✅ Supported | Used as default value |
| `templating.list[].current.text` | 🔶 Partial | Used as fallback if `value` is missing |
| `templating.list[].allValue` | ✅ Supported | Used when value is `$__all`, falls back to `.*` |
| `templating.list[].type` | 🔶 Partial | `query` variables are resolved for Prometheus |
| `templating.list[].query` | 🔶 Partial | Supports Prometheus `label_values(...)` and `query_result(...)` |
| `templating.list[].definition` | 🔶 Partial | Used as a fallback query expression for dynamic query variables |
| `templating.list[].datasource` | ❌ Not Implemented | |
| `templating.list[].regex` | 🔶 Partial | Applied to dynamic query variable results |
| `templating.list[].sort` | ❌ Not Implemented | |
| `templating.list[].multi` | ❌ Not Implemented | Multi-value selection not supported |
| `templating.list[].includeAll` | ❌ Not Implemented | |
| `templating.list[].refresh` | 🔶 Partial | Dynamic variables refresh before panel queries |
| `templating.list[].options` | ❌ Not Implemented | No dropdown/picker UI |
| `templating.list[].hide` | ❌ Not Implemented | |
| CLI `--var KEY=VALUE` override | ✅ Supported | Overrides dashboard defaults from command line |
| Config file `vars` override | ✅ Supported | Overrides via TOML config |

### Variable Substitution

| Pattern | Status | Notes |
|---|---|---|
| `$varname` | ✅ Supported | Simple substitution |
| `${varname}` | ✅ Supported | Braced substitution |
| `${varname:regex}` | ❌ Not Implemented | Format modifiers not supported |
| `${varname:pipe}` | ❌ Not Implemented | |
| `${varname:csv}` | ❌ Not Implemented | |
| `${varname:json}` | ❌ Not Implemented | |
| `${varname:queryparam}` | ❌ Not Implemented | |
| `$__all` | ✅ Supported | Replaced with `allValue` or `.*` |

---

## Field Configuration (`fieldConfig`)

`fieldConfig` is partially implemented. Thresholds, min/max bounds, selected
display formatting fields, threshold style, and per-panel autogrid settings are
parsed; value mappings, display names, and field overrides remain major gaps.

| JSON Field | Status | Notes |
|---|---|---|
| `fieldConfig` | 🔶 Partial | Parsed for supported defaults/custom fields below |
| `fieldConfig.defaults` | 🔶 Partial | Parsed for min/max, thresholds, and selected custom fields |
| `fieldConfig.defaults.unit` | 🔶 Partial | Common units such as bytes, bits, seconds, milliseconds, percent, percentunit, ops, request rate, and byte rate are formatted; unknown units fall back to Grafatui's compact SI formatter |
| `fieldConfig.defaults.min` | ✅ Supported | Used for interpolating percentage thresholds and Gauge limits |
| `fieldConfig.defaults.max` | ✅ Supported | Used for scaling gauges and threshold boundaries |
| `fieldConfig.defaults.decimals` | ✅ Supported | Controls numeric precision in panel values, graph axes, legends, and exports |
| `fieldConfig.defaults.color` | ❌ Not Implemented | Uses theme palette instead |
| `fieldConfig.defaults.mappings` | ❌ Not Implemented | Value mappings not supported |
| `fieldConfig.defaults.noValue` | 🔶 Partial | Used for null Stat/Table values and exports; empty panels still show Grafatui's `No data` state |
| `fieldConfig.defaults.displayName` | ❌ Not Implemented | |
| `fieldConfig.defaults.custom` | 🔶 Partial | Used for threshold style and axis grid visibility |
| `fieldConfig.defaults.custom.drawStyle` | ❌ Not Implemented | Always drawn as lines |
| `fieldConfig.defaults.custom.lineWidth` | ❌ Not Implemented | TUI limitation |
| `fieldConfig.defaults.custom.fillOpacity` | ⛔ Not Applicable | TUI limitation |
| `fieldConfig.defaults.custom.pointSize` | ⛔ Not Applicable | TUI limitation |
| `fieldConfig.defaults.custom.stacking` | ❌ Not Implemented | No stacked charts |
| `fieldConfig.defaults.custom.axisPlacement` | ❌ Not Implemented | |
| `fieldConfig.defaults.custom.axisLabel` | ❌ Not Implemented | |
| `fieldConfig.defaults.custom.axisGridShow` | ✅ Supported | Controls per-panel autogrid guide lines for graph/time-series panels |
| `fieldConfig.defaults.custom.thresholdsStyle` | 🔶 Partial | `mode` is parsed for threshold rendering; glyph style is also controlled by Grafatui's marker setting |
| `fieldConfig.defaults.custom.scaleDistribution` | ❌ Not Implemented | Always linear |
| `fieldConfig.overrides` | ❌ Not Implemented | |

### Thresholds

| JSON Field | Status | Notes |
|---|---|---|
| `fieldConfig.defaults.thresholds` | ✅ Supported | Applied to Graph limit lines and dynamic coloring for Stat, Gauge & BarGauge |
| `fieldConfig.defaults.thresholds.mode` | ✅ Supported | (`absolute` / `percentage`) |
| `fieldConfig.defaults.thresholds.steps` | ✅ Supported | |
| `fieldConfig.defaults.thresholds.steps[].value` | ✅ Supported | Evaluated mathematically against metric values |
| `fieldConfig.defaults.thresholds.steps[].color` | ✅ Supported | Maps keywords (e.g., `green`) and hex codes (e.g., `#FF0000`) |

---

## Panel Options (`options`)

Panel-specific `options` are not parsed yet. Grafatui currently applies its own
compact TUI defaults for legends, stat sparklines, gauges, and inspect-mode
tooltips.

| JSON Field | Status | Notes |
|---|---|---|
| `options` | ❌ Not Implemented | Panel-specific options object is ignored |
| `options.legend` | ❌ Not Implemented | Grafatui uses its own compact legend |
| `options.legend.displayMode` | ❌ Not Implemented | Always shows inline legend |
| `options.legend.placement` | ❌ Not Implemented | Always bottom |
| `options.legend.calcs` | ❌ Not Implemented | No calculated legend values (min/max/avg) |
| `options.tooltip` | ❌ Not Implemented | Inspect mode serves as tooltip substitute |
| `options.tooltip.mode` | ❌ Not Implemented | |
| `options.orientation` | ❌ Not Implemented | |
| `options.reduceOptions` | ❌ Not Implemented | Stat/Gauge always use last value |
| `options.reduceOptions.calcs` | ❌ Not Implemented | |
| `options.reduceOptions.fields` | ❌ Not Implemented | |
| `options.textMode` | ❌ Not Implemented | |
| `options.colorMode` | ❌ Not Implemented | |
| `options.graphMode` | ❌ Not Implemented | Stat always shows sparkline |

---

## Annotations

| JSON Field | Status | Notes |
|---|---|---|
| `annotations` | ❌ Not Implemented | |
| `annotations.list` | ❌ Not Implemented | |

---

## Data Links & Transformations

| JSON Field | Status | Notes |
|---|---|---|
| `options.dataLinks` | ⛔ Not Applicable | No browser navigation in TUI |
| `transformations` | ❌ Not Implemented | |
| `transformations[].id` | ❌ Not Implemented | (e.g., `organize`, `merge`, `reduce`) |

---

## Alert Rules

| JSON Field | Status | Notes |
|---|---|---|
| `alert` | ❌ Not Implemented | Panel-level alerts |
| `alert.conditions` | ❌ Not Implemented | |
| `alert.notifications` | ❌ Not Implemented | |

---

## Datasource Configuration

| Feature | Status | Notes |
|---|---|---|
| Prometheus (`query_range`) | ✅ Supported | Primary and only supported datasource |
| Prometheus (`query` instant) | ✅ Supported | Used for dynamic template variables and instant panel targets |
| Prometheus labels API | ✅ Supported | Used for dynamic variable `label_values(...)` |
| Mixed datasource | ❌ Not Implemented | |
| InfluxDB | ❌ Not Implemented | |
| Loki | ❌ Not Implemented | |
| Elasticsearch | ❌ Not Implemented | |
| Other datasources | ❌ Not Implemented | |

---

## Summary Statistics

| Category | Supported | Partial | Not Implemented | Not Applicable |
|---|---|---|---|---|
| Dashboard Properties | 1 | 0 | 10 | 4 |
| Panel Types | 7 | 1 | 14 | 5 |
| Panel Common Fields | 8 | 0 | 6 | 2 |
| Targets / Queries | 3 | 0 | 8 | 1 |
| PromQL Variables | 7 | 0 | 0 | 0 |
| Templating | 6 | 6 | 6 | 0 |
| Variable Substitution | 3 | 0 | 5 | 0 |
| Field Config | 4 | 6 | 10 | 2 |
| Thresholds | 5 | 0 | 0 | 0 |
| Panel Options | 0 | 0 | 14 | 0 |
| Annotations | 0 | 0 | 2 | 0 |
| Data Links / Transforms | 0 | 0 | 2 | 1 |
| Alert Rules | 0 | 0 | 3 | 0 |
| Datasources | 2 | 1 | 5 | 0 |
| **Total** | **46** | **14** | **85** | **15** |

---

## Most Requested Missing Features

Based on user feedback, the following missing features are most commonly expected:

1. **Value mappings** (`fieldConfig.defaults.mappings`) — Map numeric values to text labels
2. **Broader unit formatting** (`fieldConfig.defaults.unit`) — Extend the current common-unit subset to more Grafana unit families
3. **Reduce options** (`options.reduceOptions`) — Use min/max/mean/total instead of always using the latest value
4. **Import diagnostics** — Warn clearly about skipped panel types and ignored high-impact fields
5. **Instant query panel targets** (`targets[].instant`) — Support point-in-time queries for stat/table-style panels
6. **Additional panel types** — `text`, `piechart`, `histogram`, `logs`

---

## What Grafatui Does Instead

Grafatui provides several TUI-native capabilities that don't map directly to Grafana JSON features:

| Grafatui Feature | Description |
|---|---|
| **8 color themes** | `default`, `dracula`, `monokai`, `solarized-dark`, `solarized-light`, `gruvbox`, `tokyo-night`, `catppuccin` |
| **Keyboard navigation** | Vim-style (`j`/`k`), arrow keys, page up/down |
| **Panel search** | `/` opens a fuzzy-search popup |
| **Fullscreen mode** | `f` to focus on a single panel |
| **Inspect mode** | `v` enables cursor-based point-in-time inspection |
| **Y-axis toggle** | `y` switches between auto-scale and zero-based |
| **Series toggling** | `1`–`9` to show/hide individual series |
| **Autogrid toggle** | `g` toggles automatic guide lines |
| **Mouse support** | Click to select, scroll to navigate, drag cursor in fullscreen |
| **Smart caching** | Request deduplication and caching for identical queries |
| **Client-side downsampling** | Max-pooling to ~200 points to preserve peaks |
| **SVG/PNG export and recordings** | Save dashboard snapshots or changed-frame recording bundles |
| **TOML configuration** | Persistent config file for all CLI options |

---

*This document was reviewed against the Grafatui source code at v0.1.7. If you notice any inaccuracies, please open an issue or PR.*
