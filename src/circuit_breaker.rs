use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Circuit breaker state machine for handling failures.
#[derive(Debug, Clone)]
pub enum CircuitState {
    /// Normal operation - all calls pass through
    Closed {
        /// Number of consecutive failures
        failure_count: u32,
    },
    /// Circuit is open - all calls fail fast
    Open {
        /// When the circuit was opened
        opened_at: Instant,
    },
    /// Testing if the service has recovered
    HalfOpen {
        /// Number of successful calls in half-open state
        success_count: u32,
        /// Number of failed calls in half-open state
        failure_count: u32,
    },
}

/// Configuration for circuit breaker behavior.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening circuit
    pub failure_threshold: u32,
    /// How long to wait before attempting recovery
    pub recovery_timeout: Duration,
    /// Maximum successful calls needed to close circuit from half-open
    pub half_open_success_threshold: u32,
    /// Maximum failures allowed in half-open before reopening
    pub half_open_failure_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(60),
            half_open_success_threshold: 3,
            half_open_failure_threshold: 1,
        }
    }
}

/// Circuit breaker for protecting against cascading failures.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitState>>,
    config: CircuitBreakerConfig,
    name: String,
}

impl CircuitBreaker {
    /// Creates a new circuit breaker with the given configuration.
    pub fn new(name: String, config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitState::Closed { failure_count: 0 })),
            config,
            name,
        }
    }

    /// Checks if a call is allowed through the circuit breaker.
    pub async fn call_allowed(&self) -> bool {
        let mut state = self.state.lock().await;

        match &*state {
            CircuitState::Closed { .. } => true,
            CircuitState::Open { opened_at } => {
                // Check if recovery timeout has elapsed
                if opened_at.elapsed() >= self.config.recovery_timeout {
                    tracing::info!(
                        circuit_breaker = %self.name,
                        "Circuit breaker transitioning from Open to HalfOpen"
                    );
                    *state = CircuitState::HalfOpen {
                        success_count: 0,
                        failure_count: 0,
                    };
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen { .. } => true,
        }
    }

    /// Records a successful call.
    pub async fn record_success(&self) {
        let mut state = self.state.lock().await;

        match &*state {
            CircuitState::Closed { .. } => {
                // Reset failure count on success
                *state = CircuitState::Closed { failure_count: 0 };
            }
            CircuitState::HalfOpen { success_count, .. } => {
                let new_success_count = success_count + 1;

                if new_success_count >= self.config.half_open_success_threshold {
                    tracing::info!(
                        circuit_breaker = %self.name,
                        "Circuit breaker transitioning from HalfOpen to Closed"
                    );
                    *state = CircuitState::Closed { failure_count: 0 };
                } else {
                    *state = CircuitState::HalfOpen {
                        success_count: new_success_count,
                        failure_count: 0,
                    };
                }
            }
            CircuitState::Open { .. } => {
                // This shouldn't happen, but handle gracefully
                tracing::warn!(
                    circuit_breaker = %self.name,
                    "Success recorded while circuit is Open"
                );
            }
        }
    }

    /// Records a failed call.
    pub async fn record_failure(&self) {
        let mut state = self.state.lock().await;

        match &*state {
            CircuitState::Closed { failure_count } => {
                let new_failure_count = failure_count + 1;

                if new_failure_count >= self.config.failure_threshold {
                    tracing::warn!(
                        circuit_breaker = %self.name,
                        failure_count = new_failure_count,
                        "Circuit breaker opening due to excessive failures"
                    );
                    *state = CircuitState::Open {
                        opened_at: Instant::now(),
                    };
                } else {
                    *state = CircuitState::Closed {
                        failure_count: new_failure_count,
                    };
                }
            }
            CircuitState::HalfOpen {
                success_count,
                failure_count,
            } => {
                let new_failure_count = failure_count + 1;

                if new_failure_count >= self.config.half_open_failure_threshold {
                    tracing::warn!(
                        circuit_breaker = %self.name,
                        "Circuit breaker reopening from HalfOpen due to failures"
                    );
                    *state = CircuitState::Open {
                        opened_at: Instant::now(),
                    };
                } else {
                    *state = CircuitState::HalfOpen {
                        success_count: *success_count,
                        failure_count: new_failure_count,
                    };
                }
            }
            CircuitState::Open { .. } => {
                // This shouldn't happen, but handle gracefully
                tracing::warn!(
                    circuit_breaker = %self.name,
                    "Failure recorded while circuit is already Open"
                );
            }
        }
    }

    /// Checks if the circuit is currently open.
    pub async fn is_open(&self) -> bool {
        matches!(*self.state.lock().await, CircuitState::Open { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test".to_string(), config);

        // Circuit should start closed
        assert!(breaker.call_allowed().await);
        assert!(!breaker.is_open().await);

        // Record failures up to threshold
        for i in 1..=3 {
            breaker.record_failure().await;
            if i < 3 {
                assert!(breaker.call_allowed().await);
                assert!(!breaker.is_open().await);
            }
        }

        // Circuit should now be open
        assert!(!breaker.call_allowed().await);
        assert!(breaker.is_open().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_closes_on_success() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            half_open_success_threshold: 2,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test".to_string(), config);

        // Open the circuit
        breaker.record_failure().await;
        breaker.record_failure().await;
        assert!(breaker.is_open().await);

        // Manually transition to half-open for testing
        *breaker.state.lock().await = CircuitState::HalfOpen {
            success_count: 0,
            failure_count: 0,
        };

        // Record successes
        assert!(breaker.call_allowed().await);
        breaker.record_success().await;
        assert!(breaker.call_allowed().await);
        breaker.record_success().await;

        // Circuit should now be closed
        assert!(!breaker.is_open().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reopens_from_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            half_open_failure_threshold: 1,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test".to_string(), config);

        // Open the circuit
        breaker.record_failure().await;
        breaker.record_failure().await;
        assert!(breaker.is_open().await);

        // Manually transition to half-open for testing
        *breaker.state.lock().await = CircuitState::HalfOpen {
            success_count: 0,
            failure_count: 0,
        };

        // Record a failure in half-open state
        assert!(breaker.call_allowed().await);
        breaker.record_failure().await;

        // Circuit should reopen
        assert!(breaker.is_open().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_recovery_timeout() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            recovery_timeout: Duration::from_millis(50),
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test".to_string(), config);

        // Open the circuit
        breaker.record_failure().await;
        assert!(breaker.is_open().await);
        assert!(!breaker.call_allowed().await);

        // Wait for recovery timeout
        sleep(Duration::from_millis(60)).await;

        // Circuit should allow calls (half-open)
        assert!(breaker.call_allowed().await);
        // After call_allowed transitions to half-open, the circuit is no longer "open"
        // but it's in the half-open state which allows calls
    }

    #[tokio::test]
    async fn test_success_resets_failure_count() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test".to_string(), config);

        // Record some failures
        breaker.record_failure().await;
        breaker.record_failure().await;

        // Success should reset the counter
        breaker.record_success().await;

        // Should need full threshold again to open
        breaker.record_failure().await;
        breaker.record_failure().await;
        assert!(!breaker.is_open().await);

        breaker.record_failure().await;
        assert!(breaker.is_open().await);
    }
}
