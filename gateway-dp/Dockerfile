FROM rust:1.75-bookworm AS builder

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY gateway-dp/Cargo.toml gateway-dp/Cargo.toml
COPY gateway-cp/Cargo.toml gateway-cp/Cargo.toml
COPY gateway-proto/Cargo.toml gateway-proto/Cargo.toml
COPY policy-runtime/Cargo.toml policy-runtime/Cargo.toml
COPY policy-sdk/Cargo.toml policy-sdk/Cargo.toml
COPY gateway-it/Cargo.toml gateway-it/Cargo.toml

COPY gateway-dp/src gateway-dp/src
COPY gateway-proto gateway-proto
COPY config config

RUN cargo build -p gateway-dp --release

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/gateway-dp /usr/local/bin/gateway-dp
COPY config /app/config

ENV GATEWAY_DP_CONFIG=/app/config/gateway.example.toml

EXPOSE 8080

CMD ["/usr/local/bin/gateway-dp"]
