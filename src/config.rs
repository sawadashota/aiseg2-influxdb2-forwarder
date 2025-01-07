use anyhow::{anyhow, Result};
use envy;
use serde_derive::Deserialize;
use std::str::FromStr;

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Deserialize, Debug)]
pub struct AppConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl AppConfig {
    pub fn log_level(&self) -> tracing::Level {
        tracing::Level::from_str(self.log_level.as_str()).unwrap_or(tracing::Level::INFO)
    }
}

pub(crate) fn load_app_config() -> Result<AppConfig> {
    match envy::from_env::<AppConfig>() {
        Ok(config) => Ok(config),
        Err(err) => Err(anyhow!("Failed to load AppConfig: {}", err)),
    }
}

fn default_status_interval_sec() -> u64 {
    5
}

fn default_total_interval_sec() -> u64 {
    60
}

fn default_total_initial_days() -> u64 {
    30
}

#[derive(Deserialize, Debug)]
pub struct CollectorConfig {
    #[serde(default = "default_status_interval_sec")]
    pub status_interval_sec: u64,
    #[serde(default = "default_total_interval_sec")]
    pub total_interval_sec: u64,
    // how many days to collect total metrics at initial
    #[serde(default = "default_total_initial_days")]
    pub total_initial_days: u64,
}

pub fn load_collector_config() -> Result<CollectorConfig> {
    match envy::prefixed("COLLECTOR_").from_env::<CollectorConfig>() {
        Ok(config) => Ok(config),
        Err(err) => Err(anyhow!("Failed to load CollectorConfig: {}", err)),
    }
}

#[derive(Deserialize, Debug)]
pub struct Aiseg2Config {
    pub url: String,
    pub user: String,
    pub password: String,
}

pub(crate) fn load_aiseg_config() -> Result<Aiseg2Config> {
    match envy::prefixed("AISEG2_").from_env::<Aiseg2Config>() {
        Ok(config) => Ok(config),
        Err(err) => Err(anyhow!("Failed to load AisegConfig: {}", err)),
    }
}

#[derive(Deserialize, Debug)]
pub struct InfluxConfig {
    pub url: String,
    pub token: String,
    pub org: String,
    pub bucket: String,
}

pub fn load_influx_config() -> Result<InfluxConfig> {
    match envy::prefixed("INFLUXDB_").from_env::<InfluxConfig>() {
        Ok(config) => Ok(config),
        Err(err) => Err(anyhow!("Failed to load InfluxConfig: {}", err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_load_app_config() {
        std::env::set_var("LOG_LEVEL", "debug");

        let result = load_app_config();

        std::env::remove_var("LOG_LEVEL");

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.log_level, "debug");
    }

    #[test]
    #[serial]
    fn test_load_app_config_missing() {
        let result = load_app_config();
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.log_level, "info");
    }

    #[test]
    #[serial]
    fn test_load_collector_config() {
        std::env::set_var("COLLECTOR_TOTAL_INTERVAL_SEC", "10");
        std::env::set_var("COLLECTOR_STATUS_INTERVAL_SEC", "20");
        std::env::set_var("COLLECTOR_TOTAL_INITIAL_DAYS", "30");

        let result = load_collector_config();

        std::env::remove_var("COLLECTOR_TOTAL_INTERVAL_SEC");
        std::env::remove_var("COLLECTOR_STATUS_INTERVAL_SEC");
        std::env::remove_var("COLLECTOR_TOTAL_INITIAL_DAYS");

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.total_interval_sec, 10);
        assert_eq!(config.status_interval_sec, 20);
        assert_eq!(config.total_initial_days, 30);
    }

    #[test]
    #[serial]
    fn test_load_collector_config_missing() {
        let result = load_collector_config();
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.status_interval_sec, 5);
        assert_eq!(config.total_interval_sec, 60);
        assert_eq!(config.total_initial_days, 30);
    }

    #[test]
    #[serial]
    fn test_load_aiseg_config() {
        std::env::set_var("AISEG2_URL", "http://localhost:8080");
        std::env::set_var("AISEG2_USER", "root");
        std::env::set_var("AISEG2_PASSWORD", "password");

        let result = load_aiseg_config();

        std::env::remove_var("AISEG2_HOST");
        std::env::remove_var("AISEG2_USER");
        std::env::remove_var("AISEG2_PASSWORD");

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.url, "http://localhost:8080");
        assert_eq!(config.user, "root");
        assert_eq!(config.password, "password");
    }

    #[test]
    #[serial]
    fn test_load_aiseg_config_missing() {
        let result = load_aiseg_config();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to load AisegConfig"));
    }

    #[test]
    #[serial]
    fn test_load_influx_config() {
        std::env::set_var("INFLUXDB_URL", "http://localhost:8086");
        std::env::set_var("INFLUXDB_TOKEN", "token");
        std::env::set_var("INFLUXDB_ORG", "org");
        std::env::set_var("INFLUXDB_BUCKET", "bucket");

        let result = load_influx_config();

        std::env::remove_var("INFLUXDB_HOST");
        std::env::remove_var("INFLUXDB_TOKEN");
        std::env::remove_var("INFLUXDB_ORG");
        std::env::remove_var("INFLUXDB_BUCKET");

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.url, "http://localhost:8086");
        assert_eq!(config.token, "token");
        assert_eq!(config.org, "org");
        assert_eq!(config.bucket, "bucket");
    }

    #[test]
    #[serial]
    fn test_load_influx_config_missing() {
        let result = load_influx_config();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Failed to load InfluxConfig"));
    }
}
