# Launchly

Launchly turns a Git repository into an explainable deployment plan. This repository contains the first runnable MVP: a Rust/Axum API and lightweight HTML dashboard.

## Run locally

From the repository root:

```bash
cargo run --manifest-path backend/Cargo.toml
```

Open <http://localhost:8080>. The MVP currently plans from repository URL signals only; it does not clone or build untrusted repositories yet.

## Run with Podman

Ensure the Podman machine is running on macOS:

```bash
podman machine start
podman build -t launchly:dev -f Containerfile .
podman run --rm --name launchly -p 8080:8080 launchly:dev
```

Then open <http://localhost:8080>.

## API

```bash
curl http://localhost:8080/api/health
curl -X POST http://localhost:8080/api/plans \
  -H 'content-type: application/json' \
  -d '{"repository_url":"https://github.com/example/node-dashboard.git"}'
```

The next implementation stages are repository cloning in an isolated workspace, evidence-based manifest detection, Podman build/run adapters, health checks, persistent deployment state, and Caddy activation after verification.
