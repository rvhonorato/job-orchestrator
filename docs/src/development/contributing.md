# Contributing

Contributions to job-orchestrator are welcome! This guide explains how to contribute.

## Ways to Contribute

- **Bug reports**: Found a bug? Open an issue
- **Feature requests**: Have an idea? Open an issue to discuss
- **Code contributions**: Fix bugs or implement features
- **Documentation**: Improve docs, fix typos, add examples
- **Testing**: Add tests, report edge cases

## Getting Started

### 1. Fork the Repository

Click "Fork" on GitHub, then clone your fork:

```bash
git clone https://github.com/YOUR_USERNAME/job-orchestrator.git
cd job-orchestrator
```

### 2. Set Up Development Environment

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install dependencies (Debian/Ubuntu)
apt-get install libsqlite3-dev

# Build
cargo build

# Run tests
cargo test
```

### 3. Create a Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/your-bug-fix
```

## Development Workflow

### Making Changes

1. Write code
2. Add tests for new functionality
3. Run tests: `cargo test`
4. Run linter: `cargo clippy -- -D warnings`
5. Format code: `cargo fmt`

### Commit Messages

Follow conventional commits:

```
type(scope): description

[optional body]
```

Types:

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `refactor`: Code refactoring
- `test`: Adding tests
- `chore`: Maintenance

Examples:

```
feat(quota): add per-service quota limits
fix(client): handle non-zero exit codes correctly
docs(readme): update quick start instructions
```

### Pull Request Process

1. Push your branch:

   ```bash
   git push origin feature/your-feature-name
   ```

2. Open a Pull Request on GitHub

3. Fill in the PR template:
   - Describe your changes
   - Link related issues
   - Note any breaking changes

4. Wait for review
   - CI must pass
   - Maintainer will review
   - Address feedback

5. Merge!

## Code Style

### Rust Style

Follow standard Rust conventions:

- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Prefer descriptive variable names
- Add doc comments for public APIs

```rust
/// Uploads a job to the orchestrator.
///
/// # Arguments
///
/// * `files` - Files to upload
/// * `user_id` - User submitting the job
/// * `service` - Target service name
///
/// # Returns
///
/// The created job with its ID and initial status.
pub async fn upload_job(
    files: Vec<File>,
    user_id: i32,
    service: String,
) -> Result<Job, Error> {
    // Implementation
}
```

### Error Handling

- Use `Result` types, not panics
- Provide context with error messages
- Use `?` operator for propagation

```rust
// Good
let file = File::open(path)
    .map_err(|e| Error::FileOpen { path: path.clone(), source: e })?;

// Avoid
let file = File::open(path).unwrap();
```

## Testing Guidelines

### Write Tests For

- New functionality
- Bug fixes (regression tests)
- Edge cases

### Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_describes_behavior() {
        // Arrange
        let input = create_test_input();

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

### Async Tests

```rust
#[tokio::test]
async fn test_async_operation() {
    let result = async_function().await;
    assert!(result.is_ok());
}
```

## Documentation

### Code Documentation

Add doc comments for:

- Public functions
- Public structs
- Public modules

```rust
/// A job represents a unit of work to be processed.
pub struct Job {
    /// Unique identifier for the job.
    pub id: i32,
    /// User who submitted the job.
    pub user_id: i32,
    // ...
}
```

### mdbook Documentation

Documentation is in `docs/src/`. To preview:

```bash
# Install mdbook
cargo install mdbook

# Serve locally
cd docs
mdbook serve --open
```

## Reporting Issues

### Bug Reports

Include:

- job-orchestrator version
- Operating system
- Steps to reproduce
- Expected vs actual behavior
- Logs if relevant

### Feature Requests

Include:

- Use case description
- Proposed solution (if any)
- Alternatives considered

## Code of Conduct

Be respectful and constructive. We're all here to build something useful together.

## Questions?

- Open an issue for questions
- Email: Rodrigo V. Honorato <rvhonorato@protonmail.com>

## License

Contributions are licensed under the MIT License.
