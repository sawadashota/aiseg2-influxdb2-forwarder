//! Shared HTML parsing utilities for AiSEG2 collectors.
//!
//! This module provides common HTML parsing functions used across different collectors
//! to reduce code duplication and ensure consistent parsing behavior.

use crate::error::{ParseError, Result};
use scraper::{ElementRef, Html};
use std::str::FromStr;

use crate::aiseg::helper::{
    html_selector, kilowatts_to_watts, parse_f64_from_html, parse_text_from_html,
};

/// Generic HTML value extractor that supports any type implementing FromStr.
///
/// This function provides a unified way to extract and parse values from HTML elements,
/// reducing duplication across collectors.
///
/// # Arguments
/// * `document` - The parsed HTML document
/// * `selector` - CSS selector for the element containing the value
///
/// # Returns
/// * `Ok(T)` - The parsed value of type T
/// * `Err` - If the element is not found or parsing fails
///
/// # Example
/// ```no_run
/// // Extract a float value
/// let value: f64 = extract_value(&document, "#power_value")?;
///
/// // Extract an integer value
/// let count: i32 = extract_value(&document, ".item-count")?;
///
/// // Extract a string value
/// let name: String = extract_value(&document, "#device_name")?;
/// ```
pub fn extract_value<T: FromStr>(document: &Html, selector: &str) -> Result<T, ParseError>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let selector_obj = html_selector(selector)?;
    let element = document
        .select(&selector_obj)
        .next()
        .ok_or_else(|| ParseError::element_not_found(selector))?;

    let text = element.text().collect::<String>().trim().to_string();

    text.parse::<T>()
        .map_err(|e| ParseError::number_parse(&text, e))
}

/// Parses a graph page with title and value elements.
///
/// This function handles the common pattern of AiSEG2 graph pages that have
/// a title element and a value element (typically kWh values).
///
/// # Arguments
/// * `document` - The parsed HTML document
/// * `title_selector` - CSS selector for the title element (default: "#h_title")
/// * `value_selector` - CSS selector for the value element (default: "#val_kwh")
///
/// # Returns
/// A tuple of (title, value_in_watts)
pub fn parse_graph_page(
    document: &Html,
    title_selector: Option<&str>,
    value_selector: Option<&str>,
) -> Result<(String, i64), ParseError> {
    let title_sel = title_selector.unwrap_or("#h_title");
    let value_sel = value_selector.unwrap_or("#val_kwh");

    let title = extract_value::<String>(document, title_sel)?;
    let kwh_value = extract_value::<f64>(document, value_sel)?;
    let watts_value = kilowatts_to_watts(kwh_value);

    Ok((title, watts_value))
}

/// Extracts a numeric value from HTML elements with class attributes containing digits.
///
/// This function is used for parsing climate data where values are represented
/// as individual digit elements (e.g., temperature "23.5" as three elements).
///
/// # Arguments
/// * `elements` - Iterator of HTML elements with numeric class attributes
///
/// # Returns
/// A floating-point number parsed from the extracted digits
///
/// # Format
/// Expects exactly 3 elements representing XX.X format:
/// - Element 0: tens place
/// - Element 1: ones place  
/// - Element 2: tenths place (after decimal)
pub fn extract_numeric_from_digit_elements<'a, I>(elements: I) -> Result<f64, ParseError>
where
    I: Iterator<Item = ElementRef<'a>>,
{
    let mut digits = ['0', '0', '0'];

    for (i, element) in elements.enumerate() {
        if i >= 3 {
            break;
        }

        if let Some(class) = element.value().attr("class") {
            for ch in class.chars() {
                if ch.is_numeric() {
                    digits[i] = ch;
                    break;
                }
            }
        }
    }

    let value_str = format!("{}{}.{}", digits[0], digits[1], digits[2]);
    value_str
        .parse::<f64>()
        .map_err(|e| ParseError::number_parse(&value_str, e))
}

/// Parses a single consumption device entry from HTML.
///
/// # Arguments
/// * `document` - The parsed HTML document
/// * `stage_id` - The stage element ID (e.g., "#stage_1")
///
/// # Returns
/// A tuple of (device_name, power_value) or None if the element doesn't exist
pub fn parse_consumption_device(
    document: &Html,
    stage_id: &str,
) -> Result<Option<(String, f64)>, ParseError> {
    let device_selector = format!("{} > div.c_device", stage_id);
    let device_name = match parse_text_from_html(document, &device_selector) {
        Ok(name) => name,
        Err(_) => return Ok(None),
    };

    let value_selector = format!("{} > div.c_value", stage_id);
    let power_value = parse_f64_from_html(document, &value_selector).unwrap_or(0.0);

    Ok(Some((device_name, power_value)))
}

