# Building from Source

This guide covers building job-orchestrator from source code.

## Prerequisites

### Rust Toolchain

Install Rust via rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Minimum version: Rust 1.75 (edition 2021)

Verify installation:

```bash
rustc --version
cargo --version
```

### System Dependencies

#### Debian/Ubuntu

```bash
apt-get update
apt-get install -y build-essential libsqlite3-dev pkg-config
```

#### Fedora/RHEL

```bash
dnf install gcc sqlite-devel
```

#### macOS

```bash
brew install sqlite
```

#### Windows

Install Visual Studio Build Tools and SQLite development libraries.

## Clone Repository

```bash
git clone https://github.com/rvhonorato/job-orchestrator.git
cd job-orchestrator
```

## Build Commands

### Debug Build

Fast compilation, includes debug symbols:

```bash
cargo build
```

Binary location: `target/debug/job-orchestrator`

### Release Build

Optimized for performance:

```bash
cargo build --release
```

Binary location: `target/release/job-orchestrator`

### Check (No Build)

Verify code compiles without producing binary:

```bash
cargo check
```

## Running

### From Cargo

```bash
# Server mode
cargo run -- server --port 5000

# Client mode
cargo run -- client --port 9000
```

### From Binary

```bash
# After release build
./target/release/job-orchestrator server --port 5000
```

## Build Options

### Features

Currently no optional features. All functionality is included by default.

### Target Platforms

Cross-compile for different targets:

```bash
# Add target
rustup target add x86_64-unknown-linux-musl

# Build for target
cargo build --release --target x86_64-unknown-linux-musl
```

Common targets:

- `x86_64-unknown-linux-gnu` - Linux (glibc)
- `x86_64-unknown-linux-musl` - Linux (static)
- `x86_64-apple-darwin` - macOS Intel
- `aarch64-apple-darwin` - macOS Apple Silicon
- `x86_64-pc-windows-msvc` - Windows

## Docker Build

### Using Dockerfile

```bash
docker build -t job-orchestrator .
```

### Multi-stage Build

The Dockerfile uses multi-stage builds for smaller images:

1. **Builder stage**: Compiles with full toolchain
2. **Runtime stage**: Minimal image with just the binary

## See Also

- [Testing](./testing.md)
- [Contributing](./contributing.md)
