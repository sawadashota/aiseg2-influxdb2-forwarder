use crate::aiseg::helper::html_selector;
use crate::aiseg::Client;
use crate::model::{
    ClimateStatusMetric, ClimateStatusMetricCategory, DataPointBuilder, Measurement,
    MetricCollector,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use scraper::Html;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct ClimateMetricCollector {
    client: Arc<Client>,
}

impl ClimateMetricCollector {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }
}

impl MetricCollector for ClimateMetricCollector {
    fn collect<'a>(
        &'a self,
        timestamp: DateTime<Local>,
    ) -> Pin<Box<dyn Future<Output=Result<Vec<Box<dyn DataPointBuilder>>>> + Send + 'a>> {
        Box::pin(async move {
            let mut list: Vec<ClimateStatusMetric> = vec![];

            'root: for page in 1..=20 {
                let response = self
                    .client
                    .get(&format!("/page/airenvironment/41?page={}", page))
                    .await?;
                let document = Html::parse_document(&response);

                for i in 1..=3 {
                    let base_id = format!("#base{}_1", i);
                    let metrics = match parse(&document, &base_id, timestamp) {
                        Ok(metrics) => metrics,
                        Err(_) => break 'root,
                    };
                    list.extend(metrics);
                }
            }

            Ok(list
                .into_iter()
                .map(|item| Box::new(item) as Box<dyn DataPointBuilder>)
                .collect())
        })
    }
}

