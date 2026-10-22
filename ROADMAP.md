# Grafatui Roadmap

Grafatui is a terminal-based Grafana-like UI for Prometheus. The roadmap is
oriented around two priorities:

1. **Grafana parity** - imported dashboards should preserve as much meaning as a
   terminal UI can reasonably express.
2. **User-visible product value** - parity work should make real dashboards
   easier to read, debug, and share.

> **Current release**: 0.1.18 · **Status**: Active development, pre-1.0

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
| Queries | **Instant target queries** | Honors `targets[].instant` and defaults summary table/gauge panels to Prometheus instant queries |
| Queries | **Last-good query state** | Retains failed-query series, merges partial successes, and surfaces Prometheus warnings without hiding usable data |
| Queries | **Single-calculation summary reduction** | Classic/V2 Stat, Gauge, and BarGauge panels support `last`, `lastNotNull`, `min`, `max`, `mean`, and `sum` over raw samples |
| Queries | **Legend formatting** | `{{label}}` syntax from Grafana |
| UI | **8 color themes** | default, dracula, monokai, solarized dark/light, gruvbox, tokyo-night, catppuccin |
| UI | **Time controls** | Zoom in/out, pan left/right, live mode toggle |
| UI | **Panel navigation** | Arrow keys, vim-style `j`/`k`, PgUp/PgDn, fullscreen, inspect mode |
| UI | **Panel search** | `/` to fuzzy-search panels by name |
| UI | **Interactive dashboard rows** | Classic row headers and collapsed state plus nested V2 `RowsLayout`; Enter/Space toggles, Left collapses, and Right expands selected rows |
| UI | **Interactive dashboard tabs** | Nested V2 `TabsLayout`; Left/Right switches the focused tab and Enter/Space enters its content |
| UI | **Multiple dashboard files and top-level tabs** | Load multiple dashboards and switch between their independently refreshing views |
| UI | **Mouse support** | Click to select, scroll, drag cursor in fullscreen inspect mode |
| UI | **Value inspection** | Cursor-based point-in-time data exploration |
| UI | **Series toggling** | Show/hide individual series with `1`-`9` |
| Annotations | **External annotation providers (iterations 1–3)** | Read-only file/command JSONL events on graph/timeseries panels, targeting, tag filtering, bounded provider protocol, concurrent refresh, export, and recording parity |
| Rendering | **Smart caching** | Request deduplication and caching for identical queries |
| Rendering | **Downsampling** | Max-pooling to preserve peaks while fitting the terminal |
| Rendering | **Adaptive time labels** | Date/time axis labels adjust to the selected range |
| Rendering | **Autogrid** | Global and per-panel guide lines, including configurable color |
| Field config | **Thresholds** | `fieldConfig.defaults.thresholds` for graph limit lines and Stat/Gauge/BarGauge coloring |
| Field config | **Threshold marker styles** | Dashed line, dot, braille, block, quadrant, sextant, octant, and related modes |
| Field config | **Field min/max bounds** | `fieldConfig.defaults.min` / `max` for graph y-axis bounds, gauge scaling, and percentage thresholds |
| Field config | **Display formatting subset** | Common `unit` values, `decimals`, and `noValue` for supported panel values, axes, legends, and exports |
| Field config | **Exact numeric value mappings** | Imported Classic/V2 values render mapped text in supported panels and exports; other mapping kinds produce diagnostics |
| Export | **SVG/PNG snapshots** | Export the visible dashboard to SVG, PNG, or both |
| Export | **Recording frame bundles** | Changed-frame recordings with manifest metadata and frame caps |
| Config | **Config file** | TOML-based persistent configuration |
| Distribution | **Shell completions and man page** | Bash, Zsh, Fish, PowerShell, Elvish, plus generated man page |
| Distribution | **Cross-platform binaries** | Linux, macOS, and Windows release assets |
| Distribution | **Package formats** | `.deb`, `.rpm`, Homebrew formula support |

For a field-by-field breakdown of Grafana JSON compatibility, see the
[compatibility matrix](docs/grafana-compatibility.md). Keep that document
refreshed alongside parity work so it stays aligned with the current release.

### External Annotation Iterations

| Iteration | Scope | Status |
|---|---|---|
| 1 | External JSONL point annotations | ✅ Shipped |
| 2 | Navigable annotation popup, panel targeting, and tag filtering | ✅ Shipped |
| 3 | Bounded command provider protocol and Prometheus-coordinated refresh | ✅ Shipped |

