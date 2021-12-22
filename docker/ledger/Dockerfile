# syntax=docker/dockerfile:1.3-labs

# Build this container using ```docker build --ssh default -f docker/build/Dockerfile .```
# in the repo's root directory.
FROM rust:latest as builder

RUN apt-get update && \
    apt-get -y upgrade && \
    apt-get -y install make openssh-client git jq

# Install build dependencies
RUN apt-get -y install musl-dev libssl-dev clang lld librocksdb-dev

RUN <<EOR
    mkdir ~/.cargo
    echo "[net]" >> ~/.cargo/config.toml
    echo "git-fetch-with-cli = true" >> ~/.cargo/config.toml
    echo "retry = 2" >> ~/.cargo/config.toml
EOR

WORKDIR /src-deps
COPY /rust-toolchain.toml /Cargo.* ./
COPY /http_proxy/Cargo.toml ./http_proxy/
COPY /kvstore/Cargo.toml ./kvstore/
COPY /ledger/Cargo.toml ./ledger/
COPY /omni/Cargo.toml ./omni/
COPY /omni-abci/Cargo.toml ./omni-abci/
COPY /omni-echo/Cargo.toml ./omni-echo/
COPY /omni-kvstore/Cargo.toml ./omni-kvstore/
COPY /omni-ledger/Cargo.toml ./omni-ledger/

# Separate building the dependencies so that it can get cached
# by docker.
# Loop through all packages to replace the main.rs and lib.rs with
# empty files.
RUN --mount=type=ssh <<EOR
  for p in *; do
    [ -d $p ] && {
      mkdir $p/src
      echo "fn main() {}" > $p/src/main.rs
    }
  done
  cargo build
EOR

WORKDIR /src
COPY / ./
RUN --mount=type=ssh cargo build


# ==== STAGE 2 ====
FROM debian:bullseye as ledger-runtime

RUN useradd -ms /bin/bash omni
RUN apt-get update && \
    apt-get -y upgrade && \
    apt-get -y install openssl

# Contains the ledger.db store.
VOLUME /persistent

STOPSIGNAL SIGTERM

USER omni
WORKDIR /app

EXPOSE 8000

# Generate the private key.
RUN openssl genpkey -algorithm Ed25519 -out /app/id.pem

COPY /docker/ledger/local/ledger.json /app/state.json
COPY --from=builder /src/target/debug/omni-ledger /app/omni-ledger

ENTRYPOINT ["./omni-ledger"]
CMD ["--addr", "0.0.0.0:8000", "--state", "/app/state.json", "--persistent", "/app/ledger.db", "--pem", "/app/id.pem"]
