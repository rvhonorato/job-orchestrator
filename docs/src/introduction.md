# Introduction

**job-orchestrator** is an asynchronous job orchestration system for managing and distributing computational workloads across heterogeneous computing resources with intelligent quota-based load balancing.

It serves as a middleware layer that connects web applications to computing resources: users submit files and a `run.sh` script via a REST API; the server queues and routes jobs to client nodes that execute them; results are returned as a ZIP archive.

job-orchestrator is a central component of [WeNMR](https://wenmr.science.uu.nl), a worldwide e-Infrastructure for structural biology operated by the [BonvinLab](https://bonvinlab.org) at [Utrecht University](https://uu.nl).

## Key Features

- **Asynchronous Job Management**: Built with Rust and Tokio for high-performance async operations
- **Quota-Based Load Balancing**: Per-user, per-service quotas prevent resource exhaustion
- **Dual-Mode Architecture**: Runs as server (job orchestration) or client (job execution)
- **RESTful API**: Simple HTTP interface for job submission, retrieval, and cancellation
- **Automatic Cleanup**: Configurable retention policies for completed jobs
- **Job Termination**: Cancel running jobs via API endpoints

## Use Cases

job-orchestrator is designed for scenarios requiring:

- **Scientific Computing Workflows**: Distribute computational biology/chemistry jobs across clusters
- **Multi-Tenant Systems**: Fair resource allocation with per-user quotas
- **Heterogeneous Computing**: Route jobs to appropriate backends (local, HPC, cloud)
- **Web-Based Science Platforms**: Decouple frontend from compute infrastructure
- **Batch Processing**: Handle high-throughput job submissions with automatic queuing

## Get Started

- [Quick Start](./getting-started/quick-start.md) — up and running with Docker Compose in minutes
- [Your First Job](./getting-started/first-job.md) — submit and retrieve a job
- [Architecture Overview](./architecture/overview.md) — understand how the system works

## Project Status

**Current State**: Production-ready with server/client architecture

**Planned Features**:

- DIRAC Interware integration
- SLURM direct integration
- Enhanced monitoring and metrics
- Job priority queues
- Advanced scheduling policies

## Getting Help

- **API Documentation**: Available via Swagger UI at `/swagger` when running
- **Issues**: [GitHub Issues](https://github.com/rvhonorato/job-orchestrator/issues)
- **Email**: Rodrigo V. Honorato <rvhonorato@protonmail.com>

## License

MIT License - see [LICENSE](https://github.com/rvhonorato/job-orchestrator/blob/main/LICENSE) for details.

---

*Parts of this documentation were automatically generated with the assistance of AI tools and human reviewed for accuracy.*
