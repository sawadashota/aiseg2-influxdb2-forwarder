//! Unified trait system for HTML parsing operations.
//!
//! This module provides a trait-based abstraction for parsing HTML documents
//! from AiSEG2 web interface, reducing code duplication across collectors.

use anyhow::Result;
use scraper::{ElementRef, Html};

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

/// Trait for element-level parsing operations.
///
/// Useful for parsing specific HTML elements within a larger document.
pub trait ElementParser {
    /// The type of value this parser extracts
    type Output;

    /// Parse a single element
    fn parse_element(&self, element: ElementRef) -> Result<Self::Output>;
}

/// Trait for parsing operations that may fail gracefully.
///
/// Use this for optional parsing where missing data should not cause errors.
pub trait OptionalParser {
    /// The type of data this parser produces
    type Output;

    /// Parse HTML document, returning None if data is not found
    fn parse_optional(&self, document: &Html) -> Option<Self::Output>;
}

/// Builder for creating parsers with common configuration.
pub struct ParserBuilder<P> {
    parser: P,
}

impl<P> ParserBuilder<P> {
    /// Create a new parser builder
    pub fn new(parser: P) -> Self {
        Self { parser }
    }

    /// Build the configured parser
    pub fn build(self) -> P {
        self.parser
    }
}

/// Provides error context for parsing operations.
pub struct ParseContext<'a> {
    /// Name of the parser or operation
    operation: &'a str,
    /// Additional context information
    context: Option<&'a str>,
}

impl<'a> ParseContext<'a> {
    /// Create a new parse context
    pub fn new(operation: &'a str) -> Self {
        Self {
            operation,
            context: None,
        }
    }

    /// Add additional context information
    pub fn with_context(mut self, context: &'a str) -> Self {
        self.context = Some(context);
        self
    }

    /// Create an error message with context
    pub fn error_message(&self, error: &str) -> String {
        match self.context {
            Some(ctx) => format!("{}: {} ({})", self.operation, error, ctx),
            None => format!("{}: {}", self.operation, error),
        }
    }
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

    #[test]
    fn test_parse_context() {
        let context = ParseContext::new("TestOperation").with_context("additional info");

        let message = context.error_message("something went wrong");
        assert_eq!(
            message,
            "TestOperation: something went wrong (additional info)"
        );
    }
}
