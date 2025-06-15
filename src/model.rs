use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use futures::future::join_all;
use influxdb2::models::DataPoint;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

pub trait DataPointBuilder: Send + Sync {
    fn to_point(&self) -> Result<DataPoint>;
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Measurement {
    Power,
    DailyTotal,
    CircuitDailyTotal,
    Climate,
}

impl fmt::Display for Measurement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Measurement::Power => write!(f, "power"),
            Measurement::DailyTotal => write!(f, "daily_total"),
            Measurement::CircuitDailyTotal => write!(f, "circuit_daily_total"),
            Measurement::Climate => write!(f, "climate"),
        }
    }
}

#[derive(Debug)]
pub struct PowerStatusMetric {
    pub measurement: Measurement,
    pub name: String,
    pub value: i64,
}

impl DataPointBuilder for PowerStatusMetric {
    fn to_point(&self) -> Result<DataPoint> {
        match DataPoint::builder(self.measurement.to_string().as_str())
            .tag("summary", self.name.clone())
            .field("value", self.value)
            .build()
        {
            Ok(point) => Ok(point),
            Err(e) => Err(anyhow!("Failed to build DataPoint: {}", e)),
        }
    }
}

#[derive(Debug)]
pub struct PowerStatusBreakdownMetric {
    pub measurement: Measurement,
    pub category: PowerStatusBreakdownMetricCategory,
    pub name: String,
    pub value: i64,
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub(crate) enum PowerStatusBreakdownMetricCategory {
    Generation,
    Consumption,
}

pub fn merge_same_name_power_status_breakdown_metrics(
    metrics: Vec<PowerStatusBreakdownMetric>,
) -> Vec<PowerStatusBreakdownMetric> {
    #[derive(Eq, Hash, PartialEq)]
    struct Key {
        measurement: Measurement,
        category: PowerStatusBreakdownMetricCategory,
        name: String,
    }

    let mut map = HashMap::<Key, i64>::new();
    for metric in metrics {
        let key = Key {
            measurement: metric.measurement,
            category: metric.category,
            name: metric.name.clone(),
        };
        let entry = map.entry(key).or_insert(0);
        *entry += metric.value;
    }
    map.into_iter()
        .map(|(key, value)| PowerStatusBreakdownMetric {
            measurement: key.measurement,
            category: key.category,
            name: key.name,
            value,
        })
        .collect()
}

impl fmt::Display for PowerStatusBreakdownMetricCategory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PowerStatusBreakdownMetricCategory::Generation => write!(f, "generation"),
            PowerStatusBreakdownMetricCategory::Consumption => write!(f, "consumption"),
        }
    }
}

impl DataPointBuilder for PowerStatusBreakdownMetric {
    fn to_point(&self) -> Result<DataPoint> {
        match DataPoint::builder(self.measurement.to_string().as_str())
            .tag("detail-type", self.category.to_string())
            .tag("detail-section", self.name.clone())
            .field("value", self.value)
            .build()
        {
            Ok(point) => Ok(point),
            Err(e) => Err(anyhow!("Failed to build DataPoint: {}", e)),
        }
    }
}

#[derive(Debug)]
pub struct PowerTotalMetric {
    pub measurement: Measurement,
    pub name: String,
    pub value: f64,
    pub date: DateTime<Local>,
}

impl DataPointBuilder for PowerTotalMetric {
    fn to_point(&self) -> Result<DataPoint> {
        match DataPoint::builder(self.measurement.to_string().as_str())
            .tag("detail-section", self.name.clone())
            .field("value", self.value)
            .timestamp(self.date.timestamp_nanos_opt().unwrap())
            .build()
        {
            Ok(point) => Ok(point),
            Err(e) => Err(anyhow!("Failed to build DataPoint: {}", e)),
        }
    }
}

#[derive(Debug)]
pub struct ClimateStatusMetric {
    pub measurement: Measurement,
    pub category: ClimateStatusMetricCategory,
    pub name: String,
    pub value: f64,
    pub timestamp: DateTime<Local>,
}

#[derive(Debug)]
pub enum ClimateStatusMetricCategory {
    Temperature,
    Humidity,
}

impl fmt::Display for ClimateStatusMetricCategory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ClimateStatusMetricCategory::Temperature => write!(f, "temperature"),
            ClimateStatusMetricCategory::Humidity => write!(f, "humidity"),
        }
    }
}

