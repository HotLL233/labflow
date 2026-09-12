# syntax=docker/dockerfile:1

FROM node:20-bookworm-slim AS frontend-builder
WORKDIR /src

COPY frontend/package.json frontend/package-lock.json ./frontend/
RUN cd frontend && npm ci --ignore-scripts
COPY frontend ./frontend
RUN cd frontend && npm run build

FROM rust:1.97-bookworm AS backend-builder
WORKDIR /src

# Keep the Linux build free of the Windows tray stack while retaining image/PDF support.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY postgres-compat ./postgres-compat
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime
ENV WORKLOAD_SERVER_MODE=true \
    WORKLOAD_SERVER_PORT=8000 \
    WORKLOAD_DATA_DIR=/app/data

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --home-dir /app --create-home --shell /usr/sbin/nologin labflow

WORKDIR /app
COPY --from=backend-builder /src/target/release/workload-tool /app/workload-tool
COPY --from=frontend-builder /src/backend/static /app/static
COPY config.example.toml /app/config.example.toml
RUN mkdir -p /app/data && chown -R labflow:labflow /app

EXPOSE 8000
VOLUME ["/app/data"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=45s --retries=5 \
    CMD curl --fail --silent http://127.0.0.1:8000/api/health || exit 1

USER labflow
ENTRYPOINT ["/app/workload-tool"]
