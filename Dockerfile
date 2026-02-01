# Multi-stage Dockerfile for Progress Bot
# Optimized for size and security in Kubernetes deployments

# Stage 1: Builder
FROM rust:1.75-slim as builder

# Install build dependencies (PostgreSQL client libraries and build tools)
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Install diesel_cli for running migrations
RUN cargo install diesel_cli --no-default-features --features postgres

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY migrations ./migrations
COPY Rocket.toml ./

# Build for release
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies (only PostgreSQL client libraries)
RUN apt-get update && \
    apt-get install -y \
    libpq5 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user for security (non-root)
RUN useradd -m -u 1000 appuser

# Create app directory
WORKDIR /app

# Copy diesel_cli from builder (for migrations)
COPY --from=builder /usr/local/cargo/bin/diesel /usr/local/bin/diesel

# Copy compiled binaries from builder
COPY --from=builder /app/target/release/main /app/main
COPY --from=builder /app/target/release/reminders /app/reminders

# Copy migrations directory (needed for diesel migration run)
COPY --from=builder /app/migrations /app/migrations

# Copy Rocket configuration
COPY --from=builder /app/Rocket.toml /app/Rocket.toml

# Change ownership to app user
RUN chown -R appuser:appuser /app

# Switch to app user
USER appuser

# Expose port (default Rocket port)
EXPOSE 8800

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD ["/bin/sh", "-c", "curl -f http://localhost:${PORT:-8800}/ || exit 1"]

# Default command runs migrations then starts the web server
CMD diesel migration run && ./main
