# Testing

This guide covers running and writing tests for job-orchestrator.

## Running Tests

### All Tests

```bash
cargo test
```

### With Output

See println! output from tests:

```bash
cargo test -- --nocapture
```

### Specific Test

```bash
# By name
cargo test test_upload

# By module
cargo test orchestrator::tests
```

### Ignored Tests

Some tests may be ignored by default (slow, require setup):

```bash
cargo test -- --ignored
```

## Test Coverage

### Using cargo-tarpaulin

Install:

```bash
cargo install cargo-tarpaulin
```

Generate coverage:

```bash
# HTML report
cargo tarpaulin --out Html --output-dir ./coverage

# XML report (for CI)
cargo tarpaulin --out Xml --output-dir ./coverage
```

View report:

```bash
open coverage/tarpaulin-report.html
```

## Test Structure

Tests are organized alongside the code they test:

```
src/
├── lib.rs
├── orchestrator/
│   ├── mod.rs
│   └── tests.rs      # Orchestrator tests
├── client/
│   ├── mod.rs
│   └── tests.rs      # Client tests
└── utils/
    └── mod.rs        # Inline tests with #[cfg(test)]
```

## Writing Tests

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        let result = function_under_test();
        assert_eq!(result, expected_value);
    }
}
```

### Async Tests

Use `tokio::test` for async functions:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_function() {
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

### Integration Tests

Create files in `tests/` directory:

```rust
// tests/integration_test.rs
use job_orchestrator::*;

#[tokio::test]
async fn test_full_workflow() {
    // Setup
    // Test
    // Verify
}
```

## Mocking

### Using mockall

For trait-based mocking:

```rust
use mockall::automock;

#[automock]
trait Database {
    fn get(&self, id: i32) -> Option<Job>;
}

#[test]
fn test_with_mock() {
    let mut mock = MockDatabase::new();
    mock.expect_get()
        .with(eq(1))
        .returning(|_| Some(Job::default()));

    // Use mock in test
}
```

### Using mockito

For HTTP mocking:

```rust
use mockito::Server;

#[tokio::test]
async fn test_http_client() {
    let mut server = Server::new();
    let mock = server.mock("GET", "/health")
        .with_status(200)
        .create();

    // Test against server.url()

    mock.assert();
}
```

## Test Utilities

### Test Fixtures

Create reusable test data:

```rust
#[cfg(test)]
mod test_utils {
    pub fn create_test_job() -> Job {
        Job {
            id: 1,
            user_id: 1,
            service: "test".to_string(),
            status: Status::Queued,
            ..Default::default()
        }
    }
}
```

### Temporary Directories

Use `tempfile` for temporary test directories:

```rust
use tempfile::TempDir;

#[test]
fn test_file_operations() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    // Test file operations

    // TempDir is automatically cleaned up
}
```

## CI Testing

Tests run automatically on GitHub Actions:

```yaml
# .github/workflows/ci.yml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test
```

## Linting

### Clippy

Run Clippy for additional checks:

```bash
cargo clippy -- -D warnings
```

Common fixes:

- `#[allow(clippy::lint_name)]` to suppress specific lints
- Configure in `clippy.toml` or `Cargo.toml`

### Formatting

Check formatting:

```bash
cargo fmt -- --check
```

Fix formatting:

```bash
cargo fmt
```

## Debugging Tests

### With println

```rust
#[test]
fn test_debug() {
    let value = compute_something();
    println!("Debug value: {:?}", value);  // Use --nocapture to see
    assert!(value.is_valid());
}
```

### With RUST_BACKTRACE

```bash
RUST_BACKTRACE=1 cargo test
```

## See Also

- [Building from Source](./building.md)
- [Contributing](./contributing.md)
