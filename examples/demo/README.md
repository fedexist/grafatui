# Grafatui Demo Setup

Get started with grafatui in less than a minute using this pre-configured Prometheus + node-exporter stack.

## Quick Start (One-Liner)

```bash
docker-compose up -d && sleep 5 && cargo run -- --grafana-json ../dashboards/prometheus_demo.json --prometheus http://localhost:10001
```

> **Note**: The docker-compose is configured to use port **10001** to avoid conflicts with development tools and other services that commonly use 9090-9092.

This will:
1. Start Prometheus (port **10001**) and node-exporter (port 9100)
2. Wait 5 seconds for metrics to populate
3. Launch grafatui with the demo dashboard

## What's Included

**Services:**
- **Prometheus**: Scrapes itself and node-exporter every 5 seconds (port **10001**)
- **node-exporter**: Exposes system metrics (CPU, memory, network, etc.) (port 9100)

**Dashboard (`prometheus_demo.json`):**
All 6 visualization types, optimized for their use cases:

1. **Graph** - HTTP request rate by status code
2. **Gauge** - Current memory usage
3. **Stat** - Uptime with trend sparkline
4. **Graph** - Network receive rate per interface
5. **Bar Gauge** - Active targets count
6. **Table** - Target health status
7. **Heatmap** - HTTP request duration distribution
8. **Graph** - TSDB chunk creation rate

## Manual Setup

```bash
# Start the stack
cd examples/demo
docker-compose up -d

# Verify Prometheus is running
curl http://localhost:8585/-/healthy

# Run grafatui (from repo root)
cd ../..
cargo run -- --grafana-json examples/dashboards/prometheus_demo.json --prometheus http://localhost:8585

# Optional: customize time range and step
cargo run -- --grafana-json examples/dashboards/prometheus_demo.json --prometheus http://localhost:8585 --range 5m --step 2s
```

## Cleanup

```bash
cd examples/demo
docker-compose down -v  # -v removes volumes (data)
```

## Troubleshooting

**"Connection refused" errors:**
- Wait a few seconds after `docker-compose up` for services to start
- Check services: `docker-compose ps`
- Check logs: `docker-compose logs prometheus`

**No data showing:**
- Prometheus needs ~10-15 seconds to scrape and build TSDB
- Try: `docker-compose restart prometheus`

**Port already in use:**
- Change port in `docker-compose.yml`: `"9091:9090"` (use 9091 instead)
- Then run grafatui with: `--prometheus http://localhost:9091`
