use crate::aiseg::client::Client;
use crate::aiseg::helper::{day_of_beginning, parse_f64_from_html, parse_text_from_html};
use crate::model::{DataPointBuilder, Measurement, MetricCollector, PowerTotalMetric, Unit};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Datelike, Local};
use scraper::Html;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct DailyTotalMetricCollector {
    client: Arc<Client>,
}

impl DailyTotalMetricCollector {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    async fn collect_by_graph_id(
        &self,
        date: DateTime<Local>,
        graph_id: &str,
        unit: Unit,
    ) -> Result<PowerTotalMetric> {
        let the_day = day_of_beginning(&date);
        let response = self
            .client
            .get(&format!(
                "/page/graph/{}?data={}",
                graph_id,
                make_query(the_day)
            ))
            .await?;
        let document = Html::parse_document(&response);
        let name = parse_text_from_html(&document, "#h_title")?;
        let value = parse_f64_from_html(&document, "#val_kwh")?;

        Ok(PowerTotalMetric {
            measurement: Measurement::DailyTotal,
            name: format!("{}({})", name, unit),
            value,
            date: the_day,
        })
    }
}

impl MetricCollector for DailyTotalMetricCollector {
    fn collect<'a>(
        &'a self,
        timestamp: DateTime<Local>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn DataPointBuilder>>>> + Send + 'a>> {
        Box::pin(async move {
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
        })
    }
}

