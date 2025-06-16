use crate::aiseg::client::Client;
use crate::aiseg::helper::day_of_beginning;
use crate::aiseg::html_parsing::parse_graph_page;
use crate::aiseg::query_builder::make_daily_total_query;
use crate::model::{DataPointBuilder, Measurement, MetricCollector, PowerTotalMetric, Unit};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Local};
use scraper::Html;
use std::sync::Arc;

/// Collector for daily total metrics from AiSEG2 system.
///
/// This collector retrieves daily aggregated metrics for power generation,
/// consumption, buying, selling, hot water consumption, and gas consumption.
/// It runs on a 60-second interval and fetches data for the current day.
pub struct DailyTotalMetricCollector {
    client: Arc<Client>,
}

impl DailyTotalMetricCollector {
    /// Creates a new instance of DailyTotalMetricCollector.
    ///
    /// # Arguments
    ///
    /// * `client` - Shared AiSEG2 client for making HTTP requests
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Collects a specific daily total metric by graph ID.
    ///
    /// # Arguments
    ///
    /// * `date` - The date to collect metrics for (will be normalized to beginning of day)
    /// * `graph_id` - The AiSEG2 graph ID for the specific metric type
    /// * `unit` - The unit of measurement for the metric
    ///
    /// # Returns
    ///
    /// A PowerTotalMetric containing the collected data or an error if collection fails
    async fn collect_by_graph_id(
        &self,
        date: DateTime<Local>,
        graph_id: &str,
        unit: Unit,
    ) -> Result<PowerTotalMetric> {
        let the_day = day_of_beginning(&date)?;
        let response = self
            .client
            .get(&format!(
                "/page/graph/{}?data={}",
                graph_id,
                make_daily_total_query(the_day)
            ))
            .await?;
        let document = Html::parse_document(&response);

        // Use the new parse_graph_page utility
        let (name, _watts_value) = parse_graph_page(&document, None, None)?;
        // For daily totals, we need the kWh value, not watts
        // So we'll use the extract_value function directly
        use crate::aiseg::html_parsing::extract_value;
        let value: f64 = extract_value(&document, "#val_kwh")?;

        Ok(PowerTotalMetric {
            measurement: Measurement::DailyTotal,
            name: format!("{}({})", name, unit),
            value,
            date: the_day,
        })
    }
}

