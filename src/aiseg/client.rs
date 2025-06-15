use crate::config;
use anyhow::{anyhow, Context, Result};
use diqwest::WithDigestAuth;
use reqwest::Client as HttpClient;

pub struct Client {
    http_client: HttpClient,
    config: config::Aiseg2Config,
}

impl Client {
    pub fn new(config: config::Aiseg2Config) -> Self {
        let http_client = HttpClient::new();
        Self {
            http_client,
            config,
        }
    }

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
    use mockito;

    fn test_config() -> config::Aiseg2Config {
        config::Aiseg2Config {
            url: "http://test.local".to_string(),
            user: "test_user".to_string(),
            password: "test_password".to_string(),
        }
    }

    #[test]
    fn test_client_new() {
        let config = test_config();
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

        let config = config::Aiseg2Config {
            url: mock_url,
            user: "test_user".to_string(),
            password: "test_password".to_string(),
        };
        
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

        let config = config::Aiseg2Config {
            url: mock_url,
            user: "test_user".to_string(),
            password: "test_password".to_string(),
        };
        
        let client = Client::new(config);
        let result = client.get("/not/found").await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Request failed with status: 404"));
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

        let config = config::Aiseg2Config {
            url: mock_url,
            user: "test_user".to_string(),
            password: "test_password".to_string(),
        };
        
        let client = Client::new(config);
        let result = client.get("/error").await;
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Request failed with status: 500"));
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

        let config = config::Aiseg2Config {
            url: mock_url,
            user: "test_user".to_string(),
            password: "test_password".to_string(),
        };
        
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

        let config = config::Aiseg2Config {
            url: mock_url,
            user: "test_user".to_string(),
            password: "test_password".to_string(),
        };
        
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
        let config = config::Aiseg2Config {
            url: "http://non-existent-server.local:12345".to_string(),
            user: "test_user".to_string(),
            password: "test_password".to_string(),
        };
        
        let client = Client::new(config);
        let result = client.get("/test").await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to send GET request"));
    }
}
