//! Mock implementations of MetricCollector for testing.

use crate::model::{DataPointBuilder, Measurement, MetricCollector, PowerStatusMetric};
use async_trait::async_trait;
use chrono::{DateTime, Local};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A mock metric collector that can be configured to succeed or fail.
pub struct MockMetricCollector {
    should_fail: bool,
    error_message: String,
    create_data: Box<dyn Fn() -> Vec<Box<dyn DataPointBuilder>> + Send + Sync>,
}

impl MockMetricCollector {
    /// Creates a new mock collector that succeeds.
    pub fn new_success() -> Self {
        Self {
            should_fail: false,
            error_message: String::new(),
            create_data: Box::new(|| {
                vec![Box::new(PowerStatusMetric {
                    measurement: Measurement::Power,
                    name: "test".to_string(),
                    value: 100,
                })]
            }),
        }
    }

    /// Creates a new mock collector that fails with the given error message.
    pub fn new_failure(error_message: impl Into<String>) -> Self {
        Self {
            should_fail: true,
            error_message: error_message.into(),
            create_data: Box::new(|| Vec::new()),
        }
    }

    /// Creates a new mock collector with custom success data.
    pub fn new_with_data<F>(create_fn: F) -> Self
    where
        F: Fn() -> Vec<Box<dyn DataPointBuilder>> + Send + Sync + 'static,
    {
        Self {
            should_fail: false,
            error_message: String::new(),
            create_data: Box::new(create_fn),
        }
    }
}

#[async_trait]
impl MetricCollector for MockMetricCollector {
    async fn collect(
        &self,
        _timestamp: DateTime<Local>,
    ) -> anyhow::Result<Vec<Box<dyn DataPointBuilder>>> {
        if self.should_fail {
            anyhow::bail!(self.error_message.clone())
        } else {
            Ok((self.create_data)())
        }
    }
}

/// A mock collector that tracks the number of times it has been called.
pub struct CountingMockCollector {
    call_count: Arc<AtomicUsize>,
    should_fail: bool,
}

impl CountingMockCollector {
    /// Creates a new counting mock collector.
    pub fn new(should_fail: bool) -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
            should_fail,
        }
    }

    /// Gets the number of times this collector has been called.
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl MetricCollector for CountingMockCollector {
    async fn collect(
        &self,
        _timestamp: DateTime<Local>,
    ) -> anyhow::Result<Vec<Box<dyn DataPointBuilder>>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        if self.should_fail {
            anyhow::bail!("Mock collector configured to fail")
        } else {
            Ok(Vec::new())
        }
    }
}

/// A mock collector that can be configured to timeout.
pub struct TimeoutMockCollector {
    timeout_duration: std::time::Duration,
}

impl TimeoutMockCollector {
    /// Creates a new mock collector that will timeout after the specified duration.
    pub fn new(timeout_duration: std::time::Duration) -> Self {
        Self { timeout_duration }
    }
}

#[async_trait]
impl MetricCollector for TimeoutMockCollector {
    async fn collect(
        &self,
        _timestamp: DateTime<Local>,
    ) -> anyhow::Result<Vec<Box<dyn DataPointBuilder>>> {
        tokio::time::sleep(self.timeout_duration).await;
        Ok(Vec::new())
    }
}

/// A mock collector that returns different results based on the timestamp.
pub struct TimeSensitiveMockCollector {
    results: Vec<(
        DateTime<Local>,
        Box<dyn Fn() -> Vec<Box<dyn DataPointBuilder>> + Send + Sync>,
    )>,
}

impl TimeSensitiveMockCollector {
    /// Creates a new time-sensitive mock collector.
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Adds a result for a specific timestamp.
    pub fn add_result<F>(mut self, timestamp: DateTime<Local>, create_fn: F) -> Self
    where
        F: Fn() -> Vec<Box<dyn DataPointBuilder>> + Send + Sync + 'static,
    {
        self.results.push((timestamp, Box::new(create_fn)));
        self
    }
}

#[async_trait]
impl MetricCollector for TimeSensitiveMockCollector {
    async fn collect(
        &self,
        timestamp: DateTime<Local>,
    ) -> anyhow::Result<Vec<Box<dyn DataPointBuilder>>> {
        for (ts, create_fn) in &self.results {
            if *ts == timestamp {
                return Ok(create_fn());
            }
        }
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_success_collector() {
        let collector = MockMetricCollector::new_success();
        let result = collector.collect(Local::now()).await;
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.len(), 1); // Should have default test data
    }

    #[tokio::test]
    async fn test_mock_failure_collector() {
        let collector = MockMetricCollector::new_failure("Test error");
        let result = collector.collect(Local::now()).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Test error"));
        }
    }

    #[tokio::test]
    async fn test_counting_mock_collector() {
        let collector = CountingMockCollector::new(false);

        assert_eq!(collector.call_count(), 0);

        let _ = collector.collect(Local::now()).await;
        assert_eq!(collector.call_count(), 1);

        let _ = collector.collect(Local::now()).await;
        assert_eq!(collector.call_count(), 2);
    }

    #[tokio::test]
    async fn test_timeout_mock_collector() {
        let collector = TimeoutMockCollector::new(std::time::Duration::from_millis(10));

        let start = std::time::Instant::now();
        let _ = collector.collect(Local::now()).await;
        let elapsed = start.elapsed();

        assert!(elapsed >= std::time::Duration::from_millis(10));
    }
}
