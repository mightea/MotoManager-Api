# --- Build Stage ---
FROM rust:1.94-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests and pre-build dependencies to leverage Docker cache
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
# Provide a dummy DATABASE_URL for dependency compilation if needed
RUN DATABASE_URL=sqlite:db.sqlite cargo build --release
RUN rm -f target/release/deps/moto_manager_api*

# Copy actual source code
COPY . .

# Build the application
# The compile-time sqlx macros are verified against this schema, so apply ALL
# migrations in order (globbing keeps it drift-proof as new ones are added) —
# not just an early subset, which would let dropped/renamed columns pass here
# yet fail at runtime against the real database.
RUN touch db.sqlite && \
    for f in migrations/*.sql; do echo "Applying $f"; sqlite3 db.sqlite < "$f"; done && \
    DATABASE_URL=sqlite:db.sqlite cargo build --release

# --- Runtime Stage ---
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies:
#  - ca-certificates: HTTPS to allowlisted image sources (BMWBike import)
#  - libssl3: sqlx native-tls
#  - libstdc++6: required by libpdfium.so (present in the base image today,
#    but named explicitly so a slimmer base can't silently break PDF parsing)
#  - curl: pdfium download below + container healthchecks
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libstdc++6 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Download and install PDFium — required for document previews AND the
# invoice import's text extraction (a missing library is a hard error there).
# Arch-aware: TARGETARCH is set automatically by BuildKit (amd64/arm64);
# default to x64 for legacy builders.
ARG TARGETARCH
RUN PDFIUM_ARCH=$([ "$TARGETARCH" = "arm64" ] && echo arm64 || echo x64) && \
    curl -fsSL "https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-linux-${PDFIUM_ARCH}.tgz" \
    | tar -xz -C /usr/local/lib/ --strip-components=1 lib/libpdfium.so

# Ensure the library path includes /usr/local/lib
ENV LD_LIBRARY_PATH=/usr/local/lib

# Copy the binary from the builder
COPY --from=builder /app/target/release/moto-manager-api /app/moto-manager-api
# Copy migrations (required for startup migration)
COPY --from=builder /app/migrations /app/migrations

# Set default environment variables
ENV DATABASE_URL=sqlite:/app/data/db.sqlite
ENV PORT=3001
ENV DATA_DIR=/app/data
ENV CACHE_DIR=/app/cache
ENV RUST_LOG=info
ENV ENABLE_REGISTRATION=false
# Automatic backups: DB snapshot (VACUUM INTO) + tar.gz of images/ & documents/,
# written to DATA_DIR/backups and monitorable/triggerable by admins in the webapp.
ENV BACKUP_ENABLED=true
ENV BACKUP_INTERVAL_HOURS=24
ENV BACKUP_KEEP=14
# Invoice-import LLM (optional): set LLM_BASE_URL (e.g. http://10.0.0.2:8542/v1),
# LLM_MODEL and LLM_API_KEY at deploy time. Unset = deterministic parser only.

# Expose the API port
EXPOSE 3001

# Create volumes for persistent data
VOLUME ["/app/data", "/app/cache"]

# Run the application
CMD ["/app/moto-manager-api"]