#[async_trait]
impl MetricCollector for DailyTotalMetricCollector {
    /// Collects all daily total metrics for the given timestamp.
    ///
    /// Fetches the following metrics from AiSEG2:
    /// - Graph ID 51111: Daily total power generation (kWh)
    /// - Graph ID 52111: Daily total power consumption (kWh)
    /// - Graph ID 53111: Daily total power buying (kWh)
    /// - Graph ID 54111: Daily total power selling (kWh)
    /// - Graph ID 55111: Daily total hot water consumption (L)
    /// - Graph ID 57111: Daily total gas consumption (㎥)
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp for collection (normalized to beginning of day)
    ///
    /// # Returns
    ///
    /// A vector of DataPointBuilder instances or an error if any collection fails
    async fn collect(&self, timestamp: DateTime<Local>) -> Result<Vec<Box<dyn DataPointBuilder>>> {
        Ok(vec![
            // DailyTotalPowerGeneration
            self.collect_by_graph_id(timestamp, "51111", Unit::Kwh)
                .await?,
            // DailyTotalPowerConsumption
            self.collect_by_graph_id(timestamp, "52111", Unit::Kwh)
                .await?,
            // DailyTotalPowerBuying
            self.collect_by_graph_id(timestamp, "53111", Unit::Kwh)
                .await?,
            // DailyTotalPowerSelling
            self.collect_by_graph_id(timestamp, "54111", Unit::Kwh)
                .await?,
            // DailyTotalHotWaterConsumption
            self.collect_by_graph_id(timestamp, "55111", Unit::Liter)
                .await?,
            // DailyTotalGasConsumption
            self.collect_by_graph_id(timestamp, "57111", Unit::CubicMeter)
                .await?,
        ]
        .into_iter()
        .map(|x| Box::new(x) as Box<dyn DataPointBuilder>)
        .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aiseg::query_builder::make_daily_total_query;
    use crate::aiseg::test_utils::test_config;
    use chrono::TimeZone;

    fn create_html_response(title: &str, value: &str) -> String {
        format!(
            r#"<html><body>
                <div id="h_title">{}</div>
                <div id="val_kwh">{}</div>
            </body></html>"#,
            title, value
        )
    }

    mod succeeds {
        use super::*;

        #[tokio::test]
        async fn test_collect_by_graph_id_parses_valid_html() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local.with_ymd_and_hms(2024, 6, 6, 10, 0, 0).unwrap();
            let expected_query = make_daily_total_query(day_of_beginning(&date).unwrap());

            let _mock = server
                .mock(
                    "GET",
                    format!("/page/graph/51111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_html_response("太陽光発電量", "123.45"))
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result = collector
                .collect_by_graph_id(date, "51111", Unit::Kwh)
                .await;

            assert!(result.is_ok());
            let metric = result.unwrap();
            assert_eq!(metric.value, 123.45);
            assert_eq!(metric.name, "太陽光発電量(kWh)");
            assert_eq!(metric.measurement, Measurement::DailyTotal);
            assert_eq!(metric.date, day_of_beginning(&date).unwrap());
        }

        #[tokio::test]
        async fn test_collect_by_graph_id_returns_correct_metric() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local.with_ymd_and_hms(2024, 6, 6, 15, 30, 45).unwrap();
            let expected_date = day_of_beginning(&date).unwrap();
            let expected_query = make_daily_total_query(expected_date);

            let _mock = server
                .mock(
                    "GET",
                    format!("/page/graph/52111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_html_response("消費電力量", "456.78"))
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result = collector
                .collect_by_graph_id(date, "52111", Unit::Kwh)
                .await;

            assert!(result.is_ok());
            let metric = result.unwrap();
            assert_eq!(metric.measurement, Measurement::DailyTotal);
            assert_eq!(metric.name, "消費電力量(kWh)");
            assert_eq!(metric.value, 456.78);
            assert_eq!(metric.date, expected_date);
        }

        #[tokio::test]
        async fn test_collect_by_graph_id_handles_different_units() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_query = make_daily_total_query(day_of_beginning(&date).unwrap());

            // Test kWh unit
            let _mock1 = server
                .mock(
                    "GET",
                    format!("/page/graph/51111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_html_response("電力", "100.0"))
                .create_async()
                .await;

            let config = test_config(mock_url.clone());
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result1 = collector
                .collect_by_graph_id(date, "51111", Unit::Kwh)
                .await;
            assert!(result1.is_ok());
            assert_eq!(result1.unwrap().name, "電力(kWh)");

            // Test Liter unit
            let _mock2 = server
                .mock(
                    "GET",
                    format!("/page/graph/55111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_html_response("給湯量", "200.5"))
                .create_async()
                .await;

            let config = test_config(mock_url.clone());
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result2 = collector
                .collect_by_graph_id(date, "55111", Unit::Liter)
                .await;
            assert!(result2.is_ok());
            assert_eq!(result2.unwrap().name, "給湯量(L)");

            // Test CubicMeter unit
            let _mock3 = server
                .mock(
                    "GET",
                    format!("/page/graph/57111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_html_response("ガス使用量", "15.3"))
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result3 = collector
                .collect_by_graph_id(date, "57111", Unit::CubicMeter)
                .await;
            assert!(result3.is_ok());
            assert_eq!(result3.unwrap().name, "ガス使用量(㎥)");
        }

        #[tokio::test]
        async fn test_collect_returns_all_six_metrics() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_date = day_of_beginning(&date).unwrap();
            let expected_query = make_daily_total_query(expected_date);

            // Mock all six metric responses
            let metrics = vec![
                ("51111", "発電量", "100.0"),
                ("52111", "消費量", "200.0"),
                ("53111", "買電量", "50.0"),
                ("54111", "売電量", "75.0"),
                ("55111", "給湯量", "300.0"),
                ("57111", "ガス量", "25.5"),
            ];

            for (graph_id, title, value) in &metrics {
                let _mock = server
                    .mock(
                        "GET",
                        format!("/page/graph/{}?data={}", graph_id, expected_query).as_str(),
                    )
                    .with_status(200)
                    .with_body(create_html_response(title, value))
                    .create_async()
                    .await;
            }

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result = collector.collect(date).await;

            assert!(result.is_ok());
            let data_points = result.unwrap();
            assert_eq!(data_points.len(), 6);

            // Verify each data point can be converted to InfluxDB format
            for dp in data_points {
                assert!(dp.to_point().is_ok());
            }
        }

        #[tokio::test]
        async fn test_collect_with_mixed_values() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_date = day_of_beginning(&date).unwrap();
            let expected_query = make_daily_total_query(expected_date);

            // Mock responses with different values including edge cases
            let _mock1 = server
                .mock(
                    "GET",
                    format!("/page/graph/51111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_html_response("発電", "0.0"))
                .create_async()
                .await;

            let _mock2 = server
                .mock(
                    "GET",
                    format!("/page/graph/52111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_html_response("消費", "999.999"))
                .create_async()
                .await;

            let _mock3 = server
                .mock(
                    "GET",
                    format!("/page/graph/53111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_html_response("買電", "1"))
                .create_async()
                .await;

            let _mock4 = server
                .mock(
                    "GET",
                    format!("/page/graph/54111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_html_response("売電", "0.001"))
                .create_async()
                .await;

            let _mock5 = server
                .mock(
                    "GET",
                    format!("/page/graph/55111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_html_response("給湯", "1234.5678"))
                .create_async()
                .await;

            let _mock6 = server
                .mock(
                    "GET",
                    format!("/page/graph/57111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_html_response("ガス", "99.99"))
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result = collector.collect(date).await;

            assert!(result.is_ok());
            let data_points = result.unwrap();
            assert_eq!(data_points.len(), 6);
        }
    }

    mod fails {
        use super::*;

        #[tokio::test]
        async fn test_collect_by_graph_id_missing_h_title() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_query = make_daily_total_query(day_of_beginning(&date).unwrap());

            // HTML without #h_title element
            let html_without_title = r#"<html><body><div id="val_kwh">123.45</div></body></html>"#;

            let _mock = server
                .mock(
                    "GET",
                    format!("/page/graph/51111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(html_without_title)
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result = collector
                .collect_by_graph_id(date, "51111", Unit::Kwh)
                .await;

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Element not found"));
        }

        #[tokio::test]
        async fn test_collect_by_graph_id_missing_val_kwh() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_query = make_daily_total_query(day_of_beginning(&date).unwrap());

            // HTML without #val_kwh element
            let html_without_val =
                r#"<html><body><div id="h_title">Test Title</div></body></html>"#;

            let _mock = server
                .mock(
                    "GET",
                    format!("/page/graph/51111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(html_without_val)
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result = collector
                .collect_by_graph_id(date, "51111", Unit::Kwh)
                .await;

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Element not found"));
        }

        #[tokio::test]
        async fn test_collect_by_graph_id_invalid_numeric_value() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_query = make_daily_total_query(day_of_beginning(&date).unwrap());

            // HTML with non-numeric value
            let _mock = server
                .mock(
                    "GET",
                    format!("/page/graph/51111?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_html_response("Title", "not-a-number"))
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result = collector
                .collect_by_graph_id(date, "51111", Unit::Kwh)
                .await;

            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Failed to parse"));
        }

        #[tokio::test]
        async fn test_collect_by_graph_id_http_error() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_query = make_daily_total_query(day_of_beginning(&date).unwrap());

            let _mock = server
                .mock(
                    "GET",
                    format!("/page/graph/51111?data={}", expected_query).as_str(),
                )
                .with_status(500)
                .with_body("Internal Server Error")
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result = collector
                .collect_by_graph_id(date, "51111", Unit::Kwh)
                .await;

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Request failed with status: 500"));
        }

        #[tokio::test]
        async fn test_collect_one_metric_fails() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_date = day_of_beginning(&date).unwrap();
            let expected_query = make_daily_total_query(expected_date);

            // First five metrics succeed
            let success_metrics = vec![
                ("51111", "発電", "100.0"),
                ("52111", "消費", "200.0"),
                ("53111", "買電", "50.0"),
                ("54111", "売電", "75.0"),
                ("55111", "給湯", "300.0"),
            ];

            for (graph_id, title, value) in &success_metrics {
                let _mock = server
                    .mock(
                        "GET",
                        format!("/page/graph/{}?data={}", graph_id, expected_query).as_str(),
                    )
                    .with_status(200)
                    .with_body(create_html_response(title, value))
                    .create_async()
                    .await;
            }

            // Last metric fails
            let _mock_fail = server
                .mock(
                    "GET",
                    format!("/page/graph/57111?data={}", expected_query).as_str(),
                )
                .with_status(404)
                .with_body("Not Found")
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result = collector.collect(date).await;

            // The collect method should fail if any metric fails
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_collect_all_metrics_fail() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_date = day_of_beginning(&date).unwrap();
            let expected_query = make_daily_total_query(expected_date);

            // All metrics return errors
            let graph_ids = vec!["51111", "52111", "53111", "54111", "55111", "57111"];

            for graph_id in graph_ids {
                let _mock = server
                    .mock(
                        "GET",
                        format!("/page/graph/{}?data={}", graph_id, expected_query).as_str(),
                    )
                    .with_status(503)
                    .with_body("Service Unavailable")
                    .create_async()
                    .await;
            }

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = DailyTotalMetricCollector::new(client);

            let result = collector.collect(date).await;

            assert!(result.is_err());
            match result {
                Err(e) => assert!(e.to_string().contains("Request failed with status: 503")),
                Ok(_) => panic!("Expected error but got success"),
            }
        }
    }
}
