# AGENTS.md

## Project: Launchly

Launchly is a self-hosted deployment platform built around one idea:

> Give Launchly a Git repository. Launchly figures out the rest.

Launchly inspects a repository, detects how it should be built and run, creates an explainable deployment plan, builds it with Podman, runs it in an isolated container, verifies it is healthy, and exposes it with Caddy and HTTPS.

The goal is a simple, reliable, zero-config deployment experience—not a Kubernetes-sized orchestration platform.

---

## Product Principles

Launchly should feel like:

```text
Git repository
      |
      v
   Discover
      |
      v
     Build
      |
      v
      Run
      |
      v
    Verify
      |
      v
     Alive
```

When making a design or implementation decision, prefer the option that is:

1. **Simple** — few moving parts, minimal configuration, easy local operation.
2. **Reliable** — explicit state transitions, actionable failures, safe cleanup and retries.
3. **Explainable** — users can see what was detected, what will run, and why.
4. **Secure by default** — deployments are isolated and untrusted repository input is handled carefully.
5. **Focused** — solve single-host application deployment well; do not introduce cluster-management abstractions without a concrete need.

Avoid speculative extensibility, provider abstractions, controller loops, CRDs, or Kubernetes-inspired complexity unless the project explicitly grows to require them.

---

## Technology Direction

- **Backend:** Rust with Axum.
- **Frontend:** lightweight HTML and CSS; use TypeScript only where client-side behavior materially improves the interface.
- **Container runtime:** Podman.
- **Reverse proxy and TLS:** Caddy.
- **Primary environment:** self-hosted, typically one machine under the operator's control.

Follow established dependencies and patterns already present in the repository. Do not add a dependency where a small standard-library or existing-project solution is clearer.

---

## System Model

A deployment should have a clear, inspectable lifecycle:

1. **Receive repository source** — validate URL/input and create an isolated working directory.
2. **Discover** — inspect repository signals such as manifests, lockfiles, framework files, Dockerfiles, and configuration.
3. **Plan** — produce a concrete deployment plan with detected language/framework, package manager, build command, start command, port, environment requirements, and confidence or rationale.
4. **Build** — build an application image using Podman, preserving useful build logs.
5. **Run** — start the image in an isolated Podman container with explicit networking, environment, volumes, and naming.
6. **Verify** — check readiness/health through the actual application path, with bounded retries and useful diagnostics on failure.
7. **Expose** — configure Caddy to route the desired hostname to the healthy deployment and obtain HTTPS where applicable.
8. **Maintain state** — record enough deployment state to show status, logs, configuration, and failure reason; clean up abandoned resources safely.

Do not mark a deployment successful merely because a build completed or a container started. It is successful only after its health verification succeeds.

---

## Backend Guidance

### Axum and Rust

- Keep HTTP handlers thin: parse/validate input, call an application service, and translate known errors to HTTP responses.
- Put discovery, planning, Podman execution, Caddy configuration, health checking, and persistence behind focused modules/services rather than embedding shell logic in handlers.
- Prefer typed domain structures and explicit deployment states over stringly typed maps or implicit booleans.
- Use `Result` and domain-specific errors; preserve command stderr, exit codes, and relevant context in errors and logs.
- Bound all external work: subprocesses, Git operations, builds, health checks, and network requests need timeouts.
- Never block the async runtime with long-running synchronous work. Run blocking subprocess/file work appropriately for the runtime and existing project conventions.
- Treat repository contents, Git URLs, deployment environment variables, hostnames, and command output as untrusted input.

### State and errors

Use clear, user-facing states such as `queued`, `discovering`, `planning`, `building`, `starting`, `verifying`, `live`, and `failed` if they match the existing model. State transitions must be deliberate and failures should record:

- the phase that failed;
- a concise human-readable explanation;
- safe diagnostic details, including relevant command output;
- timestamps and any cleanup outcome.

Never expose secrets in API responses, logs, browser output, or error strings.

---

## Detection and Planning

Detection is a core Launchly feature, not a hidden implementation detail.

