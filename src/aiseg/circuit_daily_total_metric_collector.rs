use crate::aiseg::client::Client;
use crate::aiseg::helper::{day_of_beginning, parse_f64_from_html};
use crate::model::{DataPointBuilder, Measurement, MetricCollector, PowerTotalMetric, Unit};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Datelike, Local};
use scraper::Html;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct CircuitDailyTotalMetricCollector {
    client: Arc<Client>,
}

impl CircuitDailyTotalMetricCollector {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    async fn collect_by_circuit_id(
        &self,
        date: DateTime<Local>,
        name: &str,
        circuit_id: &str,
        unit: Unit,
    ) -> Result<PowerTotalMetric> {
        let the_day = day_of_beginning(&date);
        let response = self
            .client
            .get(&format!(
                "/page/graph/584?data={}",
                make_query(circuit_id, the_day)
            ))
            .await?;
        let document = Html::parse_document(&response);
        let value = parse_f64_from_html(&document, "#val_kwh")?;
        Ok(PowerTotalMetric {
            measurement: Measurement::CircuitDailyTotal,
            name: format!("{}({})", name, unit),
            value,
            date: the_day,
        })
    }
}

impl MetricCollector for CircuitDailyTotalMetricCollector {
    fn collect<'a>(
        &'a self,
        timestamp: DateTime<Local>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn DataPointBuilder>>>> + 'a + Send>> {
        Box::pin(async move {
            Ok(vec![
                self.collect_by_circuit_id(timestamp, "EV", "30", Unit::Kwh)
                    .await?,
                self.collect_by_circuit_id(timestamp, "リビングエアコン", "27", Unit::Kwh)
                    .await?,
                self.collect_by_circuit_id(timestamp, "主寝室エアコン", "26", Unit::Kwh)
                    .await?,
                self.collect_by_circuit_id(timestamp, "洋室２エアコン", "25", Unit::Kwh)
                    .await?,
            ]
            .into_iter()
            .map(|x| Box::new(x) as Box<dyn DataPointBuilder>)
            .collect())
        })
    }
}

