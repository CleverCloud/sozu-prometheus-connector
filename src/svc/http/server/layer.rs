//! # Layer module
//!
//! This module provides middlewares to give to the server implementation.
//! It could be seen as interceptor in h2.

use std::{fmt::Debug, time::Instant};

use axum::{http::Request, middleware::Next};
use once_cell::sync::Lazy;
use prometheus::{register_int_counter_vec, IntCounterVec};
use tracing::{info, info_span, Instrument};

// -----------------------------------------------------------------------------
// Telemetry

static ACCESS_REQUEST: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "http_access_requests_count",
        "Number of access request",
        &["method", "host", "status"]
    )
    .expect("'http_access_requests_count' to not be already registered")
});

static ACCESS_REQUEST_DURATION: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "http_access_requests_duration",
        "Duration of access request",
        &["method", "host", "status"]
    )
    .expect("'http_access_requests_duration' to not be already registered")
});

// -----------------------------------------------------------------------------
// Access

#[tracing::instrument(skip_all)]
pub async fn access<T>(req: Request<T>, next: Next<T>) -> axum::response::Response
where
    T: Debug,
{
    // -------------------------------------------------------------------------
    // Retrieve information
    let method = req.method().to_string();
    let uri = req.uri().to_string();
    let headers = req.headers();

    let origin = headers
        .get(hyper::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_string();

    let referer = headers
        .get(hyper::header::REFERER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_string();

    let forwarded = headers
        .get(hyper::header::FORWARDED)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_string();

    let agent = headers
        .get(hyper::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("<none>")
        .to_string();

    let host = req
        .uri()
        .host()
        .unwrap_or_else(|| {
            headers
                .get(hyper::header::HOST)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("<none>")
        })
        .to_string();

    // -------------------------------------------------------------------------
    // Call next handler
    let begin = Instant::now();
    let res = next.run(req).instrument(info_span!("next.run")).await;
    let duration = begin.elapsed().as_millis();

    // -------------------------------------------------------------------------
    // Log the access
    let status = res.status().as_u16();

    ACCESS_REQUEST
        .with_label_values(&[&method.to_string(), &host.to_string(), &status.to_string()])
        .inc();

    ACCESS_REQUEST_DURATION
        .with_label_values(&[&method.to_string(), &host.to_string(), &status.to_string()])
        .inc_by(duration as u64);

    info!(
        method = method,
        uri = uri,
        origin = origin,
        referer = referer,
        forwarded = forwarded,
        agent = agent,
        host = host,
        duration = duration,
        status = status,
        "Request received"
    );

    res
}
