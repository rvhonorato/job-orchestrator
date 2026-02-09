# job-orchestrator

![GitHub License](https://img.shields.io/github/license/rvhonorato/job-orchestrator)
![GitHub Release](https://img.shields.io/github/v/release/rvhonorato/job-orchestrator)
[![ci](https://github.com/rvhonorato/job-orchestrator/actions/workflows/ci.yml/badge.svg)](https://github.com/rvhonorato/job-orchestrator/actions/workflows/ci.yml)
[![Codacy Badge](https://app.codacy.com/project/badge/Grade/7f2a8816886645d28cbaac0fead038f9)](https://app.codacy.com/gh/rvhonorato/job-orchestrator/dashboard?utm_source=gh&utm_medium=referral&utm_content=&utm_campaign=Badge_grade)
[![Crates.io](https://img.shields.io/crates/v/job-orchestrator)](https://crates.io/crates/job-orchestrator)
[![Documentation](https://img.shields.io/badge/docs-mdbook-blue)](https://rvhonorato.me/job-orchestrator/)

> An asynchronous job orchestration system for managing
> and distributing computational workloads across
> heterogeneous computing resources with intelligent
> quota-based load balancing.

## Overview

The `job-orchestrator` is a central component of
[WeNMR](https://wenmr.science.uu.nl), a worldwide
e-Infrastructure for structural biology operated by
the [BonvinLab](https://bonvinlab.org) at
[Utrecht University](https://uu.nl). It serves as a
reactive middleware layer that connects web applications
to diverse computing resources.

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
  -F "file=@example/2oob_A.pdb" \
  -F "file=@example/2oob_B.pdb" \
  -F "user_id=1" \
  -F "service=example"

# Check status / Download results
curl -I http://localhost:5000/download/1
curl -o results.zip http://localhost:5000/download/1
```

## Documentation

**[Full Documentation](https://rvhonorato.me/job-orchestrator/)**

- [Installation](https://rvhonorato.me/job-orchestrator/getting-started/installation.html)
- [Architecture](https://rvhonorato.me/job-orchestrator/architecture/overview.html)
- [Configuration](https://rvhonorato.me/job-orchestrator/configuration/server.html)
- [API Reference](https://rvhonorato.me/job-orchestrator/api/server-endpoints.html)
- [Deployment](https://rvhonorato.me/job-orchestrator/deployment/docker.html)

**API Documentation**: Available via Swagger UI at
`http://localhost:5000/swagger/` when running.

## Installation

```bash
# From crates.io
cargo install job-orchestrator

# From source
git clone https://github.com/rvhonorato/job-orchestrator.git
cd job-orchestrator
cargo build --release
```

## Security

The client component executes user-submitted `run.sh` scripts
with the full privileges of the process. No filesystem isolation
(chroot, namespaces, etc.) is enforced — the script has the same
access as the client process itself. Input filenames are sanitized,
and a built-in script validator rejects scripts containing
obviously dangerous patterns (destructive commands, network
exfiltration tools, reverse shells, privilege escalation, etc.).

**Important**: The script validator is a sanity check, not a
sandbox. It can be bypassed by determined actors. Input scripts
are still expected to come from trusted or semi-trusted sources.
True isolation must be enforced at the deployment level.

**Recommended hardening when deploying:**

- Run the client inside a container with resource limits
  (CPU, memory, PIDs)
- Use a read-only root filesystem (`--read-only`)
- Drop all capabilities (`--cap-drop=ALL`)
- Place the client on an internal network with no direct
  external exposure
- Block outbound network access from the client container
  (e.g., `--network=internal` or firewall rules)
- Do not run the container as root
- Mount the job data directory with `noexec` where possible
- Set a per-job timeout to prevent resource exhaustion

## Contributing

Contributions, bug reports, and feature requests are
welcome via
[GitHub Issues](https://github.com/rvhonorato/job-orchestrator/issues).

See the
[Contributing Guide](https://rvhonorato.me/job-orchestrator/development/contributing.html)
for details.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contact

- **Issues**:
  [GitHub Issues](https://github.com/rvhonorato/job-orchestrator/issues)
- **Email**:
  Rodrigo V. Honorato <rvhonorato@protonmail.com>
