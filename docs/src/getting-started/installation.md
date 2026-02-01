# Installation

There are several ways to install job-orchestrator depending on your needs.

## From crates.io

The easiest way to install job-orchestrator is via Cargo:

```bash
cargo install job-orchestrator
```

## From Source

Clone the repository and build:

```bash
git clone https://github.com/rvhonorato/job-orchestrator.git
cd job-orchestrator
cargo build --release
```

The binary will be available at `target/release/job-orchestrator`.

## Using Docker

Pull the pre-built image:

```bash
docker pull ghcr.io/rvhonorato/job-orchestrator:latest
```

Or build locally:

```bash
docker build -t job-orchestrator .
```

## Prerequisites

### For Building from Source

- **Rust**: 1.75 or later (edition 2021)
- **SQLite**: Development libraries

On Debian/Ubuntu:

```bash
apt-get install libsqlite3-dev
```

On macOS:

```bash
brew install sqlite
```

### For Running

- **SQLite**: Runtime library (usually included in most systems)
- **Filesystem access**: Write permissions for database and job storage directories

## Verifying Installation

After installation, verify it works:

```bash
job-orchestrator --version
```

You should see the version number displayed.

## Next Steps

- [Quick Start](./quick-start.md) - Get running with Docker Compose
- [Your First Job](./first-job.md) - Submit and retrieve a job
