# Sōzu Prometheus connector

Receives HTTP GET requests on a `/metrics` route, forwards the request to Sōzu,
packages the responses in a Prometheus format and sends them back in a HTTP response.

## Configure

In the `config.toml` of this repository,
you have to indicate the absolute path to the configuration file
of the Sōzu that runs on the machine.

```toml
sozu_configuration_path = "/path/to/sozu/on/the/machine/config.toml"
```

The Sōzu config file will be parsed to find to unix socket on which to write requests to Sōzu.

You will have to provide a socket address on which the prometheus connector will
wait for HTTP requests on the `/metrics` path.

```toml
# address on which to listen. Must be parsable to the SocketAddr type
listening_address = "0.0.0.0:3000"
```

you can also chose to flatten all metric and to compound them if they have the same name,
turning thig:

```
# TYPE bytes_out counter
bytes_out{worker="0"} 246
bytes_out{cluster_id="MyCluster",backend_id="the-backend-to-my-app"} 250
bytes_out{cluster_id="MyCluster",backend_id="the-backend-to-my-app-2"} 250
bytes_out{worker="1"} 246
bytes_out{cluster_id="MyCluster",backend_id="the-backend-to-my-app"} 250
bytes_out{cluster_id="MyCluster",backend_id="the-backend-to-my-app-2"} 250
```

into this:

```
# TYPE bytes_out counter
bytes_out{} 492
bytes_out{cluster_id="MyCluster"} 1000
```

You can do this with:

```toml
aggregate-backend-metrics = true
```

## How to test

1. Run Sōzu on your machine
2. Run `sozu-prometheus-connector` with `cargo run -- --config config.toml`
3. In a web browser, query the URL `127.0.0.1:3000`

The prometheus-formatted metrics should appear in the browser.
