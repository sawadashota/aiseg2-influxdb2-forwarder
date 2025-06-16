use chrono::{DateTime, Local};
use futures::future::join_all;
use influxdb2::models::DataPoint;

use super::traits::MetricCollector;

/// Collects metrics from multiple collectors concurrently.
///
/// This function runs all collectors in parallel, handles errors gracefully,
/// and converts successful results to InfluxDB data points. Failed collections
/// or conversions are logged but don't stop other collectors.
///
/// # Arguments
/// * `clients` - Vector of metric collectors to run
/// * `timestamp` - The timestamp to use for all collected metrics
///
/// # Returns
/// A vector of successfully collected and converted data points
pub async fn batch_collect_metrics<'a>(
    clients: &Vec<Box<dyn MetricCollector + 'a>>,
    timestamp: DateTime<Local>,
) -> Vec<DataPoint> {
    let results = join_all(clients.iter().map(|client| client.collect(timestamp))).await;

    results
        .into_iter()
        .filter_map(|res| match res {
            Ok(builders) => Some(builders),
            Err(e) => {
                tracing::error!("Failed to get metrics: {:?}", e);
                None
            }
        })
        .flatten()
        .filter_map(|p| match p.to_point() {
            Ok(point) => Some(point),
            Err(e) => {
                tracing::error!("Failed to convert to point: {:?}", e);
                None
            }
        })
        .collect()
}
