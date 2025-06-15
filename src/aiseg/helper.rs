//! Helper functions for parsing AiSEG2 HTML responses and data conversion.
//! 
//! This module provides utility functions for:
//! - Parsing HTML content from AiSEG2 web pages
//! - Extracting numeric values from HTML elements
//! - Converting between different units (kW to W)
//! - Date/time manipulation

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local, NaiveTime};
use scraper::{Html, Selector};

/// Parses text content from an HTML element.
/// 
/// This function is used to extract text values from AiSEG2 HTML responses,
/// such as metric names and labels. It ensures the element has actual content
/// by checking for child nodes.
/// 
/// # Arguments
/// 
/// * `document` - The parsed HTML document
/// * `selector` - CSS selector string (e.g., "#h_title", ".meter-name")
/// 
/// # Returns
/// 
/// * `Ok(String)` - The concatenated text content from the element and its children
/// * `Err` - If the selector is invalid, no element matches, or the element has no children
/// 
/// # Example
/// 
/// ```no_run
/// let html = Html::parse_document(r#"<div id="h_title">太陽光発電量</div>"#);
/// let title = parse_text_from_html(&html, "#h_title")?;
/// assert_eq!(title, "太陽光発電量");
/// ```
pub fn parse_text_from_html(document: &Html, selector: &str) -> Result<String> {
    let selector = html_selector(selector)?;
    let element = document
        .select(&selector)
        .next()
        .context("Failed to find value")?;
    if !element.has_children() {
        return Err(anyhow!("Element has no children"));
    }
    Ok(element.text().collect::<String>())
}

/// Creates a CSS selector from a string.
/// 
/// This is a wrapper around scraper's Selector::parse that converts
/// parsing errors into anyhow errors for consistent error handling.
/// 
/// # Arguments
/// 
/// * `selector` - CSS selector string
/// 
/// # Returns
/// 
/// * `Ok(Selector)` - A parsed selector ready for use
/// * `Err` - If the selector syntax is invalid
/// 
/// # Examples
/// 
/// Valid selectors:
/// - `"#id"` - ID selector
/// - `".class"` - Class selector
/// - `"div > span"` - Complex selector
pub fn html_selector(selector: &str) -> Result<Selector> {
    match Selector::parse(selector) {
        Ok(s) => Ok(s),
        Err(e) => Err(anyhow!("Failed to parse selector: {}", e)),
    }
}

/// Parses a floating-point number from an HTML element.
/// 
/// This function extracts numeric values from HTML elements that may contain
/// units or other non-numeric characters. It filters out everything except
/// digits and decimal points before parsing.
/// 
/// # Arguments
/// 
/// * `document` - The parsed HTML document
/// * `selector` - CSS selector for the element containing the numeric value
/// 
/// # Returns
/// 
/// * `Ok(f64)` - The parsed numeric value
/// * `Err` - If the element is not found, has no text, or contains no valid number
/// 
/// # Example
/// 
/// ```no_run
/// // Parses "123.45" from "123.45kW"
/// let html = Html::parse_document(r#"<div id="val_kwh">123.45kW</div>"#);
/// let value = parse_f64_from_html(&html, "#val_kwh")?;
/// assert_eq!(value, 123.45);
/// ```
/// 
/// # Note
/// 
/// This function ignores negative signs, so "-123.45" becomes 123.45
pub fn parse_f64_from_html(document: &Html, selector: &str) -> Result<f64> {
    let selector = html_selector(selector)?;
    let element = document
        .select(&selector)
        .next()
        .context("Failed to find value")?;
    let inner_text = element.text().next().context("Failed to get text")?;
    inner_text
        .chars()
        .filter(|c| c.is_numeric() || c == &'.')
        .collect::<String>()
        .parse::<f64>()
        .context("Failed to parse value")
}

/// Normalizes a DateTime to the beginning of the day (00:00:00).
/// 
/// This function is used to ensure consistent date handling for daily metrics,
/// where the time component should always be midnight.
/// 
/// # Arguments
/// 
/// * `date` - Any DateTime in the local timezone
/// 
/// # Returns
/// 
/// The same date with time set to 00:00:00
/// 
/// # Example
/// 
/// ```no_run
/// let date = Local::now(); // e.g., 2024-06-15 14:30:45
/// let normalized = day_of_beginning(&date); // 2024-06-15 00:00:00
/// ```
/// 
/// # Safety
/// 
/// The unwrap is safe because with_time only fails if the resulting DateTime
/// would be out of range. Since we're setting to midnight (00:00:00) using
/// NaiveTime::default(), this is safe for all valid input dates.
pub fn day_of_beginning(date: &DateTime<Local>) -> DateTime<Local> {
    date.with_time(NaiveTime::default()).unwrap()
}

