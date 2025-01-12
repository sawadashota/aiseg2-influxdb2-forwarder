# AiSEG2 to InfluxDB2 Forwarder

This is a simple forwarder that reads data from AiSEG2 and writes it to InfluxDB2.

## Requirements

- Environment
    - Nix `2.24.11`
    - System `aarch64-darwin`
- AiSEG2
    - Model `MKN713`
    - Firmware `Ver.2.97I-01`

## Set up

```shell
cp .envrc.sample .envrc
vi .envrc
# Edit .envrc
direnv allow
```

Prepare InfluxDB2 and Grafana.

```shell
make setup
```

## Run

```shell
make start
```

then, open `http://localhost:3030` in your browser.
