# Introduction

Grafatui is a terminal user interface for Prometheus dashboards. It is designed for fast inspection, SSH sessions, local debugging, and environments where opening a browser-based Grafana instance is inconvenient.

Grafatui reads Prometheus directly and can import Grafana dashboard JSON files. It renders supported panels as terminal charts, tables, gauges, stats, and heatmaps while keeping the workflow keyboard-first.

## When Grafatui Fits

Use Grafatui when you want:

- A lightweight Prometheus dashboard in your terminal.
- A familiar way to inspect exported Grafana dashboards.
- Fast startup and low resource usage.
- A dashboard that works well over SSH.
- SVG or PNG snapshots of the current TUI view.

Grafatui is not a Grafana server replacement. It does not manage users, alerts, annotations, dashboard editing, plugins, or browser-only visualizations.

## Project Links

- [Repository](https://github.com/fedexist/grafatui)
- [Crate](https://crates.io/crates/grafatui)
- [Rust API docs](https://docs.rs/grafatui)
- [Grafana compatibility matrix](grafana-compatibility.md)
