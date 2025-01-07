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
