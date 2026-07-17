# Contributing to geo-redis

Thank you for taking the time to contribute.

## Development setup

```powershell
# Windows
.\scripts\setup.ps1
.\scripts\run-demo.ps1           # single-node aircraft tracker
.\scripts\demo-cluster.ps1      # 4-node distributed cluster
```

```bash
# Linux / macOS
./scripts/setup.sh && ./scripts/run-demo.sh
```

## Workflow

1. **Fork** the repository and create a feature branch.
2. **Write tests.** Library changes must include unit or integration tests in `lib/tests/`.
3. **Run CI locally** before pushing:
   ```bash
   cargo fmt --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   ```
4. **Open a pull request** against `main`. Describe the problem being solved and how to verify the fix.

## Project structure

| Path | What lives here |
|------|----------------|
| `lib/` | Core Rust library — publishable to crates.io |
| `demo/geo-node/` | Distributed shard daemon (gossip, split, merge) |
| `demo/server/` | Single-node demo backend (OpenSky poller) |
| `demo/weather-server/` | METAR weather demo backend |
| `demo/ui/` | React + Leaflet frontend (aircraft + weather) |
| `demo/cluster-ui/` | React cluster-monitor frontend |
| `demo/cluster-test/` | Integration tests using real Redis containers |
| `demo/k8s/` | Kubernetes manifests |

## Current maturity

geo-redis is **experimental**. The core library and single-node demo are production-ready;
the distributed split/merge protocol is functional but lacks a consensus layer — see
[TECHNICAL.md §5.3](TECHNICAL.md) for the documented gap.

Contributions that improve:
- Split/merge correctness (consensus, idempotent state machine)
- Failure-injection tests
- TLS / service-to-service auth
- Production operations tooling

…are especially welcome.

## Commit messages

Use the [Conventional Commits](https://www.conventionalcommits.org/) style:
`fix:`, `feat:`, `docs:`, `ci:`, `test:`, `chore:`.

## Code of conduct

Be respectful, constructive, and welcoming. We follow the
[Contributor Covenant v2.1](https://www.contributor-covenant.org/version/2/1/code_of_conduct/).
