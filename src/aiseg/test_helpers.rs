//! Shared test utilities for AiSEG2 module tests.
//!
//! This module provides common test helper functions and mock data generators
//! to reduce duplication across test files.

#![cfg(test)]

use scraper::Html;

/// Creates a standard HTML document wrapper for test content.
pub fn create_html_document(content: &str) -> Html {
    Html::parse_document(&format!(r#"<html><body>{}</body></html>"#, content))
}

/// Creates a simple HTML response with a title and value element.
/// Commonly used for daily total collectors.
#[allow(dead_code)]
pub fn create_title_value_html(title: &str, value: &str) -> String {
    format!(
        r#"<html><body>
            <div id="h_title">{}</div>
            <div id="val_kwh">{}</div>
        </body></html>"#,
        title, value
    )
}

/// Creates an HTML response with only a value element.
/// Commonly used for circuit daily total collectors.
#[allow(dead_code)]
pub fn create_value_only_html(value: &str) -> String {
    format!(
        r#"<html><body><div id="val_kwh">{}</div></body></html>"#,
        value
    )
}

/// Creates an HTML response for power generation/consumption display.
pub fn create_power_flow_html(generation: &str, consumption: &str) -> String {
    format!(
        r#"<html><body>
            <div id="g_capacity">{}</div>
            <div id="u_capacity">{}</div>
        </body></html>"#,
        generation, consumption
    )
}

/// Creates an HTML response for generation details with multiple sources.
pub fn create_generation_details_html(sources: &[(usize, &str, &str)]) -> String {
    let mut html = String::from(r#"<html><body>"#);

    for (index, title, capacity) in sources {
        html.push_str(&format!(
            r#"<div id="g_d_{}_title"><span>{}</span></div>
               <div id="g_d_{}_capacity"><span>{}</span></div>"#,
            index, title, index, capacity
        ));
    }

    html.push_str("</body></html>");
    html
}

/// Creates an HTML response for consumption device pages.
pub fn create_consumption_devices_html(devices: &[(usize, &str, &str)]) -> String {
    let mut html = String::from(r#"<html><body>"#);

    for (index, device, value) in devices {
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

/// Creates an HTML response for climate data with digit-based display.
pub fn create_climate_html(locations: Vec<(&str, &str, &str)>) -> String {
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

/// Common test data for power metrics.
#[allow(dead_code)]
pub mod test_data {
    /// Standard test generation value in kW.
    pub const TEST_GENERATION_KW: f64 = 2.5;

    /// Standard test consumption value in kW.
    pub const TEST_CONSUMPTION_KW: f64 = 3.8;

    /// Standard test temperature value.
    pub const TEST_TEMPERATURE: f64 = 23.5;

    /// Standard test humidity value.
    pub const TEST_HUMIDITY: f64 = 65.0;

    /// Common device names for testing.
    pub const TEST_DEVICES: &[&str] = &["エアコン", "冷蔵庫", "テレビ", "照明"];

    /// Common circuit names for testing.
    pub const TEST_CIRCUITS: &[(&str, &str)] = &[
        ("30", "EV"),
        ("27", "リビングエアコン"),
        ("26", "主寝室エアコン"),
        ("25", "洋室２エアコン"),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_html_document() {
        let html = create_html_document("<div>Test</div>");
        let text = html.root_element().text().collect::<String>();
        assert!(text.contains("Test"));
    }

    #[test]
    fn test_create_title_value_html() {
        let html_str = create_title_value_html("太陽光発電量", "123.45");
        assert!(html_str.contains(r#"<div id="h_title">太陽光発電量</div>"#));
        assert!(html_str.contains(r#"<div id="val_kwh">123.45</div>"#));
    }

    #[test]
    fn test_create_power_flow_html() {
        let html_str = create_power_flow_html("2.5", "3.8");
        assert!(html_str.contains(r#"<div id="g_capacity">2.5</div>"#));
        assert!(html_str.contains(r#"<div id="u_capacity">3.8</div>"#));
    }

    #[test]
    fn test_create_generation_details_html() {
        let sources = vec![(1, "太陽光", "2.5"), (2, "燃料電池", "0.5")];
        let html_str = create_generation_details_html(&sources);

        assert!(html_str.contains(r#"<div id="g_d_1_title"><span>太陽光</span></div>"#));
        assert!(html_str.contains(r#"<div id="g_d_1_capacity"><span>2.5</span></div>"#));
        assert!(html_str.contains(r#"<div id="g_d_2_title"><span>燃料電池</span></div>"#));
        assert!(html_str.contains(r#"<div id="g_d_2_capacity"><span>0.5</span></div>"#));
    }

    #[test]
    fn test_create_consumption_devices_html() {
        let devices = vec![(1, "エアコン", "1.2"), (2, "冷蔵庫", "0.5")];
        let html_str = create_consumption_devices_html(&devices);

        assert!(html_str.contains(r#"<div class="c_device"><span>エアコン</span></div>"#));
        assert!(html_str.contains(r#"<div class="c_value"><span>1.2</span></div>"#));
    }

    #[test]
    fn test_create_climate_html() {
        let locations = vec![("リビング", "235", "650")];
        let html_str = create_climate_html(locations);

        assert!(html_str.contains(r#"<div class="txt_name">リビング</div>"#));
        assert!(html_str.contains(r#"class="num no2""#)); // First digit of temperature
        assert!(html_str.contains(r#"class="num no6""#)); // First digit of humidity
    }
}
