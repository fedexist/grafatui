# Grafatui Roadmap

Grafatui is a terminal-based Grafana-like UI for Prometheus. The roadmap is
oriented around two priorities:

1. **Grafana parity** - imported dashboards should preserve as much meaning as a
   terminal UI can reasonably express.
2. **User-visible product value** - parity work should make real dashboards
   easier to read, debug, and share.

> **Current version**: 0.1.7 · **Status**: Active development, pre-1.0

**Legend**:
- 🟢 Low complexity · 🟡 Medium complexity · 🔴 High complexity
- ✅ Shipped · 🔜 Up next · 📋 Planned · 💡 Exploring

---

## What's Already Built (v0.1.x)

These features are shipped and available today:

| Area | Feature | Details |
|---|---|---|
| Panels | **7 panel type aliases** | `graph`, `timeseries`, `stat`, `gauge`, `bargauge`, `table`, `heatmap` |
| Import | **Grafana JSON import** | Load exported dashboards from local JSON files |
| Import | **24-column grid layout** | Faithful reproduction of Grafana's `gridPos` positioning |
| Variables | **Template variables** | `$var` / `${var}` substitution with CLI and config overrides |
| Variables | **Dynamic Prometheus variables** | Query-backed variables using `label_values(...)` and `query_result(...)` |
| Variables | **PromQL built-ins** | `$__interval`, `$__interval_ms`, `$__range`, `$__range_s`, `$__range_ms`, `$__rate_interval` |
| Queries | **Multiple targets per panel** | Multiple PromQL expressions render as separate series |
| Queries | **Legend formatting** | `{{label}}` syntax from Grafana |
| UI | **8 color themes** | default, dracula, monokai, solarized dark/light, gruvbox, tokyo-night, catppuccin |
| UI | **Time controls** | Zoom in/out, pan left/right, live mode toggle |
| UI | **Panel navigation** | Arrow keys, vim-style `j`/`k`, PgUp/PgDn, fullscreen, inspect mode |
| UI | **Panel search** | `/` to fuzzy-search panels by name |
| UI | **Mouse support** | Click to select, scroll, drag cursor in fullscreen inspect mode |
| UI | **Value inspection** | Cursor-based point-in-time data exploration |
| UI | **Series toggling** | Show/hide individual series with `1`-`9` |
| Rendering | **Smart caching** | Request deduplication and caching for identical queries |
| Rendering | **Downsampling** | Max-pooling to preserve peaks while fitting the terminal |
| Rendering | **Adaptive time labels** | Date/time axis labels adjust to the selected range |
| Rendering | **Autogrid** | Global and per-panel guide lines, including configurable color |
| Field config | **Thresholds** | `fieldConfig.defaults.thresholds` for graph limit lines and Stat/Gauge/BarGauge coloring |
| Field config | **Threshold marker styles** | Dashed line, dot, braille, block, quadrant, sextant, octant, and related modes |
| Field config | **Field min/max bounds** | `fieldConfig.defaults.min` / `max` for gauge scaling and percentage thresholds |
| Field config | **Display formatting subset** | Common `unit` values, `decimals`, and `noValue` for supported panel values, axes, legends, and exports |
| Export | **SVG/PNG snapshots** | Export the visible dashboard to SVG, PNG, or both |
| Export | **Recording frame bundles** | Changed-frame recordings with manifest metadata and frame caps |
| Config | **Config file** | TOML-based persistent configuration |
| Distribution | **Shell completions and man page** | Bash, Zsh, Fish, PowerShell, Elvish, plus generated man page |
| Distribution | **Cross-platform binaries** | Linux, macOS, and Windows release assets |
| Distribution | **Package formats** | `.deb`, `.rpm`, Homebrew formula support |

For a field-by-field breakdown of Grafana JSON compatibility, see
[GRAFANA_COMPATIBILITY.md](GRAFANA_COMPATIBILITY.md). That document should be
refreshed alongside roadmap work because several v0.1.x parity features have
landed since its original snapshot.

---

## Roadmap Principles

Roadmap items are prioritized by:

1. **Grafana parity impact** - Does this make imported Grafana dashboards behave
   more like users expect?
2. **User-visible value** - Does this make dashboards more readable, trustworthy,
   or useful in a terminal?
3. **Trust in import results** - Does this reduce silent degradation when a
   dashboard contains unsupported fields?
4. **Implementation complexity** - Can the feature be shipped safely in a small,
   reviewable step?

