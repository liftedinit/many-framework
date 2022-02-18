FROM omni_local/repo_build:latest as builder

# ==== STAGE 2 ====
FROM debian:bullseye as runtime

RUN useradd -ms /bin/bash omni
RUN apt-get update && \
    apt-get -y upgrade && \
    apt-get -y install openssl curl netcat iputils-ping

# Contains the ledger.db store.
VOLUME /persistent

STOPSIGNAL SIGTERM

USER omni
WORKDIR /app

EXPOSE 8000 26658

# Generate the private key.
RUN openssl genpkey -algorithm Ed25519 -out /app/id.pem

COPY --from=builder /src/target/release/omni-abci /app/omni-abci

ENTRYPOINT ["./omni-abci"]
