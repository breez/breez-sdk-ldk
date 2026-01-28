ARG USER=vss
ARG VERSION=fe94e106788423579e13cd3345d1f589bbc4ca66
ARG REPOSITORY=https://github.com/lightningdevkit/vss-server.git

FROM rust:1.86-slim-bookworm AS builder

WORKDIR /app

RUN apt-get update -qq && \
    apt-get install -qq -y --no-install-recommends \
        ca-certificates \
        git \
        libssl-dev \
        pkg-config

ARG VERSION
ARG REPOSITORY

RUN git init && \
    git remote add origin "$REPOSITORY" && \
    git fetch --depth 1 origin "$VERSION" && \
    git checkout FETCH_HEAD

RUN cargo install --path rust/server --no-default-features --features sigs --locked

FROM debian:bookworm-slim AS final

ARG USER

RUN apt-get update -qq && \
    apt-get install -qq -y --no-install-recommends \
        libssl3

RUN adduser --disabled-password \
            --home "/data" \
            --gecos "" \
            "$USER"

USER $USER

COPY --from=builder /usr/local/cargo/bin/vss-server /usr/local/bin/

ENTRYPOINT ["vss-server"]
