//! HTTP client for communicating with the AiSEG2 system.
//! 
//! This module provides a client that handles HTTP requests to the AiSEG2
//! web interface using digest authentication. The AiSEG2 system requires
//! digest auth for all API endpoints.

use crate::config;
use anyhow::{anyhow, Context, Result};
use diqwest::WithDigestAuth;
use reqwest::Client as HttpClient;

/// HTTP client for AiSEG2 API communication.
/// 
/// This client wraps the reqwest HTTP client and adds digest authentication
/// support required by the AiSEG2 system. It handles all HTTP communication
/// with the AiSEG2 device, including authentication and error handling.
/// 
/// # Authentication
/// 
/// The AiSEG2 system uses HTTP digest authentication. This client automatically
/// handles the authentication challenge-response flow using the configured
/// username and password.
pub struct Client {
    /// Underlying HTTP client from reqwest
    http_client: HttpClient,
    /// Configuration containing base URL and credentials
    config: config::Aiseg2Config,
}

impl Client {
    /// Creates a new AiSEG2 HTTP client.
    /// 
    /// # Arguments
    /// 
    /// * `config` - Configuration containing the AiSEG2 base URL, username, and password
    /// 
    /// # Example
    /// 
    /// ```no_run
    /// use crate::config::Aiseg2Config;
    /// 
    /// let config = Aiseg2Config {
    ///     url: "http://192.168.1.100".to_string(),
    ///     user: "admin".to_string(),
    ///     password: "password".to_string(),
    /// };
    /// 
    /// let client = Client::new(config);
    /// ```
    pub fn new(config: config::Aiseg2Config) -> Self {
        let http_client = HttpClient::new();
        Self {
            http_client,
            config,
        }
    }

    /// Performs an HTTP GET request to the AiSEG2 system.
    /// 
    /// This method constructs the full URL by combining the base URL from
    /// configuration with the provided path, then sends a GET request with
    /// digest authentication.
    /// 
    /// # Arguments
    /// 
    /// * `path` - The API endpoint path (e.g., "/page/electricflow/111")
    /// 
    /// # Returns
    /// 
    /// * `Ok(String)` - The response body as a string on successful requests (2xx status)
    /// * `Err(anyhow::Error)` - An error containing status code and response body for failed requests
    /// 
    /// # Errors
    /// 
    /// This method can fail in several ways:
    /// - Network connectivity issues
    /// - Authentication failures (401 Unauthorized)
    /// - Server errors (5xx status codes)
    /// - Invalid paths (404 Not Found)
    /// 
    /// # Example
    /// 
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// # let client = Client::new(config);
    /// // Fetch current power flow data
    /// let html = client.get("/page/electricflow/111").await?;
    /// 
    /// // Fetch daily total graph
    /// let graph_data = client.get("/page/graph/51111?data=...").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get(&self, path: &str) -> Result<String> {
        let url = format!("{}{}", self.config.url, path);
        let response = self
            .http_client
            .get(&url)
            .header("user-agent", "reqwest")
            .send_with_digest_auth(&self.config.user, &self.config.password)
            .await
            .context("Failed to send GET request")?;

        if response.status().is_success() {
            let body = response
                .text()
                .await
                .context("Failed to read response body")?;
            Ok(body)
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .context("Failed to read response body")?;
            Err(anyhow!("Request failed with status: {}\n{}", status, body))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aiseg::test_utils::test_config;

    #[test]
    fn test_client_new() {
        let config = test_config("http://test.local".to_string());
        let client = Client::new(config);

        assert_eq!(client.config.url, "http://test.local");
        assert_eq!(client.config.user, "test_user");
        assert_eq!(client.config.password, "test_password");
    }

    #[tokio::test]
    async fn test_get_success() {
        let mut server = mockito::Server::new_async().await;
        let mock_url = server.url();

        // Mock successful response
        let _mock = server
            .mock("GET", "/test/path")
            .with_status(200)
            .with_body("<html><body>Test Response</body></html>")
            .create_async()
            .await;

        let config = test_config(mock_url);

        let client = Client::new(config);
        let result = client.get("/test/path").await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "<html><body>Test Response</body></html>");
    }

    #[tokio::test]
    async fn test_get_404_error() {
        let mut server = mockito::Server::new_async().await;
        let mock_url = server.url();

        // Mock 404 response
        let _mock = server
            .mock("GET", "/not/found")
            .with_status(404)
            .with_body("Not Found")
            .create_async()
            .await;

        let config = test_config(mock_url);

        let client = Client::new(config);
        let result = client.get("/not/found").await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Request failed with status: 404"));
        assert!(error.to_string().contains("Not Found"));
    }

    #[tokio::test]
    async fn test_get_500_error() {
        let mut server = mockito::Server::new_async().await;
        let mock_url = server.url();

        // Mock 500 response
        let _mock = server
            .mock("GET", "/error")
            .with_status(500)
            .with_body("Internal Server Error")
            .create_async()
            .await;

        let config = test_config(mock_url);

        let client = Client::new(config);
        let result = client.get("/error").await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Request failed with status: 500"));
        assert!(error.to_string().contains("Internal Server Error"));
    }

    #[tokio::test]
    async fn test_get_with_json_response() {
        let mut server = mockito::Server::new_async().await;
        let mock_url = server.url();

        // Mock JSON response
        let _mock = server
            .mock("GET", "/api/data")
            .with_status(200)
            .with_body(r#"{"status":"ok","value":123.45}"#)
            .create_async()
            .await;

        let config = test_config(mock_url);

        let client = Client::new(config);
        let result = client.get("/api/data").await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), r#"{"status":"ok","value":123.45}"#);
    }

    #[tokio::test]
    async fn test_get_with_html_response() {
        let mut server = mockito::Server::new_async().await;
        let mock_url = server.url();

        // Mock HTML response with Japanese characters
        let html_body = r#"
            <html>
                <body>
                    <div id="g_capacity">2.5</div>
                    <div class="meter-name">太陽光発電</div>
                </body>
            </html>
        "#;

        let _mock = server
            .mock("GET", "/page/electricflow/111")
            .with_status(200)
            .with_body(html_body)
            .create_async()
            .await;

        let config = test_config(mock_url);

        let client = Client::new(config);
        let result = client.get("/page/electricflow/111").await;

        assert!(result.is_ok());
        let body = result.unwrap();
        assert!(body.contains("g_capacity"));
        assert!(body.contains("2.5"));
        assert!(body.contains("太陽光発電"));
    }

    #[tokio::test]
    async fn test_get_connection_error() {
        // Use a non-existent server URL
        let config = test_config("http://non-existent-server.local:12345".to_string());

        let client = Client::new(config);
        let result = client.get("/test").await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to send GET request"));
    }
}
