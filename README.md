# Sōzu Prometheus connector

Receives HTTP GET requests on a `/metrics` route, forwards the request to Sōzu,
packages the responses in a Prometheus format and sends them back in a HTTP response.

## Configure

In the `config.toml`, you have to indicate the absolute path to the configuration file
of the Sōzu that runs on the machine.

```toml
sozu_configuration_path = "/path/to/sozu/on/the/machine/config.toml"
```

## How to test

1. Run Sōzu on your machine
2. Run `sozu-prometheus-connector` with `cargo run -- --config config.toml`
3. In a web browser, query the URL `127.0.0.1:3000`

The prometheus-formatted metrics should appear in the browser.
