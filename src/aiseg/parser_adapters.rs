//! Adapters that wrap existing parser functions to implement the unified trait system.

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Local};
use scraper::Html;

use crate::aiseg::parser_traits::{ContextualHtmlParser, HtmlParser};
use crate::aiseg::parsers::climate_parser::parse_climate_page;
use crate::aiseg::parsers::power_parser::{
    parse_consumption_page, parse_generation_sources, parse_total_power,
};
use crate::model::{ClimateStatusMetric, PowerStatusBreakdownMetric};

/// Adapter for the total power parser.
pub struct TotalPowerParserAdapter;

impl HtmlParser for TotalPowerParserAdapter {
    type Output = (f64, f64);

    fn parse(&self, document: &Html) -> Result<Self::Output> {
        parse_total_power(document)
    }
}

/// Adapter for the generation sources parser.
pub struct GenerationSourcesParserAdapter;

impl HtmlParser for GenerationSourcesParserAdapter {
    type Output = Vec<(String, f64)>;

    fn parse(&self, document: &Html) -> Result<Self::Output> {
        parse_generation_sources(document)
    }
}

/// Adapter for the consumption page parser.
pub struct ConsumptionPageParserAdapter;

impl HtmlParser for ConsumptionPageParserAdapter {
    type Output = Vec<PowerStatusBreakdownMetric>;

    fn parse(&self, document: &Html) -> Result<Self::Output> {
        parse_consumption_page(document)
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
        parse_climate_page(document, timestamp)
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
