//! Example demonstrating direct usage of parser traits in collectors.
//!
//! This module shows how new collectors can directly use the trait system
//! for improved flexibility and testability.

use anyhow::Result;
use scraper::Html;

use crate::aiseg::parser_adapters::ParserAdapterBuilder;
use crate::aiseg::parser_traits::{ContextualHtmlParser, HtmlParser};
use crate::model::{ClimateStatusMetric, PowerStatusBreakdownMetric};
use chrono::{DateTime, Local};

/// Example function showing direct trait usage for power parsing
pub fn parse_power_with_traits(html: &str) -> Result<Vec<PowerStatusBreakdownMetric>> {
    let document = Html::parse_document(html);
    
    // Create parser using builder pattern
    let parser = ParserAdapterBuilder::consumption_page();
    
    // Use trait method directly
    parser.parse(&document)
}

/// Example function showing contextual parser usage for climate data
pub fn parse_climate_with_traits(
    html: &str,
    timestamp: DateTime<Local>,
) -> Result<Vec<ClimateStatusMetric>> {
    let document = Html::parse_document(html);
    
    // Create parser using builder pattern
    let parser = ParserAdapterBuilder::climate_page();
    
    // Use contextual trait method
    parser.parse_with_context(&document, timestamp)
}

/// Example showing how to create custom parsers that implement the traits
pub struct CustomPowerParser {
    max_devices: usize,
}

impl CustomPowerParser {
    pub fn new(max_devices: usize) -> Self {
        Self { max_devices }
    }
}

impl HtmlParser for CustomPowerParser {
    type Output = Vec<String>;

    fn parse(&self, document: &Html) -> Result<Self::Output> {
        use crate::aiseg::html_parsing::parse_consumption_device;
        
        let mut device_names = Vec::new();
        
        for i in 1..=self.max_devices {
            let stage_id = format!("#stage_{}", i);
            match parse_consumption_device(document, &stage_id)? {
                Some((name, _watts)) => device_names.push(name),
                None => break,
            }
        }
        
        Ok(device_names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_custom_parser() {
        let html = r#"
            <html>
                <body>
                    <div id="stage_1">
                        <div class="c_device">Device 1</div>
                        <div class="c_value">100</div>
                    </div>
                    <div id="stage_2">
                        <div class="c_device">Device 2</div>
                        <div class="c_value">200</div>
                    </div>
                </body>
            </html>
        "#;

        let document = Html::parse_document(html);
        let parser = CustomPowerParser::new(5);
        let device_names = parser.parse(&document).unwrap();
        
        assert_eq!(device_names.len(), 2);
        assert_eq!(device_names[0], "Device 1");
        assert_eq!(device_names[1], "Device 2");
    }

    #[test]
    fn test_parse_power_with_traits() {
        let html = r#"
            <html>
                <body>
                    <div id="stage_1">
                        <div class="c_device">エアコン</div>
                        <div class="c_value">1500</div>
                    </div>
                </body>
            </html>
        "#;

        let metrics = parse_power_with_traits(html).unwrap();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].name, "エアコン(W)");
        assert_eq!(metrics[0].value, 1500);
    }

    #[test]
    fn test_parse_climate_with_traits() {
        let html = r#"
            <html>
                <body>
                    <div id="base1_1">
                        <div class="txt_name">リビング</div>
                        <div class="num_wrapper">
                            <span id="num_ond_1_1" class="num no2"></span>
                            <span id="num_ond_1_2" class="num no3"></span>
                            <span id="num_ond_1_3" class="num no5"></span>
                            <span id="num_shitudo_1_1" class="num no6"></span>
                            <span id="num_shitudo_1_2" class="num no5"></span>
                            <span id="num_shitudo_1_3" class="num no0"></span>
                        </div>
                    </div>
                </body>
            </html>
        "#;

        let timestamp = Local.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let metrics = parse_climate_with_traits(html, timestamp).unwrap();
        
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0].name, "リビング");
        assert_eq!(metrics[0].value, 23.5);
        assert_eq!(metrics[1].name, "リビング");
        assert_eq!(metrics[1].value, 65.0);
    }
}