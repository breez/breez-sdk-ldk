ARG USER=rgs
ARG VERSION=ad079707df0735128a64ae8bfbfe7908852ebcfb
ARG REPOSITORY=https://github.com/lightningdevkit/rapid-gossip-sync-server.git

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
