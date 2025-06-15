//! InfluxDB2 client wrapper for writing time-series metrics.
//!
//! This module provides a simplified interface to the InfluxDB2 client, handling
//! connection management and data point writing operations. It wraps the official
//! influxdb2 Rust client to provide a focused API for the forwarder's needs.
//!
//! # Architecture
//! The client maintains a persistent connection to InfluxDB2 and streams data points
//! in batches for efficient network utilization. All write operations are asynchronous
//! to prevent blocking the metric collection tasks.
//!
//! # Error Handling
//! Network errors, authentication failures, and InfluxDB server errors are propagated
//! as anyhow::Result errors, allowing the caller to implement retry logic or failover
//! strategies as needed.

use crate::config::InfluxConfig;
use anyhow::Result;
use futures::prelude::stream;

/// InfluxDB2 client wrapper for writing metrics data.
///
/// This client encapsulates the InfluxDB2 connection and provides a simple interface
/// for writing data points. It maintains the target bucket configuration and handles
/// the streaming of data points to the InfluxDB2 write API.
///
/// # Example
/// ```rust
/// let config = InfluxConfig {
///     url: "http://localhost:8086".to_string(),
///     org: "my-org".to_string(),
///     token: "my-token".to_string(),
///     bucket: "metrics".to_string(),
/// };
/// let client = Client::new(config);
/// let points = vec![/* data points */];
/// client.write(points).await?;
/// ```
pub struct Client {
    /// The underlying InfluxDB2 client instance
    client: influxdb2::Client,
    /// The target bucket for all write operations
    bucket: String,
}

impl Client {
    /// Creates a new InfluxDB client instance.
    ///
    /// Initializes the client with the provided configuration, establishing the connection
    /// parameters for the InfluxDB2 instance. The actual network connection is established
    /// lazily on the first write operation.
    ///
    /// # Arguments
    /// * `config` - InfluxDB connection configuration containing:
    ///   - `url`: The InfluxDB2 server URL (e.g., "http://localhost:8086")
    ///   - `org`: The organization name for authentication
    ///   - `token`: The API token for authentication
    ///   - `bucket`: The target bucket for write operations
    ///
    /// # Returns
    /// A new Client instance configured with the provided settings
    ///
    /// # Connection Behavior
    /// The client uses lazy connection establishment - no network activity occurs
    /// during construction. The first write operation will establish the connection.
    pub(crate) fn new(config: InfluxConfig) -> Self {
        let client = influxdb2::Client::new(config.url, config.org, config.token);
        Self {
            client,
            bucket: config.bucket,
        }
    }

    /// Writes a batch of data points to InfluxDB.
    ///
    /// Streams the provided data points to the configured bucket using the InfluxDB2
    /// line protocol. The operation is atomic - either all points are written successfully
    /// or none are written.
    ///
    /// # Arguments
    /// * `points` - Vector of DataPoint instances to write. Can be empty.
    ///
    /// # Returns
    /// * `Ok(())` - All points were written successfully
    /// * `Err(e)` - Write operation failed due to:
    ///   - Network connectivity issues
    ///   - Authentication/authorization failures (401/403)
    ///   - Invalid data point format (400)
    ///   - Server errors (500)
    ///   - Rate limiting (429)
    ///
    /// # Performance
    /// The client streams points efficiently using the line protocol, batching them
    /// for optimal network utilization. Large batches are automatically handled by
    /// the underlying client.
    ///
    /// # Example
    /// ```rust
    /// let points = vec![
    ///     DataPoint::builder("temperature")
    ///         .tag("location", "room1")
    ///         .field("value", 23.5)
    ///         .build()?,
    /// ];
    /// client.write(points).await?;
    /// ```
    pub async fn write(&self, points: Vec<influxdb2::models::DataPoint>) -> Result<()> {
        Ok(self
            .client
            .write(self.bucket.as_str(), stream::iter(points))
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InfluxConfig;
    use influxdb2::models::DataPoint;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_config(url: String) -> InfluxConfig {
        InfluxConfig {
            url,
            org: "test-org".to_string(),
            token: "test-token".to_string(),
            bucket: "test-bucket".to_string(),
        }
    }

    fn create_test_point() -> DataPoint {
        DataPoint::builder("test_measurement")
            .tag("test_tag", "test_value")
            .field("test_field", 123.45)
            .build()
            .unwrap()
    }

    mod succeeds {
        use super::*;

        #[test]
        fn test_client_new() {
            let config = test_config("http://localhost:8086".to_string());
            let client = Client::new(config);

            assert_eq!(client.bucket, "test-bucket");
        }

        #[tokio::test]
        async fn test_write_single_point() {
            let mock_server = MockServer::start().await;
            let config = test_config(mock_server.uri());
            let client = Client::new(config);

            Mock::given(method("POST"))
                .and(path("/api/v2/write"))
                .respond_with(ResponseTemplate::new(204))
                .expect(1)
                .mount(&mock_server)
                .await;

            let point = create_test_point();
            let result = client.write(vec![point]).await;

            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_write_multiple_points() {
            let mock_server = MockServer::start().await;
            let config = test_config(mock_server.uri());
            let client = Client::new(config);

            Mock::given(method("POST"))
                .and(path("/api/v2/write"))
                .respond_with(ResponseTemplate::new(204))
                .expect(1)
                .mount(&mock_server)
                .await;

            let points = vec![
                create_test_point(),
                create_test_point(),
                create_test_point(),
            ];
            let result = client.write(points).await;

            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_write_empty_points() {
            let mock_server = MockServer::start().await;
            let config = test_config(mock_server.uri());
            let client = Client::new(config);

            // The influxdb2 client handles empty vectors gracefully
            // It might not even make a request, but if it does, we're ready
            Mock::given(method("POST"))
                .and(path("/api/v2/write"))
                .respond_with(ResponseTemplate::new(204))
                .mount(&mock_server)
                .await;

            let result = client.write(vec![]).await;

            assert!(result.is_ok());
        }
    }

    mod fails {
        use super::*;

        #[tokio::test]
        async fn test_write_network_error() {
            // Use a non-existent server to simulate network error
            let config = test_config("http://localhost:1".to_string());
            let client = Client::new(config);

            let point = create_test_point();
            let result = client.write(vec![point]).await;

            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_write_auth_error() {
            let mock_server = MockServer::start().await;
            let config = test_config(mock_server.uri());
            let client = Client::new(config);

            Mock::given(method("POST"))
                .and(path("/api/v2/write"))
                .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
                .expect(1)
                .mount(&mock_server)
                .await;

            let point = create_test_point();
            let result = client.write(vec![point]).await;

            assert!(result.is_err());
            let err_str = result.unwrap_err().to_string();
            assert!(err_str.contains("401") || err_str.contains("Unauthorized"));
        }

        #[tokio::test]
        async fn test_write_server_error() {
            let mock_server = MockServer::start().await;
            let config = test_config(mock_server.uri());
            let client = Client::new(config);

            Mock::given(method("POST"))
                .and(path("/api/v2/write"))
                .respond_with(ResponseTemplate::new(500).set_body_string("internal server error"))
                .expect(1)
                .mount(&mock_server)
                .await;

            let point = create_test_point();
            let result = client.write(vec![point]).await;

            assert!(result.is_err());
            let err_str = result.unwrap_err().to_string();
            assert!(err_str.contains("500") || err_str.contains("Internal Server Error"));
        }
    }
}
