# select build image
FROM rust:1.70.0 AS builder

# create a new empty shell project
RUN USER=root cargo new --bin glados
WORKDIR /glados

ARG GIT_HASH=unknown
ENV GIT_HASH=$GIT_HASH

RUN apt-get update 
RUN apt-get install clang -y

RUN apt-get install -y --no-install-recommends ca-certificates
RUN update-ca-certificates

COPY . .

RUN cargo build -p glados-monitor --release

FROM ubuntu:20.04

RUN apt-get update
RUN apt-get install libssl-dev -y

# copy build artifacts from build stage
COPY --from=builder /glados/target/release/glados-monitor /usr/bin/
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/a

ENV RUST_LOG=info

ENTRYPOINT ["/usr/bin/glados-monitor"]
