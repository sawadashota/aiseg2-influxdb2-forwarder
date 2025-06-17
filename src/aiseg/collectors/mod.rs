//! Collector implementations for different AiSEG2 data types.

pub mod climate_collector;
pub mod power_collector;

#[cfg(test)]
mod trait_usage_example;

pub use climate_collector::ClimateMetricCollector;
pub use power_collector::PowerMetricCollector;