impl DataPointBuilder for ClimateStatusMetric {
    fn to_point(&self) -> Result<DataPoint> {
        match DataPoint::builder(self.measurement.to_string().as_str())
            .tag("detail-type", self.category.to_string())
            .tag("detail-section", self.name.clone())
            .field("value", self.value)
            .timestamp(self.timestamp.timestamp_nanos_opt().unwrap())
            .build()
        {
            Ok(point) => Ok(point),
            Err(e) => Err(anyhow!("Failed to build DataPoint: {}", e)),
        }
    }
}

pub trait MetricCollector: Send + Sync {
    fn collect<'a>(
        &'a self,
        timestamp: DateTime<Local>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn DataPointBuilder>>>> + Send + 'a>>;
}

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

pub enum Unit {
    Watt,
    Kwh,
    Liter,
    CubicMeter,
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Unit::Watt => write!(f, "W"),
            Unit::Kwh => write!(f, "kWh"),
            Unit::Liter => write!(f, "L"),
            Unit::CubicMeter => write!(f, "㎥"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // Helper function to create a test timestamp
    fn test_timestamp() -> DateTime<Local> {
        Local.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap()
    }

    // Mock implementation of MetricCollector for testing
    struct MockSuccessCollector {
        // Return a function that creates metrics instead of storing them
        create_metrics: fn() -> Vec<Box<dyn DataPointBuilder>>,
    }

    impl MetricCollector for MockSuccessCollector {
        fn collect<'a>(
            &'a self,
            _timestamp: DateTime<Local>,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn DataPointBuilder>>>> + Send + 'a>>
        {
            let metrics = (self.create_metrics)();
            Box::pin(async move { Ok(metrics) })
        }
    }

    struct MockFailureCollector {
        error_message: String,
    }

    impl MetricCollector for MockFailureCollector {
        fn collect<'a>(
            &'a self,
            _timestamp: DateTime<Local>,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn DataPointBuilder>>>> + Send + 'a>>
        {
            Box::pin(async move { Err(anyhow!(self.error_message.clone())) })
        }
    }

    // Mock DataPointBuilder that always fails
    #[derive(Clone)]
    struct FailingDataPointBuilder;

    impl DataPointBuilder for FailingDataPointBuilder {
        fn to_point(&self) -> Result<DataPoint> {
            Err(anyhow!("Mock conversion failure"))
        }
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

        #[test]
        fn test_merge_same_name_power_status_breakdown_metrics_empty() {
            let metrics = vec![];
            let result = merge_same_name_power_status_breakdown_metrics(metrics);
            assert_eq!(result.len(), 0);
        }

        #[test]
        fn test_merge_same_name_power_status_breakdown_metrics_single() {
            let metrics = vec![PowerStatusBreakdownMetric {
                measurement: Measurement::Power,
                category: PowerStatusBreakdownMetricCategory::Generation,
                name: "solar".to_string(),
                value: 100,
            }];

            let result = merge_same_name_power_status_breakdown_metrics(metrics);
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].value, 100);
            assert_eq!(result[0].name, "solar");
        }

