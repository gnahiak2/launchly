# Launchly

Launchly deploys **vanilla static websites** from public Git repositories.

```text
public Git repository → clone → check root index.html → run Nginx → health check → live
```

It does not run repository build scripts and does not support Node, Python, Rust, Dockerfiles, or arbitrary application servers. A deployable repository simply contains `index.html` at its root, plus any HTML, CSS, JavaScript, images, or fonts it needs.

## How it works

1. Validate the repository URL.
2. Shallow-clone the repository.
3. Inspect the actual files and require root `index.html`.
4. Mount the clone read-only into `docker.io/nginxinc/nginx-unprivileged:alpine`.
5. Run the site with Podman using resource and privilege limits.
6. Require HTTP 200 on port 8080 before reporting `live`.

No generated Containerfiles, site image builds, Gemini calls, or repository commands are involved.

## Fedora Linux (recommended)

Fedora uses native Podman; do not use `podman machine`.

```bash
sudo dnf install -y git podman curl
sudo systemctl enable --now podman.socket

git clone https://github.com/gnahiak2/launchly.git
cd launchly
sudo mkdir -p /tmp/launchly
sudo chmod 777 /tmp/launchly

sudo podman build --network=host -t launchly:dev -f Containerfile .
sudo podman rm -f launchly 2>/dev/null || true
sudo podman run -d \
  --name launchly \
  --network host \
  --security-opt label=disable \
  -v /run/podman/podman.sock:/run/podman/podman.sock \
  -v /tmp/launchly:/tmp/launchly:rw \
  launchly:dev

curl http://127.0.0.1:8080/api/health
```

Expected response:

```json
{"status":"ok"}
```

The control-plane container needs the host Podman socket and shared `/tmp/launchly` directory because it clones sites and asks host Podman to run them. Do not mount this socket into deployed website containers.

If the build cannot resolve crates.io or the pnpm registry, verify the server itself has DNS and retain `--network=host`. If a deployment cannot resolve GitHub, verify the Launchly container uses host networking:

```bash
sudo podman inspect launchly --format '{{.HostConfig.NetworkMode}}'
```

## Docker

Launchly controls Podman, but the image is OCI-compatible and can be built/run with Docker for local UI/API development:

```bash
docker build -t launchly:dev -f Containerfile .
docker run --rm -p 8080:8080 launchly:dev
```

Actual deployments still require a Podman API and a shared workspace. For a Linux server, use native Podman as above. On macOS, install Podman and start its VM, then expose/configure the Podman connection before running Launchly; Docker Desktop alone is not the deployment runtime used by Launchly.

## Nginx reverse proxy and HTTPS

Install Nginx on Fedora and proxy the control plane:

```bash
sudo dnf install -y nginx
sudo setsebool -P httpd_can_network_connect 1
sudo nginx -t
sudo systemctl enable --now nginx
```

Use this server block in `/etc/nginx/conf.d/launchly.conf`:

```nginx
server {
    listen 80;
    server_name launchly.example.com;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_read_timeout 300s;
    }
}
```

Replace the hostname, point DNS at the server, then use either Fedora's Certbot package or your preferred ACME client. If `certbot --nginx` reports a missing Python module, reinstall the distro plugin rather than using a mixed pip/system Certbot installation:

```bash
sudo dnf reinstall -y certbot python3-certbot-nginx
sudo certbot --nginx -d launchly.example.com
```

## API

Inspect the real repository contents:

```bash
curl -X POST http://localhost:8080/api/plans \
  -H 'content-type: application/json' \
  -d '{"repository_url":"https://github.com/example/site.git"}'
```

Deploy and poll the returned ID:

```bash
curl -X POST http://localhost:8080/api/deployments \
  -H 'content-type: application/json' \
  -d '{"repository_url":"https://github.com/example/site.git"}'

curl http://localhost:8080/api/deployments/<deployment-id>
```

States are `queued`, `discovering`, `planning`, `starting`, `verifying`, `live`, or `failed`.

## Development

```bash
cargo fmt --manifest-path backend/Cargo.toml
cargo test --manifest-path backend/Cargo.toml
cd frontend && pnpm install && pnpm run build
```

The backend is Axum, the UI is plain HTML/CSS with a small TypeScript file, and deployments use direct argument-based Git/Podman commands. No shell interpolation is used for repository input.
