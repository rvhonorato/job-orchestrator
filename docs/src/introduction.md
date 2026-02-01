# Introduction

**job-orchestrator** is an asynchronous job orchestration system for managing and distributing computational workloads across heterogeneous computing resources with intelligent quota-based load balancing.

## What is job-orchestrator?

job-orchestrator is a central component of [WeNMR](https://wenmr.science.uu.nl), a worldwide e-Infrastructure for structural biology operated by the [BonvinLab](https://bonvinlab.org) at [Utrecht University](https://uu.nl). It serves as a reactive middleware layer that connects web applications to diverse computing resources, enabling efficient job distribution for scientific computing workflows.

## Key Features

- **Asynchronous Job Management**: Built with Rust and Tokio for high-performance async operations
- **Quota-Based Load Balancing**: Per-user, per-service quotas prevent resource exhaustion
- **Dual-Mode Architecture**: Runs as server (job orchestration) or client (job execution)
- **Multiple Backend Support**: Extensible to integrate with various computing resources:
  - Native client mode for local job execution
  - [DIRAC Interware](https://dirac.readthedocs.io/en/latest/index.html) *(planned)*
  - SLURM clusters *(planned)*
  - Educational cloud services *(planned)*
- **RESTful API**: Simple HTTP interface for job submission and retrieval
- **Automatic Cleanup**: Configurable retention policies for completed jobs

## Use Cases

job-orchestrator is designed for scenarios requiring:

- **Scientific Computing Workflows**: Distribute computational biology/chemistry jobs across clusters
- **Multi-Tenant Systems**: Fair resource allocation with per-user quotas
- **Heterogeneous Computing**: Route jobs to appropriate backends (local, HPC, cloud)
- **Web-Based Science Platforms**: Decouple frontend from compute infrastructure
- **Batch Processing**: Handle high-throughput job submissions with automatic queuing

## Project Status

**Current State**: Production-ready with server/client architecture

**Planned Features**:

- **Auto-Scaling**: Dynamic creation and termination of cloud-based client instances based on workload
- DIRAC Interware integration
- SLURM direct integration
- Enhanced monitoring and metrics
- Job priority queues
- Advanced scheduling policies

## Getting Help

- **API Documentation**: Available via Swagger UI at `/swagger-ui/` when running
- **Issues**: [GitHub Issues](https://github.com/rvhonorato/job-orchestrator/issues)
- **Email**: Rodrigo V. Honorato <rvhonorato@protonmail.com>

## License

MIT License - see [LICENSE](https://github.com/rvhonorato/job-orchestrator/blob/main/LICENSE) for details.
