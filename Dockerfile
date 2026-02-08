FROM rust:1.93-slim AS builder

WORKDIR /app
COPY . .

RUN cargo build --release --workspace

FROM debian:trixie-slim

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

HEALTHCHECK --interval=5s --timeout=5s --retries=5 \
    CMD bash -c "exec 3<>/dev/tcp/localhost/9001 && echo -e 'GET /ready HTTP/1.0\r\nHost: localhost\r\n\r\n' >&3 && head -1 <&3 | grep -q '200 OK'"

ENTRYPOINT ["simples3-server"]
