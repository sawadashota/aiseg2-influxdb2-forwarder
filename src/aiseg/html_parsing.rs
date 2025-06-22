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

/// Extracts a value with error recovery, returning a default if parsing fails.
///
/// This function provides resilience against malformed HTML by returning
/// a default value instead of propagating errors.
///
/// # Arguments
/// * `document` - The parsed HTML document
/// * `selector` - CSS selector for the element
/// * `default` - Default value to return on error
///
/// # Returns
/// The parsed value or the default value if parsing fails
#[allow(dead_code)]
pub fn extract_value_or_default<T: FromStr + Clone>(
    document: &Html,
    selector: &str,
    default: T,
) -> T
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    extract_value(document, selector).unwrap_or(default)
}

/// Attempts to extract a value with multiple selector fallbacks.
///
/// This function tries multiple selectors in order and returns the first
/// successful result, providing resilience against HTML structure changes.
///
/// # Arguments
/// * `document` - The parsed HTML document
/// * `selectors` - List of CSS selectors to try in order
///
/// # Returns
/// * `Ok(T)` - The first successfully parsed value
/// * `Err` - If all selectors fail
#[allow(dead_code)]
pub fn extract_value_with_fallbacks<T: FromStr>(
    document: &Html,
    selectors: &[&str],
) -> Result<T, ParseError>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    for selector in selectors {
        if let Ok(value) = extract_value(document, selector) {
            return Ok(value);
        }
    }

    Err(ParseError::UnexpectedStructure(format!(
        "Failed to extract value from any selector: {:?}",
        selectors
    )))
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

