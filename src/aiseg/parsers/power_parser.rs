//! HTML parsing for AiSEG2 power pages.

use anyhow::Result;
use scraper::Html;

use crate::aiseg::helper::{parse_f64_from_html, truncate_to_i64};
use crate::aiseg::html_parsing::{parse_consumption_device, parse_generation_details};
use crate::model::PowerStatusBreakdownMetric;

/// Parses total power metrics from the main electricity flow page.
///
/// # Arguments
/// * `document` - Parsed HTML document from `/page/electricflow/111`
///
/// # Returns
/// Tuple of (generation_kw, consumption_kw)
pub fn parse_total_power(document: &Html) -> Result<(f64, f64)> {
    let generation = parse_f64_from_html(document, "#g_capacity")?;
    let consumption = parse_f64_from_html(document, "#u_capacity")?;
    Ok((generation, consumption))
}

/// Parses generation source details from the main page.
///
/// # Arguments
/// * `document` - Parsed HTML document
///
/// # Returns
/// Vector of (source_name, value_in_watts) tuples
pub fn parse_generation_sources(document: &Html) -> Result<Vec<(String, f64)>> {
    let details = parse_generation_details(document, 4)?;
    Ok(details
        .into_iter()
        .map(|(name, kw)| (name, kw * 1000.0))
        .collect())
}

/// Parses a consumption detail page.
///
/// # Arguments
/// * `document` - Parsed HTML document from `/page/electricflow/1113?id=X`
///
/// # Returns
/// Vector of consumption metrics found on the page
pub fn parse_consumption_page(document: &Html) -> Result<Vec<PowerStatusBreakdownMetric>> {
    let mut items = Vec::new();

    for i in 1..=10 {
        let stage_id = format!("#stage_{}", i);
        match parse_consumption_device(document, &stage_id)? {
            Some((name, watts)) => {
                items.push(PowerStatusBreakdownMetric {
                    measurement: crate::model::Measurement::Power,
                    category: crate::model::PowerStatusBreakdownMetricCategory::Consumption,
                    name: format!("{}({})", name, crate::model::Unit::Watt),
                    value: truncate_to_i64(watts),
                });
            }
            None => break,
        }
    }

    Ok(items)
}

/// Checks if two consumption pages have the same device names.
///
/// Used to detect when pagination has wrapped around.
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

