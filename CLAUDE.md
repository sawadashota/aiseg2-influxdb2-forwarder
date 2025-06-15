# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

AiSEG2 to InfluxDB2 Forwarder - A Rust application that collects energy monitoring metrics from Panasonic AiSEG2 systems and forwards them to InfluxDB2 for storage and visualization with Grafana.

## Essential Commands

```bash
# Development environment setup
make setup           # Setup InfluxDB and Grafana containers
make start          # Start all services (InfluxDB, Grafana, and forwarder)
make stop-middleware # Stop middleware services
make clean          # Clean data directories

# Build and test
cargo build --release
cargo test
cargo fmt --check   # Format check (used in CI)

# Run the application
cargo run
```

## Architecture

### Collector Pattern
The application uses a modular collector architecture with a `MetricCollector` trait. Collectors are divided into:
- **Status collectors** (5-second interval): `PowerMetricCollector`, `ClimateMetricCollector`
- **Total collectors** (60-second interval): `DailyTotalMetricCollector`, `CircuitDailyTotalMetricCollector`

Each collector:
1. Scrapes specific pages from the AiSEG2 web interface
2. Parses HTML using the `scraper` crate
3. Returns `Vec<DataPoint>` for InfluxDB

### Async Task Management
- Uses Tokio runtime with separate tasks for status and total collection
- 10-second timeout for metric collection (configurable via `COLLECTOR_TASK_TIMEOUT_SECONDS`)
- Graceful shutdown on SIGTERM/SIGINT signals
- Failed tasks are automatically restarted

### Configuration
All configuration is via environment variables with these prefixes:
- `AISEG2_*` - AiSEG2 connection settings
- `INFLUXDB_*` - InfluxDB connection settings
- `COLLECTOR_*` - Collection intervals and behavior
- `LOG_LEVEL` - Logging configuration

### Key Source Files
- `src/main.rs` - Application entry point and task orchestration
- `src/config.rs` - Configuration management
- `src/collector/mod.rs` - Collector trait and implementations
- `src/influxdb.rs` - InfluxDB client wrapper