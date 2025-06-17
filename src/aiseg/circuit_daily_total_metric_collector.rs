use crate::aiseg::client::Client;
use crate::aiseg::helper::day_of_beginning;
use crate::aiseg::html_parsing::extract_value;
use crate::aiseg::query_builder::make_circuit_query;
use crate::error::{AisegError, CollectorError, Result};
use crate::model::{DataPointBuilder, Measurement, MetricCollector, PowerTotalMetric, Unit};
use async_trait::async_trait;
use chrono::{DateTime, Local};
use scraper::Html;
use std::sync::Arc;

/// Collector for individual circuit daily total power consumption metrics.
///
/// This collector retrieves daily power consumption data for specific electrical
/// circuits in the home, such as air conditioners and electric vehicle chargers.
/// Unlike the main daily total collector, this focuses on individual circuit
/// consumption to provide detailed breakdowns of electricity usage.
///
/// # Circuits Monitored
/// - Circuit 30: EV (Electric Vehicle charger)
/// - Circuit 27: Living room air conditioner
/// - Circuit 26: Master bedroom air conditioner
/// - Circuit 25: Western room 2 air conditioner
pub struct CircuitDailyTotalMetricCollector {
    client: Arc<Client>,
}

impl CircuitDailyTotalMetricCollector {
    /// Creates a new instance of CircuitDailyTotalMetricCollector.
    ///
    /// # Arguments
    ///
    /// * `client` - Shared AiSEG2 client for making HTTP requests
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Collects daily total power consumption for a specific circuit.
    ///
    /// Retrieves the power consumption data for an individual circuit from
    /// the AiSEG2 system using graph ID 584 with circuit-specific parameters.
    ///
    /// # Arguments
    ///
    /// * `date` - The date to collect metrics for (normalized to beginning of day)
    /// * `name` - Human-readable name for the circuit (e.g., "EV", "リビングエアコン")
    /// * `circuit_id` - The AiSEG2 circuit ID (e.g., "30", "27")
    /// * `unit` - The unit of measurement (typically kWh for power consumption)
    ///
    /// # Returns
    ///
    /// A PowerTotalMetric containing the circuit's daily consumption data
    async fn collect_by_circuit_id(
        &self,
        date: DateTime<Local>,
        name: &str,
        circuit_id: &str,
        unit: Unit,
    ) -> Result<PowerTotalMetric, AisegError> {
        let the_day = day_of_beginning(&date).map_err(|e| AisegError::Parse(e))?;
        let response = self
            .client
            .get(&format!(
                "/page/graph/584?data={}",
                make_circuit_query(circuit_id, the_day)
            ))
            .await?;
        let document = Html::parse_document(&response);

        // Use the new extract_value utility
        let value: f64 = extract_value(&document, "#val_kwh")
            .map_err(|e| AisegError::Parse(e))?;

        Ok(PowerTotalMetric {
            measurement: Measurement::CircuitDailyTotal,
            name: format!("{}({})", name, unit),
            value,
            date: the_day,
        })
    }
}

