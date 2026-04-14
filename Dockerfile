# syntax=docker/dockerfile:1

# --- Stage 1: planner (cargo-chef recipe) ---
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- Stage 2: cook (cache dependency builds) ---
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# --- Stage 3: build the binary ---
COPY . .
RUN cargo build --release --bin poker-manager

# --- Stage 4: minimal runtime image ---
FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/poker-manager /app/poker-manager
COPY --from=builder /app/static                       /app/static
COPY --from=builder /app/templates                    /app/templates

# /data is the persistent volume mount point for SQLite
ENV PORT=8080
ENV DATABASE_URL=sqlite:/data/poker.db?mode=rwc
ENV RUST_LOG=info

VOLUME ["/data"]
EXPOSE 8080

ENTRYPOINT ["/app/poker-manager"]
