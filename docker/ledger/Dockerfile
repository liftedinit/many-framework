FROM omni_local/repo_build:latest as builder

# ==== STAGE 2 ====
FROM debian:bullseye as runtime

RUN useradd -ms /bin/bash omni
RUN apt-get update && \
    apt-get -y upgrade && \
    apt-get -y install openssl

# Contains the ledger.db store.
VOLUME /persistent
VOLUME /state

STOPSIGNAL SIGTERM

USER omni
WORKDIR /app

EXPOSE 8000

COPY --from=builder /src/target/release/omni-ledger /app/omni-ledger

ENTRYPOINT ["./omni-ledger"]
CMD ["--addr", "0.0.0.0:8000", "-v", "-v", "--state", "/state/state.json", "--persistent", "/persistent/ledger.db", "--pem", "/state/id.pem"]
