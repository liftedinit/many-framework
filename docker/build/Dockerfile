# syntax=docker/dockerfile:1.3-labs

# Build this container using ```docker build --ssh default -f docker/build/Dockerfile .```
# in the repo's root directory.

FROM rust:alpine

WORKDIR /app
COPY ../../ .

# Install ssh and git
RUN apk add --no-cache openssh-client git

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev

RUN <<EOR
    mkdir ~/.cargo
    echo "[net]" >> ~/.cargo/config.toml
    echo "git-fetch-with-cli = true" >> ~/.cargo/config.toml
    echo "retry = 2" >> ~/.cargo/config.toml
EOR

RUN --mount=type=ssh cargo build
