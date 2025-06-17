//! Generic pagination utilities for AiSEG2 collectors.
//!
//! This module provides reusable pagination functionality to reduce duplication
//! across collectors that need to iterate through multiple pages of data.

use crate::error::{AisegError, Result};
use scraper::Html;
use std::future::Future;
use std::pin::Pin;

/// Type alias for the fetch function used in pagination.
pub type FetchFn<'a> = Box<
    dyn Fn(usize) -> Pin<Box<dyn Future<Output = Result<String, AisegError>> + Send + 'a>> + Send + Sync + 'a,
>;

/// Type alias for the parse function used in pagination.
pub type ParseFn<'a, T> = Box<dyn Fn(&Html) -> Result<Vec<T>, AisegError> + Send + Sync + 'a>;

/// Configuration for pagination behavior.
#[derive(Clone)]
pub struct PaginationConfig {
    /// Maximum number of pages to fetch
    pub max_pages: usize,
    /// Starting page number (usually 1)
    pub start_page: usize,
}

impl Default for PaginationConfig {
    fn default() -> Self {
        Self {
            max_pages: 20,
            start_page: 1,
        }
    }
}

/// Trait for types that can be collected across multiple pages.
pub trait PageItem: Clone + PartialEq {
    /// Returns a key that identifies this item for duplicate detection.
    /// Used to detect when pagination has wrapped around.
    fn dedup_key(&self) -> String;
}

/// Generic paginator for collecting items across multiple pages.
pub struct Paginator<'a, T> {
    config: PaginationConfig,
    fetch_fn: FetchFn<'a>,
    parse_fn: ParseFn<'a, T>,
}

impl<'a, T: PageItem> Paginator<'a, T> {
    /// Creates a new paginator with the given configuration and functions.
    #[allow(dead_code)]
    pub fn new<F, P>(config: PaginationConfig, fetch_fn: F, parse_fn: P) -> Self
    where
        F: Fn(usize) -> Pin<Box<dyn Future<Output = Result<String, AisegError>> + Send + 'a>>
            + Send
            + Sync
            + 'a,
        P: Fn(&Html) -> Result<Vec<T>, AisegError> + Send + Sync + 'a,
    {
        Self {
            config,
            fetch_fn: Box::new(fetch_fn),
            parse_fn: Box::new(parse_fn),
        }
    }

    /// Collects all items from all pages.
    pub async fn collect_all(&self) -> Result<Vec<T>, AisegError> {
        let mut all_items = Vec::new();
        let mut last_page_items: Vec<T> = Vec::new();

        for page in self.config.start_page..=self.config.max_pages {
            let response = (self.fetch_fn)(page).await?;
            let document = Html::parse_document(&response);

            let page_items = match (self.parse_fn)(&document) {
                Ok(items) => items,
                Err(_) => break, // Stop on parsing error
            };

            // Check for end of data
            if page_items.is_empty() {
                break;
            }

            // Check if items indicate end of pagination by comparing page signatures
            if !last_page_items.is_empty() && !page_items.is_empty() {
                // Get keys for both pages
                let last_keys: Vec<String> = last_page_items
                    .iter()
                    .map(|item| item.dedup_key())
                    .collect();
                let current_keys: Vec<String> =
                    page_items.iter().map(|item| item.dedup_key()).collect();

                // If the pages have the same items in the same order, we've wrapped around
                if last_keys == current_keys {
                    break;
                }
            }

            last_page_items = page_items.clone();
            all_items.extend(page_items);
        }

        Ok(all_items)
    }
}

/// Helper builder for creating paginators with fluent API.
pub struct PaginatorBuilder<'a, T> {
    config: PaginationConfig,
    fetch_fn: Option<FetchFn<'a>>,
    parse_fn: Option<ParseFn<'a, T>>,
}

impl<'a, T: PageItem> PaginatorBuilder<'a, T> {
    /// Creates a new paginator builder.
    pub fn new() -> Self {
        Self {
            config: PaginationConfig::default(),
            fetch_fn: None,
            parse_fn: None,
        }
    }

    /// Sets the maximum number of pages to fetch.
    pub fn max_pages(mut self, max_pages: usize) -> Self {
        self.config.max_pages = max_pages;
        self
    }

    /// Sets the starting page number.
    #[allow(dead_code)]
    pub fn start_page(mut self, start_page: usize) -> Self {
        self.config.start_page = start_page;
        self
    }

    /// Sets the fetch function for retrieving page content.
    pub fn fetch_with<F>(mut self, fetch_fn: F) -> Self
    where
        F: Fn(usize) -> Pin<Box<dyn Future<Output = Result<String, AisegError>> + Send + 'a>>
            + Send
            + Sync
            + 'a,
    {
        self.fetch_fn = Some(Box::new(fetch_fn));
        self
    }

    /// Sets the parse function for extracting items from HTML.
    pub fn parse_with<P>(mut self, parse_fn: P) -> Self
    where
        P: Fn(&Html) -> Result<Vec<T>, AisegError> + Send + Sync + 'a,
    {
        self.parse_fn = Some(Box::new(parse_fn));
        self
    }

    /// Builds the paginator.
    pub fn build(self) -> Result<Paginator<'a, T>, AisegError> {
        let fetch_fn = self
            .fetch_fn
            .ok_or_else(|| AisegError::Parse(crate::error::ParseError::UnexpectedStructure("Fetch function not set".to_string())))?;
        let parse_fn = self
            .parse_fn
            .ok_or_else(|| AisegError::Parse(crate::error::ParseError::UnexpectedStructure("Parse function not set".to_string())))?;

