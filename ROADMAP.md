# Grafatui Roadmap

Grafatui is a terminal-based Grafana-like UI for Prometheus. This roadmap outlines what's been built, what's coming next, and longer-term goals. It's intended to help contributors find impactful areas to work on and to set expectations for users.

> **Current version**: 0.1.5 · **Status**: Active development, pre-1.0

**Legend**:
- 🟢 Low complexity · 🟡 Medium complexity · 🔴 High complexity
- ✅ Shipped · 🔜 Up next · 📋 Planned · 💡 Exploring

---

## What's Already Built (v0.1.x)

These features are shipped and available today:

| Feature | Details |
|---|---|
| **6 panel types** | `graph`, `timeseries`, `stat`, `gauge`, `bargauge`, `table`, `heatmap` |
| **Grafana JSON import** | Load dashboards from exported JSON files |
| **24-column grid layout** | Faithful reproduction of Grafana's `gridPos` positioning |
| **Template variables** | `$var` / `${var}` substitution with CLI and config overrides |
| **Legend formatting** | `{{label}}` syntax from Grafana |
| **8 color themes** | default, dracula, monokai, solarized (dark/light), gruvbox, tokyo-night, catppuccin |
| **Time controls** | Zoom in/out, pan left/right, live mode toggle |
| **Panel navigation** | Arrow keys, vim-style `j`/`k`, fullscreen, inspect mode |
| **Panel search** | `/` to fuzzy-search panels by name |
| **Mouse support** | Click to select, scroll, drag cursor in fullscreen |
| **Value inspection** | Cursor-based point-in-time data exploration |
| **Series toggling** | Show/hide individual series with `1`–`9` |
| **Smart caching** | Request deduplication and caching for identical queries |
| **Downsampling** | Max-pooling to ~200 points to preserve peaks |
| **Config file** | TOML-based persistent configuration |
| **`$__rate_interval`** | Automatic computation as `max(step × 4, 60s)` |
| **Thresholds** | `fieldConfig.defaults.thresholds` for graph limit lines and dynamic Stat/Gauge/BarGauge coloring |
| **Threshold marker styles** | Configurable marker styles including dashed line, braille, block, quadrant, sextant, and octant modes |
| **Field min/max bounds** | `fieldConfig.defaults.min` / `max` for gauge scaling and percentage threshold interpolation |
| **Export to SVG/PNG** | Native SVG dashboard snapshots, PNG rasterization, and recording frame bundles |
| **Shell completions** | Bash, Zsh, Fish, PowerShell, Elvish |
| **Cross-platform binaries** | Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows (x86_64) |
| **Package formats** | `.deb`, `.rpm`, Homebrew formula |

For a field-by-field breakdown of Grafana JSON compatibility, see [GRAFANA_COMPATIBILITY.md](GRAFANA_COMPATIBILITY.md).

---

## Up Next — Grafana JSON Compatibility

| Feature | Description | Complexity | Status |
|---|---|---|---|
| **Unit formatting** | Display values as bytes, percent, duration, etc. (`fieldConfig.defaults.unit`) | 🟡 | 🔜 |
| **Unsupported panel warnings** | Surface clear warnings for panel types that can't be rendered | 🟢 | 🔜 |
| **Value mappings** | Map numeric values to text labels (`fieldConfig.defaults.mappings`) | 🟡 | 📋 |
| **Reduce options** | Support `calcs` other than "last" for stat/gauge panels | 🟡 | 📋 |
| **Legend configuration** | Respect `options.legend` display mode, placement, and calcs | 🟡 | 📋 |
| **Additional PromQL variables** | `$__interval`, `$__range`, `$__range_s` | 🟢 | 📋 |
| **Dynamic template variables** | Query Prometheus for variable values (`type: "query"`) | 🟡 | 📋 |
| **Draw styles** | Respect `bars`, `points`, `line` from `fieldConfig.defaults.custom.drawStyle` | 🟡 | 📋 |
| **Stacking** | Stacked area/bar charts from `fieldConfig.defaults.custom.stacking` | 🟡 | 📋 |

---

## Import & Debugging

| Feature | Description | Complexity | Status |
|---|---|---|---|
| **Import validation** | `--validate` flag to check JSON before loading | 🟢 | 🔜 |
| **Better parse errors** | Line numbers and context when JSON parsing fails | 🟢 | 📋 |
| **Variable substitution debugging** | Show which variables failed to resolve | 🟢 | 📋 |

