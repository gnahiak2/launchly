# Launchly

Launchly is a self-hosted deployment platform for turning a Git repository into a running website.

Give Launchly a repository and it will:

```text
Git repository
      ↓
Inspect
      ↓
Explain a deployment plan
      ↓
Build with Podman
      ↓
Run in an isolated container
      ↓
Verify health
      ↓
Live
```

The project is intentionally focused on simple, explainable, single-machine deployments rather than Kubernetes-style orchestration.

## Current capabilities

- Rust/Axum backend with JSON APIs.
- Lightweight server-served frontend.
- TypeScript frontend client compiled during the container build.
- Shallow Git repository inspection with bounded timeouts.
- Detection for:
  - Rust/Cargo projects;
  - Node.js projects using npm, pnpm, or Yarn;
  - Python projects using pip or Poetry;
  - Dockerfile/Containerfile-based applications;
  - vanilla HTML/CSS/JavaScript websites.
- Static website support for repositories containing `index.html`.
- Explainable plans containing framework, language, package manager, commands, port, confidence, and rationale.
- Optional Gemini fallback for repositories that local detection cannot identify.
- Asynchronous deployment jobs with observable states.
- Podman image builds and isolated application containers.
- Resource limits, dropped capabilities, read-only runtime filesystems, and HTTP health checks.
- Cleanup of failed deployment workspaces, images, and containers.

## Requirements

For local development:

- Rust 1.88 or newer;
- Cargo;
- Git;
- Node.js and npm for frontend TypeScript compilation;
- Podman 4 or newer for image builds and application containers.

On macOS, start the Podman virtual machine before using Podman:

```bash
podman machine start
```

## Run with Podman

Build the complete backend and frontend image:

```bash
podman build -t launchly:dev -f Containerfile .
```

Run Launchly:

```bash
podman rm -f launchly 2>/dev/null || true
podman run -d \
  --name launchly \
  -p 8080:8080 \
  launchly:dev
```

Open the dashboard at <http://localhost:8080>.

Check that the service is healthy:

```bash
curl http://localhost:8080/api/health
```

Expected response:

```json
{"status":"ok"}
```

View application logs or stop the service:

```bash
podman logs -f launchly
podman stop launchly
```

### Podman-in-Podman note

The runtime image includes the Podman client because deployment jobs invoke Podman. When Launchly itself runs inside a container, deployment execution also requires:

1. access to a Podman API socket;
2. a workspace path visible to both Launchly and the Podman service.

Running the backend directly on the host is the simplest development setup. Do not expose the Podman socket to arbitrary application containers or untrusted workloads.

## Local development

Run the backend directly from the repository root:

```bash
cargo run --manifest-path backend/Cargo.toml
```

Build the frontend TypeScript client:

```bash
cd frontend
npm install
npm run build
cd ..
```

The container build performs this frontend compilation automatically. The backend serves the generated browser bundle from `frontend/app.js` in the runtime image.

Run backend tests in a Rust 1.88+ environment:

```bash
cargo test --manifest-path backend/Cargo.toml
```

Format Rust code:

```bash
cargo fmt --manifest-path backend/Cargo.toml
```

## Dashboard workflow

1. Open <http://localhost:8080>.
2. Enter a public HTTP(S) Git repository URL.
3. Click **Inspect repository**.
4. Launchly shallow-clones the repository into a temporary isolated workspace.
5. The plan is generated from actual repository files—not only from the repository name.
6. Review the detected stack, commands, port, confidence, and rationale.
7. Optionally provide a hostname.
8. Click **Deploy application**.
9. Watch the deployment status as it moves through the deployment lifecycle.

Vanilla HTML repositories containing `index.html` are planned as static Nginx websites:

```text
Framework:       Vanilla HTML
Package manager: None
Build command:   No build required
Port:            8080
```

## API

### Health

```bash
curl http://localhost:8080/api/health
```

### Inspect a repository

`POST /api/plans` performs a shallow clone and inspects the repository contents. The temporary checkout is removed after planning.

