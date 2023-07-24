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
            Inner::Percentiles(percentiles) => {
                create_percentile_lines(&metric_name, labels, percentiles)
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

#[tracing::instrument(skip(percentiles))]
fn create_percentile_lines(
    metric_name: &str,
    labels: &[(&str, &str)],
    percentiles: &Percentiles,
) -> String {
    let mut lines = String::new();
    let sample_line = create_metric_line_with_labels(
        &format!("{}_samples", metric_name),
        labels,
        percentiles.samples,
    );
    let p_50_line =
        create_metric_line_with_labels(&format!("{}_p_50", metric_name), labels, percentiles.p_50);
    let p_90_line =
        create_metric_line_with_labels(&format!("{}_p_90", metric_name), labels, percentiles.p_90);

    let p_99_line =
        create_metric_line_with_labels(&format!("{}_p_99", metric_name), labels, percentiles.p_99);

    let p_99_9_line = create_metric_line_with_labels(
        &format!("{}_p_99_9", metric_name),
        labels,
        percentiles.p_99_9,
    );

    let p_99_99_line = create_metric_line_with_labels(
        &format!("{}_p_99_99", metric_name),
        labels,
        percentiles.p_99_99,
    );

    let p_99_999_line = create_metric_line_with_labels(
        &format!("{}_p_99_999", metric_name),
        labels,
        percentiles.p_99_999,
    );
    let p_100_line = create_metric_line_with_labels(
        &format!("{}_p_100", metric_name),
        labels,
        percentiles.p_100,
    );
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

    lines
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
            Inner::Percentiles(_) => "percentiles".to_string(),
            Inner::TimeSerie(_) => "time series".to_string(),
        },
        None => "none".to_string(), // very very unlikely
    }
}

// typically:
// # TYPE service_time percentiles
#[tracing::instrument(skip_all)]
fn create_type_line(name: &str, filtered_metric: &FilteredMetrics) -> String {
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
