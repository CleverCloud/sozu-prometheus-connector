use sozu_command_lib::proto::command::{
    filtered_metrics::{self, Inner},
    AggregatedMetrics, FilteredMetrics,
};

pub fn convert_metrics_to_prometheus(aggregated_metrics: AggregatedMetrics) -> String {
    let mut formatted_for_prometheus = "".to_string();

    // metrics of the main process
    for (metric_name, filtered_metric) in aggregated_metrics.main.iter() {
        let main_metric_name = format_main_metric_name(metric_name);
        let type_line = create_type_line(&main_metric_name, filtered_metric);
        let metric_line = create_metric_line(&main_metric_name, filtered_metric);
        formatted_for_prometheus.push_str(&type_line);
        formatted_for_prometheus.push('\n');
        formatted_for_prometheus.push_str(&metric_line);
        formatted_for_prometheus.push('\n');
    }

    // worker metrics
    for (worker_id, worker_metrics) in aggregated_metrics.workers {
        // proxy metrics (bytes in, accept queue…)
        for (metric_name, filtered_metric) in worker_metrics.proxy {
            let worker_metric_name = format_worker_metric_name(&worker_id, &metric_name);
            let type_line = create_type_line(&worker_metric_name, &filtered_metric);
            let metric_line = create_metric_line(&worker_metric_name, &filtered_metric);
            formatted_for_prometheus.push_str(&type_line);
            formatted_for_prometheus.push('\n');
            formatted_for_prometheus.push_str(&metric_line);
            formatted_for_prometheus.push('\n');
        }

        // cluster metrics (applications)
        for (cluster_id, cluster_metrics) in worker_metrics.clusters {
            for (metric_name, filtered_metric) in cluster_metrics.cluster {
                let metric_line = create_metric_line_with_label(
                    &metric_name,
                    "cluster_id",
                    &cluster_id,
                    &filtered_metric,
                );
                formatted_for_prometheus.push_str(&metric_line);
                formatted_for_prometheus.push('\n');
            }
        }
    }

    formatted_for_prometheus
}

fn format_main_metric_name(metric_name: &str) -> String {
    format!("main_{}", replace_dots_with_underscores(metric_name))
}

fn format_worker_metric_name(worker_id: &str, name: &str) -> String {
    format!(
        "worker_{}_{}",
        worker_id,
        replace_dots_with_underscores(name)
    )
}

fn replace_dots_with_underscores(str: &str) -> String {
    str.replace(".", "_")
}

fn get_metric_kind(filtered_metric: &FilteredMetrics) -> String {
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

fn create_type_line(name: &str, filtered_metric: &FilteredMetrics) -> String {
    format!("# TYPE {} {}", name, get_metric_kind(filtered_metric))
}

fn create_metric_line(name: &str, filtered_metric: &FilteredMetrics) -> String {
    format!("{} {}", name, format_filtered_metric(filtered_metric))
}

fn create_metric_line_with_label(
    name: &str,
    label_name: &str,
    label_value: &str,
    filtered_metric: &FilteredMetrics,
) -> String {
    format!(
        "{}{{{}=\"{}\"}} {}",
        name,
        label_name,
        label_value,
        format_filtered_metric(filtered_metric)
    )
}

fn format_filtered_metric(filtered_metric: &FilteredMetrics) -> String {
    match &filtered_metric.inner {
        Some(inner) => match inner {
            Inner::Gauge(value) => format!("{}", value),
            Inner::Count(value) => format!("{}", value),
            Inner::Time(value) => format!("{}", value),
            Inner::Percentiles(percentiles) => format!("{:?}", percentiles),
            Inner::TimeSerie(time_series) => format!("{}", time_series),
        },
        None => "none".to_string(), // very very unlikely
    }
}
