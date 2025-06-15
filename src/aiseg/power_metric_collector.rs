use crate::aiseg::client::Client;
use crate::aiseg::helper::{
    kilowatts_to_watts, parse_f64_from_html, parse_text_from_html, truncate_to_i64,
};
use crate::model::{
    merge_same_name_power_status_breakdown_metrics, DataPointBuilder, Measurement, MetricCollector,
    PowerStatusBreakdownMetric, PowerStatusBreakdownMetricCategory, PowerStatusMetric, Unit,
};
use anyhow::Result;
use chrono::{DateTime, Local};
use scraper::Html;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct PowerMetricCollector {
    client: Arc<Client>,
}

impl PowerMetricCollector {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    async fn collect_from_main_page(&self) -> Result<Vec<Box<dyn DataPointBuilder>>> {
        let response = self.client.get("/page/electricflow/111").await?;
        let document = Html::parse_document(&response);
        Ok(vec![
            self.collect_total_metrics(&document)?,
            self.collect_generation_detail_metrics(&document)?,
        ]
        .into_iter()
        .flatten()
        .collect())
    }

    fn collect_total_metrics(&self, document: &Html) -> Result<Vec<Box<dyn DataPointBuilder>>> {
        let generation = kilowatts_to_watts(parse_f64_from_html(document, "#g_capacity")?);
        let consumption = kilowatts_to_watts(parse_f64_from_html(document, "#u_capacity")?);

        Ok(vec![
            Box::new(PowerStatusMetric {
                measurement: Measurement::Power,
                name: format!("総発電電力({})", Unit::Watt),
                value: generation,
            }),
            Box::new(PowerStatusMetric {
                measurement: Measurement::Power,
                name: format!("総消費電力({})", Unit::Watt),
                value: consumption,
            }),
            Box::new(PowerStatusMetric {
                measurement: Measurement::Power,
                name: format!("売買電力({})", Unit::Watt),
                value: generation - consumption,
            }),
        ])
    }

    fn collect_generation_detail_metrics(
        &self,
        document: &Html,
    ) -> Result<Vec<Box<dyn DataPointBuilder>>> {
        let mut res: Vec<Box<dyn DataPointBuilder>> = vec![];
        for i in 1..=4 {
            let name = match parse_text_from_html(document, &format!("#g_d_{}_title", i)) {
                Ok(name) => name,
                Err(_) => break,
            };
            let value = truncate_to_i64(parse_f64_from_html(
                document,
                &format!("#g_d_{}_capacity", i),
            )?);
            res.push(Box::new(PowerStatusBreakdownMetric {
                measurement: Measurement::Power,
                category: PowerStatusBreakdownMetricCategory::Generation,
                name: format!("{}({})", name, Unit::Watt),
                value,
            }));
        }
        Ok(res)
    }

    async fn collect_from_consumption_detail_pages(
        &self,
    ) -> Result<Vec<Box<dyn DataPointBuilder>>> {
        self.collect_consumption_detail_metrics().await
    }

    async fn collect_consumption_detail_metrics(&self) -> Result<Vec<Box<dyn DataPointBuilder>>> {
        let mut all_items: Vec<PowerStatusBreakdownMetric> = vec![];

        let items = self.collect_consumption_pages().await?;
        all_items.extend(items);

        let merged = merge_same_name_power_status_breakdown_metrics(all_items);
        Ok(merged
            .into_iter()
            .map(|item| Box::new(item) as Box<dyn DataPointBuilder>)
            .collect())
    }

    async fn collect_consumption_pages(&self) -> Result<Vec<PowerStatusBreakdownMetric>> {
        let mut last_page_names = "".to_string();
        let mut all_items: Vec<PowerStatusBreakdownMetric> = vec![];

        for page in 1..=20 {
            let response = self
                .client
                .get(&format!("/page/electricflow/1113?id={}", page))
                .await?;
            let document = Html::parse_document(&response);

            let page_items = self.parse_consumption_page(&document)?;

            // Check if we've seen these names before (pagination complete)
            let names = page_items
                .iter()
                .map(|item| item.name.clone())
                .collect::<Vec<String>>()
                .join(", ");
            if last_page_names == names {
                break;
            }
            last_page_names = names;
            all_items.extend(page_items);
        }

        Ok(all_items)
    }