```bash
curl -X POST http://localhost:8080/api/plans \
  -H 'content-type: application/json' \
  -d '{"repository_url":"https://github.com/gnahiak2/gnahiak2.github.io.git"}'
```

A vanilla HTML response looks like:

```json
{
  "repository_url": "https://github.com/gnahiak2/gnahiak2.github.io.git",
  "framework": "Vanilla HTML",
  "language": "HTML/CSS/JavaScript",
  "package_manager": "None",
  "build_command": "No build required",
  "start_command": "nginx -g 'daemon off;'",
  "port": 8080,
  "confidence": "high",
  "rationale": [
    "Detected index.html without a framework manifest.",
    "This repository can be served as a static site without a package manager."
  ]
}
```

### Start a deployment

```bash
curl -X POST http://localhost:8080/api/deployments \
  -H 'content-type: application/json' \
  -d '{
    "repository_url":"https://github.com/gnahiak2/gnahiak2.github.io.git",
    "domain":"example.com"
  }'
```

The API immediately returns a deployment record and an ID.

### Check deployment status

```bash
curl http://localhost:8080/api/deployments/<deployment-id>
```

Deployment states are:

```text
queued
  → discovering
  → planning
  → building
  → starting
  → verifying
  → live
```

A deployment can enter `failed` from any execution phase. Failures include a safe diagnostic message and trigger cleanup of temporary resources.

## Optional Gemini fallback

If local repository signals are insufficient, Launchly can use Gemini as an optional second opinion. It is disabled unless `GEMINI_API_KEY` is configured:

```bash
podman rm -f launchly 2>/dev/null || true
podman run -d \
  --name launchly \
  -p 8080:8080 \
  -e GEMINI_API_KEY='your-api-key' \
  launchly:dev
```

Gemini receives a bounded repository snapshot, not the full repository. Launchly excludes `.git`, dependency directories, build output, environment files, and common secret/key filenames. Returned JSON is validated before it can influence a plan.

Using Gemini has external API, privacy, quota, and cost implications. Do not configure it when repository metadata must remain entirely on the deployment host.

## Security and safety boundaries

Launchly treats repository contents and repository URLs as untrusted input.

- Git commands use argument arrays rather than shell interpolation.
- Repository URLs must use HTTP(S), include a host and path, and cannot contain credentials, query strings, fragments, localhost targets, or path traversal patterns.
- Clone, build, and run operations have timeouts.
- Application containers use explicit names and ports.
- Application containers run with a read-only filesystem, dropped capabilities, `no-new-privileges`, CPU/memory/PID limits, and bridge networking.
- Static-site build contexts exclude common secret and dependency paths.
- A deployment is not marked live until an HTTP health check returns status 200.
- Caddy must only be configured for deployments that have reached `live`.
- Secrets must not be placed in repository files, image layers, command arguments, deployment plans, or logs.

## Project structure

```text
.
├── backend/
│   ├── Cargo.toml
│   └── src/
│       ├── api.rs       # Axum handlers and API translation
│       ├── deploy.rs    # Deployment jobs, Git, Podman, health checks
│       ├── detector.rs  # Deterministic repository detection
│       ├── gemini.rs    # Optional bounded Gemini fallback
│       ├── models.rs    # Serializable request and domain models
│       └── main.rs      # Application startup and routing
├── frontend/
│   ├── app.ts          # Typed browser API client and polling
│   ├── index.html      # Dashboard markup
│   ├── style.css       # Dashboard styling
│   ├── package.json     # TypeScript build scripts
│   └── tsconfig.json
├── Containerfile
├── Caddyfile
└── podman-compose.yml
```

## Current limitations

- Deployment state is currently in memory and is lost when the backend restarts.
- Private repositories require an authentication design that is not implemented yet.
- The Podman socket/workspace sharing setup needs explicit operator configuration when Launchly runs inside a container.
- Caddy configuration is not automatically reloaded yet; validated live-state routing is the next integration step.
- The plan detector intentionally fails safely when evidence is ambiguous.
- Gemini is an optional fallback, not a replacement for deterministic detection or operator review.
