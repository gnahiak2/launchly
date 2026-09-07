# Launchly

Launchly turns a Git repository into an explainable deployment plan. This repository contains the first runnable MVP: a Rust/Axum API and lightweight HTML dashboard.

## Run locally

From the repository root:

```bash
cargo run --manifest-path backend/Cargo.toml
```

Open <http://localhost:8080>. Repository planning is based on inspected files when using the deployment API. Vanilla HTML sites are detected as static sites with no package manager or build step.

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

## Deployment API

The backend now exposes a deployment job pipeline:

```bash
curl -X POST http://localhost:8080/api/deployments \
  -H 'content-type: application/json' \
  -d '{"repository_url":"https://github.com/example/app.git"}'

curl http://localhost:8080/api/deployments/<deployment-id>
```

Jobs move through `queued`, `discovering`, `planning`, `building`, `starting`, `verifying`, `live`, or `failed`. The pipeline clones with a bounded, argument-based Git command, inspects manifests without executing repository commands, builds a Dockerfile/Containerfile image with Podman, starts it with resource limits, and requires an HTTP 200 health response before marking it live.

For deployment execution, Launchly needs access to the Podman service and to the workspace visible to that service. Running the backend directly on the host is the simplest supported development mode. Running Launchly itself inside a container requires mounting the Podman API socket and a shared workspace volume; do not expose the Podman socket to untrusted containers.

Caddy should only be configured to route to records in the `live` state. The current slice records an optional validated domain but does not reload Caddy automatically yet.

## Optional Gemini fallback

If local manifest and Dockerfile detection cannot identify a stack, Launchly can use Gemini as an optional second opinion:

```bash
export GEMINI_API_KEY=your-key
```

Gemini is never required. Without the key, Launchly keeps the safe unknown/failed result. When enabled, Launchly sends a bounded snapshot of repository filenames and small text excerpts, skips `.env`, secret, credential, token, PEM, and key files, excludes `.git`, dependencies, and build directories, and validates the returned JSON before using it. Do not configure this if repository metadata must remain entirely on-host; external API, privacy, quota, and cost policies apply.
