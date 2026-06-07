# Examples

The repository includes example Grafana dashboards and a local demo environment.

## Demo Stack

Start Prometheus, node-exporter, and mock vLLM metrics:

```bash
cd examples/demo
docker-compose up -d
```

Run Grafatui from the repository root:

```bash
cargo run -- --grafana-json examples/dashboards/prometheus_demo.json --prometheus-url http://localhost:19090
```

Stop the demo:

```bash
cd examples/demo
docker-compose down -v
```

## Included Dashboards

- `examples/dashboards/prometheus_demo.json`: recommended first demo for the bundled Prometheus stack.
- `examples/dashboards/all_visualizations.json`: compact dashboard showing the supported visualization types.
- `examples/demo/vllm/grafana.json`: vLLM-oriented dashboard for the mock demo services.

## More Detail

See the repository example docs:

- [examples/README.md](https://github.com/fedexist/grafatui/blob/main/examples/README.md)
- [examples/demo/README.md](https://github.com/fedexist/grafatui/blob/main/examples/demo/README.md)
