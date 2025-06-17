use anyhow::{anyhow, Result};
use serde_derive::Deserialize;
use std::str::FromStr;

/// Provides the default log level when not specified in environment.
///
/// Returns "info" as the default logging level.
fn default_log_level() -> String {
    "info".to_string()
}

/// Application-wide configuration settings.
///
/// Controls general application behavior such as logging level.
/// Loaded from environment variables without prefix.
#[derive(Deserialize, Debug)]
pub struct AppConfig {
    /// Logging level (trace, debug, info, warn, error)
    /// Defaults to "info" if not specified
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl AppConfig {
    /// Converts the string log level to a tracing::Level enum.
    ///
    /// Falls back to INFO level if the string cannot be parsed.
    pub fn log_level(&self) -> tracing::Level {
        tracing::Level::from_str(self.log_level.as_str()).unwrap_or(tracing::Level::INFO)
    }
}

/// Loads application configuration from environment variables.
///
/// Reads environment variables:
/// - `LOG_LEVEL`: Sets the logging level (default: "info")
///
/// # Returns
/// - `Ok(AppConfig)` if configuration loads successfully
/// - `Err` if required environment variables are missing or invalid
pub(crate) fn load_app_config() -> Result<AppConfig> {
    match envy::from_env::<AppConfig>() {
        Ok(config) => Ok(config),
        Err(err) => Err(anyhow!("Failed to load AppConfig: {}", err)),
    }
}

/// Default interval for collecting real-time status metrics (5 seconds).
fn default_status_interval_sec() -> u64 {
    5
}

/// Default interval for collecting daily total metrics (60 seconds).
fn default_total_interval_sec() -> u64 {
    60
}

/// Default number of days to collect historical data on startup (30 days).
fn default_total_initial_days() -> u64 {
    30
}

/// Default timeout for collector tasks in seconds (10 seconds).
fn default_task_timeout_seconds() -> u64 {
    10
}

/// Default number of consecutive failures before opening circuit (5).
fn default_circuit_breaker_failure_threshold() -> u32 {
    5
}

/// Default recovery timeout in seconds (60).
fn default_circuit_breaker_recovery_timeout_seconds() -> u64 {
    60
}

/// Default number of successful calls to close circuit from half-open (3).
fn default_circuit_breaker_half_open_success_threshold() -> u32 {
    3
}

/// Default number of failures allowed in half-open before reopening (1).
fn default_circuit_breaker_half_open_failure_threshold() -> u32 {
    1
}

/// Configuration for metric collection intervals and behavior.
///
/// Controls how frequently different types of metrics are collected
/// from the AiSEG2 system. Loaded from environment variables with
/// COLLECTOR_ prefix.
#[derive(Deserialize, Debug)]
pub struct CollectorConfig {
    /// Interval for collecting real-time status metrics (power, climate)
    /// Default: 5 seconds
    #[serde(default = "default_status_interval_sec")]
    pub status_interval_sec: u64,

    /// Interval for collecting daily total metrics
    /// Default: 60 seconds
    #[serde(default = "default_total_interval_sec")]
    pub total_interval_sec: u64,

    /// Number of past days to collect when starting up
    /// Used to backfill historical data on first run
    /// Default: 30 days
    #[serde(default = "default_total_initial_days")]
    pub total_initial_days: u64,

    /// Timeout for individual collector tasks in seconds
    /// Prevents collector tasks from hanging indefinitely
    /// Default: 10 seconds
    #[serde(default = "default_task_timeout_seconds")]
    pub task_timeout_seconds: u64,
}

/// Configuration for circuit breaker behavior.
///
/// Controls how the circuit breaker responds to failures to prevent
/// cascading failures and reduce load on failing systems.
/// Loaded from environment variables with CIRCUIT_BREAKER_ prefix.
#[derive(Deserialize, Debug)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening circuit
    /// Default: 5
    #[serde(default = "default_circuit_breaker_failure_threshold")]
    pub failure_threshold: u32,

    /// How long to wait before attempting recovery (in seconds)
    /// Default: 60
    #[serde(default = "default_circuit_breaker_recovery_timeout_seconds")]
    pub recovery_timeout_seconds: u64,

    /// Number of successful calls needed to close circuit from half-open
    /// Default: 3
    #[serde(default = "default_circuit_breaker_half_open_success_threshold")]
    pub half_open_success_threshold: u32,

