# select build image
FROM rust:1.75.0 AS builder

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

RUN cargo build -p glados-web --release

FROM ubuntu:22.04

RUN apt-get update

# copy build artifacts from build stage
COPY --from=builder ./glados/glados-web/assets/ /usr/bin/assets/
COPY --from=builder ./glados/target/release/glados-web /usr/bin/
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

ENV RUST_LOG=info
ENV GLADOS_WEB_ASSETS_PATH=/usr/bin/assets

ENTRYPOINT ["/usr/bin/glados-web"]
