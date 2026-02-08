# syntax=docker/dockerfile:1.7-labs
FROM rust:bookworm AS chef

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        cmake \
        pkg-config \
        libssl-dev \
    && cargo install cargo-chef --locked \
    && rm -rf /var/lib/apt/lists/*

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS cacher
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
COPY . .
COPY --from=cacher /app/target /app/target
RUN cargo build --release -p gateway-cp --locked

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/gateway-cp /usr/local/bin/gateway-cp
COPY config /app/config

ENV GATEWAY_CP_CONFIG=/app/config/control-plane.example.toml

EXPOSE 8081 9090

CMD ["/usr/local/bin/gateway-cp"]