        Ok(Paginator {
            config: self.config,
            fetch_fn,
            parse_fn,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::Mutex;

    #[derive(Clone, Debug, PartialEq)]
    struct TestItem {
        id: usize,
        name: String,
    }

    impl PageItem for TestItem {
        fn dedup_key(&self) -> String {
            format!("{}-{}", self.id, self.name)
        }
    }

    fn create_test_html(items: &[(usize, &str)]) -> String {
        let mut html = "<html><body>".to_string();
        for (id, name) in items {
            html.push_str(&format!(
                r#"<div class="item" data-id="{}">{}</div>"#,
                id, name
            ));
        }
        html.push_str("</body></html>");
        html
    }

    #[tokio::test]
    async fn test_paginator_collects_all_pages() {
        let pages = vec![
            vec![(1, "Item 1"), (2, "Item 2")],
            vec![(3, "Item 3"), (4, "Item 4")],
            vec![(5, "Item 5")],
        ];

        let pages_arc = Arc::new(pages);
        let fetch_count = Arc::new(Mutex::new(0));
        let fetch_count_clone = fetch_count.clone();

        let paginator = PaginatorBuilder::new()
            .max_pages(3)
            .fetch_with(move |page| {
                let pages = pages_arc.clone();
                let fetch_count = fetch_count_clone.clone();
                Box::pin(async move {
                    *fetch_count.lock().unwrap() += 1;
                    let page_data = &pages[page - 1];
                    Ok(create_test_html(page_data))
                })
            })
            .parse_with(|document| {
                let selector = crate::aiseg::helper::html_selector(".item")?;
                let items: Result<Vec<TestItem>, AisegError> = document
                    .select(&selector)
                    .map(|element| {
                        let id = element
                            .value()
                            .attr("data-id")
                            .and_then(|s| s.parse().ok())
                            .ok_or_else(|| AisegError::Parse(crate::error::ParseError::UnexpectedStructure("Missing data-id attribute".to_string())))?;
                        let name = element.text().collect::<String>();
                        Ok(TestItem { id, name })
                    })
                    .collect();
                items
            })
            .build()
            .unwrap();

        let result = paginator.collect_all().await.unwrap();

        assert_eq!(result.len(), 5);
        assert_eq!(result[0].name, "Item 1");
        assert_eq!(result[4].name, "Item 5");
        assert_eq!(*fetch_count.lock().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_paginator_stops_on_empty_page() {
        let pages = vec![
            vec![(1, "Item 1"), (2, "Item 2")],
            vec![],              // Empty page
            vec![(3, "Item 3")], // This should not be fetched
        ];

        let pages_arc = Arc::new(pages);
        let fetch_count = Arc::new(Mutex::new(0));
        let fetch_count_clone = fetch_count.clone();

        let paginator = PaginatorBuilder::new()
            .max_pages(3)
            .fetch_with(move |page| {
                let pages = pages_arc.clone();
                let fetch_count = fetch_count_clone.clone();
                Box::pin(async move {
                    *fetch_count.lock().unwrap() += 1;
                    let page_data = &pages[page - 1];
                    Ok(create_test_html(page_data))
                })
            })
            .parse_with(|document| {
                let selector = crate::aiseg::helper::html_selector(".item")?;
                let items: Result<Vec<TestItem>, AisegError> = document
                    .select(&selector)
                    .map(|element| {
                        let id = element
                            .value()
                            .attr("data-id")
                            .and_then(|s| s.parse().ok())
                            .ok_or_else(|| AisegError::Parse(crate::error::ParseError::UnexpectedStructure("Missing data-id attribute".to_string())))?;
                        let name = element.text().collect::<String>();
                        Ok(TestItem { id, name })
                    })
                    .collect();
                items
            })
            .build()
            .unwrap();

        let result = paginator.collect_all().await.unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(*fetch_count.lock().unwrap(), 2); // Should only fetch 2 pages
    }

    #[tokio::test]
    async fn test_paginator_stops_on_duplicate_page() {
        let pages = vec![
            vec![(1, "Item 1"), (2, "Item 2")],
            vec![(3, "Item 3"), (4, "Item 4")],
            vec![(3, "Item 3"), (4, "Item 4")], // Same as page 2
        ];

        let pages_arc = Arc::new(pages);

        let paginator = PaginatorBuilder::new()
            .max_pages(3)
            .fetch_with(move |page| {
                let pages = pages_arc.clone();
                Box::pin(async move {
                    let page_data = &pages[page - 1];
                    Ok(create_test_html(page_data))
                })
            })
            .parse_with(|document| {
                let selector = crate::aiseg::helper::html_selector(".item")?;
                let items: Result<Vec<TestItem>, AisegError> = document
                    .select(&selector)
                    .map(|element| {
                        let id = element
                            .value()
                            .attr("data-id")
                            .and_then(|s| s.parse().ok())
                            .ok_or_else(|| AisegError::Parse(crate::error::ParseError::UnexpectedStructure("Missing data-id attribute".to_string())))?;
                        let name = element.text().collect::<String>();
                        Ok(TestItem { id, name })
                    })
                    .collect();
                items
            })
            .build()
            .unwrap();

        let result = paginator.collect_all().await.unwrap();

        // The paginator will collect items from pages 1 and 2 (4 items total),
        // then stop when it sees that page 3's first item (id=1) already exists
        assert_eq!(result.len(), 4); // Should stop before adding duplicates
    }
}