        #[test]
        fn test_merge_same_name_power_status_breakdown_metrics_same_key() {
            let metrics = vec![
                PowerStatusBreakdownMetric {
                    measurement: Measurement::Power,
                    category: PowerStatusBreakdownMetricCategory::Generation,
                    name: "solar".to_string(),
                    value: 100,
                },
                PowerStatusBreakdownMetric {
                    measurement: Measurement::Power,
                    category: PowerStatusBreakdownMetricCategory::Generation,
                    name: "solar".to_string(),
                    value: 200,
                },
                PowerStatusBreakdownMetric {
                    measurement: Measurement::Power,
                    category: PowerStatusBreakdownMetricCategory::Generation,
                    name: "solar".to_string(),
                    value: 50,
                },
            ];

            let result = merge_same_name_power_status_breakdown_metrics(metrics);
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].value, 350); // 100 + 200 + 50
            assert_eq!(result[0].name, "solar");
        }

        #[test]
        fn test_merge_same_name_power_status_breakdown_metrics_different_keys() {
            let metrics = vec![
                PowerStatusBreakdownMetric {
                    measurement: Measurement::Power,
                    category: PowerStatusBreakdownMetricCategory::Generation,
                    name: "solar".to_string(),
                    value: 100,
                },
                PowerStatusBreakdownMetric {
                    measurement: Measurement::Power,
                    category: PowerStatusBreakdownMetricCategory::Consumption,
                    name: "solar".to_string(), // Same name but different category
                    value: 50,
                },
                PowerStatusBreakdownMetric {
                    measurement: Measurement::Power,
                    category: PowerStatusBreakdownMetricCategory::Generation,
                    name: "wind".to_string(), // Different name
                    value: 200,
                },
            ];

            let result = merge_same_name_power_status_breakdown_metrics(metrics);
            assert_eq!(result.len(), 3); // No merging should occur
        }

        #[test]
        fn test_merge_same_name_power_status_breakdown_metrics_mixed() {
            let metrics = vec![
                PowerStatusBreakdownMetric {
                    measurement: Measurement::Power,
                    category: PowerStatusBreakdownMetricCategory::Generation,
                    name: "solar".to_string(),
                    value: 100,
                },
                PowerStatusBreakdownMetric {
                    measurement: Measurement::Power,
                    category: PowerStatusBreakdownMetricCategory::Generation,
                    name: "solar".to_string(),
                    value: 150,
                },
                PowerStatusBreakdownMetric {
                    measurement: Measurement::Power,
                    category: PowerStatusBreakdownMetricCategory::Consumption,
                    name: "appliances".to_string(),
                    value: 300,
                },
                PowerStatusBreakdownMetric {
                    measurement: Measurement::Power,
                    category: PowerStatusBreakdownMetricCategory::Consumption,
                    name: "appliances".to_string(),
                    value: 200,
                },
            ];

            let result = merge_same_name_power_status_breakdown_metrics(metrics);
            assert_eq!(result.len(), 2);

            // Find the merged results
            let solar_result = result.iter().find(|m| m.name == "solar").unwrap();
            assert_eq!(solar_result.value, 250); // 100 + 150

            let appliances_result = result.iter().find(|m| m.name == "appliances").unwrap();
            assert_eq!(appliances_result.value, 500); // 300 + 200
        }

        #[tokio::test]
        async fn test_batch_collect_metrics_empty_collectors() {
            let collectors: Vec<Box<dyn MetricCollector>> = vec![];
            let result = batch_collect_metrics(&collectors, test_timestamp()).await;
            assert_eq!(result.len(), 0);
        }

        #[tokio::test]
        async fn test_batch_collect_metrics_successful_collection() {
            let collector = Box::new(MockSuccessCollector {
                create_metrics: || {
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
                },
            });

            let collectors: Vec<Box<dyn MetricCollector>> = vec![collector];
            let result = batch_collect_metrics(&collectors, test_timestamp()).await;
            assert_eq!(result.len(), 2);
        }

        #[tokio::test]
        async fn test_batch_collect_metrics_multiple_collectors() {
            let collector1 = Box::new(MockSuccessCollector {
                create_metrics: || {
                    vec![Box::new(PowerStatusMetric {
                        measurement: Measurement::Power,
                        name: "collector1".to_string(),
                        value: 100,
                    })]
                },
            });

            let collector2 = Box::new(MockSuccessCollector {
                create_metrics: || {
                    vec![Box::new(PowerStatusMetric {
                        measurement: Measurement::Power,
                        name: "collector2".to_string(),
                        value: 200,
                    })]
                },
            });

            let collectors: Vec<Box<dyn MetricCollector>> = vec![collector1, collector2];
            let result = batch_collect_metrics(&collectors, test_timestamp()).await;
            assert_eq!(result.len(), 2);
        }
    }

    mod fails {
        use super::*;

        #[tokio::test]
        async fn test_batch_collect_metrics_with_collector_failure() {
            let success_collector = Box::new(MockSuccessCollector {
                create_metrics: || {
                    vec![Box::new(PowerStatusMetric {
                        measurement: Measurement::Power,
                        name: "success".to_string(),
                        value: 100,
                    })]
                },
            });

            let failure_collector = Box::new(MockFailureCollector {
                error_message: "Collection failed".to_string(),
            });

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

            impl MetricCollector for MixedCollector {
                fn collect<'a>(
                    &'a self,
                    _timestamp: DateTime<Local>,
                ) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn DataPointBuilder>>>> + Send + 'a>>
                {
                    Box::pin(async move {
                        Ok(vec![
                            Box::new(PowerStatusMetric {
                                measurement: Measurement::Power,
                                name: "valid".to_string(),
                                value: 100,
                            }) as Box<dyn DataPointBuilder>,
                            Box::new(FailingDataPointBuilder) as Box<dyn DataPointBuilder>,
                        ])
                    })
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
            let collector1 = Box::new(MockFailureCollector {
                error_message: "Collector 1 failed".to_string(),
            });

            let collector2 = Box::new(MockFailureCollector {
                error_message: "Collector 2 failed".to_string(),
            });

            let collectors: Vec<Box<dyn MetricCollector>> = vec![collector1, collector2];
            let result = batch_collect_metrics(&collectors, test_timestamp()).await;

            // Should return empty vec when all collectors fail
            assert_eq!(result.len(), 0);
        }
    }
}
