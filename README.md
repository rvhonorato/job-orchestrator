# job-orchestrator

![GitHub License](https://img.shields.io/github/license/rvhonorato/job-orchestrator)
![GitHub Release](https://img.shields.io/github/v/release/rvhonorato/job-orchestrator)
[![ci](https://github.com/rvhonorato/job-orchestrator/actions/workflows/ci.yml/badge.svg)](https://github.com/rvhonorato/job-orchestrator/actions/workflows/ci.yml)
[![Codacy Badge](https://app.codacy.com/project/badge/Grade/7f2a8816886645d28cbaac0fead038f9)](https://app.codacy.com/gh/rvhonorato/job-orchestrator/dashboard?utm_source=gh&utm_medium=referral&utm_content=&utm_campaign=Badge_grade)
[![Crates.io](https://img.shields.io/crates/v/job-orchestrator)](https://crates.io/crates/job-orchestrator)
[![Documentation](https://img.shields.io/badge/docs-mdbook-blue)](https://rvhonorato.me/job-orchestrator/)

> An asynchronous job orchestration system for managing and distributing
> computational workloads across heterogeneous computing resources with
> intelligent quota-based load balancing.

Part of [WeNMR](https://wenmr.science.uu.nl), operated by
[BonvinLab](https://bonvinlab.org) at [Utrecht University](https://uu.nl).

## Quick Start

```bash
git clone https://github.com/rvhonorato/job-orchestrator.git
cd job-orchestrator
docker compose up --build
```

The server starts on port 5000. Submit a job:

```bash
curl -X POST http://localhost:5000/upload \
  -F "file=@example/run.sh" \
  -F "file=@example/2oob_A.pdb" \
  -F "file=@example/2oob_B.pdb" \
  -F "user_id=1" \
  -F "service=example"
```

Interactive API docs available at `http://localhost:5000/swagger`.

## Documentation

**[Full Documentation →](https://rvhonorato.me/job-orchestrator/)**

- [Installation](https://rvhonorato.me/job-orchestrator/getting-started/installation.html)
- [Your First Job](https://rvhonorato.me/job-orchestrator/getting-started/first-job.html)
- [Architecture](https://rvhonorato.me/job-orchestrator/architecture/overview.html)
- [Configuration](https://rvhonorato.me/job-orchestrator/configuration/server.html)
- [API Reference](https://rvhonorato.me/job-orchestrator/api/server-endpoints.html)
- [Deployment](https://rvhonorato.me/job-orchestrator/deployment/docker.html)
- [Security](https://rvhonorato.me/job-orchestrator/deployment/production.html#security)

## License

MIT — see [LICENSE](LICENSE) for details.

## Contact

- **Issues**: [GitHub Issues](https://github.com/rvhonorato/job-orchestrator/issues)
- **Email**: Rodrigo V. Honorato <rvhonorato@protonmail.com>