The intent is to make parity work feel practical rather than academic. For
example, unit support is not just `fieldConfig.defaults.unit`; it is the
difference between readable latency/cache panels and raw float noise.

---

## Compatibility Ladder

This is the main backlog, ordered by Grafana parity domain.

### 1. Import Diagnostics & Trust

| Feature | Grafana field / behavior | User value | Complexity | Status |
|---|---|---|---|---|
| **Compatibility matrix refresh** | Documentation generated from current code | Keeps users and contributors aligned with reality | 🟢 | 🔜 |
| **Unsupported panel warnings** | Unsupported `panels[].type` and skipped targets | Makes import degradation visible instead of silent | 🟢 | 🔜 |
| **Import validation** | `schemaVersion`, panel shape, target shape, required fields | Lets users check dashboards before launching the TUI | 🟢 | 🔜 |
| **Better JSON/import errors** | Parse errors with path/context where possible | Faster debugging for broken exports | 🟢 | 📋 |
| **Variable substitution diagnostics** | Missing variables, unsupported format modifiers | Explains empty panels caused by unresolved variables | 🟢 | 📋 |

### 2. Field Config Parity

| Feature | Grafana field / behavior | User value | Complexity | Status |
|---|---|---|---|---|
| **Broader unit formatting** | `fieldConfig.defaults.unit` | Extends the shipped common-unit subset to more Grafana units | 🟡 | 📋 |
| **Additional no-value coverage** | `fieldConfig.defaults.noValue` | Extends the shipped null-value fallback beyond current Stat/Table/export paths where relevant | 🟢 | 📋 |
| **Value mappings** | `fieldConfig.defaults.mappings` | Status codes and enum-like values become readable labels | 🟡 | 🔜 |
| **Display names** | `fieldConfig.defaults.displayName` | Series and table labels match Grafana naming | 🟢 | 📋 |
| **Color mode subset** | `fieldConfig.defaults.color` | Honors configured color intent where terminal rendering permits | 🟡 | 📋 |
| **Field overrides subset** | `fieldConfig.overrides` | Per-series units/names/colors for common matcher types | 🔴 | 💡 |

### 3. Panel Options Parity

| Feature | Grafana field / behavior | User value | Complexity | Status |
|---|---|---|---|---|
| **Reduce options** | `options.reduceOptions.calcs` | Stat/Gauge/BarGauge can use last, min, max, mean, total | 🟡 | 🔜 |
| **Legend display mode** | `options.legend.displayMode` | Hide/list/table modes map to compact TUI equivalents | 🟡 | 📋 |
| **Legend placement** | `options.legend.placement` | Bottom/right placement influences terminal layout where useful | 🟡 | 📋 |
| **Legend calculations** | `options.legend.calcs` | Min/max/avg/current values appear beside series names | 🟡 | 📋 |
| **Text/graph/color modes** | Stat and gauge display options | Imported summary panels better match Grafana intent | 🟡 | 📋 |
| **Tooltip behavior mapping** | `options.tooltip` | Documented mapping to inspect mode, with useful defaults | 🟢 | 📋 |

### 4. Target & Query Parity

| Feature | Grafana field / behavior | User value | Complexity | Status |
|---|---|---|---|---|
| **Hidden targets** | `targets[].hide` | Helper queries do not clutter imported panels | 🟢 | 🔜 |
| **Instant queries** | `targets[].instant` / Prometheus `query` | Stat/table panels can use point-in-time queries | 🟡 | 🔜 |
| **Target interval** | `targets[].interval` / `intervalFactor` | Panel-specific resolution is respected | 🟡 | 📋 |
| **Target ref IDs** | `targets[].refId` | Better diagnostics and future transformation support | 🟢 | 📋 |
| **Format handling** | `targets[].format` | Tables and heatmaps can choose more appropriate handling | 🟡 | 📋 |
| **Exemplar awareness** | `targets[].exemplar` | Document ignored behavior or expose limited metadata later | 🔴 | 💡 |

### 5. Template Variable Parity

| Feature | Grafana field / behavior | User value | Complexity | Status |
|---|---|---|---|---|
| **Multi-value variables** | `templating.list[].multi` | Imported dashboards can query multiple instances/jobs | 🟡 | 📋 |
| **Include-all variables** | `templating.list[].includeAll` | Grafana "All" semantics work more predictably | 🟡 | 📋 |
| **Variable option sorting** | `templating.list[].sort` | Deterministic variable values from Prometheus | 🟢 | 📋 |
| **Variable picker UI** | `templating.list[].options` | Users can switch variable values without restarting | 🟡 | 📋 |
| **Format modifiers** | `${var:regex}`, `${var:pipe}`, `${var:csv}` | Common Grafana PromQL templates import correctly | 🟡 | 📋 |
| **Datasource-aware variables** | `templating.list[].datasource` | Foundation for multi-source dashboards | 🔴 | 💡 |

