use crate::aiseg::client::Client;
use crate::aiseg::helper::{
    f64_kw_to_i64_watt, f64_to_i64, parse_f64_from_html, parse_text_from_html,
};
use crate::model::{
    merge_same_name_power_status_breakdown_metrics, DataPointBuilder, Measurement, MetricCollector,
    PowerStatusBreakdownMetric, PowerStatusBreakdownMetricCategory, PowerStatusMetric, Unit,
};
use anyhow::Result;
use chrono::{DateTime, Local};
use scraper::Html;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct PowerMetricCollector {
    client: Arc<Client>,
}

impl PowerMetricCollector {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    async fn collect_from_main_page(&self) -> Result<Vec<Box<dyn DataPointBuilder>>> {
        let response = self.client.get("/page/electricflow/111").await?;
        let document = Html::parse_document(&response);
        Ok(vec![
            self.collect_total_metrics(&document)?,
            self.collect_generation_detail_metrics(&document)?,
        ]
        .into_iter()
        .flatten()
        .collect())
    }

    fn collect_total_metrics(&self, document: &Html) -> Result<Vec<Box<dyn DataPointBuilder>>> {
        let generation = f64_kw_to_i64_watt(parse_f64_from_html(&document, "#g_capacity")?);
        let consumption = f64_kw_to_i64_watt(parse_f64_from_html(&document, "#u_capacity")?);

        Ok(vec![
            Box::new(PowerStatusMetric {
                measurement: Measurement::Power,
                name: format!("総発電電力({})", Unit::Watt),
                value: generation,
            }),
            Box::new(PowerStatusMetric {
                measurement: Measurement::Power,
                name: format!("総消費電力({})", Unit::Watt),
                value: consumption,
            }),
            Box::new(PowerStatusMetric {
                measurement: Measurement::Power,
                name: format!("売買電力({})", Unit::Watt),
                value: generation - consumption,
            }),
        ])
    }

    fn collect_generation_detail_metrics(
        &self,
        document: &Html,
    ) -> Result<Vec<Box<dyn DataPointBuilder>>> {
        let mut res: Vec<Box<dyn DataPointBuilder>> = vec![];
        for i in 1..=4 {
            let name = match parse_text_from_html(&document, &format!("#g_d_{}_title", i)) {
                Ok(name) => name,
                Err(_) => break,
            };
            let value = f64_to_i64(parse_f64_from_html(
                &document,
                &format!("#g_d_{}_capacity", i),
            )?);
            res.push(Box::new(PowerStatusBreakdownMetric {
                measurement: Measurement::Power,
                category: PowerStatusBreakdownMetricCategory::Generation,
                name: format!("{}({})", name, Unit::Watt),
                value,
            }));
        }
        Ok(res)
    }

    async fn collect_from_consumption_detail_pages(
        &self,
    ) -> Result<Vec<Box<dyn DataPointBuilder>>> {
        self.collect_consumption_detail_metrics().await
    }

    async fn collect_consumption_detail_metrics(&self) -> Result<Vec<Box<dyn DataPointBuilder>>> {
        let mut last_page_names = "".to_string();
        let mut list: Vec<PowerStatusBreakdownMetric> = vec![];

        for page in 1..=20 {
            let response = self
                .client
                .get(&format!("/page/electricflow/1113?id={}", page))
                .await?;
            let document = Html::parse_document(&response);

            let mut items: Vec<PowerStatusBreakdownMetric> = vec![];
            for i in 1..=10 {
                let name = match parse_text_from_html(
                    &document,
                    &format!("#stage_{} > div.c_device", i),
                ) {
                    Ok(name) => name,
                    Err(_) => break,
                };
                let watt =
                    match parse_f64_from_html(&document, &format!("#stage_{} > div.c_value", i)) {
                        Ok(kw) => f64_to_i64(kw),
                        Err(_) => 0,
                    };
                items.push(PowerStatusBreakdownMetric {
                    measurement: Measurement::Power,
                    category: PowerStatusBreakdownMetricCategory::Consumption,
                    name: format!("{}({})", name, Unit::Watt),
                    value: watt,
                });
            }

            let names = items
                .iter()
                .map(|item| item.name.clone())
                .collect::<Vec<String>>()
                .join(", ");
            if last_page_names == names {
                break;
            }
            last_page_names = names;
            list.extend(items);
        }

        let merged = merge_same_name_power_status_breakdown_metrics(list);
        Ok(merged
            .into_iter()
            .map(|item| Box::new(item) as Box<dyn DataPointBuilder>)
            .collect())
    }
}

impl MetricCollector for PowerMetricCollector {
    fn collect<'a>(
        &'a self,
        _: DateTime<Local>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn DataPointBuilder>>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(vec![
                self.collect_from_main_page().await?,
                self.collect_from_consumption_detail_pages().await?,
            ]
            .into_iter()
            .flatten()
            .collect())
        })
    }
}
