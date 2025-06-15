//! Climate metric building utilities.

use chrono::{DateTime, Local};

use crate::model::{
    ClimateStatusMetric, ClimateStatusMetricCategory, DataPointBuilder, Measurement,
};

/// Creates temperature and humidity metrics for a location.
///
/// # Arguments
/// * `name` - Location name
/// * `temperature` - Temperature value in Celsius
/// * `humidity` - Humidity percentage
/// * `timestamp` - Timestamp for the metrics
///
/// # Returns
/// Array containing temperature and humidity metrics
pub fn create_climate_metrics(
    name: String,
    temperature: f64,
    humidity: f64,
    timestamp: DateTime<Local>,
) -> [ClimateStatusMetric; 2] {
    [
        ClimateStatusMetric {
            measurement: Measurement::Climate,
            category: ClimateStatusMetricCategory::Temperature,
            name: name.clone(),
            value: temperature,
            timestamp,
        },
        ClimateStatusMetric {
            measurement: Measurement::Climate,
            category: ClimateStatusMetricCategory::Humidity,
            name,
            value: humidity,
            timestamp,
        },
    ]
}

/// Converts climate metrics to DataPointBuilder format.
pub fn climate_metrics_to_builders(
    metrics: Vec<ClimateStatusMetric>,
) -> Vec<Box<dyn DataPointBuilder>> {
    metrics
        .into_iter()
        .map(|m| Box::new(m) as Box<dyn DataPointBuilder>)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_climate_metrics() {
        let timestamp = Local::now();
        let metrics = create_climate_metrics("リビング".to_string(), 23.5, 65.0, timestamp);

        assert_eq!(metrics.len(), 2);

        // Temperature metric
        assert_eq!(metrics[0].name, "リビング");
        assert_eq!(metrics[0].value, 23.5);
        assert_eq!(
            metrics[0].category,
            ClimateStatusMetricCategory::Temperature
        );

        // Humidity metric
        assert_eq!(metrics[1].name, "リビング");
        assert_eq!(metrics[1].value, 65.0);
        assert_eq!(metrics[1].category, ClimateStatusMetricCategory::Humidity);

        // Both should have the same timestamp
        assert_eq!(metrics[0].timestamp, timestamp);
        assert_eq!(metrics[1].timestamp, timestamp);
    }

    #[test]
    fn test_climate_metrics_to_builders() {
        let timestamp = Local::now();
        let metrics = vec![
            ClimateStatusMetric {
                measurement: Measurement::Climate,
                category: ClimateStatusMetricCategory::Temperature,
                name: "Room1".to_string(),
                value: 20.0,
                timestamp,
            },
            ClimateStatusMetric {
                measurement: Measurement::Climate,
                category: ClimateStatusMetricCategory::Humidity,
                name: "Room1".to_string(),
                value: 50.0,
                timestamp,
            },
        ];

        let builders = climate_metrics_to_builders(metrics);

        assert_eq!(builders.len(), 2);

        // Verify all can be converted to points
        for builder in builders {
            assert!(builder.to_point().is_ok());
        }
    }
}

