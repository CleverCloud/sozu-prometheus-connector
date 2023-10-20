use sozu_command_lib::proto::command::{
    filtered_metrics::Inner, AggregatedMetrics, BackendMetrics, FilteredMetrics, Percentiles,
};

/// Convert aggregated metrics into prometheus serialize one
#[tracing::instrument(skip_all)]
pub fn convert_metrics_to_prometheus(aggregated_metrics: AggregatedMetrics) -> String {
    let mut formatted_for_prometheus = "".to_string();

    // metrics of the main process
    for (metric_name, filtered_metric) in aggregated_metrics.main.iter() {
        let metric_lines = create_metric_lines(metric_name, &[("worker", "main")], filtered_metric);
        formatted_for_prometheus.push_str(&metric_lines);
    }

    // worker metrics
    for (worker_id, worker_metrics) in aggregated_metrics.workers {
        // proxy metrics (bytes in, accept queue…)
        for (metric_name, filtered_metric) in worker_metrics.proxy {
            let metric_lines = create_metric_lines(
                &metric_name,
                &[("worker", worker_id.as_str())],
                &filtered_metric,
            );
            formatted_for_prometheus.push_str(&metric_lines);
        }

        // cluster metrics (applications)
        for (cluster_id, cluster_metrics) in worker_metrics.clusters {
            for (metric_name, filtered_metric) in cluster_metrics.cluster {
                let metric_lines = create_metric_lines(
                    &metric_name,
                    &[("cluster_id", cluster_id.as_str())],
                    &filtered_metric,
                );
                formatted_for_prometheus.push_str(&metric_lines);
            }

            // backend metrics (several backends for a given cluster)
            for backend_metrics in cluster_metrics.backends {
                let BackendMetrics {
                    backend_id,
                    metrics,
                } = backend_metrics;

                for (metric_name, filtered_metric) in metrics {
                    let metric_lines = create_metric_lines(
                        &metric_name,
                        &[
                            ("cluster_id", cluster_id.as_str()),
                            ("backend_id", backend_id.as_str()),
                        ],
                        &filtered_metric,
                    );
                    formatted_for_prometheus.push_str(&metric_lines);
                }
            }
        }
    }

    formatted_for_prometheus
}

#[tracing::instrument(skip(filtered_metric))]
fn create_metric_lines(
    metric_name: &str,
    labels: &[(&str, &str)],
    filtered_metric: &FilteredMetrics,
) -> String {
    let mut lines = String::new();
    let metric_name = replace_dots_with_underscores(metric_name);
    let type_line = create_type_line(&metric_name, filtered_metric);
    let metric_lines = match &filtered_metric.inner {
        Some(inner) => match inner {
            Inner::Gauge(value) => create_metric_line_with_labels(&metric_name, labels, value),
            Inner::Count(value) => create_metric_line_with_labels(&metric_name, labels, value),
            Inner::Time(value) => create_metric_line_with_labels(&metric_name, labels, value),
            Inner::TimeSerie(value) => create_metric_line_with_labels(&metric_name, labels, value),
            Inner::Percentiles(_percentiles) => {
                // skip conversion of percentiles
                // this was useless and misleading since percentiles are not a prometheus metric format
                // TODO: convert Sōzu histograms once they are produced by sozu_command_lib
                // create_percentile_lines(&metric_name, labels, percentiles)
                String::new()
            }
        },
        None => "none".to_string(), // very very unlikely
    };
    lines.push_str(&type_line);
    lines.push('\n');
    lines.push_str(&metric_lines);
    lines.push('\n');
    lines
}

