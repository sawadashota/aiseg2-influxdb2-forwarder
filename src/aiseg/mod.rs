mod circuit_daily_total_metric_collector;
mod client;
mod daily_total_metric_collector;
mod helper;

// New modular structure
mod collector_base;
mod collectors;
mod html_parsing;
mod metrics;
mod pagination;
mod parsers;
mod query_builder;

// Re-export from new structure
pub use collectors::{ClimateMetricCollector, PowerMetricCollector};

// Keep existing exports
pub use circuit_daily_total_metric_collector::CircuitDailyTotalMetricCollector;
pub use client::Client;
pub use daily_total_metric_collector::DailyTotalMetricCollector;
