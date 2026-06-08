# Sōzu Prometheus connector

Receives HTTP GET requests on a `/metrics` route, forwards the request to Sōzu,
packages the responses in a Prometheus format and sends them back in a HTTP response.

## Configure

The connector is configured through a TOML file (see [`example.config.toml`](./example.config.toml)).
It is loaded either from the path given with `--config`, or from the first of the
following locations that exists: `/usr/share/sozu-prometheus-connector/config`,
`/etc/sozu-prometheus-connector/config`, `$HOME/.config/sozu-prometheus-connector/config`,
`$HOME/.local/share/sozu-prometheus-connector/config`, or `config` in the working directory.

```toml
# Socket address on which to listen. Must be parsable to a SocketAddr.
listening-address = "0.0.0.0:3000"

# Emit per-worker metric series (labelled with `worker_id`) in addition to the
# aggregated ones. Optional, defaults to false (see "Per-worker metrics" below).
# per-worker-metrics = false

[sozu]
# Path to Sōzu's configuration file. It is parsed to find the unix command
# socket on which to query Sōzu.
configuration = "/path/to/sozu/on/the/machine/config.toml"

# Optional: forward errors to a Sentry/GlitchTip endpoint.
# [sentry]
# dsn = "https://..."
# env = "production"
```

## Per-worker metrics

By default the connector exports only the metrics Sōzu aggregates across all of
its workers (the `main`, `proxying`, and per-`cluster_id`/`backend_id` series),
which keeps cardinality low and the output stable across releases.

Setting `per-worker-metrics = true` additionally requests the per-worker
breakdown from Sōzu and exports it:

- worker proxy metrics are emitted under a distinct `_worker`-suffixed family,
  labelled with `worker_id` (e.g. `bytes_in_worker{worker_id="0"}`), so they
  never share a metric name with the aggregated `_total` series;
- per-worker cluster and backend metrics keep the aggregated metric name and
  gain a `worker_id` label on top of `cluster_id` (and `backend_id`):

```
# aggregated (always present): no worker_id label
requests{cluster_id="MyCluster"} 1000
requests{cluster_id="MyCluster",backend_id="the-backend"} 500

# per-worker (only when per-worker-metrics = true): worker_id label present
requests{worker_id="0",cluster_id="MyCluster"} 600
requests{worker_id="0",cluster_id="MyCluster",backend_id="the-backend"} 300
```

Because the aggregated and per-worker cluster/backend series share a metric name
and differ only by the presence of the `worker_id` label, select one or the
other in PromQL to avoid double-counting:

```promql
# aggregated only (across all workers)
sum(requests{worker_id=""})

# per-worker only
sum by (worker_id) (requests{worker_id!=""})
```

## How to test

1. Run Sōzu on your machine
2. Run `sozu-prometheus-connector` with `cargo run -- --config config.toml`
3. Query the URL `http://127.0.0.1:3000/metrics` (e.g. with `curl` or a browser)

The prometheus-formatted metrics should appear in the response.
