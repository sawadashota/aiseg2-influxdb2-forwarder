use std::fmt;

/// Represents the type of measurement being collected.
///
/// Each measurement type corresponds to a different InfluxDB measurement
/// (table) where the data will be stored.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Measurement {
    /// Real-time power metrics (generation, consumption, etc.)
    Power,
    /// Daily aggregated totals for various metrics
    DailyTotal,
    /// Daily totals for specific electrical circuits
    CircuitDailyTotal,
    /// Environmental metrics (temperature, humidity)
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

/// Units of measurement used in the system.
///
/// These units are appended to metric names to provide
/// clear context about what is being measured.
pub enum Unit {
    /// Watts (W) - for instantaneous power
    Watt,
    /// Kilowatt-hours (kWh) - for energy over time
    Kwh,
    /// Liters (L) - for water volume
    Liter,
    /// Cubic meters (㎥) - for gas volume
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

/// Categories for power breakdown metrics.
///
/// Distinguishes between power generation sources and
/// power consumption endpoints.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum PowerStatusBreakdownMetricCategory {
    /// Power generation sources (solar, fuel cell, etc.)
    Generation,
    /// Power consumption endpoints (appliances, circuits, etc.)
    Consumption,
}

impl fmt::Display for PowerStatusBreakdownMetricCategory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PowerStatusBreakdownMetricCategory::Generation => write!(f, "generation"),
            PowerStatusBreakdownMetricCategory::Consumption => write!(f, "consumption"),
        }
    }
}

/// Categories for climate metrics.
///
/// Distinguishes between different types of environmental
/// measurements from climate sensors.
#[derive(Debug, PartialEq, Clone)]
pub enum ClimateStatusMetricCategory {
    /// Temperature in degrees Celsius
    Temperature,
    /// Relative humidity percentage
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
