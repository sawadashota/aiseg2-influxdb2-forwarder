//! Power metric building utilities.

use crate::aiseg::helper::{kilowatts_to_watts, truncate_to_i64};
use crate::model::{
    DataPointBuilder, Measurement, PowerStatusBreakdownMetric, PowerStatusBreakdownMetricCategory,
    PowerStatusMetric, Unit,
};

/// Creates total power metrics from generation and consumption values.
///
/// # Arguments
/// * `generation_kw` - Total generation in kilowatts
/// * `consumption_kw` - Total consumption in kilowatts
///
/// # Returns
/// Vector containing generation, consumption, and net power metrics
pub fn create_total_power_metrics(
    generation_kw: f64,
    consumption_kw: f64,
) -> Vec<Box<dyn DataPointBuilder>> {
    let generation_w = kilowatts_to_watts(generation_kw);
    let consumption_w = kilowatts_to_watts(consumption_kw);

    vec![
        Box::new(PowerStatusMetric {
            measurement: Measurement::Power,
            name: format!("総発電電力({})", Unit::Watt),
            value: generation_w,
        }),
        Box::new(PowerStatusMetric {
            measurement: Measurement::Power,
            name: format!("総消費電力({})", Unit::Watt),
            value: consumption_w,
        }),
        Box::new(PowerStatusMetric {
            measurement: Measurement::Power,
            name: format!("売買電力({})", Unit::Watt),
            value: generation_w - consumption_w,
        }),
    ]
}

/// Creates generation breakdown metrics from source details.
///
/// # Arguments
/// * `sources` - Vector of (name, value_in_watts) tuples
///
/// # Returns
/// Vector of power breakdown metrics for each generation source
pub fn create_generation_metrics(sources: Vec<(String, f64)>) -> Vec<Box<dyn DataPointBuilder>> {
    sources
        .into_iter()
        .map(|(name, watts)| {
            Box::new(PowerStatusBreakdownMetric {
                measurement: Measurement::Power,
                category: PowerStatusBreakdownMetricCategory::Generation,
                name: format!("{}({})", name, Unit::Watt),
                value: truncate_to_i64(watts),
            }) as Box<dyn DataPointBuilder>
        })
        .collect()
}

/// Creates consumption breakdown metrics from device details.
///
/// # Arguments
/// * `devices` - Vector of (name, value_in_watts) tuples
///
/// # Returns
/// Vector of power breakdown metrics for each consumption device
pub fn create_consumption_metrics(devices: Vec<(String, f64)>) -> Vec<Box<dyn DataPointBuilder>> {
    devices
        .into_iter()
        .map(|(name, watts)| {
            Box::new(PowerStatusBreakdownMetric {
                measurement: Measurement::Power,
                category: PowerStatusBreakdownMetricCategory::Consumption,
                name: format!("{}({})", name, Unit::Watt),
                value: truncate_to_i64(watts),
            }) as Box<dyn DataPointBuilder>
        })
        .collect()
}

/// Merges power breakdown metrics with the same name by summing their values.
///
/// This is used when the same device appears on multiple pages.
pub fn merge_power_breakdown_metrics(
    metrics: Vec<PowerStatusBreakdownMetric>,
) -> Vec<PowerStatusBreakdownMetric> {
    use std::collections::HashMap;

    let mut merged: HashMap<String, PowerStatusBreakdownMetric> = HashMap::new();

    for metric in metrics {
        merged
            .entry(metric.name.clone())
            .and_modify(|e| e.value += metric.value)
            .or_insert(metric);
    }

    merged.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_total_power_metrics() {
        let metrics = create_total_power_metrics(2.5, 3.8);

        assert_eq!(metrics.len(), 3);

        // Verify all metrics can be converted to points
        for metric in metrics {
            assert!(metric.to_point().is_ok());
        }
    }

    #[test]
    fn test_create_generation_metrics() {
        let sources = vec![
            ("太陽光".to_string(), 2500.0),
            ("燃料電池".to_string(), 500.0),
        ];

        let metrics = create_generation_metrics(sources);

        assert_eq!(metrics.len(), 2);

        for metric in metrics {
            assert!(metric.to_point().is_ok());
        }
    }

    #[test]
    fn test_merge_power_breakdown_metrics() {
        let metrics = vec![
            PowerStatusBreakdownMetric {
                measurement: Measurement::Power,
                category: PowerStatusBreakdownMetricCategory::Consumption,
                name: "エアコン(W)".to_string(),
                value: 100,
            },
            PowerStatusBreakdownMetric {
                measurement: Measurement::Power,
                category: PowerStatusBreakdownMetricCategory::Consumption,
                name: "エアコン(W)".to_string(),
                value: 200,
            },
            PowerStatusBreakdownMetric {
                measurement: Measurement::Power,
                category: PowerStatusBreakdownMetricCategory::Consumption,
                name: "冷蔵庫(W)".to_string(),
                value: 50,
            },
        ];

        let merged = merge_power_breakdown_metrics(metrics);

        assert_eq!(merged.len(), 2);

        let aircon = merged.iter().find(|m| m.name.contains("エアコン")).unwrap();
        assert_eq!(aircon.value, 300);

        let fridge = merged.iter().find(|m| m.name.contains("冷蔵庫")).unwrap();
        assert_eq!(fridge.value, 50);
    }
}