#[async_trait]
impl MetricCollector for CircuitDailyTotalMetricCollector {
    /// Collects daily total metrics for all monitored circuits.
    ///
    /// Fetches power consumption data for four predefined circuits:
    /// 1. EV charger (circuit 30)
    /// 2. Living room air conditioner (circuit 27)
    /// 3. Master bedroom air conditioner (circuit 26)
    /// 4. Western room 2 air conditioner (circuit 25)
    ///
    /// All circuits use the same graph endpoint (584) but with different
    /// circuit IDs in the query parameters.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp for collection (normalized to beginning of day)
    ///
    /// # Returns
    ///
    /// A vector of DataPointBuilder instances for all circuits, or an error
    /// if any circuit data collection fails
    async fn collect(&self, timestamp: DateTime<Local>) -> Result<Vec<Box<dyn DataPointBuilder>>, CollectorError> {
        let metrics = vec![
            self.collect_by_circuit_id(timestamp, "EV", "30", Unit::Kwh)
                .await
                .map_err(CollectorError::Source)?,
            self.collect_by_circuit_id(timestamp, "リビングエアコン", "27", Unit::Kwh)
                .await
                .map_err(CollectorError::Source)?,
            self.collect_by_circuit_id(timestamp, "主寝室エアコン", "26", Unit::Kwh)
                .await
                .map_err(CollectorError::Source)?,
            self.collect_by_circuit_id(timestamp, "洋室２エアコン", "25", Unit::Kwh)
                .await
                .map_err(CollectorError::Source)?,
        ];
        
        Ok(metrics
            .into_iter()
            .map(|x| Box::new(x) as Box<dyn DataPointBuilder>)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aiseg::query_builder::make_circuit_query;
    use crate::test_utils::{config::test_aiseg2_config_with_url, html::create_value_only_html};
    use chrono::TimeZone;

    mod succeeds {
        use super::*;

        #[tokio::test]
        async fn test_collect_by_circuit_id_parses_valid_html() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local.with_ymd_and_hms(2024, 6, 8, 10, 0, 0).unwrap();
            let expected_query = make_circuit_query("30", day_of_beginning(&date).unwrap());

            let _mock = server
                .mock(
                    "GET",
                    format!("/page/graph/584?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_value_only_html("123.45"))
                .create_async()
                .await;

            let config = test_aiseg2_config_with_url(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);

            let result = collector
                .collect_by_circuit_id(date, "EV", "30", Unit::Kwh)
                .await;

            assert!(result.is_ok());
            let metric = result.unwrap();
            assert_eq!(metric.value, 123.45);
            assert_eq!(metric.name, "EV(kWh)");
            assert_eq!(metric.measurement, Measurement::CircuitDailyTotal);
        }

        #[tokio::test]
        async fn test_collect_by_circuit_id_returns_correct_metric() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local.with_ymd_and_hms(2024, 6, 8, 15, 30, 45).unwrap();
            let expected_date = day_of_beginning(&date).unwrap();
            let expected_query = make_circuit_query("27", expected_date);

            let _mock = server
                .mock(
                    "GET",
                    format!("/page/graph/584?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_value_only_html("456.78"))
                .create_async()
                .await;

            let config = test_aiseg2_config_with_url(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);

            let result = collector
                .collect_by_circuit_id(date, "リビングエアコン", "27", Unit::Kwh)
                .await;

            assert!(result.is_ok());
            let metric = result.unwrap();
            assert_eq!(metric.measurement, Measurement::CircuitDailyTotal);
            assert_eq!(metric.name, "リビングエアコン(kWh)");
            assert_eq!(metric.value, 456.78);
            assert_eq!(metric.date, expected_date);
        }

        #[tokio::test]
        async fn test_collect_by_circuit_id_handles_decimal_values() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let test_cases = vec![
                ("0.0", 0.0),
                ("1", 1.0),
                ("999.999", 999.999),
                ("1234.5678", 1234.5678),
            ];

            for (html_value, expected_value) in test_cases {
                let date = Local::now();
                let expected_query = make_circuit_query("25", day_of_beginning(&date).unwrap());

                let _mock = server
                    .mock(
                        "GET",
                        format!("/page/graph/584?data={}", expected_query).as_str(),
                    )
                    .with_status(200)
                    .with_body(create_value_only_html(html_value))
                    .create_async()
                    .await;

                let config = test_aiseg2_config_with_url(mock_url.clone());
                let client = Arc::new(Client::new(config));
                let collector = CircuitDailyTotalMetricCollector::new(client);

                let result = collector
                    .collect_by_circuit_id(date, "Test", "25", Unit::Kwh)
                    .await;

                assert!(result.is_ok());
                assert_eq!(result.unwrap().value, expected_value);
            }
        }

        #[tokio::test]
        async fn test_collect_returns_all_four_circuits() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_date = day_of_beginning(&date).unwrap();

            // Mock all four circuit responses
            let circuits = vec![
                ("30", "100.0"),
                ("27", "200.0"),
                ("26", "300.0"),
                ("25", "400.0"),
            ];

            for (circuit_id, value) in &circuits {
                let expected_query = make_circuit_query(circuit_id, expected_date);
                let _mock = server
                    .mock(
                        "GET",
                        format!("/page/graph/584?data={}", expected_query).as_str(),
                    )
                    .with_status(200)
                    .with_body(create_value_only_html(value))
                    .create_async()
                    .await;
            }

            let config = test_aiseg2_config_with_url(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);

            let result = collector.collect(date).await;

            assert!(result.is_ok());
            let data_points = result.unwrap();
            assert_eq!(data_points.len(), 4);

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

            // Mock responses with different values
            let _mock1 = server
                .mock(
                    "GET",
                    format!(
                        "/page/graph/584?data={}",
                        make_circuit_query("30", expected_date)
                    )
                    .as_str(),
                )
                .with_status(200)
                .with_body(create_value_only_html("0.0"))
                .create_async()
                .await;

            let _mock2 = server
                .mock(
                    "GET",
                    format!(
                        "/page/graph/584?data={}",
                        make_circuit_query("27", expected_date)
                    )
                    .as_str(),
                )
                .with_status(200)
                .with_body(create_value_only_html("999.99"))
                .create_async()
                .await;

            let _mock3 = server
                .mock(
                    "GET",
                    format!(
                        "/page/graph/584?data={}",
                        make_circuit_query("26", expected_date)
                    )
                    .as_str(),
                )
                .with_status(200)
                .with_body(create_value_only_html("50.5"))
                .create_async()
                .await;

            let _mock4 = server
                .mock(
                    "GET",
                    format!(
                        "/page/graph/584?data={}",
                        make_circuit_query("25", expected_date)
                    )
                    .as_str(),
                )
                .with_status(200)
                .with_body(create_value_only_html("1.23"))
                .create_async()
                .await;

            let config = test_aiseg2_config_with_url(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);

            let result = collector.collect(date).await;

            assert!(result.is_ok());
            let data_points = result.unwrap();
            assert_eq!(data_points.len(), 4);
        }
    }

    mod fails {
        use super::*;

        #[tokio::test]
        async fn test_collect_by_circuit_id_missing_val_kwh() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_query = make_circuit_query("30", day_of_beginning(&date).unwrap());

            // HTML without #val_kwh element
            let html_without_val_kwh = r#"<html><body><div>No value here</div></body></html>"#;

            let _mock = server
                .mock(
                    "GET",
                    format!("/page/graph/584?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(html_without_val_kwh)
                .create_async()
                .await;

            let config = test_aiseg2_config_with_url(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);

            let result = collector
                .collect_by_circuit_id(date, "EV", "30", Unit::Kwh)
                .await;

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("HTML parsing error"));
        }

        #[tokio::test]
        async fn test_collect_by_circuit_id_invalid_numeric_value() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_query = make_circuit_query("30", day_of_beginning(&date).unwrap());

            // HTML with non-numeric value
            let _mock = server
                .mock(
                    "GET",
                    format!("/page/graph/584?data={}", expected_query).as_str(),
                )
                .with_status(200)
                .with_body(create_value_only_html("not-a-number"))
                .create_async()
                .await;

            let config = test_aiseg2_config_with_url(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);

            let result = collector
                .collect_by_circuit_id(date, "EV", "30", Unit::Kwh)
                .await;

            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("HTML parsing error"));
        }

        #[tokio::test]
        async fn test_collect_by_circuit_id_http_error() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_query = make_circuit_query("30", day_of_beginning(&date).unwrap());

            let _mock = server
                .mock(
                    "GET",
                    format!("/page/graph/584?data={}", expected_query).as_str(),
                )
                .with_status(500)
                .with_body("Internal Server Error")
                .create_async()
                .await;

            let config = test_aiseg2_config_with_url(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);

            let result = collector
                .collect_by_circuit_id(date, "EV", "30", Unit::Kwh)
                .await;

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("server error (status 500)"));
        }

        #[tokio::test]
        async fn test_collect_one_circuit_fails() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_date = day_of_beginning(&date).unwrap();

            // First three circuits succeed
            let _mock1 = server
                .mock(
                    "GET",
                    format!(
                        "/page/graph/584?data={}",
                        make_circuit_query("30", expected_date)
                    )
                    .as_str(),
                )
                .with_status(200)
                .with_body(create_value_only_html("100.0"))
                .create_async()
                .await;

            let _mock2 = server
                .mock(
                    "GET",
                    format!(
                        "/page/graph/584?data={}",
                        make_circuit_query("27", expected_date)
                    )
                    .as_str(),
                )
                .with_status(200)
                .with_body(create_value_only_html("200.0"))
                .create_async()
                .await;

            let _mock3 = server
                .mock(
                    "GET",
                    format!(
                        "/page/graph/584?data={}",
                        make_circuit_query("26", expected_date)
                    )
                    .as_str(),
                )
                .with_status(200)
                .with_body(create_value_only_html("300.0"))
                .create_async()
                .await;

            // Fourth circuit fails
            let _mock4 = server
                .mock(
                    "GET",
                    format!(
                        "/page/graph/584?data={}",
                        make_circuit_query("25", expected_date)
                    )
                    .as_str(),
                )
                .with_status(404)
                .with_body("Not Found")
                .create_async()
                .await;

            let config = test_aiseg2_config_with_url(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);

            let result = collector.collect(date).await;

            // The collect method should fail if any circuit fails
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_collect_all_circuits_fail() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let date = Local::now();
            let expected_date = day_of_beginning(&date).unwrap();

            // All circuits return errors
            let circuits = vec!["30", "27", "26", "25"];

            for circuit_id in circuits {
                let expected_query = make_circuit_query(circuit_id, expected_date);
                let _mock = server
                    .mock(
                        "GET",
                        format!("/page/graph/584?data={}", expected_query).as_str(),
                    )
                    .with_status(503)
                    .with_body("Service Unavailable")
                    .create_async()
                    .await;
            }

            let config = test_aiseg2_config_with_url(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);

            let result = collector.collect(date).await;

            assert!(result.is_err());
            match result {
                Err(e) => assert!(e.to_string().contains("failed to collect from source")),
                Ok(_) => panic!("Expected error but got success"),
            }
        }
    }
}