These iterations are separate from Grafana `annotations` and `annotations.list`,
which remain unsupported.

### Provider Ecosystem and Integrations (Post-Core)

| Item | User value | Complexity | Status |
|---|---|---|---|
| Independent provider scheduling and redraw | Refresh slow providers without coupling them to Prometheus redraws | 🟡 | 📋 |
| Stable public Rust provider API | Build supported native providers outside Grafatui | 🟡 | 📋 |
| Possible Python SDK | Make provider authoring accessible without Rust | 🟡 | 💡 |
| Long-running providers | Reuse authenticated clients and streams safely | 🔴 | 📋 |
| Multiple annotation sources | Combine independent event systems in one overlay | 🔴 | 📋 |
| Range events | Show maintenance windows and other duration-based annotations | 🟡 | 📋 |
| Stable event IDs | Support deduplication and reliable provider updates | 🟡 | 📋 |
| Per-panel tag filters | Let each panel narrow the shared annotation stream | 🟡 | 📋 |
| Reference/community CI/CD providers | Turn durable CI/CD deployment records into useful overlays | 🟡 | 📋 |
| HTTP, Grafana, and vendor adapters | Add integrations only where demonstrated demand exists | 🔴 | 💡 |

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

## Recent Releases and Proposed Next Slice

The next release stays within the `0.1.x` line. Each feature remains a focused,
reviewable slice that preserves current defaults and can ship independently.

| Release | Focus | Outcome |
|---|---|---|
| **0.1.15** ✅ | Secure connectivity | Released: private CAs and mutual TLS work without weakening default TLS behavior |
| **0.1.16** ✅ | Variable correctness | Released: Grafana refresh settings and dependent variables behave predictably |
| **0.1.17** ✅ | Value mappings | Released: exact numeric mappings render consistently across supported panels |
| **0.1.18** ✅ | Multiple dashboards and summary accuracy | Released: multiple input files, concurrent top-level tabs, Gauge null-value correction, and single-calculation `reduceOptions` |
| **0.1.19** 🔜 Proposed | Dashboard readability | Candidate slices: display names, more common units, and numeric range mappings; scope remains adjustable |

### 0.1.15 - Secure Connectivity (released)

- Add optional private CA, client certificate, and client key settings.
- Keep CLI-over-config precedence and the existing HTTP/public HTTPS defaults.
- Do not add an insecure TLS bypass, custom headers, named connection profiles,
  or hot reload in this slice.
- Never expose certificate or key contents in diagnostics.
- Partially port the useful behavior and fixtures from the fork's mTLS work,
  while simplifying the implementation to fit Grafatui's configuration model.

**Exit criteria:** public HTTPS remains unchanged; private-CA and mTLS paths are
covered by tests; invalid and incomplete settings fail with actionable,
secret-safe errors.

### 0.1.16 - Variable Correctness (released)

- Import Classic and schema-v2 variable refresh settings.
- Distinguish initial-load, time-range/manual, and periodic refresh behavior.
- Preserve the last selected value when a variable refresh fails, continue
  resolving later variables, and refresh dependent variables in order.
- Keep `label_values(...)` requests bounded and preserve scoped row/tab
  variables, repeats, and conditional rendering.
- Partially port the fork's variable semantics, excluding its background
  refresh coupling. A variable picker UI remains out of scope.

**Exit criteria:** refresh modes and dependency ordering have fixture coverage;
one failed variable cannot invalidate otherwise usable dashboard state.

### 0.1.17 - Value Mappings (released)

- Support exact numeric `value` mappings with mapped text; defer range, regex,
  special-value, and mapping-color behavior.
- Preserve raw numeric values for calculations and apply mapped text only at
  the display layer.
- Use one mapping path across Stat, Gauge, BarGauge, Table, applicable legends,
  and exports, while preserving `noValue` fallback behavior.
- Report unsupported mapping kinds through import diagnostics.
- Implement this cleanly in Grafatui; the fork does not provide a suitable
  implementation to port.

**Exit criteria:** shared mapping tests cover classic and v2 parsing,
precedence, calculations, panel rendering, exports, fallbacks, and
unsupported-kind diagnostics.

### 0.1.18 - Multiple Dashboards and Summary Accuracy (released)

- Accept multiple dashboard files and show independently refreshing dashboards
  in top-level tabs, distinct from imported Grafana layout tabs.
- Show configured no-value text for null Gauge samples rather than an empty
  numeric fill.