### 6. Graph & Timeseries Parity

| Feature | Grafana field / behavior | User value | Complexity | Status |
|---|---|---|---|---|
| **Draw styles** | `fieldConfig.defaults.custom.drawStyle` | Line, bars, and points map to distinct terminal renderings | 🟡 | 📋 |
| **Stacking** | `fieldConfig.defaults.custom.stacking` | Stacked area/bar intent is visible in dense dashboards | 🟡 | 📋 |
| **Axis labels** | `fieldConfig.defaults.custom.axisLabel` | Imported axis meaning is visible where space allows | 🟢 | 📋 |
| **Axis placement** | `fieldConfig.defaults.custom.axisPlacement` | Left/right/hidden axis settings map to TUI behavior | 🟡 | 📋 |
| **Scale distribution** | `fieldConfig.defaults.custom.scaleDistribution` | Linear/log choices are respected or explicitly warned | 🟡 | 💡 |

### 7. Panel Type Parity

| Feature | Grafana panel type | User value | Complexity | Status |
|---|---|---|---|---|
| **Row headers** | `row` | Dashboard sections stay recognizable | 🟢 | 📋 |
| **Collapsed rows** | `row.collapsed` | Large dashboards can start folded | 🟡 | 📋 |
| **Text panel** | `text` | Notes/runbook snippets survive import | 🟢 | 📋 |
| **Histogram panel** | `histogram` | Histogram dashboards import with fewer skips | 🟡 | 💡 |
| **Pie chart fallback** | `piechart` | Small category summaries can render as bars/table | 🟡 | 💡 |
| **Logs panel path** | `logs` | Opens the route toward Loki/log exploration | 🔴 | 💡 |
| **State timeline/status history** | `state-timeline`, `status-history` | Better status dashboards | 🔴 | 💡 |

### 8. Datasource & Import Parity

| Feature | Grafana field / behavior | User value | Complexity | Status |
|---|---|---|---|---|
| **Grafana API dashboard loading** | Load dashboards by UID from Grafana | Avoids manual JSON export flow | 🟡 | 📋 |
| **Multiple Prometheus sources** | `datasource` references and source switching | Supports cluster/environment dashboards | 🟡 | 📋 |
| **Mixed datasource warnings** | Mixed panels and unsupported datasources | Makes unsupported imports clear | 🟢 | 📋 |
| **Loki support** | Logs datasource and logs panel | Useful terminal observability companion | 🔴 | 💡 |
| **InfluxDB support** | Influx datasource | Broadens dashboard compatibility | 🔴 | 💡 |

---

## Milestone Slices

The compatibility ladder defines the backlog. Milestones turn it into shippable
increments.

### v0.2 - Grafana Import Fidelity

Goal: imported dashboards should be more readable and less silently degraded.

| Item | Why it belongs here | Complexity | Status |
|---|---|---|---|
| Compatibility matrix refresh | Establish the current truth before adding more parity | 🟢 | 🔜 |
| Value mappings | Makes status/stat panels useful instead of numeric-only | 🟡 | 🔜 |
| Broader unit formatting | Expands the shipped common-unit subset to more Grafana dashboards | 🟡 | 📋 |
| Additional no-value coverage | Completes the shipped fallback behavior where terminal rendering can use it | 🟢 | 📋 |
| Hidden targets | Prevents helper queries from appearing as normal series | 🟢 | 🔜 |
| Unsupported panel warnings | Builds trust in imported results | 🟢 | 🔜 |
| `--validate` import diagnostics | Gives users a non-interactive dashboard check | 🟢 | 🔜 |

### v0.3 - Panel Semantics

Goal: stat, gauge, table, and legend behavior should match common Grafana
expectations.

| Item | Why it belongs here | Complexity | Status |
|---|---|---|---|
| Reduce options | Summary panels need more than "last" | 🟡 | 📋 |
| Instant query support | Many summary/table panels are intended as instant queries | 🟡 | 📋 |
| Display names | Imported labels become clearer without changing queries | 🟢 | 📋 |
| Legend display modes and placement | Dense dashboards need predictable legend behavior | 🟡 | 📋 |
| Legend calculations | Adds useful table-like summaries without a new panel type | 🟡 | 📋 |
| Target interval support | Respects panel-specific query resolution | 🟡 | 📋 |

