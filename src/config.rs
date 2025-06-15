use anyhow::{anyhow, Result};
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
    use std::env::VarError;

    /// Helper to temporarily set an environment variable and restore it after
    fn with_env_var<F, R>(key: &str, value: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let original = std::env::var(key).ok();
        std::env::set_var(key, value);
        let result = f();
        match original {
            Some(val) => std::env::set_var(key, val),
            None => std::env::remove_var(key),
        }
        result
    }

    /// Helper to temporarily clear environment variables and restore them after
    fn without_env_vars<F, R>(keys: &[&str], f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let originals: Vec<(String, Result<String, VarError>)> = keys
            .iter()
            .map(|&key| (key.to_string(), std::env::var(key)))
            .collect();

        // Clear all specified variables
        for key in keys {
            std::env::remove_var(key);
        }

        let result = f();

        // Restore original values
        for (key, original) in originals {
            match original {
                Ok(val) => std::env::set_var(&key, val),
                Err(_) => std::env::remove_var(&key),
            }
        }

        result
    }

    #[test]
    #[serial]
    fn test_load_app_config() {
        with_env_var("LOG_LEVEL", "debug", || {
            let result = load_app_config();
            assert!(result.is_ok());
            let config = result.unwrap();
            assert_eq!(config.log_level, "debug");
        });
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
        // Save and restore original values
        let original_total = std::env::var("COLLECTOR_TOTAL_INTERVAL_SEC").ok();
        let original_status = std::env::var("COLLECTOR_STATUS_INTERVAL_SEC").ok();
        let original_days = std::env::var("COLLECTOR_TOTAL_INITIAL_DAYS").ok();

        std::env::set_var("COLLECTOR_TOTAL_INTERVAL_SEC", "10");
        std::env::set_var("COLLECTOR_STATUS_INTERVAL_SEC", "20");
        std::env::set_var("COLLECTOR_TOTAL_INITIAL_DAYS", "30");

        let result = load_collector_config();

        // Restore original values
        match original_total {
            Some(val) => std::env::set_var("COLLECTOR_TOTAL_INTERVAL_SEC", val),
            None => std::env::remove_var("COLLECTOR_TOTAL_INTERVAL_SEC"),
        }
        match original_status {
            Some(val) => std::env::set_var("COLLECTOR_STATUS_INTERVAL_SEC", val),
            None => std::env::remove_var("COLLECTOR_STATUS_INTERVAL_SEC"),
        }
        match original_days {
            Some(val) => std::env::set_var("COLLECTOR_TOTAL_INITIAL_DAYS", val),
            None => std::env::remove_var("COLLECTOR_TOTAL_INITIAL_DAYS"),
        }

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
        // Save original values
        let original_url = std::env::var("AISEG2_URL").ok();
        let original_user = std::env::var("AISEG2_USER").ok();
        let original_password = std::env::var("AISEG2_PASSWORD").ok();

        std::env::set_var("AISEG2_URL", "http://localhost:8080");
        std::env::set_var("AISEG2_USER", "root");
        std::env::set_var("AISEG2_PASSWORD", "password");

        let result = load_aiseg_config();

        // Restore original values
        match original_url {
            Some(val) => std::env::set_var("AISEG2_URL", val),
            None => std::env::remove_var("AISEG2_URL"),
        }
        match original_user {
            Some(val) => std::env::set_var("AISEG2_USER", val),
            None => std::env::remove_var("AISEG2_USER"),
        }
        match original_password {
            Some(val) => std::env::set_var("AISEG2_PASSWORD", val),
            None => std::env::remove_var("AISEG2_PASSWORD"),
        }

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.url, "http://localhost:8080");
        assert_eq!(config.user, "root");
        assert_eq!(config.password, "password");
    }

    #[test]
    #[serial]
    fn test_load_aiseg_config_missing() {
        // Temporarily clear AISEG2 environment variables
        without_env_vars(&["AISEG2_URL", "AISEG2_USER", "AISEG2_PASSWORD"], || {
            let result = load_aiseg_config();
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.to_string().contains("Failed to load AisegConfig"));
        });
    }

    #[test]
    #[serial]
    fn test_load_influx_config() {
        // Save original values
        let original_url = std::env::var("INFLUXDB_URL").ok();
        let original_token = std::env::var("INFLUXDB_TOKEN").ok();
        let original_org = std::env::var("INFLUXDB_ORG").ok();
        let original_bucket = std::env::var("INFLUXDB_BUCKET").ok();

        std::env::set_var("INFLUXDB_URL", "http://localhost:8086");
        std::env::set_var("INFLUXDB_TOKEN", "token");
        std::env::set_var("INFLUXDB_ORG", "org");
        std::env::set_var("INFLUXDB_BUCKET", "bucket");

        let result = load_influx_config();

        // Restore original values
        match original_url {
            Some(val) => std::env::set_var("INFLUXDB_URL", val),
            None => std::env::remove_var("INFLUXDB_URL"),
        }
        match original_token {
            Some(val) => std::env::set_var("INFLUXDB_TOKEN", val),
            None => std::env::remove_var("INFLUXDB_TOKEN"),
        }
        match original_org {
            Some(val) => std::env::set_var("INFLUXDB_ORG", val),
            None => std::env::remove_var("INFLUXDB_ORG"),
        }
        match original_bucket {
            Some(val) => std::env::set_var("INFLUXDB_BUCKET", val),
            None => std::env::remove_var("INFLUXDB_BUCKET"),
        }

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
        // Temporarily clear INFLUXDB environment variables
        without_env_vars(
            &[
                "INFLUXDB_URL",
                "INFLUXDB_TOKEN",
                "INFLUXDB_ORG",
                "INFLUXDB_BUCKET",
            ],
            || {
                let result = load_influx_config();
                assert!(result.is_err());
                let err = result.unwrap_err();
                assert!(err.to_string().contains("Failed to load InfluxConfig"));
            },
        );
    }
}
