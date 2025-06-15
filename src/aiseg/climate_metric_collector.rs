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

/// Collector for climate metrics (temperature and humidity) from AiSEG2 air environment monitoring pages.
///
/// This collector scrapes temperature and humidity data from multiple rooms/locations
/// connected to the AiSEG2 system. It iterates through multiple pages (up to 20) to
/// gather all available climate data, with each page potentially containing up to 3
/// monitored locations.
///
/// # Data Collection
/// - **Temperature**: Measured in degrees Celsius with one decimal place precision (XX.X°C)
/// - **Humidity**: Measured as percentage with one decimal place precision (XX.X%)
///
/// # Page Structure
/// The AiSEG2 web interface displays climate data across multiple pages at
/// `/page/airenvironment/41?page={n}`. Each page can contain up to 3 location entries
/// identified by base element IDs (#base1_1, #base2_1, #base3_1).
///
/// # Example Usage
/// ```rust
/// let client = Arc::new(Client::new(config));
/// let collector = ClimateMetricCollector::new(client);
/// let metrics = collector.collect(Local::now()).await?;
/// ```
pub struct ClimateMetricCollector {
    client: Arc<Client>,
}

impl ClimateMetricCollector {
    /// Creates a new ClimateMetricCollector instance.
    ///
    /// # Arguments
    /// * `client` - Shared reference to the AiSEG2 HTTP client for making requests
    ///
    /// # Returns
    /// A new instance of ClimateMetricCollector ready to collect climate metrics
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }
}

impl MetricCollector for ClimateMetricCollector {
    /// Collects temperature and humidity metrics from all available climate monitoring locations.
    ///
    /// This method implements the MetricCollector trait's collect function, performing the following:
    /// 1. Iterates through AiSEG2 air environment pages (up to 20 pages)
    /// 2. For each page, attempts to parse up to 3 location entries
    /// 3. Extracts temperature and humidity values for each location
    /// 4. Returns a vector of DataPointBuilder instances for InfluxDB
    ///
    /// # Arguments
    /// * `timestamp` - The timestamp to assign to all collected metrics
    ///
    /// # Returns
    /// A future that resolves to a Result containing a vector of DataPointBuilder instances.
    /// Each location produces 2 data points (temperature and humidity).
    ///
    /// # Pagination Behavior
    /// The collector uses an early termination strategy - if parsing fails for any base element
    /// (indicating no more data), it stops iteration and returns the metrics collected so far.
    /// This prevents unnecessary HTTP requests to empty pages.
    ///
    /// # Error Handling
    /// - HTTP request failures are propagated up as errors
    /// - HTML parsing failures for individual locations trigger early termination
    /// - The method ensures at least partial data collection even if some pages fail
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

/// Parses climate data (temperature and humidity) from a specific base element in the HTML document.
///
/// # Arguments
/// * `document` - The parsed HTML document
/// * `base_id` - The base element ID (e.g., "#base1_1")
/// * `timestamp` - The timestamp for the metrics
///
/// # Returns
/// An array of two metrics: [temperature, humidity]
///
/// # Behavior
/// - Extracts location name from `.txt_name` element
/// - Looks for temperature values in elements matching `[id^="num_ond_"][class*="num no"]`
/// - Looks for humidity values in elements matching `[id^="num_shitudo_"][class*="num no"]`
/// - If either temperature or humidity elements are missing, defaults to 0.0
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

