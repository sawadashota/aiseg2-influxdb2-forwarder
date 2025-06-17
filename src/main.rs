//! AiSEG2 to InfluxDB2 Forwarder
//!
//! This application collects energy monitoring metrics from Panasonic AiSEG2 systems
//! and forwards them to InfluxDB2 for storage and visualization.
//!
//! # Architecture
//!
//! The application runs two parallel collection loops:
//! - **Status collectors** (5-second interval): Real-time power and climate metrics
//! - **Total collectors** (60-second interval): Daily aggregated consumption metrics
//!
//! # Features
//!
//! - Automatic retry on task failure
//! - Graceful shutdown on SIGTERM/SIGINT
//! - Historical data backfill on startup
//! - Configurable collection intervals
//! - Timeout protection for hung tasks

mod aiseg;
mod circuit_breaker;
mod collector;
mod config;
mod error;
mod influxdb;
mod model;

#[cfg(test)]
mod test_utils;

use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig as CircuitConfig};
use crate::collector::circuit_protected::CircuitProtectedCollector;
use crate::model::{batch_collect_metrics, MetricCollector};
use chrono::{Local, NaiveTime};
use std::future::IntoFuture;
use std::ops::Sub;
use std::sync::Arc;
use tokio::signal::ctrl_c;
use tokio::signal::unix::{signal, SignalKind};
use tokio::task::JoinError;
use tokio::time;
use tokio::time::{sleep, Duration};

/// Application entry point.
///
/// Initializes configuration, sets up collectors, and manages the main event loop
/// with signal handling for graceful shutdown.
#[tokio::main]
async fn main() {
    let app_config = config::load_app_config().expect("Failed to load AppConfig");
    tracing_subscriber::fmt()
        .with_max_level(app_config.log_level())
        .init();

    let collector_config =
        Arc::new(config::load_collector_config().expect("Failed to load CollectorConfig"));
    let circuit_breaker_config =
        config::load_circuit_breaker_config().expect("Failed to load CircuitBreakerConfig");
    let influx_config = config::load_influx_config().expect("Failed to load InfluxConfig");
    let influx_client = Arc::new(influxdb::Client::new(influx_config));

    let aiseg_config = config::load_aiseg_config().expect("Failed to load AisegConfig");
    let aiseg_client = Arc::new(aiseg::Client::new(aiseg_config));

    // Convert circuit breaker config to internal format
    let circuit_config = CircuitConfig {
        failure_threshold: circuit_breaker_config.failure_threshold,
        recovery_timeout: Duration::from_secs(circuit_breaker_config.recovery_timeout_seconds),
        half_open_success_threshold: circuit_breaker_config.half_open_success_threshold,
        half_open_failure_threshold: circuit_breaker_config.half_open_failure_threshold,
    };

    // Helper to create circuit-protected collectors
    let create_protected_collector =
        |name: &str, collector: Box<dyn MetricCollector>| -> Box<dyn MetricCollector> {
            let circuit_breaker = CircuitBreaker::new(name.to_string(), circuit_config.clone());
            Box::new(CircuitProtectedCollector::new(
                name.to_string(),
                Arc::from(collector),
                circuit_breaker,
            ))
        };

    // Initialize collectors for daily totals (60-second interval)
    let total_collectors: Arc<Vec<Box<dyn MetricCollector>>> = Arc::new(vec![
        create_protected_collector(
            "DailyTotalMetricCollector",
            Box::new(aiseg::DailyTotalMetricCollector::new(Arc::clone(
                &aiseg_client,
            ))),
        ),
        create_protected_collector(
            "CircuitDailyTotalMetricCollector",
            Box::new(aiseg::CircuitDailyTotalMetricCollector::new(Arc::clone(
                &aiseg_client,
            ))),
        ),
    ]);

    // Initialize collectors for real-time status (5-second interval)
    let status_collectors: Arc<Vec<Box<dyn MetricCollector>>> = Arc::new(vec![
        create_protected_collector(
            "PowerMetricCollector",
            Box::new(aiseg::PowerMetricCollector::new(Arc::clone(&aiseg_client))),
        ),
        create_protected_collector(
            "ClimateMetricCollector",
            Box::new(aiseg::ClimateMetricCollector::new(Arc::clone(
                &aiseg_client,
            ))),
        ),
    ]);

    // Spawn background task to collect historical data
    tokio::spawn(collect_past_total(
        Arc::clone(&total_collectors),
        Arc::clone(&influx_client),
        collector_config.total_initial_days,
    ));

    // Factory functions for creating collector tasks
    // These allow easy task recreation after failures
    let create_collect_status_task = || -> tokio::task::JoinHandle<()> {
        let config = Arc::clone(&collector_config);
        tokio::spawn(create_collect_task(
            Arc::clone(&influx_client),
            Arc::clone(&status_collectors),
            Duration::from_secs(config.status_interval_sec),
            "status_collectors",
            config.task_timeout_seconds,
        ))
    };
    let create_collect_total_task = || -> tokio::task::JoinHandle<()> {
        let config = Arc::clone(&collector_config);
        tokio::spawn(create_collect_task(
            Arc::clone(&influx_client),
            Arc::clone(&total_collectors),
            Duration::from_secs(config.total_interval_sec),
            "total_collectors",
            config.task_timeout_seconds,
        ))
    };
    let mut collect_status_task = create_collect_status_task();
    let mut collect_total_task = create_collect_total_task();

    let mut sig_term = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    tracing::info!("Running... Press Ctrl-C or send SIGTERM to terminate.");
    // Main event loop with signal handling and task supervision
    loop {
        tokio::select! {
            // Handle SIGTERM for graceful shutdown in containers
            _ = sig_term.recv() => {
                tracing::info!("Received SIGTERM. Exiting...");
                break;
            }
            // Handle Ctrl-C for manual termination
            _ = ctrl_c() => {
                tracing::info!("Received SIGINT. Exiting...");
                break;
            }
            // Monitor status collector task and restart on failure
            result = &mut collect_status_task => {
                handle_task_result("status_collectors", result);
                collect_status_task = create_collect_status_task();
            }
            // Monitor total collector task and restart on failure
            result = &mut collect_total_task => {
                handle_task_result("total_collectors", result);
                collect_total_task = create_collect_total_task();
            }
        }
    }
}

