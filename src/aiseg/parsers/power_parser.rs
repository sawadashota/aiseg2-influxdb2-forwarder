//! HTML parsing for AiSEG2 power pages.

use anyhow::Result;
use scraper::Html;

use crate::aiseg::parser_adapters::ParserAdapterBuilder;
use crate::aiseg::parser_traits::HtmlParser;
use crate::model::PowerStatusBreakdownMetric;

/// Parses total power metrics from the main electricity flow page.
///
/// # Arguments
/// * `document` - Parsed HTML document from `/page/electricflow/111`
///
/// # Returns
/// Tuple of (generation_kw, consumption_kw)
pub fn parse_total_power(document: &Html) -> Result<(f64, f64)> {
    // Use trait-based parser adapter
    let parser = ParserAdapterBuilder::total_power();
    parser.parse(document)
}

/// Parses generation source details from the main page.
///
/// # Arguments
/// * `document` - Parsed HTML document
///
/// # Returns
/// Vector of (source_name, value_in_watts) tuples
pub fn parse_generation_sources(document: &Html) -> Result<Vec<(String, f64)>> {
    // Use trait-based parser adapter
    let parser = ParserAdapterBuilder::generation_sources();
    parser.parse(document)
}

/// Parses a consumption detail page.
///
/// # Arguments
/// * `document` - Parsed HTML document from `/page/electricflow/1113?id=X`
///
/// # Returns
/// Vector of consumption metrics found on the page
pub fn parse_consumption_page(document: &Html) -> Result<Vec<PowerStatusBreakdownMetric>> {
    // Use trait-based parser adapter
    let parser = ParserAdapterBuilder::consumption_page();
    parser.parse(document)
}

/// Checks if two consumption pages have the same device names.
///
/// Used to detect when pagination has wrapped around.
#[allow(dead_code)]
pub fn has_duplicate_device_names(
    previous: &[PowerStatusBreakdownMetric],
    current: &[PowerStatusBreakdownMetric],
) -> bool {
    if previous.is_empty() || current.is_empty() {
        return false;
    }

    let prev_names: Vec<&str> = previous.iter().map(|m| m.name.as_str()).collect();
    let curr_names: Vec<&str> = current.iter().map(|m| m.name.as_str()).collect();

    prev_names == curr_names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_html(content: &str) -> Html {
        Html::parse_document(&format!(r#"<html><body>{}</body></html>"#, content))
    }

    #[test]
    fn test_parse_total_power() {
        let html = create_test_html(
            r#"<div id="g_capacity">2.5</div>
               <div id="u_capacity">3.8</div>"#,
        );

        let result = parse_total_power(&html);
        assert!(result.is_ok());

        let (gen, cons) = result.unwrap();
        assert_eq!(gen, 2.5);
        assert_eq!(cons, 3.8);
    }

    #[test]
    fn test_parse_generation_sources() {
        let html = create_test_html(
            r#"<div id="g_d_1_title"><span>太陽光</span></div>
               <div id="g_d_1_capacity"><span>2.5</span></div>
               <div id="g_d_2_title"><span>燃料電池</span></div>
               <div id="g_d_2_capacity"><span>0.5</span></div>"#,
        );

        let result = parse_generation_sources(&html);
        assert!(result.is_ok());

        let sources = result.unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0], ("太陽光".to_string(), 2500.0));
        assert_eq!(sources[1], ("燃料電池".to_string(), 500.0));
    }

    #[test]
    fn test_has_duplicate_device_names() {
        let metrics1 = vec![
            PowerStatusBreakdownMetric {
                measurement: crate::model::Measurement::Power,
                category: crate::model::PowerStatusBreakdownMetricCategory::Consumption,
                name: "Device1(W)".to_string(),
                value: 100,
            },
            PowerStatusBreakdownMetric {
                measurement: crate::model::Measurement::Power,
                category: crate::model::PowerStatusBreakdownMetricCategory::Consumption,
                name: "Device2(W)".to_string(),
                value: 200,
            },
        ];

        let metrics2 = metrics1.clone();
        let metrics3 = vec![PowerStatusBreakdownMetric {
            measurement: crate::model::Measurement::Power,
            category: crate::model::PowerStatusBreakdownMetricCategory::Consumption,
            name: "Device3(W)".to_string(),
            value: 300,
        }];

        assert!(has_duplicate_device_names(&metrics1, &metrics2));
        assert!(!has_duplicate_device_names(&metrics1, &metrics3));
        assert!(!has_duplicate_device_names(&[], &metrics1));
    }
}
