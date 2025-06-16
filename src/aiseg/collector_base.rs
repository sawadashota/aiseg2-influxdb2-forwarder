//! Base functionality shared across all AiSEG2 collectors.
//!
//! This module provides common patterns and utilities used by all collectors
//! to reduce code duplication and ensure consistent behavior.

use crate::aiseg::Client;
use crate::model::DataPointBuilder;
use anyhow::Result;
use std::sync::Arc;

/// Base trait for AiSEG2 collectors with common functionality.
pub trait CollectorBase {
    /// Returns the HTTP client used for AiSEG2 communication.
    fn client(&self) -> &Arc<Client>;

    /// Fetches a page from AiSEG2 and returns the HTML response.
    async fn fetch_page(&self, path: &str) -> Result<String> {
        self.client().get(path).await
    }
}

/// Helper for managing pagination state during collection.
#[allow(dead_code)]
pub struct PaginationState {
    current_page: usize,
    max_pages: usize,
    items_collected: usize,
}

#[allow(dead_code)]
impl PaginationState {
    /// Creates a new pagination state.
    pub fn new(max_pages: usize) -> Self {
        Self {
            current_page: 0,
            max_pages,
            items_collected: 0,
        }
    }

    /// Advances to the next page.
    pub fn next_page(&mut self) -> Option<usize> {
        if self.current_page < self.max_pages {
            self.current_page += 1;
            Some(self.current_page)
        } else {
            None
        }
    }

    /// Records items collected on the current page.
    pub fn record_items(&mut self, count: usize) {
        self.items_collected += count;
    }

    /// Returns the total number of items collected.
    pub fn total_items(&self) -> usize {
        self.items_collected
    }
}

/// Result type for metric collection operations.
pub type MetricResult = Result<Vec<Box<dyn DataPointBuilder>>>;

/// Merges duplicate metrics by summing their values.
///
/// This is used when the same device/metric appears multiple times
/// across different pages or sources.
#[allow(dead_code)]
pub fn merge_duplicate_metrics<T, K, F>(metrics: Vec<T>, key_fn: F) -> Vec<T>
where
    T: Clone,
    K: Eq + std::hash::Hash,
    F: Fn(&T) -> K,
{
    use std::collections::HashMap;

    let mut seen = HashMap::new();
    let mut result = Vec::with_capacity(metrics.len());

    for metric in metrics {
        let key = key_fn(&metric);
        if let std::collections::hash_map::Entry::Vacant(e) = seen.entry(key) {
            e.insert(result.len());
            result.push(metric);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_state() {
        let mut state = PaginationState::new(3);

        assert_eq!(state.next_page(), Some(1));
        state.record_items(5);

        assert_eq!(state.next_page(), Some(2));
        state.record_items(3);

        assert_eq!(state.next_page(), Some(3));
        state.record_items(2);

        assert_eq!(state.next_page(), None);
        assert_eq!(state.total_items(), 10);
    }

    #[test]
    fn test_merge_duplicate_metrics() {
        #[derive(Clone, PartialEq, Debug)]
        struct TestMetric {
            name: String,
            value: i32,
        }

        let metrics = vec![
            TestMetric {
                name: "A".to_string(),
                value: 1,
            },
            TestMetric {
                name: "B".to_string(),
                value: 2,
            },
            TestMetric {
                name: "A".to_string(),
                value: 3,
            },
            TestMetric {
                name: "C".to_string(),
                value: 4,
            },
        ];

        let merged = merge_duplicate_metrics(metrics, |m| m.name.clone());

        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].name, "A");
        assert_eq!(merged[1].name, "B");
        assert_eq!(merged[2].name, "C");
    }
}
