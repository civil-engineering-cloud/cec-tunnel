# 构建阶段
FROM rust:latest AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./

# 缓存依赖
RUN mkdir -p src/server src/client src/common \
    && echo "fn main() {}" > src/server/main.rs \
    && echo "fn main() {}" > src/client/main.rs \
    && touch src/common/mod.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src target/release/cec-tunnel* target/release/deps/cec_tunnel*

COPY src ./src
RUN cargo build --release

# 运行阶段
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/cec-tunnel-server /app/cec-tunnel-server

ENV RUST_LOG=info

EXPOSE 8080

CMD ["/app/cec-tunnel-server", "-p", "8080"]
