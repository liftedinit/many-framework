# syntax=docker/dockerfile:1.3-labs

# Build this container using ```docker build --ssh default -f docker/build/Dockerfile .```
# in the repo's root directory.
FROM rust:latest as builder

RUN apt-get update && \
    apt-get upgrade && \
    apt-get -y install make openssh-client git

# Install build dependencies
RUN apt-get -y install musl-dev libssl-dev clang lld librocksdb-dev

COPY / /app
WORKDIR /app

RUN <<EOR
    mkdir ~/.cargo
    echo "[net]" >> ~/.cargo/config.toml
    echo "git-fetch-with-cli = true" >> ~/.cargo/config.toml
    echo "retry = 2" >> ~/.cargo/config.toml
EOR

RUN --mount=type=ssh cargo build


# ==== STAGE 2 ====
FROM debian:bullseye as ledger-runtime

EXPOSE 8000:8000

COPY --from=builder /app/target/debug/omni-ledger /app/target/debug/omni-ledger

