use crate::config::InfluxConfig;
use anyhow::Result;
use futures::prelude::stream;

pub struct Client {
    client: influxdb2::Client,
    bucket: String,
}

impl Client {
    pub(crate) fn new(config: InfluxConfig) -> Self {
        let client = influxdb2::Client::new(config.url, config.org, config.token);
        Self {
            client,
            bucket: config.bucket,
        }
    }

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
