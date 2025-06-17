//! Adapters that wrap existing parser functions to implement the unified trait system.

use anyhow::Result;
use chrono::{DateTime, Local};
use scraper::Html;

use crate::aiseg::helper::{html_selector, parse_f64_from_html, truncate_to_i64};
use crate::aiseg::html_parsing::{
    extract_numeric_from_digit_elements, parse_consumption_device, parse_generation_details,
};
use crate::aiseg::metrics::climate::create_climate_metrics;
use crate::aiseg::parser_traits::{ContextualHtmlParser, HtmlParser};
use crate::model::{
    ClimateStatusMetric, Measurement, PowerStatusBreakdownMetric,
    PowerStatusBreakdownMetricCategory,
};

/// Adapter for the total power parser.
pub struct TotalPowerParserAdapter;

impl HtmlParser for TotalPowerParserAdapter {
    type Output = (f64, f64);

    fn parse(&self, document: &Html) -> Result<Self::Output> {
        let generation = parse_f64_from_html(document, "#g_capacity")?;
        let consumption = parse_f64_from_html(document, "#u_capacity")?;
        Ok((generation, consumption))
    }
}

/// Adapter for the generation sources parser.
pub struct GenerationSourcesParserAdapter;

impl HtmlParser for GenerationSourcesParserAdapter {
    type Output = Vec<(String, f64)>;

    fn parse(&self, document: &Html) -> Result<Self::Output> {
        let details = parse_generation_details(document, 4)?;
        Ok(details
            .into_iter()
            .map(|(name, kw)| (name, kw * 1000.0))
            .collect())
    }
}

/// Adapter for the consumption page parser.
pub struct ConsumptionPageParserAdapter;

impl HtmlParser for ConsumptionPageParserAdapter {
    type Output = Vec<PowerStatusBreakdownMetric>;

    fn parse(&self, document: &Html) -> Result<Self::Output> {
        let mut items = Vec::new();

        for i in 1..=10 {
            let stage_id = format!("#stage_{}", i);
            match parse_consumption_device(document, &stage_id)? {
                Some((name, watts)) => {
                    items.push(PowerStatusBreakdownMetric {
                        measurement: Measurement::Power,
                        category: PowerStatusBreakdownMetricCategory::Consumption,
                        name: format!("{}(W)", name),
                        value: truncate_to_i64(watts),
                    });
                }
                None => break,
            }
        }

        Ok(items)
    }
}

/// Adapter for the climate page parser.
pub struct ClimatePageParserAdapter;

impl ContextualHtmlParser for ClimatePageParserAdapter {
    type Output = Vec<ClimateStatusMetric>;
    type Context = DateTime<Local>;

    fn parse_with_context(
        &self,
        document: &Html,
        timestamp: Self::Context,
    ) -> Result<Self::Output> {
        let mut metrics = Vec::new();

        for i in 1..=3 {
            let base_id = format!("#base{}_1", i);
            match self.parse_climate_location(document, &base_id, timestamp) {
                Ok(location_metrics) => {
                    metrics.extend(location_metrics);
                }
                Err(_) => break, // No more locations on this page
            }
        }

        Ok(metrics)
    }
}

impl ClimatePageParserAdapter {
    fn parse_climate_location(
        &self,
        document: &Html,
        base_id: &str,
        timestamp: DateTime<Local>,
    ) -> Result<[ClimateStatusMetric; 2]> {
        use anyhow::Context;

        let base_selector = html_selector(base_id)?;
        let base_element = document
            .select(&base_selector)
            .next()
            .context("Failed to find base element")?;

        // Extract location name
        let name_selector = html_selector(".txt_name")?;
        let name = base_element
            .select(&name_selector)
            .next()
            .context("Failed to find name")?
            .text()
            .next()
            .context("Failed to get text")?
            .to_string();

        // Find num_wrapper element
        let num_wrapper_selector = html_selector(".num_wrapper")?;
        let num_wrapper = base_element
            .select(&num_wrapper_selector)
            .next()
            .context("Failed to find num_wrapper")?;

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
}

/// Builder for creating parser adapters with consistent configuration.
pub struct ParserAdapterBuilder;

impl ParserAdapterBuilder {
    /// Create a total power parser adapter.
    pub fn total_power() -> TotalPowerParserAdapter {
        TotalPowerParserAdapter
    }

    /// Create a generation sources parser adapter.
    pub fn generation_sources() -> GenerationSourcesParserAdapter {
        GenerationSourcesParserAdapter
    }

    /// Create a consumption page parser adapter.
    pub fn consumption_page() -> ConsumptionPageParserAdapter {
        ConsumptionPageParserAdapter
    }

    /// Create a climate page parser adapter.
    pub fn climate_page() -> ClimatePageParserAdapter {
        ClimatePageParserAdapter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_total_power_adapter() {
        let html = Html::parse_document(
            r#"<html><body>
                <div id="g_capacity">2.5</div>
                <div id="u_capacity">3.8</div>
            </body></html>"#,
        );

        let parser = ParserAdapterBuilder::total_power();
        let result = parser.parse(&html).unwrap();
        assert_eq!(result, (2.5, 3.8));
    }

    #[test]
    fn test_generation_sources_adapter() {
        let html = Html::parse_document(
            r#"<html><body>
                <div id="g_d_1_title"><span>Solar</span></div>
                <div id="g_d_1_capacity"><span>2.5</span></div>
            </body></html>"#,
        );

        let parser = ParserAdapterBuilder::generation_sources();
        let result = parser.parse(&html).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], ("Solar".to_string(), 2500.0));
    }
}
