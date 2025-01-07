use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local, NaiveTime};
use scraper::{Html, Selector};

pub fn parse_text_from_html(document: &Html, selector: &str) -> Result<String> {
    let selector = html_selector(selector)?;
    let element = document
        .select(&selector)
        .next()
        .context("Failed to find value")?;
    if !element.has_children() {
        return Err(anyhow!("Element has no children"));
    }
    Ok(element.text().collect::<String>())
}

pub fn html_selector(selector: &str) -> Result<Selector> {
    match Selector::parse(selector) {
        Ok(s) => Ok(s),
        Err(e) => Err(anyhow!("Failed to parse selector: {}", e)),
    }
}

pub fn parse_f64_from_html(document: &Html, selector: &str) -> Result<f64> {
    let selector = html_selector(selector)?;
    let element = document
        .select(&selector)
        .next()
        .context("Failed to find value")?;
    let inner_text = element.text().next().context("Failed to get text")?;
    Ok(inner_text
        .chars()
        .filter(|c| c.is_numeric() || c == &'.')
        .collect::<String>()
        .parse::<f64>()
        .context("Failed to parse value")?)
}

pub fn day_of_beginning(date: &DateTime<Local>) -> DateTime<Local> {
    date.with_time(NaiveTime::default()).unwrap()
}

pub fn f64_to_i64(kw: f64) -> i64 {
    kw.trunc() as i64
}

pub fn f64_kw_to_i64_watt(kw: f64) -> i64 {
    f64_to_i64(kw * 1000.0)
}
