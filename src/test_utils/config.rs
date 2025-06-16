//! Configuration utilities for testing.
//!
//! This module provides test configuration builders and helpers for creating
//! mock configurations used throughout the test suite.

use crate::config::{Aiseg2Config, InfluxConfig};

/// Builder for creating test AiSEG2 configurations.
#[derive(Debug)]
pub struct TestAiseg2ConfigBuilder {
    url: String,
    user: String,
    password: String,
}

impl TestAiseg2ConfigBuilder {
    /// Creates a new test config builder with default values.
    pub fn new() -> Self {
        Self {
            url: "http://test.local".to_string(),
            user: "test_user".to_string(),
            password: "test_password".to_string(),
        }
    }

    /// Sets the URL for the test configuration.
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// Sets the username for the test configuration.
    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = user.into();
        self
    }

    /// Sets the password for the test configuration.
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = password.into();
        self
    }

    /// Builds the AiSEG2 configuration.
    pub fn build(self) -> Aiseg2Config {
        Aiseg2Config {
            url: self.url,
            user: self.user,
            password: self.password,
        }
    }
}

/// Builder for creating test InfluxDB configurations.
#[derive(Debug)]
pub struct TestInfluxConfigBuilder {
    url: String,
    org: String,
    token: String,
    bucket: String,
}

impl TestInfluxConfigBuilder {
    /// Creates a new test config builder with default values.
    pub fn new() -> Self {
        Self {
            url: "http://localhost:8086".to_string(),
            org: "test-org".to_string(),
            token: "test-token".to_string(),
            bucket: "test-bucket".to_string(),
        }
    }

    /// Sets the URL for the test configuration.
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// Sets the organization for the test configuration.
    pub fn with_org(mut self, org: impl Into<String>) -> Self {
        self.org = org.into();
        self
    }

    /// Sets the token for the test configuration.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = token.into();
        self
    }

    /// Sets the bucket for the test configuration.
    pub fn with_bucket(mut self, bucket: impl Into<String>) -> Self {
        self.bucket = bucket.into();
        self
    }

    /// Builds the InfluxDB configuration.
    pub fn build(self) -> InfluxConfig {
        InfluxConfig {
            url: self.url,
            org: self.org,
            token: self.token,
            bucket: self.bucket,
        }
    }
}

/// Creates a default test AiSEG2 configuration.
/// This is a convenience function for simple test cases.
pub fn test_aiseg2_config() -> Aiseg2Config {
    TestAiseg2ConfigBuilder::new().build()
}

/// Creates a test AiSEG2 configuration with a custom URL.
/// This is a convenience function for tests that need to specify a mock server URL.
pub fn test_aiseg2_config_with_url(url: impl Into<String>) -> Aiseg2Config {
    TestAiseg2ConfigBuilder::new().with_url(url).build()
}

/// Creates a default test InfluxDB configuration.
/// This is a convenience function for simple test cases.
pub fn test_influx_config() -> InfluxConfig {
    TestInfluxConfigBuilder::new().build()
}

/// Creates a test InfluxDB configuration with a custom URL.
/// This is a convenience function for tests that need to specify a mock server URL.
pub fn test_influx_config_with_url(url: impl Into<String>) -> InfluxConfig {
    TestInfluxConfigBuilder::new().with_url(url).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aiseg2_config_builder() {
        let config = TestAiseg2ConfigBuilder::new()
            .with_url("http://custom.local")
            .with_user("custom_user")
            .with_password("custom_pass")
            .build();

        assert_eq!(config.url, "http://custom.local");
        assert_eq!(config.user, "custom_user");
        assert_eq!(config.password, "custom_pass");
    }

    #[test]
    fn test_influx_config_builder() {
        let config = TestInfluxConfigBuilder::new()
            .with_url("http://influx.local")
            .with_org("my-org")
            .with_token("my-token")
            .with_bucket("my-bucket")
            .build();

        assert_eq!(config.url, "http://influx.local");
        assert_eq!(config.org, "my-org");
        assert_eq!(config.token, "my-token");
        assert_eq!(config.bucket, "my-bucket");
    }

    #[test]
    fn test_convenience_functions() {
        let aiseg_config = test_aiseg2_config();
        assert_eq!(aiseg_config.url, "http://test.local");

        let aiseg_config_with_url = test_aiseg2_config_with_url("http://mock.local");
        assert_eq!(aiseg_config_with_url.url, "http://mock.local");

        let influx_config = test_influx_config();
        assert_eq!(influx_config.url, "http://localhost:8086");

        let influx_config_with_url = test_influx_config_with_url("http://mock:8086");
        assert_eq!(influx_config_with_url.url, "http://mock:8086");
    }
}