    // extract temperature from `#num_ond_XX_Y` where XX is base number and Y is 1,2,3
    let temperature_selector = html_selector(r#"[id^="num_ond_"][class*="num no"]"#)?;
    let temperature =
        extract_num_from_html_class(num_wrapper_element.select(&temperature_selector))?;

    // extract humidity from `#num_shitudo_XX_Y` where XX is base number and Y is 1,2,3
    let humidity_selector = html_selector(r#"[id^="num_shitudo_"][class*="num no"]"#)?;
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

/// Extracts a numeric value from HTML elements representing digits of a decimal number.
///
/// # Arguments
/// * `elements` - Iterator of HTML elements with class attributes containing numeric values
///
/// # Returns
/// A floating-point number parsed from the extracted digits
///
/// # Expected Format
/// The function expects exactly 3 elements representing digits in format XX.X:
/// - Element 0: First digit (tens place)
/// - Element 1: Second digit (ones place)
/// - Element 2: Third digit (tenths place after decimal)
///
/// # Behavior
/// - Extracts numeric characters from each element's class attribute (e.g., "num no5" -> '5')
/// - Automatically inserts a decimal point between the second and third digits
/// - If fewer than 3 elements are provided, remaining positions default to '0'
/// - If more than 3 elements are provided, only the first 3 are processed
/// - Returns 0.0 if no elements are provided
///
/// # Example
/// Given elements with classes ["num no2", "num no3", "num no5"], returns 23.5
fn extract_num_from_html_class(elements: scraper::element_ref::Select) -> Result<f64> {
    const EXPECTED_DIGITS: usize = 3;
    
    // Initialize with default values for XX.X format
    let mut digits = vec!['0', '0', '0']; // [tens, ones, tenths]
    let mut processed_count = 0;
    
    // Process up to 3 elements
    for element in elements.take(EXPECTED_DIGITS) {
        // Extract class attribute
        let class_value = element.attr("class").context("Failed to get class")?;
        
        // Extract the first numeric character from the class
        let digit = class_value
            .chars()
            .filter(|c| c.is_numeric())
            .next()
            .context("No numeric character found in class")?;
        
        digits[processed_count] = digit;
        processed_count += 1;
    }
    
    // Build the decimal number string: "XX.X"
    let number_str = format!(
        "{}{}.{}",
        digits[0],
        digits[1],
        digits[2]
    );
    
    // Parse to f64
    number_str
        .parse::<f64>()
        .context("Failed to parse decimal number")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aiseg::test_utils::test_config;

    fn create_climate_html(items: Vec<(&str, &str, &str)>) -> String {
        let mut html = r#"<!DOCTYPE html><html><body>"#.to_string();

        for (i, (name, temp, humidity)) in items.iter().enumerate() {
            let base_id = format!("{}", i + 1);

            // Extract temperature digits (format: XX.X)
            let temp_digit1 = temp.chars().next().unwrap_or('0');
            let temp_digit2 = temp.chars().nth(1).unwrap_or('0');
            let temp_digit3 = temp.chars().nth(3).unwrap_or('0'); // Skip decimal point at position 2

            // Extract humidity digits (format: XX.X)
            let hum_digit1 = humidity.chars().next().unwrap_or('0');
            let hum_digit2 = humidity.chars().nth(1).unwrap_or('0');
            let hum_digit3 = humidity.chars().nth(3).unwrap_or('0'); // Skip decimal point at position 2

            html.push_str(&format!(
                r#"<div id="base{}_1">
                    <div class="txt_name">{}</div>
                    <div class="num_wrapper">
                        <div class="num_ond" style="visibility:visible">
                            <div class="icon_ond"></div>
                            <div id="num_ond_{}_1" class="num no{}"></div>
                            <div id="num_ond_{}_2" class="num no{}"></div>
                            <div id="num_dot_place1" class="num_dot"></div>
                            <div id="num_ond_{}_3" class="num no{}"></div>
                            <div class="num_tani"></div>
                        </div>
                        <div class="num_shitudo" style="visibility:visible">
                            <div class="icon_shitudo"></div>
                            <div id="num_shitudo_{}_1" class="num no{}"></div>
                            <div id="num_shitudo_{}_2" class="num no{}"></div>
                            <div id="num_dot_place2" class="num_dot"></div>
                            <div id="num_shitudo_{}_3" class="num no{}"></div>
                            <div class="num_tani"></div>
                        </div>
                    </div>
                </div>"#,
                base_id,
                name,
                base_id, temp_digit1,
                base_id, temp_digit2,
                base_id, temp_digit3,
                base_id, hum_digit1,
                base_id, hum_digit2,
                base_id, hum_digit3
            ));
        }

        html.push_str(r#"</body></html>"#);
        html
    }

    mod succeeds {
        use super::*;

        #[test]
        fn test_parse_single_base_element() {
            let html = create_climate_html(vec![("Living Room", "23.5", "45.6")]);
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
            // The function expects exactly 3 elements:
            // Element 0 -> position 0 (tens digit)
            // Element 1 -> position 1 (ones digit)  
            // Element 2 -> position 3 (decimal digit) - skips position 2 which has '.'
            let test_cases = vec![
                (vec!["0", "0", "0"], 0.0),   // "00.0"
                (vec!["2", "5", "5"], 25.5),  // "25.5"
                (vec!["9", "9", "9"], 99.9),  // "99.9"
                (vec!["0", "1", "2"], 1.2),   // "01.2"
                (vec!["5", "0", "0"], 50.0),  // "50.0"
            ];

            for (digits, expected) in test_cases {
                let html = format!(
                    r#"<div id="wrapper">
                        <span class="num{}"></span>
                        <span class="num{}"></span>
                        <span class="num{}"></span>
                    </div>"#,
                    digits[0], digits[1], digits[2]
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
                        <div id="num_shitudo_1_1" class="num no5"></div>
                        <div id="num_shitudo_1_2" class="num no0"></div>
                        <div id="num_shitudo_1_3" class="num no0"></div>
                    </div>
                </div>
            "#;
            let document = Html::parse_document(html);
            let timestamp = Local::now();

            let result = parse(&document, "#base1_1", timestamp);

            // With missing temperature elements, it uses default 0.0
            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 2);
            assert_eq!(metrics[0].value, 0.0); // temperature defaults to 0.0
            assert_eq!(metrics[1].value, 50.0); // humidity is 50.0
        }

        #[test]
        fn test_parse_missing_humidity() {
            let html = r#"
                <div id="base1_1">
                    <div class="txt_name">Room</div>
                    <div class="num_wrapper">
                        <div id="num_ond_1_1" class="num no2"></div>
                        <div id="num_ond_1_2" class="num no5"></div>
                        <div id="num_ond_1_3" class="num no0"></div>
                    </div>
                </div>
            "#;
            let document = Html::parse_document(html);
            let timestamp = Local::now();

            let result = parse(&document, "#base1_1", timestamp);

            // With missing humidity elements, it uses default 0.0
            assert!(result.is_ok());
            let metrics = result.unwrap();
            assert_eq!(metrics.len(), 2);
            assert_eq!(metrics[0].value, 25.0); // temperature is 25.0
            assert_eq!(metrics[1].value, 0.0); // humidity defaults to 0.0
        }

        #[test]
        fn test_extract_num_from_html_class_valid_input() {
            // Function now expects exactly 3 elements: [0]='2', [1]='3', skip position 2, [2]='4'
            let html = r#"
                <div id="wrapper">
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
                .contains("No numeric character found in class"));
        }

        #[test]
        fn test_extract_num_from_html_class_too_few_elements() {
            let html = r#"
                <div id="wrapper">
                    <span class="num1"></span>
                    <span class="num2"></span>
                </div>
            "#;
            let document = scraper::Html::parse_document(html);
            let wrapper_selector = scraper::Selector::parse("#wrapper").unwrap();
            let wrapper_element = document.select(&wrapper_selector).next().unwrap();
            let span_selector = scraper::Selector::parse("span").unwrap();
            let elements = wrapper_element.select(&span_selector);

            let result = extract_num_from_html_class(elements);

            // With only 2 elements, the function will process what it can
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 12.0);  // Formats as "12.0"
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

            // The function processes exactly 3 elements and ignores the rest
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
