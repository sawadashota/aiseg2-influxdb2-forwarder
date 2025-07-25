//! Climate metric collector implementation.

use async_trait::async_trait;
use chrono::{DateTime, Local};
use std::sync::Arc;

use crate::aiseg::client::Client;
use crate::aiseg::collector_base::CollectorBase;
use crate::aiseg::metrics::climate::climate_metrics_to_builders;
use crate::aiseg::pagination::{PageItem, PaginatorBuilder};
use crate::aiseg::parsers::climate_parser::parse_climate_page;
use crate::error::{CollectorError, Result};
use crate::model::{ClimateStatusMetric, DataPointBuilder, MetricCollector};

// Implement PageItem for ClimateStatusMetric to support pagination
impl PageItem for ClimateStatusMetric {
    fn dedup_key(&self) -> String {
        // Use location name and metric type as the key
        format!("{}-{}", self.name, self.category)
    }
}

/// Collector for climate metrics (temperature and humidity) from AiSEG2.
///
/// Fetches environmental data from multiple rooms/locations connected
/// to the AiSEG2 system.
pub struct ClimateMetricCollector {
    client: Arc<Client>,
}

impl ClimateMetricCollector {
    /// Creates a new ClimateMetricCollector instance.
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }
}

impl CollectorBase for ClimateMetricCollector {
    fn client(&self) -> &Arc<Client> {
        &self.client
    }
}

#[async_trait]
impl MetricCollector for ClimateMetricCollector {
    async fn collect(
        &self,
        timestamp: DateTime<Local>,
    ) -> Result<Vec<Box<dyn DataPointBuilder>>, CollectorError> {
        let client = Arc::clone(&self.client);

        let paginator = PaginatorBuilder::new()
            .max_pages(20)
            .fetch_with(move |page| {
                let client = Arc::clone(&client);
                Box::pin(async move {
                    client
                        .get(&format!("/page/airenvironment/41?page={}", page))
                        .await
                })
            })
            .parse_with(move |document| parse_climate_page(document, timestamp))
            .build()
            .map_err(CollectorError::Source)?;

        let all_metrics = paginator
            .collect_all()
            .await
            .map_err(CollectorError::Source)?;
        Ok(climate_metrics_to_builders(all_metrics))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::config::test_aiseg2_config_with_url;

    #[tokio::test]
    async fn test_climate_collector_creation() {
        let config = test_aiseg2_config_with_url("http://localhost");
        let client = Arc::new(Client::new(config));
        let collector = ClimateMetricCollector::new(client);

        // Verify collector is created
        assert!(!collector.client().base_url().is_empty());
    }
}