- Import one supported `reduceOptions` calculation for Classic/V2 Stat, Gauge,
  and BarGauge panels. Explicit target query modes still win; unsupported
  settings retain defaults and produce diagnostics.

### 0.1.19 - Dashboard Readability (proposed)

These are candidates, not a promise to ship every item. Design and review each
as its own PR; defer a slice if its Grafana semantics or regression evidence are
not ready.

1. **Display names:** import a useful documented subset of
   `fieldConfig.defaults.displayName` for series labels without changing query
   identity. Define supported expressions and fallback behavior before coding.
2. **Broader unit formatting:** add common duration and data-rate units selected
   from real dashboard fixtures. Preserve the existing compact fallback for
   unknown units and keep formatting consistent across TUI and exports.
3. **Numeric range value mappings:** extend the exact-value mapping path to
   numeric ranges and mapped text. Define overlap, boundary, and precedence
   rules before implementation; regex, special-value, and mapping-color modes
   remain separate work.

**Release gate:** Classic and V2 fixtures, unsupported-setting diagnostics,
render-buffer and export assertions, full tests, strict linting, and updated
compatibility notes. Existing dashboards without these fields must keep their
current behavior.

### Continuous Maintenance

- Fix export clipping only when a current-main reproduction demonstrates it;
  do not import the fork's wholesale export rewrite.
- Add CI checks incrementally: formatting and linting first, then MSRV and
  platform coverage.
- Audit repeated-panel storage with churn and soak tests before applying any
  memory-lifetime fix from the fork.
- Keep one feature per branch and pull request, with implementation and
  documentation separated into clear commits.

### Deferred Queue

- **Broader background scheduling:** beyond the shipped concurrent dashboard
  refresh, requires a generation, cancellation, and stale-result design before
  changing query or provider lifecycles.
- **Custom themes and a live theme picker:** lower priority while the shipped
  theme set covers common needs.
- **Project-wide rebranding, package-manager removal, and documentation-site
  migration:** explicitly outside this roadmap.

---

## Compatibility Ladder

This is the main backlog, ordered by Grafana parity domain.

### 1. Import Diagnostics & Trust

| Feature | Grafana field / behavior | User value | Complexity | Status |
|---|---|---|---|---|
| **Compatibility matrix automation** | Documentation generated or checked against current code | Keeps users and contributors aligned with reality | 🟢 | 📋 |
| **Unsupported panel warnings** | Unsupported `panels[].type` and ignored high-impact fields | Makes import degradation visible instead of silent | 🟢 | ✅ |
| **Import validation** | `--validate`, `--strict`, and JSON import checks | Lets users check the supported dashboard subset before launching the TUI | 🟢 | ✅ |
| **Better JSON/import errors** | Parse errors with path/context where possible | Faster debugging for broken exports | 🟢 | 📋 |
| **Variable substitution diagnostics** | Missing variables, unsupported format modifiers | Explains empty panels caused by unresolved variables | 🟢 | ✅ |

### 2. Field Config Parity

| Feature | Grafana field / behavior | User value | Complexity | Status |
|---|---|---|---|---|
| **Broader unit formatting** | `fieldConfig.defaults.unit` | Extends the shipped common-unit subset to more Grafana units | 🟡 | 🔜 Proposed |
| **Additional no-value coverage** | `fieldConfig.defaults.noValue` | Audit remaining gaps after the 0.1.18 Gauge null-value fix | 🟢 | 📋 |
| **Value mappings** | `fieldConfig.defaults.mappings` | Exact numeric values render mapped text; numeric ranges are proposed next | 🟡 | 🔶 Partial |
| **Numeric range mappings** | `fieldConfig.defaults.mappings` with `type: range` | Adds mapped text for bounded numeric intervals | 🟡 | 🔜 Proposed |
| **Display names** | `fieldConfig.defaults.displayName` | Series and table labels better match Grafana naming | 🟡 | 🔜 Proposed |
| **Color mode subset** | `fieldConfig.defaults.color` | Honors configured color intent where terminal rendering permits | 🟡 | 📋 |
| **Field overrides subset** | `fieldConfig.overrides` | Per-series units/names/colors for common matcher types | 🔴 | 💡 |

### 3. Panel Options Parity

