# Stage 1: build frontend and Rust application.
FROM rust:1.88-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev libglib2.0-dev libgtk-3-dev curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get update && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY postgres-compat/ ./postgres-compat/
COPY src/ ./src/
COPY frontend/ ./frontend/
COPY build.rs icon.ico ./

RUN cd frontend && npm ci && npm run build
RUN cargo build --release --features console

# Stage 2: application runtime. PostgreSQL is supplied by the deployment environment.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/workload-tool /app/workload-tool
COPY --from=builder /app/backend/static/ /app/static/

RUN mkdir -p /app/data

ENV WORKLOAD_DATA_DIR=/app/data
EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD curl -fsS http://localhost:8000/api/health || exit 1

CMD ["./workload-tool", "--server"]
