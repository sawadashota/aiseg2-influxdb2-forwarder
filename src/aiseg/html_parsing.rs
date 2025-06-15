//! Shared HTML parsing utilities for AiSEG2 collectors.
//!
//! This module provides common HTML parsing functions used across different collectors
//! to reduce code duplication and ensure consistent parsing behavior.

use anyhow::{Context, Result};
use scraper::{ElementRef, Html};

use crate::aiseg::helper::{parse_f64_from_html, parse_text_from_html};

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
pub fn extract_numeric_from_digit_elements<'a, I>(elements: I) -> Result<f64>
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
        .context("Failed to parse numeric value from digit elements")
}

/// Parses a single consumption device entry from HTML.
///
/// # Arguments
/// * `document` - The parsed HTML document
/// * `stage_id` - The stage element ID (e.g., "#stage_1")
///
/// # Returns
/// A tuple of (device_name, power_value) or None if the element doesn't exist
pub fn parse_consumption_device(document: &Html, stage_id: &str) -> Result<Option<(String, f64)>> {
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
pub fn parse_generation_details(document: &Html, max_items: usize) -> Result<Vec<(String, f64)>> {
    let mut results = Vec::new();

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

/// Generic pagination handler for collectors.
///
/// # Arguments
/// * `max_pages` - Maximum number of pages to iterate
/// * `fetch_page` - Async function to fetch a page by number
/// * `parse_page` - Function to parse items from a page
/// * `should_continue` - Function to determine if pagination should continue
///
/// # Returns
/// All items collected from all pages
#[allow(dead_code)]
pub async fn paginate_collection<T, F, P, C>(
    max_pages: usize,
    mut fetch_page: F,
    parse_page: P,
    should_continue: C,
) -> Result<Vec<T>>
where
    F: FnMut(usize) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send>>,
    P: Fn(&Html) -> Result<Vec<T>>,
    C: Fn(&[T], &[T]) -> bool,
    T: Clone,
{
    let mut all_items = Vec::new();
    let mut last_page_items = Vec::new();

    for page in 1..=max_pages {
        let response = fetch_page(page).await?;
        let document = Html::parse_document(&response);
        let page_items = parse_page(&document)?;

        if !should_continue(&last_page_items, &page_items) {
            break;
        }

        last_page_items = page_items.clone();
        all_items.extend(page_items);
    }

    Ok(all_items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aiseg::helper::html_selector;

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
