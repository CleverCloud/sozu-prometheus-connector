use std::fmt::Display;

use sozu_command_lib::proto::command::{
    filtered_metrics::Inner, AggregatedMetrics, BackendMetrics, FilteredMetrics,
};
use tracing::debug;
use urlencoding::encode;

#[derive(PartialEq)]
enum MetricType {
    Counter,
    Gauge,
    // Histogram,
    Unsupported,
}

impl Display for MetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            MetricType::Counter => write!(f, "counter"),
            MetricType::Gauge => write!(f, "gauge"),
            // MetricType::Histogram => write!(f, "histogram"),
            MetricType::Unsupported => write!(f, "unsupported"), // should never happen
        }
    }
}

/// convertible to prometheus metric in this form:
/// metric_name{label="something",second_lable="something-else"} value
struct LabeledMetric {
    metric_name: String,
    labels: Vec<(String, String)>,
    value: FilteredMetrics,
    metric_type: MetricType,
}

impl LabeledMetric {
    fn with_name(&mut self, name: &str) {
        self.metric_name = name.to_owned();
    }

    fn with_label(&mut self, label_name: &str, label_value: &str) {
        let label_value = encode(label_value);
        self.labels
            .push((label_name.to_owned(), label_value.into()));
    }

    /// remove dots from the name, replace with underscores
    fn printable_name(&self) -> String {
        self.metric_name.replace('.', "_")
    }

    /// # TYPE protocol_https gauge
    fn type_line(&self) -> String {
        let printable_metric_name = self.printable_name();
        format!("# TYPE {} {}", printable_metric_name, self.metric_type)
    }

    /// http_active_requests{worker="0"} 0
    fn metric_line(&self) -> String {
        let printable_metric_name = self.printable_name();
        let formatted_labels: String = self
            .labels
            .iter()
            .map(|(label_name, label_value)| format!("{}=\"{}\"", label_name, label_value))
            .collect();
        let value = match &self.value.inner {
            Some(inner) => {
                match inner {
                    Inner::Gauge(value) => value.to_string(),
                    Inner::Count(value) => value.to_string(),
                    Inner::Time(_) | Inner::Percentiles(_) | Inner::TimeSerie(_) => {
                        // should not happen at that point
                        return String::new();
                    }
                }
            }
            None => return String::new(),
        };
        format!(
            "{}{{{}}} {}",
            printable_metric_name, formatted_labels, value
        )
    }
}

impl From<FilteredMetrics> for LabeledMetric {
    fn from(value: FilteredMetrics) -> Self {
        let metric_type = match &value.inner {
            Some(inner) => match inner {
                Inner::Gauge(_) => MetricType::Gauge,
                Inner::Count(_) => MetricType::Counter,
                Inner::Time(_) | Inner::Percentiles(_) | Inner::TimeSerie(_) => {
                    MetricType::Unsupported
                }
            },
            None => MetricType::Unsupported,
        };
        Self {
            metric_name: String::new(),
            labels: Vec::new(),
            value,
            metric_type,
        }
    }
}

/// Convert aggregated metrics into prometheus serialize one
#[tracing::instrument(skip_all)]
pub fn convert_metrics_to_prometheus(aggregated_metrics: AggregatedMetrics) -> String {
    debug!("Converting metrics to prometheus format");
    let labeled_metrics = apply_labels(aggregated_metrics);

    let metric_names = get_unique_metric_names(&labeled_metrics);

    let mut prometheus_metrics = String::new();

    for metric_name in metric_names {
        prometheus_metrics.push_str(&produce_lines_for_one_metric_name(
            &labeled_metrics,
            &metric_name,
        ));
    }

    prometheus_metrics
}

