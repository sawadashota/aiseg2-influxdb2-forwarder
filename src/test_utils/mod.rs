//! Consolidated test utilities and helpers for the AiSEG2 to InfluxDB2 forwarder.
//!
//! This module provides a centralized location for all test utilities, mock implementations,
//! and test data builders used throughout the codebase.

#![cfg(test)]

pub mod builders;
pub mod config;
pub mod fixtures;
pub mod html;
pub mod mocks;

// Re-export commonly used items for convenience
// Note: We're using qualified imports in test code to be explicit about what we're using
