//! # Handler module
//!
//! This module provides handlers to use with the server implementation

use std::time::SystemTime;

use axum::{
    extract::State,
    http::{HeaderValue, Request, Response, StatusCode, header},
    body::Body,
};
use prometheus::{Encoder, TextEncoder};
use sozu_client::Sender;
use sozu_command_lib::proto::command::{
    self, request::RequestType, response_content::ContentType, QueryMetricsOptions, ResponseContent,
};
use tracing::{debug, error};

use crate::svc::{http::server, telemetry::prometheus::convert_metrics_to_prometheus};

// -----------------------------------------------------------------------------
// Constants

pub const X_REQUEST_ID: &str = "X-Request-Id";
pub const X_TIMESTAMP: &str = "X-Timestamp";

// -----------------------------------------------------------------------------
// Not found

#[tracing::instrument]
pub async fn not_found(_req: Request<Body>) -> Response<Body> {
    let mut res = Response::default();

    *res.status_mut() = StatusCode::NOT_FOUND;
    res
}

// -----------------------------------------------------------------------------
// Healthz

#[tracing::instrument]
pub async fn healthz(req: Request<Body>) -> Response<Body> {
    let mut res = Response::default();

    *res.status_mut() = StatusCode::OK;
    let headers = res.headers_mut();
    if let Some(header) = req.headers().get(X_REQUEST_ID) {
        headers.insert(X_REQUEST_ID, header.to_owned());
    }

    if let Ok(now) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        headers.insert(
            X_TIMESTAMP,
            HeaderValue::from_str(&now.as_secs().to_string())
                .expect("number to be iso8859-1 compliant"),
        );
    }

    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime::APPLICATION_JSON.as_ref())
            .expect("constant to be iso8859-1 compliant"),
    );

    let message = serde_json::json!({"message": "Everything is fine! 🚀"}).to_string();

    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&message.len().to_string())
            .expect("constant to be iso8859-1 compliant"),
    );

    *res.body_mut() = Body::from(message);
    res
}

// -----------------------------------------------------------------------------
// Telemetry

#[tracing::instrument]
/// Retrieve Sōzu internals and connector telemetry
pub async fn telemetry(State(state): State<server::State>, _req: Request<Body>) -> Response<Body> {
    let mut buf = vec![];
    let mut res = Response::default();

    // -------------------------------------------------------------------------
    // Query Sōzu to get its internal metrics
    debug!("Querying Sōzu metrics");
    let mut sozu_metrics = match state
        .client
        .send(RequestType::QueryMetrics(QueryMetricsOptions::default()))
        .await
    {
        Ok(command::Response {
            content:
                Some(ResponseContent {
                    content_type: Some(ContentType::Metrics(aggregated_metrics)),
                }),
            ..
        }) => convert_metrics_to_prometheus(
            aggregated_metrics,
            state.config.aggregate_backend_metrics,
        ),
        Ok(response) => {
            let headers = res.headers_mut();
            let message = serde_json::json!({
                "error":
                    format!(
                        "Could not query Sōzu on its command socket, got response status {}",
                        response.status
                    )
            })
            .to_string();

            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(mime::APPLICATION_JSON.as_ref())
                    .expect("constant to be iso8859-1 compliant"),
            );

            headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&message.len().to_string())
                    .expect("buffer size to be iso8859-1 compliant"),
            );

            *res.status_mut() = StatusCode::OK;
            *res.body_mut() = Body::from(message);

            error!(
                status = response.status,
                "Could not query Sōzu on its command socket, got an invalid response"
            );
            return res;
        }
        Err(err) => {
            let headers = res.headers_mut();
            let message = serde_json::json!({"error": err.to_string() }).to_string();

            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(mime::APPLICATION_JSON.as_ref())
                    .expect("constant to be iso8859-1 compliant"),
            );

            headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&message.len().to_string())
                    .expect("buffer size to be iso8859-1 compliant"),
            );

            *res.status_mut() = StatusCode::OK;
            *res.body_mut() = Body::from(message);

            error!(
                error = err.to_string(),
                "Could not query Sōzu on its command socket"
            );
            return res;
        }
    };

    // -------------------------------------------------------------------------
    // Retrieve internals telemetry

    let encoder = TextEncoder::new();
    let metrics = prometheus::gather();

    if let Err(err) = encoder.encode(&metrics, &mut buf) {
        let headers = res.headers_mut();
        let message = serde_json::json!({"error": err.to_string() }).to_string();

        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(mime::APPLICATION_JSON.as_ref())
                .expect("constant to be iso8859-1 compliant"),
        );

        headers.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&message.len().to_string())
                .expect("buffer size to be iso8859-1 compliant"),
        );

        *res.status_mut() = StatusCode::OK;
        *res.body_mut() = Body::from(message);

        return res;
    }

    // -------------------------------------------------------------------------
    // Answer to http request

    buf.append(unsafe { sozu_metrics.as_mut_vec() });

    let headers = res.headers_mut();

    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime::TEXT_PLAIN_UTF_8.as_ref())
            .expect("constant to be iso8859-1 compliant"),
    );

    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&buf.len().to_string())
            .expect("buffer size to be iso8859-1 compliant"),
    );

    *res.status_mut() = StatusCode::OK;
    *res.body_mut() = Body::from(buf);

    res
}
