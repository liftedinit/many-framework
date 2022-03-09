FROM many_local/repo_build:latest as builder

# ==== STAGE 2 ====
FROM debian:bullseye as runtime

RUN useradd -ms /bin/bash many
RUN apt-get update && \
    apt-get -y upgrade && \
    apt-get -y install openssl curl netcat iputils-ping

STOPSIGNAL SIGTERM

USER many
WORKDIR /app

EXPOSE 8000 26658

# Generate the private key.
COPY --from=builder /src/target/release/many-abci /app/many-abci

ENTRYPOINT ["./many-abci"]
