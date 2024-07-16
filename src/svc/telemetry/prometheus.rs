use std::{fmt::Display, iter};

use sozu_command_lib::proto::command::{
    filtered_metrics::Inner, AggregatedMetrics, BackendMetrics, FilteredMetrics,
};
use tracing::debug;
use urlencoding::encode;

#[derive(PartialEq)]
enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Unsupported,
}

impl Display for MetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            MetricType::Counter => write!(f, "counter"),
            MetricType::Gauge => write!(f, "gauge"),
            MetricType::Histogram => write!(f, "histogram"),
            MetricType::Unsupported => write!(f, "unsupported"), // should never happen
        }
    }
}

/// convertible to prometheus metric in this form:
/// metric_name{label="something",second_label="something-else"} value
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

    /// Create a type line, typically:
    ///
    /// # TYPE protocol_https gauge
    fn type_line(&self) -> String {
        let printable_metric_name = self.printable_name();
        format!("# TYPE {} {}", printable_metric_name, self.metric_type)
    }

    /// Format labels in a comma-separated list:
    ///
    /// ```plain
    /// "label"="value","other"="value"
    /// ```
    fn formatted_labels(&self) -> String {
        self.labels
            .iter()
            .map(|(name, value)| format!("{}=\"{}\"", name, value))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Create a metric line, typically:
    ///
    /// ```plain
    /// http_active_requests{worker="0"} 0
    /// ```
    /// For histograms, several lines are produced: sum, count, buckets
    fn metric_line(&self) -> String {
        let printable_metric_name = self.printable_name();
        let formatted_labels = self.formatted_labels();
        match &self.value.inner {
            Some(inner) => {
                match inner {
                    Inner::Gauge(value) => format!(
                        "{}{{{}}} {}",
                        printable_metric_name, formatted_labels, value
                    ),
                    Inner::Count(value) => format!(
                        "{}{{{}}} {}",
                        printable_metric_name, formatted_labels, value
                    ),
                    Inner::Histogram(hist) => hist
                        .buckets
                        .iter()
                        .map(|bucket| {
                            if formatted_labels.is_empty() {
                                format!(
                                    "{}_bucket{{le=\"{}\"}} {}\n",
                                    printable_metric_name, bucket.le, bucket.count
                                )
                            } else {
                                format!(
                                    "{}_bucket{{{}, le=\"{}\"}} {}\n",
                                    printable_metric_name,
                                    formatted_labels,
                                    bucket.le,
                                    bucket.count
                                )
                            }
                        })
                        .chain(iter::once(format!(
                            "{}_sum{{{}}} {}\n",
                            printable_metric_name, formatted_labels, hist.sum
                        )))
                        .chain(iter::once(format!(
                            "{}_count{{{}}} {}",
                            printable_metric_name, formatted_labels, hist.count
                        )))
                        .collect::<String>(),
                    Inner::Time(_) | Inner::Percentiles(_) | Inner::TimeSerie(_) => {
                        // should not happen at that point
                        String::new()
                    }
                }
            }
            None => String::new(),
        }
    }
}

impl From<FilteredMetrics> for LabeledMetric {
    fn from(value: FilteredMetrics) -> Self {
        let metric_type = match &value.inner {
            Some(inner) => match inner {
                Inner::Gauge(_) => MetricType::Gauge,
                Inner::Count(_) => MetricType::Counter,
                Inner::Histogram(_) => MetricType::Histogram,
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
        labeled.with_name(&format!("{}_main", metric_name));

        labeled_metrics.push(labeled);
    }

    // proxying metrics
    for (metric_name, value) in aggregated_metrics.proxying.iter() {
        let mut labeled = LabeledMetric::from(value.clone());
        labeled.with_name(&format!("{}_total", metric_name));

        labeled_metrics.push(labeled);
    }

    // cluster metrics (applications)
    for (cluster_id, cluster_metrics) in aggregated_metrics.clusters {
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
        filtered_metrics::Inner, AggregatedMetrics, ClusterMetrics, FilteredMetrics,
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

        let aggregated_metrics = AggregatedMetrics {
            clusters,
            ..Default::default()
        };

        let prometheus_metrics = convert_metrics_to_prometheus(aggregated_metrics);

        let expected = r#"# TYPE http_response_status gauge
http_response_status{cluster_id="http%3A%2F%2Fmy-cluster-id.com%2Fapi%3Fparam%3Dvalue"} 3
"#;

        assert_eq!(expected.to_string(), prometheus_metrics);
    }

    #[test]
    fn format_labels() {
        let metric = FilteredMetrics {
            inner: Some(Inner::Count(3)),
        };
        let mut labeled_metric = LabeledMetric::from(metric);

        assert_eq!(labeled_metric.formatted_labels(), "");

        labeled_metric.with_label("le", "3");

        assert_eq!(labeled_metric.formatted_labels(), r#"le="3""#);

        labeled_metric.with_label("cluster_id", "http://my-cluster-id.com/api?param=value");

        assert_eq!(
            labeled_metric.formatted_labels(),
            r#"le="3",cluster_id="http%3A%2F%2Fmy-cluster-id.com%2Fapi%3Fparam%3Dvalue""#
        )
    }
}
