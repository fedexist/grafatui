# Troubleshooting

## Prometheus Connection Refused

Check that Prometheus is running and reachable:

```bash
curl http://localhost:9090/-/healthy
```

If you are using the demo stack, the Prometheus port is `19090`:

```bash
curl http://localhost:19090/-/healthy
```

## TLS Configuration Errors

Grafatui validates configured certificate files at startup. If startup fails:

- use an `https://` Prometheus URL whenever a TLS option is set;
- set `client_cert` and `client_key` together;
- verify that the configured paths are readable PEM files and that the client
  private key is unencrypted;
- verify that `client_key` belongs to the first certificate in `client_cert`.

For a private-CA server, inspect the server chain and the configured CA bundle:

```bash
openssl s_client -connect prometheus.internal:9090 -showcerts </dev/null
openssl x509 -in ca.pem -noout -subject -issuer -enddate
```

For mutual TLS, verify the client certificate and key match:

```bash
openssl x509 -in client.pem -pubkey -noout | openssl pkey -pubin -outform DER | openssl sha256
openssl pkey -in client.key -pubout -outform DER | openssl sha256
```

Grafatui reports file paths and validation failures, but never certificate or
private-key contents.

## No Data Appears

Prometheus may need a few scrape intervals before data is available. Wait 10 to 15 seconds and force a refresh with `r`.

Also confirm that the dashboard queries match labels in your Prometheus server:

```bash
grafatui --prometheus-url http://localhost:9090 --grafana-json ./dashboard.json --var job=prometheus
```

## Dashboard Variables Do Not Match

Override variables explicitly with `--var`:

```bash
grafatui --grafana-json ./dashboard.json --var instance=localhost:9090
```

If a Grafana dashboard uses multi-select formatting modifiers such as `${var:csv}` or `${var:regex}`, check the [compatibility matrix](grafana-compatibility.md). Not every Grafana interpolation mode is implemented.

## Stale Data and Query Warnings

Grafatui keeps the last successful series for a query when a later refresh
fails. The panel remains readable and its title shows `stale` when every
displayed query is stale, or `partial` when fresh and stale query results are
mixed. Select the panel to see the query error in the footer.

A `warning` marker means Prometheus returned data with a warning or info
message. A later successful refresh clears both the marker and its detail.

These markers are also included in SVG and PNG export titles so snapshots do
not present stale or incomplete data as fully current.

## Demo Port Conflict

The demo Prometheus service uses host port `19090`. If that port is already in use, edit `examples/demo/docker-compose.yml` and run Grafatui with the updated URL.

## Export Directory Problems

Set an explicit export directory:

```bash
grafatui --export-dir ./grafatui-exports
```

Make sure the directory is writable by your current user.
