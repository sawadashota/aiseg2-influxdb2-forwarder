//! Base functionality shared across all AiSEG2 collectors.
//!
//! This module provides common patterns and utilities used by all collectors
//! to reduce code duplication and ensure consistent behavior.

use crate::aiseg::Client;
use crate::error::{AisegError, Result};
use crate::model::DataPointBuilder;
use std::sync::Arc;

/// Base trait for AiSEG2 collectors with common functionality.
pub trait CollectorBase {
    /// Returns the HTTP client used for AiSEG2 communication.
    fn client(&self) -> &Arc<Client>;

    /// Fetches a page from AiSEG2 and returns the HTML response.
    async fn fetch_page(&self, path: &str) -> Result<String, AisegError> {
        self.client().get(path).await
    }
}

/// Result type for metric collection operations.
pub type MetricResult =
    std::result::Result<Vec<Box<dyn DataPointBuilder>>, crate::error::CollectorError>;