    /// Number of failures allowed in half-open before reopening
    /// Default: 1
    #[serde(default = "default_circuit_breaker_half_open_failure_threshold")]
    pub half_open_failure_threshold: u32,
}

/// Loads collector configuration from environment variables.
///
/// Reads environment variables with COLLECTOR_ prefix:
/// - `COLLECTOR_STATUS_INTERVAL_SEC`: Interval for status metrics (default: 5)
/// - `COLLECTOR_TOTAL_INTERVAL_SEC`: Interval for total metrics (default: 60)
/// - `COLLECTOR_TOTAL_INITIAL_DAYS`: Days of history to collect (default: 30)
/// - `COLLECTOR_TASK_TIMEOUT_SECONDS`: Timeout for collector tasks (default: 10)
///
/// # Returns
/// - `Ok(CollectorConfig)` with loaded or default values
/// - `Err` if environment variables contain invalid values
pub fn load_collector_config() -> Result<CollectorConfig> {
    match envy::prefixed("COLLECTOR_").from_env::<CollectorConfig>() {
        Ok(config) => Ok(config),
        Err(err) => Err(anyhow!("Failed to load CollectorConfig: {}", err)),
    }
}

/// Configuration for connecting to the AiSEG2 system.
///
/// Contains credentials and connection details for the
/// Panasonic AiSEG2 energy monitoring system.
/// Loaded from environment variables with AISEG2_ prefix.
#[derive(Deserialize, Debug)]
pub struct Aiseg2Config {
    /// Base URL of the AiSEG2 system (e.g., "http://192.168.1.100")
    pub url: String,
    /// Username for AiSEG2 authentication
    pub user: String,
    /// Password for AiSEG2 authentication
    pub password: String,
}

/// Loads AiSEG2 configuration from environment variables.
///
/// Reads required environment variables with AISEG2_ prefix:
/// - `AISEG2_URL`: The base URL of the AiSEG2 system
/// - `AISEG2_USER`: Username for authentication
/// - `AISEG2_PASSWORD`: Password for authentication
///
/// # Returns
/// - `Ok(Aiseg2Config)` if all required variables are present
/// - `Err` if any required variables are missing
pub(crate) fn load_aiseg_config() -> Result<Aiseg2Config> {
    match envy::prefixed("AISEG2_").from_env::<Aiseg2Config>() {
        Ok(config) => Ok(config),
        Err(err) => Err(anyhow!("Failed to load AisegConfig: {}", err)),
    }
}

/// Configuration for connecting to InfluxDB 2.x.
///
/// Contains all necessary parameters for establishing
/// a connection to InfluxDB and writing metrics.
/// Loaded from environment variables with INFLUXDB_ prefix.
#[derive(Deserialize, Debug)]
pub struct InfluxConfig {
    /// InfluxDB server URL (e.g., "http://localhost:8086")
    pub url: String,
    /// Authentication token with write permissions
    pub token: String,
    /// InfluxDB organization name
    pub org: String,
    /// Target bucket for storing metrics
    pub bucket: String,
}

/// Loads InfluxDB configuration from environment variables.
///
/// Reads required environment variables with INFLUXDB_ prefix:
/// - `INFLUXDB_URL`: The InfluxDB server URL
/// - `INFLUXDB_TOKEN`: Authentication token
/// - `INFLUXDB_ORG`: Organization name
/// - `INFLUXDB_BUCKET`: Target bucket name
///
/// # Returns
/// - `Ok(InfluxConfig)` if all required variables are present
/// - `Err` if any required variables are missing
pub fn load_influx_config() -> Result<InfluxConfig> {
    match envy::prefixed("INFLUXDB_").from_env::<InfluxConfig>() {
        Ok(config) => Ok(config),
        Err(err) => Err(anyhow!("Failed to load InfluxConfig: {}", err)),
    }
}

