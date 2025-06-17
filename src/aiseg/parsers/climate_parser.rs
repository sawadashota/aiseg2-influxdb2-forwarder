//! HTML parsing for AiSEG2 climate pages.

#[cfg(test)]
use crate::error::ParseError;
use crate::error::{AisegError, Result};
use chrono::{DateTime, Local};
use scraper::Html;

use crate::aiseg::parser_adapters::ParserAdapterBuilder;
use crate::aiseg::parser_traits::ContextualHtmlParser;
use crate::model::ClimateStatusMetric;

#[cfg(test)]
use crate::aiseg::helper::html_selector;
#[cfg(test)]
use crate::aiseg::html_parsing::extract_numeric_from_digit_elements;
#[cfg(test)]
use crate::aiseg::metrics::climate::create_climate_metrics;

/// Parses climate data from a specific base element.
///
/// # Arguments
/// * `document` - The parsed HTML document
/// * `base_id` - The base element ID (e.g., "#base1_1", "#base2_1")
/// * `timestamp` - Timestamp for the metrics
///
/// # Returns
/// Array of [temperature, humidity] metrics for the location
#[cfg(test)]
fn parse_climate_location(
    document: &Html,
    base_id: &str,
    timestamp: DateTime<Local>,
) -> Result<[ClimateStatusMetric; 2], ParseError> {
    let base_selector = html_selector(base_id)?;
    let base_element = document
        .select(&base_selector)
        .next()
        .ok_or_else(|| ParseError::element_not_found(base_id))?;

    // Extract location name
    let name_selector = html_selector(".txt_name")?;
    let name = base_element
        .select(&name_selector)
        .next()
        .ok_or_else(|| ParseError::element_not_found(".txt_name"))?
        .text()
        .next()
        .ok_or_else(|| ParseError::EmptyElement {
            selector: ".txt_name".to_string(),
        })?
        .to_string();

    // Find num_wrapper element
    let num_wrapper_selector = html_selector(".num_wrapper")?;
    let num_wrapper = base_element
        .select(&num_wrapper_selector)
        .next()
        .ok_or_else(|| ParseError::element_not_found(".num_wrapper"))?;

    // Extract temperature
    let temp_selector = html_selector(r#"[id^="num_ond_"][class*="num no"]"#)?;
    let temperature = extract_numeric_from_digit_elements(num_wrapper.select(&temp_selector))?;

    // Extract humidity
    let humidity_selector = html_selector(r#"[id^="num_shitudo_"][class*="num no"]"#)?;
    let humidity = extract_numeric_from_digit_elements(num_wrapper.select(&humidity_selector))?;

    Ok(create_climate_metrics(
        name,
        temperature,
        humidity,
        timestamp,
    ))
}

/// Parses all climate locations from a page.
///
/// # Arguments
/// * `document` - Parsed HTML document from `/page/airenvironment/41?page=X`
/// * `timestamp` - Timestamp for all metrics
///
/// # Returns
/// Vector of climate metrics for all locations found on the page
pub fn parse_climate_page(
    document: &Html,
    timestamp: DateTime<Local>,
) -> Result<Vec<ClimateStatusMetric>, AisegError> {
    // Use trait-based parser adapter
    let parser = ParserAdapterBuilder::climate_page();
    parser.parse_with_context(document, timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn create_climate_html(locations: Vec<(&str, &str, &str)>) -> String {
        let mut html = r#"<html><body>"#.to_string();

        for (i, (name, temp_digits, humidity_digits)) in locations.iter().enumerate() {
            let base_num = i + 1;
            html.push_str(&format!(
                r#"
                <div id="base{}_1">
                    <div class="txt_name">{}</div>
                    <div class="num_wrapper">
                        <span id="num_ond_{}_1" class="num no{}"></span>
                        <span id="num_ond_{}_2" class="num no{}"></span>
                        <span id="num_ond_{}_3" class="num no{}"></span>
                        <span id="num_shitudo_{}_1" class="num no{}"></span>
                        <span id="num_shitudo_{}_2" class="num no{}"></span>
                        <span id="num_shitudo_{}_3" class="num no{}"></span>
                    </div>
                </div>"#,
                base_num,
                name,
                base_num,
                temp_digits.chars().next().unwrap(),
                base_num,
                temp_digits.chars().nth(1).unwrap(),
                base_num,
                temp_digits.chars().nth(2).unwrap(),
                base_num,
                humidity_digits.chars().next().unwrap(),
                base_num,
                humidity_digits.chars().nth(1).unwrap(),
                base_num,
                humidity_digits.chars().nth(2).unwrap(),
            ));
        }

        html.push_str("</body></html>");
        html
    }

    #[test]
    fn test_parse_climate_location() {
        let html = Html::parse_document(&create_climate_html(vec![("リビング", "235", "650")]));

        let timestamp = Local.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let result = parse_climate_location(&html, "#base1_1", timestamp);

        assert!(result.is_ok());
        let metrics = result.unwrap();

        assert_eq!(metrics[0].name, "リビング");
        assert_eq!(metrics[0].value, 23.5);
        assert_eq!(
            metrics[0].category,
            crate::model::ClimateStatusMetricCategory::Temperature
        );

        assert_eq!(metrics[1].name, "リビング");
        assert_eq!(metrics[1].value, 65.0);
        assert_eq!(
            metrics[1].category,
            crate::model::ClimateStatusMetricCategory::Humidity
        );
    }

    #[test]
    fn test_parse_climate_page() {
        let html = Html::parse_document(&create_climate_html(vec![
            ("リビング", "235", "650"),
            ("寝室", "210", "550"),
            ("子供部屋", "225", "600"),
        ]));

        let timestamp = Local.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let result = parse_climate_page(&html, timestamp);

        assert!(result.is_ok());
        let metrics = result.unwrap();

        assert_eq!(metrics.len(), 6); // 3 locations × 2 metrics each

        // Check first location
        assert_eq!(metrics[0].name, "リビング");
        assert_eq!(metrics[0].value, 23.5);
        assert_eq!(metrics[1].name, "リビング");
        assert_eq!(metrics[1].value, 65.0);

        // Check second location
        assert_eq!(metrics[2].name, "寝室");
        assert_eq!(metrics[2].value, 21.0);
        assert_eq!(metrics[3].name, "寝室");
        assert_eq!(metrics[3].value, 55.0);
    }

    #[test]
    fn test_parse_climate_page_partial() {
        let html = Html::parse_document(&create_climate_html(vec![("リビング", "235", "650")]));

        let timestamp = Local.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let result = parse_climate_page(&html, timestamp);

        assert!(result.is_ok());
        let metrics = result.unwrap();

        assert_eq!(metrics.len(), 2); // 1 location × 2 metrics
    }
}
