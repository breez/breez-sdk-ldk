ARG USER=rgs
ARG VERSION=05d5420539261156a70bb54429119d87f56371d5
ARG REPOSITORY=https://github.com/lightningdevkit/rapid-gossip-sync-server.git

# Waiting for https://github.com/lightningdevkit/rapid-gossip-sync-server/pull/103
ARG VERSION=10439b8151d0cb299a60f11ecd33313a095d44f8
ARG REPOSITORY=https://github.com/andrei-21/rapid-gossip-sync-server.git

FROM rust:1.86-slim-bookworm AS builder

WORKDIR /app/

RUN apt-get update -qq && \
    apt-get install -qq -y --no-install-recommends \
        ca-certificates \
        git

ARG VERSION
ARG REPOSITORY

RUN git init && \
    git remote add origin "$REPOSITORY" && \
    git fetch --depth 1 origin "$VERSION" && \
    git checkout FETCH_HEAD

RUN cargo install --path .

FROM debian:bookworm-slim AS final

ARG USER

RUN adduser --disabled-password \
            --home "/data" \
            --gecos "" \
            "$USER"

USER $USER

RUN mkdir -p "/data/.rgs"

VOLUME /data/.rgs/

ENV RAPID_GOSSIP_SYNC_SERVER_CACHES_PATH=/data/.rgs

COPY --from=builder /usr/local/cargo/bin/rapid-gossip-sync-server /usr/local/bin/

ENTRYPOINT ["rapid-gossip-sync-server"]
