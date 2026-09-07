FROM docker.io/library/rust:1.88-bookworm AS backend
WORKDIR /src
COPY backend/Cargo.toml backend/Cargo.lock ./backend/
COPY backend/src ./backend/src
RUN cargo build --locked --release --manifest-path backend/Cargo.toml

FROM docker.io/library/node:22-bookworm-slim AS frontend
WORKDIR /src
COPY frontend/package.json frontend/tsconfig.json frontend/app.ts ./
RUN corepack enable && pnpm install --no-frozen-lockfile && pnpm run build
COPY frontend/index.html frontend/style.css ./
RUN cp index.html style.css dist/

FROM docker.io/library/debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates git podman && rm -rf /var/lib/apt/lists/*
COPY --from=backend /src/backend/target/release/launchly /usr/local/bin/launchly
COPY --from=frontend /src/dist /opt/launchly/frontend
ENV LAUNCHLY_FRONTEND_DIR=/opt/launchly/frontend
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/launchly"]
