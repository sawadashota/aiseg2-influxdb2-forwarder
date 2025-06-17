use crate::error::{Result, StorageError};
use chrono::{DateTime, Local};
use influxdb2::models::DataPoint;

use super::traits::DataPointBuilder;
use super::types::{ClimateStatusMetricCategory, Measurement, PowerStatusBreakdownMetricCategory};

/// Represents a real-time power status metric.
///
/// Used for summary-level power metrics like total generation,
/// total consumption, and net power flow (buying/selling).
#[derive(Debug)]
pub struct PowerStatusMetric {
    /// The measurement type (should be Measurement::Power)
    pub measurement: Measurement,
    /// Descriptive name of the metric (e.g., "総発電電力(W)")
    pub name: String,
    /// The power value in watts (W)
    pub value: i64,
}

impl DataPointBuilder for PowerStatusMetric {
    fn to_point(&self) -> Result<DataPoint, StorageError> {
        DataPoint::builder(self.measurement.to_string().as_str())
            .tag("summary", self.name.clone())
            .field("value", self.value)
            .build()
            .map_err(|e| StorageError::InvalidDataPoint(format!("Failed to build PowerStatusMetric: {}", e)))
    }
}

/// Represents a detailed breakdown of power metrics.
///
/// Used for component-level power metrics that show individual
/// sources of generation or consumption (e.g., solar panels,
/// specific appliances).
#[derive(Debug, Clone, PartialEq)]
pub struct PowerStatusBreakdownMetric {
    /// The measurement type (should be Measurement::Power)
    pub measurement: Measurement,
    /// Whether this is generation or consumption
    pub category: PowerStatusBreakdownMetricCategory,
    /// Name of the specific component (e.g., "太陽光(W)")
    pub name: String,
    /// The power value in watts (W)
    pub value: i64,
}

impl DataPointBuilder for PowerStatusBreakdownMetric {
    fn to_point(&self) -> Result<DataPoint, StorageError> {
        DataPoint::builder(self.measurement.to_string().as_str())
            .tag("detail-type", self.category.to_string())
            .tag("detail-section", self.name.clone())
            .field("value", self.value)
            .build()
            .map_err(|e| StorageError::InvalidDataPoint(format!("Failed to build PowerStatusBreakdownMetric: {}", e)))
    }
}

/// Represents daily or periodic total metrics.
///
/// Used for aggregated metrics over a time period, such as
/// daily energy consumption, daily energy generation, or
/// daily resource usage (water, gas).
#[derive(Debug)]
pub struct PowerTotalMetric {
    /// The measurement type (DailyTotal or CircuitDailyTotal)
    pub measurement: Measurement,
    /// Descriptive name with unit (e.g., "発電量(kWh)")
    pub name: String,
    /// The accumulated value (kWh, liters, cubic meters)
    pub value: f64,
    /// The date for which this total applies
    pub date: DateTime<Local>,
}

impl DataPointBuilder for PowerTotalMetric {
    fn to_point(&self) -> Result<DataPoint, StorageError> {
        let timestamp = self.date
            .timestamp_nanos_opt()
            .ok_or_else(|| StorageError::InvalidDataPoint("Timestamp overflow".to_string()))?;
        
        DataPoint::builder(self.measurement.to_string().as_str())
            .tag("detail-section", self.name.clone())
            .field("value", self.value)
            .timestamp(timestamp)
            .build()
            .map_err(|e| StorageError::InvalidDataPoint(format!("Failed to build PowerTotalMetric: {}", e)))
    }
}

/// Represents environmental climate metrics.
///
/// Used for room-specific temperature and humidity readings
/// from AiSEG2's climate monitoring sensors.
#[derive(Debug, Clone, PartialEq)]
pub struct ClimateStatusMetric {
    /// The measurement type (should be Measurement::Climate)
    pub measurement: Measurement,
    /// Whether this is temperature or humidity
    pub category: ClimateStatusMetricCategory,
    /// Location name (e.g., "Living Room")
    pub name: String,
    /// The measured value (°C for temperature, % for humidity)
    pub value: f64,
    /// When this measurement was taken
    pub timestamp: DateTime<Local>,
}

impl DataPointBuilder for ClimateStatusMetric {
    fn to_point(&self) -> Result<DataPoint, StorageError> {
        let timestamp = self.timestamp
            .timestamp_nanos_opt()
            .ok_or_else(|| StorageError::InvalidDataPoint("Timestamp overflow".to_string()))?;
        
        DataPoint::builder(self.measurement.to_string().as_str())
            .tag("detail-type", self.category.to_string())
            .tag("detail-section", self.name.clone())
            .field("value", self.value)
            .timestamp(timestamp)
            .build()
            .map_err(|e| StorageError::InvalidDataPoint(format!("Failed to build ClimateStatusMetric: {}", e)))
    }
}
