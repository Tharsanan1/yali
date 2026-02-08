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
RUN cargo chef cook --recipe-path recipe.json

FROM chef AS builder
COPY . .
COPY --from=cacher /app/target /app/target
RUN cargo test -p gateway-it --no-run --locked

CMD ["cargo", "test", "-p", "gateway-it"]
