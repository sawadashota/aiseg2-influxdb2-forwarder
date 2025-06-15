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