- Prefer strong repository signals: lockfiles/manifests, established framework config, explicit process files, and conventional project structure.
- Resolve conflicting signals predictably and expose the decision rationale.
- Prefer deterministic rules over opaque heuristics.
- Keep detected values separate from user overrides, so the UI/API can explain both.
- Fail safely when detection is ambiguous or unsupported. Give the user the signals found and the smallest useful configuration/override needed to continue.
- Do not execute repository-provided commands during discovery merely to identify a framework.

A deployment plan should be serializable, reviewable, and reusable by build/run steps. It is the contract between discovery and execution.

---

## Podman, Git, and Host Safety

Launchly executes untrusted application code. Treat host safety as a first-class concern.

- Invoke Podman and Git without shell interpolation whenever possible; pass arguments directly.
- Never construct `sh -c` commands from repository, user, URL, hostname, or environment input.
- Validate repository URLs, image/container names, hostnames, port values, filesystem paths, and environment-variable names before use.
- Use per-deployment working directories and container names; prevent path traversal and collisions.
- Avoid privileged containers, host networking, host PID/IPC namespaces, and broad host mounts by default.
- Mount only explicit, required paths and use read-only mounts where possible.
- Keep secrets out of image layers, build logs, process arguments, and persisted deployment plans.
- Ensure failed deployments clean up temporary containers, networks, files, and partial proxy configuration without deleting resources belonging to another deployment.
- Make retry/redeploy behavior idempotent where possible.

Do not assume a repository is trustworthy just because its URL is public or supplied by an authenticated user.

---

## Caddy and HTTPS

- Only route traffic to a deployment after health verification succeeds.
- Validate hostnames/domain names before generating Caddy configuration.
- Generate configuration from structured values, not string concatenation of untrusted input.
- Preserve a valid proxy configuration if a new deployment fails; avoid taking down live routes during updates.
- Reload or validate Caddy configuration using its supported mechanisms and report configuration failures clearly.
- Make local-development HTTP and production HTTPS behavior explicit rather than silently masking certificate or domain errors.

---

## Frontend Guidance

The frontend should make deployment status understandable at a glance.

- Favor server-rendered HTML and semantic forms/content for core flows.
- Use CSS for layout and visual hierarchy; avoid adding a heavy frontend framework without a demonstrated need.
- Use TypeScript only for focused interactive behavior, such as log streaming, polling, progressive status updates, or copy controls.
- Show the deployment plan before or alongside execution: detected stack, commands, port, domain, and relevant rationale.
- Surface failures with the failed phase, a concise explanation, and safely redacted logs/details.
- Keep accessibility in mind: semantic HTML, labels, keyboard operation, visible focus, and non-color-only status indicators.
- Escape all user- or repository-derived content before rendering it.

---

## Testing and Validation

For every change:

1. Run the narrowest relevant formatter, test, or check first.
2. Run broader project validation when it is practical and relevant.
3. Add or update tests when changing discovery rules, planning, deployment state transitions, command construction, proxy configuration, or health-check behavior.
4. Do not claim a command passed unless it was actually run successfully.

Prioritize tests that cover:

- repository detection and conflicting signals;
- deterministic plan generation;
- invalid or hostile inputs;
- command argument construction without shell injection;
- deployment state transitions and cleanup;
- health-check success, timeout, and failure paths;
- Caddy configuration generation and rollback behavior.

Tests must not require a production Caddy instance, Podman daemon, external network, or real TLS unless they are explicitly integration tests and are safely isolated. Prefer abstractions or fixtures for command runners and external services.

---

## Change Discipline

- Read surrounding code before changing it and follow existing module, error-handling, naming, and test conventions.
- Keep changes focused on the requested behavior; do not refactor unrelated code opportunistically.
- Do not remove, overwrite, or revert user changes unless explicitly asked.
- Do not commit, create branches, or alter Git history unless explicitly asked.
- Document user-visible configuration, environment variables, deployment behavior, and operational requirements when they change.
- Do not add comments that restate code; document non-obvious safety, lifecycle, or operational decisions instead.

When uncertain, choose the implementation that makes a deployment's behavior easiest for an operator to inspect, understand, and recover from.
