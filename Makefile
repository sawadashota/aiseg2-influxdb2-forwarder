SHELL := /bin/bash
.DEFAULT_GOAL := help

.PHONY: status
process: ## Process status
	@docker-compose ps

.PHONY: build
build: ## Build the application
	@echo "Building the application..."
	cargo build --release

.PHONY: start
start: start-middleware build ## Start the application
	@echo "Starting the application..."
	@RUST_LOG=info ./target/release/aiseg2-influxdb2-forwarder

.PHONY: start-middleware
start-middleware: ## Start middleware (InfluxDB and Grafana)
	@echo "Starting middleware..."
	@docker-compose up -d

.PHONY: setup-app
setup-app: ## Setup the application
	@echo "Setting up the application..."
	cargo install

.PHONY: stop
stop-middleware: ## Stop middleware (InfluxDB and Grafana)
	@echo "Stopping middleware..."
	@docker-compose down

.PHONY: setup
setup: start-middleware setup-app ## Setup the application
	@echo "Waiting for InfluxDB to be ready..."
	@sleep 5
	@echo "Setup complete. InfluxDB is auto-configured via Docker."

.PHONY: clean
clean: ## Clean the application
	@echo "Cleaning the application..."
	@docker-compose down -v 2>/dev/null || true
	rm -rf .influxdbv2
	rm -rf .grafana

.PHONY: start-db
start-db: ## Start InfluxDB
	@echo "Starting InfluxDB..."
	@docker-compose up -d influxdb

.PHONY: stop-db
stop-db: ## Stop InfluxDB
	@echo "Stopping InfluxDB..."
	@docker-compose stop influxdb

.PHONY: setup-db
setup-db: start-db ## Setup InfluxDB (auto-configured via Docker)
	@echo "InfluxDB is auto-configured via Docker environment variables."
	@sleep 5

.PHONY: start-grafana
start-grafana: ## Start Grafana
	@echo "Starting Grafana..."
	@docker-compose up -d grafana

.PHONY: stop-grafana
stop-grafana: ## Stop Grafana
	@echo "Stopping Grafana..."
	@docker-compose stop grafana

.PHONY: setup-grafana
setup-grafana: start-grafana ## Setup Grafana (auto-configured via Docker)
	@echo "Grafana is auto-configured via Docker."

.PHONY: logs
logs: ## Show middleware logs
	@docker-compose logs -f

# https://gist.github.com/tadashi-aikawa/da73d277a3c1ec6767ed48d1335900f3
.PHONY: $(shell grep -h -E '^[a-zA-Z_-]+:' $(MAKEFILE_LIST) | sed 's/://')

# https://postd.cc/auto-documented-makefile/
help: ## Show help message
	@grep -h -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'
