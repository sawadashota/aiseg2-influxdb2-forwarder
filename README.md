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

## Configuration

### Environment Variables

#### Required Variables
- `AISEG2_URL`: Base URL of your AiSEG2 system
- `AISEG2_USER`: Username for AiSEG2 authentication
- `AISEG2_PASSWORD`: Password for AiSEG2 authentication
- `INFLUXDB_URL`: InfluxDB server URL
- `INFLUXDB_TOKEN`: InfluxDB authentication token
- `INFLUXDB_ORG`: InfluxDB organization
- `INFLUXDB_BUCKET`: InfluxDB bucket for storing metrics

#### Optional Variables
- `LOG_LEVEL`: Logging level (default: `info`)
- `COLLECTOR_STATUS_INTERVAL_SEC`: Interval for status metrics collection (default: `5`)
- `COLLECTOR_TOTAL_INTERVAL_SEC`: Interval for total metrics collection (default: `60`)
- `COLLECTOR_TOTAL_INITIAL_DAYS`: Days of historical data to collect on startup (default: `30`)

#### Circuit Breaker Configuration
The application includes a circuit breaker pattern to handle collector failures gracefully:

- `CIRCUIT_BREAKER_FAILURE_THRESHOLD`: Number of consecutive failures before opening circuit (default: `5`)
- `CIRCUIT_BREAKER_RECOVERY_TIMEOUT_SECONDS`: Seconds to wait before attempting recovery (default: `60`)
- `CIRCUIT_BREAKER_HALF_OPEN_SUCCESS_THRESHOLD`: Successful calls needed to close circuit (default: `3`)
- `CIRCUIT_BREAKER_HALF_OPEN_FAILURE_THRESHOLD`: Failures allowed in half-open state before reopening (default: `1`)

When a collector fails repeatedly, the circuit breaker will:
1. Open after the failure threshold is reached, preventing further calls
2. Wait for the recovery timeout before entering half-open state
3. Allow limited calls in half-open state to test recovery
4. Close fully after sufficient successful calls or reopen on failure

## Developer Guidelines

### Testing Principles

This project follows specific testing patterns to ensure code reliability and maintainability:

#### Test Structure
All tests must be organized under `succeeds` or `fails` submodules based on whether an error is expected:
- **`succeeds`**: Tests where the function is expected to return `Ok(...)` or complete without errors
- **`fails`**: Tests where the function is expected to return `Err(...)` or fail with an error

```rust
#[cfg(test)]
mod tests {
    mod succeeds {
        // Tests that expect successful execution
    }
    
    mod fails {
        // Tests that expect errors
    }
}
```

#### Table-Driven Tests
Prefer table-driven tests for better readability and maintainability when testing multiple scenarios:

```rust
let test_cases = vec![
    ("test_name", input, expected_output),
    // more cases...
];

for (name, input, expected) in test_cases {
    // test implementation
}
```

#### Environment Variable Testing
Use `serial_test` crate for tests that modify environment variables to prevent race conditions:

```rust
#[test]
#[serial]
fn test_with_env_vars() {
    // Save original values
    let original = std::env::var("VAR_NAME").ok();
    
    // Test logic
    
    // Restore original values
    match original {
        Some(val) => std::env::set_var("VAR_NAME", val),
        None => std::env::remove_var("VAR_NAME"),
    }
}
```

### Pre-Push Checklist

**IMPORTANT**: Always run these commands before pushing commits:

```bash
# 1. Format check (required by CI)
cargo fmt --check

# 2. Clippy linting (required by CI)
cargo clippy -- -D warnings

# 3. Run all tests
cargo test

# 4. Build release version
cargo build --release

# 5. (Optional) Run with debug logging to verify functionality
RUST_LOG=debug cargo run
```

If any of these commands fail, fix the issues before committing.

### Development Best Practices

#### Error Handling
- Use `anyhow::Result` for error propagation
- Provide context with `.context()` for better error messages
- Use custom error types when domain-specific errors are needed

#### Async Programming
- All collectors use `async/await` with Tokio runtime
- Use timeouts for external requests (default: 10 seconds)
- Handle task cancellation gracefully

#### Testing External Services
- Use `mockito` or `wiremock` for HTTP mocking
- Create test utilities for common mock scenarios
- Always test both success and failure paths

#### Code Organization
- Keep collectors modular and implement the `MetricCollector` trait
- Place test utilities in a `test_utils` module within test blocks
- Use meaningful module names that reflect functionality

### Available Make Commands

```bash
make help          # Show all available commands
make setup         # Initial setup (InfluxDB, Grafana, app)
make build         # Build the application
make start         # Start all services
make stop-middleware # Stop InfluxDB and Grafana
make clean         # Clean data directories
```

### Continuous Integration

The GitHub Actions workflow runs on every push and checks:
1. Code builds successfully (`cargo build`)
2. Code formatting is correct (`cargo fmt --check`)
3. No clippy warnings (`cargo clippy -- -D warnings`)
4. All tests pass (`cargo test`)

Ensure your changes pass all these checks locally before pushing.
