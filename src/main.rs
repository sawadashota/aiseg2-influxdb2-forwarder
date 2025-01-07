mod aiseg;
mod config;
mod influxdb;
mod model;

use crate::model::{batch_collect_metrics, MetricCollector};
use chrono::{Local, NaiveTime};
use std::ops::Sub;
use std::sync::Arc;
use tokio::signal::ctrl_c;
use tokio::signal::unix::{signal, SignalKind};
use tokio::task::JoinError;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let app_config = config::load_app_config().expect("Failed to load AppConfig");
    tracing_subscriber::fmt()
        .with_max_level(app_config.log_level())
        .init();

    let collector_config = config::load_collector_config().expect("Failed to load CollectorConfig");
    let influx_config = config::load_influx_config().expect("Failed to load InfluxConfig");
    let influx_client = Arc::new(influxdb::Client::new(influx_config));

    let aiseg_config = config::load_aiseg_config().expect("Failed to load AisegConfig");
    let aiseg_client = Arc::new(aiseg::Client::new(aiseg_config));

    let total_collectors: Arc<Vec<Box<dyn MetricCollector>>> = Arc::new(vec![
        Box::new(aiseg::DailyTotalMetricCollector::new(Arc::clone(
            &aiseg_client,
        ))),
        Box::new(aiseg::CircuitDailyTotalMetricCollector::new(Arc::clone(
            &aiseg_client,
        ))),
    ]);
    let status_collectors: Arc<Vec<Box<dyn MetricCollector>>> = Arc::new(vec![
        Box::new(aiseg::PowerMetricCollector::new(Arc::clone(&aiseg_client))),
        Box::new(aiseg::ClimateMetricCollector::new(Arc::clone(
            &aiseg_client,
        ))),
    ]);

    tokio::spawn(collect_past_total(
        Arc::clone(&total_collectors),
        Arc::clone(&influx_client),
        collector_config.total_initial_days,
    ));

    let create_collect_status_task = || -> tokio::task::JoinHandle<()> {
        tokio::spawn(create_collect_task(
            Arc::clone(&influx_client),
            Arc::clone(&status_collectors),
            Duration::from_secs(collector_config.status_interval_sec),
            "status_collectors",
        ))
    };
    let create_collect_total_task = || -> tokio::task::JoinHandle<()> {
        tokio::spawn(create_collect_task(
            Arc::clone(&influx_client),
            Arc::clone(&total_collectors),
            Duration::from_secs(collector_config.total_interval_sec),
            "total_collectors",
        ))
    };
    let mut collect_status_task = create_collect_status_task();
    let mut collect_total_task = create_collect_total_task();

    let mut sig_term = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    tracing::info!("Running... Press Ctrl-C or send SIGTERM to terminate.");
    loop {
        tokio::select! {
            _ = sig_term.recv() => {
                tracing::info!("Received SIGTERM. Exiting...");
                break;
            }
            _ = ctrl_c() => {
                tracing::info!("Received SIGINT. Exiting...");
                break;
            }
            result = &mut collect_status_task => {
                handle_task_result("status_collectors", result);
                collect_status_task = create_collect_status_task();
            }
            result = &mut collect_total_task => {
                handle_task_result("status_collectors", result);
                collect_total_task = create_collect_total_task();
            }
        }
    }
}

async fn create_collect_task(
    influx_client: Arc<influxdb::Client>,
    collectors: Arc<Vec<Box<dyn MetricCollector>>>,
    interval: Duration,
    task_name: &'static str,
) {
    sleep(interval).await;

    let points = batch_collect_metrics(&collectors, Local::now()).await;

    for point in &points {
        tracing::debug!("{:?}", point);
    }

    match influx_client.write(points).await {
        Ok(_) => tracing::info!("Successfully wrote points to InfluxDB ({})", task_name),
        Err(e) => tracing::error!(
            "Failed to write points to InfluxDB ({}): {:?}",
            task_name,
            e
        ),
    }
}

fn handle_task_result(task_name: &str, result: Result<(), JoinError>) {
    match result {
        Ok(_) => {
            tracing::debug!("Task {} completed.", task_name);
        }
        Err(e) => {
            tracing::error!("Task {} failed: {:?}", task_name, e);
        }
    }
}

async fn collect_past_total(
    collectors: Arc<Vec<Box<dyn MetricCollector>>>,
    influx_client: Arc<influxdb::Client>,
    days: u64,
) {
    tracing::info!("Inserting last {} days...", days);
    for i in 1..=days {
        let timestamp = Local::now()
            .sub(Duration::from_secs(i * 24 * 60 * 60))
            .with_time(NaiveTime::default())
            .unwrap();
        let points = batch_collect_metrics(&collectors, timestamp).await;

        for point in &points {
            tracing::debug!("{:?}", point);
        }

        match influx_client.write(points).await {
            Ok(_) => tracing::info!(
                "Successfully wrote points to InfluxDB: day={}",
                timestamp.format("%Y-%m-%d")
            ),
            Err(e) => tracing::error!("Failed to write points to InfluxDB: {:?}", e),
        }
    }
    tracing::info!("Finished inserting last {} days.", days);
}