/// assign worker_id and cluster_id as labels
fn apply_labels(aggregated_metrics: AggregatedMetrics) -> Vec<LabeledMetric> {
    let mut labeled_metrics = Vec::new();

    // metrics of the main process
    for (metric_name, value) in aggregated_metrics.main.iter() {
        let mut labeled = LabeledMetric::from(value.clone());
        labeled.with_name(metric_name);
        labeled.with_label("worker", "main");

        labeled_metrics.push(labeled);
    }

    // worker metrics
    for (worker_id, worker_metrics) in aggregated_metrics.workers {
        // proxy metrics (bytes in, accept queue…)
        for (metric_name, value) in worker_metrics.proxy {
            let mut labeled = LabeledMetric::from(value.clone());
            labeled.with_name(&metric_name);
            labeled.with_label("worker", &worker_id);
            labeled_metrics.push(labeled);
        }

        // cluster metrics (applications)
        for (cluster_id, cluster_metrics) in worker_metrics.clusters {
            for (metric_name, value) in cluster_metrics.cluster {
                let mut labeled = LabeledMetric::from(value.clone());
                labeled.with_name(&metric_name);
                labeled.with_label("cluster_id", &cluster_id);
                labeled_metrics.push(labeled);
            }

            // backend metrics (several backends for a given cluster)
            for backend_metrics in cluster_metrics.backends {
                let BackendMetrics {
                    backend_id,
                    metrics,
                } = backend_metrics;

                for (metric_name, value) in metrics {
                    let mut labeled = LabeledMetric::from(value.clone());
                    labeled.with_name(&metric_name);
                    labeled.with_label("cluster_id", &cluster_id);
                    labeled.with_label("backend_id", &backend_id);
                    labeled_metrics.push(labeled);
                }
            }
        }
    }
    labeled_metrics
}

fn get_unique_metric_names(labeled_metrics: &Vec<LabeledMetric>) -> Vec<String> {
    let mut names = Vec::new();
    for metric in labeled_metrics {
        if !names.contains(&metric.metric_name) {
            names.push(metric.metric_name.clone());
        }
    }
    names
}

fn produce_lines_for_one_metric_name(
    labeled_metrics: &Vec<LabeledMetric>,
    metric_name: &str,
) -> String {
    let mut lines = String::new();

    // find the first item to produce the type line only once
    let first_item = match labeled_metrics
        .iter()
        .find(|metric| metric.metric_name == metric_name)
    {
        Some(item) => item,
        None => return String::new(),
    };
    if first_item.metric_type == MetricType::Unsupported {
        return String::new();
    }
    lines.push_str(&first_item.type_line());
    lines.push('\n');

    for metric in labeled_metrics {
        if metric.metric_name == metric_name {
            lines.push_str(&metric.metric_line());
            lines.push('\n');
        }
    }

    lines
}

#[tracing::instrument(skip_all)]
fn replace_dots_with_underscores(str: &str) -> String {
    str.replace('.', "_")
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use sozu_command_lib::proto::command::{
        filtered_metrics::Inner, AggregatedMetrics, ClusterMetrics, FilteredMetrics, WorkerMetrics,
    };

    use super::*;

    #[test]
    fn encode_one_counter() {
        let cluster_id = "http://my-cluster-id.com/api?param=value".to_string();

        let metric_name = "http_response_status";
        let one_filtered_metric = FilteredMetrics {
            inner: Some(Inner::Gauge(3)),
        };
        let mut cluster = BTreeMap::new();
        cluster.insert(metric_name.to_owned(), one_filtered_metric);

        let cluster_metrics = ClusterMetrics {
            cluster,
            backends: Vec::new(),
        };

        let mut clusters = BTreeMap::new();
        clusters.insert(cluster_id, cluster_metrics);

        let worker_metrics = WorkerMetrics {
            proxy: BTreeMap::new(),
            clusters,
        };

        let mut workers = BTreeMap::new();
        workers.insert("WORKER-01".to_string(), worker_metrics);


        let aggregated_metrics = AggregatedMetrics {
            main: BTreeMap::new(),
            workers,
        };



        let prometheus_metrics = convert_metrics_to_prometheus(aggregated_metrics);

        let expected = r#"# TYPE http_response_status gauge
http_response_status{cluster_id="http%3A%2F%2Fmy-cluster-id.com%2Fapi%3Fparam%3Dvalue"} 3
"#;

        assert_eq!(expected.to_string(), prometheus_metrics);
    }
}

/* this is all false

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

*/
