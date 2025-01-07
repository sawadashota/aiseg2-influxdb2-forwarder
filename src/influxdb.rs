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
