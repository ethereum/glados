# Prepare base image
FROM rust:1.85 AS chef
WORKDIR /glados

ARG GIT_HASH=unknown
ENV GIT_HASH=$GIT_HASH

RUN apt-get update \
    && apt-get install -y --no-install-recommends clang ca-certificates \
    && update-ca-certificates \
    && cargo install cargo-chef --locked

ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt


# Compute dependencies
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Build dependencies
FROM chef AS builder
COPY --from=planner /glados/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build Glados
COPY . .
RUN cargo build -p glados-audit --release

# Build runtime
FROM ubuntu:22.04
RUN apt-get update

# copy build artifacts from build stage
COPY --from=builder /glados/target/release/glados-audit /usr/bin/
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

ENV RUST_LOG=info

ENTRYPOINT ["/usr/bin/glados-audit"]