/// Parses generation detail metrics from the main power page.
///
/// # Arguments
/// * `document` - The parsed HTML document
/// * `max_items` - Maximum number of generation items to parse
///
/// # Returns
/// A vector of tuples (name, value) for each generation source found
pub fn parse_generation_details(
    document: &Html,
    max_items: usize,
) -> Result<Vec<(String, f64)>, ParseError> {
    let mut results = Vec::with_capacity(max_items);

    for i in 1..=max_items {
        let title_selector = format!("#g_d_{}_title", i);
        let name = match parse_text_from_html(document, &title_selector) {
            Ok(name) => name,
            Err(_) => break,
        };

        let capacity_selector = format!("#g_d_{}_capacity", i);
        let value = parse_f64_from_html(document, &capacity_selector)?;

        results.push((name, value));
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_value_string() {
        let html = Html::parse_document(r#"<div id="title">Test Title</div>"#);
        let result: Result<String, ParseError> = extract_value(&html, "#title");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Test Title");
    }

    #[test]
    fn test_extract_value_f64() {
        let html = Html::parse_document(r#"<div class="value">123.45</div>"#);
        let result: Result<f64, ParseError> = extract_value(&html, ".value");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 123.45);
    }

    #[test]
    fn test_extract_value_i32() {
        let html = Html::parse_document(r#"<span id="count">42</span>"#);
        let result: Result<i32, ParseError> = extract_value(&html, "#count");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_extract_value_with_whitespace() {
        let html = Html::parse_document(r#"<div class="num">  789  </div>"#);
        let result: Result<i32, ParseError> = extract_value(&html, ".num");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 789);
    }

    #[test]
    fn test_extract_value_element_not_found() {
        let html = Html::parse_document(r#"<div>Content</div>"#);
        let result: Result<String, ParseError> = extract_value(&html, "#missing");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("element not found"));
    }

    #[test]
    fn test_extract_value_parse_error() {
        let html = Html::parse_document(r#"<div id="val">not_a_number</div>"#);
        let result: Result<f64, ParseError> = extract_value(&html, "#val");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("failed to parse number"));
    }

    #[test]
    fn test_parse_graph_page_default_selectors() {
        let html = Html::parse_document(
            r#"<div id="h_title">Solar Generation</div><div id="val_kwh">2.5</div>"#,
        );
        let result = parse_graph_page(&html, None, None);
        assert!(result.is_ok());
        let (title, watts) = result.unwrap();
        assert_eq!(title, "Solar Generation");
        assert_eq!(watts, 2500);
    }

    #[test]
    fn test_parse_graph_page_custom_selectors() {
        let html = Html::parse_document(
            r#"<div class="title">Custom Title</div><div class="power">1.234</div>"#,
        );
        let result = parse_graph_page(&html, Some(".title"), Some(".power"));
        assert!(result.is_ok());
        let (title, watts) = result.unwrap();
        assert_eq!(title, "Custom Title");
        assert_eq!(watts, 1234);
    }

    #[test]
    fn test_extract_numeric_from_digit_elements() {
        let html = Html::parse_document(
            r#"<div>
                <span class="num no2"></span>
                <span class="num no3"></span>
                <span class="num no5"></span>
            </div>"#,
        );

        let selector = html_selector(r#"span[class*="num no"]"#).unwrap();
        let elements = html.select(&selector);
        let result = extract_numeric_from_digit_elements(elements);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 23.5);
    }

    #[test]
    fn test_parse_consumption_device() {
        let html = Html::parse_document(
            r#"<div id="stage_1">
                <div class="c_device"><span>エアコン</span></div>
                <div class="c_value"><span>1.2</span></div>
            </div>"#,
        );

        let result = parse_consumption_device(&html, "#stage_1");
        assert!(result.is_ok());

        let device = result.unwrap();
        assert!(device.is_some());

        let (name, value) = device.unwrap();
        assert_eq!(name, "エアコン");
        assert_eq!(value, 1.2);
    }

    #[test]
    fn test_parse_generation_details() {
        let html = Html::parse_document(
            r#"<div>
                <div id="g_d_1_title"><span>太陽光</span></div>
                <div id="g_d_1_capacity"><span>2.5</span></div>
                <div id="g_d_2_title"><span>燃料電池</span></div>
                <div id="g_d_2_capacity"><span>0.5</span></div>
            </div>"#,
        );

        let result = parse_generation_details(&html, 4);
        assert!(result.is_ok());

        let details = result.unwrap();
        assert_eq!(details.len(), 2);
        assert_eq!(details[0], ("太陽光".to_string(), 2.5));
        assert_eq!(details[1], ("燃料電池".to_string(), 0.5));
    }
}
