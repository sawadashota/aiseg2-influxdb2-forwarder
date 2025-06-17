//! Power metric collector implementation.

use async_trait::async_trait;
use chrono::{DateTime, Local};
use scraper::Html;
use std::sync::Arc;

use crate::aiseg::client::Client;
use crate::aiseg::collector_base::{CollectorBase, MetricResult};
use crate::aiseg::metrics::power::{
    create_consumption_metrics, create_generation_metrics, create_total_power_metrics,
    merge_power_breakdown_metrics,
};
use crate::aiseg::pagination::{PageItem, PaginatorBuilder};
use crate::aiseg::parsers::power_parser::{
    parse_consumption_page, parse_generation_sources, parse_total_power,
};
use crate::error::{CollectorError, Result};
use crate::model::{DataPointBuilder, MetricCollector, PowerStatusBreakdownMetric};

// Implement PageItem for PowerStatusBreakdownMetric to support pagination
impl PageItem for PowerStatusBreakdownMetric {
    fn dedup_key(&self) -> String {
        // Use the device name as the key for detecting duplicate pages
        self.name.clone()
    }
}

/// Collector for real-time power metrics from AiSEG2.
///
/// Fetches instantaneous power generation and consumption data,
/// including both summary totals and detailed breakdowns.
pub struct PowerMetricCollector {
    client: Arc<Client>,
}

impl PowerMetricCollector {
    /// Creates a new PowerMetricCollector instance.
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Collects metrics from the main electricity flow page.
    async fn collect_from_main_page(&self) -> MetricResult {
        let response = self.fetch_page("/page/electricflow/111").await
            .map_err(CollectorError::Source)?;
        let document = Html::parse_document(&response);

        let mut metrics = Vec::new();

        // Parse and create total metrics
        let (gen_kw, cons_kw) = parse_total_power(&document)
            .map_err(CollectorError::Source)?;
        metrics.extend(create_total_power_metrics(gen_kw, cons_kw));

        // Parse and create generation breakdown
        let sources = parse_generation_sources(&document)
            .map_err(CollectorError::Source)?;
        metrics.extend(create_generation_metrics(sources));

        Ok(metrics)
    }

    /// Collects consumption metrics from paginated detail pages.
    async fn collect_consumption_metrics(&self) -> MetricResult {
        let client = Arc::clone(&self.client);

        let paginator = PaginatorBuilder::new()
            .max_pages(20)
            .fetch_with(move |page| {
                let client = Arc::clone(&client);
                Box::pin(async move {
                    client
                        .get(&format!("/page/electricflow/1113?id={}", page))
                        .await
                })
            })
            .parse_with(parse_consumption_page)
            .build()
            .map_err(CollectorError::Source)?;

        let all_items = paginator.collect_all().await
            .map_err(CollectorError::Source)?;

        // Merge duplicates and convert to metrics
        let merged = merge_power_breakdown_metrics(all_items);
        let devices: Vec<(String, f64)> = merged
            .into_iter()
            .map(|m| (m.name.trim_end_matches("(W)").to_string(), m.value as f64))
            .collect();

        Ok(create_consumption_metrics(devices))
    }
}

impl CollectorBase for PowerMetricCollector {
    fn client(&self) -> &Arc<Client> {
        &self.client
    }
}

#[async_trait]
impl MetricCollector for PowerMetricCollector {
    async fn collect(&self, _timestamp: DateTime<Local>) -> Result<Vec<Box<dyn DataPointBuilder>>, CollectorError> {
        let mut all_metrics = Vec::new();

        // Collect from main page
        all_metrics.extend(self.collect_from_main_page().await?);

        // Collect consumption details
        all_metrics.extend(self.collect_consumption_metrics().await?);

        Ok(all_metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::config::test_aiseg2_config_with_url;

    #[tokio::test]
    async fn test_power_collector_creation() {
        let config = test_aiseg2_config_with_url("http://localhost");
        let client = Arc::new(Client::new(config));
        let collector = PowerMetricCollector::new(client);

        // Verify collector is created
        assert!(!collector.client().base_url().is_empty());
    }
}