| Feature | Grafana field / behavior | User value | Complexity | Status |
|---|---|---|---|---|
| **Reduce options** | `options.reduceOptions.calcs` | Stat/Gauge/BarGauge support one of last, lastNotNull, min, max, mean, or total; other modes warn | 🟡 | 🔶 Partial |
| **Legend display mode** | `options.legend.displayMode` | Hide/list/table modes map to compact TUI equivalents | 🟡 | 📋 |
| **Legend placement** | `options.legend.placement` | Bottom/right placement influences terminal layout where useful | 🟡 | 📋 |
| **Legend calculations** | `options.legend.calcs` | Min/max/avg/current values appear beside series names | 🟡 | 📋 |
| **Text/graph/color modes** | Stat and gauge display options | Imported summary panels better match Grafana intent | 🟡 | 📋 |
| **Tooltip behavior mapping** | `options.tooltip` | Documented mapping to inspect mode, with useful defaults | 🟢 | 📋 |

### 4. Target & Query Parity

| Feature | Grafana field / behavior | User value | Complexity | Status |
|---|---|---|---|---|
| **Hidden targets** | `targets[].hide` | Helper queries do not clutter imported panels | 🟢 | ✅ |
| **Instant query defaults** | Panel-specific fallback behavior when `targets[].instant` is omitted | Keeps summary panels fast while preserving range queries for charts | 🟢 | ✅ |
| **Target interval** | `targets[].interval` / `intervalFactor` | Panel-specific resolution is respected; `interval` is supported, `intervalFactor` is not | 🟡 | ✅ |
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
| **Draw styles** | `fieldConfig.defaults.custom.drawStyle` | Supported line, bars, and points map to distinct terminal renderings | 🟡 | 🔶 Partial |
| **Stacking** | `fieldConfig.defaults.custom.stacking` | Supported stacking modes retain area/bar intent | 🟡 | 🔶 Partial |
| **Axis labels** | `fieldConfig.defaults.custom.axisLabel` | Imported axis meaning is visible where space allows | 🟢 | 📋 |
| **Axis placement** | `fieldConfig.defaults.custom.axisPlacement` | Supported left/right/hidden axis settings map to TUI behavior | 🟡 | 🔶 Partial |
| **Scale distribution** | `fieldConfig.defaults.custom.scaleDistribution` | Linear/log choices are respected or explicitly warned | 🟡 | 💡 |

### 7. Panel Type Parity

| Feature | Grafana panel type | User value | Complexity | Status |
|---|---|---|---|---|
| **Row headers** | `row` | Dashboard sections stay recognizable | 🟢 | ✅ |
| **Collapsed rows** | `row.collapsed` | Large dashboards can start folded | 🟡 | ✅ |
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

### 9. Grafana Dashboard Schema v2

The first v2 milestone is format compatibility: accept JSON resources with the
exact `dashboard.grafana.app/v2` API version and map the subset that has an
existing Grafatui equivalent. Unsupported v2-only layout semantics must fail
with a clear import error rather than silently changing the dashboard.

| Feature | Grafana field / behavior | User value | Complexity | Status |
|---|---|---|---|---|
| **V2 Resource JSON compatibility** | `apiVersion: dashboard.grafana.app/v2` with a `GridLayout` | Imports Grafana 13's default JSON format without requiring a Classic export | 🔴 | ✅ |
| **Rows layout** | `RowsLayout` and nested row layouts | Preserves dashboard grouping and collapsed sections | 🔴 | ✅ |
| **Tabs layout** | `TabsLayout` and nested tabs | Preserves tabbed dashboard organization | 🔴 | ✅ |
| **Auto-grid fundamentals** | Static `AutoGridLayout` | Responsive ordered placement, fixed heights, nested rows/tabs, navigation and exports | 🟡 | ✅ Stage 1 implemented |
| **Auto-grid viewport filling** | `fillScreen` | Minimum-preserving row growth, nested allocation, resize and exports | 🟢 | ✅ Stage 2 implemented |
| **Auto-grid content fitting** | Table `fitContent`, minima and matching | Unbounded measured tables, item overrides, refresh reconciliation and shared export windows | 🟢 | ✅ Stage 3a implemented |
| **Auto-grid bounded content** | Maximum heights, body scrolling and error fitting | Completes content sizing and capped content access | 🟢 | ✅ Stage 3b implemented |
| **AutoGrid item repeats** | `AutoGridLayoutItem.repeat` | Expands panels from selected variable values with per-instance scope | 🔴 | ✅ |
| **Repeated rows, tabs, and grids** | Container and fixed-grid repeat settings | Expands structural groups from variable values | 🔴 | 📋 |
| **AutoGrid item conditions** | `AutoGridLayoutItem.conditionalRendering` | Shows or hides panels using variable, query-data, and time-range conditions | 🔴 | ✅ |
| **Row/tab conditions** | Container `conditionalRendering` | Shows or hides structural groups using v2 conditions | 🔴 | 📋 |
| **Nested layout variables** | Variables scoped to rows and tabs | Preserves local variable scope for descendant queries, AutoGrid item repeats, and predicates | 🔴 | 🔶 Partial |
| **Library panel resolution** | `LibraryPanel` element references | Imports reusable panels by resolving their external definitions | 🔴 | 📋 |
| **V2 Resource YAML** | YAML representation of the v2 resource | Supports Grafana's alternative as-code export format | 🟡 | ✅ |