/// Parses a power value from HTML and converts it to watts.
///
/// This function extracts a numeric value (assumed to be in kW) and
/// automatically converts it to watts for storage.
///
/// # Arguments
/// * `document` - The parsed HTML document
/// * `selector` - CSS selector for the power value element
///
/// # Returns
/// Power value in watts (as i64)
#[allow(dead_code)]
pub fn parse_power_value(document: &Html, selector: &str) -> Result<i64, ParseError> {
    let kw_value = extract_value::<f64>(document, selector)?;
    Ok(kilowatts_to_watts(kw_value))
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

/// Parses a numeric value with its unit from HTML.
///
/// This function extracts both the numeric value and any unit suffix,
/// useful for values like "123.45kW" or "25.5℃".
///
/// # Arguments
/// * `document` - The parsed HTML document
/// * `selector` - CSS selector for the element
///
/// # Returns
/// A tuple of (value, unit_text) where unit_text may be empty
#[allow(dead_code)]
pub fn parse_numeric_with_unit(
    document: &Html,
    selector: &str,
) -> Result<(f64, String), ParseError> {
    let selector_obj = html_selector(selector)?;
    let element = document
        .select(&selector_obj)
        .next()
        .ok_or_else(|| ParseError::element_not_found(selector))?;

    let full_text = element.text().collect::<String>();

    // Extract numeric part
    let numeric_str: String = full_text
        .chars()
        .filter(|c| c.is_numeric() || *c == '.')
        .collect();

    let value = numeric_str
        .parse::<f64>()
        .map_err(|e| ParseError::number_parse(&numeric_str, e))?;

    // Extract unit part (everything after the last digit)
    let unit_start = full_text
        .rfind(|c: char| c.is_numeric() || c == '.')
        .map(|i| i + 1)
        .unwrap_or(full_text.len());

    let unit = full_text[unit_start..].trim().to_string();

    Ok((value, unit))
}

/// Parses a list of items from repeated HTML elements.
///
/// This generic function can parse tables, lists, or any repeated structure.
///
/// # Arguments
/// * `document` - The parsed HTML document
/// * `container_selector` - Selector for the container element
/// * `item_selector` - Selector for individual items within the container
/// * `parse_item` - Function to parse each item element
///
/// # Returns
/// Vector of parsed items
#[allow(dead_code)]
pub fn parse_item_list<T, F>(
    document: &Html,
    container_selector: &str,
    item_selector: &str,
    parse_item: F,
) -> Result<Vec<T>, ParseError>
where
    F: Fn(ElementRef) -> Result<T, ParseError>,
{
    let container_sel = html_selector(container_selector)?;
    let container = document
        .select(&container_sel)
        .next()
        .ok_or_else(|| ParseError::element_not_found(container_selector))?;

    let item_sel = html_selector(item_selector)?;
    let items: Result<Vec<T>, ParseError> = container.select(&item_sel).map(parse_item).collect();

    items
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
    fn test_extract_value_or_default_success() {
        let html = Html::parse_document(r#"<div class="num">100</div>"#);
        let result = extract_value_or_default(&html, ".num", 0);
        assert_eq!(result, 100);
    }

    #[test]
    fn test_extract_value_or_default_fallback() {
        let html = Html::parse_document(r#"<div>No number here</div>"#);
        let result = extract_value_or_default(&html, ".missing", 42);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_extract_value_with_fallbacks_first_succeeds() {
        let html =
            Html::parse_document(r#"<div id="primary">100</div><div id="secondary">200</div>"#);
        let result: Result<i32, ParseError> =
            extract_value_with_fallbacks(&html, &["#primary", "#secondary"]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 100);
    }

    #[test]
    fn test_extract_value_with_fallbacks_second_succeeds() {
        let html = Html::parse_document(r#"<div id="secondary">200</div>"#);
        let result: Result<i32, ParseError> =
            extract_value_with_fallbacks(&html, &["#primary", "#secondary"]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 200);
    }

    #[test]
    fn test_extract_value_with_fallbacks_all_fail() {
        let html = Html::parse_document(r#"<div>Nothing here</div>"#);
        let result: Result<i32, ParseError> =
            extract_value_with_fallbacks(&html, &["#primary", "#secondary"]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParseError::UnexpectedStructure(_)
        ));
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
    fn test_parse_power_value() {
        let html = Html::parse_document(r#"<div id="power">3.75</div>"#);
        let result = parse_power_value(&html, "#power");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3750);
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

    #[test]
    fn test_parse_numeric_with_unit() {
        let html = Html::parse_document(
            r#"<div>
                <div class="value1">123.45kW</div>
                <div class="value2">25.5℃</div>
                <div class="value3">100</div>
                <div class="value4">50.0 %</div>
            </div>"#,
        );

        // Test with unit attached
        let result1 = parse_numeric_with_unit(&html, ".value1");
        assert!(result1.is_ok());
        let (value1, unit1) = result1.unwrap();
        assert_eq!(value1, 123.45);
        assert_eq!(unit1, "kW");

        // Test with temperature unit
        let result2 = parse_numeric_with_unit(&html, ".value2");
        assert!(result2.is_ok());
        let (value2, unit2) = result2.unwrap();
        assert_eq!(value2, 25.5);
        assert_eq!(unit2, "℃");

        // Test without unit
        let result3 = parse_numeric_with_unit(&html, ".value3");
        assert!(result3.is_ok());
        let (value3, unit3) = result3.unwrap();
        assert_eq!(value3, 100.0);
        assert_eq!(unit3, "");

        // Test with space-separated unit
        let result4 = parse_numeric_with_unit(&html, ".value4");
        assert!(result4.is_ok());
        let (value4, unit4) = result4.unwrap();
        assert_eq!(value4, 50.0);
        assert_eq!(unit4, "%");
    }

    #[test]
    fn test_parse_item_list() {
        let html = Html::parse_document(
            r#"<div class="container">
                <div class="item" data-id="1">Item 1</div>
                <div class="item" data-id="2">Item 2</div>
                <div class="item" data-id="3">Item 3</div>
            </div>"#,
        );

        let result = parse_item_list(&html, ".container", ".item", |element| {
            let id = element
                .value()
                .attr("data-id")
                .ok_or_else(|| {
                    ParseError::UnexpectedStructure("Missing data-id attribute".to_string())
                })?
                .parse::<u32>()
                .map_err(|e| ParseError::number_parse("data-id", e))?;
            let text = element.text().collect::<String>();
            Ok((id, text))
        });

        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], (1, "Item 1".to_string()));
        assert_eq!(items[1], (2, "Item 2".to_string()));
        assert_eq!(items[2], (3, "Item 3".to_string()));
    }

    #[test]
    fn test_parse_item_list_empty() {
        let html = Html::parse_document(r#"<div class="container"></div>"#);

        let result = parse_item_list(&html, ".container", ".item", |element| {
            let text = element.text().collect::<String>();
            Ok(text)
        });

        assert!(result.is_ok());
        let items = result.unwrap();
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn test_parse_numeric_with_unit_error_cases() {
        let html = Html::parse_document(r#"<div class="value">abc</div>"#);

        let result = parse_numeric_with_unit(&html, ".value");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("failed to parse number"));

        // Test missing element
        let result2 = parse_numeric_with_unit(&html, ".nonexistent");
        assert!(result2.is_err());
        assert!(result2
            .unwrap_err()
            .to_string()
            .contains("element not found"));
    }
}
