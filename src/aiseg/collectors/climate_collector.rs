//! Climate metric collector implementation.

use anyhow::Result;
use chrono::{DateTime, Local};
use scraper::Html;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::aiseg::client::Client;
use crate::aiseg::collector_base::CollectorBase;
use crate::aiseg::metrics::climate::climate_metrics_to_builders;
use crate::aiseg::parsers::climate_parser::parse_climate_page;
use crate::model::{DataPointBuilder, MetricCollector};

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

impl MetricCollector for ClimateMetricCollector {
    fn collect<'a>(
        &'a self,
        timestamp: DateTime<Local>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn DataPointBuilder>>>> + Send + 'a>> {
        Box::pin(async move {
            let mut all_metrics = Vec::new();

            // Iterate through pages
            for page in 1..=20 {
                let response = self
                    .fetch_page(&format!("/page/airenvironment/41?page={}", page))
                    .await?;
                let document = Html::parse_document(&response);

                match parse_climate_page(&document, timestamp) {
                    Ok(page_metrics) => {
                        if page_metrics.is_empty() {
                            break; // No more data
                        }
                        all_metrics.extend(page_metrics);
                    }
                    Err(_) => break, // Parsing error, likely no more pages
                }
            }

            Ok(climate_metrics_to_builders(all_metrics))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Aiseg2Config;

    fn test_config(url: String) -> Aiseg2Config {
        Aiseg2Config {
            url,
            user: "test".to_string(),
            password: "test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_climate_collector_creation() {
        let config = test_config("http://localhost".to_string());
        let client = Arc::new(Client::new(config));
        let collector = ClimateMetricCollector::new(client);

        // Verify collector is created
        assert!(!collector.client().base_url().is_empty());
    }
}

