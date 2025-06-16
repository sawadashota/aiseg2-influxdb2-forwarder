//! Test data builders for complex test scenarios.
//!
//! This module provides builder patterns for creating complex test data structures
//! and scenarios used in integration and unit tests.

use crate::model::{
    ClimateStatusMetric, ClimateStatusMetricCategory, DataPointBuilder, Measurement,
    PowerStatusMetric, PowerTotalMetric,
};
use anyhow;
use chrono::{DateTime, Local};
use influxdb2::models::DataPoint as InfluxDataPoint;

/// Builder for creating InfluxDB DataPoint instances for testing.
#[derive(Debug)]
pub struct TestInfluxDataPointBuilder {
    measurement: String,
    tags: Vec<(String, String)>,
    fields: Vec<(String, f64)>,
    timestamp: Option<i64>,
}

impl TestInfluxDataPointBuilder {
    /// Creates a new InfluxDB DataPoint builder.
    pub fn new(measurement: impl Into<String>) -> Self {
        Self {
            measurement: measurement.into(),
            tags: Vec::new(),
            fields: Vec::new(),
            timestamp: None,
        }
    }

    /// Adds a tag to the data point.
    pub fn add_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.push((key.into(), value.into()));
        self
    }

    /// Adds a field to the data point.
    pub fn add_field(mut self, key: impl Into<String>, value: f64) -> Self {
        self.fields.push((key.into(), value));
        self
    }

    /// Sets the timestamp (nanoseconds since epoch).
    pub fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Builds the InfluxDB DataPoint.
    pub fn build(self) -> anyhow::Result<InfluxDataPoint> {
        let mut builder = InfluxDataPoint::builder(&self.measurement);

        for (key, value) in self.tags {
            builder = builder.tag(key, value);
        }

        for (key, value) in self.fields {
            builder = builder.field(key, value);
        }

        if let Some(ts) = self.timestamp {
            builder = builder.timestamp(ts);
        }

        builder
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build InfluxDB data point: {}", e))
    }
}

/// Builder for creating test metric collections.
pub struct MetricCollectionBuilder {
    metrics: Vec<Box<dyn DataPointBuilder>>,
}

impl MetricCollectionBuilder {
    /// Creates a new metric collection builder.
    pub fn new() -> Self {
        Self {
            metrics: Vec::new(),
        }
    }

    /// Adds a single metric to the collection.
    pub fn add_metric(mut self, metric: Box<dyn DataPointBuilder>) -> Self {
        self.metrics.push(metric);
        self
    }

    /// Adds multiple metrics to the collection.
    pub fn add_metrics(mut self, metrics: Vec<Box<dyn DataPointBuilder>>) -> Self {
        self.metrics.extend(metrics);
        self
    }

    /// Adds a power generation/consumption pair.
    pub fn add_power_pair(mut self, generation: i64, consumption: i64) -> Self {
        self.metrics.push(Box::new(PowerStatusMetric {
            measurement: Measurement::Power,
            name: "発電".to_string(),
            value: generation,
        }));
        self.metrics.push(Box::new(PowerStatusMetric {
            measurement: Measurement::Power,
            name: "消費".to_string(),
            value: consumption,
        }));
        self
    }

    /// Adds a climate temperature/humidity pair.
    pub fn add_climate_pair(mut self, location: &str, temperature: f64, humidity: f64) -> Self {
        let timestamp = Local::now();
        self.metrics.push(Box::new(ClimateStatusMetric {
            measurement: Measurement::Climate,
            category: ClimateStatusMetricCategory::Temperature,
            name: location.to_string(),
            value: temperature,
            timestamp,
        }));
        self.metrics.push(Box::new(ClimateStatusMetric {
            measurement: Measurement::Climate,
            category: ClimateStatusMetricCategory::Humidity,
            name: location.to_string(),
            value: humidity,
            timestamp,
        }));
        self
    }

    /// Adds a daily total metric.
    pub fn add_daily_total(mut self, name: &str, value: f64, date: DateTime<Local>) -> Self {
        self.metrics.push(Box::new(PowerTotalMetric {
            measurement: Measurement::DailyTotal,
            name: name.to_string(),
            value,
            date,
        }));
        self
    }

