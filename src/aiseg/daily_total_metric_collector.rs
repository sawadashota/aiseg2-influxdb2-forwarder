use crate::aiseg::client::Client;
use crate::aiseg::helper::{day_of_beginning, parse_f64_from_html, parse_text_from_html};
use crate::model::{DataPointBuilder, Measurement, MetricCollector, PowerTotalMetric, Unit};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Datelike, Local};
use scraper::Html;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct DailyTotalMetricCollector {
    client: Arc<Client>,
}

impl DailyTotalMetricCollector {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    async fn collect_by_graph_id(
        &self,
        date: DateTime<Local>,
        graph_id: &str,
        unit: Unit,
    ) -> Result<PowerTotalMetric> {
        let the_day = day_of_beginning(&date);
        let response = self
            .client
            .get(&format!(
                "/page/graph/{}?data={}",
                graph_id,
                make_query(the_day)
            ))
            .await?;
        let document = Html::parse_document(&response);
        let name = parse_text_from_html(&document, "#h_title")?;
        let value = parse_f64_from_html(&document, "#val_kwh")?;

        Ok(PowerTotalMetric {
            measurement: Measurement::DailyTotal,
            name: format!("{}({})", name, unit),
            value,
            date: the_day,
        })
    }
}

impl MetricCollector for DailyTotalMetricCollector {
    fn collect<'a>(
        &'a self,
        timestamp: DateTime<Local>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn DataPointBuilder>>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(vec![
                // DailyTotalPowerGeneration
                self.collect_by_graph_id(timestamp, "51111", Unit::Kwh)
                    .await?,
                // DailyTotalPowerConsumption
                self.collect_by_graph_id(timestamp, "52111", Unit::Kwh)
                    .await?,
                // DailyTotalPowerBuying
                self.collect_by_graph_id(timestamp, "53111", Unit::Kwh)
                    .await?,
                // DailyTotalPowerSelling
                self.collect_by_graph_id(timestamp, "54111", Unit::Kwh)
                    .await?,
                // DailyTotalHotWaterConsumption
                self.collect_by_graph_id(timestamp, "55111", Unit::Liter)
                    .await?,
                // DailyTotalGasConsumption
                self.collect_by_graph_id(timestamp, "57111", Unit::CubicMeter)
                    .await?,
            ]
            .into_iter()
            .map(|x| Box::new(x) as Box<dyn DataPointBuilder>)
            .collect())
        })
    }
}

// makeDataQuery is base64 encoded JSON string
// ex: {"day":[2024,6,6],"month_compare":"mon","day_compare":"day"}
fn make_query(date: DateTime<Local>) -> String {
    let query = format!(
        r#"{{"day":[{}, {}, {}],"month_compare":"mon","day_compare":"day"}}"#,
        date.year(),
        date.month(),
        date.day(),
    );
    STANDARD.encode(query)
}
