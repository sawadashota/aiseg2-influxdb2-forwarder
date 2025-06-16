//! Test fixtures and common test data.
//!
//! This module provides common test data, constants, and fixture generators
//! used across the test suite.

use chrono::{DateTime, Local, TimeZone, Timelike};

/// Common test data constants.
pub mod constants {
    /// Standard test generation value in kW.
    pub const TEST_GENERATION_KW: f64 = 2.5;

    /// Standard test consumption value in kW.
    pub const TEST_CONSUMPTION_KW: f64 = 3.8;

    /// Standard test temperature value in Celsius.
    pub const TEST_TEMPERATURE: f64 = 23.5;

    /// Standard test humidity value in percentage.
    pub const TEST_HUMIDITY: f64 = 65.0;

    /// Standard test daily total value in kWh.
    pub const TEST_DAILY_TOTAL_KWH: f64 = 123.45;

    /// Common device names for testing.
    pub const TEST_DEVICES: &[&str] = &["エアコン", "冷蔵庫", "テレビ", "照明"];

    /// Common circuit names and IDs for testing.
    pub const TEST_CIRCUITS: &[(&str, &str)] = &[
        ("30", "EV"),
        ("27", "リビングエアコン"),
        ("26", "主寝室エアコン"),
        ("25", "洋室２エアコン"),
    ];

    /// Common location names for climate testing.
    pub const TEST_LOCATIONS: &[&str] = &["リビング", "寝室", "子供部屋", "書斎"];

    /// Common graph IDs used in testing.
    pub mod graph_ids {
        pub const GENERATION: &str = "51111";
        pub const CONSUMPTION: &str = "52111";
        pub const GRID_BUY: &str = "53111";
        pub const GRID_SELL: &str = "54111";
        pub const HOT_WATER: &str = "55111";
        pub const GAS: &str = "57111";
    }
}

/// Test date and time generators.
pub mod dates {
    use super::*;

    /// Creates a test date at the beginning of the day (00:00:00).
    pub fn test_date_beginning() -> DateTime<Local> {
        Local.with_ymd_and_hms(2024, 6, 15, 0, 0, 0).unwrap()
    }

    /// Creates a test date at noon (12:00:00).
    pub fn test_date_noon() -> DateTime<Local> {
        Local.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap()
    }

    /// Creates a test date at a specific time.
    pub fn test_date_at(hour: u32, minute: u32, second: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2024, 6, 15, hour, minute, second)
            .unwrap()
    }

    /// Creates a test date for a specific day of the month.
    pub fn test_date_day(day: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2024, 6, day, 10, 0, 0).unwrap()
    }
}

/// Sample data generators for different metric types.
pub mod samples {
    use super::constants::*;
    use super::dates::*;
    use crate::model::{ClimateStatusMetric, Measurement, PowerStatusMetric, PowerTotalMetric};

    /// Creates a sample power generation metric.
    pub fn power_generation_metric() -> PowerStatusMetric {
        PowerStatusMetric {
            measurement: Measurement::Power,
            name: "発電".to_string(),
            value: (TEST_GENERATION_KW * 1000.0) as i64, // Convert kW to W
        }
    }

    /// Creates a sample power consumption metric.
    pub fn power_consumption_metric() -> PowerStatusMetric {
        PowerStatusMetric {
            measurement: Measurement::Power,
            name: "消費".to_string(),
            value: (TEST_CONSUMPTION_KW * 1000.0) as i64, // Convert kW to W
        }
    }

    /// Creates a sample climate temperature metric.
    pub fn climate_temperature_metric(location: &str) -> ClimateStatusMetric {
        ClimateStatusMetric {
            measurement: Measurement::Climate,
            category: crate::model::ClimateStatusMetricCategory::Temperature,
            name: location.to_string(),
            value: TEST_TEMPERATURE,
            timestamp: test_date_noon(),
        }
    }

    /// Creates a sample climate humidity metric.
    pub fn climate_humidity_metric(location: &str) -> ClimateStatusMetric {
        ClimateStatusMetric {
            measurement: Measurement::Climate,
            category: crate::model::ClimateStatusMetricCategory::Humidity,
            name: location.to_string(),
            value: TEST_HUMIDITY,
            timestamp: test_date_noon(),
        }
    }

    /// Creates a sample daily total metric.
    pub fn daily_total_metric(name: &str, value: f64) -> PowerTotalMetric {
        PowerTotalMetric {
            measurement: Measurement::DailyTotal,
            name: name.to_string(),
            value,
            date: test_date_beginning(),
        }
    }