    /// Adds a circuit daily total metric.
    pub fn add_circuit_total(mut self, circuit: &str, value: f64, date: DateTime<Local>) -> Self {
        self.metrics.push(Box::new(PowerTotalMetric {
            measurement: Measurement::CircuitDailyTotal,
            name: circuit.to_string(),
            value,
            date,
        }));
        self
    }

    /// Builds the metric collection.
    pub fn build(self) -> Vec<Box<dyn DataPointBuilder>> {
        self.metrics
    }
}

/// Builder for creating test scenarios with multiple related data points.
pub struct TestScenarioBuilder {
    name: String,
    date: DateTime<Local>,
    power_data: Option<(f64, f64)>,        // (generation, consumption)
    climate_data: Vec<(String, f64, f64)>, // (location, temp, humidity)
    daily_totals: Vec<(String, f64)>,
    circuit_totals: Vec<(String, f64)>,
}

impl TestScenarioBuilder {
    /// Creates a new test scenario builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            date: Local::now(),
            power_data: None,
            climate_data: Vec::new(),
            daily_totals: Vec::new(),
            circuit_totals: Vec::new(),
        }
    }

    /// Sets the scenario date.
    pub fn with_date(mut self, date: DateTime<Local>) -> Self {
        self.date = date;
        self
    }

    /// Adds power generation and consumption data.
    pub fn with_power_data(mut self, generation: f64, consumption: f64) -> Self {
        self.power_data = Some((generation, consumption));
        self
    }

    /// Adds climate data for a location.
    pub fn add_climate_data(
        mut self,
        location: impl Into<String>,
        temp: f64,
        humidity: f64,
    ) -> Self {
        self.climate_data.push((location.into(), temp, humidity));
        self
    }

    /// Adds a daily total metric.
    pub fn add_daily_total(mut self, name: impl Into<String>, value: f64) -> Self {
        self.daily_totals.push((name.into(), value));
        self
    }

    /// Adds a circuit total metric.
    pub fn add_circuit_total(mut self, circuit: impl Into<String>, value: f64) -> Self {
        self.circuit_totals.push((circuit.into(), value));
        self
    }

    /// Builds all data points for the scenario.
    pub fn build(self) -> Vec<Box<dyn DataPointBuilder>> {
        let mut builder = MetricCollectionBuilder::new();

        // Add power data
        if let Some((gen, cons)) = self.power_data {
            builder = builder.add_power_pair((gen * 1000.0) as i64, (cons * 1000.0) as i64);
        }

        // Add climate data
        for (location, temp, humidity) in self.climate_data {
            builder = builder.add_climate_pair(&location, temp, humidity);
        }

        // Add daily totals
        for (name, value) in self.daily_totals {
            builder = builder.add_daily_total(&name, value, self.date);
        }

        // Add circuit totals
        for (circuit, value) in self.circuit_totals {
            builder = builder.add_circuit_total(&circuit, value, self.date);
        }

        builder.build()
    }

    /// Gets the scenario name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // Removed test_data_point_builder as TestDataPointBuilder was removed

    #[test]
    fn test_influx_data_point_builder() {
        let _point = TestInfluxDataPointBuilder::new("test_measurement")
            .add_tag("location", "living_room")
            .add_field("temperature", 23.5)
            .add_field("humidity", 65.0)
            .build()
            .unwrap();

        // The actual validation would depend on InfluxDB's DataPoint API
        // Successfully built the point
    }

    #[test]
    fn test_metric_collection_builder() {
        let date = Local.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let metrics = MetricCollectionBuilder::new()
            .add_power_pair(2500, 3800)
            .add_climate_pair("リビング", 23.5, 65.0)
            .add_daily_total("発電量(kWh)", 100.0, date)
            .build();

        assert_eq!(metrics.len(), 5);
        // Can't directly assert on trait objects, but we can verify the count
    }

    #[test]
    fn test_scenario_builder() {
        let date = Local.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let scenario = TestScenarioBuilder::new("Test Scenario")
            .with_date(date)
            .with_power_data(2.5, 3.8)
            .add_climate_data("リビング", 23.5, 65.0)
            .add_climate_data("寝室", 22.0, 60.0)
            .add_daily_total("発電量(kWh)", 100.0)
            .add_circuit_total("EV", 50.0)
            .build();

        assert_eq!(scenario.len(), 8); // 2 power + 4 climate + 1 daily + 1 circuit
    }
}