/// Wraps a future with a timeout to prevent tasks from hanging indefinitely.
///
/// # Arguments
///
/// * `task_name` - Name of the task for logging purposes
/// * `future` - The future to execute with timeout protection
///
/// # Behavior
///
/// - Timeout duration is configurable via the timeout_seconds parameter
/// - Logs an error if the task times out but doesn't propagate the error
/// - Used to prevent collector tasks from blocking the main loop
async fn with_timeout<F>(task_name: &'static str, future: F, timeout_seconds: u64)
where
    F: IntoFuture,
{
    let timeout_duration = Duration::from_secs(timeout_seconds);

    match time::timeout(timeout_duration, future).await {
        Ok(_) => {}
        Err(_) => tracing::error!("Task {} timed out.", task_name),
    }
}

/// Creates and executes a single metric collection cycle.
///
/// This function:
/// 1. Collects metrics from all provided collectors
/// 2. Writes the metrics to InfluxDB
/// 3. Sleeps for the specified interval
///
/// # Arguments
///
/// * `influx_client` - Shared InfluxDB client for writing metrics
/// * `collectors` - List of metric collectors to execute
/// * `interval` - Duration to sleep after collection completes
/// * `task_name` - Name of the task for logging purposes
///
/// # Error Handling
///
/// - Collection errors from individual collectors are logged but don't stop other collectors
/// - InfluxDB write errors are logged but don't crash the task
/// - The entire operation is wrapped in a timeout to prevent hanging
async fn create_collect_task(
    influx_client: Arc<influxdb::Client>,
    collectors: Arc<Vec<Box<dyn MetricCollector>>>,
    interval: Duration,
    task_name: &'static str,
    timeout_seconds: u64,
) {
    with_timeout(
        task_name,
        async {
            let points = batch_collect_metrics(&collectors, Local::now()).await;

            for point in &points {
                tracing::debug!("{:?}", point);
            }

            match influx_client.write(points).await {
                Ok(_) => tracing::info!("Successfully wrote points to InfluxDB ({})", task_name),
                Err(e) => tracing::error!(
                    "Failed to write points to InfluxDB ({}): {:?}",
                    task_name,
                    e
                ),
            }
        },
        timeout_seconds,
    )
    .await;
    sleep(interval).await;
}

/// Handles the result of a tokio task, logging success or failure.
///
/// # Arguments
///
/// * `task_name` - Name of the task for logging
/// * `result` - The JoinHandle result from the completed task
///
/// # Behavior
///
/// - Success is logged at debug level
/// - Failures (panics, cancellation) are logged at error level
/// - Used in the main loop to detect and log task crashes before restarting
fn handle_task_result(task_name: &str, result: Result<(), JoinError>) {
    match result {
        Ok(_) => {
            tracing::debug!("Task {} completed.", task_name);
        }
        Err(e) => {
            tracing::error!("Task {} failed: {:?}", task_name, e);
        }
    }
}

/// Collects and stores historical daily total metrics.
///
/// This function runs once at startup to backfill historical data for the
/// specified number of days. It's useful for populating graphs when the
/// forwarder is first deployed or after downtime.
///
/// # Arguments
///
/// * `collectors` - Total metric collectors (daily aggregates)
/// * `influx_client` - InfluxDB client for writing historical data
/// * `days` - Number of past days to collect (1 = yesterday only)
///
/// # Behavior
///
/// - Iterates from `days` ago to yesterday (excludes today)
/// - Each day's timestamp is normalized to midnight
/// - Continues even if individual days fail
/// - Logs progress for each day processed
async fn collect_past_total(
    collectors: Arc<Vec<Box<dyn MetricCollector>>>,
    influx_client: Arc<influxdb::Client>,
    days: u64,
) {
    tracing::info!("Inserting last {} days...", days);
    for i in 1..=days {
        let timestamp = match Local::now()
            .sub(Duration::from_secs(i * 24 * 60 * 60))
            .with_time(NaiveTime::default())
            .single()
        {
            Some(ts) => ts,
            None => {
                tracing::error!("Failed to set timestamp to midnight for day {} days ago", i);
                continue;
            }
        };
        let points = batch_collect_metrics(&collectors, timestamp).await;

        for point in &points {
            tracing::debug!("{:?}", point);
        }

        match influx_client.write(points).await {
            Ok(_) => tracing::info!(
                "Successfully wrote points to InfluxDB: day={}",
                timestamp.format("%Y-%m-%d")
            ),
            Err(e) => tracing::error!("Failed to write points to InfluxDB: {:?}", e),
        }
    }
    tracing::info!("Finished inserting last {} days.", days);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{config::test_influx_config, mocks::MockMetricCollector};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tokio::time::Duration;

    // Mock implementations are now in test_utils::mocks

    mod with_timeout {
        use super::*;

        #[tokio::test]
        async fn succeeds() {
            // Task completes within timeout
            let completed = Arc::new(AtomicBool::new(false));
            let completed_clone = completed.clone();

            with_timeout(
                "test_task",
                async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    completed_clone.store(true, Ordering::SeqCst);
                },
                10,
            )
            .await;

            assert!(completed.load(Ordering::SeqCst));
        }

        #[tokio::test]
        async fn fails() {
            // Task exceeds timeout - this will log an error
            let completed = Arc::new(AtomicBool::new(false));
            let completed_clone = completed.clone();

            with_timeout(
                "test_task",
                async move {
                    tokio::time::sleep(Duration::from_secs(15)).await;
                    completed_clone.store(true, Ordering::SeqCst);
                },
                10,
            )
            .await;

            // Task should not complete due to timeout
            assert!(!completed.load(Ordering::SeqCst));
        }
    }

    mod handle_task_result {
        use super::*;
        use tokio::task::JoinError;

        #[test]
        fn succeeds() {
            // Test successful task completion
            let result: Result<(), JoinError> = Ok(());
            handle_task_result("test_task", result);
            // Function should complete without panic
        }

        #[tokio::test]
        async fn fails() {
            // Test task failure
            let handle = tokio::spawn(async {
                panic!("Task panicked");
            });

            // Wait for the task to panic
            let result = handle.await;

            handle_task_result("test_task", result);
            // Function should handle the error without panic
        }
    }

    mod collect_past_total {
        use super::*;

        #[tokio::test]
        async fn succeeds() {
            // Mock successful collection and write
            let collectors: Arc<Vec<Box<dyn MetricCollector>>> =
                Arc::new(vec![Box::new(MockMetricCollector::new_success())]);

            let influx_config = test_influx_config();
            let influx_client = Arc::new(influxdb::Client::new(influx_config));

            // We can't easily test the actual write without a real InfluxDB instance,
            // so we'll just verify the function completes without panic
            collect_past_total(collectors, influx_client, 1).await;
        }

        #[tokio::test]
        async fn fails() {
            // Mock failed collection
            let collectors: Arc<Vec<Box<dyn MetricCollector>>> = Arc::new(vec![Box::new(
                MockMetricCollector::new_failure("Mock collection failed"),
            )]);

            let influx_config = test_influx_config();
            let influx_client = Arc::new(influxdb::Client::new(influx_config));

            // Function should handle collection failures gracefully
            collect_past_total(collectors, influx_client, 1).await;
        }
    }

    mod create_collect_task {
        use super::*;

        #[tokio::test]
        async fn succeeds() {
            // Mock successful collection and write
            let collectors: Arc<Vec<Box<dyn MetricCollector>>> =
                Arc::new(vec![Box::new(MockMetricCollector::new_success())]);

            let influx_config = test_influx_config();
            let influx_client = Arc::new(influxdb::Client::new(influx_config));

            // Run once with minimal interval
            create_collect_task(
                influx_client,
                collectors,
                Duration::from_millis(1),
                "test_task",
                10,
            )
            .await;
        }

        #[tokio::test]
        async fn fails() {
            // Test collection failure
            let collectors: Arc<Vec<Box<dyn MetricCollector>>> = Arc::new(vec![Box::new(
                MockMetricCollector::new_failure("Mock collection failed"),
            )]);

            let influx_config = test_influx_config();
            let influx_client = Arc::new(influxdb::Client::new(influx_config));

            // Function should handle failures gracefully
            create_collect_task(
                influx_client,
                collectors,
                Duration::from_millis(1),
                "test_task_fails",
                10,
            )
            .await;
        }
    }
}
