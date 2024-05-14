# select build image
FROM rust:1.76.0 AS builder

# create a new empty shell project
RUN USER=root cargo new --bin glados
WORKDIR /glados

ARG GIT_HASH=unknown
ENV GIT_HASH=$GIT_HASH

RUN apt-get update \
    && apt-get install -y --no-install-recommends clang ca-certificates \
    && update-ca-certificates

ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

COPY . .

RUN cargo build -p glados-monitor --release

FROM ubuntu:22.04

RUN apt-get update

# copy build artifacts from build stage
COPY --from=builder /glados/target/release/glados-monitor /usr/bin/
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

ENV RUST_LOG=info

ENTRYPOINT ["/usr/bin/glados-monitor"]
