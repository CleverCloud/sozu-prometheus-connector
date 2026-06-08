# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0]

### Added

- Opt-in per-worker metrics, gated by the new `per-worker-metrics` configuration
  flag (kebab key, defaults to `false`). When enabled, per-worker series are
  exported with a `worker_id` label: worker proxy metrics under a distinct
  `_worker`-suffixed family, and per-worker cluster/backend metrics under the
  aggregated metric name with an added `worker_id` label. Default-off keeps the
  exported output byte-identical to `0.2.3`.
- `CHANGELOG.md` following the Keep a Changelog format.
- README section documenting the per-worker metrics, the `worker_id` label, and
  the PromQL selectors to pick aggregated (`{worker_id=""}`) versus per-worker
  (`{worker_id!=""}`) series without double-counting.

### Changed

- **Minimum Supported Rust Version raised to `1.88.0`** (from `1.80.0`), forced by
  `sozu-command-lib 2.1.0` and `sentry`/`sentry-tracing 0.48`.
- Upgraded `sozu-command-lib` `1.0.5` → `2.1.0` and `sozu-client` `0.4.3` → `0.5.0`.
  Following the Sōzu 2.x metrics rewrite, all new metric families (e.g. UDP, HTTP/2,
  per-cluster availability) flow through the generic export path automatically.
- Upgraded the remaining dependencies: `axum` `0.7` → `0.8`, `config` `0.14` → `0.15`,
  `clap` `4.4` → `4.6`, `prometheus` `0.13` → `0.14`, `sentry`/`sentry-tracing`
  `0.34` → `0.48`, `thiserror` `1` → `2`, and refreshed `tokio`, `tracing`,
  `tracing-subscriber`, `serde`, `serde_json` to their latest compatible releases.
- Following the Sōzu 2.x rename of several metrics, the connector now exports the
  new (underscored) names: `client_connections_percent` (was
  `client_connections_percentage`), `client_connections_max` (was
  `client_max_connections`), `buffer_in_use` (was `buffer_number`), and
  `http_redirect_template_compile_error` (was
  `http_301_redirect_template_compile_error`). Update dashboards and alerts
  accordingly.
- Per-cluster `Count`/`Time` metrics are now cumulative since worker start (Sōzu
  2.x semantics) instead of hourly. Wrap the corresponding counters in `rate()`
  in PromQL.
- Modernized the CI workflow: `actions/checkout@v4`, `dtolnay/rust-toolchain`
  (replacing the archived `actions-rs/*` actions), added `clippy -D warnings` and
  `rustfmt --check` gates, and pinned the MSRV matrix entry to `1.88.0`.

### Fixed

- Replaced the `unsafe { String::as_mut_vec() }` body assembly with a safe
  `String::into_bytes()` move.
- Corrected `example.config.toml`: the `[sentry]` block used `environment`, which
  does not deserialize into `SentryContext.env` and silently dropped the value;
  it now uses `env`.
- Fixed the `systemd` unit `Description` (it described an unrelated Pulsar
  consumer) and refreshed stale `docs.rs/sentry/0.31.3` documentation links to
  `0.48.2`.
- Fixed the `initialize_with_sentry` doctest, which referenced a non-existent
  crate (`functions_sdk`).
- Corrected the README configuration schema (real kebab `[sozu]` tables), the
  `/metrics` test URL, and removed the documentation of a never-implemented
  `aggregate-backend-metrics` flatten feature.

### Removed

- Dead `replace_dots_with_underscores` helper (no call sites).

[0.3.0]: https://github.com/CleverCloud/sozu-prometheus-connector/releases/tag/v0.3.0
