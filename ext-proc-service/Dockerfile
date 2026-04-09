FROM rust:1.82-bookworm AS builder

RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./
COPY proto/ proto/

# Create a dummy main.rs to cache dependency builds
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source and rebuild
COPY src/ src/
RUN touch src/main.rs && cargo build --release

# --- Runtime stage ---
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ext-proc-service /usr/local/bin/ext-proc-service

EXPOSE 50051
ENTRYPOINT ["ext-proc-service"]
CMD ["--config", "/etc/ext-proc/config.yaml"]