### v0.4 - Graph & Timeseries Fidelity

Goal: graph and timeseries panels should preserve more visual intent within TUI
constraints.

| Item | Why it belongs here | Complexity | Status |
|---|---|---|---|
| Draw styles | Bars/points/lines should be distinguishable | 🟡 | 📋 |
| Stacking | Common Grafana area/bar semantics | 🟡 | 📋 |
| Axis labels and placement | Preserves context for imported charts | 🟡 | 📋 |
| Scale distribution handling | Honor or warn on log/non-linear scales | 🟡 | 💡 |
| Row headers and collapsed rows | Keeps large dashboard structure intact | 🟡 | 📋 |

### v0.5 - Exploration Workflow

Goal: after import fidelity improves, make Grafatui a stronger daily terminal
tool for investigating Prometheus data.

| Item | User value | Complexity | Status |
|---|---|---|---|
| Dashboard/panel quick switcher | Jump around large dashboard sets quickly | 🟢 | 📋 |
| Panel history | Return to recently inspected panels | 🟢 | 📋 |
| Ad-hoc PromQL query mode | Scratch queries without editing JSON | 🟡 | 📋 |
| Label explorer | Discover metrics and labels from the terminal | 🟡 | 📋 |
| File watch mode | Auto-reload dashboards/config while iterating | 🟢 | 📋 |
| Session restore | Resume last dashboard/time range | 🟢 | 📋 |
| Bookmarks | Save dashboard + time range combinations | 🟢 | 📋 |

### Later - Live Sources, Sharing, and Operations

These are valuable, but they should not outrank core Grafana import fidelity.

| Item | User value | Complexity | Status |
|---|---|---|---|
| Grafana API dashboard browser | Pull dashboards live from Grafana | 🟡 | 📋 |
| Multiple Prometheus sources | Switch clusters/environments | 🟡 | 📋 |
| CSV export | Export panel data for reports or debugging | 🟢 | 📋 |
| Copy to clipboard | Copy panel data or values quickly | 🟢 | 📋 |
| Share snapshot | Portable rendered dashboard snapshot | 🟡 | 💡 |
| Split view | Compare two panels side-by-side | 🟡 | 💡 |
| PromQL autocomplete | Suggest metrics, labels, and functions | 🔴 | 💡 |
| Alert rule viewer | Inspect Prometheus/Alertmanager state | 🟡 | 📋 |
| Alert silence creation | Operational action from the terminal | 🟡 | 💡 |
| Desktop notifications | Notify when thresholds are crossed | 🟡 | 💡 |
| Dashboard editor mode | Edit queries inline | 🟡 | 💡 |
| Dashboard creation wizard | Build dashboards from the TUI | 🔴 | 💡 |
| SSH tunnel mode | Connect to remote Prometheus more easily | 🟡 | 💡 |
| Colorblind palettes | Improve accessibility | 🟢 | 📋 |
| Custom keybindings | User-remappable shortcuts | 🟡 | 📋 |
| Panel annotations | Deployment markers/events on graphs | 🟡 | 💡 |

---

## Best Next Steps

Recommended order for the next focused development cycle:

1. **Refresh compatibility truth**
   - Update `GRAFANA_COMPATIBILITY.md` to match v0.1.7.
   - Add tests or fixtures for fields already supported but documented as
     missing.

2. **Extend the field display layer**
   - Add value mappings and broaden unit coverage beyond the shipped common
     units.
   - Keep the shared display configuration model applied across Stat, Gauge,
     BarGauge, Table, Graph labels, legends, and exports.

3. **Make unsupported imports visible**
   - Track skipped panel types, hidden/unsupported target behavior, and ignored
     high-impact fields.
   - Surface a concise warning in the TUI and a fuller report via `--validate`.

4. **Ship v0.2 as "Grafana Import Fidelity"**
   - Keep the release tightly scoped.
   - Prefer common Grafana dashboard correctness over new non-parity features.

---

## How to Contribute

If you'd like to help, the best starting points are items marked 🔜 and 🟢.
Roadmap items are especially useful when PRs include:

- A small Grafana JSON fixture that demonstrates the supported field.
- Unit tests for parsing and display behavior.
- A short note in `GRAFANA_COMPATIBILITY.md` explaining the TUI mapping or
  limitation.

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines, and
[GRAFANA_COMPATIBILITY.md](GRAFANA_COMPATIBILITY.md) for the full Grafana JSON
feature-parity breakdown.
