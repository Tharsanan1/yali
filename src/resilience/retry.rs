//! Retry Policy Implementation
//!
//! Provides automatic retry with exponential backoff for failed requests.

use std::time::Duration;

/// Result of a retry decision.
#[derive(Debug, Clone, PartialEq)]
pub enum RetryResult {
    /// Retry the request after the specified delay.
    Retry(Duration),
    /// Do not retry, return the error.
    DoNotRetry,
}

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_attempts: u32,
    /// Initial backoff delay.
    pub initial_backoff: Duration,
    /// Maximum backoff delay.
    pub max_backoff: Duration,
    /// Backoff multiplier.
    pub multiplier: f64,
    /// Conditions that trigger a retry.
    pub retry_conditions: Vec<RetryCondition>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            multiplier: 2.0,
            retry_conditions: vec![
                RetryCondition::StatusCode5xx,
                RetryCondition::ConnectionFailure,
                RetryCondition::Reset,
            ],
        }
    }
}

/// Conditions that can trigger a retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryCondition {
    /// Retry on 5xx status codes.
    StatusCode5xx,
    /// Retry on connection failures.
    ConnectionFailure,
    /// Retry on connection reset.
    Reset,
    /// Retry on timeout.
    Timeout,
    /// Retry on specific status code.
    StatusCode(u16),
}

impl RetryCondition {
    /// Parse a retry condition from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "5xx" => Some(RetryCondition::StatusCode5xx),
            "connect-failure" => Some(RetryCondition::ConnectionFailure),
            "reset" => Some(RetryCondition::Reset),
            "timeout" => Some(RetryCondition::Timeout),
            _ if s.parse::<u16>().is_ok() => s.parse::<u16>().ok().map(RetryCondition::StatusCode),
            _ => None,
        }
    }
}

/// Information about an error that occurred.
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    /// HTTP status code if available.
    pub status_code: Option<u16>,
    /// Whether this was a connection failure.
    pub is_connection_failure: bool,
    /// Whether this was a connection reset.
    pub is_reset: bool,
    /// Whether this was a timeout.
    pub is_timeout: bool,
}

impl ErrorInfo {
    /// Create error info for a connection failure.
    pub fn connection_failure() -> Self {
        Self {
            status_code: None,
            is_connection_failure: true,
            is_reset: false,
            is_timeout: false,
        }
    }

    /// Create error info for a status code.
    pub fn from_status(code: u16) -> Self {
        Self {
            status_code: Some(code),
            is_connection_failure: false,
            is_reset: false,
            is_timeout: false,
        }
    }

    /// Create error info for a timeout.
    pub fn timeout() -> Self {
        Self {
            status_code: None,
            is_connection_failure: false,
            is_reset: false,
            is_timeout: true,
        }
    }

    /// Create error info for a connection reset.
    pub fn reset() -> Self {
        Self {
            status_code: None,
            is_connection_failure: false,
            is_reset: true,
            is_timeout: false,
        }
    }
}

/// Retry policy for handling failed requests.
pub struct RetryPolicy {
    /// Configuration.
    config: RetryConfig,
}