fn parse(
    document: &Html,
    base_id: &str,
    timestamp: DateTime<Local>,
) -> Result<[ClimateStatusMetric; 2]> {
    let base_selector = html_selector(base_id)?;
    let base_element = document
        .select(&base_selector)
        .next()
        .context("Failed to find value")?;

    // extract place name from `.txt_name`
    let name_selector = html_selector(".txt_name")?;
    let name = base_element
        .select(&name_selector)
        .next()
        .context("Failed to find name")?
        .text()
        .next()
        .context("Failed to get text")?;

    let num_wrapper_selector = html_selector(".num_wrapper")?;
    let num_wrapper_element = base_element
        .select(&num_wrapper_selector)
        .next()
        .context("Failed to find num_wrapper")?;

    // extract temperature from `#num_ond_\d`
    let temperature_selector = html_selector(r#"[id^="num_ond_"]"#)?;
    let temperature =
        extract_num_from_html_class(num_wrapper_element.select(&temperature_selector))?;

    // extract humidity from `#num_shitudo_\d`
    let humidity_selector = html_selector(r#"[id^="num_shitudo_"]"#)?;
    let humidity = extract_num_from_html_class(num_wrapper_element.select(&humidity_selector))?;

    Ok([
        ClimateStatusMetric {
            measurement: Measurement::Climate,
            category: ClimateStatusMetricCategory::Temperature,
            name: name.to_string(),
            value: temperature,
            timestamp,
        },
        ClimateStatusMetric {
            measurement: Measurement::Climate,
            category: ClimateStatusMetricCategory::Humidity,
            name: name.to_string(),
            value: humidity,
            timestamp,
        },
    ])
}

fn extract_num_from_html_class(elements: scraper::element_ref::Select) -> Result<f64> {
    let mut chars: [char; 4] = ['0', '0', '.', '0'];
    let mut i = 0;
    let mut element_count = 0;

    for element in elements {
        element_count += 1;
        if i == 2 {
            i += 1; // skip dot
        }
        if i >= 4 {
            break; // We have all 4 digits
        }
        let class_value = element.attr("class").context("Failed to get class")?;
        chars[i] = class_value
            .chars()
            .filter(|c| c.is_numeric())
            .collect::<String>()
            .parse::<char>()
            .context("Failed to parse value")?;
        i += 1;
    }

    if element_count != 4 {
        return Err(anyhow::anyhow!(
            "Expected 4 elements but found {}",
            element_count
        ));
    }

    Ok(chars.iter().collect::<String>().parse::<f64>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aiseg::test_utils::test_config;

    fn create_climate_html(items: Vec<(&str, &str, &str)>) -> String {
        let mut html = r#"<!DOCTYPE html><html><body>"#.to_string();

        for (i, (name, temp, humidity)) in items.iter().enumerate() {
            let base_id = i + 1;
            html.push_str(&format!(
                r#"<div id="base{}_1">
                    <div class="txt_name">{}</div>
                    <div class="num_wrapper">
                        <div id="num_ond_{}" class="num{}"></div>
                        <div id="num_ond_{}" class="num{}"></div>
                        <div id="num_ond_{}" class="num{}"></div>
                        <div id="num_ond_{}" class="num{}"></div>
                        <div id="num_shitudo_{}" class="num{}"></div>
                        <div id="num_shitudo_{}" class="num{}"></div>
                        <div id="num_shitudo_{}" class="num{}"></div>
                        <div id="num_shitudo_{}" class="num{}"></div>
                    </div>
                </div>"#,
                base_id,
                name,
                base_id,
                temp.chars().next().unwrap_or('0'),
                base_id,
                temp.chars().nth(1).unwrap_or('0'),
                base_id,
                temp.chars().nth(3).unwrap_or('0'),
                base_id,
                temp.chars().nth(4).unwrap_or('0'),
                base_id,
                humidity.chars().next().unwrap_or('0'),
                base_id,
                humidity.chars().nth(1).unwrap_or('0'),
                base_id,
                humidity.chars().nth(3).unwrap_or('0'),
                base_id,
                humidity.chars().nth(4).unwrap_or('0'),
            ));
        }

        html.push_str(r#"</body></html>"#);
        html
    }

    mod succeeds {
        use super::*;

        #[test]
        fn test_parse_single_base_element() {
            let html = create_climate_html(vec![("Living Room", "23.50", "45.60")]);
            let document = Html::parse_document(&html);
            let timestamp = Local::now();

            let result = parse(&document, "#base1_1", timestamp);

            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 2);

            let temp_metric = &metrics[0];
            assert_eq!(temp_metric.measurement, Measurement::Climate);
            assert_eq!(temp_metric.name, "Living Room");
            assert_eq!(temp_metric.value, 23.5);
            matches!(
                temp_metric.category,
                ClimateStatusMetricCategory::Temperature
            );

            let humidity_metric = &metrics[1];
            assert_eq!(humidity_metric.measurement, Measurement::Climate);
            assert_eq!(humidity_metric.name, "Living Room");
            assert_eq!(humidity_metric.value, 45.6);
            matches!(
                humidity_metric.category,
                ClimateStatusMetricCategory::Humidity
            );
        }

        #[tokio::test]
        async fn test_collect_single_page() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let html = create_climate_html(vec![
                ("Living Room", "23.50", "45.60"),
                ("Bedroom", "21.30", "52.10"),
                ("Kitchen", "25.80", "38.90"),
            ]);

            let _mock1 = server
                .mock("GET", "/page/airenvironment/41?page=1")
                .with_status(200)
                .with_body(html)
                .create_async()
                .await;

            // Mock page 2 to trigger early termination
            let _mock2 = server
                .mock("GET", "/page/airenvironment/41?page=2")
                .with_status(200)
                .with_body(r#"<html><body></body></html>"#)
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = ClimateMetricCollector::new(client);

            let result = collector.collect(Local::now()).await;

            if let Err(e) = &result {
                panic!("Failed to collect: {}", e)
            }
            let data_points = result.unwrap();
            assert_eq!(data_points.len(), 6); // 3 locations * 2 metrics each

            for dp in data_points {
                assert!(dp.to_point().is_ok());
            }
        }

        #[tokio::test]
        async fn test_collect_multiple_pages() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let page1_html = create_climate_html(vec![
                ("Room 1", "20.00", "40.00"),
                ("Room 2", "21.00", "41.00"),
                ("Room 3", "22.00", "42.00"),
            ]);

            let page2_html = create_climate_html(vec![
                ("Room 4", "23.00", "43.00"),
                ("Room 5", "24.00", "44.00"),
            ]);

            let page3_html = create_climate_html(vec![]);

            let _mock1 = server
                .mock("GET", "/page/airenvironment/41?page=1")
                .with_status(200)
                .with_body(page1_html)
                .create_async()
                .await;

            let _mock2 = server
                .mock("GET", "/page/airenvironment/41?page=2")
                .with_status(200)
                .with_body(page2_html)
                .create_async()
                .await;

            let _mock3 = server
                .mock("GET", "/page/airenvironment/41?page=3")
                .with_status(200)
                .with_body(page3_html)
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = ClimateMetricCollector::new(client);

            let result = collector.collect(Local::now()).await;

            assert!(result.is_ok());
            let data_points = result.unwrap();
            assert_eq!(data_points.len(), 10); // 5 locations * 2 metrics each
        }

        #[test]
        fn test_extract_num_from_html_class_various_values() {
            // The function expects 4 elements but only uses the first 3:
            // Element 0 -> position 0 (tens digit)
            // Element 1 -> position 1 (ones digit)  
            // Element 2 -> position 3 (decimal digit) - skips position 2 which has '.'
            // Element 3 -> not used but required for validation
            let test_cases = vec![
                (vec!["0", "0", "0", "0"], 0.0),   // "00.0"
                (vec!["2", "5", "5", "0"], 25.5),  // "25.5"
                (vec!["9", "9", "9", "0"], 99.9),  // "99.9"
                (vec!["0", "1", "2", "0"], 1.2),   // "01.2"
                (vec!["5", "0", "0", "0"], 50.0),  // "50.0"
            ];

            for (digits, expected) in test_cases {
                let html = format!(
                    r#"<div id="wrapper">
                        <span class="num{}"></span>
                        <span class="num{}"></span>
                        <span class="num{}"></span>
                        <span class="num{}"></span>
                    </div>"#,
                    digits[0], digits[1], digits[2], digits[3]
                );
                let document = scraper::Html::parse_document(&html);
                let wrapper_selector = scraper::Selector::parse("#wrapper").unwrap();
                let wrapper_element = document.select(&wrapper_selector).next().unwrap();
                let span_selector = scraper::Selector::parse("span").unwrap();
                let elements = wrapper_element.select(&span_selector);

                let result = extract_num_from_html_class(elements);
                assert!(result.is_ok(), "Failed for input {:?}", digits);
                assert_eq!(result.unwrap(), expected, "Failed for input {:?}", digits);
            }
        }

        #[tokio::test]
        async fn test_collect_with_early_termination() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let page1_html = create_climate_html(vec![
                ("Room 1", "20.00", "40.00"),
                ("Room 2", "21.00", "41.00"),
            ]);

            // Page 2 has no valid base1_1 element, causing early termination
            let page2_html = r#"<html><body><div>No climate data</div></body></html>"#;

            let _mock1 = server
                .mock("GET", "/page/airenvironment/41?page=1")
                .with_status(200)
                .with_body(page1_html)
                .create_async()
                .await;

            let _mock2 = server
                .mock("GET", "/page/airenvironment/41?page=2")
                .with_status(200)
                .with_body(page2_html)
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = ClimateMetricCollector::new(client);

            let result = collector.collect(Local::now()).await;

            assert!(result.is_ok());
            let data_points = result.unwrap();
            assert_eq!(data_points.len(), 4); // Only page 1 data (2 locations * 2 metrics)
        }
    }

    mod fails {
        use super::*;

        #[test]
        fn test_parse_missing_base_element() {
            let html = r#"<html><body><div>No base element</div></body></html>"#;
            let document = Html::parse_document(html);
            let timestamp = Local::now();

            let result = parse(&document, "#base1_1", timestamp);

            if let Ok(metrics) = &result {
                panic!("Expected error but got {} metrics", metrics.len())
            }
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to find value"));
        }

        #[test]
        fn test_parse_missing_name_element() {
            let html = r#"
                <div id="base1_1">
                    <div class="num_wrapper">
                        <div id="num_ond_1" class="num2"></div>
                    </div>
                </div>
            "#;
            let document = Html::parse_document(html);
            let timestamp = Local::now();

            let result = parse(&document, "#base1_1", timestamp);

            if let Ok(metrics) = &result {
                panic!("Expected error but got {} metrics", metrics.len())
            }
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to find name"));
        }

        #[test]
        fn test_parse_missing_num_wrapper() {
            let html = r#"
                <div id="base1_1">
                    <div class="txt_name">Room</div>
                </div>
            "#;
            let document = Html::parse_document(html);
            let timestamp = Local::now();

            let result = parse(&document, "#base1_1", timestamp);

            if let Ok(metrics) = &result {
                panic!("Expected error but got {} metrics", metrics.len())
            }
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to find num_wrapper"));
        }

        #[test]
        fn test_parse_missing_temperature() {
            let html = r#"
                <div id="base1_1">
                    <div class="txt_name">Room</div>
                    <div class="num_wrapper">
                        <div id="num_shitudo_1" class="num5"></div>
                        <div id="num_shitudo_1" class="num0"></div>
                        <div id="num_shitudo_1" class="num0"></div>
                        <div id="num_shitudo_1" class="num0"></div>
                    </div>
                </div>
            "#;
            let document = Html::parse_document(html);
            let timestamp = Local::now();

            let result = parse(&document, "#base1_1", timestamp);

            if let Ok(metrics) = &result {
                panic!("Expected error but got {} metrics", metrics.len())
            }
        }

        #[test]
        fn test_parse_missing_humidity() {
            let html = r#"
                <div id="base1_1">
                    <div class="txt_name">Room</div>
                    <div class="num_wrapper">
                        <div id="num_ond_1" class="num2"></div>
                        <div id="num_ond_1" class="num5"></div>
                        <div id="num_ond_1" class="num0"></div>
                        <div id="num_ond_1" class="num0"></div>
                    </div>
                </div>
            "#;
            let document = Html::parse_document(html);
            let timestamp = Local::now();

            let result = parse(&document, "#base1_1", timestamp);

            if let Ok(metrics) = &result {
                panic!("Expected error but got {} metrics", metrics.len())
            }
        }

        #[test]
        fn test_extract_num_from_html_class_valid_input() {
            // Function expects 4 elements but only uses first 3: [0]='2', [1]='3', skip, [3]='4'
            let html = r#"
                <div id="wrapper">
                    <span class="num2"></span>
                    <span class="num3"></span>
                    <span class="num4"></span>
                    <span class="num0"></span>
                </div>
            "#;
            let document = scraper::Html::parse_document(html);
            let wrapper_selector = scraper::Selector::parse("#wrapper").unwrap();
            let wrapper_element = document.select(&wrapper_selector).next().unwrap();
            let span_selector = scraper::Selector::parse("span").unwrap();
            let elements = wrapper_element.select(&span_selector);

            let result = extract_num_from_html_class(elements);

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 23.4);  // Formats as "23.4"
        }

        #[test]
        fn test_extract_num_from_html_class_zero_values() {
            let html = r#"
                <div id="wrapper">
                    <span class="num0"></span>
                    <span class="num0"></span>
                    <span class="num0"></span>
                    <span class="num0"></span>
                </div>
            "#;
            let document = scraper::Html::parse_document(html);
            let wrapper_selector = scraper::Selector::parse("#wrapper").unwrap();
            let wrapper_element = document.select(&wrapper_selector).next().unwrap();
            let span_selector = scraper::Selector::parse("span").unwrap();
            let elements = wrapper_element.select(&span_selector);

            let result = extract_num_from_html_class(elements);

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 0.0);  // Formats as "00.0"
        }

        #[test]
        fn test_extract_num_from_html_class_mixed_digits() {
            let html = r#"
                <div id="wrapper">
                    <span class="num9"></span>
                    <span class="num8"></span>
                    <span class="num7"></span>
                    <span class="num0"></span>
                </div>
            "#;
            let document = scraper::Html::parse_document(html);
            let wrapper_selector = scraper::Selector::parse("#wrapper").unwrap();
            let wrapper_element = document.select(&wrapper_selector).next().unwrap();
            let span_selector = scraper::Selector::parse("span").unwrap();
            let elements = wrapper_element.select(&span_selector);

            let result = extract_num_from_html_class(elements);

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 98.7);  // Formats as "98.7"
        }

        #[test]
        fn test_extract_num_from_html_class_with_extra_text() {
            let html = r#"
                <div id="wrapper">
                    <span class="prefix_num1_suffix"></span>
                    <span class="text_num2_more"></span>
                    <span class="num3_end"></span>
                    <span class="start_num0"></span>
                </div>
            "#;
            let document = scraper::Html::parse_document(html);
            let wrapper_selector = scraper::Selector::parse("#wrapper").unwrap();
            let wrapper_element = document.select(&wrapper_selector).next().unwrap();
            let span_selector = scraper::Selector::parse("span").unwrap();
            let elements = wrapper_element.select(&span_selector);

            let result = extract_num_from_html_class(elements);

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 12.3);  // Formats as "12.3"
        }

        #[test]
        fn test_extract_num_from_html_class_invalid_no_digit() {
            let html = r#"
                <div id="wrapper">
                    <span class="invalid"></span>
                    <span class="num2"></span>
                    <span class="num3"></span>
                    <span class="num4"></span>
                </div>
            "#;
            let document = scraper::Html::parse_document(html);
            let wrapper_selector = scraper::Selector::parse("#wrapper").unwrap();
            let wrapper_element = document.select(&wrapper_selector).next().unwrap();
            let span_selector = scraper::Selector::parse("span").unwrap();
            let elements = wrapper_element.select(&span_selector);

            let result = extract_num_from_html_class(elements);

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to parse value"));
        }

        #[test]
        fn test_extract_num_from_html_class_too_few_elements() {
            let html = r#"
                <div id="wrapper">
                    <span class="num1"></span>
                    <span class="num2"></span>
                    <span class="num3"></span>
                </div>
            "#;
            let document = scraper::Html::parse_document(html);
            let wrapper_selector = scraper::Selector::parse("#wrapper").unwrap();
            let wrapper_element = document.select(&wrapper_selector).next().unwrap();
            let span_selector = scraper::Selector::parse("span").unwrap();
            let elements = wrapper_element.select(&span_selector);

            let result = extract_num_from_html_class(elements);

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Expected 4 elements but found 3"));
        }

        #[test]
        fn test_extract_num_from_html_class_too_many_elements() {
            let html = r#"
                <div id="wrapper">
                    <span class="num1"></span>
                    <span class="num2"></span>
                    <span class="num3"></span>
                    <span class="num0"></span>
                    <span class="num5"></span>
                </div>
            "#;
            let document = scraper::Html::parse_document(html);
            let wrapper_selector = scraper::Selector::parse("#wrapper").unwrap();
            let wrapper_element = document.select(&wrapper_selector).next().unwrap();
            let span_selector = scraper::Selector::parse("span").unwrap();
            let elements = wrapper_element.select(&span_selector);

            let result = extract_num_from_html_class(elements);

            // The function should only process the first 4 elements (but only uses first 3)
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 12.3);  // Formats as "12.3"
        }

        #[test]
        fn test_extract_num_from_html_class_missing_class_attribute() {
            let html = r#"
                <div id="wrapper">
                    <span></span>
                    <span class="num2"></span>
                    <span class="num3"></span>
                    <span class="num4"></span>
                </div>
            "#;
            let document = scraper::Html::parse_document(html);
            let wrapper_selector = scraper::Selector::parse("#wrapper").unwrap();
            let wrapper_element = document.select(&wrapper_selector).next().unwrap();
            let span_selector = scraper::Selector::parse("span").unwrap();
            let elements = wrapper_element.select(&span_selector);

            let result = extract_num_from_html_class(elements);

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to get class"));
        }

        #[tokio::test]
        async fn test_collect_http_error() {
            let mut server = mockito::Server::new_async().await;
            let mock_url = server.url();

            let _mock = server
                .mock("GET", "/page/airenvironment/41?page=1")
                .with_status(500)
                .with_body("Internal Server Error")
                .create_async()
                .await;

            let config = test_config(mock_url);
            let client = Arc::new(Client::new(config));
            let collector = ClimateMetricCollector::new(client);

            let result = collector.collect(Local::now()).await;

            assert!(result.is_err());
            match result {
                Err(e) => assert!(e.to_string().contains("Request failed with status: 500")),
                Ok(_) => panic!("Expected error but got success"),
            }
        }
    }
}
