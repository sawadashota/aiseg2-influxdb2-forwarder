//! Model definitions for AiSEG2 metrics and InfluxDB data points.
//!
//! This module provides the core data structures and traits for representing
//! metrics collected from the AiSEG2 system and converting them to InfluxDB
//! data points.

pub mod metrics;
pub mod traits;
pub mod types;
pub mod utilities;

// Re-export commonly used items at the module level
pub use metrics::{
    ClimateStatusMetric, PowerStatusBreakdownMetric, PowerStatusMetric, PowerTotalMetric,
};
pub use traits::{DataPointBuilder, MetricCollector};
pub use types::{
    ClimateStatusMetricCategory, Measurement, PowerStatusBreakdownMetricCategory, Unit,
};
pub use utilities::batch_collect_metrics;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::mocks::{FailingDataPointBuilder, MockMetricCollector};
    use crate::error::{CollectorError, Result};
    use chrono::{Local, TimeZone};

    // Helper function to create a test timestamp
    fn test_timestamp() -> chrono::DateTime<Local> {
        Local.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap()
    }

    mod succeeds {
        use super::*;

        #[test]
        fn test_measurement_display() {
            assert_eq!(Measurement::Power.to_string(), "power");
            assert_eq!(Measurement::DailyTotal.to_string(), "daily_total");
            assert_eq!(
                Measurement::CircuitDailyTotal.to_string(),
                "circuit_daily_total"
            );
            assert_eq!(Measurement::Climate.to_string(), "climate");
        }

        #[test]
        fn test_power_status_breakdown_metric_category_display() {
            assert_eq!(
                PowerStatusBreakdownMetricCategory::Generation.to_string(),
                "generation"
            );
            assert_eq!(
                PowerStatusBreakdownMetricCategory::Consumption.to_string(),
                "consumption"
            );
        }

        #[test]
        fn test_climate_status_metric_category_display() {
            assert_eq!(
                ClimateStatusMetricCategory::Temperature.to_string(),
                "temperature"
            );
            assert_eq!(
                ClimateStatusMetricCategory::Humidity.to_string(),
                "humidity"
            );
        }

        #[test]
        fn test_unit_display() {
            assert_eq!(Unit::Watt.to_string(), "W");
            assert_eq!(Unit::Kwh.to_string(), "kWh");
            assert_eq!(Unit::Liter.to_string(), "L");
            assert_eq!(Unit::CubicMeter.to_string(), "㎥");
        }

        #[test]
        fn test_power_status_metric_to_point() {
            let metric = PowerStatusMetric {
                measurement: Measurement::Power,
                name: "test_power".to_string(),
                value: 1000,
            };

            let result = metric.to_point();
            assert!(result.is_ok());
            // DataPoint is successfully created
        }

        #[test]
        fn test_power_status_breakdown_metric_to_point() {
            let metric = PowerStatusBreakdownMetric {
                measurement: Measurement::Power,
                category: PowerStatusBreakdownMetricCategory::Generation,
                name: "solar_power".to_string(),
                value: 2500,
            };

            let result = metric.to_point();
            assert!(result.is_ok());
            // DataPoint is successfully created
        }

        #[test]
        fn test_power_total_metric_to_point() {
            let metric = PowerTotalMetric {
                measurement: Measurement::DailyTotal,
                name: "daily_consumption".to_string(),
                value: 123.45,
                date: test_timestamp(),
            };

            let result = metric.to_point();
            assert!(result.is_ok());
            // DataPoint is successfully created
        }

        #[test]
        fn test_climate_status_metric_to_point() {
            let metric = ClimateStatusMetric {
                measurement: Measurement::Climate,
                category: ClimateStatusMetricCategory::Temperature,
                name: "living_room".to_string(),
                value: 22.5,
                timestamp: test_timestamp(),
            };

            let result = metric.to_point();
            assert!(result.is_ok());
            // DataPoint is successfully created
        }

        #[tokio::test]
        async fn test_batch_collect_metrics_empty_collectors() {
            let collectors: Vec<Box<dyn MetricCollector>> = vec![];
            let result = batch_collect_metrics(&collectors, test_timestamp()).await;
            assert_eq!(result.len(), 0);
        }

        #[tokio::test]
        async fn test_batch_collect_metrics_successful_collection() {
            let collector = Box::new(MockMetricCollector::new_with_data(|| {
                vec![
                    Box::new(PowerStatusMetric {
                        measurement: Measurement::Power,
                        name: "test1".to_string(),
                        value: 100,
                    }),
                    Box::new(PowerStatusMetric {
                        measurement: Measurement::Power,
                        name: "test2".to_string(),
                        value: 200,
                    }),
                ]
            }));

            let collectors: Vec<Box<dyn MetricCollector>> = vec![collector];
            let result = batch_collect_metrics(&collectors, test_timestamp()).await;
            assert_eq!(result.len(), 2);
        }

        #[tokio::test]
        async fn test_batch_collect_metrics_multiple_collectors() {
            let collector1 = Box::new(MockMetricCollector::new_with_data(|| {
                vec![Box::new(PowerStatusMetric {
                    measurement: Measurement::Power,
                    name: "collector1".to_string(),
                    value: 100,
                })]
            }));

            let collector2 = Box::new(MockMetricCollector::new_with_data(|| {
                vec![Box::new(PowerStatusMetric {
                    measurement: Measurement::Power,
                    name: "collector2".to_string(),
                    value: 200,
                })]
            }));

            let collectors: Vec<Box<dyn MetricCollector>> = vec![collector1, collector2];
            let result = batch_collect_metrics(&collectors, test_timestamp()).await;
            assert_eq!(result.len(), 2);
        }
    }

    mod fails {
        use super::*;

        #[tokio::test]
        async fn test_batch_collect_metrics_with_collector_failure() {
            let success_collector = Box::new(MockMetricCollector::new_with_data(|| {
                vec![Box::new(PowerStatusMetric {
                    measurement: Measurement::Power,
                    name: "success".to_string(),
                    value: 100,
                })]
            }));

            let failure_collector = Box::new(MockMetricCollector::new_failure("Collection failed"));

            let collectors: Vec<Box<dyn MetricCollector>> =
                vec![success_collector, failure_collector];
            let result = batch_collect_metrics(&collectors, test_timestamp()).await;

            // Should still return the successful metric
            assert_eq!(result.len(), 1);
        }

        #[tokio::test]
        async fn test_batch_collect_metrics_with_conversion_failure() {
            // Create a collector that returns both valid and failing metrics
            struct MixedCollector;

            #[async_trait::async_trait]
            impl MetricCollector for MixedCollector {
                async fn collect(
                    &self,
                    _timestamp: chrono::DateTime<Local>,
                ) -> Result<Vec<Box<dyn DataPointBuilder>>, CollectorError> {
                    Ok(vec![
                        Box::new(PowerStatusMetric {
                            measurement: Measurement::Power,
                            name: "valid".to_string(),
                            value: 100,
                        }) as Box<dyn DataPointBuilder>,
                        Box::new(FailingDataPointBuilder) as Box<dyn DataPointBuilder>,
                    ])
                }
            }

            let collector = Box::new(MixedCollector);
            let collectors: Vec<Box<dyn MetricCollector>> = vec![collector];
            let result = batch_collect_metrics(&collectors, test_timestamp()).await;

            // Should only return the valid metric
            assert_eq!(result.len(), 1);
        }

        #[tokio::test]
        async fn test_batch_collect_metrics_all_failures() {
            let collector1 = Box::new(MockMetricCollector::new_failure("Collector 1 failed"));

            let collector2 = Box::new(MockMetricCollector::new_failure("Collector 2 failed"));

            let collectors: Vec<Box<dyn MetricCollector>> = vec![collector1, collector2];
            let result = batch_collect_metrics(&collectors, test_timestamp()).await;

            // Should return empty vec when all collectors fail
            assert_eq!(result.len(), 0);
        }
    }
}
