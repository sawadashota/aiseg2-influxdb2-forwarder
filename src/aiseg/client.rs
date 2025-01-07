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
