# syntax=docker/dockerfile:1
FROM rust:1.86-bookworm AS builder
WORKDIR /src
COPY backend/Cargo.toml ./backend/
COPY backend/src ./backend/src
RUN cargo build --manifest-path backend/Cargo.toml --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /src/backend/target/release/launchly /app/launchly
COPY frontend /app/frontend
EXPOSE 8080
ENV RUST_LOG=info LAUNCHLY_FRONTEND_DIR=/app/frontend
CMD ["/app/launchly"]
