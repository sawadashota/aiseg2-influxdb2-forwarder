use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local, NaiveTime};
use scraper::{Html, Selector};

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

pub fn html_selector(selector: &str) -> Result<Selector> {
    match Selector::parse(selector) {
        Ok(s) => Ok(s),
        Err(e) => Err(anyhow!("Failed to parse selector: {}", e)),
    }
}

pub fn parse_f64_from_html(document: &Html, selector: &str) -> Result<f64> {
    let selector = html_selector(selector)?;
    let element = document
        .select(&selector)
        .next()
        .context("Failed to find value")?;
    let inner_text = element.text().next().context("Failed to get text")?;
    Ok(inner_text
        .chars()
        .filter(|c| c.is_numeric() || c == &'.')
        .collect::<String>()
        .parse::<f64>()
        .context("Failed to parse value")?)
}

pub fn day_of_beginning(date: &DateTime<Local>) -> DateTime<Local> {
    date.with_time(NaiveTime::default()).unwrap()
}

pub fn f64_to_i64(kw: f64) -> i64 {
    kw.trunc() as i64
}

pub fn f64_kw_to_i64_watt(kw: f64) -> i64 {
    f64_to_i64(kw * 1000.0)
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
        fn test_f64_to_i64_positive() {
            assert_eq!(f64_to_i64(123.45), 123);
            assert_eq!(f64_to_i64(123.99), 123);
            assert_eq!(f64_to_i64(123.0), 123);
        }

        #[test]
        fn test_f64_to_i64_negative() {
            assert_eq!(f64_to_i64(-123.45), -123);
            assert_eq!(f64_to_i64(-123.99), -123);
            assert_eq!(f64_to_i64(-123.0), -123);
        }

        #[test]
        fn test_f64_to_i64_zero() {
            assert_eq!(f64_to_i64(0.0), 0);
            assert_eq!(f64_to_i64(0.9), 0);
            assert_eq!(f64_to_i64(-0.9), 0);
        }

        #[test]
        fn test_f64_kw_to_i64_watt_positive() {
            assert_eq!(f64_kw_to_i64_watt(1.5), 1500);
            assert_eq!(f64_kw_to_i64_watt(2.345), 2345);
            assert_eq!(f64_kw_to_i64_watt(0.5), 500);
        }

        #[test]
        fn test_f64_kw_to_i64_watt_negative() {
            assert_eq!(f64_kw_to_i64_watt(-1.5), -1500);
            assert_eq!(f64_kw_to_i64_watt(-2.345), -2345);
        }

        #[test]
        fn test_f64_kw_to_i64_watt_zero() {
            assert_eq!(f64_kw_to_i64_watt(0.0), 0);
        }

        #[test]
        fn test_f64_kw_to_i64_watt_small_values() {
            assert_eq!(f64_kw_to_i64_watt(0.001), 1);
            assert_eq!(f64_kw_to_i64_watt(0.0001), 0);
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