/// this is all false
/// convert a Sōzu Percentiles struct into prometheus histogram lines:
/// ```
/// # TYPE metric_name histogram
/// metric_name_bucket{le="0.5"} value
/// metric_name_bucket{le="0.9"} value
/// metric_name_bucket{le="0.99"} value
/// metric_name_bucket{le="0.999"} value
/// metric_name_bucket{le="0.9999"} value
/// metric_name_bucket{le="0.99999"} value
/// metric_name_bucket{le="1"} value
/// metric_name_sum sum-of-measurements
/// metric_name_count percentiles.samples
/// ```
/// (additionnal labels not show between the brackets)
#[tracing::instrument(skip(percentiles))]
fn create_percentile_lines(
    metric_name: &str,
    labels: &[(&str, &str)],
    percentiles: &Percentiles,
) -> String {
    let bucket_name = format!("{}_bucket", metric_name);
    let sum = 0; // we can not compute it as of version 0.15.3 of sozu-command-lib

    let mut lines = String::new();
    let sample_line = create_metric_line_with_labels(
        &format!("{}_samples", metric_name),
        labels,
        percentiles.samples,
    );
    let p_50_line = create_histogram_line(&bucket_name, "0.5", labels, percentiles.p_50);
    let p_90_line = create_histogram_line(&bucket_name, "0.9", labels, percentiles.p_90);
    let p_99_line = create_histogram_line(&bucket_name, "0.99", labels, percentiles.p_99);
    let p_99_9_line = create_histogram_line(&bucket_name, "0.999", labels, percentiles.p_99_9);
    let p_99_99_line = create_histogram_line(&bucket_name, "0.9999", labels, percentiles.p_99_99);
    let p_99_999_line =
        create_histogram_line(&bucket_name, "0.99999", labels, percentiles.p_99_999);
    let p_100_line = create_histogram_line(&bucket_name, "1", labels, percentiles.p_100);
    let inf_line = create_histogram_line(&bucket_name, "+Inf", labels, percentiles.p_100);
    let sum_line = create_metric_line_with_labels(&format!("{}_sum", metric_name), labels, sum);
    lines.push_str(&sample_line);
    lines.push('\n');
    lines.push_str(&p_50_line);
    lines.push('\n');
    lines.push_str(&p_90_line);
    lines.push('\n');
    lines.push_str(&p_99_line);
    lines.push('\n');
    lines.push_str(&p_99_9_line);
    lines.push('\n');
    lines.push_str(&p_99_99_line);
    lines.push('\n');
    lines.push_str(&p_99_999_line);
    lines.push('\n');
    lines.push_str(&p_100_line);
    lines.push('\n');
    lines.push_str(&inf_line);
    lines.push('\n');
    lines.push_str(&sum_line);
    lines.push('\n');

    lines
}

fn create_histogram_line<T>(
    bucket_name: &str,
    less_than: &str,
    labels: &[(&str, &str)],
    value: T,
) -> String
where
    T: ToString,
{
    let mut labels = labels.to_owned();
    labels.push(("le", less_than));
    create_metric_line_with_labels(bucket_name, &labels, value)
}

#[tracing::instrument(skip_all)]
fn replace_dots_with_underscores(str: &str) -> String {
    str.replace('.', "_")
}

#[tracing::instrument(skip_all)]
fn get_metric_type(filtered_metric: &FilteredMetrics) -> String {
    match &filtered_metric.inner {
        Some(inner) => match inner {
            Inner::Gauge(_) => "gauge".to_string(),
            Inner::Count(_) => "count".to_string(),
            Inner::Time(_) => "time".to_string(),
            Inner::Percentiles(_) => "histogram".to_string(),
            Inner::TimeSerie(_) => "time series".to_string(),
        },
        None => "none".to_string(), // very very unlikely
    }
}

// typically:
// # TYPE service_time percentiles
#[tracing::instrument(skip_all)]
fn create_type_line(name: &str, filtered_metric: &FilteredMetrics) -> String {
    // temporary fix to skip conversion of percentiles
    if matches!(filtered_metric.inner,Some(Inner::Percentiles(_))) {
        return String::new()
    }
    format!("# TYPE {} {}", name, get_metric_type(filtered_metric))
}

// typically:
// http_active_requests{worker="0"} 0
#[tracing::instrument(skip_all)]
fn create_metric_line_with_labels<T>(name: &str, labels: &[(&str, &str)], value: T) -> String
where
    T: ToString,
{
    let formatted_labels: String = labels
        .iter()
        .map(|(label_name, label_value)| format!("{}=\"{}\"", label_name, label_value))
        .collect();
    format!("{}{{{}}} {}", name, formatted_labels, value.to_string())
}
