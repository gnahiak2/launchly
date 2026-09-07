# Launchly development rules

Launchly is a deliberately small static-site deployment service. Keep the product focused:

```text
public Git repository → root index.html → unprivileged Nginx → HTTP health check
```

## Scope

- Support vanilla HTML, CSS, JavaScript, and static assets only.
- Require `index.html` in the repository root.
- Do not add generic framework detection, Gemini, repository build commands, generated site Containerfiles, or per-site image builds.
- Use the fully qualified image `docker.io/nginxinc/nginx-unprivileged:alpine`.
- Use Podman for deployed websites. Docker is useful for building/running the control-plane image locally, not a reason to add a second runtime abstraction.

## Safety

- Treat URLs and repository files as untrusted.
- Validate HTTP(S) URLs; reject credentials, query/fragment input, local hosts, and traversal.
- Invoke Git and Podman with argument arrays, never shell interpolation.
- Bound cloning, Podman, and health-check operations.
- Keep deployed sites read-only, unprivileged, capability-free, and resource-limited.
- Never mount the Podman socket into a deployed website.
- Capture bounded diagnostics before removing failed containers and workspaces.
- Do not expose secrets or commit changes.

## Architecture

Keep Axum handlers thin. Put URL/file detection in `detector.rs` and lifecycle work in `deploy.rs`. A site is live only after HTTP 200 on its published port. Retain successful workspaces because Nginx mounts them read-only; remove them on failure.

The control-plane container on Fedora uses host networking, `/run/podman/podman.sock`, and a shared `/tmp/launchly` directory. Document operational changes in `README.md`.

## Validation

Run the narrowest relevant checks after changes:

```bash
cargo fmt --manifest-path backend/Cargo.toml
cargo test --manifest-path backend/Cargo.toml
cd frontend && npm run build
```

Do not claim checks passed unless they were actually run. Prefer tests for URL validation, root `index.html` detection, command safety, cleanup, and health failures. Avoid tests that require a real Podman daemon, external Git, DNS, or TLS.