/// Truncates a floating-point number to an integer.
/// 
/// This function removes the decimal part without rounding.
/// Used for converting metric values that should be whole numbers.
/// 
/// # Arguments
/// 
/// * `value` - The floating-point value to truncate
/// 
/// # Returns
/// 
/// The integer part of the value
/// 
/// # Examples
/// 
/// ```
/// assert_eq!(truncate_to_i64(123.99), 123);
/// assert_eq!(truncate_to_i64(-123.99), -123);
/// ```
pub fn truncate_to_i64(value: f64) -> i64 {
    value.trunc() as i64
}

/// Converts kilowatts to watts, truncating to whole watts.
/// 
/// This function is used to convert power values from the AiSEG2 system
/// (which reports in kW) to watts for storage in InfluxDB.
/// 
/// # Arguments
/// 
/// * `kw` - Power value in kilowatts
/// 
/// # Returns
/// 
/// Power value in watts (truncated to integer)
/// 
/// # Example
/// 
/// ```
/// assert_eq!(kilowatts_to_watts(1.5), 1500);
/// assert_eq!(kilowatts_to_watts(2.345), 2345); // Truncates, not rounds
/// ```
pub fn kilowatts_to_watts(kw: f64) -> i64 {
    truncate_to_i64(kw * 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone, Timelike};

    mod succeeds {
        use super::*;

        #[test]
        fn test_parse_text_from_html_valid_element() {
            let html = Html::parse_document(r#"<div class="test">Hello World</div>"#);
            let result = parse_text_from_html(&html, ".test");

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "Hello World");
        }

        #[test]
        fn test_parse_text_from_html_with_nested_elements() {
            let html = Html::parse_document(
                r#"<div class="test"><span>Hello</span> <span>World</span></div>"#,
            );
            let result = parse_text_from_html(&html, ".test");

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "Hello World");
        }

        #[test]
        fn test_parse_text_from_html_with_whitespace() {
            let html = Html::parse_document(r#"<div class="test">  Hello   World  </div>"#);
            let result = parse_text_from_html(&html, ".test");

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "  Hello   World  ");
        }

        #[test]
        fn test_html_selector_valid_class() {
            let result = html_selector(".test-class");
            assert!(result.is_ok());
        }

        #[test]
        fn test_html_selector_valid_id() {
            let result = html_selector("#test-id");
            assert!(result.is_ok());
        }

        #[test]
        fn test_html_selector_valid_element() {
            let result = html_selector("div");
            assert!(result.is_ok());
        }

        #[test]
        fn test_html_selector_complex_selector() {
            let result = html_selector("div.class1.class2 > span#id1");
            assert!(result.is_ok());
        }

        #[test]
        fn test_parse_f64_from_html_valid_number() {
            let html = Html::parse_document(r#"<div class="value">123.45</div>"#);
            let result = parse_f64_from_html(&html, ".value");

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 123.45);
        }

        #[test]
        fn test_parse_f64_from_html_with_units() {
            let html = Html::parse_document(r#"<div class="value">123.45kW</div>"#);
            let result = parse_f64_from_html(&html, ".value");

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 123.45);
        }

        #[test]
        fn test_parse_f64_from_html_with_special_chars() {
            let html = Html::parse_document(r#"<div class="value">¥123.45円</div>"#);
            let result = parse_f64_from_html(&html, ".value");

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 123.45);
        }

        #[test]
        fn test_parse_f64_from_html_integer() {
            let html = Html::parse_document(r#"<div class="value">123</div>"#);
            let result = parse_f64_from_html(&html, ".value");

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 123.0);
        }

        #[test]
        fn test_parse_f64_from_html_zero() {
            let html = Html::parse_document(r#"<div class="value">0</div>"#);
            let result = parse_f64_from_html(&html, ".value");

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 0.0);
        }

        #[test]
        fn test_parse_f64_from_html_negative() {
            let html = Html::parse_document(r#"<div class="value">-123.45</div>"#);
            let result = parse_f64_from_html(&html, ".value");

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 123.45); // Note: negative sign is filtered out
        }

        #[test]
        fn test_day_of_beginning() {
            let date = Local.with_ymd_and_hms(2023, 12, 25, 15, 30, 45).unwrap();
            let result = day_of_beginning(&date);

            assert_eq!(result.year(), 2023);
            assert_eq!(result.month(), 12);
            assert_eq!(result.day(), 25);
            assert_eq!(result.hour(), 0);
            assert_eq!(result.minute(), 0);
            assert_eq!(result.second(), 0);
        }

        #[test]
        fn test_day_of_beginning_already_midnight() {
            let date = Local.with_ymd_and_hms(2023, 12, 25, 0, 0, 0).unwrap();
            let result = day_of_beginning(&date);

            assert_eq!(result, date);
        }

        #[test]
        fn test_truncate_to_i64_positive() {
            assert_eq!(truncate_to_i64(123.45), 123);
            assert_eq!(truncate_to_i64(123.99), 123);
            assert_eq!(truncate_to_i64(123.0), 123);
        }

        #[test]
        fn test_truncate_to_i64_negative() {
            assert_eq!(truncate_to_i64(-123.45), -123);
            assert_eq!(truncate_to_i64(-123.99), -123);
            assert_eq!(truncate_to_i64(-123.0), -123);
        }

        #[test]
        fn test_truncate_to_i64_zero() {
            assert_eq!(truncate_to_i64(0.0), 0);
            assert_eq!(truncate_to_i64(0.9), 0);
            assert_eq!(truncate_to_i64(-0.9), 0);
        }

        #[test]
        fn test_kilowatts_to_watts_positive() {
            assert_eq!(kilowatts_to_watts(1.5), 1500);
            assert_eq!(kilowatts_to_watts(2.345), 2345);
            assert_eq!(kilowatts_to_watts(0.5), 500);
        }

        #[test]
        fn test_kilowatts_to_watts_negative() {
            assert_eq!(kilowatts_to_watts(-1.5), -1500);
            assert_eq!(kilowatts_to_watts(-2.345), -2345);
        }

        #[test]
        fn test_kilowatts_to_watts_zero() {
            assert_eq!(kilowatts_to_watts(0.0), 0);
        }

        #[test]
        fn test_kilowatts_to_watts_small_values() {
            assert_eq!(kilowatts_to_watts(0.001), 1);
            assert_eq!(kilowatts_to_watts(0.0001), 0);
        }
    }

    mod fails {
        use super::*;

        #[test]
        fn test_parse_text_from_html_element_not_found() {
            let html = Html::parse_document(r#"<div class="test">Hello World</div>"#);
            let result = parse_text_from_html(&html, ".nonexistent");

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to find value"));
        }

        #[test]
        fn test_parse_text_from_html_no_children() {
            let html = Html::parse_document(r#"<div class="test"></div>"#);
            let result = parse_text_from_html(&html, ".test");

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Element has no children"));
        }

        #[test]
        fn test_parse_text_from_html_invalid_selector() {
            let html = Html::parse_document(r#"<div class="test">Hello World</div>"#);
            let result = parse_text_from_html(&html, ":::invalid");

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to parse selector"));
        }

        #[test]
        fn test_html_selector_invalid_syntax() {
            let result = html_selector(":::invalid");
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to parse selector"));
        }

        #[test]
        fn test_html_selector_empty_string() {
            let result = html_selector("");
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to parse selector"));
        }

        #[test]
        fn test_parse_f64_from_html_element_not_found() {
            let html = Html::parse_document(r#"<div class="value">123.45</div>"#);
            let result = parse_f64_from_html(&html, ".nonexistent");

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to find value"));
        }

        #[test]
        fn test_parse_f64_from_html_invalid_selector() {
            let html = Html::parse_document(r#"<div class="value">123.45</div>"#);
            let result = parse_f64_from_html(&html, ":::invalid");

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to parse selector"));
        }

        #[test]
        fn test_parse_f64_from_html_no_text() {
            let html = Html::parse_document(r#"<div class="value"></div>"#);
            let result = parse_f64_from_html(&html, ".value");

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to get text"));
        }

        #[test]
        fn test_parse_f64_from_html_no_numeric_content() {
            let html = Html::parse_document(r#"<div class="value">abc</div>"#);
            let result = parse_f64_from_html(&html, ".value");

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to parse value"));
        }

        #[test]
        fn test_parse_f64_from_html_only_special_chars() {
            let html = Html::parse_document(r#"<div class="value">¥円$</div>"#);
            let result = parse_f64_from_html(&html, ".value");

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to parse value"));
        }

        #[test]
        fn test_parse_f64_from_html_multiple_dots() {
            let html = Html::parse_document(r#"<div class="value">123.45.67</div>"#);
            let result = parse_f64_from_html(&html, ".value");

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Failed to parse value"));
        }
    }
}