impl RetryPolicy {
    /// Create a new retry policy with the given configuration.
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Create a retry policy with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(RetryConfig::default())
    }

    /// Create a retry policy from config strings (as used in config file).
    pub fn from_config_values(
        attempts: u32,
        initial_ms: u64,
        max_ms: u64,
        multiplier: f64,
        conditions: &[String],
    ) -> Self {
        let retry_conditions = conditions
            .iter()
            .filter_map(|s| RetryCondition::from_str(s))
            .collect();

        Self::new(RetryConfig {
            max_attempts: attempts,
            initial_backoff: Duration::from_millis(initial_ms),
            max_backoff: Duration::from_millis(max_ms),
            multiplier,
            retry_conditions,
        })
    }

    /// Determine if a request should be retried.
    ///
    /// # Arguments
    /// * `attempt` - The current attempt number (1-indexed)
    /// * `error` - Information about the error
    ///
    /// # Returns
    /// Whether to retry and how long to wait.
    pub fn should_retry(&self, attempt: u32, error: &ErrorInfo) -> RetryResult {
        // Check if we've exceeded max attempts
        if attempt >= self.config.max_attempts {
            return RetryResult::DoNotRetry;
        }

        // Check if the error matches any retry condition
        let should_retry = self
            .config
            .retry_conditions
            .iter()
            .any(|condition| match condition {
                RetryCondition::StatusCode5xx => error
                    .status_code
                    .map(|c| c >= 500 && c < 600)
                    .unwrap_or(false),
                RetryCondition::ConnectionFailure => error.is_connection_failure,
                RetryCondition::Reset => error.is_reset,
                RetryCondition::Timeout => error.is_timeout,
                RetryCondition::StatusCode(code) => {
                    error.status_code.map(|c| c == *code).unwrap_or(false)
                }
            });

        if !should_retry {
            return RetryResult::DoNotRetry;
        }

        // Calculate backoff delay with exponential growth
        let delay = self.calculate_backoff(attempt);
        RetryResult::Retry(delay)
    }

    /// Calculate the backoff delay for a given attempt.
    fn calculate_backoff(&self, attempt: u32) -> Duration {
        // attempt is 1-indexed, so first retry uses multiplier^0 = 1
        let exponent = attempt.saturating_sub(1);
        let multiplier = self.config.multiplier.powi(exponent as i32);
        let delay_ms = self.config.initial_backoff.as_millis() as f64 * multiplier;
        let delay = Duration::from_millis(delay_ms as u64);

        // Cap at max backoff
        std::cmp::min(delay, self.config.max_backoff)
    }

    /// Get the maximum number of attempts.
    pub fn max_attempts(&self) -> u32 {
        self.config.max_attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_on_5xx() {
        let policy = RetryPolicy::with_defaults();

        let error = ErrorInfo::from_status(500);
        let result = policy.should_retry(1, &error);

        assert!(matches!(result, RetryResult::Retry(_)));
    }

    #[test]
    fn test_no_retry_on_4xx() {
        let policy = RetryPolicy::with_defaults();

        let error = ErrorInfo::from_status(400);
        let result = policy.should_retry(1, &error);

        assert_eq!(result, RetryResult::DoNotRetry);
    }

    #[test]
    fn test_retry_on_connection_failure() {
        let policy = RetryPolicy::with_defaults();

        let error = ErrorInfo::connection_failure();
        let result = policy.should_retry(1, &error);

        assert!(matches!(result, RetryResult::Retry(_)));
    }

    #[test]
    fn test_no_retry_after_max_attempts() {
        let policy = RetryPolicy::with_defaults(); // max_attempts = 3

        let error = ErrorInfo::from_status(500);
        let result = policy.should_retry(3, &error);

        assert_eq!(result, RetryResult::DoNotRetry);
    }

    #[test]
    fn test_exponential_backoff() {
        let config = RetryConfig {
            max_attempts: 5,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            multiplier: 2.0,
            retry_conditions: vec![RetryCondition::StatusCode5xx],
        };
        let policy = RetryPolicy::new(config);

        let error = ErrorInfo::from_status(500);

        // First attempt: 100ms
        if let RetryResult::Retry(delay) = policy.should_retry(1, &error) {
            assert_eq!(delay, Duration::from_millis(100));
        } else {
            panic!("Expected retry");
        }

        // Second attempt: 200ms
        if let RetryResult::Retry(delay) = policy.should_retry(2, &error) {
            assert_eq!(delay, Duration::from_millis(200));
        } else {
            panic!("Expected retry");
        }

        // Third attempt: 400ms
        if let RetryResult::Retry(delay) = policy.should_retry(3, &error) {
            assert_eq!(delay, Duration::from_millis(400));
        } else {
            panic!("Expected retry");
        }
    }

    #[test]
    fn test_backoff_capped_at_max() {
        let config = RetryConfig {
            max_attempts: 10,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(5),
            multiplier: 2.0,
            retry_conditions: vec![RetryCondition::StatusCode5xx],
        };
        let policy = RetryPolicy::new(config);

        let error = ErrorInfo::from_status(500);

        // Fifth attempt would be 1 * 2^4 = 16s, but capped at 5s
        if let RetryResult::Retry(delay) = policy.should_retry(5, &error) {
            assert_eq!(delay, Duration::from_secs(5));
        } else {
            panic!("Expected retry");
        }
    }

    #[test]
    fn test_retry_condition_parsing() {
        assert_eq!(
            RetryCondition::from_str("5xx"),
            Some(RetryCondition::StatusCode5xx)
        );
        assert_eq!(
            RetryCondition::from_str("connect-failure"),
            Some(RetryCondition::ConnectionFailure)
        );
        assert_eq!(
            RetryCondition::from_str("reset"),
            Some(RetryCondition::Reset)
        );
        assert_eq!(
            RetryCondition::from_str("timeout"),
            Some(RetryCondition::Timeout)
        );
        assert_eq!(
            RetryCondition::from_str("429"),
            Some(RetryCondition::StatusCode(429))
        );
        assert_eq!(RetryCondition::from_str("invalid"), None);
    }

    #[test]
    fn test_from_config_values() {
        let policy = RetryPolicy::from_config_values(
            5,
            200,
            5000,
            1.5,
            &["5xx".to_string(), "connect-failure".to_string()],
        );

        assert_eq!(policy.max_attempts(), 5);

        let error = ErrorInfo::from_status(500);
        if let RetryResult::Retry(delay) = policy.should_retry(1, &error) {
            assert_eq!(delay, Duration::from_millis(200));
        } else {
            panic!("Expected retry");
        }
    }
}
