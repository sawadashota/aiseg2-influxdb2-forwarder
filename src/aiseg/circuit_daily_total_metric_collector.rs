use crate::aiseg::client::Client;
use crate::aiseg::helper::{day_of_beginning, parse_f64_from_html};
use crate::model::{DataPointBuilder, Measurement, MetricCollector, PowerTotalMetric, Unit};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Datelike, Local};
use scraper::Html;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct CircuitDailyTotalMetricCollector {
    client: Arc<Client>,
}

impl CircuitDailyTotalMetricCollector {
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    async fn collect_by_circuit_id(
        &self,
        date: DateTime<Local>,
        name: &str,
        circuit_id: &str,
        unit: Unit,
    ) -> Result<PowerTotalMetric> {
        let the_day = day_of_beginning(&date);
        let response = self
            .client
            .get(&format!(
                "/page/graph/584?data={}",
                make_query(circuit_id, the_day)
            ))
            .await?;
        let document = Html::parse_document(&response);
        let value = parse_f64_from_html(&document, "#val_kwh")?;
        Ok(PowerTotalMetric {
            measurement: Measurement::CircuitDailyTotal,
            name: format!("{}({})", name, unit),
            value,
            date: the_day,
        })
    }
}

impl MetricCollector for CircuitDailyTotalMetricCollector {
    fn collect<'a>(
        &'a self,
        timestamp: DateTime<Local>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Box<dyn DataPointBuilder>>>> + 'a + Send>> {
        Box::pin(async move {
            Ok(vec![
                self.collect_by_circuit_id(timestamp, "EV", "30", Unit::Kwh)
                    .await?,
                self.collect_by_circuit_id(timestamp, "リビングエアコン", "27", Unit::Kwh)
                    .await?,
                self.collect_by_circuit_id(timestamp, "主寝室エアコン", "26", Unit::Kwh)
                    .await?,
                self.collect_by_circuit_id(timestamp, "洋室２エアコン", "25", Unit::Kwh)
                    .await?,
            ]
            .into_iter()
            .map(|x| Box::new(x) as Box<dyn DataPointBuilder>)
            .collect())
        })
    }
}

// makeDataQuery is base64 encoded JSON string
// ex: {"day":[2024,6,8],"term":"2024/06/08","termStr":"day","id":"1","circuitid":"30"}
fn make_query(circuit_id: &str, date: DateTime<Local>) -> String {
    let query = format!(
        r#"{{"day":[{}, {}, {}],"term":"{}","termStr":"day","id":"1","circuitid":"{}"}}"#,
        date.year(),
        date.month(),
        date.day(),
        date.format("%Y/%m/%d"),
        circuit_id,
    );
    STANDARD.encode(query)
}
