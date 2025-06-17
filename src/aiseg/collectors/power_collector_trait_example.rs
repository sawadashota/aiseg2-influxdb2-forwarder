//! Example of using trait-based parsers in a collector.
//!
//! This module demonstrates how collectors can be refactored to use
//! the unified parser trait system, reducing code duplication and
//! improving testability.

use anyhow::Result;
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
use crate::aiseg::pagination::PaginatorBuilder;
use crate::aiseg::parser_adapters::{ParserAdapterBuilder, ConsumptionPageParserAdapter};
use crate::aiseg::parser_traits::HtmlParser;
use crate::model::{DataPointBuilder, MetricCollector};

/// Example collector that uses trait-based parsers.
///
/// This demonstrates how the PowerMetricCollector could be refactored
/// to use the unified parser trait system.
pub struct PowerMetricCollectorWithTraits {
    client: Arc<Client>,
}

impl PowerMetricCollectorWithTraits {
    /// Creates a new instance.
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Collects metrics using trait-based parsers.
    async fn collect_from_main_page(&self) -> MetricResult {
        let response = self.fetch_page("/page/electricflow/111").await?;
        let document = Html::parse_document(&response);

        let mut metrics = Vec::new();

        // Use trait-based parser for total power
        let total_parser = ParserAdapterBuilder::total_power();
        let (gen_kw, cons_kw) = total_parser.parse(&document)?;
        metrics.extend(create_total_power_metrics(gen_kw, cons_kw));

        // Use trait-based parser for generation sources
        let sources_parser = ParserAdapterBuilder::generation_sources();
        let sources = sources_parser.parse(&document)?;
        metrics.extend(create_generation_metrics(sources));

        Ok(metrics)
    }

    /// Collects consumption metrics using trait-based parser.
    async fn collect_consumption_metrics(&self) -> MetricResult {
        let client = Arc::clone(&self.client);
        let parser = ConsumptionPageParserAdapter;

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
            .parse_with(move |document| parser.parse(document))
            .build()?;

        let all_items = paginator.collect_all().await?;

        // Merge duplicates and convert to metrics
        let merged = merge_power_breakdown_metrics(all_items);
        let devices: Vec<(String, f64)> = merged
            .into_iter()
            .map(|m| (m.name.trim_end_matches("(W)").to_string(), m.value as f64))
            .collect();

        Ok(create_consumption_metrics(devices))
    }
}

impl CollectorBase for PowerMetricCollectorWithTraits {
    fn client(&self) -> &Arc<Client> {
        &self.client
    }
}

#[async_trait]
impl MetricCollector for PowerMetricCollectorWithTraits {
    async fn collect(&self, _timestamp: DateTime<Local>) -> Result<Vec<Box<dyn DataPointBuilder>>> {
        let mut all_metrics = Vec::new();

        // Collect from main page
        match self.collect_from_main_page().await {
            Ok(metrics) => all_metrics.extend(metrics),
            Err(e) => tracing::error!("Failed to collect from main page: {}", e),
        }

        // Collect consumption metrics
        match self.collect_consumption_metrics().await {
            Ok(metrics) => all_metrics.extend(metrics),
            Err(e) => tracing::error!("Failed to collect consumption metrics: {}", e),
        }

        Ok(all_metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_trait_based_collector() {
        // This would test the collector with mock responses
        // demonstrating how trait-based parsers improve testability
        // Mock client implementation would be added here
    }
}