/// Loads circuit breaker configuration from environment variables.
///
/// Reads environment variables with CIRCUIT_BREAKER_ prefix:
/// - `CIRCUIT_BREAKER_FAILURE_THRESHOLD`: Failures before opening (default: 5)
/// - `CIRCUIT_BREAKER_RECOVERY_TIMEOUT_SECONDS`: Recovery timeout (default: 60)
/// - `CIRCUIT_BREAKER_HALF_OPEN_SUCCESS_THRESHOLD`: Successes to close (default: 3)
/// - `CIRCUIT_BREAKER_HALF_OPEN_FAILURE_THRESHOLD`: Failures to reopen (default: 1)
///
/// # Returns
/// - `Ok(CircuitBreakerConfig)` with loaded or default values
/// - `Err` if environment variables contain invalid values
pub fn load_circuit_breaker_config() -> Result<CircuitBreakerConfig> {
    match envy::prefixed("CIRCUIT_BREAKER_").from_env::<CircuitBreakerConfig>() {
        Ok(config) => Ok(config),
        Err(err) => Err(anyhow!("Failed to load CircuitBreakerConfig: {}", err)),
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
        let original_timeout = std::env::var("COLLECTOR_TASK_TIMEOUT_SECONDS").ok();

        std::env::set_var("COLLECTOR_TOTAL_INTERVAL_SEC", "10");
        std::env::set_var("COLLECTOR_STATUS_INTERVAL_SEC", "20");
        std::env::set_var("COLLECTOR_TOTAL_INITIAL_DAYS", "30");
        std::env::set_var("COLLECTOR_TASK_TIMEOUT_SECONDS", "15");

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
        match original_timeout {
            Some(val) => std::env::set_var("COLLECTOR_TASK_TIMEOUT_SECONDS", val),
            None => std::env::remove_var("COLLECTOR_TASK_TIMEOUT_SECONDS"),
        }

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.total_interval_sec, 10);
        assert_eq!(config.status_interval_sec, 20);
        assert_eq!(config.total_initial_days, 30);
        assert_eq!(config.task_timeout_seconds, 15);
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
        assert_eq!(config.task_timeout_seconds, 10);
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

    #[test]
    #[serial]
    fn test_load_circuit_breaker_config() {
        // Save original values
        let keys = [
            "CIRCUIT_BREAKER_FAILURE_THRESHOLD",
            "CIRCUIT_BREAKER_RECOVERY_TIMEOUT_SECONDS",
            "CIRCUIT_BREAKER_HALF_OPEN_SUCCESS_THRESHOLD",
            "CIRCUIT_BREAKER_HALF_OPEN_FAILURE_THRESHOLD",
        ];
        let originals: Vec<(String, Result<String, VarError>)> = keys
            .iter()
            .map(|&key| (key.to_string(), std::env::var(key)))
            .collect();

        // Set test values
        std::env::set_var("CIRCUIT_BREAKER_FAILURE_THRESHOLD", "3");
        std::env::set_var("CIRCUIT_BREAKER_RECOVERY_TIMEOUT_SECONDS", "30");
        std::env::set_var("CIRCUIT_BREAKER_HALF_OPEN_SUCCESS_THRESHOLD", "2");
        std::env::set_var("CIRCUIT_BREAKER_HALF_OPEN_FAILURE_THRESHOLD", "2");

        let result = load_circuit_breaker_config();

        // Restore original values
        for (key, original) in originals {
            match original {
                Ok(val) => std::env::set_var(&key, val),
                Err(_) => std::env::remove_var(&key),
            }
        }

        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.failure_threshold, 3);
        assert_eq!(config.recovery_timeout_seconds, 30);
        assert_eq!(config.half_open_success_threshold, 2);
        assert_eq!(config.half_open_failure_threshold, 2);
    }

    #[test]
    #[serial]
    fn test_load_circuit_breaker_config_defaults() {
        // Temporarily clear circuit breaker environment variables
        without_env_vars(
            &[
                "CIRCUIT_BREAKER_FAILURE_THRESHOLD",
                "CIRCUIT_BREAKER_RECOVERY_TIMEOUT_SECONDS",
                "CIRCUIT_BREAKER_HALF_OPEN_SUCCESS_THRESHOLD",
                "CIRCUIT_BREAKER_HALF_OPEN_FAILURE_THRESHOLD",
            ],
            || {
                let result = load_circuit_breaker_config();
                assert!(result.is_ok());
                let config = result.unwrap();
                assert_eq!(config.failure_threshold, 5);
                assert_eq!(config.recovery_timeout_seconds, 60);
                assert_eq!(config.half_open_success_threshold, 3);
                assert_eq!(config.half_open_failure_threshold, 1);
            },
        );
    }
}
