# Development Notes

This document contains useful information for developing and testing grafatui.

## Testing Visualizations

### Using the Example Dashboard

The `examples/dashboards/all_visualizations.json` file demonstrates all supported panel types. To test it:

**Option 1: With a Running Prometheus Instance**
```bash
# If you have Prometheus running locally
cargo run -- --grafana-json examples/dashboards/all_visualizations.json

# Or connect to a remote Prometheus
cargo run -- --grafana-json examples/dashboards/all_visualizations.json --prometheus http://your-prometheus:9090
```

**Option 2: Quick Local Test with Prometheus**
```bash
# Download and run Prometheus locally for testing
docker run -p 9090:9090 prom/prometheus

# Then in another terminal:
cargo run -- --grafana-json examples/dashboards/all_visualizations.json
```

### Panel Types Tested

The example dashboard includes:
1. **Graph** - Time-series line chart
2. **Gauge** - Single value progress bar
3. **Stat** - Big number + sparkline
4. **Bar Gauge** - Comparative bars
5. **Table** - Tabular data view
6. **Heatmap** - Color-coded intensity grid

### Creating Test Dashboards

When creating test dashboards for new features:
1. Place them in `examples/dashboards/`
2. Name them descriptively (e.g., `feature_name_test.json`)
3. Document them in `examples/README.md`
4. Use realistic Prometheus queries that work with `prometheus_http_*` metrics

## Project Structure

```
grafatui/
├── src/               # Source code
│   ├── main.rs       # CLI entry point
│   ├── app.rs        # Application state & logic
│   ├── ui.rs         # UI rendering & visualizations
│   ├── prom.rs       # Prometheus client
│   ├── grafana.rs    # Grafana JSON parser
│   ├── config.rs     # Configuration
│   └── theme.rs      # Color themes
├── examples/         # Example dashboards & demos
│   ├── dashboards/   # Grafana JSON files
│   └── README.md     # Examples documentation
└── target/           # Build artifacts (gitignored)
```

## Running Tests

```bash
# Run all unit tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_calculate_y_bounds

# Build optimized release
cargo build --release
```

## Code Style

- Use `rustfmt` for formatting: `cargo fmt`
- Check lints: `cargo clippy`
- Document public APIs with doc comments (`///`)
- Keep functions focused and under 100 lines when possible