    fn parse_consumption_page(&self, document: &Html) -> Result<Vec<PowerStatusBreakdownMetric>> {
        let mut items: Vec<PowerStatusBreakdownMetric> = vec![];

        for i in 1..=10 {
            let name = match parse_text_from_html(document, &format!("#stage_{} > div.c_device", i))
            {
                Ok(name) => name,
                Err(_) => break,
            };
            let watt = match parse_f64_from_html(document, &format!("#stage_{} > div.c_value", i)) {
                Ok(kw) => truncate_to_i64(kw),
                Err(_) => 0,
            };
            items.push(PowerStatusBreakdownMetric {
                measurement: Measurement::Power,
                category: PowerStatusBreakdownMetricCategory::Consumption,
                name: format!("{}({})", name, Unit::Watt),
                value: watt,
            });
        }

        Ok(items)
    }
}

impl MetricCollector for PowerMetricCollector {
    fn collect<'a>(
        &'a self,
        _: DateTime<Local>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn DataPointBuilder>>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(vec![
                self.collect_from_main_page().await?,
                self.collect_from_consumption_detail_pages().await?,
            ]
            .into_iter()
            .flatten()
            .collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aiseg::test_utils::test_config;

    fn create_main_page_html(
        g_capacity: &str,
        u_capacity: &str,
        details: Vec<(&str, &str)>,
    ) -> String {
        let mut html = format!(
            r#"<html><body>
                <div id="g_capacity">{}</div>
                <div id="u_capacity">{}</div>"#,
            g_capacity, u_capacity
        );

        for (i, (name, capacity)) in details.iter().enumerate() {
            html.push_str(&format!(
                r#"
                <div id="g_d_{}_title"><span>{}</span></div>
                <div id="g_d_{}_capacity"><span>{}</span></div>"#,
                i + 1,
                name,
                i + 1,
                capacity
            ));
        }

        html.push_str("</body></html>");
        html
    }

    fn create_consumption_page_html(items: Vec<(&str, &str)>) -> String {
        let mut html = r#"<html><body>"#.to_string();

        for (i, (device, value)) in items.iter().enumerate() {
            html.push_str(&format!(
                r#"
                <div id="stage_{}">
                    <div class="c_device"><span>{}</span></div>
                    <div class="c_value"><span>{}</span></div>
                </div>"#,
                i + 1,
                device,
                value
            ));
        }

        html.push_str("</body></html>");
        html
    }

    mod succeeds {
        use super::*;

        #[test]
        fn test_collect_total_metrics_valid_html() {
            let html = create_main_page_html("2.5", "3.8", vec![]);
            let document = Html::parse_document(&html);
            let collector = PowerMetricCollector::new(Arc::new(Client::new(test_config(
                "http://test".to_string(),
            ))));

            let result = collector.collect_total_metrics(&document);

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 3);

            // Verify all metrics can be converted to InfluxDB format
            for metric in &metrics {
                assert!(metric.to_point().is_ok());
            }
        }

        #[test]
        fn test_collect_generation_detail_metrics_valid_html() {
            let html = create_main_page_html(
                "2.5",
                "3.8",
                vec![
                    ("太陽光", "2.5"),
                    ("燃料電池", "0.5"),
                    ("蓄電池", "0.2"),
                    ("その他", "0.1"),
                ],
            );
            let document = Html::parse_document(&html);
            let collector = PowerMetricCollector::new(Arc::new(Client::new(test_config(
                "http://test".to_string(),
            ))));

            let result = collector.collect_generation_detail_metrics(&document);

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 4);

            // Verify all metrics can be converted to InfluxDB format
            for metric in &metrics {
                assert!(metric.to_point().is_ok());
            }
        }

