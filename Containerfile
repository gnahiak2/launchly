# syntax=docker/dockerfile:1
FROM rust:1.88-bookworm AS builder
WORKDIR /src
COPY backend/Cargo.toml ./backend/
COPY backend/src ./backend/src
RUN cargo build --manifest-path backend/Cargo.toml --release

FROM node:22-bookworm-slim AS frontend-builder
WORKDIR /src/frontend
COPY frontend/package.json frontend/tsconfig.json ./
RUN npm install
COPY frontend/app.ts ./
RUN npm run build

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates git podman && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /src/backend/target/release/launchly /app/launchly
COPY frontend/index.html frontend/style.css /app/frontend/
COPY --from=frontend-builder /src/frontend/dist/app.js /app/frontend/app.js
EXPOSE 8080
ENV RUST_LOG=info LAUNCHLY_FRONTEND_DIR=/app/frontend CONTAINER_HOST=unix:///run/podman/podman.sock
CMD ["/app/launchly"]
