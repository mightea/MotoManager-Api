# --- Build Stage ---
FROM rust:1.81-slim-bookworm as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests and pre-build dependencies to leverage Docker cache
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -f target/release/deps/moto_manager_api*

# Copy actual source code
COPY . .

# Build the application
RUN cargo build --release

# --- Runtime Stage ---
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Download and install PDFium (required for PDF previews)
# Using a stable release of PDFium binaries for Linux x64
RUN curl -L https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-linux-x64.tgz | tar -xz -C /usr/local/lib/ && \
    chmod +x /usr/local/lib/libpdfium.so

# Ensure the library path includes /usr/local/lib
ENV LD_LIBRARY_PATH=/usr/local/lib:${LD_LIBRARY_PATH}

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

# Expose the API port
EXPOSE 3001

# Create volumes for persistent data
VOLUME ["/app/data", "/app/cache"]

# Run the application
CMD ["/app/moto-manager-api"]
