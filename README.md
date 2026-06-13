# Grafatui

[![CI](https://github.com/fedexist/grafatui/workflows/CI/badge.svg)](https://github.com/fedexist/grafatui/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/grafatui.svg)](https://crates.io/crates/grafatui)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust Version](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)
[![docs.rs](https://img.shields.io/docsrs/grafatui)](https://docs.rs/grafatui)

**Grafatui** is a terminal user interface for Prometheus, inspired by Grafana. It lets you inspect time-series dashboards from a fast, keyboard-driven TUI that works well over SSH and in minimal environments.

[![asciicast](https://asciinema.org/a/vMRNEjG0FEDKGP31.svg)](https://asciinema.org/a/vMRNEjG0FEDKGP31)

## Quick Start

Install from crates.io:

```bash
cargo install grafatui
```

Run against a Prometheus instance:

```bash
grafatui --prometheus-url http://localhost:9090
```

Or try the included demo:

```bash
git clone https://github.com/fedexist/grafatui.git
cd grafatui
cd examples/demo && docker-compose up -d && sleep 5 && cd ../..
cargo run -- --grafana-json examples/dashboards/prometheus_demo.json --prometheus-url http://localhost:19090
```

## Features

- Prometheus range and instant queries with async fetching.
- Grafana dashboard JSON import for graph, timeseries, stat, gauge, bar gauge, table, and heatmap panels.
- Template variables, Grafana built-in PromQL variables, legend formatting, thresholds, and grid layout support.
- Keyboard-first navigation, panel search, fullscreen mode, mouse selection, and value inspection.
- SVG/PNG export and changed-frame recording bundles.
- TOML configuration and built-in themes.

## Documentation

- [User guide](https://fedexist.github.io/grafatui/)
- [Installation](https://fedexist.github.io/grafatui/installation.html)
- [Quick start](https://fedexist.github.io/grafatui/quick-start.html)
- [Configuration](https://fedexist.github.io/grafatui/configuration.html)
- [Grafana dashboard import](https://fedexist.github.io/grafatui/grafana-dashboard-import.html)
- [Grafana compatibility matrix](https://fedexist.github.io/grafatui/grafana-compatibility.html)
- [Examples](examples/README.md)

Rust API documentation is available on [docs.rs](https://docs.rs/grafatui).

## Common Commands

```bash
# Import a Grafana dashboard
grafatui --prometheus-url http://localhost:9090 --grafana-json ./dashboard.json

# Override Grafana template variables
grafatui --grafana-json ./dash.json --var job=node --var instance=server-01

# Use a theme
grafatui --theme tokyo-night

# Generate shell completions or a man page
grafatui completions zsh
grafatui man
```

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines and [DEVELOPMENT.md](DEVELOPMENT.md) for local development notes.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

Copyright 2025 Federico D'Ambrosio
