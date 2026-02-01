# job-orchestrator

![GitHub License](https://img.shields.io/github/license/rvhonorato/job-orchestrator)
![GitHub Release](https://img.shields.io/github/v/release/rvhonorato/job-orchestrator)
[![ci](https://github.com/rvhonorato/job-orchestrator/actions/workflows/ci.yml/badge.svg)](https://github.com/rvhonorato/job-orchestrator/actions/workflows/ci.yml)
[![Codacy Badge](https://app.codacy.com/project/badge/Grade/7f2a8816886645d28cbaac0fead038f9)](https://app.codacy.com/gh/rvhonorato/job-orchestrator/dashboard?utm_source=gh&utm_medium=referral&utm_content=&utm_campaign=Badge_grade)
[![Crates.io](https://img.shields.io/crates/v/job-orchestrator)](https://crates.io/crates/job-orchestrator)
[![Documentation](https://img.shields.io/badge/docs-mdbook-blue)](https://rvhonorato.me/job-orchestrator/)

> An asynchronous job orchestration system for managing and distributing computational workloads across heterogeneous computing resources with intelligent quota-based load balancing.

## Overview

job-orchestrator is a central component of [WeNMR](https://wenmr.science.uu.nl), a worldwide e-Infrastructure for structural biology operated by the [BonvinLab](https://bonvinlab.org) at [Utrecht University](https://uu.nl). It serves as a reactive middleware layer that connects web applications to diverse computing resources.

**Key Features:**

- Asynchronous job management with Rust + Tokio
- Quota-based load balancing per user/service
- Dual-mode architecture (server + client)
- RESTful API with Swagger UI
- Automatic job cleanup

## Quick Start

```bash
# Using Docker Compose
git clone https://github.com/rvhonorato/job-orchestrator.git
cd job-orchestrator
docker compose up --build

# Submit a job
curl -X POST http://localhost:5000/upload \
  -F "file=@example/run.sh" \
  -F "user_id=1" \
  -F "service=example"

# Check status / Download results
curl -I http://localhost:5000/download/1
curl -o results.zip http://localhost:5000/download/1
```

## Documentation

📚 **[Full Documentation](https://rvhonorato.me/job-orchestrator/)**

- [Installation](https://rvhonorato.me/job-orchestrator/getting-started/installation.html)
- [Architecture](https://rvhonorato.me/job-orchestrator/architecture/overview.html)
- [Configuration](https://rvhonorato.me/job-orchestrator/configuration/server.html)
- [API Reference](https://rvhonorato.me/job-orchestrator/api/server-endpoints.html)
- [Deployment](https://rvhonorato.me/job-orchestrator/deployment/docker.html)

**API Documentation**: Available via Swagger UI at `http://localhost:5000/swagger-ui/` when running.

## Installation

```bash
# From crates.io
cargo install job-orchestrator

# From source
git clone https://github.com/rvhonorato/job-orchestrator.git
cd job-orchestrator
cargo build --release
```

## Contributing

Contributions, bug reports, and feature requests are welcome via [GitHub Issues](https://github.com/rvhonorato/job-orchestrator/issues).

See the [Contributing Guide](https://rvhonorato.me/job-orchestrator/development/contributing.html) for details.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contact

- **Issues**: [GitHub Issues](https://github.com/rvhonorato/job-orchestrator/issues)
- **Email**: Rodrigo V. Honorato <rvhonorato@protonmail.com>