// makeDataQuery is base64 encoded JSON string
// ex: {"day":[2024,6,8],"term":"2024/06/08","termStr":"day","id":"1","circuitid":"30"}
fn make_query(circuit_id: &str, date: DateTime<Local>) -> String {
    let query = format!(
        r#"{{"day":[{}, {}, {}],"term":"{}","termStr":"day","id":"1","circuitid":"{}"}}"#,
        date.year(),
        date.month(),
        date.day(),
        date.format("%Y/%m/%d"),
        circuit_id,
    );
    STANDARD.encode(query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use chrono::TimeZone;
    use mockito;

    fn test_config(url: String) -> config::Aiseg2Config {
        config::Aiseg2Config {
            url,
            user: "test_user".to_string(),
            password: "test_password".to_string(),
        }
    }

    fn create_html_response(value: &str) -> String {
        format!(
            r#"<html><body><div id="val_kwh">{}</div></body></html>"#,
            value
        )
    }

    #[test]
    fn test_make_query() {
        let date = Local.with_ymd_and_hms(2024, 6, 8, 10, 30, 0).unwrap();
        let query = make_query("30", date);
        
        // Decode the base64 to verify the JSON content
        let decoded = String::from_utf8(STANDARD.decode(&query).unwrap()).unwrap();
        let expected = r#"{"day":[2024, 6, 8],"term":"2024/06/08","termStr":"day","id":"1","circuitid":"30"}"#;
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_make_query_handles_different_dates() {
        // Test leap year date
        let date1 = Local.with_ymd_and_hms(2024, 2, 29, 0, 0, 0).unwrap();
        let query1 = make_query("25", date1);
        let decoded1 = String::from_utf8(STANDARD.decode(&query1).unwrap()).unwrap();
        assert!(decoded1.contains(r#""day":[2024, 2, 29]"#));
        assert!(decoded1.contains(r#""term":"2024/02/29""#));

        // Test end of year
        let date2 = Local.with_ymd_and_hms(2023, 12, 31, 23, 59, 59).unwrap();
        let query2 = make_query("27", date2);
        let decoded2 = String::from_utf8(STANDARD.decode(&query2).unwrap()).unwrap();
        assert!(decoded2.contains(r#""day":[2023, 12, 31]"#));
        assert!(decoded2.contains(r#""term":"2023/12/31""#));
    }

    mod succeeds {
        use super::*;

        #[tokio::test]
        async fn test_collect_by_circuit_id_parses_valid_html() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();
            
            let date = Local.with_ymd_and_hms(2024, 6, 8, 10, 0, 0).unwrap();
            let expected_query = make_query("30", day_of_beginning(&date));
            
            let _mock = server
                .mock("GET", format!("/page/graph/584?data={}", expected_query).as_str())
                .with_status(200)
                .with_body(create_html_response("123.45"))
                .create_async()
                .await;

            let config = test_config(mock_url);
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
            let expected_date = day_of_beginning(&date);
            let expected_query = make_query("27", expected_date);
            
            let _mock = server
                .mock("GET", format!("/page/graph/584?data={}", expected_query).as_str())
                .with_status(200)
                .with_body(create_html_response("456.78"))
                .create_async()
                .await;

            let config = test_config(mock_url);
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
                let expected_query = make_query("25", day_of_beginning(&date));
                
                let _mock = server
                    .mock("GET", format!("/page/graph/584?data={}", expected_query).as_str())
                    .with_status(200)
                    .with_body(create_html_response(html_value))
                    .create_async()
                    .await;

                let config = test_config(mock_url.clone());
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
            let expected_date = day_of_beginning(&date);
            
            // Mock all four circuit responses
            let circuits = vec![
                ("30", "100.0"),
                ("27", "200.0"),
                ("26", "300.0"),
                ("25", "400.0"),
            ];
            
            for (circuit_id, value) in &circuits {
                let expected_query = make_query(circuit_id, expected_date);
                let _mock = server
                    .mock("GET", format!("/page/graph/584?data={}", expected_query).as_str())
                    .with_status(200)
                    .with_body(create_html_response(value))
                    .create_async()
                    .await;
            }

            let config = test_config(mock_url);
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
            let expected_date = day_of_beginning(&date);
            
            // Mock responses with different values
            let _mock1 = server
                .mock("GET", format!("/page/graph/584?data={}", make_query("30", expected_date)).as_str())
                .with_status(200)
                .with_body(create_html_response("0.0"))
                .create_async()
                .await;
            
            let _mock2 = server
                .mock("GET", format!("/page/graph/584?data={}", make_query("27", expected_date)).as_str())
                .with_status(200)
                .with_body(create_html_response("999.99"))
                .create_async()
                .await;
            
            let _mock3 = server
                .mock("GET", format!("/page/graph/584?data={}", make_query("26", expected_date)).as_str())
                .with_status(200)
                .with_body(create_html_response("50.5"))
                .create_async()
                .await;
            
            let _mock4 = server
                .mock("GET", format!("/page/graph/584?data={}", make_query("25", expected_date)).as_str())
                .with_status(200)
                .with_body(create_html_response("1.23"))
                .create_async()
                .await;

            let config = test_config(mock_url);
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
            let expected_query = make_query("30", day_of_beginning(&date));
            
            // HTML without #val_kwh element
            let html_without_val_kwh = r#"<html><body><div>No value here</div></body></html>"#;
            
            let _mock = server
                .mock("GET", format!("/page/graph/584?data={}", expected_query).as_str())
                .with_status(200)
                .with_body(html_without_val_kwh)
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);
            
            let result = collector
                .collect_by_circuit_id(date, "EV", "30", Unit::Kwh)
                .await;
            
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Failed to find value"));
        }

        #[tokio::test]
        async fn test_collect_by_circuit_id_invalid_numeric_value() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();
            
            let date = Local::now();
            let expected_query = make_query("30", day_of_beginning(&date));
            
            // HTML with non-numeric value
            let _mock = server
                .mock("GET", format!("/page/graph/584?data={}", expected_query).as_str())
                .with_status(200)
                .with_body(create_html_response("not-a-number"))
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);
            
            let result = collector
                .collect_by_circuit_id(date, "EV", "30", Unit::Kwh)
                .await;
            
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Failed to parse value"));
        }

        #[tokio::test]
        async fn test_collect_by_circuit_id_http_error() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();
            
            let date = Local::now();
            let expected_query = make_query("30", day_of_beginning(&date));
            
            let _mock = server
                .mock("GET", format!("/page/graph/584?data={}", expected_query).as_str())
                .with_status(500)
                .with_body("Internal Server Error")
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);
            
            let result = collector
                .collect_by_circuit_id(date, "EV", "30", Unit::Kwh)
                .await;
            
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Request failed with status: 500"));
        }

        #[tokio::test]
        async fn test_collect_one_circuit_fails() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();
            
            let date = Local::now();
            let expected_date = day_of_beginning(&date);
            
            // First three circuits succeed
            let _mock1 = server
                .mock("GET", format!("/page/graph/584?data={}", make_query("30", expected_date)).as_str())
                .with_status(200)
                .with_body(create_html_response("100.0"))
                .create_async()
                .await;
            
            let _mock2 = server
                .mock("GET", format!("/page/graph/584?data={}", make_query("27", expected_date)).as_str())
                .with_status(200)
                .with_body(create_html_response("200.0"))
                .create_async()
                .await;
            
            let _mock3 = server
                .mock("GET", format!("/page/graph/584?data={}", make_query("26", expected_date)).as_str())
                .with_status(200)
                .with_body(create_html_response("300.0"))
                .create_async()
                .await;
            
            // Fourth circuit fails
            let _mock4 = server
                .mock("GET", format!("/page/graph/584?data={}", make_query("25", expected_date)).as_str())
                .with_status(404)
                .with_body("Not Found")
                .create_async()
                .await;

            let config = test_config(mock_url);
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
            let expected_date = day_of_beginning(&date);
            
            // All circuits return errors
            let circuits = vec!["30", "27", "26", "25"];
            
            for circuit_id in circuits {
                let expected_query = make_query(circuit_id, expected_date);
                let _mock = server
                    .mock("GET", format!("/page/graph/584?data={}", expected_query).as_str())
                    .with_status(503)
                    .with_body("Service Unavailable")
                    .create_async()
                    .await;
            }

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = CircuitDailyTotalMetricCollector::new(client);
            
            let result = collector.collect(date).await;
            
            assert!(result.is_err());
            match result {
                Err(e) => assert!(e.to_string().contains("Request failed with status: 503")),
                Ok(_) => panic!("Expected error but got success"),
            }
        }
    }
}
