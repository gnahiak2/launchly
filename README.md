# Launchly

Launchly deploys simple websites from Git repositories.

```text
Git repository → inspect index.html → build Nginx image → run with Podman → health check → live
```

Launchly is designed for one self-hosted Linux machine. It intentionally focuses on static HTML/CSS/JavaScript websites instead of trying to be a general-purpose application orchestrator.

## Supported websites

A repository is deployable when it contains:

```text
index.html
```

The site may also contain CSS, JavaScript, images, fonts, and other static assets. Launchly does not run `npm`, Python, Rust, or repository-provided build scripts.

Every supported site gets the same simple plan:

```text
Framework:       Vanilla HTML
Package manager: None
Build command:   No build required
Server:          unprivileged Nginx
Port:            8080
```

## Fedora Linux deployment

Fedora uses native Podman and does not need `podman machine`.

Install dependencies:

```bash
sudo dnf install -y podman git curl
```

Clone Launchly and enter the repository:

```bash
git clone https://github.com/gnahiak2/launchly.git
cd launchly
```

Enable the rootful Podman API socket. Launchly uses this socket to build and run website containers:

```bash
sudo systemctl enable --now podman.socket
```

Build the image. `--network=host` avoids Fedora build-container DNS issues when downloading Rust or npm dependencies:

```bash
sudo podman build \
  --network=host \
  -t launchly:dev \
  -f Containerfile .
```

Prepare the shared deployment directory:

```bash
sudo mkdir -p /tmp/launchly
sudo chmod 777 /tmp/launchly
```

Run Launchly with host networking, the Podman socket, and the shared workspace:

```bash
sudo podman rm -f launchly 2>/dev/null || true

sudo podman run -d \
  --name launchly \
  --network host \
  --security-opt label=disable \
  -v /run/podman/podman.sock:/run/podman/podman.sock \
  -v /tmp/launchly:/tmp/launchly:rw \
  launchly:dev
```

`--network host` lets Launchly resolve Git hosts and health-check deployed websites through the host network. The Podman socket is only mounted into the trusted Launchly control-plane container; never mount it into deployed websites.

Check Launchly:

```bash
curl http://127.0.0.1:8080/api/health
```

Expected:

```json
{"status":"ok"}
```

## Fedora DNS troubleshooting

If the image build cannot resolve `index.crates.io`:

```bash
getent hosts index.crates.io
curl -I https://index.crates.io/config.json
```

Retry the build with host networking:

```bash
sudo podman build --network=host -t launchly:dev -f Containerfile .
```

If Git cannot resolve `github.com` during a deployment, confirm the Launchly container is using host networking:

```bash
sudo podman inspect launchly --format '{{.HostConfig.NetworkMode}}'
```

It should report:

```text
host
```

## Nginx reverse proxy and HTTPS

Launchly listens on `127.0.0.1:8080` from the host-network container. Put Nginx in front of it:

```nginx
server {
    listen 80;
    listen [::]:80;
    server_name launchly.example.com;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_read_timeout 300s;
    }
}
```

On Fedora:

```bash
sudo dnf install -y nginx certbot python3-certbot-nginx
sudo setsebool -P httpd_can_network_connect 1
sudo nginx -t
sudo systemctl enable --now nginx
sudo certbot --nginx -d launchly.example.com
```

Replace `launchly.example.com` with a real domain whose DNS points to the server.

## API

Inspect a repository. This clones the repository temporarily and checks its actual contents:

```bash
curl -X POST http://localhost:8080/api/plans \
  -H 'content-type: application/json' \
  -d '{"repository_url":"https://github.com/gnahiak2/gnahiak2.github.io.git"}'
```

Create a deployment:

```bash
curl -X POST http://localhost:8080/api/deployments \
  -H 'content-type: application/json' \
  -d '{
    "repository_url":"https://github.com/gnahiak2/gnahiak2.github.io.git",
    "domain":"example.com"
  }'
```

Check deployment status:

```bash
curl http://localhost:8080/api/deployments/<deployment-id>
```

Deployment states are:

```text
queued → discovering → planning → building → starting → verifying → live
```

Failures include a diagnostic message. Failed workspaces, images, and containers are cleaned up.

## Local development

Run the backend:

```bash
cargo run --manifest-path backend/Cargo.toml
```

Run tests:

```bash
cargo test --manifest-path backend/Cargo.toml
```

Format Rust:

```bash
cargo fmt --manifest-path backend/Cargo.toml
```

Build the TypeScript dashboard:

```bash
cd frontend
npm install
npm run build
```

## Project structure

```text
backend/src/api.rs       HTTP handlers
backend/src/deploy.rs    Git, Podman, health checks, cleanup
backend/src/detector.rs  index.html detection and URL validation
backend/src/models.rs    API and deployment models
frontend/                lightweight TypeScript dashboard
Containerfile            backend and frontend image build
Caddyfile                optional reverse-proxy configuration
```

## Safety boundaries

- Repository URLs must be public HTTP(S) URLs without credentials, fragments, query strings, localhost, or path traversal.
- Git operations have a five-minute timeout.
- Repository commands are never executed.
- Website images run with an unprivileged Nginx user.
- Website containers use read-only filesystems, temporary runtime filesystems, dropped capabilities, `no-new-privileges`, CPU/memory/PID limits, and bridge networking.
- A deployment is only `live` after an HTTP health check succeeds.
- Secrets are excluded from static build contexts where possible.
- The Podman socket is only used by the trusted Launchly control plane.
