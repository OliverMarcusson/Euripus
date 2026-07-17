FROM docker.io/library/rust:1.94-bookworm AS builder
WORKDIR /app

COPY apps/server/Cargo.toml apps/server/Cargo.toml
COPY apps/server/src apps/server/src
COPY apps/server/migrations apps/server/migrations

RUN cargo build --release --manifest-path apps/server/Cargo.toml

FROM docker.io/library/node:22-bookworm-slim AS pi-runtime
RUN npm install --ignore-scripts --no-audit --no-fund --prefix /opt/pi @earendil-works/pi-coding-agent@0.80.7

FROM docker.io/library/debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=pi-runtime /usr/local/ /usr/local/
COPY --from=pi-runtime /opt/pi /opt/pi
ENV PATH="/opt/pi/node_modules/.bin:${PATH}" \
    PI_SKIP_VERSION_CHECK=1

WORKDIR /app
COPY --from=builder /app/apps/server/target/release/euripus-server /usr/local/bin/euripus-server
COPY apps/server/migrations /app/migrations

EXPOSE 8080
CMD ["euripus-server"]