AutoGrid stages 1–3 cover static sizing, `fillScreen`, and content
fitting/bounds/row matching/body scrolling. Item repeats and item conditions
use the supported nested variable scopes. Repeated structural containers and
row/tab conditional rendering remain separate compatibility work, so this does
not claim full V2 layout compatibility.

---

## Longer-Term Themes

The compatibility ladder defines the backlog. These themes group related work;
their headings are not promised release versions. A focused `0.1.x` slice may
ship an item from any theme when its scope and evidence are ready.

### Grafana Import Fidelity

Goal: imported dashboards should be more readable and less silently degraded.

| Item | Why it belongs here | Complexity | Status |
|---|---|---|---|
| Value mappings | Exact numeric text is shipped; range and other kinds remain | 🟡 | 🔶 Partial |
| Broader unit formatting | Expands the shipped common-unit subset to more Grafana dashboards | 🟡 | 🔜 Proposed |
| Additional no-value coverage | Completes the shipped fallback behavior where terminal rendering can use it | 🟢 | 📋 |
| Hidden targets | Prevents helper queries from appearing as normal series | 🟢 | ✅ |
| Unsupported panel warnings | Builds trust in imported results | 🟢 | ✅ |
| `--validate` import diagnostics | Gives users text, JSON, and strict non-interactive checks for the supported subset | 🟢 | ✅ |

### Panel Semantics

Goal: stat, gauge, table, and legend behavior should match common Grafana
expectations.

| Item | Why it belongs here | Complexity | Status |
|---|---|---|---|
| Reduce options | One supported calculation per summary series is shipped; other modes remain | 🟡 | 🔶 Partial |
| Display names | Imported labels become clearer without changing queries | 🟡 | 🔜 Proposed |
| Legend display modes and placement | Dense dashboards need predictable legend behavior | 🟡 | 📋 |
| Legend calculations | Adds useful table-like summaries without a new panel type | 🟡 | 📋 |
| Target interval support | Respects panel-specific query resolution | 🟡 | ✅ |

### Graph & Timeseries Fidelity

Goal: graph and timeseries panels should preserve more visual intent within TUI
constraints.

| Item | Why it belongs here | Complexity | Status |
|---|---|---|---|
| Draw styles | Supported bars/points/lines are distinguishable | 🟡 | 🔶 Partial |
| Stacking | Supported Grafana area/bar semantics are visible | 🟡 | 🔶 Partial |
| Axis labels and placement | Placement is partly supported; labels remain future work | 🟡 | 🔶 Partial |
| Scale distribution handling | Honor or warn on log/non-linear scales | 🟡 | 💡 |
| Row headers and collapsed rows | Keeps large dashboard structure intact | 🟡 | ✅ |

### Exploration Workflow

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

---

## Best Next Steps

Recommended order for the next focused development cycle:

1. Choose real Classic/V2 fixtures for the proposed 0.1.19 display-name, unit,
   and numeric-range-mapping slices; verify each mapping can be represented
   without changing query semantics or existing defaults.
2. Design and review each slice separately, then implement it in its own PR
   with importer, TUI, export, diagnostic, and regression coverage.
3. Defer any slice whose behavior or verification is not ready; do not hold a
   completed slice hostage to the rest of the bundle.
4. Update the compatibility matrix with each merged subset, then review the
   generated release changelog and assets for the eventual 0.1.19 release.
5. Keep auditing fork features individually, reusing code only where it meets
   current quality and compatibility requirements.

---

## How to Contribute

If you'd like to help, the best starting points are items marked 🔜 and 🟢.
Roadmap items are especially useful when PRs include:

- A small Grafana JSON fixture that demonstrates the supported field.
- Unit tests for parsing and display behavior.
- A short note in `docs/grafana-compatibility.md` explaining the TUI mapping or
  limitation.

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines, and
[docs/grafana-compatibility.md](docs/grafana-compatibility.md) for the full Grafana JSON
feature-parity breakdown.
