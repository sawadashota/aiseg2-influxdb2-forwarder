use crate::error::{CollectorError, Result, StorageError};
use async_trait::async_trait;
use chrono::{DateTime, Local};
use influxdb2::models::DataPoint;

/// Trait for types that can be converted to InfluxDB data points.
///
/// This trait enables metric types to be transformed into InfluxDB-compatible
/// data points for storage. Implementors must be thread-safe (Send + Sync)
/// to support concurrent metric collection.
pub trait DataPointBuilder: Send + Sync {
    /// Converts the metric into an InfluxDB DataPoint.
    ///
    /// # Returns
    /// - `Ok(DataPoint)` if conversion succeeds
    /// - `Err` if the metric data cannot be converted to a valid DataPoint
    fn to_point(&self) -> Result<DataPoint, StorageError>;
}

/// Trait for types that can collect metrics from AiSEG2.
///
/// Implementors of this trait are responsible for fetching specific
/// types of metrics from the AiSEG2 system. Each collector typically
/// handles one category of metrics (power, climate, totals, etc.).
#[async_trait]
pub trait MetricCollector: Send + Sync {
    /// Collects metrics at the specified timestamp.
    ///
    /// # Arguments
    /// * `timestamp` - The time to associate with collected metrics
    ///
    /// # Returns
    /// A future that resolves to a vector of DataPointBuilder instances
    async fn collect(
        &self,
        timestamp: DateTime<Local>,
    ) -> Result<Vec<Box<dyn DataPointBuilder>>, CollectorError>;
}
