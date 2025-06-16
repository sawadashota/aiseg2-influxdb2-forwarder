//! Mock implementations and server helpers for testing.
//!
//! This module provides mock server builders and response generators
//! for testing HTTP interactions with AiSEG2 and InfluxDB.

pub mod collectors;

use mockito::{Mock, Server, ServerGuard};
use wiremock::matchers::{method, path};
use wiremock::{Mock as WireMock, MockServer, ResponseTemplate};

// Re-export collector mocks for convenience
pub use collectors::*;

/// Builder for creating mockito server mocks for AiSEG2 endpoints.
pub struct MockAiseg2ServerBuilder {
    server: ServerGuard,
    mocks: Vec<Mock>,
}

impl MockAiseg2ServerBuilder {
    /// Creates a new mock server builder.
    pub async fn new() -> Self {
        Self {
            server: Server::new_async().await,
            mocks: Vec::new(),
        }
    }

    /// Gets the server URL.
    pub fn url(&self) -> String {
        self.server.url()
    }

    /// Adds a mock for the power status page.
    pub async fn mock_power_status(mut self, generation: &str, consumption: &str) -> Self {
        let body = format!(
            r#"<html><body>
                <div id="g_capacity">{}</div>
                <div id="u_capacity">{}</div>
            </body></html>"#,
            generation, consumption
        );

        let mock = self
            .server
            .mock("GET", "/page/top")
            .with_status(200)
            .with_body(body)
            .create_async()
            .await;

        self.mocks.push(mock);
        self
    }

    /// Adds a mock for the climate status page.
    pub async fn mock_climate_status(mut self, locations: Vec<(&str, &str, &str)>) -> Self {
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

        let mock = self
            .server
            .mock("GET", "/page/climate")
            .with_status(200)
            .with_body(html)
            .create_async()
            .await;

        self.mocks.push(mock);
        self
    }

    /// Adds a mock for a daily total graph page.
    pub async fn mock_daily_total(
        mut self,
        graph_id: &str,
        title: &str,
        value: &str,
        query: &str,
    ) -> Self {
        let body = format!(
            r#"<html><body>
                <div id="h_title">{}</div>
                <div id="val_kwh">{}</div>
            </body></html>"#,
            title, value
        );

        let mock = self
            .server
            .mock(
                "GET",
                format!("/page/graph/{}?data={}", graph_id, query).as_str(),
            )
            .with_status(200)
            .with_body(body)
            .create_async()
            .await;

        self.mocks.push(mock);
        self
    }

    /// Adds a mock for a circuit daily total page.
    pub async fn mock_circuit_total(mut self, value: &str, query: &str) -> Self {
        let body = format!(
            r#"<html><body><div id="val_kwh">{}</div></body></html>"#,
            value
        );

        let mock = self
            .server
            .mock("GET", format!("/page/graph/584?data={}", query).as_str())
            .with_status(200)
            .with_body(body)
            .create_async()
            .await;

        self.mocks.push(mock);
        self
    }

    /// Adds a mock for an error response.
    pub async fn mock_error(mut self, path: &str, status: u16, body: &str) -> Self {
        let mock = self
            .server
            .mock("GET", path)
            .with_status(status as usize)
            .with_body(body)
            .create_async()
            .await;

        self.mocks.push(mock);
        self
    }

    /// Builds and returns the configured mock server.
    pub fn build(self) -> ServerGuard {
        self.server
    }
}

/// Builder for creating wiremock server mocks for InfluxDB endpoints.
pub struct MockInfluxServerBuilder {
    server: MockServer,
}

impl MockInfluxServerBuilder {
    /// Creates a new mock InfluxDB server builder.
    pub async fn new() -> Self {
        Self {
            server: MockServer::start().await,
        }
    }

    /// Gets the server URL.
    pub fn url(&self) -> String {
        self.server.uri()
    }

