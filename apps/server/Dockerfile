FROM docker.io/library/rust:1.94-bookworm AS builder
WORKDIR /app

COPY apps/server/Cargo.toml apps/server/Cargo.toml
COPY apps/server/src apps/server/src
COPY apps/server/migrations apps/server/migrations

RUN cargo build --release --manifest-path apps/server/Cargo.toml

FROM docker.io/library/debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/apps/server/target/release/euripus-server /usr/local/bin/euripus-server
COPY apps/server/migrations /app/migrations

EXPOSE 8080
CMD ["euripus-server"]
