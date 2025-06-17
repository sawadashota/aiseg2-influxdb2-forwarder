//! Unified trait system for HTML parsing operations.
//!
//! This module provides a trait-based abstraction for parsing HTML documents
//! from AiSEG2 web interface, reducing code duplication across collectors.

use anyhow::Result;
use scraper::Html;

/// Core trait for parsing HTML content into domain-specific types.
///
/// # Example
/// ```no_run
/// use scraper::Html;
/// use anyhow::Result;
///
/// struct PowerParser;
///
/// impl HtmlParser for PowerParser {
///     type Output = Vec<(String, f64)>;
///     
///     fn parse(&self, document: &Html) -> Result<Self::Output> {
///         // Parse power values from HTML
///         Ok(vec![("Solar".to_string(), 1500.0)])
///     }
/// }
/// ```
pub trait HtmlParser {
    /// The type of data this parser produces
    type Output;

    /// Parse HTML document into the output type
    fn parse(&self, document: &Html) -> Result<Self::Output>;
}

/// Trait for parsers that need additional context for parsing.
///
/// Use this when parsing requires external information like timestamps,
/// circuit names, or other contextual data.
pub trait ContextualHtmlParser {
    /// The type of data this parser produces
    type Output;
    /// The type of context needed for parsing
    type Context;

    /// Parse HTML document with context into the output type
    fn parse_with_context(&self, document: &Html, context: Self::Context) -> Result<Self::Output>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestParser;

    impl HtmlParser for TestParser {
        type Output = String;

        fn parse(&self, _document: &Html) -> Result<Self::Output> {
            Ok("test".to_string())
        }
    }

    #[test]
    fn test_parser_trait() {
        let parser = TestParser;
        let html = Html::parse_document("<html></html>");
        let result = parser.parse(&html).unwrap();
        assert_eq!(result, "test");
    }

    struct TestContextualParser;

    impl ContextualHtmlParser for TestContextualParser {
        type Output = String;
        type Context = i32;

        fn parse_with_context(
            &self,
            _document: &Html,
            context: Self::Context,
        ) -> Result<Self::Output> {
            Ok(format!("test with context: {}", context))
        }
    }

    #[test]
    fn test_contextual_parser_trait() {
        let parser = TestContextualParser;
        let html = Html::parse_document("<html></html>");
        let result = parser.parse_with_context(&html, 42).unwrap();
        assert_eq!(result, "test with context: 42");
    }
}