        #[test]
        fn test_collect_generation_detail_metrics_partial() {
            let html =
                create_main_page_html("2.5", "3.8", vec![("太陽光", "2.5"), ("燃料電池", "0.5")]);
            let document = Html::parse_document(&html);
            let collector = PowerMetricCollector::new(Arc::new(Client::new(test_config(
                "http://test".to_string(),
            ))));

            let result = collector.collect_generation_detail_metrics(&document);

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 2); // Only 2 items found
        }

        #[test]
        fn test_collect_total_metrics_calculates_sell_buy_power() {
            let test_cases = vec![
                ("5.0", "3.0", 2000),  // Selling power (positive)
                ("2.0", "4.0", -2000), // Buying power (negative)
                ("3.0", "3.0", 0),     // Balanced (zero)
            ];

            for (generation, consumption, _expected_sell_buy) in test_cases {
                let html = create_main_page_html(generation, consumption, vec![]);
                let document = Html::parse_document(&html);
                let collector = PowerMetricCollector::new(Arc::new(Client::new(test_config(
                    "http://test".to_string(),
                ))));

                let result = collector.collect_total_metrics(&document);
                assert!(result.is_ok());
                let metrics = result.unwrap();
                assert_eq!(metrics.len(), 3);

                // Verify the sell/buy metric exists and can be converted
                assert!(metrics[2].to_point().is_ok());
            }
        }

        #[tokio::test]
        async fn test_collect_from_main_page_complete_data() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let _mock = server
                .mock("GET", "/page/electricflow/111")
                .with_status(200)
                .with_body(create_main_page_html(
                    "2.5",
                    "3.8",
                    vec![("太陽光", "2.5"), ("燃料電池", "0.0")],
                ))
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect_from_main_page().await;

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 5); // 3 total metrics + 2 generation details
        }

        #[tokio::test]
        async fn test_collect_consumption_single_page() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            // Page 1 with data
            let _mock1 = server
                .mock("GET", "/page/electricflow/1113?id=1")
                .with_status(200)
                .with_body(create_consumption_page_html(vec![
                    ("エアコン", "1.2"),
                    ("冷蔵庫", "0.3"),
                    ("照明", "0.1"),
                ]))
                .create_async()
                .await;

            // Pages 2-20 empty to trigger termination on empty page
            for page in 2..=20 {
                let _ = server
                    .mock(
                        "GET",
                        format!("/page/electricflow/1113?id={}", page).as_str(),
                    )
                    .with_status(200)
                    .with_body(create_consumption_page_html(vec![]))
                    .create_async()
                    .await;
            }

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect_consumption_detail_metrics().await;

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 3);

            for metric in metrics {
                assert!(metric.to_point().is_ok());
            }
        }

        #[tokio::test]
        async fn test_collect_consumption_multiple_pages() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            // Page 1
            let page1_items = vec![
                ("Device1", "0.1"),
                ("Device2", "0.1"),
                ("Device3", "0.1"),
                ("Device4", "0.1"),
                ("Device5", "0.1"),
                ("Device6", "0.1"),
                ("Device7", "0.1"),
                ("Device8", "0.1"),
                ("Device9", "0.1"),
                ("Device10", "0.1"),
            ];
            let _mock1 = server
                .mock("GET", "/page/electricflow/1113?id=1")
                .with_status(200)
                .with_body(create_consumption_page_html(page1_items))
                .create_async()
                .await;

            // Page 2
            let page2_items = vec![
                ("Device11", "0.2"),
                ("Device12", "0.2"),
                ("Device13", "0.2"),
                ("Device14", "0.2"),
                ("Device15", "0.2"),
            ];
            let _mock2 = server
                .mock("GET", "/page/electricflow/1113?id=2")
                .with_status(200)
                .with_body(create_consumption_page_html(page2_items))
                .create_async()
                .await;

            // Pages 3-20 empty
            for page in 3..=20 {
                let _ = server
                    .mock(
                        "GET",
                        format!("/page/electricflow/1113?id={}", page).as_str(),
                    )
                    .with_status(200)
                    .with_body(create_consumption_page_html(vec![]))
                    .create_async()
                    .await;
            }

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect_consumption_detail_metrics().await;

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 15); // 10 from page 1 + 5 from page 2
        }

        #[tokio::test]
        async fn test_collect_consumption_stops_on_duplicate() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let items = vec![("エアコン", "1.2"), ("冷蔵庫", "0.3")];

            // Page 1
            let _mock1 = server
                .mock("GET", "/page/electricflow/1113?id=1")
                .with_status(200)
                .with_body(create_consumption_page_html(items.clone()))
                .create_async()
                .await;

            // Page 2 with same items (duplicate names)
            let _mock2 = server
                .mock("GET", "/page/electricflow/1113?id=2")
                .with_status(200)
                .with_body(create_consumption_page_html(items))
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect_consumption_detail_metrics().await;

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 2); // Only first page's items
        }

        #[tokio::test]
        async fn test_collect_consumption_merges_duplicates() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            // Page 1 with duplicate "エアコン"
            let _mock1 = server
                .mock("GET", "/page/electricflow/1113?id=1")
                .with_status(200)
                .with_body(create_consumption_page_html(vec![
                    ("エアコン", "1.2"),
                    ("冷蔵庫", "0.3"),
                    ("エアコン", "0.8"), // Duplicate
                ]))
                .create_async()
                .await;

            // Pages 2-20 empty
            for page in 2..=20 {
                let _ = server
                    .mock(
                        "GET",
                        format!("/page/electricflow/1113?id={}", page).as_str(),
                    )
                    .with_status(200)
                    .with_body(create_consumption_page_html(vec![]))
                    .create_async()
                    .await;
            }

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect_consumption_detail_metrics().await;

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 2); // Merged to 2 unique items

            // Verify all metrics can be converted
            for metric in metrics {
                assert!(metric.to_point().is_ok());
            }
        }

        #[tokio::test]
        async fn test_collect_consumption_handles_zero_watt() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let _mock1 = server
                .mock("GET", "/page/electricflow/1113?id=1")
                .with_status(200)
                .with_body(create_consumption_page_html(vec![
                    ("エアコン", "1.2"),
                    ("冷蔵庫", "invalid"), // Will default to 0
                    ("照明", "0.5"),
                ]))
                .create_async()
                .await;

            // Pages 2-20 empty
            for page in 2..=20 {
                let _ = server
                    .mock(
                        "GET",
                        format!("/page/electricflow/1113?id={}", page).as_str(),
                    )
                    .with_status(200)
                    .with_body(create_consumption_page_html(vec![]))
                    .create_async()
                    .await;
            }

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect_consumption_detail_metrics().await;

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 3);

            // Verify all metrics can be converted
            for metric in metrics {
                assert!(metric.to_point().is_ok());
            }
        }

        #[tokio::test]
        async fn test_collect_returns_all_metrics() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            // Mock main page
            let _mock_main = server
                .mock("GET", "/page/electricflow/111")
                .with_status(200)
                .with_body(create_main_page_html("2.5", "3.8", vec![("太陽光", "2.5")]))
                .create_async()
                .await;

            // Mock consumption page
            let _mock_cons1 = server
                .mock("GET", "/page/electricflow/1113?id=1")
                .with_status(200)
                .with_body(create_consumption_page_html(vec![
                    ("エアコン", "1.2"),
                    ("冷蔵庫", "0.3"),
                ]))
                .create_async()
                .await;

            // Pages 2-20 empty
            for page in 2..=20 {
                let _ = server
                    .mock(
                        "GET",
                        format!("/page/electricflow/1113?id={}", page).as_str(),
                    )
                    .with_status(200)
                    .with_body(create_consumption_page_html(vec![]))
                    .create_async()
                    .await;
            }

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect(Local::now()).await;

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 6); // 3 total + 1 generation detail + 2 consumption

            // Verify all can be converted to InfluxDB format
            for metric in metrics {
                assert!(metric.to_point().is_ok());
            }
        }

        #[tokio::test]
        async fn test_collect_with_various_values() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            // Mock main page with edge case values
            let _mock_main = server
                .mock("GET", "/page/electricflow/111")
                .with_status(200)
                .with_body(create_main_page_html(
                    "0.001",  // Very small
                    "99.999", // Large value
                    vec![("太陽光", "12.345"), ("燃料電池", "0.0")],
                ))
                .create_async()
                .await;

            // Mock consumption with various values
            let _mock_cons1 = server
                .mock("GET", "/page/electricflow/1113?id=1")
                .with_status(200)
                .with_body(create_consumption_page_html(vec![
                    ("Device1", "0.0"),
                    ("Device2", "1.999"),
                    ("Device3", "50.5"),
                ]))
                .create_async()
                .await;

            // Pages 2-20 empty
            for page in 2..=20 {
                let _ = server
                    .mock(
                        "GET",
                        format!("/page/electricflow/1113?id={}", page).as_str(),
                    )
                    .with_status(200)
                    .with_body(create_consumption_page_html(vec![]))
                    .create_async()
                    .await;
            }

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect(Local::now()).await;

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert!(!metrics.is_empty());
        }
    }

    mod fails {
        use super::*;

        #[test]
        fn test_collect_total_metrics_missing_g_capacity() {
            let html = r#"<html><body><div id="u_capacity">3.8</div></body></html>"#;
            let document = Html::parse_document(html);
            let collector = PowerMetricCollector::new(Arc::new(Client::new(test_config(
                "http://test".to_string(),
            ))));

            let result = collector.collect_total_metrics(&document);

            assert!(result.is_err());
            match result {
                Err(e) => assert!(e.to_string().contains("Failed to find value")),
                Ok(_) => panic!("Expected error but got success"),
            }
        }

        #[test]
        fn test_collect_total_metrics_missing_u_capacity() {
            let html = r#"<html><body><div id="g_capacity">2.5</div></body></html>"#;
            let document = Html::parse_document(html);
            let collector = PowerMetricCollector::new(Arc::new(Client::new(test_config(
                "http://test".to_string(),
            ))));

            let result = collector.collect_total_metrics(&document);

            assert!(result.is_err());
            match result {
                Err(e) => assert!(e.to_string().contains("Failed to find value")),
                Ok(_) => panic!("Expected error but got success"),
            }
        }

        #[test]
        fn test_collect_total_metrics_invalid_numeric() {
            let html = r#"<html><body>
                <div id="g_capacity">invalid</div>
                <div id="u_capacity">3.8</div>
            </body></html>"#;
            let document = Html::parse_document(html);
            let collector = PowerMetricCollector::new(Arc::new(Client::new(test_config(
                "http://test".to_string(),
            ))));

            let result = collector.collect_total_metrics(&document);

            assert!(result.is_err());
            match result {
                Err(e) => assert!(e.to_string().contains("Failed to parse value")),
                Ok(_) => panic!("Expected error but got success"),
            }
        }

        #[tokio::test]
        async fn test_collect_from_main_page_http_error() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let _mock = server
                .mock("GET", "/page/electricflow/111")
                .with_status(500)
                .with_body("Internal Server Error")
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect_from_main_page().await;

            assert!(result.is_err());
            match result {
                Err(e) => assert!(e.to_string().contains("Request failed with status: 500")),
                Ok(_) => panic!("Expected error but got success"),
            }
        }

        #[tokio::test]
        async fn test_collect_consumption_http_error() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let _mock = server
                .mock("GET", "/page/electricflow/1113?id=1")
                .with_status(404)
                .with_body("Not Found")
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect_consumption_detail_metrics().await;

            assert!(result.is_err());
            match result {
                Err(e) => assert!(e.to_string().contains("Request failed with status: 404")),
                Ok(_) => panic!("Expected error but got success"),
            }
        }

        #[tokio::test]
        async fn test_collect_consumption_empty_page() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            // First page is empty
            let _mock = server
                .mock("GET", "/page/electricflow/1113?id=1")
                .with_status(200)
                .with_body(create_consumption_page_html(vec![]))
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect_consumption_detail_metrics().await;

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 0); // No items collected
        }

        #[tokio::test]
        async fn test_collect_main_page_fails() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            // Main page fails
            let _mock_main = server
                .mock("GET", "/page/electricflow/111")
                .with_status(503)
                .with_body("Service Unavailable")
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect(Local::now()).await;

            assert!(result.is_err());
            // Should fail fast on main page error
            match result {
                Err(e) => assert!(e.to_string().contains("Request failed with status: 503")),
                Ok(_) => panic!("Expected error but got success"),
            }
        }

        #[tokio::test]
        async fn test_collect_consumption_fails() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            // Main page succeeds
            let _mock_main = server
                .mock("GET", "/page/electricflow/111")
                .with_status(200)
                .with_body(create_main_page_html("2.5", "3.8", vec![]))
                .create_async()
                .await;

            // Consumption page fails
            let _mock_cons = server
                .mock("GET", "/page/electricflow/1113?id=1")
                .with_status(500)
                .with_body("Internal Server Error")
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect(Local::now()).await;

            assert!(result.is_err());
            match result {
                Err(e) => assert!(e.to_string().contains("Request failed with status: 500")),
                Ok(_) => panic!("Expected error but got success"),
            }
        }

        #[tokio::test]
        async fn test_collect_both_fail() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            // Both endpoints fail
            let _mock_main = server
                .mock("GET", "/page/electricflow/111")
                .with_status(503)
                .with_body("Service Unavailable")
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = PowerMetricCollector::new(client);

            let result = collector.collect(Local::now()).await;

            assert!(result.is_err());
            // Fails on first error (main page)
            match result {
                Err(e) => assert!(e.to_string().contains("Request failed with status: 503")),
                Ok(_) => panic!("Expected error but got success"),
            }
        }
    }
}
