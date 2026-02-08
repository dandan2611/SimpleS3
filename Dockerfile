FROM rust:1.93-slim AS builder

WORKDIR /app
COPY . .

RUN cargo build --release --workspace

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/simples3-server /usr/local/bin/simples3-server
COPY --from=builder /app/target/release/simples3-cli /usr/local/bin/simples3-cli

RUN mkdir -p /data /metadata

ENV SIMPLES3_BIND=0.0.0.0:9000
ENV SIMPLES3_DATA_DIR=/data
ENV SIMPLES3_METADATA_DIR=/metadata
ENV SIMPLES3_HOSTNAME=s3.localhost
ENV SIMPLES3_REGION=us-east-1
ENV SIMPLES3_LOG_LEVEL=info

EXPOSE 9000
EXPOSE 9001

HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -q -O- http://localhost:9001/health || exit 1

ENTRYPOINT ["simples3-server"]