// makeDataQuery is base64 encoded JSON string
// ex: {"day":[2024,6,6],"month_compare":"mon","day_compare":"day"}
fn make_query(date: DateTime<Local>) -> String {
    let query = format!(
        r#"{{"day":[{}, {}, {}],"month_compare":"mon","day_compare":"day"}}"#,
        date.year(),
        date.month(),
        date.day(),
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

    fn create_html_response(title: &str, value: &str) -> String {
        format!(
            r#"<html><body>
                <div id="h_title">{}</div>
                <div id="val_kwh">{}</div>
            </body></html>"#,
            title, value
        )
    }

    #[test]
    fn test_make_query() {
        let date = Local.with_ymd_and_hms(2024, 6, 6, 10, 30, 0).unwrap();
        let query = make_query(date);
        
        // Decode the base64 to verify the JSON content
        let decoded = String::from_utf8(STANDARD.decode(&query).unwrap()).unwrap();
        let expected = r#"{"day":[2024, 6, 6],"month_compare":"mon","day_compare":"day"}"#;
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_make_query_handles_different_dates() {
        // Test leap year date
        let date1 = Local.with_ymd_and_hms(2024, 2, 29, 0, 0, 0).unwrap();
        let query1 = make_query(date1);
        let decoded1 = String::from_utf8(STANDARD.decode(&query1).unwrap()).unwrap();
        assert!(decoded1.contains(r#""day":[2024, 2, 29]"#));
        assert!(decoded1.contains(r#""month_compare":"mon"#));
        assert!(decoded1.contains(r#""day_compare":"day"#));

        // Test beginning of year
        let date2 = Local.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let query2 = make_query(date2);
        let decoded2 = String::from_utf8(STANDARD.decode(&query2).unwrap()).unwrap();
        assert!(decoded2.contains(r#""day":[2024, 1, 1]"#));

        // Test end of year
        let date3 = Local.with_ymd_and_hms(2023, 12, 31, 23, 59, 59).unwrap();
        let query3 = make_query(date3);
        let decoded3 = String::from_utf8(STANDARD.decode(&query3).unwrap()).unwrap();
        assert!(decoded3.contains(r#""day":[2023, 12, 31]"#));
    }

    mod succeeds {
        use super::*;

        #[tokio::test]
        async fn test_collect_by_graph_id_parses_valid_html() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();
            
            let date = Local.with_ymd_and_hms(2024, 6, 6, 10, 0, 0).unwrap();
            let expected_query = make_query(day_of_beginning(&date));
            
            let _mock = server
                .mock("GET", format!("/page/graph/51111?data={}", expected_query).as_str())
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
            assert_eq!(metric.date, day_of_beginning(&date));
        }

        #[tokio::test]
        async fn test_collect_by_graph_id_returns_correct_metric() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();
            
            let date = Local.with_ymd_and_hms(2024, 6, 6, 15, 30, 45).unwrap();
            let expected_date = day_of_beginning(&date);
            let expected_query = make_query(expected_date);
            
            let _mock = server
                .mock("GET", format!("/page/graph/52111?data={}", expected_query).as_str())
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
            let expected_query = make_query(day_of_beginning(&date));
            
            // Test kWh unit
            let _mock1 = server
                .mock("GET", format!("/page/graph/51111?data={}", expected_query).as_str())
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
                .mock("GET", format!("/page/graph/55111?data={}", expected_query).as_str())
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
                .mock("GET", format!("/page/graph/57111?data={}", expected_query).as_str())
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
            let expected_date = day_of_beginning(&date);
            let expected_query = make_query(expected_date);
            
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
                    .mock("GET", format!("/page/graph/{}?data={}", graph_id, expected_query).as_str())
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
            let expected_date = day_of_beginning(&date);
            let expected_query = make_query(expected_date);
            
            // Mock responses with different values including edge cases
            let _mock1 = server
                .mock("GET", format!("/page/graph/51111?data={}", expected_query).as_str())
                .with_status(200)
                .with_body(create_html_response("発電", "0.0"))
                .create_async()
                .await;
            
            let _mock2 = server
                .mock("GET", format!("/page/graph/52111?data={}", expected_query).as_str())
                .with_status(200)
                .with_body(create_html_response("消費", "999.999"))
                .create_async()
                .await;
            
            let _mock3 = server
                .mock("GET", format!("/page/graph/53111?data={}", expected_query).as_str())
                .with_status(200)
                .with_body(create_html_response("買電", "1"))
                .create_async()
                .await;
            
            let _mock4 = server
                .mock("GET", format!("/page/graph/54111?data={}", expected_query).as_str())
                .with_status(200)
                .with_body(create_html_response("売電", "0.001"))
                .create_async()
                .await;
            
            let _mock5 = server
                .mock("GET", format!("/page/graph/55111?data={}", expected_query).as_str())
                .with_status(200)
                .with_body(create_html_response("給湯", "1234.5678"))
                .create_async()
                .await;
            
            let _mock6 = server
                .mock("GET", format!("/page/graph/57111?data={}", expected_query).as_str())
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
            let expected_query = make_query(day_of_beginning(&date));
            
            // HTML without #h_title element
            let html_without_title = r#"<html><body><div id="val_kwh">123.45</div></body></html>"#;
            
            let _mock = server
                .mock("GET", format!("/page/graph/51111?data={}", expected_query).as_str())
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
            assert!(result.unwrap_err().to_string().contains("Failed to find value"));
        }

        #[tokio::test]
        async fn test_collect_by_graph_id_missing_val_kwh() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();
            
            let date = Local::now();
            let expected_query = make_query(day_of_beginning(&date));
            
            // HTML without #val_kwh element
            let html_without_val = r#"<html><body><div id="h_title">Test Title</div></body></html>"#;
            
            let _mock = server
                .mock("GET", format!("/page/graph/51111?data={}", expected_query).as_str())
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
            assert!(result.unwrap_err().to_string().contains("Failed to find value"));
        }

        #[tokio::test]
        async fn test_collect_by_graph_id_invalid_numeric_value() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();
            
            let date = Local::now();
            let expected_query = make_query(day_of_beginning(&date));
            
            // HTML with non-numeric value
            let _mock = server
                .mock("GET", format!("/page/graph/51111?data={}", expected_query).as_str())
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
            assert!(result.unwrap_err().to_string().contains("Failed to parse value"));
        }

        #[tokio::test]
        async fn test_collect_by_graph_id_http_error() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();
            
            let date = Local::now();
            let expected_query = make_query(day_of_beginning(&date));
            
            let _mock = server
                .mock("GET", format!("/page/graph/51111?data={}", expected_query).as_str())
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
            assert!(result.unwrap_err().to_string().contains("Request failed with status: 500"));
        }

        #[tokio::test]
        async fn test_collect_one_metric_fails() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();
            
            let date = Local::now();
            let expected_date = day_of_beginning(&date);
            let expected_query = make_query(expected_date);
            
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
                    .mock("GET", format!("/page/graph/{}?data={}", graph_id, expected_query).as_str())
                    .with_status(200)
                    .with_body(create_html_response(title, value))
                    .create_async()
                    .await;
            }
            
            // Last metric fails
            let _mock_fail = server
                .mock("GET", format!("/page/graph/57111?data={}", expected_query).as_str())
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
            let expected_date = day_of_beginning(&date);
            let expected_query = make_query(expected_date);
            
            // All metrics return errors
            let graph_ids = vec!["51111", "52111", "53111", "54111", "55111", "57111"];
            
            for graph_id in graph_ids {
                let _mock = server
                    .mock("GET", format!("/page/graph/{}?data={}", graph_id, expected_query).as_str())
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