---

## Navigation & UX

| Feature | Description | Complexity | Status |
|---|---|---|---|
| **Fuzzy finder** | `Ctrl+P` style quick switcher for dashboards/panels | 🟢 | 🔜 |
| **Bookmarks** | Save dashboard + time range combos | 🟢 | 📋 |
| **Panel history** | Recently viewed panels for quick access | 🟢 | 📋 |
| **Split view** | Compare two panels side-by-side | 🟡 | 💡 |

---

## Data Export & Sharing

| Feature | Description | Complexity | Status |
|---|---|---|---|
| **Export to CSV** | Dump metric data to file | 🟢 | 🔜 |
| **Copy to clipboard** | Copy panel data for reports | 🟢 | 📋 |
| **Share snapshot** | Generate portable single-file/share-link snapshots | 🟡 | 💡 |

---

## Query & Exploration

| Feature | Description | Complexity | Status |
|---|---|---|---|
| **Ad-hoc query mode** | Scratch panel for one-off PromQL queries | 🟡 | 📋 |
| **Label explorer** | Browse available labels and values | 🟡 | 📋 |
| **PromQL autocomplete** | Suggest metrics, labels, functions | 🔴 | 💡 |

---

## Multi-Source Support

| Feature | Description | Complexity | Status |
|---|---|---|---|
| **Grafana API connection** | Pull dashboards live from a Grafana server | 🟡 | 📋 |
| **Multiple Prometheus sources** | Switch between clusters/environments | 🟡 | 📋 |
| **InfluxDB / Loki support** | Expand beyond Prometheus | 🔴 | 💡 |

---

## Alerting & Notifications

| Feature | Description | Complexity | Status |
|---|---|---|---|
| **Alert rule viewer** | Dedicated panel for Prometheus/Alertmanager alerts | 🟡 | 📋 |
| **Alert silence creation** | Quick-silence alerts from TUI | 🟡 | 💡 |
| **Desktop notifications** | Push notifications when thresholds are crossed | 🟡 | 💡 |

---

## Dashboard Editing

| Feature | Description | Complexity | Status |
|---|---|---|---|
| **Editor mode** | Edit panel queries inline with nano/vim-style controls | 🟡 | 💡 |
| **Dashboard creation wizard** | Step-by-step TUI flow to create new dashboards | 🔴 | 💡 |

---

## Operational Features

| Feature | Description | Complexity | Status |
|---|---|---|---|
| **File watch mode** | Auto-reload on config/JSON file changes | 🟢 | 📋 |
| **Session restore** | Remember last dashboard and time range | 🟢 | 📋 |
| **SSH tunnel mode** | Built-in port forwarding for remote Prometheus | 🟡 | 💡 |

---

## Accessibility & Customization

| Feature | Description | Complexity | Status |
|---|---|---|---|
| **Colorblind palettes** | Alternative color schemes | 🟢 | 📋 |
| **Custom keybindings** | User-remappable shortcuts | 🟡 | 📋 |
| **Panel annotations** | Show deployment markers/events on graphs | 🟡 | 💡 |

---

## Priorities at a Glance

### 🔜 Up Next
High-value items actively being considered for the next release(s):
- **Unit formatting** - Display values in human-readable units
- **Import validation & panel warnings** - Better error experience
- **Export to CSV** - Simple, high-value data export
- **Fuzzy finder** - Quality-of-life navigation improvement

### 📋 Near-term
Planned features that will meaningfully improve the user experience:
- Value mappings, reduce options, legend configuration
- Additional PromQL variables (`$__interval`, `$__range`)
- Dynamic template variables from Prometheus
- File watch mode, session restore, bookmarks
- Colorblind palettes, custom keybindings

### 💡 Longer-term
Ambitious features under exploration:
- Grafana API live connection
- InfluxDB/Loki support
- PromQL autocomplete
- Dashboard editor mode
- Desktop notifications

---

## How to Contribute

If you'd like to help, the best starting points are items marked 🔜 and 🟢. These tend to be self-contained and well-defined.

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines, and [GRAFANA_COMPATIBILITY.md](GRAFANA_COMPATIBILITY.md) for the full Grafana JSON feature-parity breakdown.
