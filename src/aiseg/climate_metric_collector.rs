use crate::aiseg::helper::html_selector;
use crate::aiseg::Client;
use crate::model::{
    ClimateStatusMetric, ClimateStatusMetricCategory, DataPointBuilder, Measurement,
    MetricCollector,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use scraper::element_ref::Select;
use scraper::Html;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct ClimateMetricCollector {
    client: Arc<Client>,
}

impl ClimateMetricCollector {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }
}

impl MetricCollector for ClimateMetricCollector {
    fn collect<'a>(
        &'a self,
        timestamp: DateTime<Local>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn DataPointBuilder>>>> + Send + 'a>> {
        Box::pin(async move {
            let mut list: Vec<ClimateStatusMetric> = vec![];

            'root: for page in 1..=20 {
                let response = self
                    .client
                    .get(&format!("/page/airenvironment/41?page={}", page))
                    .await?;
                let document = Html::parse_document(&response);

                for i in 1..=3 {
                    let base_id = format!("#base{}_1", i);
                    let metrics = match parse(&document, &base_id, timestamp.clone()) {
                        Ok(metrics) => metrics,
                        Err(_) => break 'root,
                    };
                    list.extend(metrics);
                }
            }

            Ok(list
                .into_iter()
                .map(|item| Box::new(item) as Box<dyn DataPointBuilder>)
                .collect())
        })
    }
}

fn parse(
    document: &Html,
    base_id: &str,
    timestamp: DateTime<Local>,
) -> Result<[ClimateStatusMetric; 2]> {
    let base_selector = html_selector(base_id)?;
    let base_element = document
        .select(&base_selector)
        .next()
        .context("Failed to find value")?;

    // extract place name from `.txt_name`
    let name_selector = html_selector(".txt_name")?;
    let name = base_element
        .select(&name_selector)
        .next()
        .context("Failed to find name")?
        .text()
        .next()
        .context("Failed to get text")?;

    let num_wrapper_selector = html_selector(".num_wrapper")?;
    let num_wrapper_element = base_element
        .select(&num_wrapper_selector)
        .next()
        .context("Failed to find num_wrapper")?;

    // extract temperature from `#num_ond_\d`
    let temperature_selector = html_selector(r#"[id^="num_ond_"]"#)?;
    let temperature =
        extract_num_from_html_class(num_wrapper_element.select(&temperature_selector))?;

    // extract humidity from `#num_shitudo_\d`
    let humidity_selector = html_selector(r#"[id^="num_shitudo_"]"#)?;
    let humidity = extract_num_from_html_class(num_wrapper_element.select(&humidity_selector))?;

    Ok([
        ClimateStatusMetric {
            measurement: Measurement::Climate,
            category: ClimateStatusMetricCategory::Temperature,
            name: name.to_string(),
            value: temperature,
            timestamp: timestamp.clone(),
        },
        ClimateStatusMetric {
            measurement: Measurement::Climate,
            category: ClimateStatusMetricCategory::Humidity,
            name: name.to_string(),
            value: humidity,
            timestamp,
        },
    ])
}

fn extract_num_from_html_class(elements: Select) -> Result<f64> {
    let mut chars: [char; 4] = ['0', '0', '.', '0'];
    let mut i = 0;
    for element in elements {
        if i == 2 {
            i += 1; // skip dot
        }
        let class_value = element.attr("class").context("Failed to get class")?;
        chars[i] = class_value
            .chars()
            .filter(|c| c.is_numeric())
            .collect::<String>()
            .parse::<char>()
            .context("Failed to parse value")?;
        i += 1;
    }
    Ok(chars.iter().collect::<String>().parse::<f64>()?)
}
