use crate::circuit_breaker::CircuitBreaker;
use crate::error::{CollectorError, Result};
use crate::model::{DataPointBuilder, MetricCollector};
use chrono::{DateTime, Local};
use std::sync::Arc;

/// A wrapper that adds circuit breaker protection to any MetricCollector.
///
/// This decorator pattern allows existing collectors to be wrapped with
/// circuit breaker functionality without modifying their implementation.
pub struct CircuitProtectedCollector {
    /// The underlying collector being protected
    inner: Arc<dyn MetricCollector>,
    /// Circuit breaker instance for this collector
    circuit_breaker: CircuitBreaker,
    /// Name of this collector for logging
    name: String,
}

impl CircuitProtectedCollector {
    /// Creates a new circuit-protected collector.
    ///
    /// # Arguments
    /// * `name` - A descriptive name for this collector (used in logging)
    /// * `inner` - The underlying collector to protect
    /// * `circuit_breaker` - The circuit breaker instance to use
    pub fn new(
        name: String,
        inner: Arc<dyn MetricCollector>,
        circuit_breaker: CircuitBreaker,
    ) -> Self {
        Self {
            inner,
            circuit_breaker,
            name,
        }
    }
}

#[async_trait::async_trait]
impl MetricCollector for CircuitProtectedCollector {
    /// Collects metrics with circuit breaker protection.
    ///
    /// If the circuit is open, returns an empty result without calling the underlying collector.
    /// Records successes and failures to update the circuit breaker state.
    async fn collect(&self, timestamp: DateTime<Local>) -> Result<Vec<Box<dyn DataPointBuilder>>, CollectorError> {
        // Check if the circuit allows this call
        if !self.circuit_breaker.call_allowed().await {
            tracing::debug!(
                collector = %self.name,
                "Circuit breaker is open, skipping collection"
            );
            // Instead of returning an error, return empty to maintain stability
            return Ok(vec![]);
        }

        // Attempt to collect metrics
        match self.inner.collect(timestamp).await {
            Ok(data_points) => {
                // Record success
                self.circuit_breaker.record_success().await;
                tracing::trace!(
                    collector = %self.name,
                    count = data_points.len(),
                    "Collection succeeded"
                );
                Ok(data_points)
            }
            Err(err) => {
                // Record failure
                self.circuit_breaker.record_failure().await;
                tracing::warn!(
                    collector = %self.name,
                    error = %err,
                    "Collection failed, circuit breaker recorded failure"
                );

                // Check if circuit is now open
                if self.circuit_breaker.is_open().await {
                    tracing::warn!(
                        collector = %self.name,
                        "Circuit breaker is now OPEN due to repeated failures"
                    );
                }

                // Return empty result to maintain system stability
                Ok(vec![])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit_breaker::CircuitBreakerConfig;
    use crate::test_utils::mocks::collectors::MockMetricCollector;
    use std::time::Duration;

    #[tokio::test]
    async fn test_circuit_protected_collector_success() {
        let mock_collector = MockMetricCollector::new_success();

        let circuit_config = CircuitBreakerConfig::default();
        let circuit_breaker = CircuitBreaker::new("test".to_string(), circuit_config);

        let protected = CircuitProtectedCollector::new(
            "test_collector".to_string(),
            Arc::new(mock_collector),
            circuit_breaker,
        );

        let result = protected.collect(Local::now()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1); // MockMetricCollector returns one item by default
    }

    #[tokio::test]
    async fn test_circuit_protected_collector_opens_on_failures() {
        let mock_collector = MockMetricCollector::new_failure("Connection failed");

        let circuit_config = CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        };
        let circuit_breaker = CircuitBreaker::new("test".to_string(), circuit_config);

        let protected = CircuitProtectedCollector::new(
            "test_collector".to_string(),
            Arc::new(mock_collector),
            circuit_breaker.clone(),
        );

        // First failure
        let result = protected.collect(Local::now()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
        assert!(!circuit_breaker.is_open().await);

        // Second failure should open the circuit
        let result = protected.collect(Local::now()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
        assert!(circuit_breaker.is_open().await);

        // Third call should be blocked by open circuit
        let result = protected.collect(Local::now()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_circuit_protected_collector_recovery() {
        // We'll use two collectors: one that fails, one that succeeds
        let fail_collector = MockMetricCollector::new_failure("Connection failed");
        let success_collector = MockMetricCollector::new_success();

        let circuit_config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(50),
            half_open_success_threshold: 1,
            ..Default::default()
        };

        // First, open the circuit with failures
        let circuit_breaker = CircuitBreaker::new("test".to_string(), circuit_config.clone());
        let protected_fail = CircuitProtectedCollector::new(
            "test_collector".to_string(),
            Arc::new(fail_collector),
            circuit_breaker.clone(),
        );

        // Two failures to open the circuit
        protected_fail.collect(Local::now()).await.unwrap();
        protected_fail.collect(Local::now()).await.unwrap();
        assert!(circuit_breaker.is_open().await);

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Now use a successful collector with the same circuit breaker
        let protected_success = CircuitProtectedCollector::new(
            "test_collector".to_string(),
            Arc::new(success_collector),
            circuit_breaker.clone(),
        );

        // Next call should succeed and close the circuit
        let result = protected_success.collect(Local::now()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
        assert!(!circuit_breaker.is_open().await);
    }
}