    /// Creates a sample circuit daily total metric.
    pub fn circuit_daily_total_metric(circuit_name: &str, value: f64) -> PowerTotalMetric {
        PowerTotalMetric {
            measurement: Measurement::CircuitDailyTotal,
            name: circuit_name.to_string(),
            value,
            date: test_date_beginning(),
        }
    }

    /// Creates a set of sample power status metrics.
    pub fn power_status_set() -> Vec<Box<dyn crate::model::DataPointBuilder>> {
        vec![
            Box::new(power_generation_metric()),
            Box::new(power_consumption_metric()),
        ]
    }

    /// Creates a set of sample climate metrics for a location.
    pub fn climate_status_set(location: &str) -> Vec<Box<dyn crate::model::DataPointBuilder>> {
        vec![
            Box::new(climate_temperature_metric(location)),
            Box::new(climate_humidity_metric(location)),
        ]
    }

    /// Creates a full set of daily total metrics.
    pub fn daily_total_set() -> Vec<Box<dyn crate::model::DataPointBuilder>> {
        vec![
            Box::new(daily_total_metric("発電量(kWh)", 100.0)),
            Box::new(daily_total_metric("消費量(kWh)", 200.0)),
            Box::new(daily_total_metric("買電量(kWh)", 50.0)),
            Box::new(daily_total_metric("売電量(kWh)", 75.0)),
            Box::new(daily_total_metric("給湯量(L)", 300.0)),
            Box::new(daily_total_metric("ガス量(㎥)", 25.5)),
        ]
    }
}

/// URL builders for test endpoints.
pub mod urls {
    /// Creates a graph URL for daily totals.
    pub fn daily_total_url(base: &str, graph_id: &str, query: &str) -> String {
        format!("{}/page/graph/{}?data={}", base, graph_id, query)
    }

    /// Creates a circuit graph URL.
    pub fn circuit_url(base: &str, query: &str) -> String {
        format!("{}/page/graph/584?data={}", base, query)
    }

    /// Creates a power status URL.
    pub fn power_status_url(base: &str) -> String {
        format!("{}/page/top", base)
    }

    /// Creates a climate status URL.
    pub fn climate_status_url(base: &str) -> String {
        format!("{}/page/climate", base)
    }

    /// Creates generation details URL for a specific page.
    pub fn generation_details_url(base: &str, page: u32) -> String {
        format!("{}/page/generation?page={}", base, page)
    }

    /// Creates consumption devices URL for a specific page.
    pub fn consumption_devices_url(base: &str, page: u32) -> String {
        format!("{}/page/consumption_devices?page={}", base, page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(constants::TEST_GENERATION_KW, 2.5);
        assert_eq!(constants::TEST_CONSUMPTION_KW, 3.8);
        assert_eq!(constants::TEST_DEVICES.len(), 4);
        assert_eq!(constants::TEST_CIRCUITS.len(), 4);
    }

    #[test]
    fn test_date_generators() {
        let beginning = dates::test_date_beginning();
        assert_eq!(beginning.hour(), 0);
        assert_eq!(beginning.minute(), 0);

        let noon = dates::test_date_noon();
        assert_eq!(noon.hour(), 12);

        let custom = dates::test_date_at(15, 30, 45);
        assert_eq!(custom.hour(), 15);
        assert_eq!(custom.minute(), 30);
        assert_eq!(custom.second(), 45);
    }

    #[test]
    fn test_sample_data_generators() {
        let power_gen = samples::power_generation_metric();
        assert_eq!(power_gen.name, "発電");
        assert_eq!(
            power_gen.value,
            (constants::TEST_GENERATION_KW * 1000.0) as i64
        );

        let climate_temp = samples::climate_temperature_metric("リビング");
        assert_eq!(climate_temp.name, "リビング");
        assert_eq!(climate_temp.value, constants::TEST_TEMPERATURE);
        assert_eq!(
            climate_temp.category,
            crate::model::ClimateStatusMetricCategory::Temperature
        );

        let daily_total_set = samples::daily_total_set();
        assert_eq!(daily_total_set.len(), 6);
    }

    #[test]
    fn test_url_builders() {
        let base = "http://test.local";

        let daily_url = urls::daily_total_url(base, "51111", "test_query");
        assert_eq!(
            daily_url,
            "http://test.local/page/graph/51111?data=test_query"
        );

        let power_url = urls::power_status_url(base);
        assert_eq!(power_url, "http://test.local/page/top");
    }
}