    /// Mocks a successful write response.
    pub async fn mock_write_success(self) -> Self {
        WireMock::given(method("POST"))
            .and(path("/api/v2/write"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&self.server)
            .await;
        self
    }

    /// Mocks a write error response.
    pub async fn mock_write_error(self, status: u16, message: &str) -> Self {
        WireMock::given(method("POST"))
            .and(path("/api/v2/write"))
            .respond_with(ResponseTemplate::new(status).set_body_string(message))
            .mount(&self.server)
            .await;
        self
    }

    /// Mocks a write response with expectations.
    pub async fn mock_write_with_expectation(self, times: u64) -> Self {
        WireMock::given(method("POST"))
            .and(path("/api/v2/write"))
            .respond_with(ResponseTemplate::new(204))
            .expect(times)
            .mount(&self.server)
            .await;
        self
    }

    /// Builds and returns the configured mock server.
    pub fn build(self) -> MockServer {
        self.server
    }
}

/// Helper functions for creating common mock responses.
pub mod responses {
    /// Creates a standard error response body.
    pub fn error_response(code: &str, message: &str) -> String {
        format!(r#"{{"code":"{}","message":"{}"}}"#, code, message)
    }

    /// Creates an HTML error page.
    pub fn html_error_page(title: &str, message: &str) -> String {
        format!(
            r#"<html>
                <head><title>{}</title></head>
                <body>
                    <h1>{}</h1>
                    <p>{}</p>
                </body>
            </html>"#,
            title, title, message
        )
    }

    /// Creates a successful JSON response.
    pub fn json_success(data: &str) -> String {
        format!(r#"{{"status":"success","data":{}}}"#, data)
    }
}

/// Mock data generators for common test scenarios.
pub mod generators {
    use crate::test_utils::fixtures::constants;

    /// Generates mock daily totals for all six metrics.
    pub fn daily_totals_mock_set() -> Vec<(&'static str, &'static str, &'static str)> {
        vec![
            (constants::graph_ids::GENERATION, "発電量", "100.0"),
            (constants::graph_ids::CONSUMPTION, "消費量", "200.0"),
            (constants::graph_ids::GRID_BUY, "買電量", "50.0"),
            (constants::graph_ids::GRID_SELL, "売電量", "75.0"),
            (constants::graph_ids::HOT_WATER, "給湯量", "300.0"),
            (constants::graph_ids::GAS, "ガス量", "25.5"),
        ]
    }

    /// Generates mock climate data for multiple locations.
    pub fn climate_mock_set() -> Vec<(&'static str, &'static str, &'static str)> {
        vec![
            ("リビング", "235", "650"),
            ("寝室", "220", "600"),
            ("子供部屋", "240", "700"),
        ]
    }

    /// Generates mock circuit data.
    pub fn circuit_mock_set() -> Vec<(&'static str, &'static str)> {
        vec![
            ("EV", "50.5"),
            ("リビングエアコン", "12.3"),
            ("主寝室エアコン", "8.7"),
            ("洋室２エアコン", "5.2"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_aiseg2_server_builder() {
        let server = MockAiseg2ServerBuilder::new()
            .await
            .mock_power_status("2.5", "3.8")
            .await
            .build();

        let url = server.url();
        assert!(url.starts_with("http://"));
    }

    #[tokio::test]
    async fn test_mock_influx_server_builder() {
        let server = MockInfluxServerBuilder::new()
            .await
            .mock_write_success()
            .await
            .build();

        let url = server.uri();
        assert!(url.starts_with("http://"));
    }

    #[test]
    fn test_response_generators() {
        let error = responses::error_response("AUTH_ERROR", "Invalid credentials");
        assert!(error.contains("AUTH_ERROR"));
        assert!(error.contains("Invalid credentials"));

        let html_error = responses::html_error_page("404 Not Found", "Page not found");
        assert!(html_error.contains("<title>404 Not Found</title>"));
        assert!(html_error.contains("<h1>404 Not Found</h1>"));
    }

    #[test]
    fn test_mock_generators() {
        let daily_totals = generators::daily_totals_mock_set();
        assert_eq!(daily_totals.len(), 6);
        assert_eq!(daily_totals[0].0, "51111");
        assert_eq!(daily_totals[0].1, "発電量");

        let climate_data = generators::climate_mock_set();
        assert_eq!(climate_data.len(), 3);
        assert_eq!(climate_data[0].0, "リビング");
    }
}
