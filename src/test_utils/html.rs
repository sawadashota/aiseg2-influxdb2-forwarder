//! HTML generation utilities for testing.
//!
//! This module provides builders and helper functions for creating HTML responses
//! used in testing various collectors and parsers.

use scraper::Html;

/// Builder for creating HTML test documents with a fluent API.
#[derive(Debug, Default)]
pub struct HtmlTestBuilder {
    elements: Vec<(String, String)>,
    wrapper_tag: String,
}

impl HtmlTestBuilder {
    /// Creates a new HtmlTestBuilder.
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            wrapper_tag: "body".to_string(),
        }
    }

    /// Adds an element with the specified id and value.
    pub fn add_element(mut self, id: &str, value: &str) -> Self {
        self.elements.push((id.to_string(), value.to_string()));
        self
    }

    /// Adds an element with id and class.
    pub fn add_element_with_class(mut self, id: &str, class: &str, value: &str) -> Self {
        let element = format!(r#"<div id="{}" class="{}">{}</div>"#, id, class, value);
        self.elements.push(("custom".to_string(), element));
        self
    }

    /// Sets the wrapper tag (default is "body").
    pub fn with_wrapper(mut self, tag: &str) -> Self {
        self.wrapper_tag = tag.to_string();
        self
    }

    /// Builds the HTML string.
    pub fn build(self) -> String {
        let mut content = String::new();

        for (id, value) in self.elements {
            if id == "custom" {
                content.push_str(&value);
            } else {
                content.push_str(&format!(r#"<div id="{}">{}</div>"#, id, value));
            }
            content.push('\n');
        }

        format!(
            r#"<html><{}>{}</{}></html>"#,
            self.wrapper_tag, content, self.wrapper_tag
        )
    }

    /// Builds and parses the HTML document.
    pub fn build_document(self) -> Html {
        Html::parse_document(&self.build())
    }
}

/// Creates a standard HTML document wrapper for test content.
pub fn create_html_document(content: &str) -> Html {
    Html::parse_document(&format!(r#"<html><body>{}</body></html>"#, content))
}

/// Creates a simple HTML response with a title and value element.
/// Commonly used for daily total collectors.
pub fn create_title_value_html(title: &str, value: &str) -> String {
    HtmlTestBuilder::new()
        .add_element("h_title", title)
        .add_element("val_kwh", value)
        .build()
}

/// Creates an HTML response with only a value element.
/// Commonly used for circuit daily total collectors.
pub fn create_value_only_html(value: &str) -> String {
    HtmlTestBuilder::new().add_element("val_kwh", value).build()
}

/// Creates an HTML response for power generation/consumption display.
pub fn create_power_flow_html(generation: &str, consumption: &str) -> String {
    HtmlTestBuilder::new()
        .add_element("g_capacity", generation)
        .add_element("u_capacity", consumption)
        .build()
}

/// Builder for generation details HTML with multiple sources.
pub struct GenerationDetailsBuilder {
    sources: Vec<(usize, String, String)>,
}

impl GenerationDetailsBuilder {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    pub fn add_source(mut self, index: usize, title: &str, capacity: &str) -> Self {
        self.sources
            .push((index, title.to_string(), capacity.to_string()));
        self
    }

    pub fn build(self) -> String {
        let mut html = String::from(r#"<html><body>"#);

        for (index, title, capacity) in self.sources {
            html.push_str(&format!(
                r#"<div id="g_d_{}_title"><span>{}</span></div>
                   <div id="g_d_{}_capacity"><span>{}</span></div>"#,
                index, title, index, capacity
            ));
        }

        html.push_str("</body></html>");
        html
    }
}

/// Builder for consumption devices HTML.
pub struct ConsumptionDevicesBuilder {
    devices: Vec<(usize, String, String)>,
}

impl ConsumptionDevicesBuilder {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn add_device(mut self, index: usize, device: &str, value: &str) -> Self {
        self.devices
            .push((index, device.to_string(), value.to_string()));
        self
    }

    pub fn build(self) -> String {
        let mut html = String::from(r#"<html><body>"#);

        for (index, device, value) in self.devices {
            html.push_str(&format!(
                r#"<div id="stage_{}">
                    <div class="c_device"><span>{}</span></div>
                    <div class="c_value"><span>{}</span></div>
                </div>"#,
                index, device, value
            ));
        }

        html.push_str("</body></html>");
        html
    }
}

/// Builder for climate HTML with digit-based display.
pub struct ClimateHtmlBuilder {
    locations: Vec<(String, String, String)>,
}

impl ClimateHtmlBuilder {
    pub fn new() -> Self {
        Self {
            locations: Vec::new(),
        }
    }

    pub fn add_location(mut self, name: &str, temp_digits: &str, humidity_digits: &str) -> Self {
        self.locations.push((
            name.to_string(),
            temp_digits.to_string(),
            humidity_digits.to_string(),
        ));
        self
    }

    pub fn build(self) -> String {
        let mut html = r#"<html><body>"#.to_string();

        for (i, (name, temp_digits, humidity_digits)) in self.locations.iter().enumerate() {
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
                temp_digits.chars().nth(0).unwrap_or('0'),
                base_num,
                temp_digits.chars().nth(1).unwrap_or('0'),
                base_num,
                temp_digits.chars().nth(2).unwrap_or('0'),
                base_num,
                humidity_digits.chars().nth(0).unwrap_or('0'),
                base_num,
                humidity_digits.chars().nth(1).unwrap_or('0'),
                base_num,
                humidity_digits.chars().nth(2).unwrap_or('0'),
            ));
        }

        html.push_str("</body></html>");
        html
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_test_builder() {
        let html = HtmlTestBuilder::new()
            .add_element("test_id", "test_value")
            .add_element("another_id", "another_value")
            .build();

        assert!(html.contains(r#"<div id="test_id">test_value</div>"#));
        assert!(html.contains(r#"<div id="another_id">another_value</div>"#));
        assert!(html.starts_with("<html><body>"));
        assert!(html.ends_with("</body></html>"));
    }

    #[test]
    fn test_html_test_builder_with_custom_wrapper() {
        let html = HtmlTestBuilder::new()
            .with_wrapper("div")
            .add_element("test", "value")
            .build();

        assert!(html.contains("<html><div>"));
        assert!(html.contains("</div></html>"));
    }

    #[test]
    fn test_generation_details_builder() {
        let html = GenerationDetailsBuilder::new()
            .add_source(1, "太陽光", "2.5")
            .add_source(2, "燃料電池", "0.5")
            .build();

        assert!(html.contains(r#"<div id="g_d_1_title"><span>太陽光</span></div>"#));
        assert!(html.contains(r#"<div id="g_d_1_capacity"><span>2.5</span></div>"#));
        assert!(html.contains(r#"<div id="g_d_2_title"><span>燃料電池</span></div>"#));
        assert!(html.contains(r#"<div id="g_d_2_capacity"><span>0.5</span></div>"#));
    }

    #[test]
    fn test_consumption_devices_builder() {
        let html = ConsumptionDevicesBuilder::new()
            .add_device(1, "エアコン", "1.2")
            .add_device(2, "冷蔵庫", "0.5")
            .build();

        assert!(html.contains(r#"<div class="c_device"><span>エアコン</span></div>"#));
        assert!(html.contains(r#"<div class="c_value"><span>1.2</span></div>"#));
        assert!(html.contains(r#"<div id="stage_1">"#));
        assert!(html.contains(r#"<div id="stage_2">"#));
    }

    #[test]
    fn test_climate_html_builder() {
        let html = ClimateHtmlBuilder::new()
            .add_location("リビング", "235", "650")
            .build();

        assert!(html.contains(r#"<div class="txt_name">リビング</div>"#));
        assert!(html.contains(r#"class="num no2""#)); // First digit of temperature
        assert!(html.contains(r#"class="num no6""#)); // First digit of humidity
    }
}
