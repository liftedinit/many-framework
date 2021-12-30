FROM omni_local/repo_build:latest as builder

# ==== STAGE 2 ====
FROM debian:bullseye as runtime

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
COPY --from=builder /src/target/release/omni-ledger /app/omni-ledger

ENTRYPOINT ["./omni-ledger"]
CMD ["--addr", "0.0.0.0:8000", "--state", "/app/state.json", "--persistent", "/app/ledger.db", "--pem", "/app/id.pem", "--verbose"]
